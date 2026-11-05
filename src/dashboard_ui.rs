//! Reference-inspired Windows/Linux dashboard. Probability and refund engines stay unchanged.
use super::*;

const CYAN: Color32 = Color32::from_rgb(50, 218, 241);
const YELLOW: Color32 = Color32::from_rgb(244, 210, 75);
const PINK: Color32 = Color32::from_rgb(225, 112, 232);
const SUCCESS: Color32 = Color32::from_rgb(141, 238, 117);
const DEEP_CARD: Color32 = Color32::from_rgb(16, 43, 64);

/// Subtle vertical gradient; the rounded base remains visible at the corners.
fn wash(ui: &egui::Ui, rect: egui::Rect, upper: Color32, lower: Color32) {
    ui.painter().rect_filled(rect, 14.0, upper);
    let r = rect.shrink(9.0);
    if r.width() <= 0.0 || r.height() <= 0.0 { return; }
    let mut mesh = egui::Mesh::default();
    for (point, color) in [
        (r.left_top(), upper), (r.right_top(), upper),
        (r.left_bottom(), lower), (r.right_bottom(), lower),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: point, uv: egui::epaint::WHITE_UV, color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 2, 1, 1, 2, 3]);
    ui.painter().add(egui::Shape::mesh(mesh));

}

fn dashboard_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(DEEP_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(13)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(15.0).strong());
            ui.add_space(9.0);
            content(ui);
        });
}

fn metric_icon(p: &egui::Painter, c: egui::Pos2, kind: usize, color: Color32) {
    let stroke = egui::Stroke::new(2.1_f32, color);
    let pos = |x: f32,y: f32| egui::pos2(c.x + x, c.y + y);
    match kind {
        0 => { // ticket outline
            for pair in [
                [pos(-13.0,-9.0),pos(11.0,-13.0)],
                [pos(11.0,-13.0),pos(14.0,8.0)],
                [pos(14.0,8.0),pos(-10.0,12.0)],
                [pos(-10.0,12.0),pos(-13.0,-9.0)],
                [pos(0.0,-10.0),pos(3.0,9.0)]
            ] { p.line_segment(pair, stroke); }
        }
        1 => {
            p.circle_stroke(c, 13.0, stroke);
            p.circle_stroke(c, 5.0, stroke);
            p.line_segment([pos(-18.0,0.0),pos(-9.0,0.0)], stroke);
            p.line_segment([pos(9.0,0.0),pos(18.0,0.0)], stroke);
            p.line_segment([pos(0.0,-18.0),pos(0.0,-9.0)], stroke);
            p.line_segment([pos(0.0,9.0),pos(0.0,18.0)], stroke);
        }
        2 => {
            for pair in [
                [pos(0.0,-15.0),pos(13.0,-7.0)],
                [pos(13.0,-7.0),pos(13.0,10.0)],
                [pos(13.0,10.0),pos(0.0,17.0)],
                [pos(0.0,17.0),pos(-13.0,10.0)],
                [pos(-13.0,10.0),pos(-13.0,-7.0)],
                [pos(-13.0,-7.0),pos(0.0,-15.0)],
                [pos(0.0,0.0),pos(13.0,-7.0)],
                [pos(0.0,0.0),pos(-13.0,-7.0)],
                [pos(0.0,0.0),pos(0.0,17.0)]
            ] { p.line_segment(pair, stroke); }
        }
        _ => {
            let points = vec![pos(0.0,-17.0),pos(12.0,-6.0),pos(6.0,13.0),
                pos(0.0,18.0),pos(-9.0,8.0),pos(-12.0,-6.0)];
            p.add(egui::Shape::convex_polygon(points,
                color.gamma_multiply(0.45), stroke));
            p.line_segment([pos(-12.0,-6.0),pos(12.0,-6.0)],stroke);
            p.line_segment([pos(0.0,-17.0),pos(0.0,18.0)],stroke);
        }
    }
}

