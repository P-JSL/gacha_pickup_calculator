//! Reproducible Monte Carlo estimate of *recycled* summon tickets.
//!
//! Unlike the exact rarity/featured DP, four-star returns depend on the account's
//! inventory, and some exchange schedules are unknown. Every return is therefore
//! an explicit user-editable *assumption*. Never label these estimates exact.

use crate::{Banner, BannerRule, RateMode};
use crate::games::{Game, GameSetup};
use std::sync::atomic::{AtomicU64, Ordering};

// Bound CPU and memory use independently of the requested sample count.
const MAX_SAMPLES: usize = 1_000_000;
const MAX_MONTE_CARLO_WORKERS: usize = 8;
const MIN_SAMPLES_PER_WORKER: usize = 4_096;

/// One unit of `points` is the currency entered in the selected game's return store.
#[derive(Clone, Debug)]
pub struct FeedbackConfig {
    pub enabled: bool,
    pub points_per_ticket: usize,
    pub initial_points: usize,
    pub featured_first: usize,
    pub featured_duplicate: usize,
    /// Reward when a featured character is already fully upgraded (8th copy onward).
    pub featured_maxed: usize,
    pub off_banner_rare: usize,
    /// First copy of the featured 5★ weapon or record.
    pub weapon_first: usize,
    /// Subsequent copies or unconditional highest-rarity weapon payout.
    pub weapon_rare: usize,
    /// Average 4★ payout on the character banner; duplicates are assumed.
    pub four_star_reward: usize,
    /// Reward for a fully upgraded 4-star character (or A-rank character).
    pub four_star_maxed: usize,
    /// Assumed share of 4-star character hits that are already fully upgraded.
    /// Does not affect weapon / Arc outcomes; these retain their fixed payout.
    pub four_star_maxed_share: f64,
    /// Average 4★ payout on the weapon / record banner.
    pub weapon_four_star_reward: usize,
    pub character_four_star_rate: f64,
    pub weapon_four_star_rate: f64,
    /// Unconditional off-featured 5★ rate for featured-counter banners.
    pub character_other_five_star_rate: f64,
    pub weapon_other_five_star_rate: f64,
    pub four_star_hard: usize,
    /// Current 4★ or higher pity on the character banner.
    pub character_four_star_pity: usize,
    /// Current 4★ or higher pity on the weapon banner.
    pub weapon_four_star_pity: usize,
    pub samples: usize,
    /// 0 = all coral buys tickets; 1/2 = reserve enough for that many
    /// limited-character Wavebands before converting the rest to tickets.
    pub wuwa_waveband_max: usize,
    pub wuwa_waveband_cost: usize,
    /// Approximate per-board-roll chance of an extra character die (NTE only).
    pub nte_one_die_chance: f64,
    /// Approximate per-board-roll chance of five extra character dice (NTE only).
    pub nte_five_dice_chance: f64,
    /// Stella Sora limited recruitment awards Gold Disc units, never character passes.
    pub stella_record_discs_per_character_pull: usize,
    /// Account-specific first-time boosted character draws; zero if unknown or claimed.
    pub stella_first_ten_remaining: usize,
}

