use super::*;

/// All quick/detailed ticket editors update the exact same GameSetup fields.
#[derive(Clone, Copy)]
pub(crate) enum TicketRole { Shared, Character, Weapon }

pub(crate) fn edit_ticket_inventory(ui: &mut egui::Ui, setup: &mut GameSetup, role: TicketRole) -> bool {
    match role {
        TicketRole::Shared =>
            ui.add(egui::DragValue::new(&mut setup.flexible)
                .range(0..=100_000).speed(1).suffix("장")).changed(),
        TicketRole::Character =>
            ui.add(egui::DragValue::new(&mut setup.character_dedicated)
                .range(0..=100_000).speed(1).suffix("장")).changed(),
        TicketRole::Weapon => {
            if setup.game == Game::StellaSora {
                let mut discs = setup.weapon_ticket_inventory();
                let changed = ui.add(egui::DragValue::new(&mut discs)
                    .range(0..=30_000_000).speed(1).suffix("개")).changed();
                if changed { setup.set_weapon_ticket_inventory(discs); }
                changed
            } else {
                ui.add(egui::DragValue::new(&mut setup.weapon_dedicated)
                    .range(0..=100_000).speed(1).suffix("개")).changed()
            }
        }
    }
}

/// Match simulation assumptions across all three WuWa coral strategies.
pub(crate) fn wuwa_strategy_table(ui: &mut egui::Ui, strategies: &[Arc<FeedbackEstimate>],
    setup: &GameSetup, confidence: usize) {
    let owned = setup.available_flexible();
    let threshold = confidence as f64 / 100.0;
    section(ui, "WU", "명조 · 산호 투자 전략 3종 비교",
        "환급 재투입까지 계산한 몬테카를로 비교입니다. 돌파 재료는 산호를 확보한 이후 구매합니다.", |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("wuwa_comparison_table").striped(true)
                    .min_col_width(95.0).spacing([14.0, 9.0]).show(ui, |ui| {
                        for title in ["산호 사용", &format!("{}% 필요 자유뽑", confidence),
                            "추가 별의 소리", "현재 재화 성공률", "평균 총 뽑", "평균 돌파 구매"] {
                            ui.label(RichText::new(title).strong());
                        }
                        ui.end_row();
                        for (count, result) in strategies.iter().enumerate() {
                            let needed = result.quantile(threshold);
                            let extra = setup.additional_currency_for(needed);
                            let chance = result.probability(owned).0;
                            ui.label(match count {
                                0 => "뽑기권 전환", 1 => "돌파 최대 1개",
                                _ => "돌파 최대 2개",
                            });
                            ui.label(format!("{needed}뽑"));
                            ui.label(format!("{extra}개"));
                            ui.label(format!("{:.2}%", chance * 100.0));
                            ui.label(format!("{:.1}뽑", result.average_raw));
                            ui.label(format!("{:.2}개", result.average_wavebands_purchased));
                            ui.end_row();
                        }
                    });
            });
            ui.small("필요 자유뽑은 별도 전용권을 제외한 최초 뽑기 예산입니다. 확률은 표본 기반 추정치입니다.");
        });
}


