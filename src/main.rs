use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use gacha_pickup_calculator::games::{Game, GameSetup};
use gacha_pickup_calculator::{optimal_allocation, BannerRule, Distribution, RateMode};

struct Computed {
    character: Distribution,
    weapon: Distribution,
    combined: Distribution,
}

struct Calculator {
    setup: GameSetup,
    mode: RateMode,
    target_eid: usize,
    target_refine: usize,
    confidence: usize,
    stats: Option<Computed>,
    error: Option<String>,
    show_help: bool,
}

impl Calculator {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_korean_font(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app = Self {
            setup: GameSetup::new(Game::StarRail),
            mode: RateMode::CommunityEstimate,
            target_eid: 6,
            target_refine: 5,
            confidence: 90,
            stats: None,
            error: None,
            show_help: false,
        };
        app.recalculate();
        app
    }

    fn recalculate(&mut self) {
        self.setup.set_targets(self.target_eid + 1, self.target_refine);
        let char_dist = self.setup.character.distribution(self.mode);
        let weapon_dist = self.setup.weapon.distribution(self.mode);
        match (char_dist, weapon_dist) {
            (Ok(character), Ok(weapon)) => {
                let combined = character.combine(&weapon);
                self.stats = Some(Computed { character, weapon, combined });
                self.error = None;
            }
            (Err(e), _) | (_, Err(e)) => {
                self.error = Some(e);
                self.stats = None;
            }
        }
    }

    fn rule_controls(ui: &mut egui::Ui, label: &str, rule: &mut BannerRule,
                     pity: &mut usize, guaranteed: &mut bool, any_rare: &mut usize) -> bool {
        let mut dirty = false;
        ui.group(|ui| {
            ui.label(RichText::new(label).strong().color(Color32::from_rgb(140, 203, 255)));
            ui.horizontal_wrapped(|ui| {
                let (hard, unit) = match rule {
                    BannerRule::Standard { rarity, .. } => (rarity.hard, "회"),
                    BannerRule::FeaturedCounter { hard_featured, .. } => (*hard_featured, "회"),
                    BannerRule::NteArcBatch => (8, "묶음(10연)"),
                };
                ui.label(format!("현재 천장 스택 ({unit})"));
                dirty |= ui.add(egui::DragValue::new(pity).range(0..=hard - 1).speed(1)).changed();
                if let BannerRule::Standard { featured_share, .. } = rule {
                    if *featured_share < 1.0 {
                        dirty |= ui.checkbox(guaranteed, "다음 5성 픽업 확정").changed();
                    } else {
                        *guaranteed = false;
                    }
                }
                if matches!(rule, BannerRule::NteArcBatch) {
                    ui.label("마지막 S급 이후 묶음 수");
                    dirty |= ui.add(egui::DragValue::new(any_rare).range(0..=5)).changed();
                }
            });
        });
        dirty
    }

