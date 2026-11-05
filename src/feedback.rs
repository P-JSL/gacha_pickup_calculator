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
    /// Wuthering Waves: reserve coral for up to two character Wavebands.
    /// Zero means spend coral on tickets instead.
    pub waveband_limit: usize,
    pub waveband_price: usize,
    pub featured_first: usize,
    pub featured_duplicate: usize,
    pub off_banner_rare: usize,
    /// First copy of the featured 5★ weapon or record.
    pub weapon_first: usize,
    /// Subsequent copies or unconditional highest-rarity weapon payout.
    pub weapon_rare: usize,
    /// Average 4★ payout on the character banner; duplicates are assumed.
    pub four_star_reward: usize,
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
}

impl FeedbackConfig {
    pub fn for_game(game: Game) -> Self {
        match game {
            Game::StarRail => Self {
                enabled: true, points_per_ticket: 20, initial_points: 0,
                waveband_limit: 0, waveband_price: 360,
                featured_first: 0, featured_duplicate: 40,
                off_banner_rare: 40, weapon_first: 40, weapon_rare: 40,
                four_star_reward: 8, weapon_four_star_reward: 8, character_four_star_rate: 0.051,
                weapon_four_star_rate: 0.066,
                character_other_five_star_rate: 0.0, weapon_other_five_star_rate: 0.0,
                four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000,
            },
            Game::WutheringWaves => Self {
                enabled: true, points_per_ticket: 8, initial_points: 0,
                waveband_limit: 0, waveband_price: 360,
                featured_first: 15, featured_duplicate: 15,
                off_banner_rare: 15, weapon_first: 15, weapon_rare: 15,
                four_star_reward: 3, weapon_four_star_reward: 3, character_four_star_rate: 0.06,
                weapon_four_star_rate: 0.06,
                character_other_five_star_rate: 0.0, weapon_other_five_star_rate: 0.0,
                four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0,
                samples: 20_000,
            },
            Game::Neverness => Self {
                // Warp Pieces can buy dice / Tri-Keys. The 24-piece price and
                // missing board-tile rewards remain community assumptions.
                enabled: true, points_per_ticket: 24, initial_points: 0,
                waveband_limit: 0, waveband_price: 360,
                featured_first: 0, featured_duplicate: 40,
                off_banner_rare: 0, weapon_first: 40, weapon_rare: 40,
                four_star_reward: 6, weapon_four_star_reward: 4,
                character_four_star_rate: 0.06, weapon_four_star_rate: 0.07,
                character_other_five_star_rate: 0.0, weapon_other_five_star_rate: 0.0,
                four_star_hard: 10, character_four_star_pity: 0,
                weapon_four_star_pity: 0, samples: 20_000,
            },
            Game::StellaSora => Self {
                // Expert Permits: duplicate 5★ Trekker / Disc = 200,
                // duplicate 4★ Trekker / Disc = 40; 100 permits per pull.
                // First-time featured 5★ collections award no duplicate permits.
                // The 4★ return assumes all 4★ results are already owned.
                enabled: true, points_per_ticket: 100, initial_points: 0,
                waveband_limit: 0, waveband_price: 360,
                featured_first: 0, featured_duplicate: 200,
                off_banner_rare: 200, weapon_first: 0, weapon_rare: 200,
                four_star_reward: 40, weapon_four_star_reward: 40,
                character_four_star_rate: 0.08, weapon_four_star_rate: 0.08,
                character_other_five_star_rate: 0.01, weapon_other_five_star_rate: 0.005,
                four_star_hard: 10, character_four_star_pity: 0,
                weapon_four_star_pity: 0, samples: 20_000,
            },
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.points_per_ticket == 0 || self.waveband_price == 0
            || self.waveband_limit > 2 || self.samples == 0 || self.samples > 200_000
            || self.four_star_hard == 0 || self.four_star_hard > 20
            || self.character_four_star_pity >= self.four_star_hard
            || self.weapon_four_star_pity >= self.four_star_hard
            || !valid_probability(self.character_four_star_rate)
            || !valid_probability(self.weapon_four_star_rate)
            || !valid_probability(self.character_other_five_star_rate)
            || !valid_probability(self.weapon_other_five_star_rate) {
            return Err("환급 설정의 가격·확률·표본 수가 허용 범위를 벗어났습니다.".to_owned());
        }
        Ok(())
    }
}

fn valid_probability(p: f64) -> bool {
    p.is_finite() && (0.0..=1.0).contains(&p)
}

/// A single Monte Carlo path; never subtract independently chosen quantiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefundScenario {
    pub raw_pulls: usize,
    pub dedicated_used: usize,
    /// Tickets covered by already-owned plus newly earned exchange points.
    pub points_funded_pulls: usize,
    /// Minimum flexible-ticket funding required at any time before completion.
    pub initial_flexible_required: usize,
    /// All earned points, including the final pull's unspendable reward.
    pub earned_points: usize,
    pub wavebands_bought: usize,
    pub waveband_points_spent: usize,
}
impl RefundScenario {
    fn from_wallet(wallet: &Wallet, starting_points: usize) -> Self {
        let dedicated_used = wallet.raw_pulls - wallet.common_spent;
        let points_funded_pulls = wallet.point_funded_pulls;
        let scenario = Self {
            raw_pulls: wallet.raw_pulls,
            dedicated_used,
            points_funded_pulls,
            initial_flexible_required: wallet.initial_flexible_required,
            earned_points: wallet.points - starting_points,
            wavebands_bought: wallet.wavebands_bought,
            waveband_points_spent: wallet.waveband_points_spent,
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
    /// Cumulative earned plus initially owned coral (for statistics only).
    points: usize,
    /// Actual remaining coral after ticket and Waveband purchases.
    balance: usize,
    price: usize,
    common_spent: usize,
    point_funded_pulls: usize,
    initial_flexible_required: usize,
    raw_pulls: usize,
    wavebands_bought: usize,
    waveband_points_spent: usize,
}
impl Wallet {
    fn new(model: &FeedbackConfig) -> Self {
        Self { points: model.initial_points, balance: model.initial_points,
            price: model.points_per_ticket, common_spent: 0,
            point_funded_pulls: 0, initial_flexible_required: 0,
            raw_pulls: 0, wavebands_bought: 0, waveband_points_spent: 0 }
    }
    /// No refund from this draw can pay for the same draw.
    /// When the Waveband strategy is active, coral is reserved first.
    fn pay_reserved(&mut self, dedicated_remaining: &mut usize, reserve: usize) {
        self.raw_pulls += 1;
        if *dedicated_remaining > 0 {
            *dedicated_remaining -= 1;
        } else {
            self.common_spent += 1;
            if self.balance >= reserve.saturating_add(self.price) {
                self.balance -= self.price;
                self.point_funded_pulls += 1;
            } else {
                self.initial_flexible_required += 1;
            }
        }
    }
    fn pay(&mut self, dedicated_remaining: &mut usize) {
        self.pay_reserved(dedicated_remaining, 0);
    }
    fn refund(&mut self, reward: usize) {
        self.points = self.points.saturating_add(reward);
        self.balance = self.balance.saturating_add(reward);
    }
    fn buy_wavebands(&mut self, model: &FeedbackConfig, character_owned: usize,
                     gacha_won: usize, remaining_target: usize) {
        if character_owned + gacha_won == 0 { return; }
        let max_purchase = model.waveband_limit.min(remaining_target).min(2);
        while self.wavebands_bought < max_purchase
            && self.balance >= model.waveband_price
            && gacha_won + self.wavebands_bought < remaining_target
        {
            self.balance -= model.waveband_price;
            self.wavebands_bought += 1;
            self.waveband_points_spent += model.waveband_price;
        }
    }
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
        BannerKind::Weapon if owned + copies_this_run == 0 => model.weapon_first,
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
        if matches!(kind, BannerKind::Character) {
            wallet.buy_wavebands(model, owned, won, rule.target_copies);
            if won + wallet.wavebands_bought >= rule.target_copies { break; }
        }
        let reserve = if matches!(kind, BannerKind::Character) {
            let eligible = owned.saturating_add(rule.target_copies)
                .saturating_sub(1).min(rule.target_copies);
            model.waveband_limit.min(eligible)
                .saturating_sub(wallet.wavebands_bought)
                .saturating_mul(model.waveband_price)
        } else { 0 };
        wallet.pay_reserved(dedicated, reserve);
        let rare = rng.hits(rarity.chance(pity + 1, mode));
        if maybe_four_star(rng, model, kind, &mut four_pity, rare) {
            wallet.refund(match kind {
                BannerKind::Character => model.four_star_reward,
                BannerKind::Weapon => model.weapon_four_star_reward,
            });
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
        // Sampling the nonfeatured rate conditional on a featured miss retains
        // the desired unconditional rarity rate. Nonfeatured Stella Sora
        // 5-star drops do not reset the featured-only pity counter.
        let off_rate = match kind {
            BannerKind::Character => model.character_other_five_star_rate,
            BannerKind::Weapon => model.weapon_other_five_star_rate,
        };
        let off_rare = !success && rng.hits((off_rate / (1.0 - featured_rate).max(f64::EPSILON)).min(1.0));
        if maybe_four_star(rng, model, kind, &mut four_pity, success || off_rare) {
            wallet.refund(match kind {
                BannerKind::Character => model.four_star_reward,
                BannerKind::Weapon => model.weapon_four_star_reward,
            });
        }
        if success {
            pity = 0;
            award_featured(wallet, model, owned, won, kind);
            won += 1;
        } else {
            pity += 1;
            if off_rare {
                // Assumes the nonfeatured 5-star is owned (the reward is editable).
                wallet.refund(model.off_banner_rare);
            }
        }
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
                pending_four_star_points += model.weapon_four_star_reward;
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
    for (banner, extra) in [
        (&setup.character, model.character_other_five_star_rate),
        (&setup.weapon, model.weapon_other_five_star_rate),
    ] {
        if let BannerRule::FeaturedCounter { featured_rate, .. } = &banner.rule {
            if *featured_rate + extra > 1.0 + f64::EPSILON {
                return Err("픽업·비픽업 최고 등급 기본 확률의 합은 100%를 넘을 수 없습니다.".into());
            }
        }
    }
    let mut rng = SplitMix64(0xF17E_1234_ABCD_0905);
    let mut budgets = Vec::with_capacity(model.samples);
    let mut raw_pulls = Vec::with_capacity(model.samples);
    let mut scenarios = Vec::with_capacity(model.samples);
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
        scenarios.push(RefundScenario::from_wallet(&wallet, model.initial_points));
        total_raw += wallet.raw_pulls as f64;
        total_flexible += wallet.initial_flexible_required as f64;
        total_returns += (wallet.points - model.initial_points) as f64;
    }
    scenarios.sort_unstable_by_key(|scenario|
        (scenario.initial_flexible_required, scenario.raw_pulls));
    budgets.sort_unstable();
    let n = model.samples as f64;
    Ok(FeedbackEstimate { budgets, raw_pulls, scenarios,
        average_raw: total_raw / n, average_initial_flexible: total_flexible / n,
        total_returned_points_average: total_returns / n })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::{Game, GameSetup};
    use crate::{Banner, BannerRule, RateSchedule, SoftPity};

    #[test]
    fn waveband_purchases_cannot_use_same_draw_returns() {
        let mut cfg = FeedbackConfig::for_game(Game::WutheringWaves);
        cfg.waveband_limit = 2;
        let mut wallet = Wallet::new(&cfg);
        wallet.refund(359);
        wallet.buy_wavebands(&cfg, 1, 0, 6);
        assert_eq!(wallet.wavebands_bought, 0);
        wallet.refund(1);
        wallet.buy_wavebands(&cfg, 1, 0, 6);
        assert_eq!(wallet.wavebands_bought, 1);
        assert_eq!(wallet.balance, 0);
        assert_eq!(wallet.waveband_points_spent, 360);
    }

    #[test]
    fn coral_ticket_vs_waveband_strategy_is_separate() {
        let mut setup = GameSetup::new(Game::WutheringWaves);
        setup.character.target_copies = 3;
        setup.weapon.target_copies = 0;
        setup.character_owned = 1;
        setup.character.rule = BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
            featured_share: 1.0,
        };
        let mut cfg = FeedbackConfig::for_game(Game::WutheringWaves);
        cfg.initial_points = 720;
        cfg.samples = 10;
        let tickets = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        cfg.waveband_limit = 2;
        let wavebands = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert!(wavebands.average_raw < tickets.average_raw);
        // Purchasing upgrades costs coral; fewer draws does not automatically
        // imply a lower external-currency budget when tickets were plentiful.
        assert!(wavebands.quantile(1.0) >= tickets.quantile(1.0));
        assert!(wavebands.scenarios.iter().all(|s| s.wavebands_bought == 2));
        assert!(tickets.scenarios.iter().all(|s| s.wavebands_bought == 0));
    }

    #[test]
    fn wallet_requires_ticket_before_earning_refund() {
        let conf = FeedbackConfig { enabled: true, points_per_ticket: 1, initial_points: 0,
            waveband_limit: 0, waveband_price: 360,
            featured_first: 1, featured_duplicate: 1, off_banner_rare: 0,
            weapon_first: 1, weapon_rare: 1, four_star_reward: 0,
            weapon_four_star_reward: 0, character_four_star_rate: 0.0,
            weapon_four_star_rate: 0.0, character_other_five_star_rate: 0.0,
            weapon_other_five_star_rate: 0.0, four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0, samples: 10 };
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
    fn four_star_refunds_compound_across_refunded_draws() {
        let cfg = FeedbackConfig::for_game(Game::StellaSora);
        let mut wallet = Wallet::new(&cfg);
        let mut dedicated = 0;
        for _ in 0..6 {
            wallet.pay(&mut dedicated);
            // A duplicate 4★ is obtained even on pulls funded by refunds.
            wallet.refund(cfg.four_star_reward);
        }
        let sample = RefundScenario::from_wallet(&wallet, 0);
        assert_eq!(sample.raw_pulls, 6);
        assert_eq!(sample.points_funded_pulls, 2);
        assert_eq!(sample.initial_flexible_required, 4);
        assert_eq!(sample.earned_points, 240);
        assert_eq!(sample.raw_pulls,
            sample.initial_flexible_required + sample.points_funded_pulls);
    }

    #[test]
    fn all_games_have_distinct_mutable_refund_presets() {
        let hsr = FeedbackConfig::for_game(Game::StarRail);
        let wuwa = FeedbackConfig::for_game(Game::WutheringWaves);
        let nte = FeedbackConfig::for_game(Game::Neverness);
        let stella = FeedbackConfig::for_game(Game::StellaSora);
        for cfg in [&hsr, &wuwa, &nte, &stella] {
            assert!(cfg.enabled);
            cfg.validate().unwrap();
        }
        assert_eq!(hsr.points_per_ticket, 20);
        assert_eq!(wuwa.points_per_ticket, 8);
        assert_eq!(nte.points_per_ticket, 24);
        assert_eq!(stella.points_per_ticket, 100);
    }

    #[test]
    fn quantile_uses_one_correlated_scenario() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.character = Banner { rule: BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
            featured_share: 1.0,
        }, pity: 0, guaranteed: false, target_copies: 3,
           spark: None, any_rare_pity: 0 };
        setup.weapon.target_copies = 0;
        setup.character_dedicated = 1;
        let mut cfg = FeedbackConfig::for_game(Game::StarRail);
        cfg.samples = 10;
        cfg.four_star_reward = 0;
        let result = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        let sample = result.scenario_at_quantile(0.9).unwrap();
        assert_eq!(sample.initial_flexible_required, result.quantile(0.9));
        assert_eq!(sample.raw_pulls,
            sample.initial_flexible_required + sample.points_funded_pulls
                + sample.dedicated_used);
        let means = result.successful_means(result.quantile(0.9)).unwrap();
        assert!(means.count > 0);
        assert!((means.average_raw -
                 (means.average_dedicated + means.average_points_funded + means.average_flexible))
            .abs() < 1e-10);
    }

