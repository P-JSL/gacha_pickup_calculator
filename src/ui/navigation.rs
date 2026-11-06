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
        let middle = low + (high - low).div_ceil(2);
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