    fn target_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        ui.columns(2, |cols| {
            cols[0].group(|ui| {
                ui.label(RichText::new("캐릭터 목표").strong());
                egui::ComboBox::from_id_salt("eid")
                    .selected_text(format!("E{} (총 {}장)", self.target_eid, self.target_eid + 1))
                    .show_ui(ui, |ui| {
                        for level in 0..=6 {
                            dirty |= ui.selectable_value(&mut self.target_eid, level,
                                format!("E{level} / 총 {}장", level + 1)).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("이미 보유한 장수");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_owned).range(0..=7)).changed();
                });
            });
            cols[1].group(|ui| {
                ui.label(RichText::new("무기·광추 목표").strong());
                egui::ComboBox::from_id_salt("refine")
                    .selected_text(if self.target_refine == 0 { "불필요".to_owned() }
                        else { format!("R{} (총 {}장)", self.target_refine, self.target_refine) })
                    .show_ui(ui, |ui| {
                        for level in 0..=5 {
                            let text = if level == 0 { "무기 불필요".to_string() }
                                else { format!("R{level} / 총 {level}장") };
                            dirty |= ui.selectable_value(&mut self.target_refine, level, text).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("이미 보유한 장수");
                    dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_owned).range(0..=5)).changed();
                });
            });
        });
        dirty
    }

    fn budget_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        ui.label(RichText::new("보유 재화 · 배너별 티켓").strong());
        ui.horizontal_wrapped(|ui| {
            ui.label("캐릭터 전용 뽑기권");
            dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_dedicated).range(0..=100_000)).changed();
            ui.separator();
            ui.label("무기 전용 뽑기권");
            dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_dedicated).range(0..=100_000)).changed();
            ui.separator();
            ui.label("자유 배분 가능한 뽑기");
            dirty |= ui.add(egui::DragValue::new(&mut self.setup.flexible).range(0..=100_000)).changed();
        });
        ui.small("전용 티켓은 다른 배너에 배분되지 않습니다. 공통 재화로 교환할 분량만 '자유 배분'에 입력하세요.");
        if self.setup.game == Game::Neverness {
            ui.small("이환 무기 Arc 배너는 10개 묶음만 사용합니다. 무기 배분에서 10개 미만 잔여는 사용 불가 처리됩니다.");
        }
        dirty
    }

    fn spark_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        if let Some(spark) = self.setup.character.spark.as_mut() {
            ui.separator();
            ui.label(RichText::new("스텔라 소라: 배너 별도 120회 보너스").strong());
            let mut enabled = !spark.used;
            if ui.checkbox(&mut enabled, "이번 배너의 120회 별도 픽업 수령 가능").changed() {
                spark.used = !enabled;
                dirty = true;
            }
            ui.horizontal(|ui| {
                ui.label("현재 배너 누적 횟수");
                dirty |= ui.add(egui::DragValue::new(&mut spark.progress).range(0..=120)).changed();
            });
            ui.small("120회 보너스는 픽업이 일찍 나와도 별개로 적립됩니다. 다른 배너로 이월되지 않습니다.");
        }
        dirty
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        let Some(stats) = &self.stats else {
            ui.colored_label(Color32::LIGHT_RED, self.error.as_deref().unwrap_or("계산되지 않았습니다."));
            return;
        };
        let cutoff = self.confidence as f64 / 100.0;
        let quantile = stats.combined.quantile(cutoff);
        let alloc = optimal_allocation(&stats.character, &stats.weapon,
            self.setup.character_dedicated, self.setup.weapon_dedicated, self.setup.flexible);
        ui.separator();
        ui.heading("확률 계산 결과");
        ui.horizontal_wrapped(|ui| {
            ui.label("목표 달성 확률");
            egui::ComboBox::from_id_salt("confidence").selected_text(format!("{}%", self.confidence))
                .show_ui(ui, |ui| {
                    for t in [50, 75, 80, 90, 95, 99] {
                        ui.selectable_value(&mut self.confidence, t, format!("{t}%"));
                    }
                });
        });
        ui.columns(3, |cols| {
            value_card(&mut cols[0], "동시 달성 기대 뽑기", format!("{:.1}회", stats.combined.mean));
            value_card(&mut cols[1], &format!("{}% 달성 기준", self.confidence), format!("{}회", quantile));
            value_card(&mut cols[2], "현재 보유 자원 달성률", format!("{:.2}%", alloc.success_probability * 100.0));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("캐릭터 평균 {:.1}회 | 무기 평균 {:.1}회", stats.character.mean, stats.weapon.mean));
            ui.separator();
            ui.label(format!("최적 배분: 캐릭터 {}회 + 무기 {}회", alloc.character_pulls, alloc.weapon_pulls));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("전용권 제외·공통 뽑기권만 사용 시 {}% 목표: {}회",
                self.confidence, quantile));
            ui.separator();
            ui.label(format!("동일량 원화 아닌 {} 환산: {}개",
                self.setup.game.currency_name(), quantile * self.setup.game.cost_per_pull()));
        });
        ui.small("정확한 확률은 입력된 모델 내에서의 추가 뽑기 기준입니다. 기존 보유 티켓과 이벤트 보상은 별도입니다.");
        ui.separator();
        ui.heading("누적 달성 확률 분포");
        ui.label("하단 그래프: 가로=추가 뽑기 수, 세로=목표 획득 누적 확률(%)");
        let end = stats.combined.pmf.len().saturating_sub(1);
        let step = (end / 250).max(1);
        Plot::new("completion_chart")
            .legend(Legend::default())
            .height(330.0)
            .include_y(0.0)
            .include_y(100.0)
            .include_x(0.0)
            .allow_drag(true)
            .allow_zoom(true)
            .show(ui, |plot| {
                let build = |dist: &Distribution| -> PlotPoints<'_> {
                    (0..=end).step_by(step)
                        .map(|x| [x as f64, dist.at(x) * 100.0])
                        .chain(std::iter::once([end as f64, dist.at(end) * 100.0]))
                        .collect()
                };
                plot.line(Line::new("캐릭터 목표", build(&stats.character)).color(Color32::from_rgb(79, 156, 255)).width(2.0));
                plot.line(Line::new("무기 목표", build(&stats.weapon)).color(Color32::from_rgb(91, 211, 147)).width(2.0));
                plot.line(Line::new("캐릭터 + 무기 최종 목표", build(&stats.combined)).color(Color32::from_rgb(255, 164, 92)).width(3.0));
            });
        ui.separator();
        ui.heading("목표 확률별 누적 뽑기 수");
        egui::Grid::new("quantiles").num_columns(4).striped(true).spacing([22.0, 7.0]).show(ui, |ui| {
            ui.strong("확률"); ui.strong("캐릭터"); ui.strong("무기"); ui.strong("합산 (독립 1차 도달)"); ui.end_row();
            for t in [50, 75, 80, 90, 95, 99, 100] {
                let q = t as f64 / 100.0;
                ui.label(format!("{t}%"));
                ui.label(format!("{}회", stats.character.quantile(q)));
                ui.label(format!("{}회", stats.weapon.quantile(q)));
                ui.strong(format!("{}회", stats.combined.quantile(q)));
                ui.end_row();
            }
        });
        ui.separator();
        ui.label(RichText::new("모델 정확도와 환급(피드백) 적용 범위").strong());
        ui.small("천장·픽업 확정·보유 스택·배너 독립성을 상태 전이로 직접 계산하며, 두 배너의 첫 목표 달성 시간 분포는 컨볼루션으로 합산합니다. 몬테카를로 샘플링 오차는 없습니다.");
        ui.small("스타라이트·산호·디스크 등의 환급은 보유 캐릭터의 중복도, 4성 보유 상태 및 실제 상점 교환 조건에 따라 달라집니다. 현재 그래프는 환급 전 실제 소모 횟수만 계산합니다. 확정할 수 없는 환급을 확률 계산에 임의로 더하지 않습니다.");
        if self.setup.game == Game::Neverness {
            ui.colored_label(Color32::YELLOW, "Arc 10연 내부 복수 S등급 당첨을 한 번으로 취급한 근사입니다. 해당 곡선은 완전한 정확도 보장을 하지 않습니다.");
        }
        if self.mode == RateMode::CommunityEstimate && matches!(self.setup.game, Game::StarRail | Game::WutheringWaves) {
            ui.colored_label(Color32::YELLOW, "소프트 천장: 비공식 커뮤니티 추정 곡선. 실제 서버 구현을 확인한 공식 확률식이 아닙니다.");
        }
    }
}

