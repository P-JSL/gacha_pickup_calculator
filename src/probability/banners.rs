use crate::{Distribution, RateMode, RateSchedule, SoftPity};
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub enum BannerRule {
    /// A rare item resets rarity pity. Losing the featured chance guarantees the next.
    Standard { rarity: RateSchedule, featured_share: f64 },
    /// Pity counts *featured successes* and is NOT reset by off-banner rare items.
    FeaturedCounter { featured_rate: f64, hard_featured: usize },
    /// The NTE Arc banner draws one batch of 10 Arcs at a time.
    /// Exact independent natural draw counts per batch; guarantee substitution
    /// is a documented approximation because its precise internal ordering is unknown.
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
                let first_rare = rarity.hard.saturating_sub(self.pity);
                let first = if self.guaranteed || *featured_share >= 1.0 {
                    first_rare
                } else { first_rare.saturating_add(rarity.hard) };
                let later = if *featured_share >= 1.0 {
                    rarity.hard
                } else { rarity.hard.saturating_mul(2) };
                first.saturating_add(self.target_copies.saturating_sub(1).saturating_mul(later))
            }
            BannerRule::FeaturedCounter { hard_featured, .. } => {
                let normal = self.target_copies.saturating_mul(*hard_featured)
                    .saturating_sub(self.pity);
                if let Some(spark) = self.spark.as_ref().filter(|s| !s.used) {
                    let remaining = spark.at.saturating_sub(spark.progress);
                    let k_minus_one = (self.target_copies - 1)
                        .saturating_mul(*hard_featured).saturating_sub(self.pity);
                    normal.min(remaining.max(k_minus_one))
                } else { normal }
            },
            BannerRule::NteArcBatch => self.target_copies.saturating_mul(80)
                .saturating_sub(self.pity.saturating_mul(10)),
        }
    }

    /// Validate a caller-provided banner without constructing its full DP
    /// distribution. The Monte Carlo API uses this to reject invalid input
    /// without doubling expensive exact-distribution calculations.
    pub fn validate_inputs(&self) -> Result<(), String> {
        if self.target_copies > 15 {
            return Err("목표 획득 수가 허용 범위를 벗어났습니다 (최대 15).".into());
        }
        if self.target_copies == 0 { return Ok(()); }
        match &self.rule {
            BannerRule::Standard { rarity, featured_share } => {
                if rarity.hard == 0 || rarity.hard > 500 || self.pity >= rarity.hard
                    || !valid_probability(rarity.base)
                    || !valid_probability(*featured_share) || *featured_share == 0.0
                {
                    return Err("캐릭터/무기 천장 또는 확률 입력값이 유효하지 않습니다.".into());
                }
            }
            BannerRule::FeaturedCounter { featured_rate, hard_featured } => {
                if *hard_featured == 0 || *hard_featured > 500 || self.pity >= *hard_featured
                    || !valid_probability(*featured_rate)
                {
                    return Err("픽업 직접 확정 천장 또는 확률 입력값이 유효하지 않습니다.".into());
                }
                if self.spark.as_ref().is_some_and(|spark|
                    spark.at == 0 || spark.progress > spark.at)
                {
                    return Err("스파크 진행도 또는 횟수가 잘못되었습니다.".into());
                }
            }
            BannerRule::NteArcBatch => {
                if self.pity >= 8 || self.any_rare_pity >= 6 {
                    return Err("이환 Arc 천장은 8회(픽업), 6회(S등급) 미만의 10연 단위로 입력해야 합니다.".into());
                }
            }
        }
        Ok(())
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
        if rarity.hard == 0 || rarity.hard > 500 || self.pity >= rarity.hard
            || !valid_probability(rarity.base)
            || !valid_probability(featured) || featured == 0.0 {
            return Err("캐릭터/무기 천장 또는 확률 입력값이 유효하지 않습니다.".into());
        }
        let k = self.target_copies;
        let h = rarity.hard;
        let stride = h * 2;
        let mut cur = vec![0.0; k * stride];
        cur[self.pity * 2 + usize::from(self.guaranteed)] = 1.0;
        let mut pmf = vec![0.0; self.max_additional_pulls() + 1];
        // State dimensions and rarity schedule never change during this DP run.
        // Reuse a cleared buffer instead of allocating once per simulated pull.
        let mut next = vec![0.0; cur.len()];
        let chances: Vec<f64> = (1..=h).map(|draw| rarity.chance(draw, mode)).collect();
        for completed_probability in pmf.iter_mut().skip(1) {
            next.fill(0.0);
            for copies in 0..k {
                for pity in 0..h {
                    for g in 0..2 {
                        let mass = cur[copies * stride + pity * 2 + g];
                        if mass == 0.0 { continue; }
                        let rate = chances[pity];
                        let success = mass * rate * if g == 1 { 1.0 } else { featured };
                        let miss_feature = mass * rate - success;
                        let miss_rare = mass * (1.0 - rate);
                        if copies + 1 == k { *completed_probability += success; }
                        else { next[(copies + 1) * stride] += success; }
                        if miss_feature > 0.0 { next[copies * stride + 1] += miss_feature; }
                        if miss_rare > 0.0 {
                            next[copies * stride + (pity + 1) * 2 + g] += miss_rare;
                        }
                    }
                }
            }
            std::mem::swap(&mut cur, &mut next);
        }
        Ok(Distribution::new(pmf))
    }

    fn featured(&self, rate: f64, hard: usize) -> Result<Distribution, String> {
        if hard == 0 || hard > 500 || self.pity >= hard || !valid_probability(rate) {
            return Err("픽업 직접 확정 천장 또는 확률 입력값이 유효하지 않습니다.".into());
        }
        if let Some(spark) = &self.spark {
            if spark.at == 0 || spark.progress > spark.at {
                return Err("스파크 진행도 또는 횟수가 잘못되었습니다.".into());
            }
        }
        let k = self.target_copies;
        let claimed_at_start = self.spark.as_ref().is_some_and(|s| !s.used && s.progress >= s.at);
        if claimed_at_start && k == 1 { return Ok(Distribution::new(vec![1.0])); }
        let initial_copies = usize::from(claimed_at_start);
        let mut pmf = vec![0.0; self.max_additional_pulls() + 1];
        let mut cur = vec![0.0; k * hard];
        cur[initial_copies * hard + self.pity] = 1.0;
        let mut next = vec![0.0; cur.len()];
        for (step, completed_probability) in pmf.iter_mut().enumerate().skip(1) {
            next.fill(0.0);
            let spark_at_step = self.spark.as_ref().is_some_and(|s| !s.used && !claimed_at_start && s.at.saturating_sub(s.progress) == step);
            let bonus = usize::from(spark_at_step);
            for copies in 0..k {
                for pity in 0..hard {
                    let mass = cur[copies * hard + pity];
                    if mass == 0.0 { continue; }
                    let p = if pity + 1 >= hard { 1.0 } else { rate };
                    let win_copies = copies + 1 + bonus;
                    if win_copies >= k { *completed_probability += mass * p; }
                    else { next[win_copies * hard] += mass * p; }
                    let miss_copies = copies + bonus;
                    if miss_copies >= k { *completed_probability += mass * (1.0 - p); }
                    else if pity + 1 < hard { next[miss_copies * hard + pity + 1] += mass * (1.0 - p); }
                }
            }
            std::mem::swap(&mut cur, &mut next);
        }
        Ok(Distribution::new(pmf))
    }

    fn nte_arc(&self) -> Result<Distribution, String> {
        if self.pity >= 8 || self.any_rare_pity >= 6 {
            return Err("이환 Arc 천장은 8회(픽업), 6회(S등급) 미만의 10연 단위로 입력해야 합니다.".into());
        }
        // The whole ten-pull batch is paid for even when the desired copy appears early.
        // Natural outcomes are multinomial(10, featured=0.0075, off=0.0225, lower=0.97).
        // Published batch guarantees are applied after natural outcomes. The exact
        // in-batch guarantee substitution order is not disclosed by the game.
        let k = self.target_copies;
        let stride = 8 * 6;
        let mut cur = vec![0.0; k * stride];
        cur[self.pity * 6 + self.any_rare_pity] = 1.0;
        // The first guarantee already has 'self.pity' completed batches.
        // Stop exactly at the guaranteed maximum; do not expose trailing
        // unreachable entries to 100%-confidence quantiles or graphs.
        let max_batches = k * 8 - self.pity;
        let mut pmf = vec![0.0; max_batches * 10 + 1];
        let outcomes = arc_batch_outcomes();
        let mut next = vec![0.0; cur.len()];
        for batch in 1..=max_batches {
            next.fill(0.0);
            for copies in 0..k {
                for pickup_pity in 0..8 {
                    for any_pity in 0..6 {
                        let mass = cur[copies * stride + pickup_pity * 6 + any_pity];
                        if mass == 0.0 { continue; }
                        for &(natural_featured, natural_off, probability) in outcomes {
                            let rare = natural_featured + natural_off;
                            let (featured, off) = if pickup_pity == 7 && natural_featured == 0 {
                                // The eighth multi guarantees one featured S Arc.
                                (1, natural_off)
                            } else if any_pity == 5 && rare == 0 {
                                // The sixth multi guarantees one S Arc, 25% featured.
                                for (extra_featured, chance) in [(1_usize, 0.25), (0, 0.75)] {
                                    let total_featured = natural_featured + extra_featured;
                                    let total_copies = copies + total_featured;
                                    let weight = mass * probability * chance;
                                    if total_copies >= k { pmf[batch * 10] += weight; }
                                    else {
                                        let fp = if total_featured > 0 { 0 } else { pickup_pity + 1 };
                                        next[total_copies * stride + fp * 6] += weight;
                                    }
                                }
                                continue;
                            } else { (natural_featured, natural_off) };
                            let total_copies = copies + featured;
                            let weight = mass * probability;
                            if total_copies >= k { pmf[batch * 10] += weight; }
                            else {
                                let fp = if featured > 0 { 0 } else { pickup_pity + 1 };
                                let ap = if featured + off > 0 { 0 } else { any_pity + 1 };
                                next[total_copies * stride + fp * 6 + ap] += weight;
                            }
                        }
                    }
                }
            }
            std::mem::swap(&mut cur, &mut next);
        }
        Ok(Distribution::new(pmf))
    }
}

/// The analytical multinomial distribution of featured and non-featured S-Arcs
/// within ten independent natural draws (before batch guarantees).
fn arc_batch_outcomes() -> &'static [(usize, usize, f64)] {
    static OUTCOMES: OnceLock<Vec<(usize, usize, f64)>> = OnceLock::new();
    OUTCOMES.get_or_init(|| {
    let mut outcome = vec![(0, 0, 1.0)];
    for _ in 0..10 {
        let mut next = vec![vec![0.0; 11]; 11];
        for &(featured, off, p) in &outcome {
            next[featured][off] += p * 0.97;
            next[featured + 1][off] += p * 0.0075;
            next[featured][off + 1] += p * 0.0225;
        }
        outcome.clear();
        for (f, row) in next.iter().enumerate() {
            for (o, &probability) in row.iter().enumerate() {
                if probability > 0.0 { outcome.push((f, o, probability)); }
            }
        }
    }
    outcome
    }).as_slice()
}

fn valid_probability(x: f64) -> bool { x.is_finite() && (0.0..=1.0).contains(&x) }

