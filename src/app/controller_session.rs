use super::*;

impl Calculator {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // set_visuals alone changes only the current theme style. The system
        // preference could otherwise switch the UI back to Light on Windows.
        configure_dark_desktop(&cc.egui_ctx);
        install_korean_font(&cc.egui_ctx);
        // All potentially costly Markov-DP and refund simulations run off the GUI thread.
        // The worker drops queued stale requests before each calculation; the UI also
        // rejects out-of-generation responses to avoid showing outdated probabilities.
        let (worker, work_rx) = mpsc::channel::<CalculationRequest>();
        let (result_tx, results) = mpsc::channel::<CalculationResponse>();
        let active_generation = Arc::new(AtomicU64::new(0));
        let repaint = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = work_rx.recv() {
                while let Ok(newer) = work_rx.try_recv() {
                    request = newer;
                }
                // Skip queued work invalidated while the previous run was active.
                if request.active_generation.load(Ordering::Relaxed) != request.generation {
                    continue;
                }
                let result = compute(&request);
                if result_tx.send(CalculationResponse {
                    generation: request.generation,
                    result,
                }).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        // CI observes an explicit Run Simulation click and the corresponding
        // finished worker generation; it never forces automatic calculation.
        let ci_capture = std::env::var("GACHA_CI_SIMULATION_EVENTS").as_deref() == Ok("1");
        let initial_selection = GameSession::defaults(Game::StarRail);
        let app = Self {
            setup: GameSetup::new(Game::StarRail),
            mode: RateMode::PublishedOnly,
            target_eid: 0,
            target_refine: 1,
            confidence: 90,
            query_mode: QueryMode::RequiredBudget,
            n_query: 1_000,
            feedback_cfg: FeedbackConfig::for_game(Game::StarRail),
            stats: None,
            stats_game: None,
            input_needs_calculation: true,
            manual_calculation_requested: false,
            ui_preferences: UiPreferences::default(),
            language_preference: i18n::load_preference(),
            system_language: i18n::system_language(),
            language_save_error: None,
            error: None,
            show_help: false,
            view_mode: ViewMode::Calculator,
            worker,
            results,
            generation: 0,
            active_generation,
            calculating: false,
            ci_capture,
            ci_ui_ready_reported: false,
            ci_ready_pending: None,
            observation_path: String::new(),
            observation_expected_sha256: String::new(),
            observation_data: None,
            observation_banner: TrackedBanner::Character,
            observation_message: String::new(),
            history: Vec::new(),
            show_history: false,
            search_query: String::new(),
            catalog_ui: catalog::CatalogUi::new(&cc.egui_ctx),
            updates: runtime_updates::UpdateManager::new(),
            picker_kind: None,
            picker_filter: String::new(),
            selected_character_id: initial_selection.character_id,
            selected_weapon_id: initial_selection.weapon_id,
            game_sessions: HashMap::new(),
        };
        // Inputs remain editable. No worker request exists until the user
        // clicks Run Simulation after finishing the configuration.
        app
    }

    pub(crate) fn language(&self) -> Language {
        self.language_preference.effective(self.system_language)
    }
    pub(crate) fn set_language(&mut self, preference: LanguagePreference) {
        if self.language_preference == preference { return; }
        self.language_preference = preference;
        self.language_save_error = i18n::save_preference(preference).err().map(|e| e.to_string());
    }
    pub(crate) fn language_choices(&mut self, ui: &mut egui::Ui) {
        let active = self.language_preference;
        let language = self.language();
        for choice in LanguagePreference::ALL {
            if ui.selectable_label(choice == active, choice.display(self.system_language, language)).clicked() {
                self.set_language(choice);
                ui.close_menu();
            }
        }
    }

    /// Funding is hidden while inputs are stale: the current owned-copy counts
    /// must never be combined with previous distributions.
    pub(crate) fn current_goal_funding(&self) -> Option<GoalFunding> {
        if self.input_needs_calculation || self.calculating { return None; }
        let results = self.visible_stats()?;
        Some(GoalFunding::calculate(
            &self.setup, results, self.target_eid + 1,
            self.target_refine, self.confidence,
        ))
    }

