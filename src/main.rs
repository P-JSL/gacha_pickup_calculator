#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

use eframe::egui::{self, Color32, RichText};
use egui_plot::{Legend, Line, Plot, PlotPoints};
use gacha_pickup_calculator::feedback::{estimate, FeedbackConfig, FeedbackEstimate};
use gacha_pickup_calculator::games::{Game, GameSetup};
use gacha_pickup_calculator::{optimal_allocation, BannerRule, Distribution, RateMode};
use gacha_pickup_calculator::tracker::{expected_for_banner, references, TrackedBanner};
use gacha_pickup_calculator::observations::{parse_observation_csv, ObservationDataset};
use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

mod dashboard_ui;
mod catalog;
mod i18n;
use i18n::{Language, LanguagePreference, t};

const BG: Color32 = Color32::from_rgb(5, 23, 34);
const CARD: Color32 = Color32::from_rgb(17, 42, 63);
const MUTED: Color32 = Color32::from_rgb(166, 194, 209);
const PRIMARY: Color32 = Color32::from_rgb(60, 207, 242);
const MINT: Color32 = Color32::from_rgb(142, 234, 171);
const GOLD: Color32 = Color32::from_rgb(244, 210, 75);
const BORDER: Color32 = Color32::from_rgb(43, 80, 107);
const SIDEBAR: Color32 = Color32::from_rgb(3, 20, 30);
const SELECTED_NAV: Color32 = Color32::from_rgb(30, 77, 112);
const SURFACE: Color32 = Color32::from_rgb(12, 35, 54);

/// Paint navigation icons with egui primitives rather than OS/font-specific glyphs.
fn draw_nav_icon(painter: &egui::Painter, center: egui::Pos2, view: ViewMode, color: Color32) {
    let p = |x: f32, y: f32| egui::pos2(center.x + x, center.y + y);
    let stroke = egui::Stroke::new(1.8_f32, color);
    match view {
        ViewMode::Calculator => {
            painter.line_segment([p(-8.0, -1.0), p(0.0, -8.0)], stroke);
            painter.line_segment([p(0.0, -8.0), p(8.0, -1.0)], stroke);
            painter.line_segment([p(-6.0, -1.0), p(-6.0, 7.0)], stroke);
            painter.line_segment([p(-6.0, 7.0), p(6.0, 7.0)], stroke);
            painter.line_segment([p(6.0, 7.0), p(6.0, -1.0)], stroke);
        }
        ViewMode::Settings | ViewMode::Preferences => {
            for (y, x) in [(-6.0, -3.0), (0.0, 5.0), (6.0, -1.0)] {
                painter.line_segment([p(-8.0, y), p(8.0, y)], stroke);
                painter.circle_filled(p(x, y), 2.6, color);
            }
        }
        ViewMode::Currency => {
            painter.circle_stroke(p(-2.0, -2.0), 6.5, stroke);
            painter.circle_stroke(p(3.0, 3.5), 5.5, stroke);
        }
        ViewMode::Characters => {
            painter.circle_stroke(p(-3.5, -4.0), 3.6, stroke);
            painter.circle_stroke(p(5.5, -3.0), 2.7, stroke);
            painter.line_segment([p(-9.0, 7.0), p(-8.0, 2.0)], stroke);
            painter.line_segment([p(-8.0, 2.0), p(2.0, 2.0)], stroke);
            painter.line_segment([p(2.0, 2.0), p(4.0, 7.0)], stroke);
            painter.line_segment([p(5.0, 2.0), p(9.0, 2.0)], stroke);
            painter.line_segment([p(9.0, 2.0), p(10.0, 7.0)], stroke);
        }
        ViewMode::Weapons => {
            painter.line_segment([p(-7.0, -7.0), p(7.0, 7.0)], stroke);
            painter.line_segment([p(-7.0, 7.0), p(7.0, -7.0)], stroke);
            painter.line_segment([p(-7.0, -7.0), p(-3.0, -7.0)], stroke);
            painter.line_segment([p(7.0, -7.0), p(3.0, -7.0)], stroke);
        }
        ViewMode::Analytics => {
            for (x, h) in [(-7.0, 7.0), (-1.0, 12.0), (5.0, 16.0)] {
                painter.rect_filled(
                    egui::Rect::from_min_size(p(x, 8.0 - h), egui::vec2(3.5, h)),
                    1.0, color,
                );
            }
        }
        ViewMode::TrackerSources => {
            painter.line_segment([p(-7.0, -6.0), p(7.0, -6.0)], stroke);
            painter.line_segment([p(-7.0, 0.0), p(7.0, 0.0)], stroke);
            painter.line_segment([p(-7.0, 6.0), p(5.0, 6.0)], stroke);
        }
        ViewMode::ObservedDistribution => {
            for x in [-6.0, 0.0, 6.0] {
                for y in [-6.0, 0.0, 6.0] {
                    painter.rect_filled(
                        egui::Rect::from_center_size(p(x, y), egui::vec2(3.5, 3.5)),
                        0.5, color,
                    );
                }
            }
        }
    }
}

