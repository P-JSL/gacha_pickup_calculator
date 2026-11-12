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

// Model boundaries: configuration, immutable result contracts, reusable
// random path wallet, banner rules, and bounded simulation orchestration.
#[path = "feedback/config.rs"] mod config;
#[path = "feedback/results.rs"] mod results;
pub use config::FeedbackConfig;
pub use results::{BudgetSuccessBreakdown, FeedbackEstimate, RefundScenario, SuccessfulBudgetMeans, QuantileConfidenceBand};

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
}
impl Wallet {
    fn new(model: &FeedbackConfig) -> Self {
        Self { points: model.initial_points, price: model.points_per_ticket,
            common_spent: 0, initial_flexible_required: 0, raw_pulls: 0,
            waveband_points_spent: 0, wavebands_bought: 0,
            board_dice_earned: 0, record_disc_units: 0,
            }
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
        self.points = self.points.saturating_add(reward);
    }

    /// Standard Stella character draw: 30 Gold Discs, usable only on records.
    /// An optional initial paid 10-pull grants its additional 600 discs in
    /// roll_featured; it never multiplies Expert Permits.
    fn begin_character_pull(&mut self, model: &FeedbackConfig) {
        self.record_disc_units = self.record_disc_units
            .saturating_add(model.stella_record_discs_per_character_pull);
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
pub use runner::{estimate, estimate_with_options, estimate_waveband_strategies};
#[cfg(test)]
use runner::estimate_with_workers;
#[cfg(test)]
#[path = "feedback/tests.rs"] mod tests;
