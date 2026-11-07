//! Reproducible Monte Carlo estimate of *recycled* summon tickets.
//!
//! Unlike the exact rarity/featured DP, four-star returns depend on the account's
//! inventory, and some exchange schedules are unknown. Every return is therefore
//! an explicit user-editable *assumption*. Never label these estimates exact.

use crate::{Banner, BannerRule, RateMode};
use crate::games::{Game, GameSetup};

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

/// Buy only when the target character already exists and the target is not met.
fn buy_available_wavebands(wallet: &mut Wallet, model: &FeedbackConfig,
    owned: usize, won: &mut usize, target: usize, limit: usize, bought: &mut usize) {
    while *bought < limit && *won < target && owned + *won > 0
        && wallet.buy_waveband(model.wuwa_waveband_cost)
    {
        *bought += 1;
        *won += 1;
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
        BannerKind::Character if owned + copies_this_run >= 7 => model.featured_maxed,
        BannerKind::Character => model.featured_duplicate,
    };
    wallet.refund(amount);
}

fn character_four_star_refund(rng: &mut SplitMix64, model: &FeedbackConfig) -> usize {
    if rng.hits(model.four_star_maxed_share) {
        model.four_star_maxed
    } else {
        model.four_star_reward
    }
}

fn roll_standard(rng: &mut SplitMix64, rule: &Banner, (kind, game): (BannerKind, Game),
                 mode: RateMode, model: &FeedbackConfig, wallet: &mut Wallet,
                 (owned, dedicated, waveband_limit): (usize, &mut usize, usize)) {
    let BannerRule::Standard { rarity, featured_share } = &rule.rule else { return; };
    let mut pity = rule.pity;
    let mut guaranteed = rule.guaranteed;
    let mut won = 0;
    let mut bought = 0;
    if waveband_limit > 0 {
        buy_available_wavebands(wallet, model, owned, &mut won, rule.target_copies,
            waveband_limit, &mut bought);
    }
    let mut four_pity = match kind {
        BannerKind::Character => model.character_four_star_pity,
        BannerKind::Weapon => model.weapon_four_star_pity,
    };
    while won < rule.target_copies {
        wallet.pay(dedicated);
        if matches!(kind, BannerKind::Character) {
            if game == Game::StellaSora {
                wallet.begin_character_pull(model);
            }
            // A community estimate only: actual NTE tile odds depend on board position.
            if game == Game::Neverness
                && model.nte_one_die_chance + model.nte_five_dice_chance > 0.0 {
                let tile = rng.uniform();
                let returned: usize = if tile < model.nte_one_die_chance {
                    1
                } else if tile < model.nte_one_die_chance + model.nte_five_dice_chance {
                    5
                } else {
                    0
                };
                *dedicated = dedicated.saturating_add(returned);
                wallet.board_dice_earned += returned;
            }
        }
        let rare = rng.hits(rarity.chance(pity + 1, mode));
        if maybe_four_star(rng, model, kind, &mut four_pity, rare) {
            wallet.refund(match kind {
                BannerKind::Character => character_four_star_refund(rng, model),
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
        if waveband_limit > 0 {
            buy_available_wavebands(wallet, model, owned, &mut won, rule.target_copies,
                waveband_limit, &mut bought);
        }
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
        if matches!(kind, BannerKind::Character) {
            wallet.begin_character_pull(model);
        }
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
                BannerKind::Character => character_four_star_refund(rng, model),
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
        let mut off = 0_usize;
        let mut pending_four_star_points = 0_usize;
        for _ in 0..10 {
            let x = rng.uniform();
            if x < 0.0075 { featured += 1; four_pity = 0; }
            else if x < 0.03 { off += 1; four_pity = 0; }
            else if maybe_four_star(rng, model, BannerKind::Weapon, &mut four_pity, false) {
                pending_four_star_points = pending_four_star_points
                    .saturating_add(model.weapon_four_star_reward);
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
        wallet.refund(off.saturating_mul(model.off_banner_rare));
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
    // This estimator is public and can be called without the UI's exact
    // DP. Validate the same input bounds without redundantly constructing two
    // full distributions (or doing so three times for WuWa strategies).
    setup.character.validate_inputs()?;
    setup.weapon.validate_inputs()?;
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
    let available = std::thread::available_parallelism().map_or(1, |count| count.get());
    let workers = available.min(MAX_MONTE_CARLO_WORKERS)
        .min((model.samples / MIN_SAMPLES_PER_WORKER).max(1));
    estimate_with_workers(setup, mode, model, workers)
}

/// The seed depends on the trial index, not on how workers are scheduled.
/// A one-worker run and an N-worker run therefore generate the same paths.
fn sample_seed(index: usize) -> u64 {
    let mut seed = SplitMix64(0xF17E_1234_ABCD_0905
        ^ (index as u64).wrapping_mul(0xD134_2543_DE82_EF95));
    seed.next()
}

#[derive(Default)]
struct TrialTotals {
    raw: u128,
    flexible: u128,
    returned: u128,
    wavebands: u128,
    waveband_cost: u128,
    character_raw: u128,
    weapon_raw: u128,
    character_points: u128,
    weapon_points: u128,
    board_dice: u128,
    record_discs: u128,
}
impl TrialTotals {
    fn merge(&mut self, other: Self) {
        self.raw += other.raw;
        self.flexible += other.flexible;
        self.returned += other.returned;
        self.wavebands += other.wavebands;
        self.waveband_cost += other.waveband_cost;
        self.character_raw += other.character_raw;
        self.weapon_raw += other.weapon_raw;
        self.character_points += other.character_points;
        self.weapon_points += other.weapon_points;
        self.board_dice += other.board_dice;
        self.record_discs += other.record_discs;
    }
}

fn simulate_chunk(slots: &mut [RefundScenario], start_index: usize, setup: &GameSetup,
                  mode: RateMode, model: &FeedbackConfig) -> TrialTotals {
    let mut totals = TrialTotals::default();
    for (offset, slot) in slots.iter_mut().enumerate() {
        let mut rng = SplitMix64(sample_seed(start_index + offset));
        let mut wallet = Wallet::new(model);
        let mut dedicated_character = setup.character_dedicated;
        let mut dedicated_weapon = setup.weapon_dedicated;
        for (kind, banner, owned, dedicated) in [
            (BannerKind::Character, &setup.character, setup.character_owned, &mut dedicated_character),
            (BannerKind::Weapon, &setup.weapon, setup.weapon_owned, &mut dedicated_weapon),
        ] {
            // Record-only credits can pay only for the weapon/record phase.
            if matches!(kind, BannerKind::Weapon) && setup.game == Game::StellaSora {
                let disc_units = wallet.record_disc_units
                    .saturating_add(setup.weapon_ticket_remainder);
                *dedicated = dedicated.saturating_add(
                    disc_units / setup.game.weapon_ticket_units_per_pull());
            }
            let raw_before = wallet.raw_pulls;
            let points_before = wallet.points;
            match &banner.rule {
                BannerRule::Standard { .. } => {
                    let max_wavebands = if setup.game == Game::WutheringWaves
                        && matches!(kind, BannerKind::Character) {
                        model.wuwa_waveband_max
                    } else { 0 };
                    roll_standard(&mut rng, banner, (kind, setup.game), mode, model, &mut wallet,
                        (owned, dedicated, max_wavebands));
                }
                BannerRule::FeaturedCounter { .. } => {
                    roll_featured(&mut rng, banner, kind, model, &mut wallet, owned, dedicated);
                }
                BannerRule::NteArcBatch => {
                    roll_arc(&mut rng, banner, model, &mut wallet, owned, dedicated);
                }
            }
            let draws = (wallet.raw_pulls - raw_before) as u128;
            let points = (wallet.points - points_before) as u128;
            match kind {
                BannerKind::Character => {
                    totals.character_raw += draws;
                    totals.character_points += points;
                    wallet.reward_multiplier = 1;
                }
                BannerKind::Weapon => {
                    totals.weapon_raw += draws;
                    totals.weapon_points += points;
                }
            }
        }
        totals.board_dice += wallet.board_dice_earned as u128;
        totals.record_discs += wallet.record_disc_units as u128;
        totals.raw += wallet.raw_pulls as u128;
        totals.flexible += wallet.initial_flexible_required as u128;
        totals.returned += (wallet.points - model.initial_points) as u128;
        totals.wavebands += wallet.wavebands_bought as u128;
        totals.waveband_cost += wallet.waveband_points_spent as u128;
        *slot = RefundScenario::from_wallet(&wallet, model.initial_points);
    }
    totals
}

/// A single preallocated scenario array is partitioned into non-overlapping
/// mutable slices. Workers only return integer aggregates: no result cloning,
/// locks, per-worker result arrays or scheduling-dependent floating sums.
fn estimate_with_workers(setup: &GameSetup, mode: RateMode, model: &FeedbackConfig,
                         requested_workers: usize) -> Result<FeedbackEstimate, String> {
    let mut scenarios = Vec::new();
    scenarios.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    scenarios.resize_with(model.samples, RefundScenario::default);
    let workers = requested_workers.max(1).min(model.samples);
    let chunk_size = model.samples.div_ceil(workers);
    let partials = if workers == 1 {
        vec![simulate_chunk(&mut scenarios, 0, setup, mode, model)]
    } else {
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(workers);
            for (chunk_index, slots) in scenarios.chunks_mut(chunk_size).enumerate() {
                handles.push(scope.spawn(move || {
                    simulate_chunk(slots, chunk_index * chunk_size, setup, mode, model)
                }));
            }
            handles.into_iter().map(|handle| {
                handle.join().map_err(|_| "병렬 시뮬레이션 작업이 중단되었습니다.".to_owned())
            }).collect::<Result<Vec<TrialTotals>, String>>()
        })?
    };
    let mut totals = TrialTotals::default();
    for part in partials { totals.merge(part); }

    // The public v1.x contract exposes all raw trial observations. Construct
    // only those required arrays; avoid duplicate intermediate path storage.
    let mut budgets = Vec::new();
    let mut raw_pulls = Vec::new();
    budgets.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    raw_pulls.try_reserve_exact(model.samples).map_err(|_| {
        "시뮬레이션 결과 배열을 위한 메모리를 확보할 수 없습니다.".to_owned()
    })?;
    // Retain the original per-trial raw-pull order before sorting scenarios.
    for scenario in &scenarios {
        raw_pulls.push(scenario.raw_pulls);
    }
    scenarios.sort_unstable_by_key(|scenario|
        (scenario.initial_flexible_required, scenario.raw_pulls));
    // Sorting scenarios by budget already orders the budget CDF. Avoid a
    // redundant second O(N log N) sort for large trial counts.
    for scenario in &scenarios {
        budgets.push(scenario.initial_flexible_required);
    }

    // Integer aggregation makes the public means bit-identical for every
    // worker count; floating-point division happens only once per metric.
    let n = model.samples as f64;
    Ok(FeedbackEstimate { budgets, raw_pulls, scenarios,
        average_raw: totals.raw as f64 / n,
        average_initial_flexible: totals.flexible as f64 / n,
        total_returned_points_average: totals.returned as f64 / n,
        average_wavebands_purchased: totals.wavebands as f64 / n,
        average_waveband_corals_spent: totals.waveband_cost as f64 / n,
        average_character_raw: totals.character_raw as f64 / n,
        average_weapon_raw: totals.weapon_raw as f64 / n,
        average_character_return_points: totals.character_points as f64 / n,
        average_weapon_return_points: totals.weapon_points as f64 / n,
        average_board_dice_earned: totals.board_dice as f64 / n,
        average_record_disc_units_earned: totals.record_discs as f64 / n })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::{Game, GameSetup};
    use crate::{Banner, BannerRule, RateSchedule, SoftPity};

    #[test]
    fn extreme_refund_values_do_not_overflow_or_create_negative_balances() {
        let cfg = FeedbackConfig::for_game(Game::StellaSora);
        let mut wallet = Wallet::new(&cfg);
        wallet.reward_multiplier = 3;
        wallet.refund(usize::MAX);
        assert_eq!(wallet.points, usize::MAX);
        wallet.refund(5);
        assert_eq!(wallet.points, usize::MAX);
        wallet.record_disc_units = usize::MAX - 10;
        wallet.begin_character_pull(&cfg);
        assert_eq!(wallet.record_disc_units, usize::MAX);
    }

    #[test]
    fn zero_confidence_does_not_require_positive_funding() {
        let estimate = FeedbackEstimate {
            budgets: vec![2, 3, 5],
            raw_pulls: vec![],
            scenarios: vec![],
            average_raw: 0.0, average_initial_flexible: 0.0,
            total_returned_points_average: 0.0,
            average_wavebands_purchased: 0.0,
            average_waveband_corals_spent: 0.0,
            average_character_raw: 0.0, average_weapon_raw: 0.0,
            average_character_return_points: 0.0,
            average_weapon_return_points: 0.0,
            average_board_dice_earned: 0.0,
            average_record_disc_units_earned: 0.0,
        };
        assert_eq!(estimate.quantile(0.0), 0);
        assert_eq!(estimate.quantile(-0.5), 0);
        assert_eq!(estimate.quantile(0.5), 3);
        assert_eq!(estimate.quantile(1.0), 5);
    }

    #[test]
    fn direct_simulation_rejects_impossible_banner_instead_of_looping() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.set_targets(1, 0);
        setup.character.rule = BannerRule::Standard {
            rarity: RateSchedule {
                base: 0.0, hard: 90, soft: SoftPity::None,
            },
            featured_share: 0.0,
        };
        let mut cfg = FeedbackConfig::for_game(Game::StarRail);
        cfg.samples = 1;
        assert!(estimate(&setup, RateMode::PublishedOnly, &cfg).is_err());
    }

    #[test]
    fn wallet_requires_ticket_before_earning_refund() {
        let conf = FeedbackConfig { enabled: true, points_per_ticket: 1, initial_points: 0,
            featured_first: 1, featured_duplicate: 1, featured_maxed: 1, off_banner_rare: 0,
            weapon_first: 1, weapon_rare: 1, four_star_reward: 0,
            four_star_maxed: 0, four_star_maxed_share: 0.0,
            weapon_four_star_reward: 0, character_four_star_rate: 0.0,
            weapon_four_star_rate: 0.0, character_other_five_star_rate: 0.0,
            weapon_other_five_star_rate: 0.0, four_star_hard: 10, character_four_star_pity: 0, weapon_four_star_pity: 0, samples: 10,
            wuwa_waveband_max: 0, wuwa_waveband_cost: 360,
            nte_one_die_chance: 0.0, nte_five_dice_chance: 0.0,
            stella_record_discs_per_character_pull: 0, stella_first_ten_remaining: 0 };
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
    fn waveband_store_prioritizes_purchases_without_double_spending() {
        let mut setup = GameSetup::new(Game::WutheringWaves);
        setup.character = Banner { rule: BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
            featured_share: 1.0,
        }, pity: 0, guaranteed: false, target_copies: 4,
           spark: None, any_rare_pity: 0 };
        setup.weapon.target_copies = 0;
        let mut cfg = FeedbackConfig::for_game(Game::WutheringWaves);
        cfg.samples = 30;
        cfg.featured_first = 2;
        cfg.featured_duplicate = 2;
        cfg.wuwa_waveband_cost = 2;
        cfg.points_per_ticket = 2;
        cfg.four_star_reward = 0;
        cfg.character_four_star_rate = 0.0;
        let tickets = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(tickets.quantile(1.0), 1);
        cfg.wuwa_waveband_max = 2;
        let wavebands = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(wavebands.average_wavebands_purchased, 2.0);
        assert_eq!(wavebands.average_waveband_corals_spent, 4.0);
        assert_eq!(wavebands.scenarios[0].raw_pulls, 2);
        assert_eq!(wavebands.scenarios[0].initial_flexible_required, 2);
    }
    #[test]
    fn fully_upgraded_character_rewards_use_correct_copy_boundary() {
        for (game, regular, maxed) in [(Game::StarRail, 40, 100), (Game::Neverness, 40, 80)] {
            let config = FeedbackConfig::for_game(game);
            let mut wallet = Wallet::new(&config);
            award_featured(&mut wallet, &config, 0, 0, BannerKind::Character);
            assert_eq!(wallet.points, 0);
            award_featured(&mut wallet, &config, 0, 1, BannerKind::Character);
            assert_eq!(wallet.points, regular);
            award_featured(&mut wallet, &config, 0, 6, BannerKind::Character);
            assert_eq!(wallet.points, regular * 2);
            award_featured(&mut wallet, &config, 0, 7, BannerKind::Character);
            assert_eq!(wallet.points, regular * 2 + maxed);
            award_featured(&mut wallet, &config, 7, 0, BannerKind::Character);
            assert_eq!(wallet.points, regular * 2 + maxed * 2);
        }
    }

    #[test]
    fn maxed_four_star_share_is_optional_and_preserves_fixed_weapon_rewards() {
        for (game, normal, maxed, weapon) in [(Game::StarRail, 8, 20, 8), (Game::Neverness, 6, 12, 4)] {
            let mut config = FeedbackConfig::for_game(game);
            let mut rng = SplitMix64(42);
            assert_eq!(character_four_star_refund(&mut rng, &config), normal);
            config.four_star_maxed_share = 1.0;
            assert_eq!(character_four_star_refund(&mut rng, &config), maxed);
            assert_eq!(config.weapon_four_star_reward, weapon);
            config.four_star_maxed_share = 1.01;
            assert!(config.validate().is_err());
        }
    }

    #[test]
    fn compounding_after_full_upgrade_uses_maxed_refund_tickets() {
        let config = FeedbackConfig::for_game(Game::Neverness);
        let mut wallet = Wallet::new(&config);
        let mut dedicated = 0;
        wallet.pay(&mut dedicated);
        award_featured(&mut wallet, &config, 7, 0, BannerKind::Character);
        assert_eq!(wallet.initial_flexible_required, 1);
        for _ in 0..3 { wallet.pay(&mut dedicated); }
        assert_eq!(wallet.initial_flexible_required, 1);
        assert_eq!(wallet.raw_pulls, 4);
        assert_eq!(RefundScenario::from_wallet(&wallet, 0).points_funded_pulls, 3);
    }

    #[test]
    fn nte_board_dice_are_reusable_after_the_roll_not_before() {
        let mut setup = GameSetup::new(Game::Neverness);
        setup.character = Banner {
            rule: BannerRule::Standard {
                rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
                featured_share: 1.0,
            },
            pity: 0, guaranteed: false, target_copies: 3,
            spark: None, any_rare_pity: 0,
        };
        setup.weapon.target_copies = 0;
        let mut cfg = FeedbackConfig::for_game(Game::Neverness);
        cfg.samples = 10;
        cfg.nte_one_die_chance = 1.0;
        cfg.nte_five_dice_chance = 0.0;
        cfg.featured_first = 0;
        cfg.featured_duplicate = 0;
        cfg.featured_maxed = 0;
        let result = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        let scenario = &result.scenarios[0];
        assert_eq!(scenario.raw_pulls, 3);
        assert_eq!(scenario.initial_flexible_required, 1);
        assert_eq!(scenario.board_dice_earned, 3);
        assert_eq!(result.average_board_dice_earned, 3.0);
        cfg.nte_five_dice_chance = 0.1;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn stella_record_only_credit_cannot_pay_for_character_pulls() {
        let mut setup = GameSetup::new(Game::StellaSora);
        setup.character = Banner {
            rule: BannerRule::Standard {
                rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
                featured_share: 1.0,
            },
            pity: 0, guaranteed: false, target_copies: 10,
            spark: None, any_rare_pity: 0,
        };
        setup.weapon = Banner {
            rule: BannerRule::Standard {
                rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
                featured_share: 1.0,
            },
            pity: 0, guaranteed: false, target_copies: 1,
            spark: None, any_rare_pity: 0,
        };
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.samples = 10;
        cfg.featured_duplicate = 0;
        cfg.featured_maxed = 0;
        cfg.weapon_rare = 0;
        cfg.weapon_first = 0;
        cfg.off_banner_rare = 0;
        cfg.character_four_star_rate = 0.0;
        cfg.weapon_four_star_rate = 0.0;
        cfg.four_star_reward = 0;
        cfg.weapon_four_star_reward = 0;
        let actual = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(actual.average_record_disc_units_earned, 300.0);
        assert_eq!(actual.scenarios[0].initial_flexible_required, 10);
        assert_eq!(actual.scenarios[0].dedicated_used, 1);
        cfg.stella_record_discs_per_character_pull = 0;
        let disabled = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(disabled.scenarios[0].initial_flexible_required, 11);
    }

    #[test]
    fn first_ten_multiplier_requires_explicit_account_remaining_count() {
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.stella_first_ten_remaining = 2;
        let mut wallet = Wallet::new(&cfg);
        wallet.begin_character_pull(&cfg);
        wallet.refund(40);
        wallet.begin_character_pull(&cfg);
        wallet.refund(40);
        wallet.begin_character_pull(&cfg);
        wallet.refund(40);
        assert_eq!(wallet.points, 280); // 3*40 + 3*40 + 40
        assert_eq!(wallet.record_disc_units, 90);
        cfg.stella_first_ten_remaining = 11;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn off_banner_wuthering_rare_includes_30_extra_corals() {
        assert_eq!(FeedbackConfig::for_game(Game::WutheringWaves).off_banner_rare, 45);
    }


    #[test]
    fn parallel_and_sequential_paths_match_exactly_across_games_and_rate_models() {
        for game in Game::ALL {
            let mut setup = GameSetup::new(game);
            setup.set_targets(1, 1);
            let mut model = FeedbackConfig::for_game(game);
            model.samples = 513; // Uneven partitions must cover every sample once.
            for mode in [RateMode::PublishedOnly, RateMode::CommunityEstimate] {
                let reference = estimate_with_workers(&setup, mode, &model, 1).unwrap();
                for workers in [2, 4, 7] {
                    let parallel = estimate_with_workers(&setup, mode, &model, workers).unwrap();
                    assert_eq!(reference.budgets, parallel.budgets, "{game:?}/{mode:?}/{workers}");
                    assert_eq!(reference.raw_pulls, parallel.raw_pulls, "{game:?}/{mode:?}/{workers}");
                    assert_eq!(reference.scenarios, parallel.scenarios, "{game:?}/{mode:?}/{workers}");
                    for (a, b) in [
                        (reference.average_raw, parallel.average_raw),
                        (reference.average_initial_flexible, parallel.average_initial_flexible),
                        (reference.total_returned_points_average, parallel.total_returned_points_average),
                        (reference.average_wavebands_purchased, parallel.average_wavebands_purchased),
                        (reference.average_waveband_corals_spent, parallel.average_waveband_corals_spent),
                        (reference.average_character_raw, parallel.average_character_raw),
                        (reference.average_weapon_raw, parallel.average_weapon_raw),
                        (reference.average_character_return_points, parallel.average_character_return_points),
                        (reference.average_weapon_return_points, parallel.average_weapon_return_points),
                        (reference.average_board_dice_earned, parallel.average_board_dice_earned),
                        (reference.average_record_disc_units_earned, parallel.average_record_disc_units_earned),
                    ] {
                        assert_eq!(a.to_bits(), b.to_bits(), "{game:?}/{mode:?}/{workers}");
                    }
                    assert_eq!(parallel.scenarios.len(), model.samples);
                    assert!(parallel.budgets.windows(2).all(|v| v[0] <= v[1]));
                    assert!(parallel.scenarios.iter().all(|s| s.raw_pulls
                        == s.dedicated_used + s.points_funded_pulls
                            + s.initial_flexible_required));
                }
            }
        }
    }

    #[test]
    fn million_sample_limit_is_validated_without_allocating_a_million_paths() {
        let mut model = FeedbackConfig::for_game(Game::StarRail);
        model.samples = MAX_SAMPLES;
        assert!(model.validate().is_ok());
        model.samples += 1;
        assert!(model.validate().is_err());
    }


    #[test]
    fn sampled_simple_banner_agrees_with_independent_exact_dp() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.character = Banner {
            rule: BannerRule::Standard {
                rarity: RateSchedule { base: 0.5, hard: 2, soft: SoftPity::None },
                featured_share: 1.0,
            },
            pity: 0, guaranteed: false, target_copies: 1,
            spark: None, any_rare_pity: 0,
        };
        setup.weapon.target_copies = 0;
        let mut model = FeedbackConfig::for_game(Game::StarRail);
        model.samples = 25_000;
        model.four_star_reward = 0;
        model.four_star_maxed = 0;
        model.weapon_four_star_reward = 0;
        model.off_banner_rare = 0;
        let exact = setup.character.distribution(RateMode::PublishedOnly).unwrap();
        let sampled = estimate(&setup, RateMode::PublishedOnly, &model).unwrap();
        assert!((sampled.probability(1).0 - exact.at(1)).abs() < 0.025);
        assert!((sampled.average_raw - exact.mean).abs() < 0.025);
        assert_eq!(sampled.budgets.len(), model.samples);
        assert_eq!(sampled.scenarios.len(), model.samples);
    }

}