/// Crisp DPI-independent sci-fi star orbit and gacha pass artwork.
/// All geometry is rendered as vectors. No blurry upscaled raster crops.
fn paint_navigation_art(ui: &egui::Ui) {
    let rect = ui.max_rect().expand(11.0);
    let painter = ui.painter_at(rect);
    let cyan = Color32::from_rgb(99, 232, 251);
    let ice = Color32::from_rgb(195, 228, 255);
    let navy = Color32::from_rgb(8, 40, 65);
    let center = egui::pos2(rect.center().x, rect.top() + 79.0);
    painter.rect_filled(
        egui::Rect::from_min_max(rect.min,
            egui::pos2(rect.right(), rect.top()+133.0)), 0.0, navy,
    );
    // Tilted orbit around the top four-point gacha sparkle.
    let orbit: Vec<egui::Pos2> = (0..=84).map(|i| {
        let t = std::f32::consts::TAU * (i as f32) / 84.0;
        let (x, y) = (74.0*t.cos(), 24.0*t.sin());
        egui::pos2(center.x + x*0.92-y*0.40, center.y + x*0.40+y*0.92)
    }).collect();
    painter.add(egui::Shape::line(orbit,
        egui::Stroke::new(1.45_f32, Color32::from_rgb(84, 157, 214))));
    let star = |r: f32| -> Vec<egui::Pos2> {
        (0..8).map(|i| {
            let t = std::f32::consts::TAU*i as f32/8.0;
            let radius = if i%2==0 { r } else { r*0.19 };
            egui::pos2(center.x + t.sin()*radius, center.y-t.cos()*radius)
        }).collect()
    };
    painter.add(egui::Shape::convex_polygon(star(28.0),
        cyan, egui::Stroke::new(1.3_f32,ice)));
    painter.circle_filled(egui::pos2(center.x+41.0,center.y-39.0),2.0,ice);
    painter.circle_filled(egui::pos2(center.x-65.0,center.y+41.0),1.7,cyan);

    let ground = egui::Rect::from_min_max(
        egui::pos2(rect.left(),rect.bottom()-286.0),rect.right_bottom());
    painter.rect_filled(ground,0.0,Color32::from_rgb(6,34,57));
    let p = |x: f32,y: f32| egui::pos2(rect.center().x+x,ground.bottom()+y);
    let arc: Vec<egui::Pos2> = (0..=48).map(|i| {
        let t = i as f32/48.0*std::f32::consts::PI*1.15;
        p(-58.0 + 98.0*t.cos(), -81.0 - 96.0*t.sin())
    }).collect();
    painter.add(egui::Shape::line(arc,
        egui::Stroke::new(1.2_f32,Color32::from_rgb(54,131,192))));
    // A floating glossy draw pass; corners and edges stay sharp at any DPI.
    let card = [p(-76.0,-79.0),p(45.0,-101.0),p(68.0,-43.0),p(-54.0,-22.0)];
    painter.add(egui::Shape::convex_polygon(card.to_vec(),
        Color32::from_rgb(36,85,144),
        egui::Stroke::new(2.2_f32,Color32::from_rgb(132,198,252))));
    let inner = [p(-62.0,-70.0),p(34.0,-87.0),p(50.0,-49.0),p(-44.0,-33.0)];
    painter.add(egui::Shape::convex_polygon(inner.to_vec(),
        Color32::from_rgb(25,56,110),
        egui::Stroke::new(1.0_f32,ice)));
    painter.line_segment([p(-8.0,-78.0),p(4.0,-60.0)],
        egui::Stroke::new(3.0_f32,cyan));
    painter.line_segment([p(4.0,-60.0),p(22.0,-64.0)],
        egui::Stroke::new(2.0_f32,ice));
    for (x,y,r) in [(-94.0,-172.0,2.7),(86.0,-147.0,3.3),
                     (-58.0,-237.0,2.0),(70.0,-260.0,2.0)] {
        painter.circle_filled(p(x,y),r,ice);
        painter.line_segment([p(x-6.0,y),p(x+6.0,y)],
            egui::Stroke::new(0.7_f32,cyan));
        painter.line_segment([p(x,y-6.0),p(x,y+6.0)],
            egui::Stroke::new(0.7_f32,cyan));
    }
}

fn truncated_selection(ui: &egui::Ui, name: &str, button_width: f32) -> String {
    let font = egui::FontId::proportional(13.0);
    let max_width = (button_width - 35.0).max(5.0);
    let fits = |s: &str| ui.fonts(|fonts| {
        fonts.layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE).size().x <= max_width
    });
    if fits(name) { return name.to_owned(); }
    let chars: Vec<char> = name.chars().collect();
    let (mut low, mut high) = (0usize, chars.len());
    while low < high {
        let middle = low + (high - low + 1) / 2;
        let prefix: String = chars[..middle].iter().collect();
        if fits(&format!("{prefix}...")) { low = middle; }
        else { high = middle - 1; }
    }
    format!("{}...", chars[..low].iter().collect::<String>())
}

fn sidebar_nav(ui: &mut egui::Ui, selected: &mut ViewMode,
    view: ViewMode, text: &str) {
    let active = *selected == view;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 44.0), egui::Sense::click(),
    );
    if active {
        ui.painter().rect_filled(rect, 10.0, SELECTED_NAV);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 10.0, CARD);
    }
    let color = if active { Color32::WHITE }
        else { Color32::from_rgb(220, 235, 246) };
    draw_nav_icon(ui.painter(),
        egui::pos2(rect.left()+21.0,rect.center().y), view, color);
    ui.painter().text(
        egui::pos2(rect.left()+43.0,rect.center().y),
        egui::Align2::LEFT_CENTER, text,
        egui::FontId::proportional(14.0), color,
    );
    if response.clicked() {
        *selected = view;
    }
}

/// All quick/detailed ticket editors update the exact same GameSetup fields.
#[derive(Clone, Copy)]
enum TicketRole { Shared, Character, Weapon }

