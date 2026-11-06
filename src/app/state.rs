#[derive(Clone, Copy, PartialEq, Eq)]
enum QueryMode { RequiredBudget, AvailableBudget }

/// Keep the primary calculator uncluttered. Advanced evidence inspection is
/// explicitly opened from the header rather than appended to every result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode { Calculator, Analytics, Settings, Preferences, Currency, Characters, Weapons, TrackerSources, ObservedDistribution }

/// Desktop display and interaction options. These never change the probability model.
#[derive(Clone, Copy)]
struct UiPreferences {
    auto_recalculate: bool,
    debounce_ms: u64,
    show_result_panel: bool,
    show_chart_fill: bool,
    show_hover_guides: bool,
    compact_spacing: bool,
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
struct GameSession {
    setup: GameSetup,
    feedback: FeedbackConfig,
    eid: usize,
    refine: usize,
    character_id: String,
    weapon_id: String,
}

/// Apply a catalog choice to the shared game session. All tabs use these
/// same selected IDs; never infer signature equipment from list order.
fn apply_catalog_choice(
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
    fn defaults(game: Game) -> Self {
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

struct HistoryEntry {
    game: Game,
    eid: usize,
    refine: usize,
    chance: f64,
}

struct Calculator {
    setup: GameSetup,
    mode: RateMode,
    target_eid: usize,
    target_refine: usize,
    confidence: usize,
    query_mode: QueryMode,
    n_query: usize,
    feedback_cfg: FeedbackConfig,
    stats: Option<Computed>,
    stats_game: Option<Game>,
    input_needs_calculation: bool,
    manual_calculation_requested: bool,
    ui_preferences: UiPreferences,
    language_preference: LanguagePreference,
    system_language: Language,
    language_save_error: Option<String>,
    error: Option<String>,
    show_help: bool,
    view_mode: ViewMode,
    worker: Sender<CalculationRequest>,
    results: Receiver<CalculationResponse>,
    generation: u64,
    pending_since: Option<Instant>,
    calculating: bool,
    observation_path: String,
    observation_expected_sha256: String,
    observation_data: Option<ObservationDataset>,
    observation_banner: TrackedBanner,
    observation_message: String,
    history: Vec<HistoryEntry>,
    show_history: bool,
    search_query: String,
    catalog_ui: catalog::CatalogUi,
    updates: runtime_updates::UpdateManager,
    picker_kind: Option<catalog::Kind>,
    picker_filter: String,
    selected_character_id: String,
    selected_weapon_id: String,
    game_sessions: HashMap<Game, GameSession>,
}

