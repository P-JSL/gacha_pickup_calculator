#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateMode {
    /// Use only officially published base and hard guarantee numbers.
    PublishedOnly,
    /// Opt into explicitly labeled, non-official soft-pity estimates.
    CommunityEstimate,
}

#[derive(Clone, Debug)]
pub struct RateSchedule {
    pub base: f64,
    pub hard: usize,
    pub soft: SoftPity,
}

#[derive(Clone, Debug)]
pub enum SoftPity {
    None,
    /// Start at this 1-based draw and add `increment` each subsequent attempt.
    Linear { starts_at: usize, increment: f64 },
    /// Wuthering Waves community sample-based piecewise approximation.
    WutheringEstimated,
    /// NTE board changes to a fixed success probability after 70 failures.
    NteBoardChange,
}

impl RateSchedule {
    /// Exact first-rare expectation for this *specified hazard schedule* at
    /// zero pity: E[T] = sum_{k=0}^{H-1} prod_{i=1}^{k}(1-h(i)).
    /// It is not an estimate of the game's unpublished server-side hazard.
    pub fn expected_first_rare_pulls(&self, mode: RateMode) -> Option<f64> {
        if self.hard == 0 || self.hard > 500 || !self.base.is_finite()
            || !(0.0..=1.0).contains(&self.base)
        {
            return None;
        }
        let mut survival = 1.0;
        let mut expected = 0.0;
        for draw in 1..=self.hard {
            expected += survival;
            survival *= 1.0 - self.chance(draw, mode);
        }
        Some(expected)
    }

    /// Renewal rate for an unlimited sequence of rare outcomes in this model;
    /// it is not the featured-character probability or the published base rate.
    pub fn implied_rare_rate(&self, mode: RateMode) -> Option<f64> {
        self.expected_first_rare_pulls(mode).map(|mean| 1.0 / mean)
    }

    pub fn chance(&self, next: usize, mode: RateMode) -> f64 {
        if next >= self.hard {
            return 1.0;
        }
        let guessed = match &self.soft {
            SoftPity::None => self.base,
            SoftPity::Linear { starts_at, increment } if mode == RateMode::CommunityEstimate => {
                self.base + next.saturating_sub(*starts_at).saturating_add(usize::from(next >= *starts_at)) as f64 * increment
            }
            SoftPity::WutheringEstimated if mode == RateMode::CommunityEstimate => {
                let x = next;
                self.base
                    + x.saturating_sub(65).min(5) as f64 * 0.04
                    + x.saturating_sub(70).min(5) as f64 * 0.08
                    + x.saturating_sub(75).min(4) as f64 * 0.10
            }
            // Position-dependent board probabilities are not publicly specified.
            // In community mode only, fit conditional hazards to published
            // fan-estimated cumulative first-S results; do not call this a
            // fully simulated 72-cell Markov board.
            SoftPity::NteBoardChange if mode == RateMode::CommunityEstimate => {
                nte_community_hazard(next)
            }
            // Official-mode estimates retain the currently documented
            // 0.99% base, 19.59% transformed board, and 90th guarantee.
            SoftPity::NteBoardChange if next >= 71 => 0.1959,
            _ => self.base,
        };
        guessed.clamp(0.0, 1.0)
    }
}

/// Community board study (not an official probability table):
/// https://arca.live/b/nevernesstoeverness/185272303
/// For pity-free starts, these anchors reproduce the study's cumulative
/// probability of *first* S-character. Piecewise conditional hazards replace
/// unknown board positions; they are NOT a complete Markov transition model.
pub(super) const NTE_COMMUNITY_FIRST_S_CDF: &[(usize, f64)] = &[
    (0, 0.0), (10, 0.0913), (20, 0.1788), (30, 0.2559),
    (40, 0.3281), (50, 0.3917), (60, 0.4500), (70, 0.5027),
    (71, 0.5986), (80, 0.9424), (89, 0.9918),
];

fn nte_community_hazard(next: usize) -> f64 {
    for pair in NTE_COMMUNITY_FIRST_S_CDF.windows(2) {
        let (start, earlier) = pair[0];
        let (end, later) = pair[1];
        if next > start && next <= end {
            let survival_ratio = (1.0 - later) / (1.0 - earlier);
            return 1.0 - survival_ratio.powf(1.0 / (end - start) as f64);
        }
    }
    0.0
}


#[cfg(test)]
mod expectation_tests {
    use super::*;

    #[test]
    fn survival_sum_matches_closed_form_and_hard_guarantee() {
        let model = RateSchedule {
            base: 0.1, hard: 3, soft: SoftPity::None
        };
        let mean = model.expected_first_rare_pulls(RateMode::PublishedOnly).unwrap();
        assert!((mean - (1.0 + 0.9 + 0.9 * 0.9)).abs() < 1e-12);
        assert!((model.implied_rare_rate(RateMode::PublishedOnly).unwrap()
            - 1.0 / 2.71).abs() < 1e-12);
        let certain = RateSchedule { base: 0.0, hard: 80, soft: SoftPity::None };
        assert_eq!(certain.expected_first_rare_pulls(RateMode::PublishedOnly), Some(80.0));
    }

    #[test]
    fn calibration_profiles_do_not_confuse_base_and_consolidated_rates() {
        let character = RateSchedule {
            base: 0.006, hard: 90,
            soft: SoftPity::Linear { starts_at: 74, increment: 0.06 }
        };
        // Community profile is an empirical approximation of the published
        // 1.6% consolidated 5★ rate; the official exact hazard is unknown.
        let published = character.implied_rare_rate(RateMode::PublishedOnly).unwrap();
        let estimate = character.implied_rare_rate(RateMode::CommunityEstimate).unwrap();
        assert!((published - 0.0143472426868).abs() < 1e-7);
        assert!((estimate - 0.016).abs() < 0.0002);
        assert!(estimate > published);
    }
}
