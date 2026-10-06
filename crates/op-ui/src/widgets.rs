//! Reusable widgets: icon buttons, read-only fields, separators, etc.

use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::theme::{self, color};

/// Square icon button. Draws a dark background when `selected` (current tool).
pub fn icon_button(ui: &mut Ui, icon: &str, size: f32, selected: bool) -> Response {
    icon_button_sized(ui, icon, Vec2::splat(size), theme::font::ICON, selected)
}

pub fn icon_button_sized(
    ui: &mut Ui,
    icon: &str,
    size: Vec2,
    icon_size: f32,
    selected: bool,
) -> Response {
    icon_button_font(ui, icon, size, theme::icon(icon_size), selected)
}

/// An icon button drawing its icon in `font`.
pub fn icon_button_font(
    ui: &mut Ui,
    icon: &str,
    size: Vec2,
    font: egui::FontId,
    selected: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if selected || response.is_pointer_button_down_on() {
            color::TOOL_ACTIVE
        } else if response.hovered() {
            color::HOVER
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);
        let tint = if ui.is_enabled() {
            color::ICON
        } else {
            color::TEXT_DISABLED
        };
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, icon, font, tint);
    }
    response
}

/// A bare icon without a background.
pub fn icon(ui: &mut Ui, icon: &str, size: f32, tint: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size + 4.0), Sense::click());
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        theme::icon(size),
        tint,
    );
    response
}

/// A read-only field that looks like a text input (for values that aren't
/// editable yet, such as size and resolution).
pub fn field(ui: &mut Ui, text: &str, width: f32, enabled: bool) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(width, theme::size::FIELD_HEIGHT), Sense::hover());
    let (fill, stroke, text_color) = if enabled {
        (color::FIELD, color::FIELD_BORDER, color::TEXT)
    } else {
        (
            Color32::TRANSPARENT,
            color::SEPARATOR_LIGHT,
            color::TEXT_DISABLED,
        )
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(2),
        fill,
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(6.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        theme::body(),
        text_color,
    );
    response
}

/// Vertical separator (between groups in the options bar).
pub fn vseparator(ui: &mut Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, height), Sense::hover());
    let x = rect.center().x.round() + 0.5;
    ui.painter().line_segment(
        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
        Stroke::new(1.0, color::SEPARATOR_LIGHT),
    );
}

/// Full-width horizontal separator.
pub fn hseparator(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, color::SEPARATOR_LIGHT);
}

/// Checkerboard (transparency background).
pub fn checkerboard(painter: &egui::Painter, rect: Rect, cell: f32) {
    painter.rect_filled(rect, 0, Color32::WHITE);
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 1 {
                let min = rect.min + Vec2::new(x as f32 * cell, y as f32 * cell);
                let r = Rect::from_min_size(min, Vec2::splat(cell)).intersect(rect);
                painter.rect_filled(r, 0, Color32::from_gray(204));
            }
        }
    }
}

/// Shows/edits a 0..=1 value as a percentage.
pub fn percent_drag(ui: &mut Ui, value: &mut f32) -> Response {
    let mut pct = *value * 100.0;
    let response = ui.add_sized(
        [64.0, theme::size::FIELD_HEIGHT],
        egui::DragValue::new(&mut pct)
            .range(0.0..=100.0)
            .speed(0.5)
            .max_decimals(0)
            .suffix("%"),
    );
    if response.changed() {
        *value = pct / 100.0;
    }
    response
}
