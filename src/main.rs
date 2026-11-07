#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use gacha_pickup_calculator::feedback::{estimate_with_options, FeedbackConfig, FeedbackEstimate, SuccessfulBudgetMeans};
#[cfg(test)]
use gacha_pickup_calculator::feedback::estimate;
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
// Regression tests use the binary's actual private UI/application interfaces.
#[cfg(test)]
#[path = "app/tests.rs"] mod chart_tests;
