#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use gacha_pickup_calculator::feedback::{estimate, FeedbackConfig, FeedbackEstimate};
use gacha_pickup_calculator::games::{Game, GameSetup};
use gacha_pickup_calculator::{optimal_allocation, BannerRule, Distribution, RateMode};
use gacha_pickup_calculator::tracker::{expected_for_banner, references, TrackedBanner};

const BG: Color32 = Color32::from_rgb(13, 19, 33);
const CARD: Color32 = Color32::from_rgb(24, 34, 51);
const MUTED: Color32 = Color32::from_rgb(154, 171, 193);
const PRIMARY: Color32 = Color32::from_rgb(108, 187, 255);
const MINT: Color32 = Color32::from_rgb(98, 223, 176);
const GOLD: Color32 = Color32::from_rgb(255, 183, 102);
const BORDER: Color32 = Color32::from_rgb(46, 60, 78);

struct Computed {
    character: Distribution,
    weapon: Distribution,
    combined: Distribution,
    feedback: Option<FeedbackEstimate>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueryMode { RequiredBudget, AvailableBudget }

struct Calculator {
    setup: GameSetup,
    mode: RateMode,
    target_eid: usize,
    target_refine: usize,
    confidence: usize,
    query_mode: QueryMode,
    n_query: usize,
    feedback_cfg: FeedbackConfig,
    stats: Option<Computed>,
    error: Option<String>,
    show_help: bool,
}

impl Calculator {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_korean_font(&cc.egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = BG;
        visuals.window_fill = CARD;
        visuals.extreme_bg_color = Color32::from_rgb(17, 25, 40);
        visuals.widgets.inactive.bg_fill = CARD;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 58, 79);
        visuals.selection.bg_fill = Color32::from_rgb(39, 102, 158);
        cc.egui_ctx.set_visuals(visuals);
        let mut app = Self {
            setup: GameSetup::new(Game::StarRail),
            mode: RateMode::CommunityEstimate,
            target_eid: 6,
            target_refine: 5,
            confidence: 90,
            query_mode: QueryMode::RequiredBudget,
            n_query: 1_000,
            feedback_cfg: FeedbackConfig::for_game(Game::StarRail),
            stats: None,
            error: None,
            show_help: false,
        };
        app.recalculate();
        app
    }

    fn change_game(&mut self, game: Game) {
        self.setup = GameSetup::new(game);
        self.feedback_cfg = FeedbackConfig::for_game(game);
        self.target_eid = 6;
        self.target_refine = 5;
    }

    fn recalculate(&mut self) {
        self.setup.set_targets(self.target_eid + 1, self.target_refine);
        let result = (|| {
            let character = self.setup.character.distribution(self.mode)?;
            let weapon = self.setup.weapon.distribution(self.mode)?;
            let combined = character.combine(&weapon);
            let feedback = if self.feedback_cfg.enabled {
                Some(estimate(&self.setup, self.mode, &self.feedback_cfg)?)
            } else { None };
            Ok::<_, String>(Computed { character, weapon, combined, feedback })
        })();
        match result {
            Ok(stats) => { self.stats = Some(stats); self.error = None; }
            Err(message) => { self.stats = None; self.error = Some(message); }
        }
    }

