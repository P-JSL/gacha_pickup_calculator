use super::*;

/// A single Monte Carlo path; never subtract independently chosen quantiles.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RefundScenario {
    pub raw_pulls: usize,
    pub dedicated_used: usize,
    /// Tickets covered by already-owned plus newly earned exchange points.
    pub points_funded_pulls: usize,
    /// Minimum flexible-ticket funding required at any time before completion.
    pub initial_flexible_required: usize,
    /// All earned points, including the final pull's unspendable reward.
    pub earned_points: usize,
    pub wavebands_purchased: usize,
    pub corals_spent_on_wavebands: usize,
    pub board_dice_earned: usize,
    pub record_disc_units_earned: usize,
}
impl RefundScenario {
    pub(super) fn from_wallet(wallet: &Wallet, starting_points: usize) -> Self {
        let dedicated_used = wallet.raw_pulls - wallet.common_spent;
        let points_funded_pulls = wallet.common_spent - wallet.initial_flexible_required;
        let scenario = Self {
            raw_pulls: wallet.raw_pulls,
            dedicated_used,
            points_funded_pulls,
            initial_flexible_required: wallet.initial_flexible_required,
            earned_points: wallet.points - starting_points,
            wavebands_purchased: wallet.wavebands_bought,
            corals_spent_on_wavebands: wallet.waveband_points_spent,
            board_dice_earned: wallet.board_dice_earned,
            record_disc_units_earned: wallet.record_disc_units,
        };
        debug_assert_eq!(scenario.raw_pulls,
            scenario.dedicated_used + scenario.points_funded_pulls
                + scenario.initial_flexible_required);
        scenario
    }
}

#[derive(Clone, Debug)]
pub struct SuccessfulBudgetMeans {
    pub count: usize,
    pub average_raw: f64,
    pub average_dedicated: f64,
    pub average_points_funded: f64,
    pub average_flexible: f64,
    pub average_earned_points: f64,
}

/// A conservative 95% simultaneous CDF confidence band implies a range
/// of feasible resource quantiles. None for upper means finite sample data
/// cannot establish a finite upper quantile at this coverage.
#[derive(Clone, Debug, PartialEq)]
pub struct QuantileConfidenceBand {
    pub lower: usize,
    pub upper: Option<usize>,
    /// Massart/DKW sup-norm CDF tolerance, not a currency-unit margin.
    pub cdf_tolerance: f64,
}

/// Disjoint original-ticket, refund-assisted, and failed outcomes from one sample set.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BudgetSuccessBreakdown {
    pub original_only: usize,
    pub after_recycling: usize,
    pub failed: usize,
    pub samples: usize,
    pub average_extra_recycled_draws: f64,
    pub average_refund_tickets_on_recycled_success: f64,
}
impl BudgetSuccessBreakdown {
    pub fn original_probability(&self) -> f64 {
        self.original_only as f64 / self.samples.max(1) as f64
    }
    pub fn recycling_probability(&self) -> f64 {
        self.after_recycling as f64 / self.samples.max(1) as f64
    }
    pub fn failure_probability(&self) -> f64 {
        self.failed as f64 / self.samples.max(1) as f64
    }
    pub fn final_probability(&self) -> f64 {
        (self.original_only + self.after_recycling) as f64 / self.samples.max(1) as f64
    }
}

#[derive(Clone, Debug)]
pub struct FeedbackEstimate {
    /// Sorted minimum additional flexible tickets needed beyond dedicated tickets.
    pub budgets: Vec<usize>,
    /// Original trial order retained for existing API.
    pub raw_pulls: Vec<usize>,
    /// Same-trial results ordered by funding required, then raw pulls.
    pub scenarios: Vec<RefundScenario>,
    pub average_raw: f64,
    pub average_initial_flexible: f64,
    pub total_returned_points_average: f64,
    pub average_wavebands_purchased: f64,
    pub average_waveband_corals_spent: f64,
    pub average_character_raw: f64,
    pub average_weapon_raw: f64,
    pub average_character_return_points: f64,
    pub average_weapon_return_points: f64,
    pub average_board_dice_earned: f64,
    pub average_record_disc_units_earned: f64,
}

