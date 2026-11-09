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
            let chance = funding.map_or_else(|| stats.feedback.as_ref()
                .map_or(raw.success_probability, |f| f.probability(free_pulls).0),
                |gap| gap.current_probability);
            let expected = stats.feedback.as_ref().map_or(stats.combined.mean, |f| f.average_raw);
            let meta = funding.map_or_else(
                || if stats.feedback.is_some() {
                    t(language, "Including recycled refunds").to_owned()
                } else {
                    t(language, "Accurate probability DP").to_owned()
                },
                |gap| if gap.uses_refunds {
                    format!("환급 포함 {:.2}% · 환급 전(참고) {:.2}%",
                        gap.current_probability * 100.0, gap.raw_probability * 100.0)
                } else { t(language, "Accurate probability DP").to_owned() },
            );
            let mean_note = stats.feedback.as_ref().map_or_else(
                || t(language, "Average pulls to all targets").to_owned(),
                |f| format!("총 시도 평균 · 환급 후 최초 자유권 평균 {:.1}장",
                    f.average_initial_flexible),
            );
            if self.input_needs_calculation && !self.calculating {
                ui.label(RichText::new(t(language, "Changes pending. Select Run Simulation to update."))
                    .color(YELLOW).size(10.0));
            } else if self.calculating {
                ui.label(RichText::new(t(language, "Calculating in background. Previous results remain visible."))
                    .color(YELLOW).size(10.0));
            }
            let (ticket_label, ticket_value, ticket_note) = if self.setup.game.shares_featured_ticket() {
                (i18n::ticket_name(language, self.setup.game, false),
                 format!("{}", self.setup.flexible),
                 format!("{}: {} ({})", t(language, "Shared character/weapon ticket"),
                     self.setup.premium_currency / self.setup.game.cost_per_pull(),
                     i18n::currency_name(language, self.setup.game)))
            } else {
                (t(language, "Character / Weapon Tickets"),
                 format!("{} / {}", self.setup.character_dedicated, self.setup.weapon_dedicated),
                 format!("{}: {}", t(language, "Shared Currency"), free_pulls))
            };
            let (gap_value, gap_note, gap_color) = funding.map_or_else(
                || ("—".to_owned(), t(language, "Recalculate to update funding estimate").to_owned(), MUTED),
                |g| (format!("+{} {}", g.additional_shared,
                              t(language, "Featured ticket equivalents")),
                      format!("+{} {} · {} {}%", g.additional_currency,
                              i18n::currency_name(language, self.setup.game),
                              t(language, "Target confidence"), g.confidence),
                      if g.additional_shared == 0 { SUCCESS } else { GOLD }),
            );
            let cards = [
                (0, ticket_label, ticket_value, ticket_note, CYAN),
                (1, t(language, "Target Success Rate"), format!("{:.2}%", chance * 100.0), meta, SUCCESS),
                (2, t(language, "Expected Pulls"), format!("{expected:.1}"), mean_note, PINK),
                (3, t(language, "Additional featured ticket equivalents"), gap_value, gap_note, gap_color),
            ];
            let width = ui.available_width();
            if width >= 1120.0 {
                ui.columns(4, |columns| {
                    for (i, (kind, label, value, note, color)) in cards.iter().enumerate() {
                        overview_metric(&mut columns[i], *kind, label, value.clone(), note.clone(), *color);
                    }
                });
            } else if width >= 560.0 {
                for pair in cards.chunks(2) {
                    ui.columns(2, |columns| {
                        for (i, (kind, label, value, note, color)) in pair.iter().enumerate() {
                            overview_metric(&mut columns[i], *kind, label, value.clone(), note.clone(), *color);
                        }
                    });
                    ui.add_space(5.0);
                }
            } else {
                for (kind, label, value, note, color) in &cards {
                    overview_metric(ui, *kind, label, value.clone(), note.clone(), *color);
                    ui.add_space(5.0);
                }
            }

            // Make the refund-recycled value the visible primary result,
            // including in the compact chart where hover labels were clipped.
            let overall_success_label = if stats.feedback.is_some() {
                match language {
                    Language::Korean => "환급 포함 전체 성공",
                    Language::English => "Overall success with refunds",
                    Language::Japanese => "還元込み全体成功",
                    Language::ChineseSimplified => "含返还的整体成功率",
                    Language::ChineseTraditional => "含返還的整體成功率",
                }
            } else { t(language, "Total Success") };
            // The overall chart and the success-rate KPI now both use shared
            // initial tickets; the old total-pulls curve had a different X-axis
            // and could falsely suggest refunds were not being applied.
            let mut chart_total = vec![ChartSeries {
                label: "환급 전", source: ChartSource::Exact(&stats.raw_shared), color: MUTED,
            }];
            if let Some(refund) = stats.feedback.as_ref() {
                chart_total.push(ChartSeries {
                    label: "환급 재투입 후", source: ChartSource::Refund(refund), color: PINK,
                });
            }
            // The refund-inclusive joint result is immediately below the
            // summary KPIs, before long input and funding details. Retain
            // its full width so the graph and probability tooltip remain
            // readable at all supported desktop sizes.
            ui.add_space(9.0);
            probability_card(ui, t(language, "Overall Result"),
                format!("{overall_success_label}: {:.2}%", chance * 100.0),
                &chart_total, PINK, self.ui_preferences, language);


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
        // Preserve the game tabs and Run Simulation button ahead of detailed
        // results, even when the center panel is narrow or scaled on Windows.
        if !dirty && !self.input_needs_calculation && !self.calculating {
            if let Some(stats) = self.visible_stats() {
                if let (Some(stage), Some(refund)) =
                    (stats.budget_breakdown.as_ref(), stats.feedback.as_ref()) {
                    ui.add_space(9.0);
                    refund_stages_card(ui, stage, self.setup.available_flexible(),
                        refund.average_raw,
                        refund.total_returned_points_average
                            / self.feedback_cfg.points_per_ticket.max(1) as f64,
                        language);
                }
            }
        }
        // Keep the game controls ahead of the standalone banner charts;
        // the joint refunded probability is now visible above this section.
        if let Some(stats) = self.visible_stats() {
            // Keep game tabs and Run Simulation reachable even when results
            // are present: the long funding explanation follows the input
            // controls instead of displacing them below the viewport.
            ui.add_space(9.0);
            dashboard_card(ui, t(language, "Funding Gap"), |ui| {
                if let Some(gap) = self.current_goal_funding() {
                    render_goal_funding(ui, &gap, &self.setup, language);
                } else {
                    ui.label(RichText::new(t(language,
                        "Recalculate to update funding estimate")).color(GOLD));
                }
            });
            let free_pulls = self.setup.available_flexible();
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
            if ui.available_width() >= 900.0 {
                ui.columns(2, |columns| {
                    probability_card(&mut columns[0], t(language, "Character Probability"),
                        format!("E{} {}: {:.2}%", self.target_eid, t(language, "Success"), char_rate), &chart_char, CYAN, self.ui_preferences, self.language());
                    probability_card(&mut columns[1], &format!("{banner_name} {}", t(language, "Probability")),
                        format!("S{} {}: {:.2}%", self.setup.game.weapon_upgrade_level(self.target_refine), t(language, "Success"), weapon_rate), &chart_weapon, YELLOW, self.ui_preferences, self.language());
                });
            } else {
                probability_card(ui, t(language, "Character Probability"),
                    format!("{}: {char_rate:.2}%", t(language, "Success")), &chart_char, CYAN, self.ui_preferences, self.language());
                ui.add_space(8.0);
                probability_card(ui, &format!("{banner_name} {}", t(language, "Probability")),
                    format!("{}: {weapon_rate:.2}%", t(language, "Success")), &chart_weapon, YELLOW, self.ui_preferences, self.language());
            }

        }
        if let Some(stats) = self.visible_stats() {
            if let Some(strategies) = &stats.wuwa_strategies {
                ui.add_space(10.0);
                wuwa_strategy_table(ui, strategies, &self.setup, self.confidence);
            }
        }
        dirty
    }


}
