use super::*;

/// Each chart has its own X-range. Character and secondary-banner curves
/// therefore cannot be compressed into a shared 2000-pull wide axis.
#[derive(Clone, Copy)]
pub(crate) enum ChartSource<'a> {
    Exact(&'a Distribution),
    Refund(&'a FeedbackEstimate),
}

impl ChartSource<'_> {
    pub(crate) fn last_pull(self) -> usize {
        match self {
            Self::Exact(d) => d.pmf.len().saturating_sub(1),
            Self::Refund(f) => f.quantile(1.0),
        }
    }

    pub(crate) fn probability(self, n: usize) -> f64 {
        match self {
            Self::Exact(d) => d.at(n),
            Self::Refund(f) => f.probability(n).0,
        }
    }
}

pub(crate) struct ChartSeries<'a> {
    pub(crate) label: &'a str,
    pub(crate) source: ChartSource<'a>,
    pub(crate) color: Color32,
}

pub(crate) fn chart_domain_end(series: &[ChartSeries<'_>]) -> usize {
    series.iter().map(|s| s.source.last_pull()).max().unwrap_or(1).max(1)
}

/// Keep the hover label inside the plot for every pointer position.
pub(crate) fn chart_tooltip_rect(graph: egui::Rect, cursor: egui::Pos2, desired_width: f32, height: f32) -> egui::Rect {
    let inner = graph.shrink(4.0);
    let width = desired_width.min(inner.width()).max(0.0);
    let height = height.min(inner.height()).max(0.0);
    let x = (cursor.x + 12.0).clamp(inner.left(), (inner.right() - width).max(inner.left()));
    let y = (cursor.y - height - 11.0)
        .clamp(inner.top(), (inner.bottom() - height).max(inner.top()));
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(width, height))
}

/// The highlighted probability is always the *refund-recycled* joint CDF
/// when present. The unrefunded CDF stays visible only as a labeled reference.
/// Keep tooltip values on separate rows: combining both into one long line
/// silently clipped the refund probability on narrow desktop cards.
pub(crate) fn chart_tooltip_lines(
    series: &[ChartSeries<'_>], x: usize, language: Language,
) -> Vec<(String, Color32)> {
    let pull_unit = match language {
        Language::Korean => "뽑 이내",
        Language::English => "pulls or fewer",
        Language::Japanese => "連以内",
        Language::ChineseSimplified => "抽以内",
        Language::ChineseTraditional => "抽以內",
    };
    if let Some(recycled) = series.iter().find(|line| matches!(line.source, ChartSource::Refund(_))) {
        let (primary, reference) = match language {
            Language::Korean => ("환급 재투입 포함", "환급 전(참고)"),
            Language::English => ("With recycled refunds", "Before refunds (reference)"),
            Language::Japanese => ("還元再利用込み", "還元前（参考）"),
            Language::ChineseSimplified => ("含返还再投入", "返还前（参考）"),
            Language::ChineseTraditional => ("含返還再投入", "返還前（參考）"),
        };
        let mut rows = vec![(
            format!("{x} {pull_unit} · {primary} {:.1}%", recycled.source.probability(x) * 100.0),
            Color32::WHITE,
        )];
        rows.extend(series.iter()
            .filter(|line| !matches!(line.source, ChartSource::Refund(_)))
            .map(|line| (
                format!("{reference} {:.1}%", line.source.probability(x) * 100.0),
                MUTED,
            )));
        rows
    } else {
        series.iter().enumerate().map(|(i, line)| (
            format!("{}{label} {:.1}%",
                if i == 0 { format!("{x} {pull_unit} · ") } else { String::new() },
                line.source.probability(x) * 100.0,
                label = line.label),
            Color32::WHITE,
        )).collect()
    }
}

/// Common Painter plot for all desktop OSs; it uses only egui logical points.
/// The same fill, grid, axes, scaling and series colors are used on Windows
/// and Linux. No OS-native chart controls or persistence are involved.
pub(crate) fn draw_probability_chart(ui: &mut egui::Ui, series: &[ChartSeries<'_>], appearance: UiPreferences, language: Language) {
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
        let rows = chart_tooltip_lines(series, x, language);
        // Native egui popups can escape the chart. Draw a compact multi-row
        // overlay inside the plot instead, prioritizing recycled success.
        let tooltip_font = egui::FontId::proportional(10.5);
        let text_width = ui.fonts(|fonts| rows.iter().map(|(label, _)| {
            fonts.layout_no_wrap(label.clone(), tooltip_font.clone(), Color32::WHITE).size().x
        }).fold(0.0_f32, f32::max));
        let row_height = 16.0;
        let badge_height = (rows.len() as f32 * row_height + 6.0).max(22.0);
        let badge = chart_tooltip_rect(graph, pointer, text_width + 16.0, badge_height);
        painter.rect_filled(badge, 5.0, Color32::from_rgb(18, 48, 69));
        let overlay = painter.with_clip_rect(badge.intersect(graph).shrink(3.0));
        for (i, (label, color)) in rows.iter().enumerate() {
            let y = badge.center().y
                + (i as f32 - (rows.len() as f32 - 1.0) / 2.0) * row_height;
            overlay.text(egui::pos2(badge.left() + 7.0, y),
                egui::Align2::LEFT_CENTER, label, tooltip_font.clone(), *color);
        }
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

pub(crate) fn page_intro(ui: &mut egui::Ui, eyebrow: &str, title: &str, subtitle: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(eyebrow).size(9.5).color(PRIMARY).strong());
        ui.heading(RichText::new(title).size(18.0).strong());
        ui.label(RichText::new(subtitle).size(10.5).color(MUTED));
    });
    ui.add_space(6.0);
}

/// Generic card, with a fixed visual hierarchy and no stateful UI controls.
pub(crate) fn section(ui: &mut egui::Ui, number: &str, heading: &str, description: &str,
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

pub(crate) fn metric(ui: &mut egui::Ui, label: &str, value: String, note: &str, accent: Color32) {
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