/// Paint a card with fixed geometry so values cannot push other dashboard cards down.
fn overview_metric(ui: &mut egui::Ui, kind: usize, label: &str, value: String, note: String, color: Color32) {
    let (r, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(90.0), 120.0), egui::Sense::hover(),
    );
    response.on_hover_text(format!("{label}\n{value}\n{note}"));
    let base = if kind == 1 { Color32::from_rgb(14, 59, 76) }
               else if kind == 2 { Color32::from_rgb(35, 57, 94) }
               else { Color32::from_rgb(28, 63, 96) };
    wash(ui, r, base, Color32::from_rgb(17, 43, 67));
    let paint = ui.painter_at(r.shrink(6.0));
    let compact = r.width() < 205.0;
    if !compact {
        metric_icon(&paint, egui::pos2(r.left()+31.0,r.top()+47.0),kind,color);
    }
    let left = if compact { r.left()+13.0 } else { r.left()+64.0 };
    paint.text(egui::pos2(left,r.top()+17.0),
        egui::Align2::LEFT_TOP, label, egui::FontId::proportional(11.0),
        Color32::WHITE);
    paint.text(egui::pos2(left,r.top()+45.0),
        egui::Align2::LEFT_TOP, value,
        egui::FontId::proportional(if compact { 21.0 } else { 25.0 }),
        Color32::WHITE);
    paint.text(egui::pos2(left,r.top()+94.0),
        egui::Align2::LEFT_TOP, note, egui::FontId::proportional(10.0),
        MUTED);
}

/// Compact 3-column chart panel: header, right-aligned result badge, area graph.
fn probability_card(ui: &mut egui::Ui, title: &str, badge: String, source: &[ChartSeries<'_>], color: Color32, appearance: UiPreferences) {
    egui::Frame::group(ui.style())
        .fill(DEEP_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(14.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("›").size(19.0).color(MUTED));
                });
            });
            ui.add_space(6.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = RichText::new(badge).size(10.5).color(color).strong();
                    egui::Frame::default()
                        .fill(Color32::from_rgb(7, 30, 43))
                        .stroke(egui::Stroke::new(0.8_f32, color))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(7, 3))
                        .show(ui, |ui| { ui.label(label); });
                });
            });
            draw_probability_chart(ui, source, appearance);
        });
}

/// Inset card used by each quick editor, matching the mockup's two-row tiles.
fn setting_tile(ui: &mut egui::Ui, icon: &str, label: &str,
    inner: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(28, 59, 83))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(48.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(icon).size(16.0).color(Color32::WHITE));
                ui.add(egui::Label::new(
                    RichText::new(label).size(10.5).color(Color32::WHITE).strong()
                ).truncate()).on_hover_text(label);
            });
            ui.scope(|ui| {
                ui.visuals_mut().extreme_bg_color = Color32::from_rgb(10, 35, 54);
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_rgb(13, 35, 55);
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_rgb(13, 35, 55);
                ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(20, 54, 78);
                ui.visuals_mut().widgets.active.weak_bg_fill = Color32::from_rgb(25, 76, 100);
                ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(23, 63, 91);
                ui.visuals_mut().widgets.active.bg_fill = Color32::from_rgb(23, 85, 115);
                inner(ui);
            });
        });
}