    fn header(&mut self, ctx: &egui::Context) -> bool {
        let mut dirty = false;
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(9.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("◈  GACHA LAB").strong().size(20.0).color(PRIMARY));
                ui.separator();
                ui.menu_button(format!("{} ▾", self.setup.game.label()), |ui| {
                    for game in Game::ALL {
                        if ui.selectable_label(self.setup.game == game, game.label()).clicked() {
                            if self.setup.game != game { self.change_game(game); dirty = true; }
                            ui.close_menu();
                        }
                    }
                });
                ui.menu_button("확률 모델 ▾", |ui| {
                    dirty |= ui.radio_value(&mut self.mode, RateMode::PublishedOnly,
                        "공개 기본 확률 + 확정 천장").changed();
                    dirty |= ui.radio_value(&mut self.mode, RateMode::CommunityEstimate,
                        "소프트 천장 추정 모델").changed();
                });
                ui.menu_button("실측 트래커 ▾", |ui| {
                    ui.label(RichText::new("날짜가 고정된 외부 관측 자료").strong());
                    for reference in references(self.setup.game) {
                        ui.hyperlink_to(reference.name, reference.url);
                        ui.small(format!("자료 확인일: {}", reference.observed_on));
                    }
                    ui.separator();
                    ui.small("외부 통계가 내부 소프트 천장 확률을 자동 변경하지 않습니다.");
                });
                ui.menu_button("사용법 / 출처 ▾", |ui| {
                    if ui.button("모델과 출처 확인").clicked() {
                        self.show_help = true;
                        ui.close_menu();
                    }
                    ui.hyperlink_to("GitHub 저장소", "https://github.com/P-JSL/gacha_pickup_calculator");
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("RUST · EXACT DP + REFUND ESTIMATE").color(MUTED).size(10.0));
                });
            });
            ui.add_space(8.0);
        });
        dirty
    }

    fn targets(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "01", "획득 목표", "목표 장수와 기존 보유 장수를 설정합니다.", |ui| {
            ui.columns(2, |cols| {
                cols[0].label(RichText::new("캐릭터 돌파 목표").color(MUTED));
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
                cols[1].label(RichText::new("무기·광추 재련 목표").color(MUTED));
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

    fn banner_controls(ui: &mut egui::Ui, name: &str,
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

    fn pity(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "02", "천장 및 확정 상태", "캐릭터와 무기 천장은 독립적으로 관리됩니다.", |ui| {
            dirty |= Self::banner_controls(ui, "캐릭터", &mut self.setup.character.rule,
                &mut self.setup.character.pity, &mut self.setup.character.guaranteed,
                &mut self.setup.character.any_rare_pity);
            ui.separator();
            dirty |= Self::banner_controls(ui, "무기·광추", &mut self.setup.weapon.rule,
                &mut self.setup.weapon.pity, &mut self.setup.weapon.guaranteed,
                &mut self.setup.weapon.any_rare_pity);
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

    fn resources(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "03", "보유 티켓 및 사용 예산", "전용권은 다른 배너로 이전되지 않습니다.", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("캐릭터 전용권");
                dirty |= ui.add(egui::DragValue::new(&mut self.setup.character_dedicated)
                    .range(0..=100_000).suffix("뽑")).changed();
                ui.separator();
                ui.label("무기 전용권");
                dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_dedicated)
                    .range(0..=100_000).suffix("뽑")).changed();
                ui.separator();
                ui.label("자유 배분권");
                dirty |= ui.add(egui::DragValue::new(&mut self.setup.flexible)
                    .range(0..=100_000).suffix("뽑")).changed();
            });
            if self.setup.game == Game::Neverness {
                ui.label(RichText::new("이환 Arc 배너는 10연 단위이며, 배너 전용권의 10장 미만 잔여도 별도 보유권으로 처리됩니다.")
                    .color(GOLD));
            }
        });
        dirty
    }

    fn feedback_controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut dirty = false;
        section(ui, "04", "교환 재화 · 피드백 모델", "획득한 교환 재화를 다음 뽑기에 즉시 재사용합니다.", |ui| {
            dirty |= ui.checkbox(&mut self.feedback_cfg.enabled,
                "피드백 뽑기 재화 반영 (재현 가능한 몬테카를로)").changed();
            let manual = matches!(self.setup.game, Game::Neverness | Game::StellaSora);
            if manual {
                ui.colored_label(GOLD, "이 게임의 교환 비율·환급 규칙이 검증되지 않아 수동 입력 전용입니다.");
            } else {
                ui.label(RichText::new("게임 기본값은 참고용입니다. 4성의 보유·중복 상태는 계정별 평균 가정으로 입력하세요.")
                    .color(MUTED).size(11.0));
            }
            if self.feedback_cfg.enabled {
                ui.horizontal_wrapped(|ui| {
                    ui.label("1회 교환 가격");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.points_per_ticket)
                        .range(1..=100_000).suffix("포인트")).changed();
                    ui.separator();
                    ui.label("현재 교환 포인트");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.initial_points)
                        .range(0..=1_000_000)).changed();
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("픽업 캐릭터 첫 장");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.featured_first)
                        .range(0..=100_000)).changed();
                    ui.label("중복 장당");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.featured_duplicate)
                        .range(0..=100_000)).changed();
                    ui.label("픽뚫 최고 등급");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.off_banner_rare)
                        .range(0..=100_000)).changed();
                    ui.label("픽업 무기 최고 등급");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.weapon_rare)
                        .range(0..=100_000)).changed();
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("4성 1개 평균 환급");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.four_star_reward)
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
                ui.horizontal_wrapped(|ui| {
                    ui.label("4성 최대 천장");
                    dirty |= ui.add(egui::DragValue::new(&mut self.feedback_cfg.four_star_hard)
                        .range(1..=20)).changed();
                    ui.separator();
                    ui.label("시뮬레이션 횟수");
                    egui::ComboBox::from_id_salt("sample_count")
                        .selected_text(format!("{}회", self.feedback_cfg.samples))
                        .show_ui(ui, |ui| {
                            for n in [5_000, 20_000, 50_000, 100_000] {
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
            }
        });
        dirty
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
        let Some(stats) = &self.stats else {
            ui.colored_label(Color32::LIGHT_RED,
                self.error.as_deref().unwrap_or("계산되지 않았습니다."));
            return;
        };
        let q = self.confidence as f64 / 100.0;
        let raw_quantile = stats.combined.quantile(q);
        let current_raw = optimal_allocation(&stats.character, &stats.weapon,
            self.setup.character_dedicated, self.setup.weapon_dedicated, self.setup.flexible);
        let current_probability = stats.feedback.as_ref().map(|f| f.probability(self.setup.flexible).0)
            .unwrap_or(current_raw.success_probability);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.heading("확률 분석 대시보드");
            ui.label(RichText::new("입력 상태 변경 시 자동 계산").color(MUTED).size(11.0));
        });
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

        ui.columns(3, |cols| {
            metric(&mut cols[0], "전체 기대 소모 · 환급 전", format!("{:.1}뽑", stats.combined.mean),
                "정확 DP / 배너 독립", PRIMARY);
            if let Some(ref feedback) = stats.feedback {
                let n = if self.query_mode == QueryMode::RequiredBudget {
                    feedback.quantile(q)
                } else { self.n_query };
                metric(&mut cols[1],
                    if self.query_mode == QueryMode::RequiredBudget { "목표 확률에 필요한 자유 재화" }
                    else { "입력한 추가 자유 재화" },
                    format!("{n}뽑"), "전용권 별도 · 환급 반영", GOLD);
                let (p, low, high) = feedback.probability(n);
                metric(&mut cols[2],
                    if self.query_mode == QueryMode::RequiredBudget { "계산된 성공 확률" }
                    else { "N뽑 내 최종 성공 확률" },
                    format!("{:.2}%", p * 100.0),
                    &format!("95% 신뢰구간 {:.2}~{:.2}%", low * 100.0, high * 100.0), MINT);
            } else if self.query_mode == QueryMode::RequiredBudget {
                metric(&mut cols[1], &format!("{}% 달성 필요", self.confidence),
                    format!("{raw_quantile}뽑"), "환급 전 · 전체 합계", GOLD);
                metric(&mut cols[2], "보유권 조건 성공 확률",
                    format!("{current_probability:.2}%", current_probability = current_probability * 100.0),
                    "환급 없는 정확 DP", MINT);
            } else {
                let a = optimal_allocation(&stats.character, &stats.weapon,
                    self.setup.character_dedicated, self.setup.weapon_dedicated, self.n_query);
                metric(&mut cols[1], "추가 자유 배분 재화", format!("{}뽑", self.n_query),
                    "전용권 별도", GOLD);
                metric(&mut cols[2], "N뽑 내 최종 성공 확률",
                    format!("{:.2}%", a.success_probability * 100.0),
                    "환급 없는 정확 DP", MINT);
            }
        });
        ui.add_space(8.0);
        section(ui, "05", "확률 그래프", "X축은 추가 뽑기 수, Y축은 목표까지의 누적 성공 확률(%)입니다.", |ui| {
            let raw_end = stats.combined.pmf.len().saturating_sub(1);
            let show_feedback = stats.feedback.is_some();
            let end = if let Some(ref f) = stats.feedback {
                raw_end.max(f.quantile(1.0))
            } else { raw_end };
            let step = (end / 280).max(1);
            Plot::new("completion_chart").legend(Legend::default())
                .height(345.0).include_y(0.0).include_y(100.0)
                .include_x(0.0).include_x(end as f64)
                .allow_drag(true).allow_zoom(true)
                .show(ui, |plot| {
                    for (name, dist, color) in [
                        ("캐릭터 목표", &stats.character, PRIMARY),
                        ("무기 목표", &stats.weapon, MINT),
                        ("합산 · 환급 전", &stats.combined, GOLD),
                    ] {
                        let points = PlotPoints::from_iter((0..=end).step_by(step)
                            .map(|x| [x as f64, dist.at(x) * 100.0]));
                        plot.line(Line::new(points).name(name).color(color).width(2.0_f32));
                    }
                    if let Some(ref feedback) = stats.feedback {
                        // No dedicated tickets: both curves have identical x-axis units.
                        if self.setup.character_dedicated == 0 && self.setup.weapon_dedicated == 0 {
                            let points = PlotPoints::from_iter((0..=end).step_by(step)
                                .map(|x| [x as f64, feedback.probability(x).0 * 100.0]));
                            plot.line(Line::new(points).name("환급 포함 · 초기 보유 N")
                                .color(Color32::from_rgb(203, 158, 255)).width(3.5_f32));
                        }
                    }
                });
            if show_feedback && (self.setup.character_dedicated > 0 || self.setup.weapon_dedicated > 0) {
                ui.label(RichText::new("전용권 사용 중: 환급 후 자유 재화와 전체 뽑기 수는 축 단위가 달라 별도 그래프 비교를 생략했습니다.")
                    .color(GOLD).size(11.0));
            }
        });
        ui.add_space(10.0);
        section(ui, "06", "달성 확률별 필요 재화", "DP 분위수는 정확 모델값, 환급 분위수는 모의실험의 추정치입니다.", |ui| {
            egui::Grid::new("quantiles").num_columns(5).striped(true)
                .spacing([32.0, 8.0]).show(ui, |ui| {
                    ui.strong("달성률"); ui.strong("캐릭터");
                    ui.strong("무기"); ui.strong("합산 원시");
                    ui.strong("환급 후 자유 재화"); ui.end_row();
                    for t in [50, 75, 80, 90, 95, 99] {
                        let p = t as f64 / 100.0;
                        ui.label(format!("{t}%"));
                        ui.label(format!("{}", stats.character.quantile(p)));
                        ui.label(format!("{}", stats.weapon.quantile(p)));
                        ui.strong(format!("{}", stats.combined.quantile(p)));
                        if let Some(ref f) = stats.feedback {
                            ui.strong(format!("{}", f.quantile(p)));
                        } else { ui.label("미적용"); }
                        ui.end_row();
                    }
                });
            if let Some(ref f) = stats.feedback {
                ui.separator();
                ui.label(format!("환급 모의실험 {}회 · 평균 원시 {:.1}뽑 · 평균 추가 자유 재화 {:.1}뽑 · 평균 반환 {:.1} 포인트",
                    f.budgets.len(), f.average_raw, f.average_initial_flexible, f.total_returned_points_average));
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
        ui.add_space(10.0);
        section(ui, "07", "실측 트래커 교차 검증",
                "관측 평균과 현재 확률 모델을 비교합니다. 표본은 공식 서버 확률이 아니며 자동 보정하지 않습니다.", |ui| {
            let observed = references(self.setup.game);
            for source in observed {
                ui.horizontal_wrapped(|ui| {
                    ui.hyperlink_to(source.name, source.url);
                    ui.label(RichText::new(format!("관측 {} (고정 스냅샷)", source.observed_on))
                        .color(MUTED).size(11.0));
                });
                if let Some(mean) = source.mean_rare_pulls {
                    let banner = match source.banner {
                        TrackedBanner::Character => Some(&self.setup.character),
                        TrackedBanner::Weapon => Some(&self.setup.weapon),
                        TrackedBanner::General => None,
                    };
                    if let Some(pred) = banner.and_then(|b| expected_for_banner(b, self.mode)) {
                        let diff = (pred - mean) / mean * 100.0;
                        ui.label(format!("5성 간격: 트래커 평균 {:.1}뽑 | 모델 기대 {:.1}뽑 | 편차 {:+.1}%",
                            mean, pred, diff));
                        ui.small("같은 배너·천장 0부터의 5성 획득 간격 비교입니다. 픽업 확정·중복 환급은 포함되지 않습니다.");
                    }
                }
                if let Some(count) = source.total_pulls {
                    ui.small(format!("사이트 표기 표본 뽑기 횟수: {count}"));
                }
                ui.label(RichText::new(source.notes).color(MUTED).size(11.0));
                ui.separator();
            }
            ui.colored_label(GOLD,
                "평균 스택 하나만으로 스택별 확률이나 소프트 천장 함수를 역산할 수 없습니다. 이탈·불완전 이력·표본 편향을 확인해야 합니다.");
        });
    }
}

/// Generic card, with a fixed visual hierarchy and no stateful UI controls.
fn section(ui: &mut egui::Ui, number: &str, heading: &str, description: &str,
           content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(CARD).stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(13)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(number).color(PRIMARY).strong().size(12.0));
                ui.label(RichText::new(heading).strong().size(16.0));
            });
            ui.label(RichText::new(description).color(MUTED).size(11.0));
            ui.add_space(7.0);
            content(ui);
        });
}

