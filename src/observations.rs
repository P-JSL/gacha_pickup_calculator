//! Offline tracker-observation validation with discrete Kaplan–Meier estimates.
//!
//! This corrects *independent right censoring only*: it cannot correct
//! self-selection, missing early history, banner mixing or left truncation.
//! No network, account tokens, tracking URLs, or external subprocesses are used.

use crate::{RateMode, RateSchedule};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationQuality {
    /// Independent time-to-event observations, including right censoring.
    IndividualRightCensored,
    /// Explicit per-pity risk sets and losses to follow-up.
    AggregatedRightCensored,
    /// Successes only: missing censored runs makes inference unreliable.
    CompletedOnly,
}

impl ObservationQuality {
    pub fn label(self) -> &'static str {
        match self {
            Self::IndividualRightCensored => "개별 획득·중단 이력 (우측 검열 반영)",
            Self::AggregatedRightCensored => "위험집합·획득·중단 집계 (우측 검열 반영)",
            Self::CompletedOnly => "획득 성공만 기록 (검열 정보 없음, 편향 가능)",
        }
    }
    pub fn is_censor_aware(self) -> bool {
        self != Self::CompletedOnly
    }
}

#[derive(Clone, Debug)]
pub struct ObservationRow {
    pub pity: usize,
    pub at_risk: u64,
    pub events: u64,
    pub censored: u64,
    pub hazard: f64,
    pub survival: f64,
    /// Pointwise log-log Greenwood interval, absent at degenerate endpoints.
    pub cdf_ci_95: Option<(f64, f64)>,
}

#[derive(Clone, Debug)]
pub struct ObservationDataset {
    pub quality: ObservationQuality,
    pub observations: u64,
    pub events: u64,
    pub censored: u64,
    pub rows: Vec<ObservationRow>,
}

