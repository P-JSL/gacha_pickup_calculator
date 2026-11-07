use super::*;

impl Calculator {

    pub(crate) fn dashboard(&mut self, ui: &mut egui::Ui) {
        let Some(stats) = self.stats.as_ref().filter(|_| self.stats_game == Some(self.setup.game)) else {
            if self.calculating {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("설정에 따른 확률을 다시 계산하고 있습니다.");
                });
            } else {
                ui.colored_label(Color32::LIGHT_RED,
                    self.error.as_deref().unwrap_or("계산되지 않았습니다."));
            }
            return;
        };
        let q = self.confidence as f64 / 100.0;
        let raw_quantile = stats.combined.quantile(q);
        let owned_flexible = self.setup.available_flexible();
        let current_raw = optimal_allocation(&stats.character, &stats.weapon,
            self.setup.character_dedicated, self.setup.weapon_dedicated, owned_flexible);
        let current_probability = stats.feedback.as_ref().map(|f| f.probability(owned_flexible).0)
            .unwrap_or(current_raw.success_probability);
        page_intro(ui, "PROBABILITY OVERVIEW", "확률 분석 대시보드",
            "캐릭터·무기 획득 목표, 누적 확률과 환급 반영 필요 재화를 한눈에 확인합니다.");
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!(
                "{}  ·  E{} / {} S{}",
                self.setup.game.label(), self.target_eid,
                self.setup.game.secondary_banner_name(), self.target_refine,
            )).color(PRIMARY).strong().size(12.0));
            ui.separator();
            if ui.button("목표 및 예산 변경").clicked() {
                self.view_mode = ViewMode::Settings;
            }
        });
        ui.add_space(11.0);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.query_mode, QueryMode::RequiredBudget, "목표 확률 → 필요 n뽑");
            ui.selectable_value(&mut self.query_mode, QueryMode::AvailableBudget, "보유 N뽑 → 성공 확률");
            ui.separator();
            if self.query_mode == QueryMode::RequiredBudget {
                ui.label("목표 달성 확률");
                egui::ComboBox::from_id_salt("confidence").selected_text(format!("{}%", self.confidence))
                    .show_ui(ui, |ui| {
                        for p in [50, 75, 80, 90, 95, 99] {
                            ui.selectable_value(&mut self.confidence, p, format!("{p}%"));
                        }
                    });
            } else {
                ui.label("추가 자유 배분 가능한 재화 N");
                ui.add(egui::DragValue::new(&mut self.n_query).range(0..=100_000).speed(1)
                    .suffix("뽑"));
            }
        });

        let mut summary = vec![(
            "전체 기대 소모 · 환급 전".to_owned(),
            format!("{:.1}뽑", stats.combined.mean),
            "정확 DP / 배너 독립".to_owned(),
            PRIMARY,
        )];
        if let Some(ref feedback) = stats.feedback {
            let n = if self.query_mode == QueryMode::RequiredBudget {
                feedback.quantile(q)
            } else { self.n_query };
            let (p, low, high) = feedback.probability(n);
            summary.push((
                if self.query_mode == QueryMode::RequiredBudget {
                    "목표 확률에 필요한 자유 재화".to_owned()
                } else { "입력한 추가 자유 재화".to_owned() },
                format!("{n}뽑"),
                "전용권 별도 · 순차 환급 재투입".to_owned(), GOLD,
            ));
            summary.push((
                if self.query_mode == QueryMode::RequiredBudget {
                    "계산된 성공 확률".to_owned()
                } else { "N뽑 내 최종 성공 확률".to_owned() },
                format!("{:.2}%", p * 100.0),
                format!("95% 신뢰구간 {:.2}~{:.2}%", low * 100.0, high * 100.0), MINT,
            ));
        } else if self.query_mode == QueryMode::RequiredBudget {
            summary.push((
                format!("{}% 달성 필요", self.confidence),
                format!("{raw_quantile}뽑"),
                "환급 전 · 전체 합계".to_owned(), GOLD,
            ));
            summary.push((
                "보유권 조건 성공 확률".to_owned(),
                format!("{:.2}%", current_probability * 100.0),
                "환급 없는 정확 DP".to_owned(), MINT,
            ));
        } else {
            let a = optimal_allocation(&stats.character, &stats.weapon,
                self.setup.character_dedicated, self.setup.weapon_dedicated, self.n_query);
            summary.push((
                "추가 자유 배분 재화".to_owned(),
                format!("{}뽑", self.n_query), "전용권 별도".to_owned(), GOLD,
            ));
            summary.push((
                "N뽑 내 최종 성공 확률".to_owned(),
                format!("{:.2}%", a.success_probability * 100.0),
                "환급 없는 정확 DP".to_owned(), MINT,
            ));
        }
        if ui.available_width() >= 760.0 {
            ui.columns(3, |cols| {
                for (i, (label, value, note, color)) in summary.iter().enumerate() {
                    metric(&mut cols[i], label, value.clone(), note, *color);
                }
            });
        } else {
            for (label, value, note, color) in summary {
                metric(ui, &label, value, &note, color);
                ui.add_space(8.0);
            }
        };
        // Diagnose the exact hazard model without conflating public base
        // odds, published consolidated odds, and community-fitted curves.
        if let BannerRule::Standard { rarity, .. } = &self.setup.character.rule {
            if let Some(mean) = rarity.expected_first_rare_pulls(self.mode) {
                let model_rate = 100.0 / mean;
                let stated_rate = match self.setup.game {
                    Game::StarRail => 1.60,
                    Game::WutheringWaves => 1.80,
                    Game::Neverness => 1.87,
                    Game::StellaSora => 0.0,
                };
                ui.label(RichText::new(format!(
                    "5성 획득 모델 진단: 기본 {:.2}% · 이 모델의 무천장 시작 평균 {:.2}회/1장 · 역수 기준 {:.3}%/회 · 공개 종합률 약 {:.2}%/회. 서로 다른 수치이며, 실제 서버의 미공개 확률 곡선과 일치한다는 뜻은 아닙니다.",
                    rarity.base * 100.0, mean, model_rate, stated_rate
                )).color(MUTED).size(11.0));
            }
        }
        if let Some(feedback) = stats.feedback.as_ref() {
            if self.query_mode == QueryMode::RequiredBudget {
                if let Some(band) = feedback.quantile_confidence_band(q, 0.95) {
                    let upper = band.upper.map_or_else(
                        || "표본만으로 상한을 특정할 수 없음".to_owned(),
                        |n| format!("{n}뽑"));
                    ui.label(RichText::new(format!(
                        "Monte Carlo 표본 오차(95% 동시 신뢰대역): 목표 {}% 필요 재화 {}~{}. 표본 CDF 오차 한계 ±{:.2}%p. 게임 규칙·환급 가정의 오차는 포함하지 않습니다.",
                        self.confidence, band.lower, upper, band.cdf_tolerance * 100.0
                    )).color(GOLD).size(11.0));
                }
            }
        }
        ui.add_space(8.0);
        section(ui, "07", "현재 보유량 대비 목표 달성 추가 비용",
            "기보유 캐릭터·장비, 배너별 전용권 및 교환 재화를 반영한 목표 달성 비용입니다.",
            |ui| {
                if let Some(gap) = self.current_goal_funding() {
                    render_goal_funding(ui, &gap, &self.setup, self.language());
                } else {
                    ui.label(RichText::new(t(self.language(),
                        "Recalculate to update funding estimate")).color(GOLD));
                }
            });
        ui.add_space(8.0);
        let secondary_name = i18n::banner_name(self.language(), self.setup.game);
        let secondary_heading = format!("{secondary_name} 목표 확률");
        let single_description = "이 배너에서 목표를 달성하기까지의 뽑기 수와 누적 성공 확률입니다.";
        ui.add_space(10.0);
        // On a wide display the two independent banners are displayed next
        // to each other. On narrow/HiDPI Windows viewports they stack, so no
        // graph extends beyond the available logical width.
        if ui.available_width() >= 790.0 {
            ui.columns(2, |cols| {
                section(&mut cols[0], "05-A", "캐릭터 목표 확률", single_description, |ui| {
                    draw_probability_chart(ui, &[ChartSeries {
                        label: "캐릭터", source: ChartSource::Exact(&stats.character), color: PRIMARY,
                    }], self.ui_preferences, self.language());
                });
                section(&mut cols[1], "05-B", &secondary_heading, single_description, |ui| {
                    draw_probability_chart(ui, &[ChartSeries {
                        label: secondary_name, source: ChartSource::Exact(&stats.weapon), color: MINT,
                    }], self.ui_preferences, self.language());
                });
            });
        } else {
            section(ui, "05-A", "캐릭터 목표 확률", single_description, |ui| {
                draw_probability_chart(ui, &[ChartSeries {
                    label: "캐릭터", source: ChartSource::Exact(&stats.character), color: PRIMARY,
                }], self.ui_preferences, self.language());
            });
            ui.add_space(8.0);
            section(ui, "05-B", &secondary_heading, single_description, |ui| {
                draw_probability_chart(ui, &[ChartSeries {
                    label: secondary_name, source: ChartSource::Exact(&stats.weapon), color: MINT,
                }], self.ui_preferences, self.language());
            });
        }
        ui.add_space(10.0);
        section(ui, "05-C", "전체 목표 합산 및 환급 확률",
            "합산: 전체 두 목표를 달성할 누적 확률 · 환급: 최초 필요한 자유 배분권 기준 (서로 다른 비용 축).", |ui| {
            let mut curves = vec![ChartSeries {
                label: "전체 · 환급 전",
                source: ChartSource::Exact(&stats.combined), color: GOLD,
            }];
            let comparable = self.setup.character_dedicated == 0
                && self.setup.weapon_dedicated == 0;
            if comparable {
                if let Some(ref feedback) = stats.feedback {
                    curves.push(ChartSeries {
                        label: "환급 반영 · 자유 재화",
                        source: ChartSource::Refund(feedback),
                        color: Color32::from_rgb(203, 158, 255),
                    });
                }
            }
            draw_probability_chart(ui, &curves, self.ui_preferences, self.language());
            if stats.feedback.is_some() && !comparable {
                ui.colored_label(GOLD,
                    "전용권 사용 중에는 전체 뽑기 수와 최초 자유 재화의 기준이 달라 환급 곡선을 겹치지 않습니다.");
            }
        });
        ui.add_space(12.0);
        if let Some(strategies) = &stats.wuwa_strategies {
            wuwa_strategy_table(ui, strategies, &self.setup, self.confidence);
            ui.add_space(10.0);
        }
        if let Some(ref feedback) = stats.feedback {
            let target_budget = if self.query_mode == QueryMode::RequiredBudget {
                feedback.quantile(q)
            } else { self.n_query };
            ui.add_space(8.0);
            section(ui, "04-A", "총 뽑기와 환급 재투입 내역",
                    "같은 시뮬레이션 경로의 수치만 더하고 빼므로 서로 다른 분위수를 혼합하지 않습니다.", |ui| {
                if let Some(scenario) = if self.query_mode == QueryMode::RequiredBudget {
                    feedback.scenario_at_quantile(q)
                } else {
                    None
                } {
                    ui.label(RichText::new(format!(
                        "목표 {}%의 경계 표본: 총 {}뽑 중 전용권 {}뽑, 보유·환급 포인트로 {}뽑을 충당하여 최초 자유 재화 {}뽑이 필요합니다.",
                        self.confidence, scenario.raw_pulls, scenario.dedicated_used,
                        scenario.points_funded_pulls, scenario.initial_flexible_required,
                    )).strong().color(MINT));
                    ui.label(format!(
                        "관측한 한 경로의 정확한 내역: {} = {} + {} + {} (총뽑 = 전용권 + 포인트 충당 + 자유 재화)",
                        scenario.raw_pulls, scenario.dedicated_used, scenario.points_funded_pulls,
                        scenario.initial_flexible_required,
                    ));
                    ui.small(format!(
                        "해당 표본에서 총 획득 환급 {}{} (최종 뽑기로 받은 미사용 환급 포함).                         이는 동일 목표 확률의 모든 실행 결과를 대표하는 고정 환급값은 아닙니다.",
                        scenario.earned_points, self.setup.game.exchange_points_name()
                    ));
                }
                if let Some(means) = feedback.successful_means(target_budget) {
                    ui.separator();
                    ui.label(format!(
                        "자유 재화 {}뽑 이내 성공한 {}개 경로의 평균: 총 {:.1}뽑 = 전용권 {:.1} + 포인트 충당 {:.1} + 자유 재화 {:.1}뽑",
                        target_budget, means.count, means.average_raw,
                        means.average_dedicated, means.average_points_funded,
                        means.average_flexible,
                    ));
                    ui.small(format!(
                        "평균 누적 획득 환급 {:.1}{} · 환급으로 진행한 뽑기에서도 추가 환급 발생을 계속 계산합니다.",
                        means.average_earned_points, self.setup.game.exchange_points_name()
                    ));
                }
                if self.query_mode == QueryMode::RequiredBudget {
                    let shortage = target_budget.saturating_sub(owned_flexible);
                    let needed_native = self.setup.additional_currency_for(target_budget);
                    ui.separator();
                    ui.label(RichText::new(format!(
                        "보유 자유 배분권 {}뽑 + 보유 {} {}개를 제외하면 추가 준비는 약 {}뽑 상당, 정확히 {}{}입니다.",
                        self.setup.flexible, self.setup.game.currency_name(),
                        self.setup.premium_currency, shortage, needed_native,
                        self.setup.game.currency_name()
                    )).strong().color(GOLD));
                    ui.small("현재 보유 교환 포인트는 환급 시뮬레이션의 최초 잔액에 이미 포함되어 있으며, 중복 차감하지 않습니다.");
                }
            });
        }
        ui.add_space(10.0);
        section(ui, "06", "달성 확률별 필요 재화", "DP 분위수는 정확 모델값, 환급 분위수는 모의실험의 추정치입니다.", |ui| {
            egui::Grid::new("quantiles").num_columns(6).striped(true)
                .spacing([20.0, 6.0]).show(ui, |ui| {
                    ui.strong("달성률"); ui.strong("캐릭터");
                    ui.strong("무기"); ui.strong("합산 원시");
                    ui.strong("환급 후 자유 재화"); ui.strong("포인트 충당 표본"); ui.end_row();
                    for t in [50, 75, 80, 90, 95, 99] {
                        let p = t as f64 / 100.0;
                        ui.label(format!("{t}%"));
                        ui.label(format!("{}", stats.character.quantile(p)));
                        ui.label(format!("{}", stats.weapon.quantile(p)));
                        ui.strong(format!("{}", stats.combined.quantile(p)));
                        if let Some(ref f) = stats.feedback {
                            ui.strong(format!("{}", f.quantile(p)));
                            ui.label(f.scenario_at_quantile(p).map_or("–".to_owned(),
                                |s| format!("{}뽑", s.points_funded_pulls)));
                        } else { ui.label("미적용"); ui.label("–"); }
                        ui.end_row();
                    }
                });
            if let Some(ref f) = stats.feedback {
                ui.separator();
                ui.label(format!(
                    "환급 모의실험 {}회 · 평균 원시 {:.1}뽑 · 평균 최초 자유 재화 {:.1}뽑 · 평균 획득 환급 {:.1}{}",
                    f.budgets.len(), f.average_raw, f.average_initial_flexible,
                    f.total_returned_points_average, self.setup.game.exchange_points_name()
                ));
                let ch = f.average_character_return_points
                    / self.feedback_cfg.points_per_ticket.max(1) as f64
                    + f.average_board_dice_earned;
                let ch_rate = if f.average_character_raw > 0.0 {
                    100.0 * ch / f.average_character_raw
                } else { 0.0 };
                let weapon_rate = if f.average_weapon_raw > 0.0 {
                    100.0 * f.average_weapon_return_points
                        / self.feedback_cfg.points_per_ticket.max(1) as f64
                        / f.average_weapon_raw
                } else { 0.0 };
                ui.label(format!(
                    "시뮬레이션 평균 환급 생성량: 캐릭터 {:.2}% · {} {:.2}% (뽑기권 환산, 마지막 미사용 잔여분 포함)",
                    ch_rate, self.setup.game.secondary_banner_name(), weapon_rate
                ));
                if self.setup.game == Game::StellaSora {
                    ui.small(format!(
                        "별도 레코드 전용 적립: 평균 {:.1} 골든 디스크 (레코드 {:.2}회분). 캐릭터 환급률에 합산하지 않습니다.",
                        f.average_record_disc_units_earned,
                        f.average_record_disc_units_earned / 300.0
                    ));
                } else if self.setup.game == Game::Neverness {
                    ui.small(format!(
                        "보드 즉시 주사위 반환: 평균 {:.2}개. 보드 위치를 생략한 커뮤니티 추정이며 공식 확률이 아닙니다.",
                        f.average_board_dice_earned
                    ));
                }
                ui.small("환급 충당 표본은 해당 달성률 경계의 같은 실행 결과이며, 평균 환급이나 보장량이 아닙니다.");
                ui.label(RichText::new("주의: 4성 보유량·중복 판정·일부 배너 내부 우선순위는 근사입니다. 100% 보장은 표본으로 주장하지 않습니다.")
                    .color(GOLD).size(11.0));
            }
            ui.label(RichText::new(format!("환급 전 이론상 최대: {}뽑 · N = {}이면 환급 전 전체 성공 확률 {:.2}%",
                stats.combined.quantile(1.0), self.n_query, stats.combined.at(self.n_query) * 100.0))
                .color(MUTED).size(11.0));
            if self.setup.game == Game::Neverness {
                ui.colored_label(GOLD, "Arc의 자연 발생 복수 S급은 계산하지만, 10연 내부 보장 개입 시점은 공식 상세 부재로 근사입니다.");
            }
            if self.mode == RateMode::CommunityEstimate
                && matches!(self.setup.game, Game::StarRail | Game::WutheringWaves) {
                ui.colored_label(GOLD, "소프트 천장 확률은 커뮤니티 추정 모델입니다. 공식 서버 확률식이 아닙니다.");
            }
        });
    }

}
