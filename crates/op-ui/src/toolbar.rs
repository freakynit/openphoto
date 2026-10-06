//! Left toolbar.

use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use op_core::Color;
use op_tools::{TOOLBAR, Tool};

use crate::icons;
use crate::state::{AppState, PickerTarget};
use crate::theme::{self, color, size};
use crate::widgets;

pub fn show(ui: &mut Ui, app: &mut AppState) {
    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
    let x_pad = (size::TOOLBAR - size::TOOL_BUTTON) / 2.0;

    // Collapse arrows and drag grip at the top
    header(ui, icons::CARET_DOUBLE_RIGHT, Align2::LEFT_CENTER);
    grip(ui);
    ui.add_space(8.0);

    let group_breaks = [Tool::Crop, Tool::Eyedropper, Tool::Pen, Tool::Hand];
    for &tool in TOOLBAR {
        if group_breaks.contains(&tool) {
            ui.add_space(2.0);
        }
        ui.horizontal(|ui| {
            ui.add_space(x_pad);
            let selected = app.tool == tool;
            let r = widgets::icon_button(ui, icons::tool(tool), size::TOOL_BUTTON, selected);
            if tool.has_group() {
                group_marker(ui, r.rect);
            }
            let tip = match tool.shortcut() {
                Some(k) => format!("{} ({k})", tool.name()),
                None => tool.name().to_owned(),
            };
            if r.on_hover_text(tip).clicked() {
                app.tool = tool;
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
        widgets::icon_button(ui, icons::SELECTION_BACKGROUND, size::TOOL_BUTTON, false)
            .on_hover_text("Edit in Quick Mask Mode (Q)");
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
    let (rect, _) = ui.allocate_exact_size(Vec2::new(size::TOOLBAR, 58.0), Sense::hover());
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

pub fn reset_colors(app: &mut AppState) {
    app.foreground = Color::BLACK;
    app.background = Color::WHITE;
}

pub fn swap_colors(app: &mut AppState) {
    std::mem::swap(&mut app.foreground, &mut app.background);
}
