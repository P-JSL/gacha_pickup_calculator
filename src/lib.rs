//! Finite-state absorbing Markov/DP probability engine.
//!
//! `pity` is the number of completed attempts without the relevant success.
//! A rate schedule's `chance(next)` takes the 1-based attempt since reset.
//! All probability distributions describe *additional* attempts, not lifetime pulls.
//! Currency rebates are deliberately excluded from exact distributions: they depend
//! on account-specific duplicate history and cannot be inferred from banner odds.

pub mod games;

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
            // This is a documented state change rather than estimated soft pity.
            SoftPity::NteBoardChange if next >= 71 => 0.1959,
            _ => self.base,
        };
        guessed.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug)]
pub enum BannerRule {
    /// A rare item resets rarity pity. Losing the featured chance guarantees the next.
    Standard { rarity: RateSchedule, featured_share: f64 },
    /// Pity counts *featured successes* and is NOT reset by off-banner rare items.
    FeaturedCounter { featured_rate: f64, hard_featured: usize },
    /// The NTE Arc banner draws one batch of 10 Arcs at a time.
    /// This is a batch approximation: it does not count multiple featured Arcs
    /// appearing within the same batch. Guarantees are at batches 6 and 8.
    NteArcBatch,
}

#[derive(Clone, Debug)]
pub struct Spark {
    /// One-time copy awarded on reaching this banner's cumulative pull count.
    pub at: usize,
    pub progress: usize,
    pub used: bool,
}

#[derive(Clone, Debug)]
pub struct Banner {
    pub rule: BannerRule,
    pub pity: usize,
    pub guaranteed: bool,
    pub target_copies: usize,
    pub spark: Option<Spark>,
    /// Only used by NTE Arc: multi-batches since last arbitrary S-rank Arc.
    pub any_rare_pity: usize,
}

impl Banner {
    pub fn max_additional_pulls(&self) -> usize {
        if self.target_copies == 0 { return 0; }
        match &self.rule {
            BannerRule::Standard { rarity, featured_share } => {
                (self.target_copies * if *featured_share < 1.0 { 2 } else { 1 }) * rarity.hard
            }
            BannerRule::FeaturedCounter { hard_featured, .. } => self.target_copies * hard_featured,
            BannerRule::NteArcBatch => self.target_copies * 80,
        }
    }

    pub fn distribution(&self, mode: RateMode) -> Result<Distribution, String> {
        if self.target_copies > 15 {
            return Err("목표 획득 수가 허용 범위를 벗어났습니다 (최대 15).".into());
        }
        if self.target_copies == 0 { return Ok(Distribution::new(vec![1.0])); }
        match &self.rule {
            BannerRule::Standard { rarity, featured_share } => self.standard(rarity, *featured_share, mode),
            BannerRule::FeaturedCounter { featured_rate, hard_featured } => self.featured(*featured_rate, *hard_featured),
            BannerRule::NteArcBatch => self.nte_arc(),
        }
    }

    fn standard(&self, rarity: &RateSchedule, featured: f64, mode: RateMode) -> Result<Distribution, String> {
        if rarity.hard == 0 || self.pity >= rarity.hard || !valid_probability(rarity.base)
            || !valid_probability(featured) || featured == 0.0 {
            return Err("캐릭터/무기 천장 또는 확률 입력값이 유효하지 않습니다.".into());
        }
        let k = self.target_copies;
        let h = rarity.hard;
        let stride = h * 2;
        let mut cur = vec![0.0; k * stride];
        cur[self.pity * 2 + usize::from(self.guaranteed)] = 1.0;
        let mut pmf = vec![0.0; self.max_additional_pulls() + 1];
        for step in 1..pmf.len() {
            let mut next = vec![0.0; cur.len()];
            for copies in 0..k {
                for pity in 0..h {
                    for g in 0..2 {
                        let mass = cur[copies * stride + pity * 2 + g];
                        if mass == 0.0 { continue; }
                        let rate = rarity.chance(pity + 1, mode);
                        let success = mass * rate * if g == 1 { 1.0 } else { featured };
                        let miss_feature = mass * rate - success;
                        let miss_rare = mass * (1.0 - rate);
                        if copies + 1 == k { pmf[step] += success; }
                        else { next[(copies + 1) * stride] += success; }
                        if miss_feature > 0.0 { next[copies * stride + 1] += miss_feature; }
                        if miss_rare > 0.0 {
                            next[copies * stride + (pity + 1) * 2 + g] += miss_rare;
                        }
                    }
                }
            }
            cur = next;
        }
        Ok(Distribution::new(pmf))
    }

