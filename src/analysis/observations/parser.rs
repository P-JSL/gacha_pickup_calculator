use super::*;

/// A cohort's first interval must start at zero pity. Mixed banner classes,
/// missing initial pity, or already-in-progress cycles require additional
/// entry-time information and must be excluded before importing.
pub fn parse_observation_csv(text: &str, hard: usize) -> Result<ObservationDataset, String> {
    if hard == 0 || hard > 500 {
        return Err("유효하지 않은 최고 등급 천장입니다.".into());
    }
    if text.len() > 8 * 1024 * 1024 {
        return Err("CSV는 8 MiB 이하만 허용합니다.".into());
    }
    // Stream validated rows rather than first allocating a Vec for every
    // non-empty line. The 200,000-row limit must bound peak memory *before*
    // processing untrusted input (even when the file is under 8 MiB).
    let mut lines = text.lines()
        .enumerate()
        .map(|(i, s)| (i + 1, s.trim().trim_start_matches('\u{feff}')))
        .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'));
    let (_, header) = lines.next()
        .ok_or("CSV에 헤더와 최소 한 건의 데이터가 필요합니다.")?;
    let mut data_lines = lines.peekable();
    if data_lines.peek().is_none() {
        return Err("CSV에 헤더와 최소 한 건의 데이터가 필요합니다.".into());
    }
    let delimiter = if header.contains('\t') { '\t' } else { ',' };
    // Only a small, explicit schema is supported. Cap the number of fields
    // before allocating user-controlled header strings; previously duplicate
    // detection also required O(columns²) comparisons.
    const MAX_COLUMNS: usize = 64;
    let columns: Vec<String> = header.split(delimiter)
        .take(MAX_COLUMNS + 1)
        .map(|s| s.trim().trim_matches('"').to_ascii_lowercase())
        .collect();
    if columns.len() > MAX_COLUMNS {
        return Err("CSV 열은 최대 64개까지만 허용합니다.".into());
    }
    let mut seen = HashSet::with_capacity(columns.len());
    if columns.iter().any(|name| !seen.insert(name.as_str())) {
        return Err("CSV 헤더에 중복된 열 이름이 있습니다.".into());
    }
    let find = |field: &str| -> Option<usize> {
        columns.iter().position(|col| col == field)
    };
    let has_duration = find("duration").is_some() && find("event").is_some();
    let has_risk = find("pity").is_some()
        && find("at_risk").is_some() && find("events").is_some()
        && find("censored").is_some();
    let has_completed = find("pity").is_some()
        && (find("count").is_some() || find("events").is_some());
    // Never silently reinterpret an incomplete risk-set schema as a
    // completed-only histogram: censoring would be lost and bias the CDF.
    if (find("at_risk").is_some() || find("censored").is_some()) && !has_risk {
        return Err("위험집합 자료는 pity,at_risk,events,censored 네 개 열이 모두 필요합니다.".into());
    }
    if (find("duration").is_some() || find("event").is_some()) && !has_duration {
        return Err("개별 관측 자료는 duration,event 두 개 열이 모두 필요합니다.".into());
    }
    if (has_duration && find("pity").is_some())
        || (has_risk && (has_duration || find("count").is_some())) {
        return Err("서로 다른 관측 형식을 하나의 CSV 헤더에 섞을 수 없습니다.".into());
    }
    let quality = if has_duration {
        ObservationQuality::IndividualRightCensored
    } else if has_risk {
        ObservationQuality::AggregatedRightCensored
    } else if has_completed {
        ObservationQuality::CompletedOnly
    } else {
        return Err("헤더: duration,event / pity,at_risk,events,censored / pity,count 중 하나여야 합니다.".into());
    };
    let mut events = vec![0_u64; hard + 1];
    let mut censored = vec![0_u64; hard + 1];
    let mut direct_risk = vec![None; hard + 1];
    let mut observations = 0_u64;
    let mut last_pity = 0_usize;
    let mut previous_pity = 0_usize;
    let mut row_count = 0_usize;
    for (line_no, line) in data_lines {
        row_count += 1;
        if row_count > 200_000 {
            return Err("한 번에 최대 200,000개 행만 가져올 수 있습니다.".into());
        }
        let fields: Vec<&str> = line.split(delimiter)
            .map(|s| s.trim().trim_matches('"'))
            .collect();
        if fields.len() != columns.len() {
            return Err(format!("{line_no}행: 열 개수가 헤더와 다릅니다."));
        }
        let number = |name: &str| -> Result<u64, String> {
            let i = find(name).ok_or_else(|| format!("필수 열 누락: {name}"))?;
            fields[i].parse::<u64>().map_err(|_| {
                format!("{line_no}행 {name}: 음수가 아닌 정수여야 합니다.")
            })
        };
        if quality == ObservationQuality::IndividualRightCensored {
            let duration = usize::try_from(number("duration")?)
                .map_err(|_| format!("{line_no}행: duration이 플랫폼 정수 범위를 초과했습니다."))?;
            let event = fields[find("event").expect("validated header")].to_ascii_lowercase();
            let event = match event.as_str() {
                "1" | "true" => true,
                "0" | "false" => false,
                _ => return Err(format!("{line_no}행: event는 0 또는 1이어야 합니다.")),
            };
            if duration > hard || (event && duration == 0) ||
                (!event && duration >= hard) {
                return Err(format!("{line_no}행: 천장 범위 또는 중단 시점이 잘못됐습니다."));
            }
            if event {
                events[duration] = events[duration].checked_add(1)
                    .ok_or("관측 횟수가 초과했습니다.")?;
            } else {
                censored[duration] = censored[duration].checked_add(1)
                    .ok_or("중단 횟수가 초과했습니다.")?;
            }
            observations = observations.checked_add(1).ok_or("관측 횟수가 초과했습니다.")?;
            last_pity = last_pity.max(duration);
        } else {
            let pity = usize::try_from(number("pity")?)
                .map_err(|_| format!("{line_no}행: pity가 플랫폼 정수 범위를 초과했습니다."))?;
            if !(1..=hard).contains(&pity) || pity <= previous_pity {
                return Err(format!("{line_no}행: pity는 1~{hard}의 오름차순이어야 합니다."));
            }
            if quality == ObservationQuality::AggregatedRightCensored && pity != previous_pity + 1 {
                return Err(format!("{line_no}행: 위험집합 CSV는 pity가 1부터 연속되어야 합니다."));
            }
            previous_pity = pity;
            last_pity = pity;
            let d = if find("events").is_some() { number("events")? }
                else { number("count")? };
            events[pity] = d;
            if quality == ObservationQuality::AggregatedRightCensored {
                let risk = number("at_risk")?;
                let c = number("censored")?;
                if risk == 0 || d.checked_add(c).is_none_or(|sum| sum > risk) {
                    return Err(format!("{line_no}행: 위험집합보다 획득·중단 수가 많거나 위험집합이 0입니다."));
                }
                direct_risk[pity] = Some(risk);
                censored[pity] = c;
            } else {
                observations = observations.checked_add(d).ok_or("관측 횟수가 초과했습니다.")?;
            }
        }
    }
    if quality == ObservationQuality::AggregatedRightCensored {
        observations = direct_risk[1].ok_or("위험집합 첫 행이 없습니다.")?;
        for pity in 2..=last_pity {
            let prev = direct_risk[pity - 1].ok_or("연속된 위험집합 행이 없습니다.")?;
            let expected = prev - events[pity - 1] - censored[pity - 1];
            if direct_risk[pity] != Some(expected) {
                return Err(format!("pity {pity}: 다음 위험집합은 직전 위험집합에서 획득·중단을 뺀 값이어야 합니다."));
            }
        }
        let remaining = direct_risk[last_pity].unwrap_or(0)
            - events[last_pity] - censored[last_pity];
        if remaining != 0 {
            // At the observed endpoint, unresolved active runs are right-censored.
            censored[last_pity] += remaining;
        }
    }
    let total_events = events.iter().try_fold(0_u64, |n, &v| n.checked_add(v))
        .ok_or("누적 이벤트 수가 정수 범위를 초과했습니다.")?;
    let total_censored = censored.iter().try_fold(0_u64, |n, &v| n.checked_add(v))
        .ok_or("누적 검열 수가 정수 범위를 초과했습니다.")?;
    if observations == 0 {
        return Err("적어도 하나의 관측 구간이 필요합니다.".into());
    }
    let observed = if quality == ObservationQuality::CompletedOnly {
        total_events
    } else {
        observations
    };
    if quality == ObservationQuality::CompletedOnly && observed == 0 {
        return Err("완료된 획득 기록이 없습니다.".into());
    }
    let mut remaining = observed.saturating_sub(censored[0]);
    let mut survival = 1.0_f64;
    let mut greenwood_sum = 0.0_f64;
    let mut rows = Vec::with_capacity(last_pity);
    for pity in 1..=last_pity {
        if let Some(n) = direct_risk[pity] {
            if n != remaining {
                return Err(format!("pity {pity}: 관측 집계 수와 위험집합이 일치하지 않습니다."));
            }
        }
        let d = events[pity];
        let c = censored[pity];
        if d.checked_add(c).is_none_or(|total| total > remaining) {
            return Err(format!("pity {pity}: 획득·중단 수가 위험집합을 초과합니다."));
        }
        let n = remaining;
        let hazard = if n == 0 { 0.0 } else { d as f64 / n as f64 };
        survival *= 1.0 - hazard;
        if d > 0 && n > d {
            greenwood_sum += d as f64 / ((n as f64) * ((n - d) as f64));
        }
        let ci = survival_cdf_interval(survival, greenwood_sum);
        rows.push(ObservationRow {
            pity, at_risk: n, events: d, censored: c, hazard, survival,
            cdf_ci_95: ci,
        });
        remaining = n - d - c;
    }
    Ok(ObservationDataset {
        quality, observations: observed, events: total_events,
        censored: total_censored, rows,
    })
}

