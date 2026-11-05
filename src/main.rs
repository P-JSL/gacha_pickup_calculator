#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use gacha_pickup_calculator::feedback::{estimate, FeedbackConfig, FeedbackEstimate};
use gacha_pickup_calculator::games::{Game, GameSetup};
use gacha_pickup_calculator::{optimal_allocation, BannerRule, Distribution, RateMode};
use gacha_pickup_calculator::tracker::{expected_for_banner, references, TrackedBanner};
use gacha_pickup_calculator::observations::{parse_observation_csv, ObservationDataset};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

const BG: Color32 = Color32::from_rgb(13, 19, 33);
const CARD: Color32 = Color32::from_rgb(24, 34, 51);
const MUTED: Color32 = Color32::from_rgb(154, 171, 193);
const PRIMARY: Color32 = Color32::from_rgb(108, 187, 255);
const MINT: Color32 = Color32::from_rgb(98, 223, 176);
const GOLD: Color32 = Color32::from_rgb(255, 183, 102);
const BORDER: Color32 = Color32::from_rgb(45, 57, 76);
const SIDEBAR: Color32 = Color32::from_rgb(14, 21, 35);
const SELECTED_NAV: Color32 = Color32::from_rgb(33, 53, 76);
const SURFACE: Color32 = Color32::from_rgb(18, 28, 44);

struct Computed {
    character: Distribution,
    weapon: Distribution,
    combined: Distribution,
    feedback: Option<FeedbackEstimate>,
}

struct CalculationRequest {
    generation: u64,
    setup: GameSetup,
    mode: RateMode,
    feedback_config: FeedbackConfig,
}

struct CalculationResponse {
    generation: u64,
    result: Result<Computed, String>,
}

fn compute(request: &CalculationRequest) -> Result<Computed, String> {
    let character = request.setup.character.distribution(request.mode)?;
    let weapon = request.setup.weapon.distribution(request.mode)?;
    let combined = character.combine(&weapon);
    let feedback = if request.feedback_config.enabled {
        Some(estimate(&request.setup, request.mode, &request.feedback_config)?)
    } else { None };
    Ok(Computed { character, weapon, combined, feedback })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueryMode { RequiredBudget, AvailableBudget }

/// Keep the primary calculator uncluttered. Advanced evidence inspection is
/// explicitly opened from the header rather than appended to every result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode { Calculator, Settings, TrackerSources, ObservedDistribution }

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
    view_mode: ViewMode,
    worker: Sender<CalculationRequest>,
    results: Receiver<CalculationResponse>,
    generation: u64,
    pending_since: Option<Instant>,
    calculating: bool,
    observation_path: String,
    observation_data: Option<ObservationDataset>,
    observation_banner: TrackedBanner,
    observation_message: String,
}

