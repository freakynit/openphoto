//! Left toolbar.

use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use op_core::Color;
use op_tools::{TOOLBAR, Tool};

use crate::icons;
use crate::state::{AppState, PickerTarget};
use crate::theme::{self, color, size};
use crate::widgets;

/// Dark border on the toolbar's right edge, next to the canvas.
const RIGHT_BORDER: f32 = crate::theme::pt(3.0);

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let full = ui.max_rect();
    let border = Rect::from_min_max(Pos2::new(full.right() - RIGHT_BORDER, full.top()), full.max);
    ui.painter()
        .rect_filled(border, 0, Color32::from_gray(0x39));
    ui.set_max_width(full.width() - RIGHT_BORDER);
    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
    let x_pad = (size::TOOLBAR - RIGHT_BORDER - size::TOOL_BUTTON) / 2.0;

    // Collapse arrows and drag grip at the top
    header(ui, icons::CARET_DOUBLE_RIGHT, Align2::LEFT_CENTER);
    grip(ui);
    // Measured on Photoshop 2026: the first tool's center 43 pt below the
    // toolbar's top, then one tool every 25.9 pt, with no gaps between groups
    let pitch = crate::theme::pt(25.9);
    // Tool buttons are 30.5 pt wide, centered in the bar
    let button_w = crate::theme::pt(30.5);
    let tool_pad = (size::TOOLBAR - RIGHT_BORDER - button_w) / 2.0;
    let first_top = full.top() + crate::theme::pt(43.0) - pitch / 2.0;
    ui.add_space((first_top - ui.cursor().top()).max(0.0));

    for (slot, group) in TOOLBAR.iter().enumerate() {
        // Each slot shows the tool of its group that was used last
        let shown = app.tool_slots[slot];
        ui.horizontal(|ui| {
            ui.add_space(tool_pad);
            let selected = app.tool.slot() == slot;
            // Photoshop's tool icons are about 15 pt across
            let r = widgets::icon_button_font(
                ui,
                icons::tool(shown),
                Vec2::new(button_w, pitch - 1.0),
                theme::tool_icon(crate::theme::pt(17.5)),
                selected,
            );
            // The selected tool's box has a faint light outline
            if selected {
                ui.painter().rect_stroke(
                    r.rect,
                    4,
                    egui::Stroke::new(1.0, Color32::from_gray(0x60)),
                    egui::StrokeKind::Outside,
                );
            }
            if group.len() > 1 {
                group_marker(ui, r.rect);
            }
            let r = r.on_hover_text(tool_tip(shown));
            if r.clicked() {
                app.select_tool(shown);
            }
            if group.len() > 1 {
                flyout(&r, group, shown, app);
            }
        });
        ui.add_space(1.0);
    }

    ui.horizontal(|ui| {
        ui.add_space(x_pad);
        widgets::icon_button(ui, icons::DOTS_THREE, size::TOOL_BUTTON, false)
            .on_hover_text("Edit Toolbar");
    });
    ui.add_space(10.0);

    color_swatches(ui, app);
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.add_space(x_pad);
        let on = app
            .active_doc
            .and_then(|id| app.docs.get(&id))
            .is_some_and(|d| d.doc.quick_mask.is_some());
        if widgets::icon_button(ui, icons::SELECTION_BACKGROUND, size::TOOL_BUTTON, on)
            .on_hover_text("Edit in Quick Mask Mode (Q)")
            .clicked()
            && let Some(state) = app.active()
        {
            toggle_quick_mask(state);
        }
    });
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.add_space(x_pad);
        widgets::icon_button(ui, icons::APP_WINDOW, size::TOOL_BUTTON, false)
            .on_hover_text("Change Screen Mode (F)");
    });
}

pub fn header(ui: &mut Ui, icon: &str, align: Align2) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
    let pos = match align.x() {
        egui::Align::Min => rect.left_center() + Vec2::new(6.0, 0.0),
        _ => rect.right_center() - Vec2::new(6.0, 0.0),
    };
    ui.painter()
        .text(pos, align, icon, theme::icon(11.0), color::TEXT_DIM);
}

fn grip(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), Sense::hover());
    let c = rect.center();
    for i in -6..=6 {
        let x = c.x + i as f32 * 2.0;
        ui.painter()
            .circle_filled(Pos2::new(x, c.y), 0.6, Color32::from_gray(0x80));
    }
}

fn tool_tip(tool: Tool) -> String {
    match tool.shortcut() {
        Some(k) => format!("{} ({k})", tool.name()),
        None => tool.name().to_owned(),
    }
}

