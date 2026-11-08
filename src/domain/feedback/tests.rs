
    use super::*;
    use crate::games::{Game, GameSetup};
    use crate::{Banner, BannerRule, RateSchedule, SoftPity};

    #[test]
    fn extreme_refund_values_do_not_overflow_or_create_negative_balances() {
        let cfg = FeedbackConfig::for_game(Game::StellaSora);
        let mut wallet = Wallet::new(&cfg);
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
        assert!(maybe_four_star(&mut rng, &cfg, BannerKind::Character, &mut pity, false, 0.0, true));
        assert_eq!(pity, 0);
        assert!(!maybe_four_star(&mut rng, &cfg, BannerKind::Character, &mut pity, false, 0.0, true));
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
        for cfg in [&hsr, &wuwa, &nte, &stella] {
            assert_eq!(cfg.four_star_maxed_share, 1.0);
        }
        assert_eq!(stella.four_star_maxed, 70);
        assert_eq!(stella.weapon_four_star_reward, 80);
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
        cfg.four_star_maxed = 0;
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
        cfg.four_star_maxed = 0;
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
        cfg.four_star_maxed = 0;
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
        cfg.four_star_maxed = 0;
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
            assert_eq!(character_four_star_refund(&mut rng, &config), maxed);
            config.four_star_maxed_share = 0.0;
            assert_eq!(character_four_star_refund(&mut rng, &config), normal);
            config.four_star_maxed_share = 1.0;
            assert_eq!(character_four_star_refund(&mut rng, &config), maxed);
            assert_eq!(config.weapon_four_star_reward, weapon);
            config.four_star_maxed_share = 1.01;
            assert!(config.validate().is_err());
        }
    }

    #[test]
    fn featured_four_star_refunds_distinguish_characters_and_equipment() {
        // Conditional on obtaining a 4-star, featured share = 1/(2 - p).
        // Off-featured character/equipment results are approximated as 50:50.
        for (game, kind, expected) in [
            (Game::StarRail, BannerKind::Character, 18.0),
            (Game::StarRail, BannerKind::Weapon, 9.2),
            (Game::WutheringWaves, BannerKind::Character, 43.0 / 6.0),
            (Game::WutheringWaves, BannerKind::Weapon, 23.0 / 6.0),
        ] {
            let cfg = FeedbackConfig::for_game(game);
            let mut rng = SplitMix64(154);
            let mut guaranteed = false;
            let samples = 120_000;
            let points: usize = (0..samples)
                .map(|_| event_four_star_refund(
                    &mut rng, &cfg, game, kind, &mut guaranteed))
                .sum();
            let average = points as f64 / samples as f64;
            assert!((average - expected).abs() < 0.05,
                "{game:?}: expected {expected}, got {average}");
        }
    }

    #[test]
    fn featured_four_star_guarantee_is_independent_of_five_star_pity() {
        let cfg = FeedbackConfig::for_game(Game::StarRail);
        let mut rng = SplitMix64(1);
        let mut guaranteed = true;
        assert_eq!(event_four_star_refund(&mut rng, &cfg, Game::StarRail,
            BannerKind::Character, &mut guaranteed), 20);
        assert!(!guaranteed);
        guaranteed = true;
        assert_eq!(event_four_star_refund(&mut rng, &cfg, Game::StarRail,
            BannerKind::Weapon, &mut guaranteed), 8);
        assert!(!guaranteed);
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
        cfg.four_star_maxed = 0;
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
    fn verified_maxed_coral_and_off_banner_arc_return_amounts() {
        let wuwa = FeedbackConfig::for_game(Game::WutheringWaves);
        assert_eq!(wuwa.featured_first, 15);
        assert_eq!(wuwa.featured_duplicate, 15);
        assert_eq!(wuwa.featured_maxed, 40);
        assert_eq!(wuwa.four_star_reward, 3);
        assert_eq!(wuwa.four_star_maxed, 8);
        let mut wallet = Wallet::new(&wuwa);
        // The eighth character copy must give a 40-coral refund.
        award_featured(&mut wallet, &wuwa, 7, 0, BannerKind::Character);
        assert_eq!(wallet.points, 40);
        let nte = FeedbackConfig::for_game(Game::Neverness);
        assert_eq!(nte.off_banner_rare, 40);
        assert_eq!(nte.weapon_rare, 40);
    }

    #[test]
    fn nte_tenth_dice_awards_additional_a_rank_after_the_paid_roll() {
        let mut setup = GameSetup::new(Game::Neverness);
        setup.character.rule = BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 90, soft: SoftPity::None },
            featured_share: 1.0
        };
        setup.character.target_copies = 1;
        setup.weapon.target_copies = 0;
        let mut config = FeedbackConfig::for_game(Game::Neverness);
        config.samples = 20;
        config.four_star_reward = 7;
        config.four_star_maxed_share = 0.0;
        config.weapon_four_star_reward = 7;
        config.character_four_star_rate = 0.0;
        config.featured_first = 0;
        config.nte_one_die_chance = 0.0;
        config.nte_five_dice_chance = 0.0;
        config.character_four_star_pity = 9; // 9 board rolls toward an extra gift
        let first = estimate(&setup, RateMode::PublishedOnly, &config).unwrap();
        for trial in &first.scenarios {
            assert_eq!(trial.raw_pulls, 1);
            assert_eq!(trial.initial_flexible_required, 1);
            assert_eq!(trial.earned_points, 7);
        }
        config.character_four_star_pity = 0;
        let no_gift = estimate(&setup, RateMode::PublishedOnly, &config).unwrap();
        assert_eq!(no_gift.scenarios[0].earned_points, 0);
    }

    #[test]
    fn confidence_band_does_not_fabricate_a_ninety_ninth_percentile_bound() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.set_targets(1, 0);
        let mut model = FeedbackConfig::for_game(Game::StarRail);
        model.samples = 256;
        let small = estimate(&setup, RateMode::PublishedOnly, &model).unwrap();
        let band = small.quantile_confidence_band(0.99, 0.95).unwrap();
        assert_eq!(band.upper, None);
        assert!(band.lower <= small.quantile(0.99));
        assert!(small.quantile_confidence_band(0.5, 1.0).is_none());
        model.samples = 20_000;
        let large = estimate(&setup, RateMode::PublishedOnly, &model).unwrap();
        let band = large.quantile_confidence_band(0.99, 0.95).unwrap();
        assert!(band.cdf_tolerance < 0.01);
        assert!(band.upper.is_some());
        assert!(band.lower <= large.quantile(0.99));
        assert!(band.upper.unwrap() >= large.quantile(0.99));
    }

    #[test]
    fn stella_maxed_return_starts_after_sixth_character_copy() {
        let mut setup = GameSetup::new(Game::StellaSora);
        setup.character.rule = BannerRule::FeaturedCounter {
            featured_rate: 1.0, hard_featured: 160,
        };
        setup.character.spark = None;
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.samples = 16;
        cfg.character_other_five_star_rate = 0.0;
        cfg.character_four_star_rate = 0.0;
        cfg.four_star_reward = 0;
        cfg.four_star_maxed = 0;
        cfg.stella_record_discs_per_character_pull = 0;
        setup.character_owned = 5;
        setup.set_targets(6, 0);
        let sixth = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(sixth.scenarios[0].raw_pulls, 1);
        assert_eq!(sixth.scenarios[0].earned_points, cfg.featured_duplicate);
        setup.character_owned = 6;
        // A duplicate of a fully upgraded character is 7th, after six owned.
        setup.set_targets(7, 0);
        let seventh = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(seventh.scenarios[0].raw_pulls, 1);
        assert_eq!(seventh.scenarios[0].earned_points, cfg.featured_maxed);
    }

    #[test]
    fn stella_first_ten_uses_prepaid_gold_disc_bonus_not_expert_permits() {
        let mut setup = GameSetup::new(Game::StellaSora);
        setup.character.rule = BannerRule::FeaturedCounter {
            featured_rate: 1.0, hard_featured: 160,
        };
        setup.weapon.rule = BannerRule::FeaturedCounter {
            featured_rate: 1.0, hard_featured: 120,
        };
        setup.character.spark = None;
        setup.character.target_copies = 1;
        setup.weapon.target_copies = 1;
        let mut cfg = FeedbackConfig::for_game(Game::StellaSora);
        cfg.samples = 16;
        cfg.featured_first = 0;
        cfg.featured_duplicate = 0;
        cfg.featured_maxed = 0;
        cfg.weapon_first = 0;
        cfg.weapon_rare = 0;
        cfg.off_banner_rare = 0;
        cfg.character_other_five_star_rate = 0.0;
        cfg.weapon_other_five_star_rate = 0.0;
        cfg.four_star_reward = 0;
        cfg.four_star_maxed = 0;
        cfg.weapon_four_star_reward = 0;
        cfg.stella_first_ten_remaining = 10;
        let boosted = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        for trial in &boosted.scenarios {
            // Paid 10-pull is indivisible even if the featured goal hits on draw 1.
            assert_eq!(trial.raw_pulls, 11);
            assert_eq!(trial.initial_flexible_required, 10);
            assert_eq!(trial.record_disc_units_earned, 900);
            assert_eq!(trial.earned_points, 0);
            assert_eq!(trial.dedicated_used, 1);
        }
        cfg.stella_first_ten_remaining = 0;
        let regular = estimate(&setup, RateMode::PublishedOnly, &cfg).unwrap();
        assert_eq!(regular.scenarios[0].raw_pulls, 2);
        assert_eq!(regular.scenarios[0].initial_flexible_required, 2);
        assert_eq!(regular.scenarios[0].record_disc_units_earned, 30);
        cfg.stella_first_ten_remaining = 2;
        assert!(cfg.validate().is_err());
        cfg.stella_first_ten_remaining = 11;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn four_star_base_rate_is_unconditional_on_the_five_star_miss() {
        let mut config = FeedbackConfig::for_game(Game::StellaSora);
        config.four_star_hard = 1_000_000; // isolate the base 4★ probability
        let mut rng = SplitMix64(0x449F_1234);
        let mut pity = 0;
        let mut rare_count = 0;
        let mut four_count = 0;
        let samples = 240_000;
        for _ in 0..samples {
            let rare = rng.hits(0.02);
            if rare { rare_count += 1; }
            if maybe_four_star(&mut rng, &config, BannerKind::Character,
                               &mut pity, rare, 0.02, true) {
                four_count += 1;
            }
        }
        assert!((rare_count as f64 / samples as f64 - 0.02).abs() < 0.002);
        assert!((four_count as f64 / samples as f64 - 0.04).abs() < 0.002);
        assert!((config.weapon_four_star_rate - 0.08).abs() < f64::EPSILON);
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

    #[test]
    fn cancelled_generation_returns_without_retaining_partial_results() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.set_targets(1, 1);
        let mut config = FeedbackConfig::for_game(Game::StarRail);
        config.samples = 2_048;
        let generation = AtomicU64::new(11);
        assert!(estimate_with_options(&setup, RateMode::PublishedOnly, &config,
            4, Some((&generation, 10))).is_err());

        generation.store(10, Ordering::Relaxed);
        let sequential = estimate_with_options(&setup, RateMode::PublishedOnly, &config,
            1, None).unwrap();
        let active = estimate_with_options(&setup, RateMode::PublishedOnly, &config,
            4, Some((&generation, 10))).unwrap();
        assert_eq!(sequential.scenarios, active.scenarios);
        assert_eq!(sequential.raw_pulls, active.raw_pulls);
        assert_eq!(sequential.budgets, active.budgets);
        assert_eq!(sequential.average_raw.to_bits(), active.average_raw.to_bits());
    }