impl Calculator {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // set_visuals alone changes only the current theme style. The system
        // preference could otherwise switch the UI back to Light on Windows.
        configure_dark_desktop(&cc.egui_ctx);
        install_korean_font(&cc.egui_ctx);
        // All potentially costly Markov-DP and refund simulations run off the GUI thread.
        // The worker drops queued stale requests before each calculation; the UI also
        // rejects out-of-generation responses to avoid showing outdated probabilities.
        let (worker, work_rx) = mpsc::channel::<CalculationRequest>();
        let (result_tx, results) = mpsc::channel::<CalculationResponse>();
        let repaint = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut request) = work_rx.recv() {
                while let Ok(newer) = work_rx.try_recv() {
                    request = newer;
                }
                let result = compute(&request);
                if result_tx.send(CalculationResponse {
                    generation: request.generation,
                    result,
                }).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        let mut app = Self {
            setup: GameSetup::new(Game::StarRail),
            mode: RateMode::CommunityEstimate,
            target_eid: 0,
            target_refine: 1,
            confidence: 90,
            query_mode: QueryMode::RequiredBudget,
            n_query: 1_000,
            feedback_cfg: FeedbackConfig::for_game(Game::StarRail),
            stats: None,
            error: None,
            show_help: false,
            view_mode: ViewMode::Calculator,
            worker,
            results,
            generation: 0,
            pending_since: None,
            calculating: false,
            observation_path: String::new(),
            observation_data: None,
            observation_banner: TrackedBanner::Character,
            observation_message: String::new(),
        };
        app.recalculate();
        app
    }

    fn change_game(&mut self, game: Game) {
        self.setup = GameSetup::new(game);
        self.feedback_cfg = FeedbackConfig::for_game(game);
        self.target_eid = 0;
        self.target_refine = 1;
        self.observation_data = None;
        self.observation_path.clear();
        self.observation_message.clear();
    }

    fn recalculate(&mut self) {
        self.setup.set_targets(self.target_eid + 1, self.target_refine);
        self.generation = self.generation.wrapping_add(1);
        self.calculating = true;
        self.stats = None;
        self.error = None;
        let request = CalculationRequest {
            generation: self.generation,
            setup: self.setup.clone(),
            mode: self.mode,
            feedback_config: self.feedback_cfg.clone(),
        };
        if self.worker.send(request).is_err() {
            self.calculating = false;
            self.error = Some("백그라운드 계산 작업을 시작할 수 없습니다.".to_owned());
        }
    }

    fn poll_results(&mut self) {
        while let Ok(response) = self.results.try_recv() {
            // Ignore results from settings the user has since changed.
            if response.generation != self.generation || self.pending_since.is_some() {
                continue;
            }
            self.calculating = false;
            match response.result {
                Ok(stats) => { self.stats = Some(stats); self.error = None; }
                Err(message) => { self.stats = None; self.error = Some(message); }
            }
        }
    }

    fn schedule_recalculation(&mut self) {
        self.pending_since = Some(Instant::now());
        self.stats = None;
        self.error = None;
        self.calculating = true;
    }

    /// Slim top toolbar. Views are selected from the permanent navigation
    /// rail rather than an overcrowded dropdown menu.
    fn header(&mut self, ctx: &egui::Context) -> bool {
        let mut dirty = false;
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(7))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("GACHA").color(PRIMARY).strong().size(17.0));
                    ui.label(RichText::new("LAB").strong().size(17.0));
                    ui.label(RichText::new(" /  ANALYTICS WORKSPACE")
                        .color(MUTED).size(11.0));
                    ui.add_space(12.0);
                    ui.label(RichText::new("게임").color(MUTED).size(12.0));
                    egui::ComboBox::from_id_salt("top_game")
                        .width(150.0).selected_text(self.setup.game.label())
                        .show_ui(ui, |ui| {
                            for game in Game::ALL {
                                if ui.selectable_label(self.setup.game == game, game.label()).clicked()
                                    && self.setup.game != game
                                {
                                    self.change_game(game);
                                    dirty = true;
                                }
                            }
                        });
                    ui.separator();
                    ui.menu_button("확률 모델 ▾", |ui| {
                        dirty |= ui.radio_value(&mut self.mode,
                            RateMode::PublishedOnly, "공개 기본 확률 + 확정 천장").changed();
                        dirty |= ui.radio_value(&mut self.mode,
                            RateMode::CommunityEstimate, "소프트 천장 추정 모델").changed();
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("도움말").clicked() { self.show_help = true; }
                    });
                });
            });
        dirty
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("workspace_sidebar")
            .resizable(false)
            .exact_width(150.0)
            .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(7))
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("WORKSPACE").color(MUTED).size(10.0).strong());
                ui.add_space(8.0);
                for (view, title) in [
                    (ViewMode::Calculator, "01   확률 대시보드"),
                    (ViewMode::Settings, "02   시뮬레이션 설정"),
                    (ViewMode::TrackerSources, "03   트래커 교차 검증"),
                    (ViewMode::ObservedDistribution, "04   실측 CSV 분석"),
                ] {
                    let active = self.view_mode == view;
                    let label = RichText::new(title).size(13.0)
                        .color(if active { Color32::WHITE } else { MUTED })
                        .strong();
                    let button = egui::Button::new(label)
                        .fill(if active { SELECTED_NAV } else { SIDEBAR })
                        .stroke(egui::Stroke::new(if active { 1.0_f32 } else { 0.0_f32 }, BORDER))
                        .corner_radius(9);
                    if ui.add_sized([ui.available_width(), 30.0], button).clicked() {
                        self.view_mode = view;
                    }
                    ui.add_space(3.0);
                }
                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(RichText::new("CURRENT GAME").color(MUTED).size(10.0));
                ui.add_space(5.0);
                ui.label(RichText::new(self.setup.game.label()).strong().size(13.0));
                ui.label(RichText::new(format!(
                    "캐릭터 E{}  ·  {} S{}",
                    self.target_eid, self.setup.game.secondary_banner_name(),
                    self.target_refine
                )).color(MUTED).size(11.0));
                ui.add_space(7.0);
                ui.label(RichText::new("정확 DP · 조건부 환급 추정")
                    .color(MUTED).size(10.0));
                ui.add_space(12.0);
                ui.hyperlink_to("GitHub / Source", "https://github.com/P-JSL/gacha_pickup_calculator");
                ui.add_space(5.0);
                ui.label(RichText::new("v0.1.10  ·  DESKTOP").color(MUTED).size(10.0));
            });
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
                cols[1].label(RichText::new(format!("{} 재련 목표", self.setup.game.secondary_banner_name())).color(MUTED));
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
            dirty |= Self::banner_controls(ui, self.setup.game.secondary_banner_name(), &mut self.setup.weapon.rule,
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
                ui.label(format!("{} 전용권", self.setup.game.secondary_banner_name()));
                dirty |= ui.add(egui::DragValue::new(&mut self.setup.weapon_dedicated)
                    .range(0..=100_000).suffix("뽑")).changed();
                ui.separator();
                ui.label("자유 배분권");
                dirty |= ui.add(egui::DragValue::new(&mut self.setup.flexible)
                    .range(0..=100_000).suffix("뽑")).changed();
            });
            ui.horizontal_wrapped(|ui| {
                let currency = self.setup.game.currency_name();
                let price = self.setup.game.cost_per_pull();
                ui.label(format!("보유 {currency}"));
                dirty |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                    .range(0..=100_000_000).speed(1).suffix(currency)).changed();
                ui.separator();
                ui.label(format!("1뽑 = {price}{currency}"));
                ui.separator();
                ui.label(RichText::new(format!(
                    "자유 배분권 합계: {}뽑 (직접 {}뽑 + {currency} 전환 {}뽑, 잔여 {}{currency})",
                    self.setup.available_flexible(), self.setup.flexible,
                    self.setup.premium_currency / price,
                    self.setup.premium_currency % price,
                )).color(MINT));
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
            ui.label(RichText::new(format!(
                "{} · 게임별 초기 설정 (필요 시 수동 수정 가능)",
                self.setup.game.exchange_points_name()
            )).color(PRIMARY));
            if self.setup.game == Game::Neverness {
                ui.colored_label(GOLD,
                    "이환: Warp Pieces 교환 단가·보드 보상·Tri-Key 전환 조건은 추가 검증이 필요한 추정값입니다.");
            }
            if self.setup.game == Game::StellaSora {
                ui.colored_label(GOLD,
                    "스텔라 소라: 전문가 증명서 100개=1뽑, 중복 5성 200개·중복 4성 40개. 최초 신규 5성은 환급 0개로 설정합니다.");
                ui.label(RichText::new(
                    "최대 돌파 초과 추가 보상은 미반영. 4성 중복 획득률은 계정별로 달라질 수 있습니다."
                ).color(MUTED).size(11.0));
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
                        "현재 재화로 {}뽑 즉시 교환 가능",
                        self.feedback_cfg.initial_points / self.feedback_cfg.points_per_ticket.max(1)
                    ));
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
        ui.add_space(8.0);
        let secondary_name = self.setup.game.secondary_banner_name();
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
                    }]);
                });
                section(&mut cols[1], "05-B", &secondary_heading, single_description, |ui| {
                    draw_probability_chart(ui, &[ChartSeries {
                        label: secondary_name, source: ChartSource::Exact(&stats.weapon), color: MINT,
                    }]);
                });
            });
        } else {
            section(ui, "05-A", "캐릭터 목표 확률", single_description, |ui| {
                draw_probability_chart(ui, &[ChartSeries {
                    label: "캐릭터", source: ChartSource::Exact(&stats.character), color: PRIMARY,
                }]);
            });
            ui.add_space(8.0);
            section(ui, "05-B", &secondary_heading, single_description, |ui| {
                draw_probability_chart(ui, &[ChartSeries {
                    label: secondary_name, source: ChartSource::Exact(&stats.weapon), color: MINT,
                }]);
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
            draw_probability_chart(ui, &curves);
            if stats.feedback.is_some() && !comparable {
                ui.colored_label(GOLD,
                    "전용권 사용 중에는 전체 뽑기 수와 최초 자유 재화의 기준이 달라 환급 곡선을 겹치지 않습니다.");
            }
        });
        ui.add_space(12.0);
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

    fn tracker_sources(&self, ui: &mut egui::Ui) {
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

    /// The user explicitly provides a local CSV path; data remains on device.
    /// This is offline inspection only: never fetch undocumented tracker APIs
    /// or accept account-authenticated URLs and tokens.
    fn observation_validation(&mut self, ui: &mut egui::Ui) {
        section(ui, "08", "스택별 실측 분포 비교",
                "CSV를 로컬에서 분석합니다. 완전한 이력·검열 구간이 없는 트래커 그래프는 편향 가능성을 표시합니다.", |ui| {
            let mut banner_changed = false;
            ui.horizontal_wrapped(|ui| {
                ui.label("데이터 배너");
                egui::ComboBox::from_id_salt("observed_banner")
                    .selected_text(match self.observation_banner {
                        TrackedBanner::Character => "캐릭터",
                        TrackedBanner::Weapon => "무기·광추",
                        TrackedBanner::General => "선택 필요",
                    })
                    .show_ui(ui, |ui| {
                        banner_changed |= ui.selectable_value(
                            &mut self.observation_banner, TrackedBanner::Character, "캐릭터").changed();
                        banner_changed |= ui.selectable_value(
                            &mut self.observation_banner, TrackedBanner::Weapon, "무기·광추").changed();
                    });
            });
            if banner_changed {
                self.observation_data = None;
                self.observation_message = "배너가 변경되었습니다. 해당 배너의 이력을 다시 불러오세요.".into();
            }
            let rule = match self.observation_banner {
                TrackedBanner::Character => &self.setup.character.rule,
                TrackedBanner::Weapon => &self.setup.weapon.rule,
                TrackedBanner::General => &self.setup.character.rule,
            };
            // Only a one-dimensional rarity hazard can be compared directly.
            // NTE Arc is a two-counter, 10-pull batch with nontrivial guarantee
            // substitution. Stella Sora uses direct pickup counters and a free
            // one-time 120-pull reward, not ordinary 5★ rarity pity.
            let candidate = match rule {
                BannerRule::Standard { rarity, .. } => Some(rarity.clone()),
                BannerRule::FeaturedCounter { .. } | BannerRule::NteArcBatch => None,
            };
            ui.horizontal_wrapped(|ui| {
                ui.label("CSV 파일 경로");
                ui.add(egui::TextEdit::singleline(&mut self.observation_path)
                    .hint_text("예: C:\\tracker\\pity_history.csv").desired_width(460.0));
                if ui.button("로컬 CSV 분석").clicked() {
                    if let Some(ref schedule) = candidate {
                        let load = (|| -> Result<ObservationDataset, String> {
                            let path = std::path::Path::new(&self.observation_path);
                            let size = std::fs::metadata(path)
                                .map_err(|e| format!("파일 확인 실패: {e}"))?.len();
                            if size > 8 * 1024 * 1024 {
                                return Err("CSV 크기는 8 MiB 이하로 제한됩니다.".into());
                            }
                            let contents = std::fs::read_to_string(path)
                                .map_err(|e| format!("UTF-8 CSV 읽기 실패: {e}"))?;
                            parse_observation_csv(&contents, schedule.hard)
                        })();
                        match load {
                            Ok(data) => {
                                self.observation_message = format!(
                                    "가져오기 성공 · {}개 관측 · {}개 획득 · {}개 검열",
                                    data.observations, data.events, data.censored);
                                self.observation_data = Some(data);
                            }
                            Err(error) => {
                                self.observation_data = None;
                                self.observation_message = error;
                            }
                        }
                    } else {
                        self.observation_data = None;
                        self.observation_message = "이 배너는 단순 최고 등급 대기시간 모델이 아니어서 직접 비교를 지원하지 않습니다.".into();
                    }
                }
                if ui.button("데이터 지우기").clicked() {
                    self.observation_data = None;
                    self.observation_message.clear();
                }
            });
            ui.label(RichText::new(
                "지원 CSV: duration,event (개별: 획득=1/중단=0), pity,at_risk,events,censored (위험집합), pity,count (완료 기록만)"
            ).color(MUTED).size(11.0));
            ui.small("한 배너, 천장 0부터 시작한 완전한 구간만 사용해야 합니다. 기록 앞부분이 잘렸다면 해당 구간을 버리세요.");
            if !self.observation_message.is_empty() {
                ui.label(RichText::new(&self.observation_message).color(PRIMARY));
            }
            let (Some(data), Some(model)) = (&self.observation_data, candidate) else {
                ui.colored_label(GOLD, "분석 데이터가 없거나 선택한 배너를 직접 비교할 수 없습니다.");
                return;
            };
            ui.separator();
            ui.label(RichText::new(data.quality.label()).strong());
            if !data.quality.is_censor_aware() {
                ui.colored_label(GOLD,
                    "경고: 성공 스택만 기록된 자료는 미완료 이력의 검열을 보정할 수 없습니다. 아래 비교는 기술 통계이며 모델의 적합성을 확정하지 않습니다.");
            } else {
                ui.label(RichText::new("독립적인 우측 검열 가정에 따른 Kaplan–Meier 추정, Greenwood 95% 구간").color(MINT));
            }
            let Ok(comparison) = data.compare(&model, self.mode) else {
                ui.colored_label(Color32::LIGHT_RED, "배너 모델과 관측 데이터가 호환되지 않습니다.");
                return;
            };
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "관측 범위 1~{}스택 | 관측 {} | 5성 획득 {} | 검열 {}",
                    data.max_pity(), data.observations, data.events, data.censored));
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("제한 평균: 관측 {:.2}회 | 모델 {:.2}회",
                    comparison.restricted_observed_mean,
                    comparison.restricted_model_mean));
                ui.separator();
                if comparison.reliable_risk_bins > 0 {
                    ui.label(format!("최대 누적확률 편차 {:.2}%p | 위험률 평균 절대오차 {:.2}%p",
                        comparison.max_cdf_delta * 100.0,
                        comparison.mean_abs_hazard_delta * 100.0));
                    ui.small(format!("위험집합 20개 이상인 {}개 스택만 편차 계산",
                        comparison.reliable_risk_bins));
                } else {
                    ui.colored_label(GOLD, "위험집합이 20개 이상인 구간이 없어 정량 편차 판정을 보류합니다.");
                }
            });
            let mut model_survival = 1.0_f64;
            let mut observed_pts = vec![[0.0, 0.0]];
            let mut predicted_pts = vec![[0.0, 0.0]];
            let mut lower_pts = vec![[0.0, 0.0]];
            let mut upper_pts = vec![[0.0, 0.0]];
            let mut observed_hazard = Vec::new();
            let mut predicted_hazard = Vec::new();
            for row in &data.rows {
                let x = row.pity as f64;
                let chance = model.chance(row.pity, self.mode);
                model_survival *= 1.0 - chance;
                observed_pts.push([x, (1.0 - row.survival) * 100.0]);
                predicted_pts.push([x, (1.0 - model_survival) * 100.0]);
                if let Some((low, high)) = row.cdf_ci_95 {
                    lower_pts.push([x, low * 100.0]);
                    upper_pts.push([x, high * 100.0]);
                }
                if row.at_risk >= 20 {
                    observed_hazard.push([x, row.hazard * 100.0]);
                    predicted_hazard.push([x, chance * 100.0]);
                }
            }
            Plot::new("survival_observed_cdf").legend(Legend::default())
                .height(280.0).include_y(0.0).include_y(100.0)
                .include_x(0.0).include_x(model.hard as f64)
                .allow_zoom(true).allow_drag(true)
                .show(ui, |plot| {
                    plot.line(Line::new(PlotPoints::from_iter(observed_pts))
                        .name(if data.quality.is_censor_aware() {
                            "실측 누적 확률 · KM"
                        } else {
                            "완료 이력 분포 · 편향 가능"
                        }).color(MINT).width(3.0_f32));
                    plot.line(Line::new(PlotPoints::from_iter(predicted_pts))
                        .name("선택한 모델").color(PRIMARY).width(2.5_f32));
                    if data.quality.is_censor_aware() {
                        plot.line(Line::new(PlotPoints::from_iter(lower_pts))
                            .name("실측 95% 하한").color(GOLD).width(1.0_f32));
                        plot.line(Line::new(PlotPoints::from_iter(upper_pts))
                            .name("실측 95% 상한").color(GOLD).width(1.0_f32));
                    }
                });
            egui::CollapsingHeader::new("스택별 순간 5성 확률 비교 (위험집합 20건 이상)")
                .default_open(false).show(ui, |ui| {
                    Plot::new("survival_observed_hazard").legend(Legend::default())
                        .height(220.0).include_y(0.0).include_y(100.0)
                        .show(ui, |plot| {
                            plot.line(Line::new(PlotPoints::from_iter(observed_hazard))
                                .name("실측 위험률").color(MINT).width(2.0_f32));
                            plot.line(Line::new(PlotPoints::from_iter(predicted_hazard))
                                .name("모델 위험률").color(PRIMARY).width(2.0_f32));
                        });
                });
            ui.colored_label(GOLD, "주의: Greenwood 95% 구간은 최초 획득 이전(F=0)과 전원 획득 이후(F=1)의 경계에서 표시하지 않습니다. 독립적 기록 중단만 보정하며 임의 계정 업로드, 누락·혼합된 이력은 보정하지 않습니다.");
        });
    }
}