/// Shared, compact funding summary displayed on Dashboard and Analytics.
pub(crate) fn render_goal_funding(ui: &mut egui::Ui, gap: &GoalFunding, setup: &GameSetup,
                       language: Language) {
    let secondary = i18n::banner_name(language, setup.game);
    let native = i18n::currency_name(language, setup.game);
    let note = if gap.uses_refunds { t(language, "Refund-simulation estimate") }
        else { t(language, "Exact probability model") };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!(
            "{} {}% · {}",
            t(language, "Target confidence"), gap.confidence, note,
        )).color(MUTED).size(11.0));
    });
    ui.add_space(5.0);

    let individual = |ui: &mut egui::Ui, label: &str, weapon: bool,
                      counts: (usize, usize, usize, usize, usize, usize)| {
        let (target, owned, missing, raw, dedicated, shared) = counts;
        let ticket = i18n::ticket_name(language, setup.game, weapon);
        let shortage = standalone_ticket_shortage(setup, shared, weapon);
        let unit = if weapon && setup.game == Game::StellaSora {
            t(language, "Currency units")
        } else {
            t(language, "Passes")
        };
        ui.label(RichText::new(label).color(PRIMARY).strong());
        ui.label(format!(
            "{} {}  ·  {} {}  ·  {} {}",
            t(language, "Target copies"), target,
            t(language, "Owned copies"), owned,
            t(language, "Missing copies"), missing,
        ));
        ui.label(RichText::new(format!(
            "{}: {}{}  ·  {}: {}{}",
            t(language, "Solo requirement"), raw, t(language, "Pulls"),
            t(language, "After dedicated passes"), shared, t(language, "Pulls"),
        )).color(MUTED).size(11.0));
        if dedicated > 0 {
            ui.small(format!(
                "{} {}{}",
                t(language, "Owned dedicated passes"), dedicated, t(language, "Passes"),
            ));
        }
        ui.label(RichText::new(format!(
            "{} · {}: +{} {}",
            t(language, "Standalone ticket shortfall"), ticket, shortage, unit,
        )).color(if shortage == 0 { MINT } else { GOLD }).strong());
    };
    let shortage = |ui: &mut egui::Ui| {
        ui.label(RichText::new(t(language, "Additional featured ticket equivalents"))
            .color(GOLD).strong());
        ui.label(RichText::new(format!(
            "+{} {}  ·  +{} {}",
            gap.additional_shared, t(language, "Featured ticket equivalents"),
            gap.additional_currency, native,
        )).size(16.0).strong().color(
            if gap.additional_shared == 0 { MINT } else { GOLD },
        ));
        ui.small(format!(
            "{} {}{}  ·  {} {}{}",
            t(language, "Required shared passes"), gap.required_shared, t(language, "Pulls"),
            t(language, "Available shared passes"), gap.available_shared, t(language, "Pulls"),
        ));
    };
    if ui.available_width() >= 780.0 {
        ui.columns(3, |cols| {
            individual(&mut cols[0], t(language, "Characters"), false, (
                gap.character_target, gap.character_owned, gap.character_missing,
                gap.character_single_raw, gap.dedicated_character,
                gap.character_single_shared));
            individual(&mut cols[1], secondary, true, (
                gap.weapon_target, gap.weapon_owned, gap.weapon_missing,
                gap.weapon_single_raw, gap.dedicated_weapon,
                gap.weapon_single_shared));
            shortage(&mut cols[2]);
        });
    } else {
        individual(ui, t(language, "Characters"), false, (
            gap.character_target, gap.character_owned, gap.character_missing,
            gap.character_single_raw, gap.dedicated_character,
            gap.character_single_shared));
        ui.add_space(6.0);
        individual(ui, secondary, true, (
            gap.weapon_target, gap.weapon_owned, gap.weapon_missing,
            gap.weapon_single_raw, gap.dedicated_weapon,
            gap.weapon_single_shared));
        ui.separator();
        shortage(ui);
    }
    ui.add_space(8.0);
    if gap.uses_refunds {
        // The no-refund and refund figures are *joint* character+equipment
        // results at the same confidence, dedicated inventory, and shared
        // funding axis. Never add two standalone target quantiles.
        ui.label(RichText::new("전체 목표 · 환급 재투입 계산 내역")
            .strong().color(MINT).size(13.0));
        ui.label(format!(
            "{}% 달성 필요: 환급 전 {}장 → 환급 재투입 후 {}장 · 현재 보유 환산 {}장 → 추가 {}장",
            gap.confidence, gap.raw_required_shared, gap.required_shared,
            gap.available_shared, gap.additional_shared
        ));
        let difference = gap.quantile_change;
        ui.label(RichText::new(format!(
            "동일 달성률의 필요량 차이: {difference:+}장 (분위수 차이이며 실제 환급권 수량은 아님)"
        )).color(if difference >= 0 { MINT } else { GOLD }).size(11.0));
        ui.label(format!(
            "현재 보유량 기준 전체 목표 확률: 환급 전 {:.2}% → 환급 재투입 후 {:.2}%",
            gap.raw_probability * 100.0, gap.current_probability * 100.0
        ));
        if let Some((low, high)) = gap.probability_uncertainty {
            ui.small(format!(
                "환급 후 성공 확률의 Monte Carlo 95% Wilson 구간: {:.2}~{:.2}%",
                low * 100.0, high * 100.0
            ));
        }
        if let Some(path) = gap.boundary_path {
            debug_assert_eq!(path.raw_pulls, path.dedicated_used
                + path.points_funded_pulls + path.initial_flexible_required);
            ui.small(format!(
                "{}% 경계의 실제 한 표본: 총 {}회 = 전용권 {}회 + 환급 포인트 충당 {}회 + 최초 자유권 {}회",
                gap.confidence, path.raw_pulls, path.dedicated_used,
                path.points_funded_pulls, path.initial_flexible_required
            ));
        }
        if let Some((lower, upper, epsilon)) = gap.quantile_uncertainty {
            let upper_label = upper.map_or_else(
                || "상한 미확정".to_owned(), |n| format!("{n}장"));
            ui.small(format!(
                "필요 최초 자유권 표본 오차(95% 동시 신뢰대역): {}~{} · CDF 오차 ±{:.2}%p",
                lower, upper_label, epsilon * 100.0
            ));
        }
        ui.small("환급은 각 뽑기 이후에만 다음 뽑기에 투입합니다. 개별 캐릭터·장비 수량은 환급 전 독립 참고값이며 둘을 합산하지 않습니다.");
    }
    ui.add_space(5.0);
    if setup.game.shares_featured_ticket() {
        ui.label(RichText::new(format!(
            "{} {}{} + {} {} ({}{}).",
            i18n::ticket_name(language, setup.game, false), setup.flexible,
            t(language, "Passes"),
            setup.premium_currency, native,
            setup.premium_currency / setup.game.cost_per_pull(), t(language, "Pulls"),
        )).color(MUTED).size(11.0));
    } else {
        let weapon_inventory = if setup.game == Game::StellaSora {
            format!("{} {} ({}{})",
                setup.weapon_ticket_inventory(), t(language, "Currency units"),
                setup.weapon_dedicated, t(language, "Pulls"))
        } else {
            format!("{}{}", setup.weapon_dedicated, t(language, "Passes"))
        };
        ui.label(RichText::new(format!(
            "{} {}{} · {} {} · {} {} ({}{}).",
            i18n::ticket_name(language, setup.game, false),
            setup.character_dedicated, t(language, "Passes"),
            i18n::ticket_name(language, setup.game, true), weapon_inventory,
            setup.premium_currency, native,
            setup.premium_currency / setup.game.cost_per_pull(), t(language, "Pulls"),
        )).color(MUTED).size(11.0));
    }
    ui.label(RichText::new(format!(
        "{} · 전체 환급 시뮬레이션과 별개",
        t(language, "Solo ticket estimate excludes refunds")
    )).color(MUTED).size(11.0));
    ui.label(RichText::new(t(language, "Shared passes are allocated adaptively; do not add standalone requirements."))
        .color(MUTED).size(11.0));
    if setup.game == Game::StellaSora {
        ui.small(format!(
            "{}: {}",
            t(language, "Unconverted Gold Disc units"),
            setup.weapon_ticket_remainder,
        ));
    }
}


