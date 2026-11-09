//! Reference-inspired Windows/Linux dashboard. Probability and refund engines stay unchanged.
use super::*;

const CYAN: Color32 = Color32::from_rgb(50, 218, 241);
const YELLOW: Color32 = Color32::from_rgb(244, 210, 75);
const PINK: Color32 = Color32::from_rgb(225, 112, 232);
const SUCCESS: Color32 = Color32::from_rgb(141, 238, 117);
const DEEP_CARD: Color32 = Color32::from_rgb(16, 43, 64);

fn dashboard_card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(DEEP_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(13)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(15.0).strong());
            ui.add_space(9.0);
            content(ui);
        });
}

fn metric_icon(p: &egui::Painter, c: egui::Pos2, kind: usize, color: Color32) {
    let stroke = egui::Stroke::new(2.1_f32, color);
    let pos = |x: f32,y: f32| egui::pos2(c.x + x, c.y + y);
    match kind {
        0 => { // ticket outline
            for pair in [
                [pos(-13.0,-9.0),pos(11.0,-13.0)],
                [pos(11.0,-13.0),pos(14.0,8.0)],
                [pos(14.0,8.0),pos(-10.0,12.0)],
                [pos(-10.0,12.0),pos(-13.0,-9.0)],
                [pos(0.0,-10.0),pos(3.0,9.0)]
            ] { p.line_segment(pair, stroke); }
        }
        1 => {
            p.circle_stroke(c, 13.0, stroke);
            p.circle_stroke(c, 5.0, stroke);
            p.line_segment([pos(-18.0,0.0),pos(-9.0,0.0)], stroke);
            p.line_segment([pos(9.0,0.0),pos(18.0,0.0)], stroke);
            p.line_segment([pos(0.0,-18.0),pos(0.0,-9.0)], stroke);
            p.line_segment([pos(0.0,9.0),pos(0.0,18.0)], stroke);
        }
        2 => {
            for pair in [
                [pos(0.0,-15.0),pos(13.0,-7.0)],
                [pos(13.0,-7.0),pos(13.0,10.0)],
                [pos(13.0,10.0),pos(0.0,17.0)],
                [pos(0.0,17.0),pos(-13.0,10.0)],
                [pos(-13.0,10.0),pos(-13.0,-7.0)],
                [pos(-13.0,-7.0),pos(0.0,-15.0)],
                [pos(0.0,0.0),pos(13.0,-7.0)],
                [pos(0.0,0.0),pos(-13.0,-7.0)],
                [pos(0.0,0.0),pos(0.0,17.0)]
            ] { p.line_segment(pair, stroke); }
        }
        _ => {
            let points = vec![pos(0.0,-17.0),pos(12.0,-6.0),pos(6.0,13.0),
                pos(0.0,18.0),pos(-9.0,8.0),pos(-12.0,-6.0)];
            p.add(egui::Shape::convex_polygon(points,
                color.gamma_multiply(0.45), stroke));
            p.line_segment([pos(-12.0,-6.0),pos(12.0,-6.0)],stroke);
            p.line_segment([pos(0.0,-17.0),pos(0.0,18.0)],stroke);
        }
    }
}

/// Native widgets automatically wrap long text rather than overpainting peers.
fn overview_metric(ui: &mut egui::Ui, kind: usize, label: &str, value: String, note: String, color: Color32) {
    let base = if kind == 1 { Color32::from_rgb(14, 59, 76) }
               else if kind == 2 { Color32::from_rgb(35, 57, 94) }
               else { Color32::from_rgb(28, 63, 96) };
    egui::Frame::group(ui.style())
        .fill(base)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(114.0);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(42.0, 42.0), egui::Sense::hover());
                metric_icon(ui.painter(), rect.center(), kind, color);
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(label).size(11.0)).wrap());
                    ui.add(egui::Label::new(RichText::new(value).size(23.0).strong()).wrap());
                    ui.add(egui::Label::new(RichText::new(note).size(10.0).color(MUTED)).wrap());
                });
            });
        });
}

/// Full-width final joint success figure; both goals must complete in each
/// same Monte Carlo path (never multiply standalone banner percentages).
#[derive(Clone, Copy)]
struct FinalSuccessHero {
    game: Game,
    eid: usize,
    refine: usize,
    probability: f64,
    raw_probability: Option<f64>,
    sample_count: Option<usize>,
    available: usize,
}

fn final_success_hero(ui: &mut egui::Ui, result: FinalSuccessHero, language: Language) {
    let FinalSuccessHero {
        game, eid, refine, probability, raw_probability, sample_count, available,
    } = result;
    let title = match language {
        Language::Korean => "캐릭터 + 전용 장비 최종 목표 달성 확률",
        Language::English => "Final character + signature success probability",
        Language::Japanese => "キャラクター＋専用装備の最終達成確率",
        Language::ChineseSimplified => "角色＋专属装备最终达成概率",
        Language::ChineseTraditional => "角色＋專屬裝備最終達成機率",
    };
    dashboard_card(ui, title, |ui| {
        ui.add(egui::Label::new(format!(
            "E{eid} + {} S{refine}", i18n::banner_name(language, game)
        )).wrap());
        ui.label(RichText::new(format!("{:.2}%", probability * 100.0))
            .size(39.0).strong().color(SUCCESS));
        let info = match (sample_count, language) {
            (Some(n), Language::Korean) =>
                format!("{n}회 Monte Carlo · 최초 사용 가능 뽑기권 {available}장 · 환급 재투입 포함"),
            (Some(n), _) =>
                format!("{n} Monte Carlo trials · {available} initial tickets · refunds included"),
            (None, Language::Korean) =>
                format!("최초 사용 가능 뽑기권 {available}장 · 환급 미적용, 정확 DP 기준"),
            (None, _) =>
                format!("{available} initial tickets · no refunds (exact DP)"),
        };
        ui.add(egui::Label::new(RichText::new(info).color(MUTED).size(11.0)).wrap());
        if sample_count.is_some() {
            if let Some(raw) = raw_probability {
                ui.add(egui::Label::new(RichText::new(match language {
                    Language::Korean => format!("환급 전 참고 확률: {:.2}%", raw * 100.0),
                    _ => format!("Without-refund comparison: {:.2}%", raw * 100.0),
                }).color(MUTED).size(10.0)).wrap());
            }
        }
    });
}