/// Build a monotonic, endpoint-complete CDF curve for both paint and tests.
/// The DP distribution remains the source of truth; sampling only reduces
/// screen-level line geometry, not numerical probabilities.
fn sampled_cdf(distribution: &Distribution, end: usize) -> Vec<(usize, f64)> {
    let end = end.max(1);
    let n = end.min(400);
    (0..=n).map(|i| {
        let x = i * end / n;
        (x, distribution.at(x).clamp(0.0, 1.0))
    }).collect()
}

/// Each chart has its own X-range. Character and secondary-banner curves
/// therefore cannot be compressed into a shared 2000-pull wide axis.
#[derive(Clone, Copy)]
enum ChartSource<'a> {
    Exact(&'a Distribution),
    Refund(&'a FeedbackEstimate),
}

impl ChartSource<'_> {
    fn last_pull(self) -> usize {
        match self {
            Self::Exact(d) => d.pmf.len().saturating_sub(1),
            Self::Refund(f) => f.quantile(1.0),
        }
    }

    fn probability(self, n: usize) -> f64 {
        match self {
            Self::Exact(d) => d.at(n),
            Self::Refund(f) => f.probability(n).0,
        }
    }
}

struct ChartSeries<'a> {
    label: &'a str,
    source: ChartSource<'a>,
    color: Color32,
}

