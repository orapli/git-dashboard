use crate::app::GitDashboardApp;
use egui::Color32;

impl GitDashboardApp {
    // Custom bar chart renderer
    pub(super) fn draw_custom_bar_chart(
        &self,
        ui: &mut egui::Ui,
        title: &str,
        labels: &[String],
        values: &[usize],
    ) {
        let t = self.theme;
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(title).strong().size(14.0).color(t.text));
            ui.add_space(4.0);

            if values.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(crate::i18n::t(self.prefs.language, "no_data"))
                        .weak()
                        .color(t.text_dim),
                );
                ui.add_space(8.0);
                return;
            }

            // Scale to 1.25× the max value; clamp to 4 so all-zero data still renders
            let max_val = (*values.iter().max().unwrap_or(&0) as f32 * 1.25).max(4.0);
            let chart_height = 80.0;
            let spacing = 6.0;

            egui::Frame::NONE
                .fill(t.bg)
                .corner_radius(4.0)
                .show(ui, |ui| {
                    let total_width = ui.available_width().max(100.0);
                    // Allocate extra height for value labels above each bar
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(total_width, chart_height + 30.0),
                        egui::Sense::hover(),
                    );

                    let painter = ui.painter_at(rect);
                    let num_bars = values.len();
                    let bar_width =
                        (rect.width() - (num_bars - 1) as f32 * spacing) / num_bars as f32;

                    // Baseline at the bottom; top 12px reserved for value labels
                    let chart_y_offset = 12.0;
                    let y_bottom = rect.top() + chart_y_offset + chart_height;

                    // Background grid (faint horizontal line at midpoint)
                    let grid_y = rect.top() + chart_y_offset + chart_height / 2.0;
                    let grid_color = if t.is_dark {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 10)
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 10)
                    };
                    painter.line_segment(
                        [
                            egui::pos2(rect.left(), grid_y),
                            egui::pos2(rect.right(), grid_y),
                        ],
                        egui::Stroke::new(1.0, grid_color),
                    );

                    for i in 0..num_bars {
                        let val = values[i] as f32;
                        let pct = val / max_val;
                        let bar_height = chart_height * pct;

                        let x_left = rect.left() + i as f32 * (bar_width + spacing);
                        let x_right = x_left + bar_width;
                        let y_top = y_bottom - bar_height;

                        let bar_rect = egui::Rect::from_min_max(
                            egui::pos2(x_left, y_top),
                            egui::pos2(x_right, y_bottom),
                        );

                        // Use the first palette colour
                        painter.rect_filled(bar_rect, 2.0, t.chart[0]);

                        // Value label above the bar
                        if val > 0.0 {
                            let text_pos = egui::pos2(x_left + bar_width / 2.0, y_top - 3.0);
                            painter.text(
                                text_pos,
                                egui::Align2::CENTER_BOTTOM,
                                format!("{}", val as usize),
                                egui::FontId::proportional(8.0),
                                t.text,
                            );
                        }

                        // X-axis label (thinned for dense datasets)
                        let show_label = if num_bars > 12 { i % 3 == 0 } else { true };
                        if show_label && i < labels.len() {
                            let text_pos = egui::pos2(x_left + bar_width / 2.0, y_bottom + 12.0);
                            painter.text(
                                text_pos,
                                egui::Align2::CENTER_CENTER,
                                &labels[i],
                                egui::FontId::proportional(9.0),
                                t.text_dim,
                            );
                        }
                    }
                });
        });
    }

    // Donut chart renderer
    pub(super) fn draw_pie_chart(&self, ui: &mut egui::Ui, slices: &[(&str, f32, Color32)]) {
        let t = self.theme;
        let chart_size = 150.0;
        let legend_h = 12.0 * slices.len().min(5) as f32 + 8.0;
        let (rect, _response) = ui.allocate_exact_size(
            egui::vec2(chart_size, chart_size + legend_h),
            egui::Sense::hover(),
        );

        let painter = ui.painter_at(rect);
        let center = egui::pos2(rect.center().x, rect.top() + chart_size / 2.0 + 5.0);
        let radius = chart_size / 2.0 - 4.0;

        let mut start_angle = -std::f32::consts::FRAC_PI_2; // start at the top

        for &(_label, percentage, color) in slices {
            if percentage <= 0.0 {
                continue;
            }
            let sweep_angle = (percentage / 100.0) * (std::f32::consts::PI * 2.0);
            let end_angle = start_angle + sweep_angle;

            // Split slices > 180° to avoid polygon concavity artifacts
            let diff = sweep_angle;
            let num_segments = (diff / (std::f32::consts::PI / 2.0)).ceil() as usize;
            for s in 0..num_segments {
                let t1 = start_angle + diff * (s as f32) / (num_segments as f32);
                let t2 = start_angle + diff * ((s + 1) as f32) / (num_segments as f32);

                let mut points = vec![center];
                let steps = 10;
                for i in 0..=steps {
                    let t = t1 + (t2 - t1) * (i as f32) / (steps as f32);
                    points.push(center + egui::vec2(t.cos() * radius, t.sin() * radius));
                }
                painter.add(egui::Shape::convex_polygon(
                    points,
                    color,
                    egui::Stroke::NONE,
                ));
            }

            // Slice border (drawn in background colour for a subtle gap)
            let edge_pos =
                center + egui::vec2(start_angle.cos() * radius, start_angle.sin() * radius);
            painter.line_segment([center, edge_pos], egui::Stroke::new(1.0, t.bg_elevated));

            start_angle = end_angle;
        }

        // Punch out the centre with the background colour to create the donut hole
        painter.circle_filled(center, radius * 0.45, t.bg_elevated);

        // Legend (top 5 slices)
        let mut y_offset = rect.top() + chart_size + 6.0;
        for &(label, percentage, color) in slices.iter().take(5) {
            let item_rect = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 2.0, y_offset + 1.0),
                egui::pos2(rect.left() + 10.0, y_offset + 9.0),
            );
            painter.rect_filled(item_rect, 1.5, color);
            painter.text(
                egui::pos2(rect.left() + 15.0, y_offset + 5.0),
                egui::Align2::LEFT_CENTER,
                format!("{}: {:.1}%", label, percentage),
                egui::FontId::proportional(10.0),
                t.text_dim,
            );
            y_offset += 12.0;
        }
    }
}

