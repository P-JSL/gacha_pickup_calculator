use super::*;

impl eframe::App for Calculator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let previous_view = self.view_mode;
        let previous_game = self.setup.game;
        // The preceding frame already painted the new worker result. Report
        // completion only for that same, still-current button-click generation.
        if let Some(generation) = self.ci_ready_pending.take() {
            if self.ci_capture && generation == self.generation && !self.calculating
                && !self.input_needs_calculation && self.error.is_none() {
                eprintln!("GACHA_CI_SIM_COMPLETED game={:?} generation={}", self.setup.game, generation);
            }
        }
        self.poll_results();
        if self.updates.poll(ctx) {
            session_log::info("DATA_UPDATE", format!(
                "Update manager status changed: {}", self.updates.message
            ));
            self.catalog_ui.invalidate_after_update();
        }
        self.catalog_ui.pump(ctx);
        // Compact dashboard edits and detailed settings use the same worker generation.
        // Reserve a full-height left rail first, as in the reference image.
        self.sidebar(ctx);
        if self.header(ctx) {
            self.invalidate_calculation();
        }
        // Reserve a usable central editing width. The optional right sidebar
        // collapses on smaller windows rather than overlapping input controls.
        if self.view_mode == ViewMode::Calculator && self.ui_preferences.show_result_panel
            && ctx.available_rect().width() >= 1_270.0 {
            self.workspace_results_panel(ctx);
        }
        let mut settings_changed = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(18))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.ui_preferences.compact_spacing {
                            ui.spacing_mut().item_spacing.y = 5.0;
                        } else {
                            ui.spacing_mut().item_spacing.y = 10.0;
                        }
                        if self.view_mode != ViewMode::Calculator { ui.add_space(5.0); }
                        match self.view_mode {
                            ViewMode::Calculator => {
                                settings_changed |= self.workspace_dashboard(ui);
                            }
                            ViewMode::Analytics => {
                                self.dashboard(ui);
                            }
                            ViewMode::Currency => {
                                page_intro(ui, "CURRENCY & REFUNDS",
                                    "재화 및 환급 설정",
                                    "전용권·공통 재화·환급 재투입 조건을 변경합니다.");
                                settings_changed |= self.resources(ui);
                                ui.add_space(10.0);
                                settings_changed |= self.feedback_controls(ui);
                            }
                            ViewMode::Characters | ViewMode::Weapons => {
                                page_intro(ui, "TARGET & PITY",
                                    if self.view_mode == ViewMode::Characters {
                                        "캐릭터 목표 관리"
                                    } else {
                                        "무기 · 광추 · 레코드 · Arc 목표 관리"
                                    },
                                    "선택한 배너만 편집하며, 입력값은 대시보드·시뮬레이터와 공유합니다.");
                                let weapon = self.view_mode == ViewMode::Weapons;
                                settings_changed |= self.single_banner_page(ui, weapon);
                            }
                            ViewMode::Preferences => {
                                self.desktop_settings(ui);
                            }
                            ViewMode::Settings => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("SIMULATION SETTINGS")
                                            .color(PRIMARY).size(10.0).strong());
                                        ui.heading(RichText::new("시뮬레이션 설정")
                                            .size(19.0).strong());
                                        ui.label(RichText::new(
                                            "목표, 천장, 보유 재화와 게임별 환급 가정을 변경합니다."
                                        ).color(MUTED));
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("대시보드 보기 →").clicked() {
                                                self.view_mode = ViewMode::Calculator;
                                            }
                                        },
                                    );
                                });
                                ui.add_space(7.0);
                                ui.menu_button("확률 모델 ▾", |ui| {
                                    settings_changed |= ui.radio_value(&mut self.mode,
                                        RateMode::PublishedOnly, "공개 기본 확률 + 확정 천장").changed();
                                    settings_changed |= ui.radio_value(&mut self.mode,
                                        RateMode::CommunityEstimate, "커뮤니티 추정 모델 (소프트 천장·이환 보드)").changed();
                                });
                                // Two compact input rows on desktop; no settings
                                // are removed, and narrow viewports stack safely.
                                if ui.available_width() >= 910.0 {
                                    ui.columns(2, |cols| {
                                        settings_changed |= self.targets(&mut cols[0]);
                                        settings_changed |= self.pity(&mut cols[1]);
                                    });
                                    ui.add_space(6.0);
                                    ui.columns(2, |cols| {
                                        settings_changed |= self.resources(&mut cols[0]);
                                        settings_changed |= self.feedback_controls(&mut cols[1]);
                                    });
                                } else {
                                    settings_changed |= self.targets(ui);
                                    ui.add_space(6.0);
                                    settings_changed |= self.pity(ui);
                                    ui.add_space(6.0);
                                    settings_changed |= self.resources(ui);
                                    ui.add_space(6.0);
                                    settings_changed |= self.feedback_controls(ui);
                                }
                                // The editor has its own explicit Run button so
                                // finishing inputs never requires switching tabs.
                                ui.add_space(9.0);
                                if ui.add_enabled(!self.calculating, egui::Button::new(
                                    t(self.language(), "Run Simulation"))).clicked() {
                                    self.manual_calculation_requested = true;
                                }
                            }
                            ViewMode::TrackerSources => {
                                page_intro(ui, "TRACKER EVIDENCE",
                                    "트래커 교차 검증", "기준 확률과 날짜가 표시된 외부 관측값을 비교합니다.");
                                self.tracker_sources(ui);
                            }
                            ViewMode::ObservedDistribution => {
                                page_intro(ui, "OBSERVATION DATA",
                                    "CSV 실측 분석", "로컬에서 스택별 관측 데이터를 분석합니다.");
                                self.observation_validation(ui);
                            }
                        }
                        ui.add_space(12.0);
                    });
            });
        if settings_changed {
            self.invalidate_calculation();
        }
        if self.manual_calculation_requested {
            self.manual_calculation_requested = false;
            self.recalculate();
        }
        if self.ci_ready_pending.is_some() {
            // Worker completion already requested the first paint; schedule
            // one more only when CI needs to observe the rendered result.
            ctx.request_repaint();
        }
        self.show_catalog_picker(ctx);
        if self.show_history {
            egui::Window::new(t(self.language(), "Simulation History"))
                .open(&mut self.show_history)
                .default_size(egui::vec2(470.0, 330.0))
                .resizable(true).show(ctx, |ui| {
                    ui.label(RichText::new(
                        "현재 실행에서 계산한 최근 결과입니다. 종료 시 초기화됩니다."
                    ).color(MUTED).size(11.0));
                    ui.separator();
                    for result in &self.history {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(result.game.label()).strong());
                            ui.label(format!("E{} · {} S{}",
                                result.eid, result.game.secondary_banner_name(),result.refine));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(format!("{:.2}%",
                                        result.chance * 100.0)).color(MINT).strong());
                                });
                        });
                        ui.separator();
                    }
                });
        }
        if self.show_help {
            egui::Window::new(t(self.language(), "Rules, confidence and sources"))
                .open(&mut self.show_help).resizable(true).show(ctx, |ui| {
                    ui.heading(self.setup.game.label());
                    ui.label(self.setup.game.special_notes());
                    ui.separator();
                    ui.label("정확 DP는 입력 확률 모델의 상태 전이를 계산합니다. 게임 서버의 비공개 확률식을 보장하지 않습니다.");
                    ui.label("환급은 사용자 입력 중복 보상 및 4성 발생 모델을 따른 Monte Carlo 추정입니다.");
                    for (title, url) in self.setup.game.source_links() {
                        ui.hyperlink_to(*title, *url);
                    }
                });
        }
        if self.view_mode != previous_view {
            session_log::info("NAVIGATION", format!(
                "view={:?} -> {:?}", previous_view, self.view_mode
            ));
        }
        if self.setup.game != previous_game {
            session_log::info("GAME_CHANGE", format!(
                "game={:?} -> {:?}", previous_game, self.setup.game
            ));
        }
        if let Some(logger) = session_log::current() {
            self.log_view.show(ctx, logger);
            if logger.flush_if_due() {
                // The next frame flushes buffered logs even if the user is
                // idle and the calculation worker has not finished.
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        if self.ci_capture && !self.ci_ui_ready_reported {
            self.ci_ui_ready_reported = true;
            eprintln!("GACHA_CI_UI_READY game={:?}", self.setup.game);
        }
    }
}

