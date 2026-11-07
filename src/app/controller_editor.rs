use super::*;

impl Calculator {

    /// UI-only preferences are independent of the gacha input/worker state.
    pub(crate) fn desktop_settings(&mut self, ui: &mut egui::Ui) {
        let language = self.language();
        page_intro(ui, "APPLICATION SETTINGS", t(language, "Application settings"),
            t(language, "Change the appearance and calculation behavior, not banner inputs."));
        ui.add_space(12.0);
        section(ui, "00", t(language, "Language"), t(language, "System language"), |ui| {
            let chosen = self.language_preference;
            egui::ComboBox::from_id_salt("application_language")
                .selected_text(chosen.display(self.system_language, self.language()))
                .show_ui(ui, |ui| { self.language_choices(ui); });
            ui.label(RichText::new(t(language, "Saved language preference"))
                .color(MUTED).size(11.0));
            if let Some(error) = &self.language_save_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Language setting could not be saved: {error}"));
            }
        });
        ui.add_space(10.0);
        section(ui, "01", t(language, "Run Simulation"),
            t(language, "Changes pending. Select Run Simulation to update."), |ui| {
            ui.label(RichText::new(
                "모든 목표·천장·보유 재화를 입력한 후 Run Simulation을 누르면 계산합니다. \
                 입력 변경이나 게임 전환만으로는 계산을 시작하지 않습니다."
            ).color(MUTED).size(11.0));
            if ui.add_enabled(!self.calculating,
                egui::Button::new(t(language, "Run Simulation"))).clicked() {
                self.manual_calculation_requested = true;
            }
        });
        ui.add_space(10.0);
        section(ui, "02", t(language, "Display options"), t(language, "Change the appearance and calculation behavior, not banner inputs."), |ui| {
            ui.checkbox(&mut self.ui_preferences.show_result_panel, t(language, "Show right result/history panel"));
            ui.checkbox(&mut self.ui_preferences.show_chart_fill, t(language, "Fill area under probability curves"));
            ui.checkbox(&mut self.ui_preferences.show_hover_guides, t(language, "Show in-chart hover probability"));
            ui.checkbox(&mut self.ui_preferences.compact_spacing, t(language, "Compact spacing"));
        });
        ui.add_space(10.0);
        section(ui, "03", "게임 데이터 · 프로그램 업데이트",
            "실행 시 최신 커밋을 확인하고, 검증된 게임 데이터와 이미지만 자동 반영합니다.", |ui| {
            ui.label(RichText::new(&self.updates.message).color(MUTED).size(12.0));
            if let Some(rev) = &self.updates.revision {
                ui.label(format!("적용된 데이터 revision: {rev}"));
            }
            if let Some(version) = &self.updates.release {
                ui.colored_label(MINT, format!(
                    "새 프로그램 버전 {version} 확인 · 자동 설치 대신 검증된 릴리즈 페이지에서 설치"
                ));
                ui.hyperlink_to("최신 릴리즈 열기", runtime_updates::RELEASE_PAGE);
            } else {
                ui.label(format!("현재 프로그램: v{}", env!("CARGO_PKG_VERSION")));
            }
            if ui.add_enabled(!self.updates.is_running(),
                egui::Button::new("지금 업데이트 확인")).clicked() {
                self.updates.start(ui.ctx());
            }
            ui.label(RichText::new(
                "오프라인·검증 실패·poisoned 잠금 상태에서는 기존 검증 데이터를 유지합니다. \
                 캐릭터-전용 장비 관계는 임의로 추론하지 않습니다."
            ).color(MUTED).size(11.0));
        });
        ui.add_space(12.0);
        if ui.button(t(language, "Restore defaults")).clicked() {
            self.ui_preferences = UiPreferences::default();
        }
        ui.label(RichText::new(
            t(language, "Other display preferences reset at exit.")
        ).color(MUTED).size(11.0));
    }

    pub(crate) fn targets(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "01", "획득 목표", "목표 장수와 기존 보유 장수를 설정합니다.", |ui| {
            ui.columns(2, |cols| {
                cols[0].label(RichText::new("캐릭터 돌파 목표").color(MUTED));
                self.catalog_selector(&mut cols[0], catalog::Kind::Character);
                egui::ComboBox::from_id_salt("eid").width(175.0)
                    .selected_text(format!("E{} · 총 {}장", self.target_eid, self.target_eid + 1))
                    .show_ui(&mut cols[0], |ui| {
                        for level in 0..=6 {
                            dirty |= ui.selectable_value(&mut self.target_eid, level,
                                format!("E{level} / {}장", level + 1)).changed();
                        }
                    });
                cols[0].horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_owned)
                        .range(0..=7).speed(1).suffix("장")).changed();
                });
                cols[1].label(RichText::new(format!("{} 재련 목표", self.setup.game.secondary_banner_name())).color(MUTED));
                self.catalog_selector(&mut cols[1], catalog::Kind::Weapon);
                egui::ComboBox::from_id_salt("refine").width(175.0)
                    .selected_text(if self.target_refine == 0 { "필요 없음".to_owned() }
                        else { format!("S{} · 총 {}장", self.target_refine, self.target_refine) })
                    .show_ui(&mut cols[1], |ui| {
                        for level in 0..=5 {
                            let label = if level == 0 { "사용하지 않음".to_string() }
                                else { format!("S{level} / {level}장") };
                            dirty |= ui.selectable_value(&mut self.target_refine, level, label).changed();
                        }
                    });
                cols[1].horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_owned)
                        .range(0..=5).speed(1).suffix("장")).changed();
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
                        else { format!("S{} · {}장", self.target_refine, self.target_refine) })
                    .show_ui(ui, |ui| {
                        for count in 0..=5 {
                            dirty |= ui.selectable_value(&mut self.target_refine, count,
                                if count == 0 { "필요 없음".into() }
                                else { format!("S{count} / {count}장") }).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_owned)
                        .range(0..=5).suffix("장")).changed();
                });
            } else {
                self.catalog_selector(ui, catalog::Kind::Character);
                egui::ComboBox::from_id_salt("character_only_goal")
                    .selected_text(format!("E{} · 총 {}장", self.target_eid, self.target_eid + 1))
                    .show_ui(ui, |ui| {
                        for level in 0..=6 {
                            dirty |= ui.selectable_value(&mut self.target_eid, level,
                                format!("E{level} / {}장", level + 1)).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("기보유");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_owned)
                        .range(0..=7).suffix("장")).changed();
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

    pub(crate) fn resources(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        section(ui, "03", "보유 뽑기권 및 재화",
            "한정 픽업 배너의 전용 재화만 집계합니다. 일반 배너용 티켓은 합산하지 않습니다.", |ui| {
                let game = self.setup.game;
                if game.shares_featured_ticket() {
                    ui.label(RichText::new(format!(
                        "캐릭터·광추 동일 한정 전용권 · {}", game.character_ticket_name()
                    )).color(PRIMARY).strong());
                    changed |= edit_ticket_inventory(ui, &mut self.setup, TicketRole::Shared);
                    ui.label(RichText::new(
                        "스타레일 한정 캐릭터와 광추는 별의 궤도 전용 티켓을 함께 사용합니다."
                    ).color(MUTED).size(11.0));
                } else {
                    ui.columns(2, |columns| {
                        columns[0].label(RichText::new(format!(
                            "캐릭터 · {}", game.character_ticket_name()
                        )).color(PRIMARY));
                        changed |= edit_ticket_inventory(
                            &mut columns[0], &mut self.setup, TicketRole::Character
                        );
                        columns[1].label(RichText::new(format!(
                            "{} · {}", game.secondary_banner_name(), game.weapon_ticket_name()
                        )).color(PRIMARY));
                        changed |= edit_ticket_inventory(
                            &mut columns[1], &mut self.setup, TicketRole::Weapon
                        );
                    });
                    if game == Game::StellaSora {
                        ui.label(RichText::new(format!(
                            "골든 디스크 300개 = 레코드 1뽑 · 현재 {}개 → {}뽑 (잔여 {}개)",
                            self.setup.weapon_ticket_inventory(), self.setup.weapon_dedicated,
                            self.setup.weapon_ticket_remainder
                        )).color(MUTED).size(11.0));
                    } else if game == Game::Neverness {
                        ui.label(RichText::new(
                            "Arc는 Tri-Key 10개로 10연만 가능합니다. 전용권은 캐릭터 모집에 사용되지 않습니다."
                        ).color(MUTED).size(11.0));
                    }
                }
                ui.add_space(7.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let currency = game.currency_name();
                    let price = game.cost_per_pull();
                    ui.label(format!("전용 뽑기권 교환 재화 · {currency}"));
                    changed |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                        .range(0..=100_000_000).speed(1).suffix("개")).changed();
                    ui.label(RichText::new(format!(
                        "{}개 = 1뽑 · 전환 가능 {}뽑 (잔여 {}개)",
                        price, self.setup.premium_currency / price,
                        self.setup.premium_currency % price
                    )).color(MINT));
                });
                ui.label(RichText::new(
                    "캐릭터와 장비 전용권은 지정 배너에서만 사용합니다. 교환 가능한 재화는 두 배너에 중복 계산 없이 배분합니다."
                ).color(MUTED).size(11.0));
            });
        changed
    }

}
