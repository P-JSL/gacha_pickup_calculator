use super::*;

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
    /// 0 = no eligible first 10-pull; 10 = explicitly buy the first 10-pull.
    /// Its bonus is 900 versus 300 Gold Discs, NOT 3× Expert Permits.
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
                featured_first: 0, featured_duplicate: 200, featured_maxed: 500,
                off_banner_rare: 200, weapon_first: 0, weapon_rare: 200,
                four_star_reward: 40, four_star_maxed: 70, four_star_maxed_share: 0.0,
                weapon_four_star_reward: 40,
                character_four_star_rate: 0.04, weapon_four_star_rate: 0.08,
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
            || !matches!(self.stella_first_ten_remaining, 0 | 10) {
            return Err("환급 설정의 가격·확률·표본 수가 허용 범위를 벗어났습니다.".to_owned());
        }
        Ok(())
    }
}

fn valid_probability(p: f64) -> bool {
    p.is_finite() && (0.0..=1.0).contains(&p)
}