/// One theme and logical-point typography for Windows and Linux.
/// OS-level DPI still controls physical pixel density, but widget dimensions,
/// colors and CDF Painter are identical in egui points.
pub(crate) fn configure_dark_desktop(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = Color32::from_rgb(17, 25, 40);
    visuals.override_text_color = Some(Color32::from_rgb(231, 239, 249));
    visuals.widgets.noninteractive.fg_stroke.color = MUTED;
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 58, 79);
    visuals.widgets.active.bg_fill = Color32::from_rgb(43, 73, 99);
    visuals.selection.bg_fill = Color32::from_rgb(39, 102, 158);
    // Both dark visual slots and the current theme are explicitly set so a
    // Windows light OS theme won't reintroduce light native widget styling.
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(7.0, 5.0);
        style.spacing.button_padding = egui::vec2(9.0, 4.0);
        style.spacing.interact_size.y = 25.0;
        style.text_styles.insert(egui::TextStyle::Body,
            egui::FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Button,
            egui::FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Small,
            egui::FontId::proportional(11.0));
    });
}

pub(crate) fn install_korean_font(ctx: &egui::Context) {
    let locations = [
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\malgunbd.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ];
    for location in locations {
        if let Ok(bytes) = std::fs::read(location) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("korean".to_owned(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().insert(0, "korean".to_owned());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }
}

