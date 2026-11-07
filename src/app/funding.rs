use super::*;

pub(crate) const CONFIDENCE_LEVELS: [usize; 6] = [50, 75, 80, 90, 95, 99];

/// For two independent banners, the exact adaptive flexible-ticket demand
/// equals max(0, character draws - character dedicated) plus the analogous
/// weapon demand. Neither banner can spend the other's dedicated tickets.
pub(crate) fn remaining_shared_distribution(
    character: &Distribution,
    weapon: &Distribution,
    dedicated_character: usize,
    dedicated_weapon: usize,
) -> Distribution {
    fn residual(source: &Distribution, dedicated: usize) -> Distribution {
        let mut pmf = vec![0.0; source.pmf.len().saturating_sub(dedicated).max(1)];
        for (draws, &probability) in source.pmf.iter().enumerate() {
            pmf[draws.saturating_sub(dedicated)] += probability;
        }
        Distribution::new(pmf)
    }
    residual(character, dedicated_character).combine(&residual(weapon, dedicated_weapon))
}

/// One O(n log n) dense distribution join and one O(log n) quantile lookup.
pub(crate) fn minimum_shared_pulls(
    character: &Distribution,
    weapon: &Distribution,
    dedicated_character: usize,
    dedicated_weapon: usize,
    confidence: f64,
) -> usize {
    remaining_shared_distribution(character, weapon, dedicated_character, dedicated_weapon)
        .quantile(confidence)
}

/// One *actual* Monte Carlo path at the confidence boundary, not the
/// difference between two independently selected distribution quantiles.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RefundBoundaryPath {
    pub(crate) raw_pulls: usize,
    pub(crate) dedicated_used: usize,
    pub(crate) points_funded_pulls: usize,
    pub(crate) initial_flexible_required: usize,
}

/// The overall shortfall is calculated from the same shared-ticket CDF used
/// for the displayed success rate. Individual banners remain refund-free
/// reference values and must never be summed into the total.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GoalFunding {
    pub(crate) confidence: usize,
    pub(crate) character_target: usize,
    pub(crate) character_owned: usize,
    pub(crate) character_missing: usize,
    pub(crate) weapon_target: usize,
    pub(crate) weapon_owned: usize,
    pub(crate) weapon_missing: usize,
    pub(crate) character_single_raw: usize,
    pub(crate) character_single_shared: usize,
    pub(crate) weapon_single_raw: usize,
    pub(crate) weapon_single_shared: usize,
    pub(crate) dedicated_character: usize,
    pub(crate) dedicated_weapon: usize,
    pub(crate) required_shared: usize,
    /// Exact no-refund comparison at the same confidence and same dedicated
    /// inventory. Never derive this by summing standalone quantiles.
    pub(crate) raw_required_shared: usize,
    /// Difference in overall quantiles, not the number of redeemed tickets
    /// in any one trial; can be negative under optional store strategies.
    pub(crate) quantile_change: i64,
    pub(crate) boundary_path: Option<RefundBoundaryPath>,
    pub(crate) quantile_uncertainty: Option<(usize, Option<usize>, f64)>,
    pub(crate) current_probability: f64,
    pub(crate) raw_probability: f64,
    pub(crate) probability_uncertainty: Option<(f64, f64)>,
    pub(crate) available_shared: usize,
    pub(crate) additional_shared: usize,
    pub(crate) additional_currency: usize,
    pub(crate) uses_refunds: bool,
}

impl GoalFunding {
    pub(crate) fn calculate(
        setup: &GameSetup,
        results: &Computed,
        character_target: usize,
        weapon_target: usize,
        confidence: usize,
    ) -> Self {
        let probability = confidence as f64 / 100.0;
        let raw_required_shared = results.exact_shared_required
            .and_then(|thresholds| CONFIDENCE_LEVELS.iter()
                .position(|&p| p == confidence).map(|i| thresholds[i]))
            .unwrap_or_else(|| results.raw_shared.quantile(probability));
        let required_shared = results.feedback.as_ref()
            .map_or(raw_required_shared, |f| f.quantile(probability));
        let character_single_raw = results.character.quantile(probability);
        let weapon_single_raw = results.weapon.quantile(probability);
        let available_shared = setup.available_flexible();
        let raw_probability = results.raw_shared.at(available_shared);
        let (current_probability, probability_uncertainty) =
            if let Some(refund) = results.feedback.as_ref() {
                let (point, low, high) = refund.probability(available_shared);
                (point, Some((low, high)))
            } else { (raw_probability, None) };
        let boundary_path = results.feedback.as_ref()
            .and_then(|refund| refund.scenario_at_quantile(probability))
            .map(|scenario| RefundBoundaryPath {
                raw_pulls: scenario.raw_pulls,
                dedicated_used: scenario.dedicated_used,
                points_funded_pulls: scenario.points_funded_pulls,
                initial_flexible_required: scenario.initial_flexible_required,
            });
        let quantile_uncertainty = results.feedback.as_ref()
            .and_then(|refund| refund.quantile_confidence_band(probability, 0.95))
            .map(|band| (band.lower, band.upper, band.cdf_tolerance));
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
            raw_required_shared,
            quantile_change: raw_required_shared as i64 - required_shared as i64,
            boundary_path,
            quantile_uncertainty,
            current_probability,
            raw_probability,
            probability_uncertainty,
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
pub(crate) fn standalone_ticket_shortage(setup: &GameSetup, needed_after_dedicated: usize, weapon: bool) -> usize {
    let missing_pulls = needed_after_dedicated.saturating_sub(setup.available_flexible());
    let units_per_pull = if weapon { setup.game.weapon_ticket_units_per_pull() } else { 1 };
    let missing_units = missing_pulls.saturating_mul(units_per_pull);
    if weapon && setup.game == Game::StellaSora {
        missing_units.saturating_sub(setup.weapon_ticket_remainder)
    } else {
        missing_units
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

#[cfg(test)]
mod adaptive_distribution_tests {
    use super::*;

    #[test]
    fn residual_distribution_matches_exact_adaptive_allocation() {
        for cp in [vec![0.0, 0.5, 0.5], vec![0.2, 0.3, 0.5], vec![1.0]] {
            for wp in [vec![0.0, 0.25, 0.75], vec![0.1, 0.9], vec![1.0]] {
                let character = Distribution::new(cp.clone());
                let weapon = Distribution::new(wp.clone());
                for dc in 0..=4 {
                    for dw in 0..=4 {
                        let demand = remaining_shared_distribution(&character, &weapon, dc, dw);
                        for budget in 0..=8 {
                            let expected = optimal_allocation(&character, &weapon, dc, dw, budget);
                            assert!((demand.at(budget) - expected.success_probability).abs() < 1e-10);
                        }
                    }
                }
            }
        }
    }
}
