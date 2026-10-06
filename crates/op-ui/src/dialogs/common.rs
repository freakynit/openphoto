//! Pieces shared by the modal dialogs: the window frame with its light title
//! bar, and the pill-shaped buttons Photoshop uses in dialogs.

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::theme::{color, pt};

pub const TITLE_BAR: f32 = pt(28.0);
const TITLE_FILL: Color32 = Color32::from_rgb(0xd0, 0xd2, 0xd4);
const TITLE_TEXT: Color32 = Color32::from_rgb(0x33, 0x33, 0x33);
const BUTTON_BORDER: Color32 = Color32::from_gray(0xd0);
const RADIUS: u8 = 10;

/// Draws the dialog body, shadow, border and title bar into `frame`.
pub fn frame(ui: &Ui, frame: Rect, title: &str, title_font: FontId) {
    let painter = ui.painter();
    painter.add(
        egui::Shadow {
            offset: [0, 8],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(110),
        }
        .as_shape(frame, RADIUS),
    );
    painter.rect_filled(frame, RADIUS, color::PANEL);
    let bar = Rect::from_min_size(frame.min, vec2(frame.width(), TITLE_BAR));
    painter.rect_filled(
        bar,
        CornerRadius {
            nw: RADIUS,
            ne: RADIUS,
            sw: 0,
            se: 0,
        },
        TITLE_FILL,
    );
    painter.line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(0x30)),
    );
    painter.text(
        bar.center(),
        Align2::CENTER_CENTER,
        title,
        title_font,
        TITLE_TEXT,
    );
    painter.rect_stroke(
        frame,
        RADIUS,
        Stroke::new(1.0, Color32::from_gray(0x2a)),
        StrokeKind::Outside,
    );
}

/// A pill-shaped dialog button. `id` must be unique within the dialog.
pub fn pill_button(
    ui: &mut Ui,
    rect: Rect,
    label: &str,
    font: FontId,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(("pill", label)), sense);
    let fill = if enabled && response.is_pointer_button_down_on() {
        color::TOOL_ACTIVE
    } else if enabled && response.hovered() {
        color::HOVER
    } else {
        color::PANEL
    };
    let (border, text) = if enabled {
        (BUTTON_BORDER, color::TEXT)
    } else {
        (color::SEPARATOR_LIGHT, color::TEXT_DISABLED)
    };
    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(255),
        fill,
        Stroke::new(1.5, border),
        StrokeKind::Inside,
    );
    painter.text(rect.center(), Align2::CENTER_CENTER, label, font, text);
    response
}
