#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use gacha_pickup_calculator::feedback::{estimate_with_options, FeedbackConfig, FeedbackEstimate, SuccessfulBudgetMeans};
use gacha_pickup_calculator::games::{Game, GameSetup};
use gacha_pickup_calculator::{optimal_allocation, BannerRule, Distribution, RateMode};
use gacha_pickup_calculator::tracker::{expected_for_banner, references, TrackedBanner};
use gacha_pickup_calculator::observations::{parse_observation_csv, ObservationDataset};
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "ui/dashboard.rs"] mod dashboard_ui;
#[path = "data/catalog.rs"] mod catalog;
#[path = "ui/i18n.rs"] mod i18n;
#[path = "data/security.rs"] mod security;
#[path = "data/runtime_updates.rs"] mod runtime_updates;
use i18n::{Language, LanguagePreference, t};


// Explicit module boundaries, with only crate-private internal interfaces.
// Keep the public library entrypoints and the eframe lifecycle unchanged.
#[path = "ui/navigation.rs"] mod navigation;
#[path = "ui/charts.rs"] mod charts;
#[path = "ui/funding_view.rs"] mod funding_view;
#[path = "app/funding.rs"] mod funding;
#[path = "app/compute.rs"] mod compute;
#[path = "app/state.rs"] mod state;
#[path = "app/controller_session.rs"] mod controller_session;
#[path = "app/controller_editor.rs"] mod controller_editor;
#[path = "app/controller_refunds.rs"] mod controller_refunds;
#[path = "app/controller_analytics.rs"] mod controller_analytics;
#[path = "app/controller_evidence.rs"] mod controller_evidence;
#[path = "app/lifecycle.rs"] mod lifecycle;

use navigation::*;
use charts::*;
use funding_view::*;
use funding::*;
use compute::*;
use state::*;
use controller_evidence::sampled_cdf;
use lifecycle::{configure_dark_desktop, install_korean_font};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1536.0, 864.0])
            .with_min_inner_size([1080.0, 700.0])
            .with_title("Gacha Pickup Calculator"),
        ..Default::default()
    };
    eframe::run_native("Gacha Lab", options,
        Box::new(|cc| Ok(Box::new(Calculator::new(cc)))))
}
#[cfg(test)]
mod chart_tests {
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
        let exact_shared_required = Some(CONFIDENCE_LEVELS.map(|confidence| {
            minimum_shared_pulls(&character, &weapon,
                setup.character_dedicated, setup.weapon_dedicated,
                confidence as f64 / 100.0)
        }));
        Computed {
            character, weapon, combined, feedback: None,
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
        for line in include_str!("signature_pairs.tsv").lines().filter(|l|
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
}