    #[test]
    fn stella_sora_nonfeatured_five_star_refunds_without_resetting_featured_counter() {
        let mut setup = GameSetup::new(Game::StellaSora);
        setup.character = Banner {
            rule: BannerRule::FeaturedCounter { featured_rate: 0.0, hard_featured: 3 },
            pity: 0, guaranteed: false, target_copies: 1,
            spark: None, any_rare_pity: 0,
        };
        setup.weapon.target_copies = 0;
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.samples = 10;
        cfg.character_other_five_star_rate = 1.0;
        cfg.character_four_star_rate = 0.0;
        cfg.four_star_reward = 0;
        let result = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        let path = result.scenario_at_quantile(0.5).unwrap();
        assert_eq!(path.raw_pulls, 3);
        assert_eq!(path.initial_flexible_required, 1);
        assert_eq!(path.points_funded_pulls, 2);
        assert_eq!(path.earned_points, 400);
    }

    #[test]
    fn stella_sora_expert_permits_apply_to_trekker_and_record_duplicates() {
        let cfg = FeedbackConfig::for_game(Game::StellaSora);
        assert!(cfg.enabled);
        assert_eq!(cfg.points_per_ticket, 100);
        assert_eq!(cfg.featured_first, 0);
        assert_eq!(cfg.weapon_first, 0);
        assert_eq!(cfg.featured_duplicate, 200);
        assert_eq!(cfg.weapon_rare, 200);
        assert_eq!(cfg.four_star_reward, 40);
        assert_eq!(cfg.weapon_four_star_reward, 40);
    }

