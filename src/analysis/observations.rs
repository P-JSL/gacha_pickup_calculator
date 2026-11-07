//! Offline tracker-observation validation with discrete Kaplan–Meier estimates.
//!
//! This corrects *independent right censoring only*: it cannot correct
//! self-selection, missing early history, banner mixing or left truncation.
//! No network, account tokens, tracking URLs, or external subprocesses are used.

use crate::{RateMode, RateSchedule};
use std::collections::HashSet;

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

#[path = "observations/parser.rs"] mod parser;
pub use parser::parse_observation_csv;

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
#[path = "observations/tests.rs"] mod tests;