fn chart_domain_end(series: &[ChartSeries<'_>]) -> usize {
    series.iter().map(|s| s.source.last_pull()).max().unwrap_or(1).max(1)
}

/// Common Painter plot for all desktop OSs; it uses only egui logical points.
/// The same fill, grid, axes, scaling and series colors are used on Windows
/// and Linux. No OS-native chart controls or persistence are involved.
fn draw_probability_chart(ui: &mut egui::Ui, series: &[ChartSeries<'_>]) {
    let end = chart_domain_end(series);
    let width = ui.available_width().max(270.0);
    let height = if width < 650.0 { 205.0 } else { 232.0 };
    let (response, painter) = ui.allocate_painter(
        egui::vec2(width, height), egui::Sense::hover(),
    );
    let bounds = response.rect;
    let graph = egui::Rect::from_min_max(
        egui::pos2(bounds.left() + 49.0, bounds.top() + 12.0),
        egui::pos2(bounds.right() - 10.0, bounds.bottom() - 34.0),
    );
    painter.rect_filled(bounds, 10.0, SURFACE);
    for edge in [
        [graph.left_top(), graph.right_top()],
        [graph.left_bottom(), graph.right_bottom()],
        [graph.left_top(), graph.left_bottom()],
        [graph.right_top(), graph.right_bottom()],
    ] {
        painter.line_segment(edge, egui::Stroke::new(1.0_f32, BORDER));
    }

    let project = |x: usize, probability: f64| -> egui::Pos2 {
        egui::pos2(
            graph.left() + (x as f32 / end as f32) * graph.width(),
            graph.bottom() - probability.clamp(0.0, 1.0) as f32 * graph.height(),
        )
    };
    for step in 0..=4 {
        let probability = step as f64 / 4.0;
        let y = project(0, probability).y;
        painter.line_segment(
            [egui::pos2(graph.left(), y), egui::pos2(graph.right(), y)],
            egui::Stroke::new(1.0_f32, BORDER),
        );
        painter.text(
            egui::pos2(graph.left() - 7.0, y),
            egui::Align2::RIGHT_CENTER, format!("{}%", step * 25),
            egui::FontId::proportional(11.0), MUTED,
        );
    }
    for step in 0..=5 {
        let x = end * step / 5;
        painter.text(
            egui::pos2(project(x, 0.0).x, graph.bottom() + 10.0),
            egui::Align2::CENTER_TOP, format!("{x}"),
            egui::FontId::proportional(11.0), MUTED,
        );
    }
    for line in series {
        let points: Vec<egui::Pos2> = match line.source {
            ChartSource::Exact(d) => sampled_cdf(d, end).into_iter()
                .map(|(x, p)| project(x, p)).collect(),
            ChartSource::Refund(f) => {
                let n = end.min(400);
                (0..=n).map(|i| {
                    let x = i * end / n;
                    project(x, f.probability(x).0)
                }).collect()
            }
        };
        painter.add(egui::Shape::line(
            points, egui::Stroke::new(2.8_f32, line.color),
        ));
    }
    if let Some(pointer) = response.hover_pos().filter(|pos| graph.contains(*pos)) {
        let fraction = (pointer.x - graph.left()) / graph.width();
        let x = ((fraction * end as f32).round() as usize).min(end);
        let px = project(x, 0.0).x;
        painter.line_segment([
            egui::pos2(px, graph.top()), egui::pos2(px, graph.bottom()),
        ], egui::Stroke::new(1.0_f32, MUTED));
        for line in series {
            painter.circle_filled(
                project(x, line.source.probability(x)), 3.5, line.color,
            );
        }
        ui.horizontal_wrapped(|ui| {
            ui.strong(format!("{x}뽑 이내"));
            for line in series {
                ui.colored_label(line.color,
                    format!("{} {:.1}%", line.label, line.source.probability(x) * 100.0));
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        for line in series {
            ui.colored_label(line.color, format!("● {}", line.label));
        }
    });
}

fn page_intro(ui: &mut egui::Ui, eyebrow: &str, title: &str, subtitle: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(eyebrow).size(9.5).color(PRIMARY).strong());
        ui.heading(RichText::new(title).size(18.0).strong());
        ui.label(RichText::new(subtitle).size(10.5).color(MUTED));
    });
    ui.add_space(6.0);
}

/// Generic card, with a fixed visual hierarchy and no stateful UI controls.
fn section(ui: &mut egui::Ui, number: &str, heading: &str, description: &str,
           content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(CARD).stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(11))
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                egui::Frame::default()
                    .fill(SELECTED_NAV)
                    .corner_radius(egui::CornerRadius::same(5))
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(number).color(PRIMARY).strong().size(10.0));
                    });
                ui.label(RichText::new(heading).strong().size(13.0));
                if !description.is_empty() {
                    ui.label(RichText::new(description).color(MUTED).size(10.0));
                }
            });
            ui.add_space(4.0);
            content(ui);
        });
}