fn value_card(ui: &mut egui::Ui, title: &str, value: String) {
    ui.group(|ui| {
        ui.label(RichText::new(title).color(Color32::LIGHT_GRAY));
        ui.label(RichText::new(value).size(22.0).strong());
    });
}

impl eframe::App for Calculator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut dirty = false;
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("GACHA PROBABILITY LAB").strong().size(18.0));
                ui.separator();
                ui.menu_button(format!("게임 선택: {} ▼", self.setup.game.label()), |ui| {
                    for game in Game::ALL {
                        if ui.selectable_label(self.setup.game == game, game.label()).clicked() {
                            self.setup = GameSetup::new(game);
                            dirty = true;
                            ui.close();
                        }
                    }
                });
                ui.menu_button("확률 모델 ▼", |ui| {
                    dirty |= ui.radio_value(&mut self.mode, RateMode::PublishedOnly, "공식 기본 확률 + 하드 천장").changed();
                    dirty |= ui.radio_value(&mut self.mode, RateMode::CommunityEstimate, "검증되지 않은 소프트 천장 추정 포함").changed();
                });
                ui.menu_button("도움말 / 출처 ▼", |ui| {
                    if ui.button("확률 규칙 / 출처 보기").clicked() {
                        self.show_help = true;
                        ui.close();
                    }
                    ui.hyperlink_to("원본 저장소", "https://github.com/P-JSL/gacha_pickup_calculator");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading(self.setup.game.label());
                ui.label(self.setup.game.special_notes());
                ui.add_space(8.0);
                dirty |= self.target_controls(ui);
                ui.add_space(8.0);
                dirty |= Self::rule_controls(ui, "캐릭터 픽업 천장", &mut self.setup.character.rule,
                    &mut self.setup.character.pity, &mut self.setup.character.guaranteed, &mut self.setup.character.any_rare_pity);
                dirty |= Self::rule_controls(ui, "무기 / 광추 픽업 천장", &mut self.setup.weapon.rule,
                    &mut self.setup.weapon.pity, &mut self.setup.weapon.guaranteed, &mut self.setup.weapon.any_rare_pity);
                dirty |= self.spark_controls(ui);
                ui.add_space(6.0);
                dirty |= self.budget_controls(ui);
                if dirty { self.recalculate(); }
                self.results(ui);
            });
        });
        if self.show_help {
            egui::Window::new("게임별 규칙 · 참고 자료")
                .open(&mut self.show_help)
                .resizable(true)
                .show(ctx, |ui| {
                    ui.heading(self.setup.game.label());
                    ui.label(self.setup.game.special_notes());
                    ui.separator();
                    ui.label("공식 규칙과 커뮤니티 모델은 동일한 신뢰도를 갖지 않습니다.");
                    for (name, url) in self.setup.game.source_links() {
                        ui.hyperlink_to(*name, *url);
                    }
                    ui.separator();
                    ui.hyperlink_to("흡수 마르코프 연쇄의 정확한 재귀식 — Symmetry (2026)", "https://doi.org/10.3390/sym18061051");
                });
        }
    }
}

fn install_korean_font(ctx: &egui::Context) {
    let locations = [
        r"C:\Windows\Fonts\malgun.ttf",
        r"C:\Windows\Fonts\malgunbd.ttf",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    ];
    for location in locations {
        if let Ok(bytes) = std::fs::read(location) {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert("korean".to_owned(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().insert(0, "korean".to_owned());
            }
            ctx.set_fonts(fonts);
            break;
        }
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 900.0])
            .with_min_inner_size([780.0, 570.0])
            .with_title("Gacha Pickup Calculator — Rust"),
        ..Default::default()
    };
    eframe::run_native("Gacha Pickup Calculator", options,
        Box::new(|cc| Ok(Box::new(Calculator::new(cc)))))
}