fn edit_ticket_inventory(ui: &mut egui::Ui, setup: &mut GameSetup, role: TicketRole) -> bool {
    match role {
        TicketRole::Shared =>
            ui.add(egui::DragValue::new(&mut setup.flexible)
                .range(0..=100_000).speed(1).suffix("장")).changed(),
        TicketRole::Character =>
            ui.add(egui::DragValue::new(&mut setup.character_dedicated)
                .range(0..=100_000).speed(1).suffix("장")).changed(),
        TicketRole::Weapon => {
            if setup.game == Game::StellaSora {
                let mut discs = setup.weapon_ticket_inventory();
                let changed = ui.add(egui::DragValue::new(&mut discs)
                    .range(0..=30_000_000).speed(1).suffix("개")).changed();
                if changed { setup.set_weapon_ticket_inventory(discs); }
                changed
            } else {
                ui.add(egui::DragValue::new(&mut setup.weapon_dedicated)
                    .range(0..=100_000).speed(1).suffix("개")).changed()
            }
        }
    }
}

/// Match simulation assumptions across all three WuWa coral strategies.
fn wuwa_strategy_table(ui: &mut egui::Ui, strategies: &[FeedbackEstimate],
    setup: &GameSetup, confidence: usize) {
    let owned = setup.available_flexible();
    let threshold = confidence as f64 / 100.0;
    section(ui, "WU", "명조 · 산호 투자 전략 3종 비교",
        "환급 재투입까지 계산한 몬테카를로 비교입니다. 돌파 재료는 산호를 확보한 이후 구매합니다.", |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("wuwa_comparison_table").striped(true)
                    .min_col_width(95.0).spacing([14.0, 9.0]).show(ui, |ui| {
                        for title in ["산호 사용", &format!("{}% 필요 자유뽑", confidence),
                            "추가 별의 소리", "현재 재화 성공률", "평균 총 뽑", "평균 돌파 구매"] {
                            ui.label(RichText::new(title).strong());
                        }
                        ui.end_row();
                        for (count, result) in strategies.iter().enumerate() {
                            let needed = result.quantile(threshold);
                            let extra = setup.additional_currency_for(needed);
                            let chance = result.probability(owned).0;
                            ui.label(match count {
                                0 => "뽑기권 전환", 1 => "돌파 최대 1개",
                                _ => "돌파 최대 2개",
                            });
                            ui.label(format!("{needed}뽑"));
                            ui.label(format!("{extra}개"));
                            ui.label(format!("{:.2}%", chance * 100.0));
                            ui.label(format!("{:.1}뽑", result.average_raw));
                            ui.label(format!("{:.2}개", result.average_wavebands_purchased));
                            ui.end_row();
                        }
                    });
            });
            ui.small("필요 자유뽑은 별도 전용권을 제외한 최초 뽑기 예산입니다. 확률은 표본 기반 추정치입니다.");
        });
}

struct Computed {
    character: Distribution,
    weapon: Distribution,
    combined: Distribution,
    feedback: Option<FeedbackEstimate>,
    wuwa_strategies: Option<Vec<FeedbackEstimate>>,
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
    let (feedback, wuwa_strategies) = if request.feedback_config.enabled
        && request.setup.game == Game::WutheringWaves
    {
        let mut strategies = Vec::with_capacity(3);
        for max_wavebands in 0..=2 {
            let mut variant = request.feedback_config.clone();
            variant.wuwa_waveband_max = max_wavebands;
            strategies.push(estimate(&request.setup, request.mode, &variant)?);
        }
        let selected = strategies[request.feedback_config.wuwa_waveband_max.min(2)].clone();
        (Some(selected), Some(strategies))
    } else if request.feedback_config.enabled {
        (Some(estimate(&request.setup, request.mode, &request.feedback_config)?), None)
    } else {
        (None, None)
    };
    Ok(Computed { character, weapon, combined, feedback, wuwa_strategies })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueryMode { RequiredBudget, AvailableBudget }

/// Keep the primary calculator uncluttered. Advanced evidence inspection is
/// explicitly opened from the header rather than appended to every result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode { Calculator, Analytics, Settings, Preferences, Currency, Characters, Weapons, TrackerSources, ObservedDistribution }

/// Desktop display and interaction options. These never change the probability model.
#[derive(Clone, Copy)]
struct UiPreferences {
    auto_recalculate: bool,
    debounce_ms: u64,
    show_result_panel: bool,
    show_chart_fill: bool,
    show_hover_guides: bool,
    compact_spacing: bool,
}
impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            auto_recalculate: false,
            debounce_ms: 350,
            show_result_panel: true,
            show_chart_fill: true,
            show_hover_guides: true,
            compact_spacing: true,
        }
    }
}

/// Each game's currency, targets and selected artwork are independent.
#[derive(Clone)]
struct GameSession {
    setup: GameSetup,
    feedback: FeedbackConfig,
    eid: usize,
    refine: usize,
    character_id: String,
    weapon_id: String,
}

impl GameSession {
    fn defaults(game: Game) -> Self {
        Self {
            setup: GameSetup::new(game),
            feedback: FeedbackConfig::for_game(game),
            eid: 0, refine: 1,
            character_id: catalog::first(game, catalog::Kind::Character),
            weapon_id: catalog::first(game, catalog::Kind::Weapon),
        }
    }
}

