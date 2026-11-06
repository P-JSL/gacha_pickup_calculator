//! Contract tests for the 1.0 public probability and resource interfaces.
//! These are integration tests: they do not access private implementation details.
use gacha_pickup_calculator::games::{Game, GameSetup};
use gacha_pickup_calculator::{optimal_allocation, BannerRule, RateMode};

const MODES: [RateMode; 2] = [RateMode::PublishedOnly, RateMode::CommunityEstimate];

#[test]
fn all_game_models_normalize_and_preserve_guarantee_boundaries() {
    for game in Game::ALL {
        let mut setup = GameSetup::new(game);
        setup.set_targets(1, 1);
        for mode in MODES {
            for (name, banner) in [("character", &setup.character), ("equipment", &setup.weapon)] {
                banner.validate_inputs().unwrap();
                let dist = banner.distribution(mode).unwrap();
                let sum: f64 = dist.pmf.iter().sum();
                assert!((sum - 1.0).abs() < 1e-8, "{game:?}/{name}/{mode:?}: mass={sum}");
                assert!(dist.cdf.windows(2).all(|w| w[0] <= w[1] + 1e-10),
                    "{game:?}/{name}/{mode:?}: nonmonotonic CDF");
                assert_eq!(dist.quantile(0.0), 0);
                assert!(dist.quantile(1.0) <= banner.max_additional_pulls(),
                    "{game:?}/{name}/{mode:?}: guarantee violated");
                assert!(dist.mean.is_finite() && dist.mean > 0.0);
                assert!((dist.at(usize::MAX) - 1.0).abs() < 1e-8);
            }
        }
    }
}

#[test]
fn existing_copies_reduce_only_the_outstanding_goal() {
    for game in Game::ALL {
        let mut setup = GameSetup::new(game);
        for goal in 0..=7 {
            setup.character_owned = 0;
            setup.weapon_owned = 0;
            setup.set_targets(goal, goal);
            for owned in 0..=7 {
                setup.character_owned = owned;
                setup.weapon_owned = owned;
                setup.set_targets(goal, goal);
                assert_eq!(setup.character.target_copies, goal.saturating_sub(owned));
                assert_eq!(setup.weapon.target_copies, goal.saturating_sub(owned));
            }
        }
    }
}

#[test]
fn fractional_currency_is_not_lost_or_double_counted() {
    for game in Game::ALL {
        let mut setup = GameSetup::new(game);
        let price = game.cost_per_pull();
        assert!(price > 0);
        setup.flexible = 2;
        setup.premium_currency = price - 1;
        assert_eq!(setup.available_flexible(), 2);
        assert_eq!(setup.additional_currency_for(3), 1);
        setup.premium_currency = price + 1;
        assert_eq!(setup.available_flexible(), 3);
        assert_eq!(setup.additional_currency_for(3), 0);
        assert_eq!(setup.additional_currency_for(4), price - 1);
    }
}

#[test]
fn stella_record_discs_round_trip_at_ticket_boundaries() {
    let mut setup = GameSetup::new(Game::StellaSora);
    for units in [0, 1, 299, 300, 301, 899, 900, 30_000] {
        setup.set_weapon_ticket_inventory(units);
        assert_eq!(setup.weapon_ticket_inventory(), units);
        assert!(setup.weapon_ticket_remainder < 300);
    }
}

#[test]
fn adaptive_budget_success_is_monotonic_and_bounded() {
    for game in Game::ALL {
        let mut setup = GameSetup::new(game);
        setup.set_targets(1, 1);
        let a = setup.character.distribution(RateMode::PublishedOnly).unwrap();
        let b = setup.weapon.distribution(RateMode::PublishedOnly).unwrap();
        let mut previous = 0.0_f64;
        for budget in 0..=100 {
            let found = optimal_allocation(&a, &b, 0, 0, budget);
            let p = found.success_probability;
            assert!((0.0..=1.0).contains(&p), "{game:?}, {budget}: {p}");
            assert!(p + 1e-10 >= previous, "{game:?}: budget {budget} reduced success");
            previous = p;
        }
        let guarantee = optimal_allocation(
            &a, &b, setup.character.max_additional_pulls(),
            setup.weapon.max_additional_pulls(), 0,
        );
        assert!((guarantee.success_probability - 1.0).abs() < 1e-8,
            "{game:?}: dedicated guarantee failed");
    }
}

#[test]
fn invalid_pity_and_rates_fail_before_probability_calculation() {
    for game in Game::ALL {
        let mut setup = GameSetup::new(game);
        setup.set_targets(1, 1);
        let banner = &mut setup.character;
        banner.pity = usize::MAX;
        assert!(banner.validate_inputs().is_err());
        assert!(banner.distribution(RateMode::PublishedOnly).is_err());
    }
    let mut setup = GameSetup::new(Game::StarRail);
    setup.set_targets(1, 1);
    if let BannerRule::Standard { ref mut rarity, .. } = setup.character.rule {
        rarity.base = f64::NAN;
        assert!(setup.character.validate_inputs().is_err());
        assert!(setup.character.distribution(RateMode::PublishedOnly).is_err());
    } else {
        panic!("expected standard banner");
    }
}
