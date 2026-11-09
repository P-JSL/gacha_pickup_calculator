use super::*;

impl Calculator {
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

                            // Persistent in-process diagnostics remain available
                            // without opening a console or leaving this program.
                            if ui.add(egui::Button::new("로그")
                                .fill(CARD).corner_radius(10)
                                .min_size(egui::vec2(48.0, 39.0))).clicked() {
                                self.log_view.open = true;
                                session_log::info("LOG_VIEW_OPEN", "User opened live session log");
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