    fn featured(&self, rate: f64, hard: usize) -> Result<Distribution, String> {
        if hard == 0 || self.pity >= hard || !valid_probability(rate) {
            return Err("픽업 직접 확정 천장 또는 확률 입력값이 유효하지 않습니다.".into());
        }
        if let Some(spark) = &self.spark {
            if spark.at == 0 || spark.progress > spark.at {
                return Err("스파크 진행도 또는 횟수가 잘못되었습니다.".into());
            }
        }
        let k = self.target_copies;
        let mut pmf = vec![0.0; self.max_additional_pulls() + 1];
        let mut cur = vec![0.0; k * hard];
        cur[self.pity] = 1.0;
        for step in 1..pmf.len() {
            let mut next = vec![0.0; cur.len()];
            let spark_at_step = self.spark.as_ref().is_some_and(|s| !s.used && s.progress + step == s.at);
            let bonus = usize::from(spark_at_step);
            for copies in 0..k {
                for pity in 0..hard {
                    let mass = cur[copies * hard + pity];
                    if mass == 0.0 { continue; }
                    let p = if pity + 1 >= hard { 1.0 } else { rate };
                    let win_copies = copies + 1 + bonus;
                    if win_copies >= k { pmf[step] += mass * p; }
                    else { next[win_copies * hard] += mass * p; }
                    let miss_copies = copies + bonus;
                    if miss_copies >= k { pmf[step] += mass * (1.0 - p); }
                    else if pity + 1 < hard { next[miss_copies * hard + pity + 1] += mass * (1.0 - p); }
                }
            }
            cur = next;
        }
        Ok(Distribution::new(pmf))
    }

    fn nte_arc(&self) -> Result<Distribution, String> {
        if self.pity >= 8 || self.any_rare_pity >= 6 {
            return Err("이환 Arc 천장은 8회(픽업), 6회(S등급) 미만의 10연 단위로 입력해야 합니다.".into());
        }
        let k = self.target_copies;
        let stride = 8 * 6;
        let mut cur = vec![0.0; k * stride];
        cur[self.pity * 6 + self.any_rare_pity] = 1.0;
        let mut pmf = vec![0.0; (self.target_copies * 8 + 1) * 10];
        let natural_any = 1.0_f64 - (1.0_f64 - 0.03_f64).powi(10);
        let natural_featured = 1.0_f64 - (1.0_f64 - 0.03_f64 * 0.25_f64).powi(10);
        for batch in 1..=k * 8 {
            let mut next = vec![0.0; cur.len()];
            for copies in 0..k {
                for pickup_pity in 0..8 {
                    for any_pity in 0..6 {
                        let mass = cur[copies * stride + pickup_pity * 6 + any_pity];
                        if mass == 0.0 { continue; }
                        let (p_feat, p_any) = if pickup_pity == 7 { (1.0, 1.0) }
                            else if any_pity == 5 { (0.25, 1.0) }
                            else { (natural_featured, natural_any) };
                        if copies + 1 == k { pmf[batch * 10] += mass * p_feat; }
                        else { next[(copies + 1) * stride] += mass * p_feat; }
                        next[copies * stride + (pickup_pity + 1) * 6] += mass * (p_any - p_feat);
                        if p_any < 1.0 {
                            next[copies * stride + (pickup_pity + 1) * 6 + any_pity + 1] += mass * (1.0 - p_any);
                        }
                    }
                }
            }
            cur = next;
        }
        Ok(Distribution::new(pmf))
    }
}

fn valid_probability(x: f64) -> bool { x.is_finite() && (0.0..=1.0).contains(&x) }

#[derive(Clone, Debug)]
pub struct Distribution {
    pub pmf: Vec<f64>,
    pub cdf: Vec<f64>,
    pub mean: f64,
}

impl Distribution {
    pub fn new(pmf: Vec<f64>) -> Self {
        let mut cdf = Vec::with_capacity(pmf.len());
        let mut acc = 0.0;
        let mut mean = 0.0;
        for (n, &p) in pmf.iter().enumerate() {
            acc += p;
            mean += n as f64 * p;
            cdf.push(acc.min(1.0));
        }
        Self { pmf, cdf, mean }
    }

    pub fn at(&self, pulls: usize) -> f64 {
        self.cdf.get(pulls).copied().unwrap_or_else(|| self.cdf.last().copied().unwrap_or(0.0))
    }

    pub fn quantile(&self, chance: f64) -> usize {
        self.cdf.iter().position(|&p| p + 1e-10 >= chance).unwrap_or(self.cdf.len().saturating_sub(1))
    }

