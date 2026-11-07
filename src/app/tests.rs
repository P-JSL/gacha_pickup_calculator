
    use super::*;
    #[test]
    fn simulation_requires_explicit_user_action_by_default() {
        let prefs = UiPreferences::default();
        assert!(prefs.show_hover_guides);
        // Neither auto-recalculate nor a debounce timer exists in UI preferences.
        // Calculation is issued only after the Run Simulation click is consumed.
    }

    fn deterministic_distribution(pulls: usize) -> Distribution {
        let mut pmf = vec![0.0; pulls + 1];
        pmf[pulls] = 1.0;
        Distribution::new(pmf)
    }

    fn deterministic_goals(setup: &GameSetup) -> Computed {
        let character = deterministic_distribution(setup.character.target_copies);
        let weapon = deterministic_distribution(setup.weapon.target_copies);
        let combined = character.combine(&weapon);
        let raw_shared = remaining_shared_distribution(&character, &weapon,
            setup.character_dedicated, setup.weapon_dedicated);
        let exact_shared_required = Some(CONFIDENCE_LEVELS.map(|confidence| {
            raw_shared.quantile(confidence as f64 / 100.0)
        }));
        Computed {
            character, weapon, combined, raw_shared, feedback: None,
            successful_budget_means: None,
            wuwa_strategies: None, exact_shared_required,
        }
    }

    #[test]
    fn changing_owned_copies_reduces_goal_budget_in_all_four_games() {
        for game in Game::ALL {
            let mut setup = GameSetup::new(game);
            setup.set_targets(3, 2);
            let initial = GoalFunding::calculate(&setup, &deterministic_goals(&setup),
                3, 2, 90);
            assert_eq!(initial.character_missing, 3);
            assert_eq!(initial.weapon_missing, 2);
            assert_eq!(initial.required_shared, 5);

            setup.character_owned = 2;
            setup.weapon_owned = 1;
            setup.set_targets(3, 2);
            let partial = GoalFunding::calculate(&setup, &deterministic_goals(&setup),
                3, 2, 90);
            assert_eq!(partial.character_missing, 1);
            assert_eq!(partial.weapon_missing, 1);
            assert_eq!(partial.required_shared, 2);

            setup.character_owned = 3;
            setup.weapon_owned = 2;
            setup.set_targets(3, 2);
            let completed = GoalFunding::calculate(&setup, &deterministic_goals(&setup),
                3, 2, 90);
            assert_eq!(completed.character_missing, 0);
            assert_eq!(completed.weapon_missing, 0);
            assert_eq!(completed.additional_shared, 0);
            assert_eq!(completed.additional_currency, 0);
        }
    }

    #[test]
    fn dedicated_passes_and_partial_native_currency_reduce_only_the_right_budget() {
        let mut setup = GameSetup::new(Game::WutheringWaves);
        setup.character_owned = 2;
        setup.weapon_owned = 1;
        setup.set_targets(3, 2);
        setup.character_dedicated = 1;
        setup.premium_currency = 159;
        let stats = deterministic_goals(&setup);
        let gap = GoalFunding::calculate(&setup, &stats, 3, 2, 90);
        assert_eq!(gap.character_single_shared, 0);
        assert_eq!(gap.weapon_single_shared, 1);
        assert_eq!(gap.required_shared, 1);
        assert_eq!(gap.available_shared, 0);
        assert_eq!(gap.additional_shared, 1);
        assert_eq!(gap.additional_currency, 1); // 159 already owned of the 160 required

        setup.premium_currency = 160;
        let filled = GoalFunding::calculate(&setup, &stats, 3, 2, 90);
        assert_eq!(filled.available_shared, 1);
        assert_eq!(filled.additional_shared, 0);
        assert_eq!(filled.additional_currency, 0);

        // A weapon pass must not be counted as a character pass.
        setup.character_dedicated = 0;
        setup.weapon_dedicated = 1;
        setup.premium_currency = 0;
        let new_gap = GoalFunding::calculate(&setup, &deterministic_goals(&setup),
            3, 2, 90);
        assert_eq!(new_gap.character_single_shared, 1);
        assert_eq!(new_gap.weapon_single_shared, 0);
        assert_eq!(new_gap.additional_shared, 1);
    }

    #[test]
    fn shared_star_rail_ticket_is_not_counted_twice() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.character_owned = 1;
        setup.set_targets(1, 1);
        setup.flexible = 1; // Star Rail has one common limited ticket type.
        let gap = GoalFunding::calculate(&setup, &deterministic_goals(&setup),
            1, 1, 90);
        assert_eq!(gap.character_missing, 0);
        assert_eq!(gap.weapon_missing, 1);
        assert_eq!(gap.required_shared, 1);
        assert_eq!(gap.available_shared, 1);
        assert_eq!(gap.additional_shared, 0);
    }

    #[test]
    fn exact_budget_uses_joint_adaptive_success_not_added_solo_quantiles() {
        let character = Distribution::new(vec![0.0, 0.5, 0.5]);
        let weapon = deterministic_distribution(1);
        assert_eq!(minimum_shared_pulls(&character, &weapon, 0, 0, 0.5), 2);
        assert_eq!(minimum_shared_pulls(&character, &weapon, 0, 0, 0.9), 3);
        assert_eq!(minimum_shared_pulls(&character, &weapon, 1, 0, 0.9), 2);
    }

    #[test]
    fn feedback_budget_uses_recycled_pulls_instead_of_the_raw_sum() {
        use gacha_pickup_calculator::{Banner, RateSchedule, SoftPity};
        let mut setup = GameSetup::new(Game::StarRail);
        setup.character = Banner {
            rule: BannerRule::Standard {
                rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
                featured_share: 1.0,
            },
            pity: 0, guaranteed: false, target_copies: 2,
            spark: None, any_rare_pity: 0,
        };
        setup.weapon.target_copies = 0;
        let mut config = FeedbackConfig::for_game(Game::StarRail);
        config.samples = 10;
        config.featured_first = 20;
        config.featured_duplicate = 20;
        let feedback = estimate(&setup, RateMode::PublishedOnly, &config).unwrap();
        let character = setup.character.distribution(RateMode::PublishedOnly).unwrap();
        let weapon = setup.weapon.distribution(RateMode::PublishedOnly).unwrap();
        let stats = Computed {
            combined: character.combine(&weapon),
            raw_shared: remaining_shared_distribution(&character, &weapon, 0, 0),
            character, weapon, feedback: Some(Arc::new(feedback)),
            successful_budget_means: None,
            wuwa_strategies: None, exact_shared_required: None,
        };
        let gap = GoalFunding::calculate(&setup, &stats, 2, 0, 90);
        assert_eq!(gap.character_single_raw, 2);
        assert_eq!(gap.required_shared, 1);
        assert_eq!(gap.additional_shared, 1);
        assert!(gap.uses_refunds);
    }

    #[test]
    fn dashboard_refund_probability_and_ticket_shortfall_use_same_distribution() {
        use gacha_pickup_calculator::{Banner, RateSchedule, SoftPity};
        let mut setup = GameSetup::new(Game::StarRail);
        let certain = BannerRule::Standard {
            rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None },
            featured_share: 1.0,
        };
        setup.character = Banner {
            rule: certain.clone(), pity: 0, guaranteed: false,
            target_copies: 2, spark: None, any_rare_pity: 0,
        };
        setup.weapon = Banner {
            rule: certain, pity: 0, guaranteed: false,
            target_copies: 1, spark: None, any_rare_pity: 0,
        };
        // Three deterministic pulls, but each completed roll recycles
        // a ticket for the NEXT roll. Only one initial shared ticket is needed.
        setup.flexible = 1;
        let mut config = FeedbackConfig::for_game(Game::StarRail);
        config.samples = 128;
        config.points_per_ticket = 1;
        config.featured_first = 1;
        config.featured_duplicate = 1;
        config.weapon_first = 1;
        config.weapon_rare = 1;
        config.four_star_reward = 0;
        config.weapon_four_star_reward = 0;
        let feedback = estimate(&setup, RateMode::PublishedOnly, &config).unwrap();
        let character = setup.character.distribution(RateMode::PublishedOnly).unwrap();
        let weapon = setup.weapon.distribution(RateMode::PublishedOnly).unwrap();
        let raw_shared = remaining_shared_distribution(&character, &weapon, 0, 0);
        let stats = Computed {
            combined: character.combine(&weapon),
            character, weapon, raw_shared,
            feedback: Some(Arc::new(feedback)),
            successful_budget_means: None, wuwa_strategies: None,
            exact_shared_required: None,
        };
        let gap = GoalFunding::calculate(&setup, &stats, 2, 1, 90);
        assert_eq!(gap.raw_required_shared, 3);
        assert_eq!(gap.required_shared, 1);
        assert_eq!(gap.quantile_change, 2);
        assert_eq!(gap.additional_shared, 0);
        assert_eq!(gap.additional_currency, 0);
        assert_eq!(gap.raw_probability, 0.0);
        assert_eq!(gap.current_probability, 1.0);
        assert!(gap.probability_uncertainty.is_some());
        let path = gap.boundary_path.unwrap();
        assert_eq!(path.raw_pulls, 3);
        assert_eq!(path.dedicated_used, 0);
        assert_eq!(path.points_funded_pulls, 2);
        assert_eq!(path.initial_flexible_required, 1);
        assert_eq!(path.raw_pulls, path.dedicated_used
            + path.points_funded_pulls + path.initial_flexible_required);
        // At n=128, the simultaneous DKW band cannot bound the 90th
        // percentile from above: do not fabricate a finite upper limit.
        assert_eq!(gap.quantile_uncertainty.unwrap().1, None);

        // Increasing the pre-existing balance can never reduce the success
        // probability, and no refund may subsidize the first payment.
        setup.flexible = 0;
        let no_balance = GoalFunding::calculate(&setup, &stats, 2, 1, 90);
        assert_eq!(no_balance.additional_shared, 1);
        assert_eq!(no_balance.current_probability, 0.0);
        assert_eq!(no_balance.raw_probability, 0.0);
        setup.flexible = 3;
        let abundant = GoalFunding::calculate(&setup, &stats, 2, 1, 90);
        assert_eq!(abundant.additional_shared, 0);
        assert_eq!(abundant.raw_probability, 1.0);
        assert_eq!(abundant.current_probability, 1.0);
    }

    #[test]
    fn dashboard_disables_refund_difference_when_refunds_are_not_simulated() {
        let mut setup = GameSetup::new(Game::StarRail);
        setup.set_targets(2, 1);
        setup.flexible = 2;
        let stats = deterministic_goals(&setup);
        let gap = GoalFunding::calculate(&setup, &stats, 2, 1, 90);
        assert!(!gap.uses_refunds);
        assert_eq!(gap.required_shared, gap.raw_required_shared);
        assert_eq!(gap.quantile_change, 0);
        assert!(gap.boundary_path.is_none());
        assert!(gap.quantile_uncertainty.is_none());
        assert!(gap.probability_uncertainty.is_none());
        assert_eq!(gap.current_probability, gap.raw_probability);
    }

    #[test]
    fn selecting_stella_sora_character_updates_correct_disc_across_tabs() {
        let mut character = "162".to_owned();
        let mut disc = "214066".to_owned();
        assert!(apply_catalog_choice(
            Game::StellaSora, catalog::Kind::Character, "139",
            &mut character, &mut disc,
        ));
        assert_eq!(character, "139");
        assert_eq!(disc, "214061"); // Allie / Bedtime Story
        assert!(apply_catalog_choice(
            Game::StellaSora, catalog::Kind::Character, "162",
            &mut character, &mut disc,
        ));
        assert_eq!(disc, "214066"); // Firstlight Noya / My Guiding Star
        assert!(apply_catalog_choice(
            Game::StellaSora, catalog::Kind::Weapon, "214005",
            &mut character, &mut disc,
        ));
        assert_eq!(character, "162");
        assert_eq!(disc, "214005"); // Manual disc override remains possible
    }

    #[test]
    fn selecting_any_mapped_character_resolves_its_unique_five_star_equipment() {
        for line in include_str!("../signature_pairs.tsv").lines().filter(|l|
            !l.is_empty() && !l.starts_with('#')
        ) {
            let p: Vec<_> = line.split('\t').collect();
            let game = match p[0] {
                "hsr" => Game::StarRail, "wuwa" => Game::WutheringWaves,
                "nte" => Game::Neverness, "sora" => Game::StellaSora,
                _ => panic!("unknown game in mapping"),
            };
            let mut character = catalog::first(game, catalog::Kind::Character);
            let mut weapon = catalog::first(game, catalog::Kind::Weapon);
            assert!(apply_catalog_choice(game, catalog::Kind::Character, p[1],
                &mut character, &mut weapon), "{line}");
            assert_eq!(character, p[1]);
            assert_eq!(weapon, p[2], "wrong equipment for {line}");
        }
    }

    #[test]
    fn unmapped_character_clears_stale_equipment_but_allows_manual_choice() {
        let mut character = "1503".to_owned();
        let mut weapon = "23028".to_owned();
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Character,
            "1503", &mut character, &mut weapon));
        assert_eq!(weapon, "23055"); // Pearl -> Colors for Tomorrow
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Character,
            "1511", &mut character, &mut weapon));
        assert_eq!(character, "1511");
        assert_eq!(weapon, "23065"); // Confirmed v4.7 Aha signature
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Weapon,
            "23028", &mut character, &mut weapon));
        assert_eq!(weapon, "23028"); // Explicit manual choice remains available
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Character,
            "1503", &mut character, &mut weapon));
        assert_eq!(weapon, "23055"); // Changing character restores its signature
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Character,
            "upcoming_astra_yao", &mut character, &mut weapon));
        assert!(weapon.is_empty()); // v4.8 signatures not announced
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Character,
            "upcoming_ellen_joe", &mut character, &mut weapon));
        assert!(weapon.is_empty());
        assert!(apply_catalog_choice(Game::StarRail, catalog::Kind::Character,
            "1511", &mut character, &mut weapon));
        assert_eq!(weapon, "23065"); // Restores Aha signature
        assert!(!apply_catalog_choice(Game::StarRail, catalog::Kind::Weapon,
            "none", &mut character, &mut weapon));
        assert_eq!(weapon, "23065");
    }

    #[test]
    fn default_selection_does_not_assign_unrelated_weapon() {
        let session = GameSession::defaults(Game::StarRail);
        assert_eq!(session.character_id, "1511");
        assert_eq!(session.weapon_id, "23065");
    }

    #[test]
    fn chart_tooltip_stays_within_plot_at_all_edges() {
        let graph = egui::Rect::from_min_size(egui::pos2(40.0, 20.0), egui::vec2(170.0, 90.0));
        for pos in [graph.left_top(), graph.right_top(), graph.left_bottom(),
                    graph.right_bottom(), graph.center()] {
            let rect = chart_tooltip_rect(graph, pos, 340.0, 22.0);
            assert!(graph.contains(rect.left_top()));
            assert!(graph.contains(rect.right_bottom()));
            assert!(rect.width() <= graph.width());
            assert!(rect.height() <= graph.height());
        }
    }

    #[test]
    fn sample_curve_has_endpoints_and_monotonic_probabilities() {
        let dist = Distribution::new(vec![0.0, 0.2, 0.3, 0.5]);
        let curve = sampled_cdf(&dist, 3);
        assert_eq!(curve.first().copied(), Some((0, 0.0)));
        assert_eq!(curve.last().copied(), Some((3, 1.0)));
        assert!(curve.windows(2).all(|x| x[0].0 < x[1].0 && x[0].1 <= x[1].1));
        let zero_goal = Distribution::new(vec![1.0]);
        let immediate = sampled_cdf(&zero_goal, 0);
        assert_eq!(immediate, vec![(0, 1.0), (1, 1.0)]);
    }

    #[test]
    fn character_and_weapon_chart_ranges_are_independent() {
        let character = Distribution::new(vec![0.0, 0.3, 0.7]);
        let mut weapon_pmf = vec![0.0; 301];
        weapon_pmf[300] = 1.0;
        let weapon = Distribution::new(weapon_pmf);
        let character_only = [ChartSeries {
            label: "캐릭터", source: ChartSource::Exact(&character), color: PRIMARY,
        }];
        let weapon_only = [ChartSeries {
            label: "무기", source: ChartSource::Exact(&weapon), color: MINT,
        }];
        assert_eq!(chart_domain_end(&character_only), 2);
        assert_eq!(chart_domain_end(&weapon_only), 300);
    }

    #[test]
    fn long_plot_curve_remains_bounded_and_includes_last_pity() {
        let mut pmf = vec![0.0; 1600];
        pmf[1599] = 1.0;
        let curve = sampled_cdf(&Distribution::new(pmf), 1599);
        assert_eq!(curve.len(), 401);
        assert_eq!(curve.last(), Some(&(1599, 1.0)));
    }