/// Three mutually-exclusive stages from the SAME Monte Carlo samples.
fn refund_stages_card(ui: &mut egui::Ui, result: &BudgetSuccessBreakdown,
                      owned: usize, mean_raw: f64, average_refund_tickets: f64,
                      language: Language) {
    let (title, first, second, third, mean_title, all_refunds, subset) = match language {
        Language::Korean => (
            "보유권 소진 후 환급 재투입 결과", "기존 보유 뽑기권만으로 성공",
            "기존권 소진 후 환급으로 추가 성공", "환급까지 사용했지만 실패",
            "전체 평균 소요 뽑기", "전체 평균 생성 환급권", "환급 추가 성공 경로 평균 (추가 뽑기 / 생성 환급권)"),
        _ => (
            "Original tickets, then recycled refunds", "Success with original tickets",
            "Additional success using refunds", "Failed after refunds",
            "Mean raw draws", "Mean earned refund tickets", "Refund-only wins (extra draws / earned refund tickets)"),
    };
    dashboard_card(ui, title, |ui| {
        ui.add(egui::Label::new(match language {
            Language::Korean => format!(
                "보유 뽑기권 {owned}장을 우선 사용하고, 누적 교환 포인트로 추가 뽑기를 진행하는 결과입니다. ({}회 동일 표본)",
                result.samples),
            _ => format!(
                "Use {owned} original tickets first, then accumulated exchange points for extra draws ({} trials).",
                result.samples),
        }).wrap());
        ui.add_space(5.0);
        for (label, probability, color) in [
            (first, result.original_probability(), SUCCESS),
            (second, result.recycling_probability(), CYAN),
            (third, result.failure_probability(), Color32::from_rgb(210, 149, 91)),
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.label(label);
                ui.label(RichText::new(format!("{:.2}%", probability * 100.0))
                    .strong().color(color));
            });
            ui.add(egui::ProgressBar::new(probability as f32)
                .fill(color).desired_height(9.0));
            ui.add_space(4.0);
        }
        ui.separator();
        ui.label(format!("{mean_title}: {mean_raw:.1}"));
        ui.label(format!("{all_refunds}: {average_refund_tickets:.1}"));
        if result.after_recycling > 0 {
            ui.label(format!("{subset}: +{:.1} / {:.1}",
                result.average_extra_recycled_draws,
                result.average_refund_tickets_on_recycled_success));
        }
        ui.add(egui::Label::new(match language {
            Language::Korean => "생성 환급권에는 목표 완료 후 미사용 잔액도 포함됩니다. 환급은 해당 뽑기 다음부터 사용 가능하며 초기 보유 교환 포인트는 두 번째 단계에 포함됩니다.",
            _ => "Generated refunds include unspent rewards at completion. Returns fund only later draws; starting exchange points count toward stage two.",
        }).wrap());
    });
}

/// Compact 3-column chart panel: header, right-aligned result badge, area graph.
fn probability_card(ui: &mut egui::Ui, title: &str, badge: String, source: &[ChartSeries<'_>], color: Color32, appearance: UiPreferences, language: Language) {
    egui::Frame::group(ui.style())
        .fill(DEEP_CARD)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(14.0).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("›").size(19.0).color(MUTED));
                });
            });
            ui.add_space(6.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = RichText::new(badge).size(10.5).color(color).strong();
                    egui::Frame::default()
                        .fill(Color32::from_rgb(7, 30, 43))
                        .stroke(egui::Stroke::new(0.8_f32, color))
                        .corner_radius(8)
                        .inner_margin(egui::Margin::symmetric(7, 3))
                        .show(ui, |ui| { ui.label(label); });
                });
            });
            draw_probability_chart(ui, source, appearance, language);
        });
}

/// Inset card used by each quick editor, matching the mockup's two-row tiles.
fn setting_tile(ui: &mut egui::Ui, icon: &str, label: &str,
    inner: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(28, 59, 83))
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(6)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(48.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(icon).size(16.0).color(Color32::WHITE));
                ui.label(RichText::new(label).size(10.5).color(Color32::WHITE).strong());
            });
            ui.scope(|ui| {
                ui.visuals_mut().extreme_bg_color = Color32::from_rgb(10, 35, 54);
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_rgb(13, 35, 55);
                ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::from_rgb(13, 35, 55);
                ui.visuals_mut().widgets.hovered.weak_bg_fill = Color32::from_rgb(20, 54, 78);
                ui.visuals_mut().widgets.active.weak_bg_fill = Color32::from_rgb(25, 76, 100);
                ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(23, 63, 91);
                ui.visuals_mut().widgets.active.bg_fill = Color32::from_rgb(23, 85, 115);
                inner(ui);
            });
        });
}

// Compose the overview, controls and right-hand results panel as independent
// egui lifecycle components. Shared painting primitives remain private here.
#[path = "dashboard/overview.rs"] mod overview;
#[path = "dashboard/settings.rs"] mod settings;
#[path = "dashboard/results.rs"] mod results;
