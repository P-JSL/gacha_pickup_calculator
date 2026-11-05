//! Reference-inspired Windows/Linux dashboard. Probability and refund engines stay unchanged.
use super::*;

const CYAN: Color32 = Color32::from_rgb(50, 218, 241);
const YELLOW: Color32 = Color32::from_rgb(244, 210, 75);
const PINK: Color32 = Color32::from_rgb(225, 112, 232);
const SUCCESS: Color32 = Color32::from_rgb(141, 238, 117);
const DEEP_CARD: Color32 = Color32::from_rgb(16, 43, 64);

fn dashboard_card(
    ui: &mut egui::Ui,
    title: &str,
    content: impl FnOnce(&mut egui::Ui),
) {
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

fn overview_metric(ui: &mut egui::Ui, symbol: &str, label: &str, value: String, note: String, color: Color32) {
    egui::Frame::group(ui.style())
        .fill(DEEP_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(symbol).size(18.0).color(color));
                ui.label(RichText::new(label).size(12.0).strong());
            });
            ui.add_space(6.0);
            ui.label(RichText::new(value).size(26.0).color(Color32::WHITE).strong());
            ui.add_space(5.0);
            ui.label(RichText::new(note).color(MUTED).size(10.5));
        });
}

fn probability_card(ui: &mut egui::Ui, title: &str, badge: String, source: &[ChartSeries<'_>], color: Color32) {
    dashboard_card(ui, title, |ui| {
        ui.label(RichText::new(badge).color(color).size(12.0).strong());
        ui.add_space(2.0);
        draw_probability_chart(ui, source);
    });
}

impl Calculator {
    /// Reference-style overview. All visible values come from the active model, never from demo data.
    pub(super) fn workspace_dashboard(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("Dashboard").size(25.0).strong());
                ui.label(RichText::new("뽑기 확률 · 재화 · 환급 시뮬레이션").color(MUTED).size(12.0));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("상세 분석  →").clicked() {
                    self.view_mode = ViewMode::Analytics;
                }
                ui.label(RichText::new(self.setup.game.label()).color(CYAN).size(12.0));
            });
        });
        ui.add_space(16.0);

        if let Some(stats) = &self.stats {
            let free_pulls = self.setup.available_flexible();
            let ticket_count = self.setup.character_dedicated
                .saturating_add(self.setup.weapon_dedicated)
                .saturating_add(free_pulls);
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
            ui.columns(4, |columns| {
                overview_metric(&mut columns[0], "◇", "Owned Pulls",
                    format!("{ticket_count}"),
                    format!("전용권 {} · 자유 {}",
                        self.setup.character_dedicated + self.setup.weapon_dedicated, free_pulls), CYAN);
                overview_metric(&mut columns[1], "◎", "Target Success Rate",
                    format!("{:.2}%", chance * 100.0), meta.to_owned(), SUCCESS);
                overview_metric(&mut columns[2], "⬡", "Expected Pulls",
                    format!("{expected:.1}"), "목표까지의 평균 총 뽑기".to_owned(), PINK);
                overview_metric(&mut columns[3], "◆", "Premium Currency",
                    self.setup.premium_currency.to_string(),
                    format!("{}뽑 환산 · {}", self.setup.premium_currency / self.setup.game.cost_per_pull(),
                        self.setup.game.currency_name()), CYAN);
            });
            ui.add_space(14.0);
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
            if ui.available_width() >= 720.0 {
                ui.columns(3, |columns| {
                    probability_card(&mut columns[0], "Character Probability",
                        format!("전량 단독 배분 시  {char_rate:.2}%"), &chart_char, CYAN);
                    probability_card(&mut columns[1], &format!("{banner_name} Probability"),
                        format!("전량 단독 배분 시  {weapon_rate:.2}%"), &chart_weapon, YELLOW);
                    probability_card(&mut columns[2], "Overall Result",
                        format!("현재 동시 달성  {:.2}%", chance * 100.0), &chart_total, PINK);
                });
            } else {
                probability_card(ui, "Character Probability",
                    format!("단독 배분 시 {char_rate:.2}%"), &chart_char, CYAN);
                ui.add_space(8.0);
                probability_card(ui, &format!("{banner_name} Probability"),
                    format!("단독 배분 시 {weapon_rate:.2}%"), &chart_weapon, YELLOW);
                ui.add_space(8.0);
                probability_card(ui, "Overall Result",
                    format!("현재 동시 달성 {:.2}%", chance * 100.0), &chart_total, PINK);
            }
            ui.add_space(3.0);
            ui.label(RichText::new("캐릭터/무기 개별 수치는 각각 자유 재화를 전량 배분한 가정입니다. 두 수치를 곱한 값은 동시 달성 확률이 아닙니다.")
                .color(MUTED).size(10.0));
        } else if self.calculating {
            dashboard_card(ui, "Probability Overview", |ui| {
                ui.horizontal(|ui| { ui.spinner(); ui.label("게임별 확률과 환급을 계산하고 있습니다."); });
            });
        } else {
            dashboard_card(ui, "Probability Overview", |ui| {
                ui.colored_label(Color32::LIGHT_RED, self.error.as_deref().unwrap_or("계산 결과가 없습니다."));
            });
        }

        ui.add_space(16.0);
        dirty |= self.workspace_quick_settings(ui);
        dirty
    }

    fn workspace_quick_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        dashboard_card(ui, "⚙  Simulation Settings", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Game").color(MUTED).size(11.0));
                for (game, short) in [
                    (Game::StarRail, "✦  HSR"),
                    (Game::WutheringWaves, "◈  Wuthering Waves"),
                    (Game::StellaSora, "✧  Stella Sora"),
                    (Game::Neverness, "✺  Yihuan"),
                ] {
                    if ui.add(
                        egui::Button::new(RichText::new(short).size(12.0))
                            .selected(self.setup.game == game)
                            .corner_radius(9)
                    ).clicked() && self.setup.game != game {
                        self.change_game(game);
                        dirty = true;
                    }
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.columns(3, |cols| {
                cols[0].label(RichText::new("Owned Tickets · 자유 배분권").color(MUTED).size(11.0));
                dirty |= cols[0].add(
                    egui::DragValue::new(&mut self.setup.flexible).range(0..=100_000).suffix("뽑")
                ).changed();
                cols[1].label(RichText::new(format!("Premium Currency · {}", self.setup.game.currency_name()))
                    .color(MUTED).size(11.0));
                dirty |= cols[1].add(
                    egui::DragValue::new(&mut self.setup.premium_currency)
                        .range(0..=100_000_000).speed(1)
                ).changed();
                cols[2].label(RichText::new(format!("Refund Currency · {}", self.setup.game.exchange_points_name()))
                    .color(MUTED).size(11.0));
                dirty |= cols[2].add(
                    egui::DragValue::new(&mut self.feedback_cfg.initial_points)
                        .range(0..=1_000_000)
                ).changed();
            });
            ui.add_space(9.0);
            ui.columns(4, |cols| {
                cols[0].label(RichText::new("Target Character").color(MUTED).size(11.0));
                egui::ComboBox::from_id_salt("workspace_character")
                    .width(cols[0].available_width().min(155.0))
                    .selected_text(format!("E{} / {}장", self.target_eid, self.target_eid + 1))
                    .show_ui(&mut cols[0], |ui| {
                        for level in 0..=6 {
                            dirty |= ui.selectable_value(&mut self.target_eid, level,
                                format!("E{level} / {}장", level + 1)).changed();
                        }
                    });
                cols[1].label(RichText::new(format!("Target {}", self.setup.game.secondary_banner_name()))
                    .color(MUTED).size(11.0));
                egui::ComboBox::from_id_salt("workspace_weapon")
                    .width(cols[1].available_width().min(155.0))
                    .selected_text(if self.target_refine == 0 { "선택 안 함".to_owned() }
                        else { format!("S{} / {}장", self.target_refine, self.target_refine) })
                    .show_ui(&mut cols[1], |ui| {
                        for level in 0..=5 {
                            let name = if level == 0 { "필요 없음".to_owned() }
                                else { format!("S{level} / {level}장") };
                            dirty |= ui.selectable_value(&mut self.target_refine, level, name).changed();
                        }
                    });
                cols[2].label(RichText::new("Character Pity").color(MUTED).size(11.0));
                let hard = match &self.setup.character.rule {
                    BannerRule::Standard { rarity, .. } => rarity.hard,
                    BannerRule::FeaturedCounter { hard_featured, .. } => *hard_featured,
                    BannerRule::NteArcBatch => 8,
                };
                dirty |= cols[2].add(
                    egui::DragValue::new(&mut self.setup.character.pity)
                        .range(0..=hard.saturating_sub(1)).suffix("회")
                ).changed();
                cols[3].label(RichText::new("Rate-up Guarantee").color(MUTED).size(11.0));
                if let BannerRule::Standard { featured_share, .. } = &self.setup.character.rule {
                    if *featured_share < 1.0 {
                        dirty |= cols[3].checkbox(&mut self.setup.character.guaranteed, "픽업 확정").changed();
                    } else {
                        cols[3].label(RichText::new("항상 픽업 확정").color(SUCCESS).size(11.0));
                    }
                } else {
                    cols[3].label(RichText::new("별도 규칙 적용").color(MUTED).size(11.0));
                }
            });
            ui.add_space(11.0);
            ui.separator();
            ui.add_space(7.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!(
                    "전용권·무기 천장·환급 상세 조건은 시뮬레이션 설정에서 편집 · 교환 {}개당 1뽑",
                    self.feedback_cfg.points_per_ticket
                )).size(10.5).color(MUTED));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(RichText::new("▶  Run Simulation").strong())
                        .fill(Color32::from_rgb(28, 143, 197))
                        .min_size(egui::vec2(146.0, 33.0))
                        .corner_radius(10)).clicked() {
                        dirty = true;
                    }
                    if ui.button("상세 설정  →").clicked() {
                        self.view_mode = ViewMode::Settings;
                    }
                    if ui.button("↻  Reset").clicked() {
                        let game = self.setup.game;
                        self.change_game(game);
                        dirty = true;
                    }
                });
            });
        });
        dirty
    }

    /// Session-only run history; it is intentionally not presented as saved/persistent history.
    pub(super) fn workspace_results_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("workspace_results")
            .resizable(false)
            .exact_width(280.0)
            .frame(egui::Frame::default().fill(BG).inner_margin(10))
            .show(ctx, |ui| {
                ui.add_space(7.0);
                dashboard_card(ui, "▥  Result Summary", |ui| {
                    let available = self.setup.available_flexible();
                    if let Some(stats) = &self.stats {
                        let raw = optimal_allocation(&stats.character, &stats.weapon,
                            self.setup.character_dedicated,
                            self.setup.weapon_dedicated, available);
                        let rate = stats.feedback.as_ref()
                            .map(|f| f.probability(available).0)
                            .unwrap_or(raw.success_probability);
                        ui.label(RichText::new("현재 보유권 기준 동시 달성").size(11.0).color(MUTED));
                        ui.label(RichText::new(format!("{:.2}%", rate * 100.0))
                            .size(26.0).color(SUCCESS).strong());
                        ui.separator();
                        let char_only = stats.character.at(
                            self.setup.character_dedicated.saturating_add(available));
                        let weapon_only = stats.weapon.at(
                            self.setup.weapon_dedicated.saturating_add(available));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("캐릭터 단독 배분 시").color(MUTED).size(11.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("{:.2}%", char_only * 100.0))
                                    .color(CYAN).strong());
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} 단독 배분 시", self.setup.game.secondary_banner_name()))
                                .color(MUTED).size(11.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("{:.2}%", weapon_only * 100.0))
                                    .color(YELLOW).strong());
                            });
                        });
                        ui.add_space(7.0);
                        ui.separator();
                        ui.label(RichText::new("Refund Loop · 환급 재투입").color(MUTED).size(11.0));
                        if let Some(ref feedback) = stats.feedback {
                            if let Some(means) = feedback.successful_means(available) {
                                ui.label(RichText::new(format!("성공 경로 평균 +{:.1}뽑",
                                    means.average_points_funded)).color(SUCCESS).strong());
                            } else {
                                ui.label(RichText::new("현재 예산 내 성공 표본 없음")
                                    .color(MUTED).size(11.0));
                            }
                            ui.label(RichText::new(format!("전체 표본 평균 획득 재화 {:.1}{}",
                                feedback.total_returned_points_average,
                                self.setup.game.exchange_points_name()))
                                .color(MUTED).size(10.0));
                        } else {
                            ui.label(RichText::new("환급 시뮬레이션 미적용")
                                .color(MUTED).size(11.0));
                        }
                    } else {
                        ui.horizontal(|ui| {
                            if self.calculating { ui.spinner(); }
                            ui.label("계산 결과 대기 중");
                        });
                    }
                });
                ui.add_space(12.0);
                dashboard_card(ui, "★  Estimated Outcome", |ui| {
                    ui.label(RichText::new(format!(
                        "캐릭터 E{} + {} S{}",
                        self.target_eid, self.setup.game.secondary_banner_name(),
                        self.target_refine
                    )).strong().size(12.0));
                    ui.add_space(5.0);
                    let active = if self.feedback_cfg.enabled {
                        "환급 재투입 모델 활성화"
                    } else {
                        "환급 미반영 확률"
                    };
                    ui.label(RichText::new(active).color(SUCCESS).size(11.0));
                    ui.label(RichText::new(format!(
                        "자유 재화 {}뽑 · 캐릭터 전용 {} · {} 전용 {}",
                        self.setup.available_flexible(),
                        self.setup.character_dedicated,
                        self.setup.game.secondary_banner_name(),
                        self.setup.weapon_dedicated
                    )).color(MUTED).size(10.0));
                    ui.add_space(8.0);
                    if ui.button("상세 결과 및 분위수  →").clicked() {
                        self.view_mode = ViewMode::Analytics;
                    }
                });
                ui.add_space(12.0);
                dashboard_card(ui, "◷  Simulation History", |ui| {
                    if self.history.is_empty() {
                        ui.label(RichText::new("이번 실행의 계산 기록이 없습니다.").color(MUTED).size(11.0));
                    }
                    for item in self.history.iter().take(5) {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(item.game.label()).size(11.0).strong());
                                ui.label(RichText::new(format!(
                                    "E{} · {} S{}", item.eid,
                                    item.game.secondary_banner_name(), item.refine
                                )).color(MUTED).size(10.0));
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(format!("{:.2}%", item.chance * 100.0))
                                    .color(SUCCESS).strong());
                            });
                        });
                        ui.separator();
                    }
                    ui.label(RichText::new("현재 실행 중 계산 기록 · 프로그램 종료 시 초기화")
                        .color(MUTED).size(10.0));
                });
            });
    }
}
