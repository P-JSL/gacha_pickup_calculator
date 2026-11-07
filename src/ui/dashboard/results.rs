use super::*;

impl Calculator {
    /// User-facing summary and session history, attached to the right edge on wide screens.
    pub(crate) fn workspace_results_panel(&mut self, ctx: &egui::Context) {
        let language = self.language();
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
                        ui.label(RichText::new(format!("▥  {}", t(language, "Result Summary"))).size(15.0).strong());
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
                            ui.label(RichText::new(format!("◌  {}", t(language, "Refund Loop"))).size(12.0));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let msg = if self.feedback_cfg.enabled {
                                    t(language, "Refunds recycled")
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
                            ui.label(RichText::new(format!("◇  {}", t(language, "Extra Pulls from Refund"))).size(11.5));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let bonus = self.visible_stats()
                                    .and_then(|stats| stats.successful_budget_means.as_ref());
                                let value = bonus.map_or("—".to_owned(),
                                    |means| format!("+{:.1}", means.average_points_funded));
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
                                ui.label(RichText::new(format!("★   {}", t(language, "Estimated Outcome")))
                                    .size(13.0).strong());
                            });
                        ui.add_space(6.0);
                        for (title, value) in [
                            (t(language, "Target Character"), results.map(|(_,ch,_)|
                                format!("E{} · {:.2}%", self.target_eid,ch*100.0))),
                            (&format!("{} · {}", t(language, "Target Weapon"), i18n::banner_name(language, self.setup.game)), results.map(|(_,_,wp)|
                                format!("S{} · {:.2}%", self.target_refine,wp*100.0))),
                        ] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("☑").color(SUCCESS).size(13.0));
                                ui.label(RichText::new(title).size(11.0));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        ui.label(RichText::new(value.as_deref().unwrap_or(t(language, "Calculating")))
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
                                        "★ {} (E{}S{})", t(language, "Overall"), self.target_eid,self.target_refine
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
                            ui.label(RichText::new(format!("◷  {}", t(language, "Simulation History")))
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
                                        ui.label(RichText::new(i18n::game_name(language, entry.game))
                                            .size(10.8).strong());
                                        ui.label(RichText::new(format!(
                                            "E{} · {} S{}", entry.eid,
                                            i18n::banner_name(language, entry.game),entry.refine
                                        )).color(MUTED).size(9.5));
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(RichText::new(format!(
                                                "{:.2}%",entry.chance*100.0
                                            )).color(SUCCESS).size(11.0).strong());
                                        });
                                } else {
                                    ui.label(RichText::new(t(language, "No simulation yet"))
                                        .color(MUTED).size(10.0));
                                }
                            });
                            if i != 4 { ui.separator(); }
                        }
                        ui.add_space(7.0);
                        if ui.add(
                            egui::Button::new(format!("▤  {}  →", t(language, "View All History")))
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
