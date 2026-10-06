//! Properties panel. Only document (Canvas) properties for now.

use egui::{Align, Align2, Layout, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use op_core::{BitDepth, ColorMode};

use crate::icons;
use crate::state::AppState;
use crate::theme::{self, color, size};
use crate::widgets;

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let Some(state) = app.active() else {
        empty(ui);
        return;
    };
    let doc = &mut state.doc;

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
            ui.painter().rect(
                r,
                3,
                color::FIELD,
                Stroke::new(1.0, color::FIELD_BORDER),
                StrokeKind::Inside,
            );
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                icons::FILE,
                theme::icon(18.0),
                color::ICON,
            );
            ui.label("Document");
        });
        ui.add_space(10.0);
        widgets::hseparator(ui);
        ui.add_space(10.0);

        let id = ui.id().with("canvas-section");
        let mut open = ui.data_mut(|d| *d.get_temp_mut_or(id, true));
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            let caret = if open {
                icons::CARET_DOWN
            } else {
                icons::CARET_RIGHT
            };
            let r = ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{caret}  Canvas"))
                        .font(theme::semibold(theme::font::BODY)),
                )
                .sense(Sense::click()),
            );
            if r.clicked() {
                open = !open;
            }
        });
        ui.data_mut(|d| d.insert_temp(id, open));
        if !open {
            return;
        }
        ui.add_space(8.0);

        let (w, h) = (format!("{} px", doc.width), format!("{} px", doc.height));
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            // Width/height constraint link
            let (r, _) = ui.allocate_exact_size(
                Vec2::new(28.0, size::FIELD_HEIGHT * 2.0 + 8.0),
                Sense::hover(),
            );
            ui.painter().text(
                r.center(),
                Align2::CENTER_CENTER,
                icons::LINK_SIMPLE,
                theme::icon(16.0),
                color::ICON,
            );
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for (axis, value, pos_axis) in [("W", &w, "X"), ("H", &h, "Y")] {
                    ui.horizontal(|ui| {
                        right_label(ui, axis, 22.0);
                        widgets::field(ui, value, 78.0, true);
                        ui.add_space(12.0);
                        right_label(ui, pos_axis, 14.0);
                        widgets::field(ui, "0 px", 78.0, false);
                    });
                }
            });
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0 + 28.0 + 30.0);
            orientation_button(ui, true, doc.height >= doc.width);
            orientation_button(ui, false, doc.width > doc.height);
        });

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.add_space(14.0 + 28.0 + 30.0);
            ui.label(format!(
                "Resolution: {} pixels/inch",
                doc.resolution.round()
            ));
        });
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.add_space(14.0);
            right_label(ui, "Mode", 58.0);
            egui::ComboBox::from_id_salt("doc-mode")
                .width(180.0)
                .selected_text(doc.color_mode.label())
                .show_ui(ui, |ui| {
                    for m in ColorMode::ALL {
                        // Only RGB is implemented so far
                        ui.add_enabled_ui(m == ColorMode::Rgb, |ui| {
                            ui.selectable_value(&mut doc.color_mode, m, m.label());
                        });
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0 + 58.0 + 8.0);
            egui::ComboBox::from_id_salt("doc-depth")
                .width(180.0)
                .selected_text(doc.bit_depth.label())
                .show_ui(ui, |ui| {
                    for d in BitDepth::ALL {
                        ui.add_enabled_ui(d == BitDepth::U8, |ui| {
                            ui.selectable_value(&mut doc.bit_depth, d, d.label());
                        });
                    }
                });
        });
        ui.add_space(12.0);
    });
}

fn right_label(ui: &mut Ui, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, size::FIELD_HEIGHT),
        Layout::right_to_left(Align::Center),
        |ui| ui.label(text),
    );
}

fn orientation_button(ui: &mut Ui, portrait: bool, selected: bool) {
    let (r, response) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect(
            r,
            3,
            color::TOOL_ACTIVE,
            Stroke::new(1.0, color::TEXT_DIM),
            StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(r, 3, color::HOVER);
    }
    let size = if portrait {
        Vec2::new(12.0, 17.0)
    } else {
        Vec2::new(17.0, 12.0)
    };
    painter.rect_stroke(
        Rect::from_center_size(r.center(), size),
        1,
        Stroke::new(1.5, color::ICON),
        StrokeKind::Inside,
    );
}

fn empty(ui: &mut Ui) {
    ui.painter().text(
        ui.max_rect().center(),
        Align2::CENTER_CENTER,
        "No Properties",
        theme::small(),
        color::TEXT_DISABLED,
    );
}