impl FeedbackConfig {
    pub fn for_game(game: Game) -> Self {
        match game {
            Game::StarRail => Self {
                enabled: true, points_per_ticket: 20, initial_points: 0,
                featured_first: 0, featured_duplicate: 40, featured_maxed: 100,
                off_banner_rare: 40, weapon_first: 40, weapon_rare: 40,
                four_star_reward: 8, four_star_maxed: 20, four_star_maxed_share: 0.0,
                weapon_four_star_reward: 8, character_four_star_rate: 0.051,
                weapon_four_star_rate: 0.066,
                character_other_five_star_rate: 0.0, weapon_other_five_star_rate: 0.0,
                four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000, wuwa_waveband_max: 0, wuwa_waveband_cost: 360,
                nte_one_die_chance: 0.0, nte_five_dice_chance: 0.0,
                stella_record_discs_per_character_pull: 0, stella_first_ten_remaining: 0,
            },
            Game::WutheringWaves => Self {
                enabled: true, points_per_ticket: 8, initial_points: 0,
                featured_first: 15, featured_duplicate: 15, featured_maxed: 15,
                off_banner_rare: 45, weapon_first: 15, weapon_rare: 15,
                four_star_reward: 3, four_star_maxed: 3, four_star_maxed_share: 0.0,
                weapon_four_star_reward: 3, character_four_star_rate: 0.06,
                weapon_four_star_rate: 0.06,
                character_other_five_star_rate: 0.0, weapon_other_five_star_rate: 0.0,
                four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000, wuwa_waveband_max: 0, wuwa_waveband_cost: 360,
                nte_one_die_chance: 0.0, nte_five_dice_chance: 0.0,
                stella_record_discs_per_character_pull: 0, stella_first_ten_remaining: 0,
            },
            Game::Neverness => Self {
                // Warp Pieces can buy dice / Tri-Keys. The 24-piece price and
                // missing board-tile rewards remain community assumptions.
                enabled: true, points_per_ticket: 24, initial_points: 0,
                featured_first: 0, featured_duplicate: 40, featured_maxed: 80,
                off_banner_rare: 0, weapon_first: 40, weapon_rare: 40,
                four_star_reward: 6, four_star_maxed: 12, four_star_maxed_share: 0.0,
                weapon_four_star_reward: 4,
                character_four_star_rate: 0.06, weapon_four_star_rate: 0.07,
                character_other_five_star_rate: 0.0, weapon_other_five_star_rate: 0.0,
                four_star_hard: 10, character_four_star_pity: 0,
                weapon_four_star_pity: 0, samples: 20_000, wuwa_waveband_max: 0, wuwa_waveband_cost: 360,
                nte_one_die_chance: 0.0157, nte_five_dice_chance: 0.0038,
                stella_record_discs_per_character_pull: 0, stella_first_ten_remaining: 0,
            },
            Game::StellaSora => Self {
                // Expert Permits: duplicate 5★ Trekker / Disc = 200,
                // duplicate 4★ Trekker / Disc = 40; 100 permits per pull.
                // First-time featured 5★ collections award no duplicate permits.
                // The 4★ return assumes all 4★ results are already owned.
                enabled: true, points_per_ticket: 100, initial_points: 0,
                featured_first: 0, featured_duplicate: 200, featured_maxed: 200,
                off_banner_rare: 200, weapon_first: 0, weapon_rare: 200,
                four_star_reward: 40, four_star_maxed: 40, four_star_maxed_share: 0.0,
                weapon_four_star_reward: 40,
                character_four_star_rate: 0.08, weapon_four_star_rate: 0.08,
                character_other_five_star_rate: 0.01, weapon_other_five_star_rate: 0.005,
                four_star_hard: 10, character_four_star_pity: 0,
                weapon_four_star_pity: 0, samples: 20_000, wuwa_waveband_max: 0, wuwa_waveband_cost: 360,
                nte_one_die_chance: 0.0, nte_five_dice_chance: 0.0,
                stella_record_discs_per_character_pull: 30, stella_first_ten_remaining: 0,
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.points_per_ticket == 0 || self.samples == 0 || self.samples > MAX_SAMPLES
            || self.four_star_hard == 0 || self.four_star_hard > 20
            || self.character_four_star_pity >= self.four_star_hard
            || self.weapon_four_star_pity >= self.four_star_hard
            || self.wuwa_waveband_max > 2 || self.wuwa_waveband_cost == 0
            || self.wuwa_waveband_cost > 100_000
            || !valid_probability(self.four_star_maxed_share)
            || !valid_probability(self.character_four_star_rate)
            || !valid_probability(self.weapon_four_star_rate)
            || !valid_probability(self.character_other_five_star_rate)
            || !valid_probability(self.weapon_other_five_star_rate)
            || !valid_probability(self.nte_one_die_chance)
            || !valid_probability(self.nte_five_dice_chance)
            || self.nte_one_die_chance + self.nte_five_dice_chance > 1.0
            || self.stella_record_discs_per_character_pull > 300
            || self.stella_first_ten_remaining > 10 {
            return Err("환급 설정의 가격·확률·표본 수가 허용 범위를 벗어났습니다.".to_owned());
        }
        Ok(())
    }
}

fn valid_probability(p: f64) -> bool {
    p.is_finite() && (0.0..=1.0).contains(&p)
}

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
    fn from_wallet(wallet: &Wallet, starting_points: usize) -> Self {
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

struct SplitMix64(u64);
impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
    }
    fn hits(&mut self, p: f64) -> bool { self.uniform() < p }
}

struct Wallet {
    points: usize,
    price: usize,
    common_spent: usize,
    initial_flexible_required: usize,
    raw_pulls: usize,
    waveband_points_spent: usize,
    wavebands_bought: usize,
    board_dice_earned: usize,
    record_disc_units: usize,
    stella_first_ten_remaining: usize,
    reward_multiplier: usize,
}
impl Wallet {
    fn new(model: &FeedbackConfig) -> Self {
        Self { points: model.initial_points, price: model.points_per_ticket,
            common_spent: 0, initial_flexible_required: 0, raw_pulls: 0,
            waveband_points_spent: 0, wavebands_bought: 0,
            board_dice_earned: 0, record_disc_units: 0,
            stella_first_ten_remaining: model.stella_first_ten_remaining,
            reward_multiplier: 1 }
    }
    /// The ticket for the current pull is payable only with returns obtained *before* that pull.
    fn pay(&mut self, dedicated_remaining: &mut usize) {
        self.raw_pulls += 1;
        if *dedicated_remaining > 0 { *dedicated_remaining -= 1; }
        else {
            self.common_spent += 1;
            self.reconcile_funding();
        }
    }
    fn refund(&mut self, reward: usize) {
        self.points = self.points.saturating_add(
            reward.saturating_mul(self.reward_multiplier));
    }

    /// Activate optional account-specific first-ten rewards and accrue record-only discs.
    fn begin_character_pull(&mut self, model: &FeedbackConfig) {
        self.record_disc_units = self.record_disc_units
            .saturating_add(model.stella_record_discs_per_character_pull);
        self.reward_multiplier = if self.stella_first_ten_remaining > 0 {
            self.stella_first_ten_remaining -= 1;
            3
        } else {
            1
        };
    }
    fn reconcile_funding(&mut self) {
        // Corals spent on a Waveband cannot also subsidize a ticket.
        let recycled = self.points.saturating_sub(self.waveband_points_spent) / self.price;
        self.initial_flexible_required = self.initial_flexible_required.max(
            self.common_spent.saturating_sub(recycled));
    }
    fn buy_waveband(&mut self, cost: usize) -> bool {
        if self.points.saturating_sub(self.waveband_points_spent) < cost { return false; }
        self.waveband_points_spent += cost;
        self.wavebands_bought += 1;
        self.reconcile_funding();
        true
    }
}

// Split by domain responsibility without changing the public 1.x API.
// The path runner only orchestrates bounded sampling; rolls owns banner rules.
#[path = "feedback/rolls.rs"] mod rolls;
#[path = "feedback/runner.rs"] mod runner;
use rolls::*;
pub use runner::{estimate, estimate_with_options};
#[cfg(test)]
use runner::estimate_with_workers;
#[cfg(test)]
#[path = "feedback/tests.rs"] mod tests;
