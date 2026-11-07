use super::*;

/// Buy only when the target character already exists and the target is not met.
pub(super) fn buy_available_wavebands(wallet: &mut Wallet, model: &FeedbackConfig,
    owned: usize, won: &mut usize, target: usize, limit: usize, bought: &mut usize) {
    while *bought < limit && *won < target && owned + *won > 0
        && wallet.buy_waveband(model.wuwa_waveband_cost)
    {
        *bought += 1;
        *won += 1;
    }
}

#[derive(Clone, Copy)]
pub(super) enum BannerKind { Character, Weapon }

pub(super) fn maybe_four_star(rng: &mut SplitMix64, model: &FeedbackConfig, kind: BannerKind,
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

pub(super) fn award_featured(wallet: &mut Wallet, model: &FeedbackConfig, owned: usize,
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

pub(super) fn character_four_star_refund(rng: &mut SplitMix64, model: &FeedbackConfig) -> usize {
    if rng.hits(model.four_star_maxed_share) {
        model.four_star_maxed
    } else {
        model.four_star_reward
    }
}

pub(super) fn roll_standard(rng: &mut SplitMix64, rule: &Banner, (kind, game): (BannerKind, Game),
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

pub(super) fn roll_featured(rng: &mut SplitMix64, rule: &Banner, kind: BannerKind,
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

pub(super) fn roll_arc(rng: &mut SplitMix64, rule: &Banner, model: &FeedbackConfig,
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