impl FeedbackEstimate {
    /// Original tickets are exhausted first; only previously earned returns
    /// can finance additional draws. The prefix-maximal funding deficit stored
    /// in each path is equivalent to this payment order.
    pub fn success_breakdown(&self, available_flexible: usize, points_per_ticket: usize)
                             -> BudgetSuccessBreakdown {
        let samples = self.scenarios.len();
        let mut result = BudgetSuccessBreakdown { samples, ..Default::default() };
        let mut extra = 0_u128;
        let mut earned = 0_u128;
        for trial in &self.scenarios {
            let shared_draws = trial.raw_pulls.saturating_sub(trial.dedicated_used);
            if shared_draws <= available_flexible {
                result.original_only += 1;
            } else if trial.initial_flexible_required <= available_flexible {
                result.after_recycling += 1;
                extra += (shared_draws - available_flexible) as u128;
                earned += trial.earned_points as u128;
            } else {
                result.failed += 1;
            }
        }
        if result.after_recycling > 0 {
            result.average_extra_recycled_draws =
                extra as f64 / result.after_recycling as f64;
            result.average_refund_tickets_on_recycled_success =
                earned as f64 / result.after_recycling as f64
                    / points_per_ticket.max(1) as f64;
        }
        debug_assert_eq!(result.original_only + result.after_recycling + result.failed, samples);
        debug_assert_eq!(result.original_only + result.after_recycling,
            self.budgets.partition_point(|&needed| needed <= available_flexible));
        result
    }

    /// Empirical CDF, with the standard binomial Wilson 95% uncertainty interval.
    pub fn probability(&self, available_flexible: usize) -> (f64, f64, f64) {
        let count = self.budgets.partition_point(|&n| n <= available_flexible);
        let n = self.budgets.len() as f64;
        if n == 0.0 { return (0.0, 0.0, 1.0); }
        let p = count as f64 / n;
        let z = 1.96;
        let z2 = z * z;
        let denominator = 1.0 + z2 / n;
        let center = (p + z2 / (2.0 * n)) / denominator;
        let radius = z * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt() / denominator;
        (p, (center - radius).max(0.0), (center + radius).min(1.0))
    }

    pub fn quantile(&self, confidence: f64) -> usize {
        let n = self.budgets.len();
        if n == 0 || confidence <= 0.0 { return 0; }
        let rank = (confidence.clamp(0.0, 1.0) * n as f64).ceil() as usize;
        self.budgets[rank.saturating_sub(1).min(n - 1)]
    }

    /// Dvoretzky–Kiefer–Wolfowitz / Massart simultaneous nonparametric
    /// confidence band for a target resource quantile. Valid when simulated
    /// paths are independent draws from a fixed refund model:
    ///   P(sup_x |F_n(x)-F(x)| > e) <= 2 exp(-2 n e²)
    /// No finite upper bound is inferred when target+e >= 1. This covers
    /// Monte Carlo sampling noise, NOT incorrect banner/refund assumptions.
    pub fn quantile_confidence_band(&self, confidence: f64,
                                    coverage: f64) -> Option<QuantileConfidenceBand> {
        if self.budgets.is_empty() || !confidence.is_finite()
            || !(0.0..=1.0).contains(&confidence)
            || !coverage.is_finite() || !(0.0..1.0).contains(&coverage)
            || coverage == 0.0
        {
            return None;
        }
        let n = self.budgets.len() as f64;
        let epsilon = ( (2.0 / (1.0 - coverage)).ln() / (2.0 * n) ).sqrt();
        let low_p = (confidence - epsilon).max(0.0);
        let high_p = confidence + epsilon;
        let lower = if low_p == 0.0 { 0 } else { self.quantile(low_p) };
        let upper = if high_p >= 1.0 { None } else { Some(self.quantile(high_p)) };
        Some(QuantileConfidenceBand { lower, upper, cdf_tolerance: epsilon })
    }

    /// One observed path exactly at the requested empirical quantile.
    /// The tuple N = D + X + P is true for this single representative trial,
    /// but N, D, X vary among all possible outcomes.
    pub fn scenario_at_quantile(&self, confidence: f64) -> Option<&RefundScenario> {
        let n = self.scenarios.len();
        if n == 0 { return None; }
        let rank = (confidence.clamp(0.0, 1.0) * n as f64).ceil() as usize;
        self.scenarios.get(rank.saturating_sub(1).min(n - 1))
    }

    /// Condition on completion within this flexible resource budget.
    pub fn successful_means(&self, budget: usize) -> Option<SuccessfulBudgetMeans> {
        let subset = self.scenarios.partition_point(|s| s.initial_flexible_required <= budget);
        if subset == 0 { return None; }
        let n = subset as f64;
        let mut summary = SuccessfulBudgetMeans {
            count: subset, average_raw: 0.0, average_dedicated: 0.0,
            average_points_funded: 0.0, average_flexible: 0.0,
            average_earned_points: 0.0,
        };
        for scenario in &self.scenarios[..subset] {
            summary.average_raw += scenario.raw_pulls as f64;
            summary.average_dedicated += scenario.dedicated_used as f64;
            summary.average_points_funded += scenario.points_funded_pulls as f64;
            summary.average_flexible += scenario.initial_flexible_required as f64;
            summary.average_earned_points += scenario.earned_points as f64;
        }
        summary.average_raw /= n;
        summary.average_dedicated /= n;
        summary.average_points_funded /= n;
        summary.average_flexible /= n;
        summary.average_earned_points /= n;
        Some(summary)
    }
}