fn metric(ui: &mut egui::Ui, label: &str, value: String, note: &str, accent: Color32) {
    egui::Frame::group(ui.style()).fill(CARD).stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(10)).inner_margin(12)
        .show(ui, |ui| {
            ui.set_min_width(220.0);
            ui.label(RichText::new(label).color(MUTED).size(12.0));
            ui.add_space(6.0);
            ui.label(RichText::new(value).color(accent).size(24.0).strong());
            ui.add_space(2.0);
            ui.label(RichText::new(note).color(MUTED).size(10.0));
        });
}

impl eframe::App for Calculator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut dirty = self.header(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(16.0);
                ui.heading(RichText::new(self.setup.game.label()).size(25.0).strong());
                ui.label(RichText::new(self.setup.game.special_notes()).color(MUTED));
                ui.add_space(14.0);
                dirty |= self.targets(ui);
                ui.add_space(8.0);
                dirty |= self.pity(ui);
                ui.add_space(8.0);
                dirty |= self.resources(ui);
                ui.add_space(8.0);
                dirty |= self.feedback_controls(ui);
                if dirty { self.recalculate(); }
                self.dashboard(ui);
                ui.add_space(25.0);
            });
        });
        if self.show_help {
            egui::Window::new("게임별 규칙 · 신뢰도 · 출처")
                .open(&mut self.show_help).resizable(true).show(ctx, |ui| {
                    ui.heading(self.setup.game.label());
                    ui.label(self.setup.game.special_notes());
                    ui.separator();
                    ui.label("정확 DP는 입력 확률 모델의 상태 전이를 계산합니다. 게임 서버의 비공개 확률식을 보장하지 않습니다.");
                    ui.label("환급은 사용자 입력 중복 보상 및 4성 발생 모델을 따른 Monte Carlo 추정입니다.");
                    for (title, url) in self.setup.game.source_links() {
                        ui.hyperlink_to(*title, *url);
                    }
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
            .with_inner_size([1230.0, 940.0])
            .with_min_inner_size([840.0, 650.0])
            .with_title("Gacha Lab — Rust 확률 분석기"),
        ..Default::default()
    };
    eframe::run_native("Gacha Lab", options,
        Box::new(|cc| Ok(Box::new(Calculator::new(cc)))))
}