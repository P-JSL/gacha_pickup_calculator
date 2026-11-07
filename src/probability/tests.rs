
    use super::banners::arc_batch_outcomes;
    use super::rates::NTE_COMMUNITY_FIRST_S_CDF;
    use super::*;

    fn always_rare(share: f64, guaranteed: bool, count: usize) -> Banner {
        Banner {
            rule: BannerRule::Standard { rarity: RateSchedule { base: 1.0, hard: 1, soft: SoftPity::None }, featured_share: share },
            pity: 0, guaranteed, target_copies: count, spark: None, any_rare_pity: 0,
        }
    }

    #[test]
    fn static_allocation_bound_preserves_exact_previous_search_results() {
        let character = Distribution::new(vec![0.0, 0.4, 0.6]);
        let weapon = Distribution::new(vec![0.0, 0.2, 0.5, 0.3]);
        for dc in 0..=4 {
            for dw in 0..=4 {
                for budget in 0..=40 {
                    let output = optimal_allocation(&character, &weapon, dc, dw, budget);
                    let mut reference = (0, 0, -1.0_f64);
                    for allotment in 0..=budget {
                        let c = dc + allotment;
                        let w = dw + budget - allotment;
                        let score = character.at(c) * weapon.at(w);
                        if score > reference.2 { reference = (c, w, score); }
                    }
                    assert_eq!((output.character_pulls, output.weapon_pulls),
                               (reference.0, reference.1));
                }
            }
        }
        let huge = optimal_allocation(&character, &weapon, 0, 0, 50_000_000);
        assert!(huge.success_probability > 0.999_999);
        assert_eq!(huge.character_pulls, 2);
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
    fn nte_arc_distribution_stops_at_actual_guarantee_not_trailing_zeros() {
        for initial_pity in 0..8 {
            for copies in [1, 2, 3] {
                let banner = Banner {
                    rule: BannerRule::NteArcBatch, pity: initial_pity,
                    guaranteed: false, target_copies: copies,
                    spark: None, any_rare_pity: 0,
                };
                let dist = banner.distribution(RateMode::PublishedOnly).unwrap();
                let guaranteed_pulls = (copies * 8 - initial_pity) * 10;
                assert_eq!(banner.max_additional_pulls(), guaranteed_pulls);
                assert_eq!(dist.pmf.len(), guaranteed_pulls + 1);
                assert_eq!(dist.quantile(1.0), guaranteed_pulls);
                assert!(dist.at(guaranteed_pulls) > 1.0 - 1e-10);
                assert!(dist.at(guaranteed_pulls - 10) < 1.0);
            }
        }
    }

    #[test]
    fn cheap_input_validation_agrees_with_distribution_validation() {
        for game in games::Game::ALL {
            let setup = games::GameSetup::new(game);
            for banner in [&setup.character, &setup.weapon] {
                assert!(banner.validate_inputs().is_ok());
                assert!(banner.distribution(RateMode::PublishedOnly).is_ok());
                let mut malformed = banner.clone();
                malformed.pity = usize::MAX;
                assert!(malformed.validate_inputs().is_err());
                assert!(malformed.distribution(RateMode::PublishedOnly).is_err());
                malformed.target_copies = 0;
                assert!(malformed.validate_inputs().is_ok());
                assert!(malformed.distribution(RateMode::PublishedOnly).is_ok());
            }
        }
    }

    #[test]
    fn invalid_or_extreme_banner_inputs_return_errors_instead_of_panics() {
        let zero = Banner {
            rule: BannerRule::Standard {
                rarity: RateSchedule { base: 0.5, hard: 0, soft: SoftPity::None },
                featured_share: 1.0,
            },
            pity: 5, guaranteed: false, target_copies: 1,
            spark: None, any_rare_pity: 0,
        };
        assert!(zero.distribution(RateMode::PublishedOnly).is_err());
        assert_eq!(zero.max_additional_pulls(), 0);
        let huge = Banner {
            rule: BannerRule::FeaturedCounter {
                featured_rate: 0.01, hard_featured: usize::MAX,
            },
            pity: 0, guaranteed: false, target_copies: 15,
            spark: None, any_rare_pity: 0,
        };
        assert_eq!(huge.max_additional_pulls(), usize::MAX);
        assert!(huge.distribution(RateMode::PublishedOnly).is_err());
    }

    #[test]
    fn empty_distribution_combination_does_not_underflow() {
        let empty = Distribution::new(vec![]);
        let populated = Distribution::new(vec![0.0, 1.0]);
        let combined = empty.combine(&populated);
        assert!(combined.pmf.is_empty());
        assert_eq!(combined.at(3), 0.0);
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
    fn adaptive_budget_beats_fixed_split() {
        // One success per banner with geometric waiting times and three tickets.
        let a = Banner { rule: BannerRule::Standard { rarity: RateSchedule {
            base: 0.5, hard: 4, soft: SoftPity::None }, featured_share: 1.0 },
            pity: 0, guaranteed: false, target_copies: 1,
            spark: None, any_rare_pity: 0 };
        let dist = a.distribution(RateMode::PublishedOnly).unwrap();
        let allocation = optimal_allocation(&dist, &dist, 0, 0, 3);
        assert!((allocation.success_probability - 0.5).abs() < 1e-12);
        assert!(allocation.success_probability > dist.at(1) * dist.at(2));
    }

    #[test]
    fn guaranteed_maximum_is_not_a_float_quantile() {
        let mut a = always_rare(1.0, false, 1).distribution(RateMode::PublishedOnly).unwrap();
        a.pmf = vec![0.0, 1.0 - 1e-12, 1e-12];
        a.cdf = vec![0.0, 1.0 - 1e-12, 1.0];
        assert_eq!(a.quantile(1.0), 2);
    }

    #[test]
    fn arc_batch_can_award_multiple_featured() {
        let events = arc_batch_outcomes();
        let total: f64 = events.iter().map(|e| e.2).sum();
        assert!((total - 1.0).abs() < 1e-12);
        assert!(events.iter().any(|&(f, _, p)| f >= 2 && p > 0.0));
        let dist = Banner { rule: BannerRule::NteArcBatch, pity: 0,
            guaranteed: false, target_copies: 2, spark: None, any_rare_pity: 0 }
            .distribution(RateMode::PublishedOnly).unwrap();
        assert!(dist.at(10) > 0.0);
    }

    #[test]
    fn already_unclaimed_spark_requires_zero_additional_pulls() {
        let b = Banner { rule: BannerRule::FeaturedCounter { featured_rate: 0.01, hard_featured: 160 },
            pity: 0, guaranteed: false, target_copies: 1,
            spark: Some(Spark { at: 120, progress: 120, used: false }), any_rare_pity: 0 };
        let dist = b.distribution(RateMode::PublishedOnly).unwrap();
        assert_eq!(dist.at(0), 1.0);
    }

    #[test]
    fn all_game_profiles_have_normalized_first_passage_distributions() {
        for game in games::Game::ALL {
            let profile = games::GameSetup::new(game);
            for banner in [&profile.character, &profile.weapon] {
                let dist = banner.distribution(RateMode::CommunityEstimate).unwrap();
                let total: f64 = dist.pmf.iter().sum();
                assert!((total - 1.0).abs() < 1e-8, "{game:?}: {total}");
                assert!(dist.cdf.windows(2).all(|w| w[0] <= w[1] + 1e-11));
                assert!(dist.mean.is_finite());
            }
        }
    }

    #[test]
    fn star_rail_e6_s5_independent_reference_mean() {
        let mut profile = games::GameSetup::new(games::Game::StarRail);
        // Preserve the historical reference fixture, not the fresh-account preset.
        profile.character.pity = 3;
        profile.character.guaranteed = true;
        let ch = profile.character.distribution(RateMode::CommunityEstimate).unwrap();
        let lc = profile.weapon.distribution(RateMode::CommunityEstimate).unwrap();
        let both = ch.combine(&lc);
        // Independent Python DP cross-check for 3 pity, first limited guaranteed.
        assert!((ch.mean - 621.0719).abs() < 1e-3);
        assert!((lc.mean - 333.8191).abs() < 1e-3);
        assert!((both.mean - 954.891).abs() < 1e-3);
        assert_eq!(both.quantile(0.9), 1126);
    }

    #[test]
    fn nte_community_board_fit_matches_fan_cdf_without_changing_official_mode() {
        let mut setup = games::GameSetup::new(games::Game::Neverness);
        setup.character.target_copies = 1;
        let unofficial = setup.character.distribution(RateMode::CommunityEstimate).unwrap();
        for &(pulls, cumulative) in NTE_COMMUNITY_FIRST_S_CDF.iter().skip(1) {
            assert!((unofficial.at(pulls) - cumulative).abs() < 1e-8,
                "community board fit at {pulls} pulls");
        }
        let published = setup.character.distribution(RateMode::PublishedOnly).unwrap();
        assert_eq!(published.quantile(1.0), 90);
        assert_eq!(unofficial.quantile(1.0), 90);
        assert_ne!(published.at(10), unofficial.at(10));
    }

    #[test]
    fn nte_base_is_not_consolidated_rate() {
        let profile = games::GameSetup::new(games::Game::Neverness);
        if let BannerRule::Standard { ref rarity, .. } = profile.character.rule {
            assert_eq!(rarity.chance(70, RateMode::PublishedOnly), 0.0099);
            assert_eq!(rarity.chance(71, RateMode::PublishedOnly), 0.1959);
            assert_eq!(rarity.chance(90, RateMode::PublishedOnly), 1.0);
        } else { panic!("expected NTE character board rule"); }
    }

    #[test]
    fn one_time_stella_spark_shortens_true_hard_guarantee() {
        for (copies, maximum) in [(1, 120), (2, 160), (3, 320)] {
            let banner = Banner {
                rule: BannerRule::FeaturedCounter { featured_rate: 0.0, hard_featured: 160 },
                pity: 0, guaranteed: false, target_copies: copies,
                spark: Some(Spark { at: 120, progress: 0, used: false }),
                any_rare_pity: 0,
            };
            assert_eq!(banner.max_additional_pulls(), maximum);
            let dist = banner.distribution(RateMode::PublishedOnly).unwrap();
            assert_eq!(dist.quantile(1.0), maximum);
            assert_eq!(dist.at(maximum), 1.0);
        }
    }

    #[test]
    fn wuthering_soft_starts_after_65() {
        let model = RateSchedule { base: 0.008, hard: 80, soft: SoftPity::WutheringEstimated };
        assert_eq!(model.chance(65, RateMode::CommunityEstimate), 0.008);
        assert!(model.chance(66, RateMode::CommunityEstimate) > 0.008);
        assert_eq!(model.chance(80, RateMode::CommunityEstimate), 1.0);
        assert_eq!(model.chance(79, RateMode::PublishedOnly), 0.008);
    }
