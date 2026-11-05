//! Reproducible Monte Carlo estimate of *recycled* summon tickets.
//!
//! Unlike the exact rarity/featured DP, four-star returns depend on the account's
//! inventory, and some exchange schedules are unknown. Every return is therefore
//! an explicit user-editable *assumption*. Never label these estimates exact.

use crate::{Banner, BannerRule, RateMode};
use crate::games::{Game, GameSetup};

/// One unit of `points` is the currency entered in the selected game's return store.
#[derive(Clone, Debug)]
pub struct FeedbackConfig {
    pub enabled: bool,
    pub points_per_ticket: usize,
    pub initial_points: usize,
    pub featured_first: usize,
    pub featured_duplicate: usize,
    pub off_banner_rare: usize,
    pub weapon_rare: usize,
    pub four_star_reward: usize,
    pub character_four_star_rate: f64,
    pub weapon_four_star_rate: f64,
    pub four_star_hard: usize,
    /// Current 4★ or higher pity on the character banner.
    pub character_four_star_pity: usize,
    /// Current 4★ or higher pity on the weapon banner.
    pub weapon_four_star_pity: usize,
    pub samples: usize,
}

impl FeedbackConfig {
    pub fn for_game(game: Game) -> Self {
        match game {
            Game::StarRail => Self {
                enabled: true, points_per_ticket: 20, initial_points: 0,
                featured_first: 0, featured_duplicate: 40,
                off_banner_rare: 0, weapon_rare: 40,
                four_star_reward: 8, character_four_star_rate: 0.051,
                weapon_four_star_rate: 0.066, four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000,
            },
            Game::WutheringWaves => Self {
                enabled: true, points_per_ticket: 8, initial_points: 0,
                featured_first: 15, featured_duplicate: 15,
                off_banner_rare: 15, weapon_rare: 15,
                four_star_reward: 3, character_four_star_rate: 0.06,
                weapon_four_star_rate: 0.06, four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000,
            },
            Game::Neverness | Game::StellaSora => Self {
                // Game-specific exchange conditions have not been verified.
                // Manual custom model is supported, but is OFF by default.
                enabled: false, points_per_ticket: 1, initial_points: 0,
                featured_first: 0, featured_duplicate: 0,
                off_banner_rare: 0, weapon_rare: 0,
                four_star_reward: 0, character_four_star_rate: 0.0,
                weapon_four_star_rate: 0.0, four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000,
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.points_per_ticket == 0 || self.samples == 0 || self.samples > 200_000
            || self.four_star_hard == 0 || self.four_star_hard > 20
            || self.character_four_star_pity >= self.four_star_hard
            || self.weapon_four_star_pity >= self.four_star_hard
            || !valid_probability(self.character_four_star_rate)
            || !valid_probability(self.weapon_four_star_rate) {
            return Err("환급 설정의 가격·확률·표본 수가 허용 범위를 벗어났습니다.".to_owned());
        }
        Ok(())
    }
}

fn valid_probability(p: f64) -> bool {
    p.is_finite() && (0.0..=1.0).contains(&p)
}

#[derive(Clone, Debug)]
pub struct FeedbackEstimate {
    /// Sorted minimum *additional flexible tickets* needed after dedicated tickets.
    pub budgets: Vec<usize>,
    pub raw_pulls: Vec<usize>,
    pub average_raw: f64,
    pub average_initial_flexible: f64,
    pub total_returned_points_average: f64,
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
        if n == 0 { return 0; }
        let rank = (confidence.clamp(0.0, 1.0) * n as f64).ceil() as usize;
        self.budgets[rank.saturating_sub(1).min(n - 1)]
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
}
impl Wallet {
    fn new(model: &FeedbackConfig) -> Self {
        Self { points: model.initial_points, price: model.points_per_ticket,
            common_spent: 0, initial_flexible_required: 0, raw_pulls: 0 }
    }
    /// The ticket for the current pull is payable only with returns obtained *before* that pull.
    fn pay(&mut self, dedicated_remaining: &mut usize) {
        self.raw_pulls += 1;
        if *dedicated_remaining > 0 { *dedicated_remaining -= 1; }
        else {
            self.common_spent += 1;
            let recycled_so_far = self.points / self.price;
            self.initial_flexible_required = self.initial_flexible_required.max(
                self.common_spent.saturating_sub(recycled_so_far));
        }
    }
    fn refund(&mut self, reward: usize) { self.points += reward; }
}

#[derive(Clone, Copy)]
enum BannerKind { Character, Weapon }

fn maybe_four_star(rng: &mut SplitMix64, model: &FeedbackConfig, kind: BannerKind,
                   four_pity: &mut usize, rare: bool) -> bool {
    // The 4★ return model is approximate: exact 4★ item ownership, featured
    // status, cross-rarity precedence and separate 4★ pity are not available.
    if rare { *four_pity = 0; return false; }
    let chance = match kind {
        BannerKind::Character => model.character_four_star_rate,
        BannerKind::Weapon => model.weapon_four_star_rate,
    };
    let hit = *four_pity + 1 >= model.four_star_hard || rng.hits(chance);
    if hit { *four_pity = 0; } else { *four_pity += 1; }
    hit
}

fn award_featured(wallet: &mut Wallet, model: &FeedbackConfig, owned: usize,
                  copies_this_run: usize, kind: BannerKind) {
    let amount = match kind {
        BannerKind::Weapon => model.weapon_rare,
        BannerKind::Character if owned + copies_this_run == 0 => model.featured_first,
        BannerKind::Character => model.featured_duplicate,
    };
    wallet.refund(amount);
}

fn roll_standard(rng: &mut SplitMix64, rule: &Banner, kind: BannerKind,
                 mode: RateMode, model: &FeedbackConfig, wallet: &mut Wallet,
                 (owned, dedicated): (usize, &mut usize)) {
    let BannerRule::Standard { rarity, featured_share } = &rule.rule else { return; };
    let mut pity = rule.pity;
    let mut guaranteed = rule.guaranteed;
    let mut won = 0;
    let mut four_pity = match kind {
        BannerKind::Character => model.character_four_star_pity,
        BannerKind::Weapon => model.weapon_four_star_pity,
    };
    while won < rule.target_copies {
        wallet.pay(dedicated);
        let rare = rng.hits(rarity.chance(pity + 1, mode));
        if maybe_four_star(rng, model, kind, &mut four_pity, rare) {
            wallet.refund(model.four_star_reward);
        }
        if rare {
            pity = 0;
            if guaranteed || rng.hits(*featured_share) {
                award_featured(wallet, model, owned, won, kind);
                won += 1;
                guaranteed = false;
            } else {
                wallet.refund(model.off_banner_rare);
                guaranteed = true;
            }
        } else { pity += 1; }
    }
}

fn roll_featured(rng: &mut SplitMix64, rule: &Banner, kind: BannerKind,
                 model: &FeedbackConfig, wallet: &mut Wallet,
                 owned: usize, dedicated: &mut usize) {
    let BannerRule::FeaturedCounter { featured_rate, hard_featured } = &rule.rule else { return; };
    let mut pity = rule.pity;
    let mut won = 0;
    let mut four_pity = match kind {
        BannerKind::Character => model.character_four_star_pity,
        BannerKind::Weapon => model.weapon_four_star_pity,
    };
    let already_unclaimed = rule.spark.as_ref().is_some_and(|s| !s.used && s.progress >= s.at);
    if already_unclaimed { won += 1; }
    let mut progress = rule.spark.as_ref().map_or(0, |s| s.progress);
    while won < rule.target_copies {
        wallet.pay(dedicated);
        let success = rng.hits(if pity + 1 >= *hard_featured { 1.0 } else { *featured_rate });
        if maybe_four_star(rng, model, kind, &mut four_pity, success) {
            wallet.refund(model.four_star_reward);
        }
        if success {
            pity = 0;
            award_featured(wallet, model, owned, won, kind);
            won += 1;
        } else { pity += 1; }
        progress += 1;
        if rule.spark.as_ref().is_some_and(|s| !s.used && !already_unclaimed && progress == s.at) {
            // Free exchange reward: not an additional RNG 5★ draw.
            won += 1;
        }
    }
}

fn roll_arc(rng: &mut SplitMix64, rule: &Banner, model: &FeedbackConfig,
            wallet: &mut Wallet, owned: usize, dedicated: &mut usize) {
    let mut featured_pity = rule.pity;
    let mut any_pity = rule.any_rare_pity;
    let mut won = 0;
    let mut four_pity = model.weapon_four_star_pity;
    while won < rule.target_copies {
        // Arc is purchased as an indivisible ten-pull. Refunds from the
        // current pack must not finance any of its remaining nine pulls.
        for _ in 0..10 { wallet.pay(dedicated); }
        let mut featured = 0;
        let mut off = 0;
        let mut pending_four_star_points = 0;
        for _ in 0..10 {
            let x = rng.uniform();
            if x < 0.0075 { featured += 1; four_pity = 0; }
            else if x < 0.03 { off += 1; four_pity = 0; }
            else if maybe_four_star(rng, model, BannerKind::Weapon, &mut four_pity, false) {
                pending_four_star_points += model.four_star_reward;
            }
        }
        wallet.refund(pending_four_star_points);
        if featured_pity == 7 && featured == 0 { featured = 1; }
        else if any_pity == 5 && featured + off == 0 {
            if rng.hits(0.25) { featured += 1; } else { off += 1; }
        }
        for _ in 0..featured {
            award_featured(wallet, model, owned, won, BannerKind::Weapon);
            won += 1;
        }
        wallet.refund(off * model.off_banner_rare);
        if featured > 0 { featured_pity = 0; } else { featured_pity += 1; }
        if featured + off > 0 { any_pity = 0; } else { any_pity += 1; }
    }
}

/// Deterministic for a fixed seed, but stochastic as an estimator.
/// Pull order: character banner to goal, then weapon banner to goal.
/// A common store balance is shared throughout both phases.
pub fn estimate(setup: &GameSetup, mode: RateMode,
                model: &FeedbackConfig) -> Result<FeedbackEstimate, String> {
    model.validate()?;
    let mut rng = SplitMix64(0xF17E_1234_ABCD_0905);
    let mut budgets = Vec::with_capacity(model.samples);
    let mut raw_pulls = Vec::with_capacity(model.samples);
    let mut total_raw = 0.0;
    let mut total_flexible = 0.0;
    let mut total_returns = 0.0;
    for _ in 0..model.samples {
        let mut wallet = Wallet::new(model);
        let mut dedicated_character = setup.character_dedicated;
        let mut dedicated_weapon = setup.weapon_dedicated;
        for (kind, banner, owned, dedicated) in [
            (BannerKind::Character, &setup.character, setup.character_owned, &mut dedicated_character),
            (BannerKind::Weapon, &setup.weapon, setup.weapon_owned, &mut dedicated_weapon),
        ] {
            match &banner.rule {
                BannerRule::Standard { .. } => {
                    roll_standard(&mut rng, banner, kind, mode, model, &mut wallet, (owned, dedicated));
                }
                BannerRule::FeaturedCounter { .. } => {
                    roll_featured(&mut rng, banner, kind, model, &mut wallet, owned, dedicated);
                }
                BannerRule::NteArcBatch => {
                    roll_arc(&mut rng, banner, model, &mut wallet, owned, dedicated);
                }
            }
        }
        budgets.push(wallet.initial_flexible_required);
        raw_pulls.push(wallet.raw_pulls);
        total_raw += wallet.raw_pulls as f64;
        total_flexible += wallet.initial_flexible_required as f64;
        total_returns += (wallet.points - model.initial_points) as f64;
    }
    budgets.sort_unstable();
    let n = model.samples as f64;
    Ok(FeedbackEstimate { budgets, raw_pulls,
        average_raw: total_raw / n, average_initial_flexible: total_flexible / n,
        total_returned_points_average: total_returns / n })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::{Game, GameSetup};
    use crate::{Banner, BannerRule, RateSchedule, SoftPity};

    #[test]
    fn wallet_requires_ticket_before_earning_refund() {
        let conf = FeedbackConfig { enabled: true, points_per_ticket: 1, initial_points: 0,
            featured_first: 1, featured_duplicate: 1, off_banner_rare: 0,
            weapon_rare: 1, four_star_reward: 0, character_four_star_rate: 0.0,
            weapon_four_star_rate: 0.0, four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0, samples: 10 };
        let mut w = Wallet::new(&conf);
        let mut d = 0;
        w.pay(&mut d);
        w.refund(1);
        w.pay(&mut d);
        w.refund(1);
        assert_eq!(w.initial_flexible_required, 1);
        assert_eq!(w.raw_pulls, 2);
    }

    #[test]
    fn zero_return_matches_raw_counts() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.character = Banner { rule: BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None }, featured_share: 1.0 },
            pity: 0, guaranteed: false, target_copies: 2, spark: None, any_rare_pity: 0 };
        setup.weapon.target_copies = 0;
        let mut cfg = FeedbackConfig::for_game(Game::Neverness);
        cfg.samples = 100;
        let v = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(v.quantile(1.0), 2);
        assert_eq!(v.probability(1).0, 0.0);
        assert_eq!(v.probability(2).0, 1.0);
    }

    #[test]
    fn arc_batch_requires_all_ten_tickets_upfront() {
        let mut setup = GameSetup::new(Game::Neverness);
        setup.character.target_copies = 0;
        setup.weapon.target_copies = 1;
        let mut cfg = FeedbackConfig::for_game(Game::Neverness);
        cfg.samples = 200;
        cfg.weapon_rare = 100;
        cfg.points_per_ticket = 1;
        let result = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert!(result.budgets.iter().all(|&n| n >= 10));
    }

    #[test]
    fn feedback_with_dedicated_ticket_increases_success() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.character = Banner { rule: BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None }, featured_share: 1.0 },
            pity: 0, guaranteed: true, target_copies: 2, spark: None, any_rare_pity: 0 };
        setup.weapon.target_copies = 0;
        setup.character_dedicated = 1;
        let mut cfg = FeedbackConfig::for_game(Game::Neverness);
        cfg.samples = 100;
        let result = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(result.quantile(1.0), 1);
    }
    #[test]
    fn four_star_guarantee_applies_at_zero_base_rate() {
        let mut cfg = FeedbackConfig::for_game(Game::StarRail);
        cfg.character_four_star_rate = 0.0;
        cfg.character_four_star_pity = 9;
        let mut pity = cfg.character_four_star_pity;
        let mut rng = SplitMix64(17);
        assert!(maybe_four_star(&mut rng, &cfg, BannerKind::Character, &mut pity, false));
        assert_eq!(pity, 0);
        assert!(!maybe_four_star(&mut rng, &cfg, BannerKind::Character, &mut pity, false));
    }

    #[test]
    fn four_star_pity_outside_range_is_rejected() {
        let mut cfg = FeedbackConfig::for_game(Game::StarRail);
        cfg.weapon_four_star_pity = cfg.four_star_hard;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn off_banner_wuthering_rare_uses_conservative_default() {
        assert_eq!(FeedbackConfig::for_game(Game::WutheringWaves).off_banner_rare, 15);
    }

}