#[derive(Clone, Debug)]
pub struct ModelComparison {
    pub max_cdf_delta: f64,
    pub mean_abs_hazard_delta: f64,
    pub restricted_observed_mean: f64,
    pub restricted_model_mean: f64,
    pub last_compared_pity: usize,
    pub reliable_risk_bins: usize,
}

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
    let lines: Vec<(usize, &str)> = text.lines()
        .enumerate()
        .map(|(i, s)| (i + 1, s.trim().trim_start_matches('\u{feff}')))
        .filter(|(_, s)| !s.is_empty() && !s.starts_with('#'))
        .collect();
    if lines.len() < 2 {
        return Err("CSV에 헤더와 최소 한 건의 데이터가 필요합니다.".into());
    }
    if lines.len() > 200_001 {
        return Err("한 번에 최대 200,000개 행만 가져올 수 있습니다.".into());
    }
    let delimiter = if lines[0].1.contains('\t') { '\t' } else { ',' };
    let columns: Vec<String> = lines[0].1.split(delimiter)
        .map(|s| s.trim().trim_matches('"').to_ascii_lowercase())
        .collect();
    if columns.iter().enumerate().any(|(i, name)| columns[..i].contains(name)) {
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
    for (line_no, line) in lines.into_iter().skip(1) {
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

/// Log-log transformed Greenwood 95% confidence interval.
/// In the boundary cases, the interval is descriptive only; the tail is
/// explicitly not declared "precise" when few players remain at risk.
fn survival_cdf_interval(survival: f64, sum: f64) -> Option<(f64, f64)> {
    // Greenwood is undefined when survival is exactly zero and has zero-width,
    // misleading confidence bands before the first observed event.
    if survival <= 0.0 || survival >= 1.0 { return None; }
    let log_h = (-survival.ln()).ln();
    let err = 1.96 * sum.sqrt() / survival.ln().abs();
    let lower_s = -((log_h + err).exp());
    let upper_s = -((log_h - err).exp());
    let low = 1.0 - upper_s.exp();
    let high = 1.0 - lower_s.exp();
    Some((low.clamp(0.0, 1.0), high.clamp(0.0, 1.0)))
}

impl ObservationDataset {
    /// The last available time can be censored. This is *restricted* survival,
    /// not a complete unconditional mean to the hard pity unless at-risk reaches zero.
    pub fn compare(&self, model: &RateSchedule, mode: RateMode) -> Result<ModelComparison, String> {
        if self.rows.len() > model.hard {
            return Err("입력 데이터의 pity 범위가 선택한 배너 천장을 초과했습니다.".into());
        }
        let mut model_survival = 1.0_f64;
        let mut previous_observed_survival = 1.0_f64;
        let mut observed_mean = 0.0;
        let mut model_mean = 0.0;
        let mut max_diff = 0.0_f64;
        let mut weighted_diff = 0.0;
        let mut weighted_n = 0.0;
        let mut reliable_bins = 0;
        let mut last = 0;
        for r in &self.rows {
            observed_mean += previous_observed_survival;
            model_mean += model_survival;
            let predicted_rate = model.chance(r.pity, mode);
            model_survival *= 1.0 - predicted_rate;
            if r.at_risk >= 20 {
                max_diff = max_diff.max((r.survival - model_survival).abs());
                weighted_diff += r.at_risk as f64 * (r.hazard - predicted_rate).abs();
                weighted_n += r.at_risk as f64;
                reliable_bins += 1;
                last = r.pity;
            }
            previous_observed_survival = r.survival;
        }
        Ok(ModelComparison {
            max_cdf_delta: max_diff,
            mean_abs_hazard_delta: if weighted_n > 0.0 { weighted_diff / weighted_n } else { 0.0 },
            restricted_observed_mean: observed_mean,
            restricted_model_mean: model_mean,
            last_compared_pity: last,
            reliable_risk_bins: reliable_bins,
        })
    }

    pub fn max_pity(&self) -> usize {
        self.rows.last().map_or(0, |r| r.pity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SoftPity};

    fn constant_model(hard: usize, rate: f64) -> RateSchedule {
        RateSchedule { base: rate, hard, soft: SoftPity::None }
    }

    #[test]
    fn correctly_accounts_for_right_censoring() {
        // First completion at 2, second completion at 4, one stopped after 3.
        // At pity 2: 1/3 events. At pity 4: 1/1, not 1/2.
        let d = parse_observation_csv("duration,event\n2,1\n3,0\n4,1\n", 5).unwrap();
        assert_eq!(d.quality, ObservationQuality::IndividualRightCensored);
        assert_eq!(d.observations, 3);
        assert_eq!(d.events, 2);
        assert_eq!(d.censored, 1);
        assert!((d.rows[1].survival - 2.0 / 3.0).abs() < 1e-12);
        assert!((d.rows[3].survival - 0.0).abs() < 1e-12);
        assert_eq!(d.rows[3].at_risk, 1);
    }

    #[test]
    fn aggregated_risk_set_matches_individuals() {
        let individual = parse_observation_csv("duration,event\n2,1\n3,0\n4,1\n", 5).unwrap();
        let aggregated = parse_observation_csv(
            "pity,at_risk,events,censored\n1,3,0,0\n2,3,1,0\n3,2,0,1\n4,1,1,0\n", 5
        ).unwrap();
        for (a,b) in individual.rows.iter().zip(aggregated.rows.iter()) {
            assert_eq!(a.at_risk,b.at_risk);
            assert_eq!(a.events,b.events);
            assert!((a.survival-b.survival).abs() < 1e-12);
        }
    }

    #[test]
    fn samples_with_only_winning_pities_are_flagged_as_biased() {
        let d = parse_observation_csv("pity,count\n1,5\n2,5\n", 90).unwrap();
        assert_eq!(d.quality, ObservationQuality::CompletedOnly);
        assert!(!d.quality.is_censor_aware());
        assert_eq!(d.events, 10);
    }

    #[test]
    fn rejects_inconsistent_aggregated_risk_set() {
        let bad = "pity,at_risk,events,censored\n1,3,1,0\n2,3,0,0\n";
        assert!(parse_observation_csv(bad, 90).is_err());
    }

    #[test]
    fn rejects_out_of_order_and_excess_hard_pity() {
        let bad = "pity,count\n2,1\n1,2\n";
        assert!(parse_observation_csv(bad, 90).is_err());
        assert!(parse_observation_csv("duration,event\n91,1\n", 90).is_err());
    }

    #[test]
    fn detects_model_cdf_mismatch() {
        let d = parse_observation_csv("duration,event\n2,1\n3,0\n4,1\n", 5).unwrap();
        let report = d.compare(&constant_model(5, 0.5), RateMode::PublishedOnly).unwrap();
        assert!(report.restricted_observed_mean > 0.0);
        assert_eq!(report.reliable_risk_bins, 0);
    }


    #[test]
    fn complete_known_hazard_matches_kaplan_meier_with_no_censoring() {
        let csv = "pity,at_risk,events,censored\n1,100,50,0\n2,50,25,0\n3,25,25,0\n";
        let data = parse_observation_csv(csv, 3).unwrap();
        let c = data.compare(&constant_model(3, 0.5), RateMode::PublishedOnly).unwrap();
        assert_eq!(data.observations, 100);
        assert_eq!(data.events, 100);
        assert_eq!(data.censored, 0);
        assert_eq!(c.reliable_risk_bins, 3);
        assert!(c.max_cdf_delta < 1e-12);
        assert!(c.mean_abs_hazard_delta < 1e-12);
        let (lower, upper) = data.rows[1].cdf_ci_95.expect("interior KM confidence");
        assert!(lower <= 0.75);
        assert!(upper >= 0.75);
        assert!(data.rows[2].cdf_ci_95.is_none());
    }

    #[test]
    fn extreme_counts_are_rejected_without_integer_overflow() {
        let csv = "pity,at_risk,events,censored\n1,18446744073709551615,18446744073709551615,18446744073709551615\n";
        assert!(parse_observation_csv(csv, 90).is_err());
    }

    #[test]
    fn all_censored_cohort_is_valid_but_has_no_greenwood_band() {
        let data = parse_observation_csv("duration,event\n5,0\n5,0\n5,0\n", 90).unwrap();
        assert_eq!(data.observations, 3);
        assert_eq!(data.events, 0);
        assert_eq!(data.censored, 3);
        assert!(data.rows.iter().all(|r| r.survival == 1.0 && r.cdf_ci_95.is_none()));
    }

    #[test]
    fn partial_risk_set_header_cannot_be_interpreted_as_completed_only() {
        for csv in [
            "pity,at_risk,events\n1,100,5\n2,95,6\n",
            "pity,events,censored\n1,5,3\n",
            "pity,count,censored\n1,5,3\n",
            "pity,at_risk,count\n1,10,5\n",
            "pity,at_risk,events,censored,count\n1,10,5,5,5\n",
            "duration,pity,count\n1,1,1\n",
            "duration,event,pity,count\n1,1,1,1\n",
            "duration,duration,event\n1,1,1\n",
            "pity,pity,count\n1,1,2\n",
        ] {
            assert!(parse_observation_csv(csv, 90).is_err(), "{csv}");
        }
    }

    #[test]
    fn exact_synthetic_20_persons_match_constant_half_rate_until_pity_one() {
        let mut input = String::from("duration,event\n");
        for _ in 0..10 { input.push_str("1,1\n"); }
        for _ in 0..10 { input.push_str("2,1\n"); }
        let d = parse_observation_csv(&input, 3).unwrap();
        assert_eq!(d.rows[0].at_risk, 20);
        assert!((d.rows[0].hazard - 0.5).abs() < 1e-12);
        assert_eq!(d.compare(&constant_model(3, 0.5), RateMode::PublishedOnly)
            .unwrap().reliable_risk_bins, 1);
    }
}
