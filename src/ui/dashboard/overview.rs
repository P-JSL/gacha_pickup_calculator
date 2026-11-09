use super::*;

impl Calculator {
    /// Reference-style overview. All visible values come from the active model, never from demo data.
    pub(crate) fn workspace_dashboard(&mut self, ui: &mut egui::Ui) -> bool {
        let language = self.language();
        let mut dirty = false;
        if let Some(stats) = self.visible_stats()
            .filter(|_| !self.input_needs_calculation && !self.calculating) {
            let free_pulls = self.setup.available_flexible();
            let funding = self.current_goal_funding();
            // The final success rate is taken from the shared-ticket funding CDF.
            let chance = funding.map_or_else(
                || stats.feedback.as_ref().map_or(stats.raw_shared.at(free_pulls),
                     |f| f.probability(free_pulls).0),
                |gap| gap.current_probability,
            );
            let expected = stats.feedback.as_ref().map_or(stats.combined.mean, |f| f.average_raw);
            let mean_note = stats.feedback.as_ref().map_or_else(
                || t(language, "Average pulls to all targets").to_owned(),
                |f| format!("총 시도 평균 · 환급 후 최초 자유권 평균 {:.1}장",
                    f.average_initial_flexible),
            );
            final_success_hero(ui, self.setup.game, self.target_eid,
                self.setup.game.weapon_upgrade_level(self.target_refine),
                chance, funding.map(|gap| gap.raw_probability),
                stats.feedback.as_ref().map(|estimate| estimate.scenarios.len()),
                free_pulls, language);
            ui.add_space(7.0);
            let (return_value, return_note) = if let Some(refund) = stats.feedback.as_ref() {
                (format!("{:.1}", refund.total_returned_points_average
                    / self.feedback_cfg.points_per_ticket.max(1) as f64),
                 match language {
                     Language::Korean => "목표 달성 후 미사용분도 포함 · 생성된 교환권 환산",
                     _ => "Earned refunds, including unspent returns",
                 })
            } else {
                ("—".to_owned(), t(language, "Refunds not included"))
            };
            let cards = [
                (2, t(language, "Expected Pulls"), format!("{expected:.1}"), mean_note, PINK),
                (0, match language {
                    Language::Korean => "평균 전체 생성 환급권",
                    _ => "Average earned refund tickets",
                }, return_value, return_note.to_owned(), CYAN),
            ];
            if ui.available_width() >= 560.0 {
                ui.columns(2, |columns| {
                    for (i, (kind, label, value, note, color)) in cards.iter().enumerate() {
                        overview_metric(&mut columns[i], *kind, label, value.clone(), note.clone(), *color);
                    }
                });
            } else {
                for (kind, label, value, note, color) in &cards {
                    overview_metric(ui, *kind, label, value.clone(), note.clone(), *color);
                    ui.add_space(5.0);
                }
            }
            if let (Game::StarRail, Some(stages), Some(refund)) =
                (self.setup.game, stats.budget_breakdown.as_ref(), stats.feedback.as_ref()) {
                ui.add_space(9.0);
                refund_stages_card(ui, stages, free_pulls, refund.average_raw,
                    refund.total_returned_points_average
                        / self.feedback_cfg.points_per_ticket.max(1) as f64,
                    language);
                debug_assert!((chance - stages.final_probability()).abs() < 1e-10);
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
        // Keep the game controls ahead of the standalone banner charts;
        // the joint refunded probability is now visible above this section.
        if let Some(stats) = self.visible_stats()
            .filter(|_| !self.input_needs_calculation && !self.calculating) {
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
        if let Some(stats) = self.visible_stats()
            .filter(|_| !self.input_needs_calculation && !self.calculating) {
            if let Some(strategies) = &stats.wuwa_strategies {
                ui.add_space(10.0);
                wuwa_strategy_table(ui, strategies, &self.setup, self.confidence);
            }
        }
        dirty
    }


}