impl Calculator {
    /// Reference-style overview. All visible values come from the active model, never from demo data.
    pub(super) fn workspace_dashboard(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        if let Some(stats) = self.visible_stats() {
            let free_pulls = self.setup.available_flexible();
            let raw = optimal_allocation(
                &stats.character,
                &stats.weapon,
                self.setup.character_dedicated,
                self.setup.weapon_dedicated,
                free_pulls,
            );
            let chance = stats.feedback.as_ref().map(|f| f.probability(free_pulls).0)
                .unwrap_or(raw.success_probability);
            let expected = stats.feedback.as_ref().map(|f| f.average_raw)
                .unwrap_or(stats.combined.mean);
            let meta = if stats.feedback.is_some() { "환급 재투입 포함" } else { "정확 확률 DP" };
            if self.input_needs_calculation || self.calculating {
                ui.label(RichText::new("입력 반영 중 · 이전 계산 결과를 유지합니다.")
                    .color(YELLOW).size(10.0));
            }
            let (ticket_label, ticket_value, ticket_note) =
                if self.setup.game.shares_featured_ticket() {
                    ("별의 궤도 전용 티켓".to_owned(), self.setup.flexible.to_string(),
                        format!("캐릭터·광추 공용 · 성옥 {}뽑",
                            self.setup.premium_currency / self.setup.game.cost_per_pull()))
                } else {
                    (format!("캐릭터 / {} 전용권", self.setup.game.secondary_banner_name()),
                        format!("{} / {}", self.setup.character_dedicated,
                            self.setup.weapon_dedicated),
                        if self.setup.game == Game::StellaSora {
                            format!("골든 디스크 {}개", self.setup.weapon_ticket_inventory())
                        } else {
                            format!("공용 전환 {}뽑", free_pulls)
                        })
                };
            let metrics = [
                (ticket_label, ticket_value, ticket_note, CYAN),
                ("목표 달성 확률".to_owned(),
                    format!("{:.2}%", chance * 100.0), meta.to_owned(), SUCCESS),
                ("예상 필요 뽑기 수".to_owned(),
                    format!("{expected:.1}"), "목표까지의 평균 총 뽑기".to_owned(), PINK),
                ("보유 프리미엄 재화".to_owned(),
                    self.setup.premium_currency.to_string(),
                    format!("{}뽑 환산 · {}", self.setup.premium_currency / self.setup.game.cost_per_pull(),
                        self.setup.game.currency_name()), CYAN),
            ];
            let metric_columns = if ui.available_width() >= 760.0 { 4 }
                else if ui.available_width() >= 365.0 { 2 } else { 1 };
            for (row, chunk) in metrics.chunks(metric_columns).enumerate() {
                ui.columns(chunk.len(), |columns| {
                    for (col, (label, value, note, accent)) in chunk.iter().enumerate() {
                        overview_metric(&mut columns[col], row * metric_columns + col,
                            label, value.clone(), note.clone(), *accent);
                    }
                });
                if row + 1 < 4_usize.div_ceil(metric_columns) { ui.add_space(8.0); }
            }
            ui.add_space(13.0);
            let character_budget = self.setup.character_dedicated.saturating_add(free_pulls);
            let weapon_budget = self.setup.weapon_dedicated.saturating_add(free_pulls);
            let char_rate = stats.character.at(character_budget) * 100.0;
            let weapon_rate = stats.weapon.at(weapon_budget) * 100.0;
            let banner_name = self.setup.game.secondary_banner_name();
            let chart_char = [ChartSeries {
                label: "캐릭터", source: ChartSource::Exact(&stats.character), color: CYAN,
            }];
            let chart_weapon = [ChartSeries {
                label: banner_name, source: ChartSource::Exact(&stats.weapon), color: YELLOW,
            }];
            let chart_total = [ChartSeries {
                label: "전체 · 환급 전", source: ChartSource::Exact(&stats.combined), color: PINK,
            }];
            if ui.available_width() >= 850.0 {
                ui.columns(3, |columns| {
                    probability_card(&mut columns[0], "Character Probability",
                        format!("E{} Success: {:.2}%", self.target_eid, char_rate), &chart_char, CYAN, self.ui_preferences);
                    probability_card(&mut columns[1], &format!("{banner_name} Probability"),
                        format!("S{} Success: {:.2}%", self.target_refine, weapon_rate), &chart_weapon, YELLOW, self.ui_preferences);
                    probability_card(&mut columns[2], "Overall Result",
                        format!("Total Success: {:.2}%", chance * 100.0), &chart_total, PINK, self.ui_preferences);
                });
            } else if ui.available_width() >= 580.0 {
                ui.columns(2, |cols| {
                    probability_card(&mut cols[0], "캐릭터 획득 확률",
                        format!("E{} 성공 {:.2}%", self.target_eid, char_rate),
                        &chart_char, CYAN, self.ui_preferences);
                    probability_card(&mut cols[1], &format!("{banner_name} 획득 확률"),
                        format!("S{} 성공 {:.2}%", self.target_refine, weapon_rate),
                        &chart_weapon, YELLOW, self.ui_preferences);
                });
                ui.add_space(8.0);
                probability_card(ui, "전체 목표 달성 확률",
                    format!("전체 성공 {:.2}%", chance*100.0),
                    &chart_total, PINK, self.ui_preferences);
            } else {
                probability_card(ui, "캐릭터 획득 확률",
                    format!("E{} 성공 {:.2}%", self.target_eid, char_rate),
                    &chart_char, CYAN, self.ui_preferences);
                ui.add_space(8.0);
                probability_card(ui, &format!("{banner_name} 획득 확률"),
                    format!("S{} 성공 {:.2}%", self.target_refine, weapon_rate),
                    &chart_weapon, YELLOW, self.ui_preferences);
                ui.add_space(8.0);
                probability_card(ui, "전체 목표 달성 확률",
                    format!("전체 성공 {:.2}%", chance * 100.0),
                    &chart_total, PINK, self.ui_preferences);
            }

        } else if self.calculating {
            dashboard_card(ui, "Probability Overview", |ui| {
                ui.horizontal(|ui| { ui.spinner(); ui.label("게임별 확률과 환급을 계산하고 있습니다."); });
            });
        } else {
            dashboard_card(ui, "Probability Overview", |ui| {
                ui.colored_label(Color32::LIGHT_RED, self.error.as_deref().unwrap_or("계산 결과가 없습니다."));
            });
        }

        ui.add_space(11.0);
        dirty |= self.workspace_quick_settings(ui);
        dirty
    }