struct HistoryEntry {
    game: Game,
    eid: usize,
    refine: usize,
    chance: f64,
}

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
    stats_game: Option<Game>,
    input_needs_calculation: bool,
    manual_calculation_requested: bool,
    ui_preferences: UiPreferences,
    language_preference: LanguagePreference,
    system_language: Language,
    language_save_error: Option<String>,
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
    history: Vec<HistoryEntry>,
    show_history: bool,
    search_query: String,
    catalog_ui: catalog::CatalogUi,
    picker_kind: Option<catalog::Kind>,
    picker_filter: String,
    selected_character_id: String,
    selected_weapon_id: String,
    game_sessions: HashMap<Game, GameSession>,
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
            stats_game: None,
            input_needs_calculation: false,
            manual_calculation_requested: false,
            ui_preferences: UiPreferences::default(),
            language_preference: i18n::load_preference(),
            system_language: i18n::system_language(),
            language_save_error: None,
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
            history: Vec::new(),
            show_history: false,
            search_query: String::new(),
            catalog_ui: catalog::CatalogUi::new(&cc.egui_ctx),
            picker_kind: None,
            picker_filter: String::new(),
            selected_character_id: catalog::first(Game::StarRail, catalog::Kind::Character),
            selected_weapon_id: catalog::first(Game::StarRail, catalog::Kind::Weapon),
            game_sessions: HashMap::new(),
        };
        app.recalculate();
        app
    }

    fn language(&self) -> Language {
        self.language_preference.effective(self.system_language)
    }
    fn set_language(&mut self, preference: LanguagePreference) {
        if self.language_preference == preference { return; }
        self.language_preference = preference;
        self.language_save_error = i18n::save_preference(preference).err().map(|e| e.to_string());
    }
    fn language_choices(&mut self, ui: &mut egui::Ui) {
        let active = self.language_preference;
        let language = self.language();
        for choice in LanguagePreference::ALL {
            if ui.selectable_label(choice == active, choice.display(self.system_language, language)).clicked() {
                self.set_language(choice);
                ui.close_menu();
            }
        }
    }

    /// Last completed charts remain visible until the worker finishes for the same game.
    /// A game change never displays distributions from a different game.
    fn visible_stats(&self) -> Option<&Computed> {
        if self.stats_game == Some(self.setup.game) { self.stats.as_ref() } else { None }
    }

    /// Selection controls all reuse the catalog; target rarity remains a five-star model.
    fn catalog_selector(&mut self, ui: &mut egui::Ui, kind: catalog::Kind) {
        let game = self.setup.game;
        let id = match kind {
            catalog::Kind::Character => self.selected_character_id.clone(),
            catalog::Kind::Weapon => self.selected_weapon_id.clone(),
        };
        let item = catalog::by_id(game, kind, &id);
        ui.horizontal(|ui| {
            if let Some(item) = item {
                self.catalog_ui.image(ui, item, 30.0);
            }
            let name = item.map(|x| x.name.as_str()).unwrap_or(t(self.language(), "No target"));
            let width = ui.available_width().min(230.0).max(70.0);
            let shortened = truncated_selection(ui, name, width);
            if ui.add_sized(egui::vec2(width, 27.0),
                egui::Button::new(format!("{shortened}  ▾")))
                .on_hover_text(name).clicked() {
                self.picker_kind = Some(kind);
                self.picker_filter.clear();
            }
        });
    }

    fn show_catalog_picker(&mut self, ctx: &egui::Context) {
        let Some(kind) = self.picker_kind else { return; };
        let mut open = true;
        let current = match kind {
            catalog::Kind::Character => &self.selected_character_id,
            catalog::Kind::Weapon => &self.selected_weapon_id,
        }.clone();
        let choice = catalog::picker(ctx, &mut self.catalog_ui, self.setup.game, kind,
            &mut open, &mut self.picker_filter, &current);
        if let Some(id) = choice {
            match kind {
                catalog::Kind::Character => self.selected_character_id = id,
                catalog::Kind::Weapon => self.selected_weapon_id = id,
            }
            self.picker_kind = None;
        } else if !open {
            self.picker_kind = None;
        }
    }

    fn change_game(&mut self, game: Game) {
        if self.setup.game != game {
            // Save all editable fields so switching between four games never loses
            // inventory, target goals, pity state, selected item or refund assumptions.
            self.game_sessions.insert(self.setup.game, GameSession {
                setup: self.setup.clone(),
                feedback: self.feedback_cfg.clone(),
                eid: self.target_eid,
                refine: self.target_refine,
                character_id: self.selected_character_id.clone(),
                weapon_id: self.selected_weapon_id.clone(),
            });
            let next = self.game_sessions.remove(&game)
                .unwrap_or_else(|| GameSession::defaults(game));
            self.setup = next.setup;
            self.feedback_cfg = next.feedback;
            self.target_eid = next.eid;
            self.target_refine = next.refine;
            self.selected_character_id = next.character_id;
            self.selected_weapon_id = next.weapon_id;
        } else {
            // The reset action invokes change_game(current_game) to restore this
            // game's default inputs without touching saved values for other games.
            self.game_sessions.remove(&game);
            let defaults = GameSession::defaults(game);
            self.setup = defaults.setup;
            self.feedback_cfg = defaults.feedback;
            self.target_eid = defaults.eid;
            self.target_refine = defaults.refine;
            self.selected_character_id = defaults.character_id;
            self.selected_weapon_id = defaults.weapon_id;
        }
        self.picker_kind = None;
        self.picker_filter.clear();
        self.observation_data = None;
        self.observation_path.clear();
        self.observation_message.clear();
    }

    fn recalculate(&mut self) {
        self.setup.set_targets(self.target_eid + 1, self.target_refine);
        self.generation = self.generation.wrapping_add(1);
        self.calculating = true;
        // Keep the last completed image on screen during background calculation.
        self.input_needs_calculation = false;
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
                Ok(stats) => {
                    let available = self.setup.available_flexible();
                    let raw = optimal_allocation(
                        &stats.character, &stats.weapon,
                        self.setup.character_dedicated, self.setup.weapon_dedicated, available,
                    );
                    let chance = stats.feedback.as_ref()
                        .map(|f| f.probability(available).0)
                        .unwrap_or(raw.success_probability);
                    self.history.insert(0, HistoryEntry {
                        game: self.setup.game, eid: self.target_eid,
                        refine: self.target_refine, chance,
                    });
                    self.history.truncate(5);
                    self.stats = Some(stats);
                    self.stats_game = Some(self.setup.game);
                    self.input_needs_calculation = false;
                    self.error = None;
                }
                Err(message) => { self.error = Some(message); }
            }
        }
    }

    fn schedule_recalculation(&mut self) {
        self.pending_since = Some(Instant::now());
        self.error = None;
        self.calculating = true;
        self.input_needs_calculation = true;
    }

    /// Enter-driven navigation search; only actual sections and supported games are matched.
    fn activate_search(&mut self) -> bool {
        let query = self.search_query.trim().to_lowercase();
        if query.is_empty() {
            return false;
        }
        let mut game_changed = false;
        let game = if query.contains("스타레일") || query.contains("hsr") {
            Some(Game::StarRail)
        } else if query.contains("명조") || query.contains("wuthering") {
            Some(Game::WutheringWaves)
        } else if query.contains("스텔라") || query.contains("stella") {
            Some(Game::StellaSora)
        } else if query.contains("이환") || query.contains("yihuan") || query.contains("neverness") {
            Some(Game::Neverness)
        } else {
            None
        };
        if let Some(game) = game {
            if self.setup.game != game {
                self.change_game(game);
                game_changed = true;
            }
            self.view_mode = ViewMode::Calculator;
        } else if query.contains("currency") || query.contains("재화") || query.contains("refund") || query.contains("환급") {
            self.view_mode = ViewMode::Currency;
        } else if query.contains("character") || query.contains("캐릭터") {
            self.view_mode = ViewMode::Characters;
        } else if query.contains("weapon") || query.contains("무기") || query.contains("광추")
            || query.contains("arc") || query.contains("레코드") {
            self.view_mode = ViewMode::Weapons;
        } else if query.contains("tracker") || query.contains("트래커") {
            self.view_mode = ViewMode::TrackerSources;
        } else if query.contains("csv") || query.contains("실측") {
            self.view_mode = ViewMode::ObservedDistribution;
        } else if query.contains("analytics") || query.contains("분석") {
            self.view_mode = ViewMode::Analytics;
        } else if query.contains("settings") || query.contains("환경") || query.contains("화면 설정") {
            self.view_mode = ViewMode::Preferences;
        } else if query.contains("simulator") || query.contains("시뮬레이션") {
            self.view_mode = ViewMode::Settings;
        } else if query.contains("dashboard") || query.contains("대시보드") {
            self.view_mode = ViewMode::Calculator;
        }
        self.search_query.clear();
        game_changed
    }

    /// Compact reference-style header with functional search and utilities.
    fn header(&mut self, ctx: &egui::Context) -> bool {
        let mut dirty = false;
        egui::TopBottomPanel::top("header")
            .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(egui::Margin { left: 15, right: 15, top: 15, bottom: 0 }))
            .show(ctx, |ui| {
                ui.add_space(30.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Gacha Pickup Calculator")
                            .strong().size(23.0));
                        ui.label(RichText::new("Probability & Refund Simulator")
                            .color(MUTED).size(12.0));
                    });
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Native vector settings button replaces the removed avatar.
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(39.0, 39.0), egui::Sense::click(),
                            );
                            ui.painter().rect_filled(rect, 10.0,
                                if response.hovered() { SELECTED_NAV } else { CARD });
                            let center = rect.center();
                            let stroke = egui::Stroke::new(1.8_f32, Color32::WHITE);
                            ui.painter().circle_stroke(center, 6.0, stroke);
                            ui.painter().circle_stroke(center, 2.5, stroke);
                            for i in 0..8 {
                                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                                let direction = egui::vec2(angle.cos(), angle.sin());
                                ui.painter().line_segment(
                                    [center + direction * 9.0, center + direction * 12.0],
                                    stroke,
                                );
                            }
                            if response.on_hover_text(t(self.language(), "Display & behavior settings")).clicked() {
                                self.view_mode = ViewMode::Preferences;
                            }

                            if ui.add(egui::Button::new("?")
                                .fill(CARD).corner_radius(10)
                                .min_size(egui::vec2(39.0, 39.0))).clicked() {
                                self.show_help = true;
                            }
                            ui.menu_button("♧", |ui| {
                                ui.label(t(self.language(), "Recent simulations"));
                                ui.label(format!("현재 세션 기록 {}건", self.history.len()));
                                if ui.button(t(self.language(), "View all history")).clicked() {
                                    self.show_history = true;
                                    ui.close_menu();
                                }
                            });
                            ui.menu_button("☆", |ui| {
                                ui.label(t(self.language(), "Select game"));
                                for game in Game::ALL {
                                    if ui.button(i18n::game_name(self.language(), game)).clicked() {
                                        if self.setup.game != game {
                                            self.change_game(game);
                                            dirty = true;
                                        }
                                        self.view_mode = ViewMode::Calculator;
                                        ui.close_menu();
                                    }
                                }
                            });
                            ui.menu_button(format!("{} ▾", self.language().code()), |ui| {
                                self.language_choices(ui);
                            });
                            ui.add_space(8.0);
                            let search = ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .desired_width(ui.available_width().min(340.0).max(60.0))
                                    .hint_text(format!("⌕  {}", t(self.language(), "Search games or sections...")))
                                    .margin(egui::Margin::symmetric(10, 8))
                            );
                            if search.lost_focus()
                                && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                                dirty |= self.activate_search();
                            }
                        },
                    );
                });
            });
        dirty
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        let language = self.language();
        egui::SidePanel::left("workspace_sidebar")
            .resizable(false)
            .exact_width(222.0)
            .frame(egui::Frame::default().fill(SIDEBAR).inner_margin(11))
            .show(ctx, |ui| {
                paint_navigation_art(ui);
                ui.add_space(136.0);
                for (view, label) in [
                    (ViewMode::Calculator, "Dashboard"),
                    (ViewMode::Settings, "Simulator"),
                    (ViewMode::Currency, "Currency"),
                    (ViewMode::Characters, "Characters"),
                    (ViewMode::Weapons, "Weapons"),
                    (ViewMode::Analytics, "Analytics"),
                ] {
                    sidebar_nav(ui, &mut self.view_mode, view, t(language, label));
                    ui.add_space(9.0);
                }
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(13.0);
                sidebar_nav(ui, &mut self.view_mode, ViewMode::Preferences, t(language, "Settings"));
            });
    }

    /// UI-only preferences are independent of the gacha input/worker state.
    fn desktop_settings(&mut self, ui: &mut egui::Ui) {
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
        section(ui, "01", t(language, "Auto calculation"), t(language, "Calculating in background. Previous results remain visible."), |ui| {
            ui.checkbox(&mut self.ui_preferences.auto_recalculate,
                t(language, "Update results automatically when input changes"));
            ui.horizontal(|ui| {
                ui.label(t(language, "Input debounce"));
                ui.add(egui::Slider::new(&mut self.ui_preferences.debounce_ms, 100..=1000)
                    .step_by(50.0).suffix(" ms"));
            });
            if !self.ui_preferences.auto_recalculate {
                ui.label(RichText::new(
                    "자동 계산을 꺼도 입력값은 즉시 모든 탭에 반영됩니다. Run Simulation 버튼으로 계산을 갱신하세요."
                ).color(MUTED).size(11.0));
                if self.input_needs_calculation && ui.button(t(language, "Run calculation now")).clicked() {
                    self.manual_calculation_requested = true;
                }
            }
        });
        ui.add_space(10.0);
        section(ui, "02", t(language, "Display options"), t(language, "Change the appearance and calculation behavior, not banner inputs."), |ui| {
            ui.checkbox(&mut self.ui_preferences.show_result_panel, t(language, "Show right result/history panel"));
            ui.checkbox(&mut self.ui_preferences.show_chart_fill, t(language, "Fill area under probability curves"));
            ui.checkbox(&mut self.ui_preferences.show_hover_guides, t(language, "Show in-chart hover probability"));
            ui.checkbox(&mut self.ui_preferences.compact_spacing, t(language, "Compact spacing"));
        });
        ui.add_space(12.0);
        if ui.button(t(language, "Restore defaults")).clicked() {
            self.ui_preferences = UiPreferences::default();
        }
        ui.label(RichText::new(
            t(language, "Other display preferences reset at exit.")
        ).color(MUTED).size(11.0));
    }

    fn targets(&mut self, ui: &mut egui::Ui) -> bool {
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
    fn single_banner_page(&mut self, ui: &mut egui::Ui, weapon: bool) -> bool {
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

    fn resources(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        section(ui, "03", "보유 뽑기권 및 재화",
            "한정 픽업 배너의 전용 재화만 집계합니다. 일반 배너용 티켓은 합산하지 않습니다.", |ui| {
                let game = self.setup.game;
                if game.shares_featured_ticket() {
                    ui.label(RichText::new(format!(
                        "캐릭터·광추 공용 · {}", game.character_ticket_name()
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
                    ui.label(format!("공용 전환 재화 · {currency}"));
                    changed |= ui.add(egui::DragValue::new(&mut self.setup.premium_currency)
                        .range(0..=100_000_000).speed(1).suffix("개")).changed();
                    ui.label(RichText::new(format!(
                        "{}개 = 1뽑 · 전환 가능 {}뽑 (잔여 {}개)",
                        price, self.setup.premium_currency / price,
                        self.setup.premium_currency % price
                    )).color(MINT));
                });
                ui.label(RichText::new(
                    "전용권은 각 배너에만 사용하고, 공용 전환 재화는 계산 엔진이 두 배너에 배분합니다."
                ).color(MUTED).size(11.0));
            });
        changed
    }

    fn feedback_controls(&mut self, ui: &mut egui::Ui) -> bool {
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
                    });
            }
        });
        dirty
    }

    fn dashboard(&mut self, ui: &mut egui::Ui) {
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
                    }], self.ui_preferences);
                });
                section(&mut cols[1], "05-B", &secondary_heading, single_description, |ui| {
                    draw_probability_chart(ui, &[ChartSeries {
                        label: secondary_name, source: ChartSource::Exact(&stats.weapon), color: MINT,
                    }], self.ui_preferences);
                });
            });
        } else {
            section(ui, "05-A", "캐릭터 목표 확률", single_description, |ui| {
                draw_probability_chart(ui, &[ChartSeries {
                    label: "캐릭터", source: ChartSource::Exact(&stats.character), color: PRIMARY,
                }], self.ui_preferences);
            });
            ui.add_space(8.0);
            section(ui, "05-B", &secondary_heading, single_description, |ui| {
                draw_probability_chart(ui, &[ChartSeries {
                    label: secondary_name, source: ChartSource::Exact(&stats.weapon), color: MINT,
                }], self.ui_preferences);
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
            draw_probability_chart(ui, &curves, self.ui_preferences);
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
                            let metadata = std::fs::symlink_metadata(path)
                                .map_err(|e| format!("파일 확인 실패: {e}"))?;
                            if !metadata.file_type().is_file() {
                                return Err("심볼릭 링크·장치가 아닌 일반 CSV 파일만 허용합니다.".into());
                            }
                            const MAX_CSV_BYTES: u64 = 8 * 1024 * 1024;
                            if metadata.len() > MAX_CSV_BYTES {
                                return Err("CSV 크기는 8 MiB 이하로 제한됩니다.".into());
                            }
                            let file = std::fs::File::open(path)
                                .map_err(|e| format!("CSV 열기 실패: {e}"))?;
                            if !file.metadata().is_ok_and(|m| m.is_file()) {
                                return Err("CSV로 일반 파일만 허용합니다.".into());
                            }
                            // Bound the actual read; metadata can race with a
                            // file replacement or change after the initial check.
                            let mut bytes = Vec::new();
                            file.take(MAX_CSV_BYTES + 1).read_to_end(&mut bytes)
                                .map_err(|e| format!("CSV 읽기 실패: {e}"))?;
                            if bytes.len() as u64 > MAX_CSV_BYTES {
                                return Err("읽는 중 크기가 8 MiB를 초과했습니다.".into());
                            }
                            let contents = String::from_utf8(bytes)
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
fn draw_probability_chart(ui: &mut egui::Ui, series: &[ChartSeries<'_>], appearance: UiPreferences) {
    let end = chart_domain_end(series);
    let width = ui.available_width().max(100.0);
    let height = if width < 650.0 { 136.0 } else { 187.0 };
    let (response, painter) = ui.allocate_painter(
        egui::vec2(width, height), egui::Sense::hover(),
    );
    let bounds = response.rect;
    let graph = egui::Rect::from_min_max(
        egui::pos2(bounds.left() + 49.0, bounds.top() + 12.0),
        egui::pos2(bounds.right() - 10.0, bounds.bottom() - 34.0),
    );
    painter.rect_filled(bounds, 10.0, Color32::from_rgb(12, 35, 53));
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
    for step in 0..=5 {
        let probability = step as f64 / 5.0;
        let y = project(0, probability).y;
        painter.line_segment(
            [egui::pos2(graph.left(), y), egui::pos2(graph.right(), y)],
            egui::Stroke::new(1.0_f32, BORDER),
        );
        painter.text(
            egui::pos2(graph.left() - 7.0, y),
            egui::Align2::RIGHT_CENTER, format!("{}%", step * 20),
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
        // Area-gradient mesh: brighter at the CDF line, transparent at the baseline.
        if appearance.show_chart_fill && points.len() >= 2 {
            let mut fill = egui::Mesh::default();
            for point in &points {
                fill.vertices.push(egui::epaint::Vertex {
                    pos: *point, uv: egui::epaint::WHITE_UV,
                    color: line.color.linear_multiply(0.36),
                });
                fill.vertices.push(egui::epaint::Vertex {
                    pos: egui::pos2(point.x, graph.bottom()),
                    uv: egui::epaint::WHITE_UV, color: Color32::TRANSPARENT,
                });
            }
            for index in 0..points.len() - 1 {
                let start = (index * 2) as u32;
                fill.indices.extend_from_slice(&[
                    start, start + 1, start + 2,
                    start + 1, start + 3, start + 2,
                ]);
            }
            painter.add(egui::Shape::mesh(fill));
        }
        if let Some(end_point) = points.last() {
            painter.circle_filled(*end_point, 4.0, line.color);
        }
        painter.add(egui::Shape::line(
            points, egui::Stroke::new(2.6_f32, line.color),
        ));
    }
    if let Some(pointer) = response.hover_pos().filter(|pos| graph.contains(*pos)) {
        if appearance.show_hover_guides {
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
        let tooltip = format!(
            "{x}뽑 이내 · {}",
            series.iter().map(|line| format!("{} {:.1}%", line.label,
                line.source.probability(x) * 100.0)).collect::<Vec<_>>().join(" · ")
        );
        // Keep the exact point probability visible inside the graph as well
        // as in the native mouse tooltip, even on platforms where hover UI
        // can be delayed or positioned outside a small desktop viewport.
        let badge = egui::Rect::from_min_size(
            egui::pos2(graph.left() + 5.0, graph.top() + 5.0),
            egui::vec2((graph.width() - 10.0).max(12.0), 22.0));
        painter.rect_filled(badge, 5.0, Color32::from_rgb(18, 48, 69));
        let overlay = painter.with_clip_rect(badge.shrink(3.0));
        overlay.text(badge.left_center() + egui::vec2(5.0, 0.0),
            egui::Align2::LEFT_CENTER, &tooltip,
            egui::FontId::proportional(10.5), Color32::WHITE);
        response.clone().on_hover_text(tooltip);
        }
    }
    // The compact dashboard uses one line per card. Keep legends only where
    // multiple curves must be distinguished in the detailed analytics view.
    if series.len() > 1 {
        ui.horizontal_wrapped(|ui| {
            for line in series {
                ui.colored_label(line.color, format!("● {}", line.label));
            }
        });
    }
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
        self.catalog_ui.pump(ctx);
        // Compact dashboard edits and detailed settings use the same worker generation.
        // Reserve a full-height left rail first, as in the reference image.
        self.sidebar(ctx);
        if self.header(ctx) {
            self.input_needs_calculation = true;
            if self.ui_preferences.auto_recalculate {
                self.schedule_recalculation();
            } else {
                // Game switching and navigation never start a new expensive
                // simulation while automatic updates are disabled.
                self.pending_since = None;
                self.calculating = false;
                self.generation = self.generation.wrapping_add(1);
            }
        }
        // Reserve a usable central editing width. The optional right sidebar
        // collapses on smaller windows rather than overlapping input controls.
        if self.view_mode == ViewMode::Calculator && self.ui_preferences.show_result_panel
            && ctx.available_rect().width() >= 1_270.0 {
            self.workspace_results_panel(ctx);
        }
        let mut settings_changed = false;
        egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(BG).inner_margin(18))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.ui_preferences.compact_spacing {
                            ui.spacing_mut().item_spacing.y = 5.0;
                        } else {
                            ui.spacing_mut().item_spacing.y = 10.0;
                        }
                        if self.view_mode != ViewMode::Calculator { ui.add_space(5.0); }
                        match self.view_mode {
                            ViewMode::Calculator => {
                                settings_changed |= self.workspace_dashboard(ui);
                            }
                            ViewMode::Analytics => {
                                self.dashboard(ui);
                            }
                            ViewMode::Currency => {
                                page_intro(ui, "CURRENCY & REFUNDS",
                                    "재화 및 환급 설정",
                                    "전용권·공통 재화·환급 재투입 조건을 변경합니다.");
                                settings_changed |= self.resources(ui);
                                ui.add_space(10.0);
                                settings_changed |= self.feedback_controls(ui);
                            }
                            ViewMode::Characters | ViewMode::Weapons => {
                                page_intro(ui, "TARGET & PITY",
                                    if self.view_mode == ViewMode::Characters {
                                        "캐릭터 목표 관리"
                                    } else {
                                        "무기 · 광추 · 레코드 · Arc 목표 관리"
                                    },
                                    "선택한 배너만 편집하며, 입력값은 대시보드·시뮬레이터와 공유합니다.");
                                let weapon = self.view_mode == ViewMode::Weapons;
                                settings_changed |= self.single_banner_page(ui, weapon);
                            }
                            ViewMode::Preferences => {
                                self.desktop_settings(ui);
                            }
                            ViewMode::Settings => {
                                ui.horizontal_wrapped(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(RichText::new("SIMULATION SETTINGS")
                                            .color(PRIMARY).size(10.0).strong());
                                        ui.heading(RichText::new("시뮬레이션 설정")
                                            .size(19.0).strong());
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
                                ui.add_space(7.0);
                                ui.menu_button("확률 모델 ▾", |ui| {
                                    settings_changed |= ui.radio_value(&mut self.mode,
                                        RateMode::PublishedOnly, "공개 기본 확률 + 확정 천장").changed();
                                    settings_changed |= ui.radio_value(&mut self.mode,
                                        RateMode::CommunityEstimate, "소프트 천장 추정 모델").changed();
                                });
                                // Two compact input rows on desktop; no settings
                                // are removed, and narrow viewports stack safely.
                                if ui.available_width() >= 910.0 {
                                    ui.columns(2, |cols| {
                                        settings_changed |= self.targets(&mut cols[0]);
                                        settings_changed |= self.pity(&mut cols[1]);
                                    });
                                    ui.add_space(6.0);
                                    ui.columns(2, |cols| {
                                        settings_changed |= self.resources(&mut cols[0]);
                                        settings_changed |= self.feedback_controls(&mut cols[1]);
                                    });
                                } else {
                                    settings_changed |= self.targets(ui);
                                    ui.add_space(6.0);
                                    settings_changed |= self.pity(ui);
                                    ui.add_space(6.0);
                                    settings_changed |= self.resources(ui);
                                    ui.add_space(6.0);
                                    settings_changed |= self.feedback_controls(ui);
                                }
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
            self.input_needs_calculation = true;
            if !self.ui_preferences.auto_recalculate {
                // An edit invalidates any in-flight response, without starting
                // heavy computations or changing focus on each keystroke.
                self.generation = self.generation.wrapping_add(1);
                self.pending_since = None;
                self.calculating = false;
            }
        }
        if self.manual_calculation_requested {
            self.manual_calculation_requested = false;
            self.pending_since = None;
            self.recalculate();
        } else if settings_changed && self.ui_preferences.auto_recalculate {
            self.schedule_recalculation();
        }
        if let Some(edited_at) = self.pending_since {
            if edited_at.elapsed() >= Duration::from_millis(self.ui_preferences.debounce_ms) {
                self.pending_since = None;
                self.recalculate();
            } else {
                ctx.request_repaint_after(Duration::from_millis(80));
            }
        }
        if self.calculating {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
        self.show_catalog_picker(ctx);
        if self.show_history {
            egui::Window::new(t(self.language(), "Simulation History"))
                .open(&mut self.show_history)
                .default_size(egui::vec2(470.0, 330.0))
                .resizable(true).show(ctx, |ui| {
                    ui.label(RichText::new(
                        "현재 실행에서 계산한 최근 결과입니다. 종료 시 초기화됩니다."
                    ).color(MUTED).size(11.0));
                    ui.separator();
                    for result in &self.history {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(result.game.label()).strong());
                            ui.label(format!("E{} · {} S{}",
                                result.eid, result.game.secondary_banner_name(),result.refine));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(RichText::new(format!("{:.2}%",
                                        result.chance * 100.0)).color(MINT).strong());
                                });
                        });
                        ui.separator();
                    }
                });
        }
        if self.show_help {
            egui::Window::new(t(self.language(), "Rules, confidence and sources"))
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
            .with_inner_size([1536.0, 864.0])
            .with_min_inner_size([1080.0, 700.0])
            .with_title("Gacha Pickup Calculator"),
        ..Default::default()
    };
    eframe::run_native("Gacha Lab", options,
        Box::new(|cc| Ok(Box::new(Calculator::new(cc)))))
}
#[cfg(test)]
mod chart_tests {
    use super::*;
    #[test]
    fn automatic_recalculation_is_disabled_by_default() {
        let prefs = UiPreferences::default();
        assert!(!prefs.auto_recalculate);
        assert!(prefs.show_hover_guides);
    }

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

