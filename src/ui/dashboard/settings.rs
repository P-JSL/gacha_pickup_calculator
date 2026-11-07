use super::*;

impl Calculator {
    /// The first screen repeats the input grouping and visual hierarchy of the supplied mockup.
    pub(crate) fn workspace_quick_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let language = self.language();
        let mut dirty = false;
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(14, 39, 58))
            .stroke(egui::Stroke::new(1.0_f32, BORDER))
            .corner_radius(egui::CornerRadius::same(14))
            .inner_margin(13)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("☷").color(CYAN).size(19.0));
                    ui.label(RichText::new(t(language, "Simulation Settings")).size(16.0).strong());
                    ui.add_space(19.0);
                    ui.label(RichText::new(t(language, "Game")).color(MUTED).size(12.0));
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
                        setting_tile(&mut cols[0], "◇", i18n::ticket_name(language, game, false), |ui| {
                            dirty |= edit_ticket_inventory(ui, &mut self.setup, TicketRole::Shared);
                        });
                        setting_tile(&mut cols[1], "◆",
                            &format!("{} · {}", t(language, "Premium Currency"), i18n::currency_name(language, game)), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                                    .range(0..=100_000_000).speed(1)).changed();
                            });
                        setting_tile(&mut cols[2], "◌",
                            &format!("{} · {}", t(language, "Refund Currency"), i18n::refund_name(language, game)), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.initial_points)
                                    .range(0..=1_000_000)).changed();
                            });
                    });
                } else {
                    ui.columns(2, |cols| {
                        setting_tile(&mut cols[0], "♧",
                            &format!("{} · {}", t(language, "Characters"), i18n::ticket_name(language, game, false)), |ui| {
                                dirty |= edit_ticket_inventory(
                                    ui, &mut self.setup, TicketRole::Character
                                );
                            });
                        setting_tile(&mut cols[1], "⚔",
                            &format!("{} · {}", i18n::banner_name(language, game),
                                i18n::ticket_name(language, game, true)), |ui| {
                                dirty |= edit_ticket_inventory(
                                    ui, &mut self.setup, TicketRole::Weapon
                                );
                            });
                    });
                    ui.add_space(5.0);
                    ui.columns(2, |cols| {
                        setting_tile(&mut cols[0], "◆",
                            &format!("{} · {}", t(language, "Shared Currency"), i18n::currency_name(language, game)), |ui| {
                                dirty |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                                    .range(0..=100_000_000).speed(1)).changed();
                            });
                        setting_tile(&mut cols[1], "◌",
                            &format!("{} · {}", t(language, "Refund Currency"), i18n::refund_name(language, game)), |ui| {
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
                // Two rows prevent long selections and number inputs from
                // overflowing their peers at normal Windows window sizes.
                ui.columns(2, |cols| {
                    setting_tile(&mut cols[0], "◉", t(language, "Target Character"), |ui| {
                        self.catalog_selector(ui, catalog::Kind::Character);
                        egui::ComboBox::from_id_salt("workspace_character_goal")
                            .width(ui.available_width().min(145.0))
                            .selected_text(format!("E{} / {}", self.target_eid, self.target_eid + 1))
                            .show_ui(ui, |ui| {
                                for level in 0..self.setup.game.max_character_copies() {
                                    dirty |= ui.selectable_value(&mut self.target_eid, level,
                                        format!("E{level} / {}", level + 1)).changed();
                                }
                            });
                        ui.horizontal(|ui| {
                            ui.label(t(language, "Owned copies"));
                            dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_owned)
                                .range(0..=self.setup.game.max_character_copies()).suffix(t(language, "Copies unit"))).changed();
                        });
                    });
                    setting_tile(&mut cols[1], "⚔",
                        &format!("{} · {}", t(language, "Target Weapon"), i18n::banner_name(language, self.setup.game)), |ui| {
                            self.catalog_selector(ui, catalog::Kind::Weapon);
                            egui::ComboBox::from_id_salt("workspace_weapon_goal")
                                .width(ui.available_width().min(145.0))
                                .selected_text(if self.target_refine == 0 {
                                    t(language, "No target").to_owned()
                                } else {
                                    format!("S{} / {}", self.target_refine, self.target_refine)
                                })
                                .show_ui(ui, |ui| {
                                    for level in 0..=self.setup.game.max_weapon_copies() {
                                        let label = if level == 0 { t(language, "Not needed").to_owned() }
                                            else { format!("S{level} / {level}") };
                                        dirty |= ui.selectable_value(
                                            &mut self.target_refine, level, label
                                        ).changed();
                                    }
                                });
                            ui.horizontal(|ui| {
                                ui.label(t(language, "Owned copies"));
                                dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_owned)
                                    .range(0..=self.setup.game.max_weapon_copies()).suffix(t(language, "Copies unit"))).changed();
                            });
                        });
                });
                ui.add_space(5.0);
                ui.columns(2, |cols| {
                    setting_tile(&mut cols[0], "⚑", t(language, "Pity (Current)"), |ui| {
                        let hard = match &self.setup.character.rule {
                            BannerRule::Standard { rarity, .. } => rarity.hard,
                            BannerRule::FeaturedCounter { hard_featured, .. } => *hard_featured,
                            BannerRule::NteArcBatch => 8,
                        };
                        dirty |= ui.add(egui::DragValue::new(&mut self.setup.character.pity)
                            .range(0..=hard.saturating_sub(1))).changed();
                    });
                    setting_tile(&mut cols[1], "♧", t(language, "Rate-up Guarantee"), |ui| {
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
                                ui.label(RichText::new(t(language, "Always On")).color(SUCCESS));
                            }
                        } else {
                            ui.label(RichText::new(t(language, "Use game-specific guarantee")).color(MUTED).size(11.0));
                        }
                    });
                });
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);
                // Buttons have their own row. A long hint can no longer consume
                // width from the primary action or paint outside the center panel.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let run = ui.add_enabled(!self.calculating, egui::Button::new(
                            RichText::new(format!("▶  {}", t(language, "Run Simulation"))).size(14.0).strong().color(BG))
                            .fill(CYAN).corner_radius(11)
                            .min_size(egui::vec2(175.0, 40.0)));
                        if self.ci_capture {
                            // Report the *real* egui button geometry, including its
                            // clip visibility. CI still has to click this widget.
                            let center = run.rect.center() * ui.ctx().pixels_per_point();
                            let visible = ui.clip_rect().contains(run.rect.center());
                            eprintln!(
                                "GACHA_CI_RUN_BUTTON game={:?} x={} y={} visible={}",
                                self.setup.game, center.x.round() as i32,
                                center.y.round() as i32, visible,
                            );
                        }
                        if run.clicked() {
                            self.manual_calculation_requested = true;
                        }
                        if ui.add(egui::Button::new(format!("↻  {}", t(language, "Reset")))
                            .fill(DEEP_CARD).corner_radius(9)
                            .min_size(egui::vec2(86.0, 38.0))).clicked() {
                            self.change_game(self.setup.game);
                            dirty = true;
                        }
                        if ui.button(t(language, "Advanced settings")).clicked() {
                            self.view_mode = ViewMode::Settings;
                        }
                    });
                });
                ui.label(RichText::new(
                    format!("ⓘ  {} {}", t(language, "Target settings and refund recycling are shared across tabs."),
                        t(language, "Individual curves assume full allocation of flexible tickets.")))
                    .color(MUTED).size(10.0));
            });
        dirty
    }


}