    /// Last completed charts remain visible until the worker finishes for the same game.
    /// A game change never displays distributions from a different game.
    pub(crate) fn visible_stats(&self) -> Option<&Computed> {
        if self.stats_game == Some(self.setup.game) { self.stats.as_ref() } else { None }
    }

    /// Selection controls all reuse the catalog; target rarity remains a five-star model.
    pub(crate) fn catalog_selector(&mut self, ui: &mut egui::Ui, kind: catalog::Kind) {
        let game = self.setup.game;
        let id = match kind {
            catalog::Kind::Character => self.selected_character_id.clone(),
            catalog::Kind::Weapon => self.selected_weapon_id.clone(),
        };
        let item = catalog::live_by_id(game, kind, &id);
        ui.horizontal(|ui| {
            if let Some(item) = item.as_ref() {
                self.catalog_ui.image(ui, item, 30.0);
            }
            let name = item.as_ref().map(|x| x.name.as_str()).unwrap_or_else(|| {
                if kind == catalog::Kind::Weapon {
                    t(self.language(), "Signature unverified (select manually)")
                } else {
                    t(self.language(), "No target")
                }
            });
            let width = ui.available_width().clamp(70.0, 230.0);
            let shortened = truncated_selection(ui, name, width);
            if ui.add_sized(egui::vec2(width, 27.0),
                egui::Button::new(format!("{shortened}  ▾")))
                .on_hover_text(name).clicked() {
                self.picker_kind = Some(kind);
                self.picker_filter.clear();
            }
        });
    }

    pub(crate) fn show_catalog_picker(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.picker_kind else { return; };
        let mut open = true;
        let current = match kind {
            catalog::Kind::Character => &self.selected_character_id,
            catalog::Kind::Weapon => &self.selected_weapon_id,
        }.clone();
        let choice = catalog::picker(ctx, &mut self.catalog_ui, self.setup.game, kind,
            &mut open, &mut self.picker_filter, &current);
        if let Some(id) = choice {
            let previous_character = self.selected_character_id.clone();
            let previous_weapon = self.selected_weapon_id.clone();
            if apply_catalog_choice(
                self.setup.game, kind, &id,
                &mut self.selected_character_id, &mut self.selected_weapon_id,
            ) && (self.selected_character_id != previous_character
                || self.selected_weapon_id != previous_weapon) {
                // Choosing a character or weapon is an input edit, not a run.
                self.invalidate_calculation();
            }
            self.picker_kind = None;
        } else if !open {
            self.picker_kind = None;
        }
    }

    pub(crate) fn change_game(&mut self, game: Game) {
        if self.setup.game != game {
            // Save all editable fields so switching between four games never loses
            // inventory, target goals, pity state, selected item or refund assumptions.
            self.game_sessions.insert(self.setup.game, GameSession {
                setup: self.setup.clone(),
                feedback: self.feedback_cfg.clone(),
                eid: self.target_eid,
                refine: self.target_refine,
                character_id: self.selected_character_id.clone(),
                weapon_id: self.selected_weapon_id.clone(),
            });
            let next = self.game_sessions.remove(&game)
                .unwrap_or_else(|| GameSession::defaults(game));
            self.setup = next.setup;
            self.feedback_cfg = next.feedback;
            self.target_eid = next.eid;
            self.target_refine = next.refine;
            self.selected_character_id = next.character_id;
            self.selected_weapon_id = next.weapon_id;
        } else {
            // The reset action invokes change_game(current_game) to restore this
            // game's default inputs without touching saved values for other games.
            self.game_sessions.remove(&game);
            let defaults = GameSession::defaults(game);
            self.setup = defaults.setup;
            self.feedback_cfg = defaults.feedback;
            self.target_eid = defaults.eid;
            self.target_refine = defaults.refine;
            self.selected_character_id = defaults.character_id;
            self.selected_weapon_id = defaults.weapon_id;
        }
        self.picker_kind = None;
        self.picker_filter.clear();
        self.observation_data = None;
        self.observation_path.clear();
        self.observation_message.clear();
    }

}

// Distinct UI navigation and request lifecycle modules retain Calculator's
// original private state, event selectors, generation and worker contracts.
#[path = "controller_session/calculation.rs"] mod calculation;
#[path = "controller_session/navigation.rs"] mod navigation;
