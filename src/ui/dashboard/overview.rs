use super::*;

impl Calculator {
    /// Reference-style overview. All visible values come from the active model, never from demo data.
    pub(crate) fn workspace_dashboard(&mut self, ui: &mut egui::Ui) -> bool {
        let language = self.language();
        let mut dirty = false;
        if let Some(stats) = self.visible_stats() {
            let free_pulls = self.setup.available_flexible();
            let funding = self.current_goal_funding();
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
            let meta = if stats.feedback.is_some() { t(language, "Including recycled refunds") } else { t(language, "Accurate probability DP") };
            if self.input_needs_calculation && !self.calculating {
                ui.label(RichText::new(t(language, "Changes pending. Select Run Simulation to update."))
                    .color(YELLOW).size(10.0));
            } else if self.calculating {
                ui.label(RichText::new(t(language, "Calculating in background. Previous results remain visible."))
                    .color(YELLOW).size(10.0));
            }
            ui.columns(4, |columns| {
                let (label, value, detail) = if self.setup.game.shares_featured_ticket() {
                    (i18n::ticket_name(language, self.setup.game, false), format!("{}", self.setup.flexible),
                        format!("{}: {} ({})", t(language, "Shared character/weapon ticket"),
                            self.setup.premium_currency / self.setup.game.cost_per_pull(),
                            i18n::currency_name(language, self.setup.game)))
                } else {
                    (t(language, "Character / Weapon Tickets"),
                        format!("{} / {}", self.setup.character_dedicated,
                            self.setup.weapon_dedicated),
                        if self.setup.game == Game::StellaSora {
                            format!("{} / {}: {}",
                                i18n::ticket_name(language, self.setup.game, false),
                                i18n::ticket_name(language, self.setup.game, true),
                                self.setup.weapon_ticket_inventory())
                        } else {
                            format!("{}: {}", t(language, "Shared Currency"), free_pulls)
                        })
                };
                overview_metric(&mut columns[0], 0, label, value, detail, CYAN);
                overview_metric(&mut columns[1], 1, t(language, "Target Success Rate"),
                    format!("{:.2}%", chance * 100.0), meta.to_owned(), SUCCESS);
                overview_metric(&mut columns[2], 2, t(language, "Expected Pulls"),
                    format!("{expected:.1}"), t(language, "Average pulls to all targets").to_owned(), PINK);
                let (value, note, color) = funding.map_or_else(
                    || ("—".to_owned(), t(language, "Recalculate to update funding estimate").to_owned(), MUTED),
                    |g| (format!("+{} {}", g.additional_shared,
                                 t(language, "Featured ticket equivalents")),
                          format!("+{} {} · {} {}%",
                                  g.additional_currency, i18n::currency_name(language, self.setup.game),
                                  t(language, "Target confidence"), g.confidence),
                          if g.additional_shared == 0 { SUCCESS } else { GOLD }),
                );
                overview_metric(&mut columns[3], 3, t(language, "Additional featured ticket equivalents"),
                    value, note, color);
            });
            ui.add_space(9.0);
            dashboard_card(ui, t(language, "Funding Gap"), |ui| {
                if let Some(gap) = funding {
                    render_goal_funding(ui, &gap, &self.setup, language);
                } else {
                    ui.label(RichText::new(t(language,
                        "Recalculate to update funding estimate")).color(GOLD));
                }
            });
            ui.add_space(13.0);
            let character_budget = self.setup.character_dedicated.saturating_add(free_pulls);
            let weapon_budget = self.setup.weapon_dedicated.saturating_add(free_pulls);
            let char_rate = stats.character.at(character_budget) * 100.0;
            let weapon_rate = stats.weapon.at(weapon_budget) * 100.0;
            let banner_name = i18n::banner_name(language, self.setup.game);
            let chart_char = [ChartSeries {
                label: t(language, "Characters"), source: ChartSource::Exact(&stats.character), color: CYAN,
            }];
            let chart_weapon = [ChartSeries {
                label: banner_name, source: ChartSource::Exact(&stats.weapon), color: YELLOW,
            }];
            let chart_total = [ChartSeries {
                label: t(language, "Overall"), source: ChartSource::Exact(&stats.combined), color: PINK,
            }];
            if ui.available_width() >= 900.0 {
                ui.columns(3, |columns| {
                    probability_card(&mut columns[0], t(language, "Character Probability"),
                        format!("E{} {}: {:.2}%", self.target_eid, t(language, "Success"), char_rate), &chart_char, CYAN, self.ui_preferences, self.language());
                    probability_card(&mut columns[1], &format!("{banner_name} {}", t(language, "Probability")),
                        format!("S{} {}: {:.2}%", self.target_refine, t(language, "Success"), weapon_rate), &chart_weapon, YELLOW, self.ui_preferences, self.language());
                    probability_card(&mut columns[2], t(language, "Overall Result"),
                        format!("{}: {:.2}%", t(language, "Total Success"), chance * 100.0), &chart_total, PINK, self.ui_preferences, self.language());
                });
            } else {
                probability_card(ui, t(language, "Character Probability"),
                    format!("{}: {char_rate:.2}%", t(language, "Success")), &chart_char, CYAN, self.ui_preferences, self.language());
                ui.add_space(8.0);
                probability_card(ui, &format!("{banner_name} {}", t(language, "Probability")),
                    format!("{}: {weapon_rate:.2}%", t(language, "Success")), &chart_weapon, YELLOW, self.ui_preferences, self.language());
                ui.add_space(8.0);
                probability_card(ui, t(language, "Overall Result"),
                    format!("{}: {:.2}%", t(language, "Total Success"), chance * 100.0), &chart_total, PINK, self.ui_preferences, self.language());
            }

        } else if self.calculating {
            dashboard_card(ui, t(language, "Probability Overview"), |ui| {
                ui.horizontal(|ui| { ui.spinner(); ui.label(t(language, "Calculating probability and refunds...")); });
            });
        } else {
            dashboard_card(ui, t(language, "Probability Overview"), |ui| {
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::LIGHT_RED, error);
                } else {
                    ui.label(RichText::new(t(language,
                        "Changes pending. Select Run Simulation to update.")).color(YELLOW));
                }
            });
        }

        ui.add_space(11.0);
        dirty |= self.workspace_quick_settings(ui);
        if let Some(stats) = self.visible_stats() {
            if let Some(strategies) = &stats.wuwa_strategies {
                ui.add_space(10.0);
                wuwa_strategy_table(ui, strategies, &self.setup, self.confidence);
            }
        }
        dirty
    }


}
