//! Reference-inspired Windows/Linux dashboard. Probability and refund engines stay unchanged.
use super::*;

const CYAN: Color32 = Color32::from_rgb(50, 218, 241);
const YELLOW: Color32 = Color32::from_rgb(244, 210, 75);
const PINK: Color32 = Color32::from_rgb(225, 112, 232);
const SUCCESS: Color32 = Color32::from_rgb(141, 238, 117);
const DEEP_CARD: Color32 = Color32::from_rgb(16, 43, 64);

/// Subtle vertical gradient; the rounded base remains visible at the corners.
fn wash(ui: &egui::Ui, rect: egui::Rect, upper: Color32, lower: Color32) {
    ui.painter().rect_filled(rect, 14.0, upper);
    let r = rect.shrink(9.0);
    if r.width() <= 0.0 || r.height() <= 0.0 { return; }
    let mut mesh = egui::Mesh::default();
    for (point, color) in [
        (r.left_top(), upper), (r.right_top(), upper),
        (r.left_bottom(), lower), (r.right_bottom(), lower),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: point, uv: egui::epaint::WHITE_UV, color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 2, 1, 1, 2, 3]);
    ui.painter().add(egui::Shape::mesh(mesh));

}

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

/// Paint a card with fixed geometry so values cannot push other dashboard cards down.
fn overview_metric(ui: &mut egui::Ui, kind: usize, label: &str, value: String, note: String, color: Color32) {
    let (r, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(90.0), 120.0), egui::Sense::hover(),
    );
    let base = if kind == 1 { Color32::from_rgb(14, 59, 76) }
               else if kind == 2 { Color32::from_rgb(35, 57, 94) }
               else { Color32::from_rgb(28, 63, 96) };
    wash(ui, r, base, Color32::from_rgb(17, 43, 67));
    // Clip large text at the edge of its metric card, never paint over peers.
    let paint = ui.painter().with_clip_rect(r.shrink(4.0));
    metric_icon(&paint, egui::pos2(r.left()+34.0,r.top()+47.0), kind, color);
    let left = r.left()+66.0;
    paint.text(egui::pos2(left,r.top()+18.0),
        egui::Align2::LEFT_TOP, label, egui::FontId::proportional(12.0),
        Color32::WHITE);
    paint.text(egui::pos2(left,r.top()+43.0),
        egui::Align2::LEFT_TOP, value, egui::FontId::proportional(27.0),
        Color32::WHITE);
    paint.text(egui::pos2(left,r.top()+94.0),
        egui::Align2::LEFT_TOP, note, egui::FontId::proportional(10.0),
        MUTED);
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