    /// The first screen repeats the input grouping and visual hierarchy of the supplied mockup.
    fn workspace_quick_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(14, 39, 58))
            .stroke(egui::Stroke::new(1.0_f32, BORDER))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(13)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("☷").color(CYAN).size(19.0));
                    ui.label(RichText::new("Simulation Settings").size(16.0).strong());
                    ui.add_space(19.0);
                    ui.label(RichText::new("Game").color(MUTED).size(12.0));
                    for (game, title) in [
                        (Game::StarRail, "✦  HSR"),
                        (Game::WutheringWaves, "◈  Wuthering Waves"),
                        (Game::StellaSora, "✧  Stella Sora"),
                        (Game::Neverness, "✦  Yihuan"),
                    ] {
                        let active = self.setup.game == game;
                        let text = RichText::new(title).size(11.0)
                            .color(if active { Color32::from_rgb(4, 29, 44) }
                                else { Color32::WHITE });
                        let b = egui::Button::new(text)
                            .fill(if active { CYAN } else { DEEP_CARD })
                            .stroke(egui::Stroke::new(1.0_f32,
                                if active { CYAN } else { BORDER }))
                            .corner_radius(8)
                            .min_size(egui::vec2(
                                if game == Game::WutheringWaves { 124.0 } else { 90.0 }, 32.0));
                        if ui.add(b).clicked() && !active {
                            self.change_game(game);
                            dirty = true;
                        }
                    }
                });
                ui.add_space(3.0);
                ui.separator();
                ui.add_space(3.0);
                let game = self.setup.game;
                if game.shares_featured_ticket() {
                    ui.columns(3, |cols| {
                        setting_tile(&mut cols[0], "◇", game.character_ticket_name(), |ui| {
                            dirty |= edit_ticket_inventory(ui, &mut self.setup, TicketRole::Shared);
                        });
                        setting_tile(&mut cols[1], "◆",
                            &format!("Premium Currency · {}", game.currency_name()), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                                    .range(0..=100_000_000).speed(1)).changed();
                            });
                        setting_tile(&mut cols[2], "◌",
                            &format!("Refund Currency · {}", game.exchange_points_name()), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.initial_points)
                                    .range(0..=1_000_000)).changed();
                            });
                    });
                } else {
                    ui.columns(4, |cols| {
                        setting_tile(&mut cols[0], "♧",
                            &format!("캐릭터 · {}", game.character_ticket_name()), |ui| {
                                dirty |= edit_ticket_inventory(
                                    ui, &mut self.setup, TicketRole::Character
                                );
                            });
                        setting_tile(&mut cols[1], "⚔",
                            &format!("{} · {}", game.secondary_banner_name(),
                                game.weapon_ticket_name()), |ui| {
                                dirty |= edit_ticket_inventory(
                                    ui, &mut self.setup, TicketRole::Weapon
                                );
                            });
                        setting_tile(&mut cols[2], "◆",
                            &format!("공용 재화 · {}", game.currency_name()), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                                    .range(0..=100_000_000).speed(1)).changed();
                            });
                        setting_tile(&mut cols[3], "◌",
                            &format!("환급 · {}", game.exchange_points_name()), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.initial_points)
                                    .range(0..=1_000_000)).changed();
                            });
                    });
                    if game == Game::StellaSora {
                        ui.label(RichText::new(format!(
                            "골든 디스크 300개 = 레코드 1뽑 · {}뽑 + 잔여 {}개",
                            self.setup.weapon_dedicated, self.setup.weapon_ticket_remainder
                        )).color(MUTED).size(10.0));
                    }
                }
                ui.add_space(8.0);
                ui.columns(4, |cols| {
                    setting_tile(&mut cols[0], "◉", "Target Character", |ui| {
                        self.catalog_selector(ui, catalog::Kind::Character);
                        egui::ComboBox::from_id_salt("workspace_character_goal")
                            .width(ui.available_width().min(145.0))
                            .selected_text(format!("E{} · {}장", self.target_eid, self.target_eid + 1))
                            .show_ui(ui, |ui| {
                                for level in 0..=6 {
                                    dirty |= ui.selectable_value(&mut self.target_eid, level,
                                        format!("E{level} / {}장", level + 1)).changed();
                                }
                            });
                    });
                    setting_tile(&mut cols[1], "⚔",
                        &format!("Target {}", self.setup.game.secondary_banner_name()), |ui| {
                            self.catalog_selector(ui, catalog::Kind::Weapon);
                            egui::ComboBox::from_id_salt("workspace_weapon_goal")
                                .width(ui.available_width().min(145.0))
                                .selected_text(if self.target_refine == 0 {
                                    "선택 안 함".to_owned()
                                } else {
                                    format!("S{} / {}장", self.target_refine, self.target_refine)
                                })
                                .show_ui(ui, |ui| {
                                    for level in 0..=5 {
                                        let label = if level == 0 { "필요 없음".to_owned() }
                                            else { format!("S{level} / {level}장") };
                                        dirty |= ui.selectable_value(
                                            &mut self.target_refine, level, label
                                        ).changed();
                                    }
                                });
                        });
                    setting_tile(&mut cols[2], "⚑", "Pity (Current)", |ui| {
                        let hard = match &self.setup.character.rule {
                            BannerRule::Standard { rarity, .. } => rarity.hard,
                            BannerRule::FeaturedCounter { hard_featured, .. } => *hard_featured,
                            BannerRule::NteArcBatch => 8,
                        };
                        dirty |= ui.add(egui::DragValue::new(&mut self.setup.character.pity)
                            .range(0..=hard.saturating_sub(1)).suffix("회")).changed();
                    });
                    setting_tile(&mut cols[3], "♧", "Rate-up Guarantee", |ui| {
                        if let BannerRule::Standard { featured_share, .. } = &self.setup.character.rule {
                            if *featured_share < 1.0 {
                                ui.horizontal(|ui| {
                                    let on = self.setup.character.guaranteed;
                                    for (value, label) in [(true, "On"), (false, "Off")] {
                                        let color = if on == value { CYAN } else { SURFACE };
                                        if ui.add(
                                            egui::Button::new(RichText::new(label).size(11.0)
                                                .color(if on == value { BG } else { MUTED }))
                                                .fill(color).corner_radius(6)
                                                .min_size(egui::vec2(44.0, 24.0))
                                        ).clicked() && on != value {
                                            self.setup.character.guaranteed = value;
                                            dirty = true;
                                        }
                                    }
                                });
                            } else {
                                ui.label(RichText::new("Always On").color(SUCCESS));
                            }
                        } else {
                            ui.label(RichText::new("게임별 별도 보장").color(MUTED).size(11.0));
                        }
                    });
                });
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("ⓘ  캐릭터·무기 목표와 환급 재투입을 계산합니다.\n     캐릭터/무기 단독 그래프는 자유권을 각각 전량 배정한 가정입니다.")
                        .color(MUTED).size(10.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("▶   Run Simulation")
                                .size(15.0).strong().color(BG))
                                .fill(CYAN).corner_radius(11)
                                .min_size(egui::vec2(206.0, 43.0))
                        ).clicked() {
                            self.manual_calculation_requested = true;
                        }
                        if ui.add(egui::Button::new("↻  Reset")
                            .fill(DEEP_CARD).corner_radius(9)
                            .min_size(egui::vec2(92.0, 40.0))).clicked() {
                            self.change_game(self.setup.game);
                            self.manual_calculation_requested = true;
                        }
                        if ui.button("상세 설정").clicked() {
                            self.view_mode = ViewMode::Settings;
                        }
                    });
                });
            });
        dirty
    }

    /// User-facing summary and session history, attached to the right edge on wide screens.
    pub(super) fn workspace_results_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("workspace_results")
            .resizable(false)
            .exact_width(313.0)
            .frame(egui::Frame::default().fill(BG).inner_margin(10))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(15, 42, 62))
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .corner_radius(egui::CornerRadius::same(13))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(283.0);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new("▥  Result Summary").size(15.0).strong());
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(5.0);
                        let available = self.setup.available_flexible();
                        let results = self.visible_stats().map(|stats| {
                            let raw = optimal_allocation(&stats.character, &stats.weapon,
                                self.setup.character_dedicated,
                                self.setup.weapon_dedicated, available);
                            let joint = stats.feedback.as_ref()
                                .map(|f| f.probability(available).0)
                                .unwrap_or(raw.success_probability);
                            (joint, stats.character.at(
                                self.setup.character_dedicated.saturating_add(available)),
                                stats.weapon.at(
                                    self.setup.weapon_dedicated.saturating_add(available)))
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("◌  Refund Loop").size(12.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let msg = if self.feedback_cfg.enabled {
                                    "반복 재투입 적용"
                                } else {
                                    "Off"
                                };
                                ui.label(RichText::new(msg).color(SUCCESS).size(11.0).strong());
                            });
                        });
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("◇  Extra Pulls from Refund").size(11.5));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let bonus = self.visible_stats()
                                    .and_then(|stats| stats.feedback.as_ref())
                                    .and_then(|f| f.successful_means(available));
                                let value = bonus.map_or("—".to_owned(),
                                    |means| format!("+{:.1}뽑", means.average_points_funded));
                                ui.label(RichText::new(value).color(SUCCESS).size(12.0).strong());
                            });
                        });
                        ui.add_space(7.0);
                        egui::Frame::default()
                            .fill(Color32::from_rgb(29, 65, 91))
                            .corner_radius(9)
                            .inner_margin(egui::Margin::symmetric(9, 6))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(RichText::new("★   Estimated Outcome")
                                    .size(13.0).strong());
                            });
                        ui.add_space(6.0);
                        for (title, value) in [
                            ("Target Character", results.map(|(_,ch,_)|
                                format!("E{} · {:.2}%", self.target_eid,ch*100.0))),
                            (&format!("Target {}", self.setup.game.secondary_banner_name()), results.map(|(_,_,wp)|
                                format!("S{} · {:.2}%", self.target_refine,wp*100.0))),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("☑").color(SUCCESS).size(13.0));
                                ui.label(RichText::new(title).size(11.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(value.as_deref().unwrap_or("계산 중"))
                                            .size(11.0).color(SUCCESS).strong());
                                    });
                            });
                            ui.add_space(3.0);
                        }
                        ui.add_space(4.0);
                        egui::Frame::default()
                            .fill(Color32::from_rgb(34, 101, 71))
                            .corner_radius(9)
                            .inner_margin(9)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!(
                                        "★ Overall (E{}S{})", self.target_eid,self.target_refine
                                    )).size(11.0).strong());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            let value = results.map(|(joint,_,_)|
                                                format!("{:.2}%", joint*100.0))
                                                .unwrap_or_else(|| "—".to_owned());
                                            ui.label(RichText::new(value)
                                                .size(17.0).color(YELLOW).strong());
                                        });
                                });
                            });
                    });
                ui.add_space(12.0);
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(14, 40, 60))
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .corner_radius(egui::CornerRadius::same(13))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_min_height(344.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("◷  Simulation History")
                                .size(15.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("⋮").color(MUTED).size(18.0));
                                });
                        });
                        ui.add_space(9.0);
                        ui.separator();
                        ui.add_space(7.0);
                        for i in 0..5 {
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(36.0, 36.0), egui::Sense::hover());
                                let tint = if self.history.get(i).is_some() {
                                    Color32::from_rgb(48, 80, 118)
                                } else {
                                    Color32::from_rgb(24, 52, 73)
                                };
                                ui.painter().rect_filled(rect, 9.0, tint);
                                ui.painter().circle_stroke(rect.center(), 9.0,
                                    egui::Stroke::new(1.2_f32, CYAN));
                                ui.painter().circle_filled(rect.center(), 3.0, Color32::WHITE);
                                if let Some(entry) = self.history.get(i) {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new(entry.game.label())
                                            .size(10.8).strong());
                                        ui.label(RichText::new(format!(
                                            "E{} · {} S{}", entry.eid,
                                            entry.game.secondary_banner_name(),entry.refine
                                        )).color(MUTED).size(9.5));
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(RichText::new(format!(
                                                "{:.2}%",entry.chance*100.0
                                            )).color(SUCCESS).size(11.0).strong());
                                        });
                                } else {
                                    ui.label(RichText::new("No simulation yet")
                                        .color(MUTED).size(10.0));
                                }
                            });
                            if i != 4 { ui.separator(); }
                        }
                        ui.add_space(7.0);
                        if ui.add(
                            egui::Button::new("▤  View All History   →")
                                .fill(Color32::from_rgb(31, 65, 89))
                                .corner_radius(9)
                                .min_size(egui::vec2(ui.available_width(), 34.0))
                        ).clicked() {
                            self.show_history = true;
                        }
                    });
            });
    }
}