// Self-drawn vector icon button
pub fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    theme: &crate::theme::Theme,
) -> egui::Response {
    let size = egui::vec2(22.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    // Fill a faint background on hover
    if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, theme.bg_hover);
    }

    let color = if !ui.is_enabled() {
        theme.text_faint
    } else if response.is_pointer_button_down_on() {
        theme.text
    } else if response.hovered() {
        if icon == "trash" {
            theme.error
        } else {
            theme.accent
        }
    } else {
        theme.text_dim
    };

    let icon_rect = rect.shrink(4.0); // 14x14 px inner area
    let painter = ui.painter();

    let glyph = match icon {
        "pencil" => icons::EDIT,
        "trash" => icons::TRASH,
        "swap" => icons::ARROW_SWAP,
        "download" => icons::CLOUD_DOWNLOAD,
        "refresh" => icons::REFRESH,
        "editor" => icons::LINK_EXTERNAL,
        "back" => icons::ARROW_LEFT,
        "close" => icons::CLOSE,
        "collapse" => icons::CHEVRON_LEFT,
        "expand" => icons::CHEVRON_RIGHT,
        "fetch" => icons::GIT_FETCH,
        "user-plus" => icons::USER_PLUS,
        "user-link" => icons::LINK,
        _ => "",
    };
    if !glyph.is_empty() {
        paint_icon_glyph(painter, icon_rect, color, glyph);
    }

    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

/// VS Code Codicons (MIT license) glyph constants, embedded as `assets/codicon.ttf`
/// and registered as the `"Icons"` font family in `theme::setup_fonts`. Same font
/// aero-grep uses; codepoints verified against the font's own cmap table (the
/// codicon.html reference page and older docs can be stale — trust the font).
pub mod icons {
    pub const EDIT: &str = "\u{EA73}"; // pencil
    pub const TRASH: &str = "\u{EA81}";
    pub const ARROW_SWAP: &str = "\u{EBCB}";
    pub const CLOUD_DOWNLOAD: &str = "\u{EAC2}";
    pub const REFRESH: &str = "\u{EB37}";
    pub const LINK_EXTERNAL: &str = "\u{EB14}"; // open in external editor
    pub const ARROW_LEFT: &str = "\u{EA9B}"; // back
    pub const CLOSE: &str = "\u{EA76}";
    pub const CHEVRON_LEFT: &str = "\u{EAB5}";
    pub const CHEVRON_RIGHT: &str = "\u{EAB6}";
    pub const GIT_FETCH: &str = "\u{F101}";
    pub const HOME: &str = "\u{EB06}";
    pub const GEAR: &str = "\u{EAF8}"; // settings
    pub const USER_PLUS: &str = "\u{EB96}"; // person add / user plus
    pub const LINK: &str = "\u{EB15}"; // link / bind
}

/// Render one Codicon glyph centered in `rect`, directly against a raw
/// `Painter` (no `Ui` needed) so both simple widgets (`icon_button`) and
/// manually-laid-out rows (e.g. the home repo list's hover buttons) share
/// one implementation.
pub fn paint_icon_glyph(painter: &egui::Painter, rect: egui::Rect, color: Color32, glyph: &str) {
    let font_id = egui::FontId::new(rect.height(), egui::FontFamily::Name("Icons".into()));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        font_id,
        color,
    );
}

pub fn draw_icon_refresh_pub(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    paint_icon_glyph(painter, rect, color, icons::REFRESH);
}

pub fn draw_icon_editor_pub(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    paint_icon_glyph(painter, rect, color, icons::LINK_EXTERNAL);
}

pub(super) fn draw_icon_close(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    paint_icon_glyph(painter, rect, color, icons::CLOSE);
}
