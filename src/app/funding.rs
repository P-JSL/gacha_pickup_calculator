/// All quick/detailed ticket editors update the exact same GameSetup fields.
#[derive(Clone, Copy)]
enum TicketRole { Shared, Character, Weapon }

fn edit_ticket_inventory(ui: &mut egui::Ui, setup: &mut GameSetup, role: TicketRole) -> bool {
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
fn wuwa_strategy_table(ui: &mut egui::Ui, strategies: &[FeedbackEstimate],
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

const CONFIDENCE_LEVELS: [usize; 6] = [50, 75, 80, 90, 95, 99];

/// Exact adaptive split: dedicated passes are restricted to their banner,
/// while freely convertible passes can be used after either goal finishes.
/// This search runs on the worker thread, not on every UI frame.
fn minimum_shared_pulls(
    character: &Distribution,
    weapon: &Distribution,
    dedicated_character: usize,
    dedicated_weapon: usize,
    confidence: f64,
) -> usize {
    let mut low = 0usize;
    let mut high = character.pmf.len().saturating_sub(1)
        .saturating_sub(dedicated_character)
        .saturating_add(weapon.pmf.len().saturating_sub(1)
            .saturating_sub(dedicated_weapon));
    while low < high {
        let middle = low + (high - low) / 2;
        let probability = optimal_allocation(
            character, weapon, dedicated_character, dedicated_weapon, middle,
        ).success_probability;
        if probability >= confidence { high = middle; }
        else { low = middle + 1; }
    }
    low
}

/// The funding shortfall is always based on the latest completed calculation.
/// A single-banner requirement is informative, not an amount to add to the
/// other banner: the shared pool and recycled refunds fund both.
#[derive(Clone, Copy, Debug)]
struct GoalFunding {
    confidence: usize,
    character_target: usize,
    character_owned: usize,
    character_missing: usize,
    weapon_target: usize,
    weapon_owned: usize,
    weapon_missing: usize,
    character_single_raw: usize,
    character_single_shared: usize,
    weapon_single_raw: usize,
    weapon_single_shared: usize,
    dedicated_character: usize,
    dedicated_weapon: usize,
    required_shared: usize,
    available_shared: usize,
    additional_shared: usize,
    additional_currency: usize,
    uses_refunds: bool,
}

impl GoalFunding {
    fn calculate(
        setup: &GameSetup,
        results: &Computed,
        character_target: usize,
        weapon_target: usize,
        confidence: usize,
    ) -> Self {
        let probability = confidence as f64 / 100.0;
        let required_shared = results.feedback.as_ref()
            .map_or_else(|| {
                results.exact_shared_required
                    .and_then(|thresholds| CONFIDENCE_LEVELS.iter()
                        .position(|&p| p == confidence).map(|i| thresholds[i]))
                    .unwrap_or_else(|| minimum_shared_pulls(
                        &results.character, &results.weapon,
                        setup.character_dedicated, setup.weapon_dedicated, probability))
            }, |f| f.quantile(probability));
        let character_single_raw = results.character.quantile(probability);
        let weapon_single_raw = results.weapon.quantile(probability);
        let available_shared = setup.available_flexible();
        Self {
            confidence,
            character_target,
            character_owned: setup.character_owned,
            character_missing: character_target.saturating_sub(setup.character_owned),
            weapon_target,
            weapon_owned: setup.weapon_owned,
            weapon_missing: weapon_target.saturating_sub(setup.weapon_owned),
            character_single_raw,
            character_single_shared: character_single_raw
                .saturating_sub(setup.character_dedicated),
            weapon_single_raw,
            weapon_single_shared: weapon_single_raw
                .saturating_sub(setup.weapon_dedicated),
            dedicated_character: setup.character_dedicated,
            dedicated_weapon: setup.weapon_dedicated,
            required_shared,
            available_shared,
            additional_shared: required_shared.saturating_sub(available_shared),
            additional_currency: setup.additional_currency_for(required_shared),
            uses_refunds: results.feedback.is_some(),
        }
    }
}

/// One banner's *standalone* additional featured-ticket units. This is an
/// independent reference which assumes the entire convertible currency can
/// be used on this banner; it must never be added to the other banner's result.
/// Stella Sora's record Gold Discs are displayed in units, not 1-pull tickets.
fn standalone_ticket_shortage(setup: &GameSetup, needed_after_dedicated: usize, weapon: bool) -> usize {
    let missing_pulls = needed_after_dedicated.saturating_sub(setup.available_flexible());
    let units_per_pull = if weapon { setup.game.weapon_ticket_units_per_pull() } else { 1 };
    let missing_units = missing_pulls.saturating_mul(units_per_pull);
    if weapon && setup.game == Game::StellaSora {
        missing_units.saturating_sub(setup.weapon_ticket_remainder)
    } else {
        missing_units
    }
}

/// Shared, compact funding summary displayed on Dashboard and Analytics.
fn render_goal_funding(ui: &mut egui::Ui, gap: &GoalFunding, setup: &GameSetup,
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
    ui.label(RichText::new(t(language, "Solo ticket estimate excludes refunds"))
        .color(MUTED).size(11.0));
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


/// Unit tests of independent banner-only shortage calculation.
/// The combined goal uses the separate joint DP/Monte Carlo funding path.
#[cfg(test)]
mod standalone_ticket_tests {
    use super::*;

    #[test]
    fn standalone_shortage_never_double_counts_convertible_currency() {
        for game in Game::ALL {
            let mut setup = GameSetup::new(game);
            setup.flexible = 2;
            setup.premium_currency = game.cost_per_pull() - 1;
            assert_eq!(standalone_ticket_shortage(&setup, 3, false), 1);
            setup.premium_currency += 1;
            assert_eq!(standalone_ticket_shortage(&setup, 3, false), 0);
            assert_eq!(standalone_ticket_shortage(&setup, 0, true), 0);
        }
    }

    #[test]
    fn stella_record_disc_remainder_is_subtracted_only_from_record_cost() {
        let mut setup = GameSetup::new(Game::StellaSora);
        setup.set_weapon_ticket_inventory(299);
        setup.premium_currency = 0;
        assert_eq!(standalone_ticket_shortage(&setup, 2, true), 301);
        assert_eq!(standalone_ticket_shortage(&setup, 2, false), 2);
        setup.set_weapon_ticket_inventory(300);
        assert_eq!(standalone_ticket_shortage(&setup, 1, true), 300);
    }
}
