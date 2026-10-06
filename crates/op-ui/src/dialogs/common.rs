//! Pieces shared by the modal dialogs: the window frame with its light title
//! bar, and the pill-shaped buttons Photoshop uses in dialogs.

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2,
};

use crate::theme::{self, color, pt};

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

const FIELD_BORDER: Color32 = Color32::from_gray(0x77);
const DROPDOWN_BORDER: Color32 = Color32::from_gray(0x6a);
const FOCUS: Color32 = Color32::from_rgb(0x14, 0x73, 0xe6);

/// A text input with Photoshop's dialog styling (dark fill, light border,
/// blue focus ring). With `select_all`, it takes focus and selects its text
/// (Photoshop does this for a dialog's first field when it opens).
pub fn number_field(
    ui: &mut Ui,
    rect: Rect,
    text: &mut String,
    id: impl egui::AsIdSalt,
    font: f32,
    select_all: bool,
) -> egui::Response {
    let pad = rect.height() * 0.23;
    text_field(
        ui,
        rect,
        text,
        id,
        FontId::proportional(font),
        pad,
        select_all,
    )
}

/// [`number_field`] with a given font and left padding (the right one is
/// 2 pt).
pub fn text_field(
    ui: &mut Ui,
    rect: Rect,
    text: &mut String,
    id: impl egui::AsIdSalt,
    font: FontId,
    pad: f32,
    select_all: bool,
) -> egui::Response {
    use egui::text::{CCursor, CCursorRange};
    let id = ui.id().with(id);
    let has_focus = ui.memory(|m| m.has_focus(id));
    ui.painter().rect(
        rect,
        3,
        color::FIELD,
        Stroke::new(1.0, FIELD_BORDER),
        StrokeKind::Inside,
    );
    if has_focus {
        ui.painter()
            .rect_stroke(rect, 4, Stroke::new(2.0, FOCUS), StrokeKind::Outside);
    }

    let inner = Rect::from_min_max(
        rect.min + vec2(pad, 0.0),
        rect.max - vec2(crate::theme::pt(2.0), 0.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
    let output = egui::TextEdit::singleline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .font(font)
        .vertical_align(egui::Align::Center)
        .desired_width(inner.width())
        .min_size(vec2(0.0, rect.height()))
        .show(&mut child);

    if select_all {
        output.response.request_focus();
        let mut state = output.state;
        let end = CCursor::new(text.chars().count());
        state
            .cursor
            .set_char_range(Some(CCursorRange::two(CCursor::new(0), end)));
        state.store(ui.ctx(), id);
    }
    output.response.response
}

/// A dropdown with Photoshop's dialog styling, filling `rect`.
pub fn dropdown(
    ui: &mut Ui,
    rect: Rect,
    id: impl egui::AsIdSalt,
    selected: &str,
    font: f32,
    enabled: bool,
    menu: impl FnOnce(&mut Ui),
) {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.add_enabled_ui(enabled, |ui| {
        let v = &mut ui.visuals_mut().widgets;
        for w in [&mut v.inactive, &mut v.hovered, &mut v.active, &mut v.open] {
            w.weak_bg_fill = color::PANEL;
            w.bg_fill = color::PANEL;
            w.bg_stroke = Stroke::new(1.0, DROPDOWN_BORDER);
            w.corner_radius = CornerRadius::same(3);
        }
        v.hovered.weak_bg_fill = color::HOVER;
        ui.spacing_mut().interact_size.y = rect.height();
        ui.spacing_mut().button_padding = vec2(rect.height() * 0.35, rect.height() * 0.17);
        egui::ComboBox::from_id_salt(id)
            .width(rect.width())
            .height(400.0)
            .selected_text(egui::RichText::new(selected).font(FontId::proportional(font)))
            .show_ui(ui, menu);
    });
}

// Photoshop 2026's dialog controls, measured on its New Layer dialog: text
// in the system font (`theme::dialog`, 12 pt).

const PS_TEXT: Color32 = Color32::from_gray(0xf1);
const PS_TEXT_OFF: Color32 = Color32::from_gray(0x8e);
const PS_BORDER: Color32 = Color32::from_gray(0x7a);
const PS_CHECK_BORDER: Color32 = Color32::from_gray(0xa0);
const PS_FONT: f32 = crate::theme::pt(12.0);

/// A dialog dropdown: a rounded box with a 1 pt `#7a7a7a` border, content
/// drawn by `content`, a chevron 14 pt from the right; clicking opens `menu`.
pub fn ps_dropdown(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    content: impl FnOnce(&egui::Painter, Rect),
    menu: impl FnOnce(&mut Ui),
) {
    let response = ui.interact(rect, ui.id().with(id), Sense::click());
    let fill = if response.hovered() {
        color::HOVER
    } else {
        color::PANEL
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(pt(3.0) as u8),
        fill,
        Stroke::new(pt(1.0), PS_BORDER),
        StrokeKind::Inside,
    );
    content(ui.painter(), rect);
    crate::ps_icons::paint(
        ui.painter(),
        Pos2::new(rect.right() - pt(13.5), rect.center().y),
        crate::ps_icons::Icon::DialogChevron,
        PS_TEXT,
        fill,
    );
    egui::Popup::menu(&response)
        .id(ui.id().with((id, "menu")))
        .show(menu);
}

/// A 12 pt dialog checkbox with its label 9.5 pt to the right.
pub fn ps_checkbox(ui: &mut Ui, min: Pos2, label: &str, checked: &mut bool, enabled: bool) {
    let font = theme::dialog_medium(PS_FONT);
    let text = if enabled { PS_TEXT } else { PS_TEXT_OFF };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, text);
    let b = Rect::from_min_size(min, vec2(pt(12.0), pt(12.0)));
    let hit = Rect::from_min_max(
        b.min,
        Pos2::new(b.right() + pt(9.5) + galley.size().x, b.bottom()),
    );
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(hit, ui.id().with(("check", label)), sense);
    if response.clicked() {
        *checked = !*checked;
    }
    let painter = ui.painter();
    let border = if enabled {
        PS_CHECK_BORDER
    } else {
        Color32::from_gray(0x8e)
    };
    if *checked && enabled {
        painter.rect_filled(b, pt(2.5), color::CHECKBOX);
        let p = |x: f32, y: f32| b.min + vec2(pt(x), pt(y));
        painter.add(egui::Shape::line(
            vec![p(3.0, 6.0), p(5.25, 8.25), p(9.25, 3.75)],
            Stroke::new(pt(1.7), color::CHECK_MARK),
        ));
    } else {
        painter.rect_stroke(b, pt(2.5), Stroke::new(pt(1.0), border), StrokeKind::Inside);
    }
    painter.galley(
        Pos2::new(b.right() + pt(9.5), b.center().y - galley.size().y / 2.0),
        galley,
        text,
    );
}

/// Photoshop's dialog buttons: pills with a 1 pt border, bright for the
/// default button (bold label) and dim for the others.
pub fn ps_button(
    ui: &mut Ui,
    rect: Rect,
    label: &str,
    default: bool,
    enabled: bool,
    bold: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(("button", label)), sense);
    let fill = if enabled && response.is_pointer_button_down_on() {
        color::TOOL_ACTIVE
    } else if enabled && response.hovered() {
        color::HOVER
    } else {
        color::PANEL
    };
    let border = match (enabled, default) {
        (false, _) => Color32::from_gray(0x5e),
        (true, true) => PS_TEXT,
        (true, false) => Color32::from_gray(0x72),
    };
    // Photoshop's newer dialogs (New Layer) set every button in bold
    let font = if bold {
        theme::dialog_bold(pt(13.0))
    } else {
        theme::dialog(pt(13.0))
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(255),
        fill,
        Stroke::new(pt(1.0), border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        font,
        if enabled { PS_TEXT } else { PS_TEXT_OFF },
    );
    response
}
