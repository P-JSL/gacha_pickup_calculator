use super::*;

impl Calculator {
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
        ui.add_space(10.0);
        section(ui, "04", "실행 로그 · 진단", "앱 시작부터 종료까지 현재 세션의 이벤트와 모든 계산 단계를 기록합니다.", |ui| {
            if let Some(logger) = session_log::current() {
                ui.add(egui::Label::new(format!(
                    "로그 파일 (재시작 시 초기화): {}", logger.path().display()
                )).wrap());
            }
            if ui.button("현재 실행 로그 열기").clicked() {
                self.log_view.open = true;
                session_log::info("LOG_VIEW_OPEN", "Opened log from application settings");
            }
            ui.small("로그는 UTC 시각과 계산 세대번호를 포함하며, 몬테카를로 개별 추첨 대신 계산 단계와 집계 결과를 기록합니다.");
        });
        ui.add_space(12.0);
        if ui.button(t(language, "Restore defaults")).clicked() {
            self.ui_preferences = UiPreferences::default();
        }
        ui.label(RichText::new(
            t(language, "Other display preferences reset at exit.")
        ).color(MUTED).size(11.0));
    }


}
