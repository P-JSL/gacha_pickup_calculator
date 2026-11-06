use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueryMode { RequiredBudget, AvailableBudget }

/// Keep the primary calculator uncluttered. Advanced evidence inspection is
/// explicitly opened from the header rather than appended to every result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ViewMode { Calculator, Analytics, Settings, Preferences, Currency, Characters, Weapons, TrackerSources, ObservedDistribution }

/// Desktop display and interaction options. These never change the probability model.
#[derive(Clone, Copy)]
pub(crate) struct UiPreferences {
    pub(crate) auto_recalculate: bool,
    pub(crate) debounce_ms: u64,
    pub(crate) show_result_panel: bool,
    pub(crate) show_chart_fill: bool,
    pub(crate) show_hover_guides: bool,
    pub(crate) compact_spacing: bool,
}
impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            auto_recalculate: false,
            debounce_ms: 350,
            show_result_panel: true,
            show_chart_fill: true,
            show_hover_guides: true,
            compact_spacing: true,
        }
    }
}

/// Each game's currency, targets and selected artwork are independent.
#[derive(Clone)]
pub(crate) struct GameSession {
    pub(crate) setup: GameSetup,
    pub(crate) feedback: FeedbackConfig,
    pub(crate) eid: usize,
    pub(crate) refine: usize,
    pub(crate) character_id: String,
    pub(crate) weapon_id: String,
}

/// Apply a catalog choice to the shared game session. All tabs use these
/// same selected IDs; never infer signature equipment from list order.
pub(crate) fn apply_catalog_choice(
    game: Game, kind: catalog::Kind, id: &str,
    character: &mut String, weapon: &mut String,
) -> bool {
    let Some(entry) = catalog::live_by_id(game, kind, id) else { return false; };
    if !entry.is_featured_target() { return false; }
    match kind {
        catalog::Kind::Character => {
            *character = id.to_owned();
            *weapon = catalog::live_signature_weapon(game, id)
                .map(|signature| signature.id.clone()).unwrap_or_default();
            // Unknown signature: clear the previous character's equipment rather
            // than silently presenting an unrelated five-star as this one's own.
        }
        catalog::Kind::Weapon => *weapon = id.to_owned(),
    }
    true
}

impl GameSession {
    pub(crate) fn defaults(game: Game) -> Self {
        let character_id = catalog::live_first(game, catalog::Kind::Character);
        let weapon_id = catalog::live_signature_weapon(game, &character_id)
            .map(|entry| entry.id.clone()).unwrap_or_default();
        Self {
            setup: GameSetup::new(game),
            feedback: FeedbackConfig::for_game(game),
            eid: 0, refine: 1,
            character_id,
            weapon_id,
        }
    }
}

pub(crate) struct HistoryEntry {
    pub(crate) game: Game,
    pub(crate) eid: usize,
    pub(crate) refine: usize,
    pub(crate) chance: f64,
}

pub(crate) struct Calculator {
    pub(crate) setup: GameSetup,
    pub(crate) mode: RateMode,
    pub(crate) target_eid: usize,
    pub(crate) target_refine: usize,
    pub(crate) confidence: usize,
    pub(crate) query_mode: QueryMode,
    pub(crate) n_query: usize,
    pub(crate) feedback_cfg: FeedbackConfig,
    pub(crate) stats: Option<Computed>,
    pub(crate) stats_game: Option<Game>,
    pub(crate) input_needs_calculation: bool,
    pub(crate) manual_calculation_requested: bool,
    pub(crate) ui_preferences: UiPreferences,
    pub(crate) language_preference: LanguagePreference,
    pub(crate) system_language: Language,
    pub(crate) language_save_error: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) show_help: bool,
    pub(crate) view_mode: ViewMode,
    pub(crate) worker: Sender<CalculationRequest>,
    pub(crate) results: Receiver<CalculationResponse>,
    pub(crate) generation: u64,
    pub(crate) pending_since: Option<Instant>,
    pub(crate) calculating: bool,
    pub(crate) observation_path: String,
    pub(crate) observation_expected_sha256: String,
    pub(crate) observation_data: Option<ObservationDataset>,
    pub(crate) observation_banner: TrackedBanner,
    pub(crate) observation_message: String,
    pub(crate) history: Vec<HistoryEntry>,
    pub(crate) show_history: bool,
    pub(crate) search_query: String,
    pub(crate) catalog_ui: catalog::CatalogUi,
    pub(crate) updates: runtime_updates::UpdateManager,
    pub(crate) picker_kind: Option<catalog::Kind>,
    pub(crate) picker_filter: String,
    pub(crate) selected_character_id: String,
    pub(crate) selected_weapon_id: String,
    pub(crate) game_sessions: HashMap<Game, GameSession>,
}