    #[test]
    fn refunded_pulls_can_generate_further_refunds_on_later_rolls() {
        // Four guaranteed 5-star characters; first copy grants no points.
        // First two need initial flexible tickets. Copies 2 and 3 each
        // produce a full refund ticket to fund the next draw; last refund
        // cannot retroactively reduce initial funding.
        let mut setup = GameSetup::new(Game::StellaSora);
        setup.character = Banner { rule: BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
            featured_share: 1.0,
        }, pity: 0, guaranteed: false, target_copies: 4,
           spark: None, any_rare_pity: 0 };
        setup.weapon.target_copies = 0;
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.points_per_ticket = 1;
        cfg.featured_duplicate = 1;
        cfg.four_star_reward = 0;
        cfg.samples = 20;
        let stats = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        let sample = stats.scenario_at_quantile(0.90).unwrap();
        assert_eq!(sample.raw_pulls, 4);
        assert_eq!(sample.dedicated_used, 0);
        assert_eq!(sample.initial_flexible_required, 2);
        assert_eq!(sample.points_funded_pulls, 2);
        assert_eq!(sample.earned_points, 3);
        assert_eq!(sample.raw_pulls,
            sample.dedicated_used + sample.points_funded_pulls
                + sample.initial_flexible_required);
        assert_eq!(stats.quantile(0.90), 2);
        let mean = stats.successful_means(2).unwrap();
        assert_eq!(mean.count, 20);
        assert_eq!(mean.average_points_funded, 2.0);
    }

    #[test]
    fn first_copy_of_stella_sora_record_earns_no_duplicate_permits() {
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.samples = 10;
        let mut w = Wallet::new(&cfg);
        award_featured(&mut w, &cfg, 0, 0, BannerKind::Weapon);
        assert_eq!(w.points, 0);
        award_featured(&mut w, &cfg, 0, 1, BannerKind::Weapon);
        assert_eq!(w.points, 200);
    }

    #[test]
    fn off_banner_wuthering_rare_uses_conservative_default() {
        assert_eq!(FeedbackConfig::for_game(Game::WutheringWaves).off_banner_rare, 15);
    }

}