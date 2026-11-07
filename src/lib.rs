//! Finite-state absorbing Markov/DP probability engine.
#![forbid(unsafe_code)]
//!
//! `pity` is the number of completed attempts without the relevant success.
//! A rate schedule's `chance(next)` takes the 1-based attempt since reset.
//! All probability distributions describe *additional* attempts, not lifetime pulls.
//! Currency rebates are handled separately by a clearly labeled path simulation.
//! Raw first-passage distributions are calculated by deterministic DP.

#[path = "domain/games.rs"] pub mod games;
#[path = "domain/feedback.rs"] pub mod feedback;
#[path = "analysis/tracker.rs"] pub mod tracker;
#[path = "analysis/observations.rs"] pub mod observations;


#[path = "probability/rates.rs"] mod rates;
#[path = "probability/banners.rs"] mod banners;
#[path = "probability/distributions.rs"] mod distributions;
#[path = "probability/fft.rs"] mod fft;

// Keep all public 1.x entrypoints at the crate root.
pub use rates::{RateMode, RateSchedule, SoftPity};
pub use banners::{Banner, BannerRule, Spark};
pub use distributions::{Allocation, Distribution, optimal_allocation};

// Domain regression cases are kept outside the public library entrypoint.
#[cfg(test)]
#[path = "probability/tests.rs"] mod tests;
