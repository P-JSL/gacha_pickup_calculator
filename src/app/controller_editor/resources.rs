use super::*;

impl Calculator {
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
