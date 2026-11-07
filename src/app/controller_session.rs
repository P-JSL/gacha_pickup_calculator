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
        let repaint = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = work_rx.recv() {
                while let Ok(newer) = work_rx.try_recv() {
                    request = newer;
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
        let mut app = Self {
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

    /// Mark edited inputs stale without starting or scheduling a calculation.
    pub(crate) fn invalidate_calculation(&mut self) {
        self.input_needs_calculation = true;
        self.generation = self.generation.wrapping_add(1);
        self.calculating = false;
        self.ci_ready_pending = None;
    }

    /// Only called for an explicit Run Simulation button request.
    pub(crate) fn recalculate(&mut self) {
        self.setup.set_targets(self.target_eid + 1, self.target_refine);
        self.generation = self.generation.wrapping_add(1);
        self.calculating = true;
        self.ci_ready_pending = None;
        // Keep the last completed image on screen during background calculation.
        self.input_needs_calculation = false;
        self.error = None;
        let request = CalculationRequest {
            generation: self.generation,
            setup: self.setup.clone(),
            mode: self.mode,
            feedback_config: self.feedback_cfg.clone(),
        };
        if self.worker.send(request).is_err() {
            self.calculating = false;
            self.input_needs_calculation = true;
            self.error = Some("백그라운드 계산 작업을 시작할 수 없습니다.".to_owned());
            if self.ci_capture {
                eprintln!("GACHA_CI_SIM_FAILED game={:?} generation={}", self.setup.game, self.generation);
            }
        } else if self.ci_capture {
            eprintln!("GACHA_CI_SIM_STARTED game={:?} generation={}", self.setup.game, self.generation);
        }
    }

    pub(crate) fn poll_results(&mut self) {
        while let Ok(response) = self.results.try_recv() {
            // Ignore results from settings the user has since changed.
            if response.generation != self.generation {
                continue;
            }
            self.calculating = false;
            match response.result {
                Ok(stats) => {
                    let available = self.setup.available_flexible();
                    let raw = optimal_allocation(
                        &stats.character, &stats.weapon,
                        self.setup.character_dedicated, self.setup.weapon_dedicated, available,
                    );
                    let chance = stats.feedback.as_ref()
                        .map(|f| f.probability(available).0)
                        .unwrap_or(raw.success_probability);
                    self.history.insert(0, HistoryEntry {
                        game: self.setup.game, eid: self.target_eid,
                        refine: self.target_refine, chance,
                    });
                    self.history.truncate(5);
                    self.stats = Some(stats);
                    self.stats_game = Some(self.setup.game);
                    self.input_needs_calculation = false;
                    self.error = None;
                    if self.ci_capture { self.ci_ready_pending = Some(response.generation); }
                }
                Err(message) => {
                    self.input_needs_calculation = true;
                    self.error = Some(message);
                    if self.ci_capture {
                        eprintln!("GACHA_CI_SIM_FAILED game={:?} generation={}", self.setup.game, response.generation);
                    }
                }
            }
        }
    }

    /// Enter-driven navigation search; only actual sections and supported games are matched.
    pub(crate) fn activate_search(&mut self) -> bool {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return false;
        }
        let mut game_changed = false;
        let game = if query.contains("스타레일") || query.contains("hsr") {
            Some(Game::StarRail)
        } else if query.contains("명조") || query.contains("wuthering") {
            Some(Game::WutheringWaves)
        } else if query.contains("스텔라") || query.contains("stella") {
            Some(Game::StellaSora)
        } else if query.contains("이환") || query.contains("yihuan") || query.contains("neverness") {
            Some(Game::Neverness)
        } else {
            None
        };
        if let Some(game) = game {
            if self.setup.game != game {
                self.change_game(game);
                game_changed = true;
            }
            self.view_mode = ViewMode::Calculator;
        } else if query.contains("currency") || query.contains("재화") || query.contains("refund") || query.contains("환급") {
            self.view_mode = ViewMode::Currency;
        } else if query.contains("character") || query.contains("캐릭터") {
            self.view_mode = ViewMode::Characters;
        } else if query.contains("weapon") || query.contains("무기") || query.contains("광추")
            || query.contains("arc") || query.contains("레코드") {
            self.view_mode = ViewMode::Weapons;
        } else if query.contains("tracker") || query.contains("트래커") {
            self.view_mode = ViewMode::TrackerSources;
        } else if query.contains("csv") || query.contains("실측") {
            self.view_mode = ViewMode::ObservedDistribution;
        } else if query.contains("analytics") || query.contains("분석") {
            self.view_mode = ViewMode::Analytics;
        } else if query.contains("settings") || query.contains("환경") || query.contains("화면 설정") {
            self.view_mode = ViewMode::Preferences;
        } else if query.contains("simulator") || query.contains("시뮬레이션") {
            self.view_mode = ViewMode::Settings;
        } else if query.contains("dashboard") || query.contains("대시보드") {
            self.view_mode = ViewMode::Calculator;
        }
        self.search_query.clear();
        game_changed
    }

    /// Compact reference-style header with functional search and utilities.
    pub(crate) fn header(&mut self, ctx: &egui::Context) -> bool {
        let mut dirty = false;
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(egui::Margin { left: 15, right: 15, top: 15, bottom: 0 }))
            .show(ctx, |ui| {
                ui.add_space(30.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Gacha Pickup Calculator")
                            .strong().size(23.0));
                        ui.label(RichText::new("Probability & Refund Simulator")
                            .color(MUTED).size(12.0));
                    });
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Native vector settings button replaces the removed avatar.
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(39.0, 39.0), egui::Sense::click(),
                            );
                            ui.painter().rect_filled(rect, 10.0,
                                if response.hovered() { SELECTED_NAV } else { CARD });
                            let center = rect.center();
                            let stroke = egui::Stroke::new(1.8_f32, Color32::WHITE);
                            ui.painter().circle_stroke(center, 6.0, stroke);
                            ui.painter().circle_stroke(center, 2.5, stroke);
                            for i in 0..8 {
                                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                                let direction = egui::vec2(angle.cos(), angle.sin());
                                ui.painter().line_segment(
                                    [center + direction * 9.0, center + direction * 12.0],
                                    stroke,
                                );
                            }
                            if response.on_hover_text(t(self.language(), "Display & behavior settings")).clicked() {
                                self.view_mode = ViewMode::Preferences;
                            }

                            if ui.add(egui::Button::new("?")
                                .fill(CARD).corner_radius(10)
                                .min_size(egui::vec2(39.0, 39.0))).clicked() {
                                self.show_help = true;
                            }
                            ui.menu_button("♧", |ui| {
                                ui.label(t(self.language(), "Recent simulations"));
                                ui.label(format!("현재 세션 기록 {}건", self.history.len()));
                                if ui.button(t(self.language(), "View all history")).clicked() {
                                    self.show_history = true;
                                    ui.close_menu();
                                }
                            });
                            ui.menu_button("☆", |ui| {
                                ui.label(t(self.language(), "Select game"));
                                for game in Game::ALL {
                                    if ui.button(i18n::game_name(self.language(), game)).clicked() {
                                        if self.setup.game != game {
                                            self.change_game(game);
                                            dirty = true;
                                        }
                                        self.view_mode = ViewMode::Calculator;
                                        ui.close_menu();
                                    }
                                }
                            });
                            ui.menu_button(format!("{} ▾", self.language().code()), |ui| {
                                self.language_choices(ui);
                            });
                            ui.add_space(8.0);
                            let hint = format!("⌕  {}", t(self.language(), "Search games or sections..."));
                            let search_width = ui.available_width().clamp(60.0, 340.0);
                            let search = ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .desired_width(search_width)
                                    .hint_text(hint)
                                    .margin(egui::Margin::symmetric(10, 8))
                            );
                            if search.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                                dirty |= self.activate_search();
                            }
                        },
                    );
                });
            });
        dirty
    }

    pub(crate) fn sidebar(&mut self, ctx: &egui::Context) {
        let language = self.language();
        egui::SidePanel::left("workspace_sidebar")
            .resizable(false)
            .exact_width(222.0)
            .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(11))
            .show(ctx, |ui| {
                paint_navigation_art(ui);
                ui.add_space(136.0);
                for (view, label) in [
                    (ViewMode::Calculator, "Dashboard"),
                    (ViewMode::Settings, "Simulator"),
                    (ViewMode::Currency, "Currency"),
                    (ViewMode::Characters, "Characters"),
                    (ViewMode::Weapons, "Weapons"),
                    (ViewMode::Analytics, "Analytics"),
                ] {
                    sidebar_nav(ui, &mut self.view_mode, view, t(language, label));
                    ui.add_space(9.0);
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(13.0);
                sidebar_nav(ui, &mut self.view_mode, ViewMode::Preferences, t(language, "Settings"));
            });
    }

}
