use super::*;

impl Calculator {

    pub(crate) fn feedback_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "04", "교환 재화 · 피드백 모델", "획득한 교환 재화를 다음 뽑기에 즉시 재사용합니다.", |ui| {
            dirty |= ui.checkbox(&mut self.feedback_cfg.enabled,
                "피드백 뽑기 재화 반영 (재현 가능한 몬테카를로)").changed();
            ui.label(RichText::new(format!(
                "{} · 게임별 환급 기본값",
                self.setup.game.exchange_points_name()
            )).color(PRIMARY));
            if self.setup.game == Game::WutheringWaves {
                ui.colored_label(PRIMARY,
                    "명조: 산호를 뽑기권으로 사용하거나 픽업 캐릭터 돌파 재료를 최대 2개 우선 구매합니다.");
                ui.horizontal_wrapped(|ui| {
                    ui.label("산호 사용 전략");
                    egui::ComboBox::from_id_salt("wuwa_wavebands")
                        .selected_text(format!("돌파 최대 {}개", self.feedback_cfg.wuwa_waveband_max))
                        .show_ui(ui, |ui| {
                            for count in 0..=2 {
                                dirty |= ui.selectable_value(
                                    &mut self.feedback_cfg.wuwa_waveband_max, count,
                                    if count == 0 { "뽑기권만 교환".to_owned() }
                                    else { format!("돌파 {count}개 우선") }).changed();
                            }
                        });
                    ui.label("돌파 1개 가격");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.wuwa_waveband_cost)
                        .range(1..=100_000).suffix("산호")).changed();
                });
                ui.small("산호 8개=뽑기권 1개, 돌파 재료 360개 기본값(사용자 수정 가능). 획득 중 발생한 산호까지 반영하며 남는 산호는 뽑기에 사용합니다.");
            }
            if self.setup.game == Game::Neverness {
                ui.colored_label(GOLD,
                    "이환: Warp Pieces 24개=빨간 주사위 1개. S급 중복 40개, 풀돌 후 80개; A급 중복 6개, 풀돌 후 12개; A급 Arc 4개.");
                ui.small("Annulith 160개=빨간 주사위 1개. Arc S급 환급·Tri-Key 교환 및 보드 추가 보상은 별도 검증이 필요한 설정값입니다.");
                ui.horizontal_wrapped(|ui| {
                    ui.label("보드 주사위 1개 반환 확률(%)");
                    let mut p = self.feedback_cfg.nte_one_die_chance * 100.0;
                    if ui.add(egui::DragValue::new(&mut p).range(0.0..=100.0).speed(0.01)).changed() {
                        self.feedback_cfg.nte_one_die_chance = p / 100.0;
                        dirty = true;
                    }
                    ui.label("주사위 5개 반환 확률(%)");
                    let mut p = self.feedback_cfg.nte_five_dice_chance * 100.0;
                    if ui.add(egui::DragValue::new(&mut p).range(0.0..=100.0).speed(0.01)).changed() {
                        self.feedback_cfg.nte_five_dice_chance = p / 100.0;
                        dirty = true;
                    }
                });
                ui.small("기본값 1.57% / 0.38%는 커뮤니티 추정입니다. 실제 보드 위치·주사위 상태에 따라 다를 수 있습니다.");
            }
            if self.setup.game == Game::StarRail {
                ui.colored_label(GOLD,
                    "스타레일: 스타라이트 20개=전용 티켓 1장. 캐릭터 중복 5성 40개(풀돌 후 100개), 4성 8개(풀돌 후 20개); 광추 5성 40개, 4성 8개.");
            }
            if self.setup.game == Game::StellaSora {
                ui.colored_label(GOLD,
                    "스텔라 소라: 전문가 증명서 100개=1뽑, 중복 5성 200개·중복 4성 40개. 최초 신규 5성은 환급 0개로 설정합니다.");
                ui.label(RichText::new(
                    "최대 돌파 초과 추가 보상은 미반영. 4성 중복 획득률은 계정별로 달라질 수 있습니다."
                ).color(MUTED).size(11.0));
                ui.horizontal_wrapped(|ui| {
                    ui.label("캐릭터 1회당 레코드 전용 디스크");
                    dirty |= ui.add(egui::DragValue::new(
                        &mut self.feedback_cfg.stella_record_discs_per_character_pull)
                        .range(0..=300).suffix("개")).changed();
                    ui.label("초회 3배 혜택 남은 횟수");
                    dirty |= ui.add(egui::DragValue::new(
                        &mut self.feedback_cfg.stella_first_ten_remaining)
                        .range(0..=10).suffix("회")).changed();
                });
                ui.small("레코드 모집 1회=골든 디스크 300개. 30개/캐릭터 모집은 레코드 전용 10% 환급 가정이며 캐릭터권으로 전환하지 않습니다. 초회 3배 보너스는 계정별 남은 횟수를 알아야 하므로 기본 0회입니다.");
            } else {
                ui.label(RichText::new("4성의 중복·보유 상태는 입력한 평균 보상 가정에 따라 계산합니다.")
                    .color(MUTED).size(11.0));
            }
            if self.feedback_cfg.enabled {
                ui.horizontal_wrapped(|ui| {
                    let name = self.setup.game.exchange_points_name();
                    ui.label("1회 교환 가격");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.points_per_ticket)
                        .range(1..=100_000).suffix(name)).changed();
                    ui.separator();
                    ui.label(format!("현재 보유 {name}"));
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.initial_points)
                        .range(0..=1_000_000)).changed();
                    ui.label(format!(
                        "현재 교환 가능 {}뽑",
                        self.feedback_cfg.initial_points / self.feedback_cfg.points_per_ticket.max(1)
                    ));
                });
                egui::CollapsingHeader::new("고급 설정 · 중복 보상 / 4성 확률 / 시뮬레이션")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label("픽업 캐릭터 첫 장");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.featured_first)
                        .range(0..=100_000)).changed();
                    ui.label("중복 장당");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.featured_duplicate)
                        .range(0..=100_000)).changed();
                    ui.label("풀돌 후 중복");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.featured_maxed)
                        .range(0..=100_000)).changed();
                    ui.label("픽뚫 최고 등급");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.off_banner_rare)
                        .range(0..=100_000)).changed();
                    ui.label(format!("{} 첫 장", self.setup.game.secondary_banner_name()));
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.weapon_first)
                        .range(0..=100_000)).changed();
                    ui.label(format!("{} 중복 장당", self.setup.game.secondary_banner_name()));
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.weapon_rare)
                        .range(0..=100_000)).changed();
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("캐릭터 4성 평균 환급");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.four_star_reward)
                        .range(0..=100_000)).changed();
                    if matches!(self.setup.game, Game::StarRail | Game::Neverness) {
                        ui.label("풀돌 4성/A급 환급");
                        dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.four_star_maxed)
                            .range(0..=100_000)).changed();
                        ui.label("풀돌 4성/A급 획득 비율(%)");
                        let mut share = self.feedback_cfg.four_star_maxed_share * 100.0;
                        if ui.add(egui::DragValue::new(&mut share).range(0.0..=100.0).speed(1.0)).changed() {
                            self.feedback_cfg.four_star_maxed_share = share / 100.0;
                            dirty = true;
                        }
                    }
                    ui.label(format!("{} 4성 평균 환급", self.setup.game.secondary_banner_name()));
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.weapon_four_star_reward)
                        .range(0..=100_000)).changed();
                    ui.label("캐릭터 4성 기본률(%)");
                    let mut rate = self.feedback_cfg.character_four_star_rate * 100.0;
                    if ui.add(egui::DragValue::new(&mut rate).range(0.0..=100.0).speed(0.1)).changed() {
                        self.feedback_cfg.character_four_star_rate = rate / 100.0;
                        dirty = true;
                    }
                    ui.label("무기 4성 기본률(%)");
                    let mut rate = self.feedback_cfg.weapon_four_star_rate * 100.0;
                    if ui.add(egui::DragValue::new(&mut rate).range(0.0..=100.0).speed(0.1)).changed() {
                        self.feedback_cfg.weapon_four_star_rate = rate / 100.0;
                        dirty = true;
                    }
                });
                if self.setup.game == Game::StellaSora {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("캐릭터 비픽업 5성 기본률(%)");
                        let mut p = self.feedback_cfg.character_other_five_star_rate * 100.0;
                        if ui.add(egui::DragValue::new(&mut p)
                            .range(0.0..=100.0).speed(0.1)).changed() {
                            self.feedback_cfg.character_other_five_star_rate = p / 100.0;
                            dirty = true;
                        }
                        ui.separator();
                        ui.label("레코드 비픽업 5성 기본률(%)");
                        let mut p = self.feedback_cfg.weapon_other_five_star_rate * 100.0;
                        if ui.add(egui::DragValue::new(&mut p)
                            .range(0.0..=100.0).speed(0.1)).changed() {
                            self.feedback_cfg.weapon_other_five_star_rate = p / 100.0;
                            dirty = true;
                        }
                    });
                    ui.small("비픽업 5성은 픽업 천장을 초기화하지 않고, 중복 획득 보상 가정으로 환급됩니다.");
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label("4성 최대 천장");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.four_star_hard)
                        .range(1..=20)).changed();
                    ui.separator();
                    ui.label("시뮬레이션 횟수");
                    egui::ComboBox::from_id_salt("sample_count")
                        .selected_text(format!("{}회", self.feedback_cfg.samples))
                        .show_ui(ui, |ui| {
                            for n in [5_000, 20_000, 50_000, 100_000, 250_000, 500_000, 1_000_000] {
                                dirty |= ui.selectable_value(&mut self.feedback_cfg.samples, n,
                                    format!("{n}회")).changed();
                            }
                        });
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("캐릭터 4성 현재 스택");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.character_four_star_pity)
                        .range(0..=self.feedback_cfg.four_star_hard.saturating_sub(1))).changed();
                    ui.separator();
                    ui.label("무기 4성 현재 스택");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.weapon_four_star_pity)
                        .range(0..=self.feedback_cfg.four_star_hard.saturating_sub(1))).changed();
                });
                        ui.colored_label(MUTED, "환급은 장당 반환량과 4성 발생 모델을 지정한 조건부 추정입니다. 실제 결과·계정의 중복 보유율과 다를 수 있습니다.");
                    });
            }
        });
        dirty
    }

}
