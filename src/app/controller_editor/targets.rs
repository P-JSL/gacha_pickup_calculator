use super::*;

impl Calculator {
    pub(crate) fn targets(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "01", "획득 목표", "목표 장수와 기존 보유 장수를 설정합니다.", |ui| {
            ui.columns(2, |cols| {
                cols[0].label(RichText::new("캐릭터 돌파 목표").color(MUTED));
                self.catalog_selector(&mut cols[0], catalog::Kind::Character);
                egui::ComboBox::from_id_salt("eid").width(175.0)
                    .selected_text(format!("E{} · 총 {}장", self.target_eid, self.target_eid + 1))
                    .show_ui(&mut cols[0], |ui| {
                        for level in 0..self.setup.game.max_character_copies() {
                            dirty |= ui.selectable_value(&mut self.target_eid, level,
                                format!("E{level} / {}장", level + 1)).changed();
                        }
                    });
                cols[0].horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_owned)
                        .range(0..=self.setup.game.max_character_copies()).speed(1).suffix("장")).changed();
                });
                cols[1].label(RichText::new(format!("{} 재련 목표", self.setup.game.secondary_banner_name())).color(MUTED));
                self.catalog_selector(&mut cols[1], catalog::Kind::Weapon);
                egui::ComboBox::from_id_salt("refine").width(175.0)
                    .selected_text(if self.target_refine == 0 { "필요 없음".to_owned() }
                        else { format!("S{} · 총 {}장", self.setup.game.weapon_upgrade_level(self.target_refine), self.target_refine) })
                    .show_ui(&mut cols[1], |ui| {
                        for level in 0..=self.setup.game.max_weapon_copies() {
                            let label = if level == 0 { "사용하지 않음".to_string() }
                                else { format!("S{} / {level}장", self.setup.game.weapon_upgrade_level(level)) };
                            dirty |= ui.selectable_value(&mut self.target_refine, level, label).changed();
                        }
                    });
                cols[1].horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_owned)
                        .range(0..=self.setup.game.max_weapon_copies()).speed(1).suffix("장")).changed();
                });
            });
        });
        dirty
    }

    /// Independent banner pages share their input state with the dashboard.
    pub(crate) fn single_banner_page(&mut self, ui: &mut egui::Ui, weapon: bool) -> bool {
        let mut dirty = false;
        let title = if weapon { self.setup.game.secondary_banner_name() } else { "캐릭터" };
        section(ui, "01", &format!("{title} 목표와 보유"), "선택한 배너만 수정합니다.", |ui| {
            if weapon {
                self.catalog_selector(ui, catalog::Kind::Weapon);
                egui::ComboBox::from_id_salt("weapon_only_goal")
                    .selected_text(if self.target_refine == 0 { "필요 없음".into() }
                        else { format!("S{} · {}장", self.setup.game.weapon_upgrade_level(self.target_refine), self.target_refine) })
                    .show_ui(ui, |ui| {
                        for count in 0..=self.setup.game.max_weapon_copies() {
                            dirty |= ui.selectable_value(&mut self.target_refine, count,
                                if count == 0 { "필요 없음".into() }
                                else { format!("S{} / {count}장", self.setup.game.weapon_upgrade_level(count)) }).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_owned)
                        .range(0..=self.setup.game.max_weapon_copies()).suffix("장")).changed();
                });
            } else {
                self.catalog_selector(ui, catalog::Kind::Character);
                egui::ComboBox::from_id_salt("character_only_goal")
                    .selected_text(format!("E{} · 총 {}장", self.target_eid, self.target_eid + 1))
                    .show_ui(ui, |ui| {
                        for level in 0..self.setup.game.max_character_copies() {
                            dirty |= ui.selectable_value(&mut self.target_eid, level,
                                format!("E{level} / {}장", level + 1)).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_owned)
                        .range(0..=self.setup.game.max_character_copies()).suffix("장")).changed();
                });
            }
        });
        ui.add_space(10.0);
        section(ui, "02", &format!("{title} 천장"), "", |ui| {
            if weapon {
                dirty |= Self::banner_controls(ui, title, &mut self.setup.weapon.rule,
                    &mut self.setup.weapon.pity, &mut self.setup.weapon.guaranteed,
                    &mut self.setup.weapon.any_rare_pity);
            } else {
                dirty |= Self::banner_controls(ui, title, &mut self.setup.character.rule,
                    &mut self.setup.character.pity, &mut self.setup.character.guaranteed,
                    &mut self.setup.character.any_rare_pity);
                if let Some(spark) = self.setup.character.spark.as_mut() {
                    ui.separator();
                    let mut available = !spark.used;
                    if ui.checkbox(&mut available, "120회 보너스 미사용").changed() {
                        spark.used = !available;
                        dirty = true;
                    }
                    ui.horizontal(|ui| {
                        ui.label("배너 진행도");
                        dirty |= ui.add(egui::DragValue::new(&mut spark.progress)
                            .range(0..=120)).changed();
                    });
                }
            }
        });
        dirty
    }

    pub(crate) fn banner_controls(ui: &mut egui::Ui, name: &str,
                       rule: &mut BannerRule, pity: &mut usize,
                       guaranteed: &mut bool, any_rare: &mut usize) -> bool {
        let mut dirty = false;
        ui.label(RichText::new(name).strong().color(PRIMARY));
        ui.horizontal_wrapped(|ui| {
            let (hard, unit) = match rule {
                BannerRule::Standard { rarity, .. } => (rarity.hard, "회"),
                BannerRule::FeaturedCounter { hard_featured, .. } => (*hard_featured, "회"),
                BannerRule::NteArcBatch => (8, "묶음(10연)"),
            };
            ui.label("천장 스택");
            dirty |= ui.add(egui::DragValue::new(pity).range(0..=hard.saturating_sub(1))
                .speed(1).suffix(unit)).changed();
            if let BannerRule::Standard { featured_share, .. } = rule {
                if *featured_share < 1.0 {
                    dirty |= ui.checkbox(guaranteed, "다음 최고 등급 픽업 확정").changed();
                } else { *guaranteed = false; }
            }
            if matches!(rule, BannerRule::NteArcBatch) {
                ui.label("S급 천장 스택");
                dirty |= ui.add(egui::DragValue::new(any_rare).range(0..=5)).changed();
            }
        });
        dirty
    }

    pub(crate) fn pity(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "02", "천장 및 확정 상태", "캐릭터와 무기 천장은 독립적으로 관리됩니다.", |ui| {
            let secondary = self.setup.game.secondary_banner_name();
            if ui.available_width() >= 440.0 {
                ui.columns(2, |cols| {
                    dirty |= Self::banner_controls(
                        &mut cols[0], "캐릭터", &mut self.setup.character.rule,
                        &mut self.setup.character.pity, &mut self.setup.character.guaranteed,
                        &mut self.setup.character.any_rare_pity,
                    );
                    dirty |= Self::banner_controls(
                        &mut cols[1], secondary, &mut self.setup.weapon.rule,
                        &mut self.setup.weapon.pity, &mut self.setup.weapon.guaranteed,
                        &mut self.setup.weapon.any_rare_pity,
                    );
                });
            } else {
                dirty |= Self::banner_controls(
                    ui, "캐릭터", &mut self.setup.character.rule,
                    &mut self.setup.character.pity, &mut self.setup.character.guaranteed,
                    &mut self.setup.character.any_rare_pity,
                );
                ui.separator();
                dirty |= Self::banner_controls(
                    ui, secondary, &mut self.setup.weapon.rule,
                    &mut self.setup.weapon.pity, &mut self.setup.weapon.guaranteed,
                    &mut self.setup.weapon.any_rare_pity,
                );
            }
            if let Some(spark) = self.setup.character.spark.as_mut() {
                ui.separator();
                ui.label(RichText::new("배너별 120회 별도 교환 보너스").color(PRIMARY));
                let mut available = !spark.used;
                if ui.checkbox(&mut available, "보너스 미사용").changed() {
                    spark.used = !available;
                    dirty = true;
                }
                ui.horizontal(|ui| {
                    ui.label("배너 진행도");
                    dirty |= ui.add(egui::DragValue::new(&mut spark.progress).range(0..=120)).changed();
                });
            }
        });
        dirty
    }


}