fn metric(ui: &mut egui::Ui, label: &str, value: String, note: &str, accent: Color32) {
    egui::Frame::group(ui.style()).fill(CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(11)).inner_margin(9)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width().min(145.0));
            ui.label(RichText::new(label).color(MUTED).size(10.5));
            ui.label(RichText::new(value).color(accent).size(21.0).strong());
            ui.label(RichText::new(note).color(MUTED).size(10.0));
        });
}

impl eframe::App for Calculator {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_results();
        // Input edits occur only on the Settings page; game / model selection
        // is global and invalidates the calculation regardless of current view.
        if self.header(ctx) {
            self.schedule_recalculation();
        }
        self.sidebar(ctx);
        let mut settings_changed = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(8))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(5.0);
                        match self.view_mode {
                            ViewMode::Calculator => {
                                self.dashboard(ui);
                            }
                            ViewMode::Settings => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("SIMULATION SETTINGS")
                                            .color(PRIMARY).size(10.0).strong());
                                        ui.heading(RichText::new("시뮬레이션 설정")
                                            .size(27.0).strong());
                                        ui.label(RichText::new(
                                            "목표, 천장, 보유 재화와 게임별 환급 가정을 변경합니다."
                                        ).color(MUTED));
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("대시보드 보기 →").clicked() {
                                                self.view_mode = ViewMode::Calculator;
                                            }
                                        },
                                    );
                                });
                                ui.add_space(20.0);
                                settings_changed |= self.targets(ui);
                                ui.add_space(12.0);
                                settings_changed |= self.pity(ui);
                                ui.add_space(12.0);
                                settings_changed |= self.resources(ui);
                                ui.add_space(12.0);
                                settings_changed |= self.feedback_controls(ui);
                            }
                            ViewMode::TrackerSources => {
                                page_intro(ui, "TRACKER EVIDENCE",
                                    "트래커 교차 검증", "기준 확률과 날짜가 표시된 외부 관측값을 비교합니다.");
                                self.tracker_sources(ui);
                            }
                            ViewMode::ObservedDistribution => {
                                page_intro(ui, "OBSERVATION DATA",
                                    "CSV 실측 분석", "로컬에서 스택별 관측 데이터를 분석합니다.");
                                self.observation_validation(ui);
                            }
                        }
                        ui.add_space(12.0);
                    });
            });
        if settings_changed {
            self.schedule_recalculation();
        }
        if let Some(edited_at) = self.pending_since {
            if edited_at.elapsed() >= Duration::from_millis(300) {
                self.pending_since = None;
                self.recalculate();
            } else {
                ctx.request_repaint_after(Duration::from_millis(80));
            }
        }
        if self.calculating {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
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

/// One theme and logical-point typography for Windows and Linux.
/// OS-level DPI still controls physical pixel density, but widget dimensions,
/// colors and CDF Painter are identical in egui points.
fn configure_dark_desktop(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = CARD;
    visuals.extreme_bg_color = Color32::from_rgb(17, 25, 40);
    visuals.override_text_color = Some(Color32::from_rgb(231, 239, 249));
    visuals.widgets.noninteractive.fg_stroke.color = MUTED;
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 58, 79);
    visuals.widgets.active.bg_fill = Color32::from_rgb(43, 73, 99);
    visuals.selection.bg_fill = Color32::from_rgb(39, 102, 158);
    // Both dark visual slots and the current theme are explicitly set so a
    // Windows light OS theme won't reintroduce light native widget styling.
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(7.0, 5.0);
        style.spacing.button_padding = egui::vec2(9.0, 4.0);
        style.spacing.interact_size.y = 25.0;
        style.text_styles.insert(egui::TextStyle::Body,
            egui::FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Button,
            egui::FontId::proportional(13.0));
        style.text_styles.insert(egui::TextStyle::Small,
            egui::FontId::proportional(11.0));
    });
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
            .with_inner_size([1240.0, 800.0])
            .with_min_inner_size([820.0, 620.0])
            .with_title("Gacha Lab — Rust 확률 분석기"),
        ..Default::default()
    };
    eframe::run_native("Gacha Lab", options,
        Box::new(|cc| Ok(Box::new(Calculator::new(cc)))))
}
#[cfg(test)]
mod chart_tests {
    use super::*;
    #[test]
    fn sample_curve_has_endpoints_and_monotonic_probabilities() {
        let dist = Distribution::new(vec![0.0, 0.2, 0.3, 0.5]);
        let curve = sampled_cdf(&dist, 3);
        assert_eq!(curve.first().copied(), Some((0, 0.0)));
        assert_eq!(curve.last().copied(), Some((3, 1.0)));
        assert!(curve.windows(2).all(|x| x[0].0 < x[1].0 && x[0].1 <= x[1].1));
        let zero_goal = Distribution::new(vec![1.0]);
        let immediate = sampled_cdf(&zero_goal, 0);
        assert_eq!(immediate, vec![(0, 1.0), (1, 1.0)]);
    }

    #[test]
    fn character_and_weapon_chart_ranges_are_independent() {
        let character = Distribution::new(vec![0.0, 0.3, 0.7]);
        let mut weapon_pmf = vec![0.0; 301];
        weapon_pmf[300] = 1.0;
        let weapon = Distribution::new(weapon_pmf);
        let character_only = [ChartSeries {
            label: "캐릭터", source: ChartSource::Exact(&character), color: PRIMARY,
        }];
        let weapon_only = [ChartSeries {
            label: "무기", source: ChartSource::Exact(&weapon), color: MINT,
        }];
        assert_eq!(chart_domain_end(&character_only), 2);
        assert_eq!(chart_domain_end(&weapon_only), 300);
    }

    #[test]
    fn long_plot_curve_remains_bounded_and_includes_last_pity() {
        let mut pmf = vec![0.0; 1600];
        pmf[1599] = 1.0;
        let curve = sampled_cdf(&Distribution::new(pmf), 1599);
        assert_eq!(curve.len(), 401);
        assert_eq!(curve.last(), Some(&(1599, 1.0)));
    }
}