    /// Exact convolution of independent *first-passage* (stopping-time) distributions.
    pub fn combine(&self, other: &Self) -> Self {
        let mut combined = vec![0.0; self.pmf.len() + other.pmf.len() - 1];
        for (i, &a) in self.pmf.iter().enumerate() {
            if a == 0.0 { continue; }
            for (j, &b) in other.pmf.iter().enumerate() {
                combined[i + j] += a * b;
            }
        }
        Self::new(combined)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Allocation {
    pub character_pulls: usize,
    pub weapon_pulls: usize,
    pub success_probability: f64,
}

/// Optimal split of universally exchangeable pulls between two independent banners.
/// Dedicated tickets may not be transferred between banner types.
pub fn optimal_allocation(
    character: &Distribution,
    weapon: &Distribution,
    dedicated_character: usize,
    dedicated_weapon: usize,
    flexible: usize,
) -> Allocation {
    let mut answer = Allocation::default();
    for given_to_character in 0..=flexible {
        let c = dedicated_character.saturating_add(given_to_character);
        let w = dedicated_weapon.saturating_add(flexible - given_to_character);
        let success = character.at(c) * weapon.at(w);
        if success > answer.success_probability || given_to_character == 0 {
            answer = Allocation { character_pulls: c, weapon_pulls: w, success_probability: success };
        }
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn always_rare(share: f64, guaranteed: bool, count: usize) -> Banner {
        Banner {
            rule: BannerRule::Standard { rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None }, featured_share: share },
            pity: 0, guaranteed, target_copies: count, spark: None, any_rare_pity: 0,
        }
    }

    #[test]
    fn zero_goal_completes_without_a_pull() {
        let result = always_rare(0.5, false, 0).distribution(RateMode::PublishedOnly).unwrap();
        assert!((result.at(0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn half_rate_then_guaranteed() {
        let result = always_rare(0.5, false, 1).distribution(RateMode::PublishedOnly).unwrap();
        assert!((result.at(1) - 0.5).abs() < 1e-12);
        assert!((result.at(2) - 1.0).abs() < 1e-12);
        assert!((result.mean - 1.5).abs() < 1e-12);
    }

    #[test]
    fn guaranteed_first_rare_stays_guaranteed() {
        let result = always_rare(0.5, true, 1).distribution(RateMode::PublishedOnly).unwrap();
        assert!((result.at(1) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn repeated_targets_normalize_and_monotone() {
        let result = always_rare(0.5, false, 7).distribution(RateMode::PublishedOnly).unwrap();
        assert!((result.at(14) - 1.0).abs() < 1e-10);
        for window in result.cdf.windows(2) {
            assert!(window[1] + 1e-12 >= window[0]);
        }
    }

    #[test]
    fn spark_is_independent_of_pity_reset() {
        let result = Banner {
            rule: BannerRule::FeaturedCounter { featured_rate: 0.0, hard_featured: 160 },
            pity: 0, guaranteed: false, target_copies: 1,
            spark: Some(Spark { at: 120, progress: 0, used: false }), any_rare_pity: 0,
        }.distribution(RateMode::PublishedOnly).unwrap();
        assert_eq!(result.at(119), 0.0);
        assert_eq!(result.at(120), 1.0);
    }

    #[test]
    fn hard_pity_works_without_soft_pity() {
        let result = Banner {
            rule: BannerRule::Standard { rarity: RateSchedule { base: 0.0, hard: 90, soft: SoftPity::None }, featured_share: 1.0 },
            pity: 3, guaranteed: false, target_copies: 1, spark: None, any_rare_pity: 0,
        }.distribution(RateMode::PublishedOnly).unwrap();
        assert_eq!(result.at(86), 0.0);
        assert_eq!(result.at(87), 1.0);
    }

    #[test]
    fn nte_arc_guarantees_in_eight_batches() {
        let result = Banner {
            rule: BannerRule::NteArcBatch, pity: 0, guaranteed: false, target_copies: 1,
            spark: None, any_rare_pity: 0,
        }.distribution(RateMode::PublishedOnly).unwrap();
        assert!((result.at(80) - 1.0).abs() < 1e-12);
        assert_eq!(result.at(9), 0.0);
        assert!(result.at(10) > 0.0);
        assert_eq!(result.at(79), result.at(70));
    }

    #[test]
    fn convolution_and_allocation() {
        let a = always_rare(1.0, false, 2).distribution(RateMode::PublishedOnly).unwrap();
        let b = always_rare(1.0, false, 3).distribution(RateMode::PublishedOnly).unwrap();
        let together = a.combine(&b);
        assert_eq!(together.quantile(0.95), 5);
        let alloc = optimal_allocation(&a, &b, 0, 0, 5);
        assert_eq!(alloc.character_pulls, 2);
        assert_eq!(alloc.weapon_pulls, 3);
        assert!((alloc.success_probability - 1.0).abs() < 1e-12);
    }

    #[test]
    fn wuthering_soft_starts_after_65() {
        let model = RateSchedule { base: 0.008, hard: 80, soft: SoftPity::WutheringEstimated };
        assert_eq!(model.chance(65, RateMode::CommunityEstimate), 0.008);
        assert!(model.chance(66, RateMode::CommunityEstimate) > 0.008);
        assert_eq!(model.chance(80, RateMode::CommunityEstimate), 1.0);
        assert_eq!(model.chance(79, RateMode::PublishedOnly), 0.008);
    }
}