/// Photoshop's tool flyout: right-clicking a slot lists the tools of its
/// group to the right of the button, with the shown tool marked.
fn flyout(button: &egui::Response, group: &[Tool], shown: Tool, app: &mut AppState) {
    let open = button
        .secondary_clicked()
        .then_some(egui::SetOpenCommand::Bool(true));
    egui::Popup::from_response(button)
        .open_memory(open)
        .align(egui::RectAlign::RIGHT_START)
        .gap(crate::theme::pt(2.0))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(crate::theme::pt(200.0));
            for &tool in group {
                let row_h = crate::theme::pt(22.0);
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
                let painter = ui.painter();
                if response.hovered() {
                    painter.rect_filled(rect, 2, color::ACCENT);
                }
                if tool == shown {
                    let mark = Rect::from_center_size(
                        rect.left_center() + Vec2::new(crate::theme::pt(7.0), 0.0),
                        Vec2::splat(crate::theme::pt(4.0)),
                    );
                    painter.rect_filled(mark, 0, color::TEXT);
                }
                painter.text(
                    rect.left_center() + Vec2::new(crate::theme::pt(22.0), 0.0),
                    Align2::CENTER_CENTER,
                    icons::tool(tool),
                    theme::icon(crate::theme::pt(14.0)),
                    color::ICON,
                );
                painter.text(
                    rect.left_center() + Vec2::new(crate::theme::pt(36.0), 0.0),
                    Align2::LEFT_CENTER,
                    tool.name(),
                    theme::body(),
                    color::TEXT,
                );
                if let Some(k) = tool.shortcut() {
                    painter.text(
                        rect.right_center() - Vec2::new(crate::theme::pt(8.0), 0.0),
                        Align2::RIGHT_CENTER,
                        k,
                        theme::body(),
                        color::TEXT,
                    );
                }
                if response.clicked() {
                    app.select_tool(tool);
                }
            }
        });
}

/// Small corner triangle indicating more tools in the group.
fn group_marker(ui: &Ui, rect: Rect) {
    let br = rect.right_bottom() + Vec2::new(-3.0, -3.0);
    ui.painter().add(Shape::convex_polygon(
        vec![br, br + Vec2::new(-4.0, 0.0), br + Vec2::new(0.0, -4.0)],
        color::ICON,
        Stroke::NONE,
    ));
}

/// Foreground/background swatches; the top-left icon resets to defaults and the
/// top-right one swaps them.
fn color_swatches(ui: &mut Ui, app: &mut AppState) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(size::TOOLBAR - RIGHT_BORDER, 58.0),
        Sense::hover(),
    );
    let sw = 26.0;
    let fg_rect = Rect::from_min_size(rect.min + Vec2::new(9.0, 16.0), Vec2::splat(sw));
    let bg_rect = fg_rect.translate(Vec2::splat(13.0));

    let reset_rect = Rect::from_min_size(rect.min + Vec2::new(6.0, 0.0), Vec2::splat(14.0));
    let swap_rect = Rect::from_min_size(rect.min + Vec2::new(34.0, 0.0), Vec2::splat(14.0));

    let reset = ui.interact(reset_rect, ui.id().with("reset"), Sense::click());
    let swap = ui.interact(swap_rect, ui.id().with("swap"), Sense::click());
    let bg = ui.interact(bg_rect, ui.id().with("bg"), Sense::click());
    let fg = ui.interact(fg_rect, ui.id().with("fg"), Sense::click());

    if reset
        .on_hover_text("Default Foreground and Background Colors (D)")
        .clicked()
    {
        reset_colors(app);
    }
    if swap
        .on_hover_text("Switch Foreground and Background Colors (X)")
        .clicked()
    {
        swap_colors(app);
    }
    if fg.on_hover_text("Set foreground color").clicked() {
        app.editing_background = false;
        app.open_color_picker(PickerTarget::Foreground);
    }
    if bg.on_hover_text("Set background color").clicked() {
        app.editing_background = true;
        app.open_color_picker(PickerTarget::Background);
    }

    let painter = ui.painter();
    painter.text(
        reset_rect.center(),
        Align2::CENTER_CENTER,
        icons::SQUARE_HALF,
        theme::icon(12.0),
        color::ICON,
    );
    painter.text(
        swap_rect.center(),
        Align2::CENTER_CENTER,
        icons::ARROWS_CLOCKWISE,
        theme::icon(12.0),
        color::ICON,
    );
    swatch(painter, bg_rect, app.background);
    swatch(painter, fg_rect, app.foreground);
}

pub fn swatch(painter: &egui::Painter, rect: Rect, c: Color) {
    let [r, g, b, _] = c.to_rgba8();
    painter.rect(
        rect,
        0,
        Color32::from_rgb(r, g, b),
        Stroke::new(1.0, Color32::from_gray(0xe0)),
        StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect.expand(1.0),
        0,
        Stroke::new(1.0, Color32::from_gray(0x30)),
        StrokeKind::Inside,
    );
}

/// Q: enters or leaves Quick Mask, recorded as "Quick Mask" either way.
pub fn toggle_quick_mask(state: &mut crate::state::DocState) {
    if state.doc.quick_mask.is_some() {
        state.doc.exit_quick_mask();
    } else {
        state.doc.enter_quick_mask();
    }
    state.record("Quick Mask");
}

pub fn reset_colors(app: &mut AppState) {
    app.foreground = Color::BLACK;
    app.background = Color::WHITE;
}

pub fn swap_colors(app: &mut AppState) {
    std::mem::swap(&mut app.foreground, &mut app.background);
}
