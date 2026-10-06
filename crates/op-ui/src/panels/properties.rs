//! Properties panel. Only document (Canvas) properties for now, laid out at
//! the positions measured on Photoshop 2026 (points from the panel body's
//! top-left corner) and drawn with shapes traced from it at 2x.

use egui::{Align2, Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};
use op_core::{BitDepth, ColorMode};

use crate::state::AppState;
use crate::theme::{self, color, pt};
use crate::widgets;

/// Labels in the panel.
const LABEL: Color32 = Color32::from_gray(0xd6);
/// Values in fields and dropdowns, and section titles.
const VALUE: Color32 = Color32::from_gray(0xf0);
/// The document icon's and orientation buttons' boxes.
const BOX_FILL: Color32 = Color32::from_gray(0x38);
const BOX_BORDER: Color32 = Color32::from_gray(0x63);
const ICON: Color32 = Color32::from_gray(0xd7);
/// The content's height; Photoshop's panel continues below with sections
/// this app doesn't have yet.
const CONTENT_H: f32 = pt(228.0);

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let Some(state) = app.active() else {
        empty(ui);
        return;
    };
    let doc = &mut state.doc;

    egui::ScrollArea::vertical().show(ui, |ui| {
        let (content, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), CONTENT_H), Sense::hover());
        let at = |x: f32, y: f32| content.min + Vec2::new(pt(x), pt(y));
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        let painter = ui.painter().clone();
        let text = |x: f32, cy: f32, s: &str, align: Align2, font: egui::FontId, c: Color32| {
            painter.text(
                Pos2::new(at(x, 0.0).x, at(0.0, cy).y + TEXT_DY),
                align,
                s,
                font,
                c,
            );
        };

        // The kind of properties: a document
        let doc_box = rect(10.0, 5.0, 34.0, 29.0);
        painter.rect(
            doc_box,
            0,
            BOX_FILL,
            Stroke::new(pt(1.0), BOX_BORDER),
            StrokeKind::Inside,
        );
        document_icon(&painter, doc_box.center());
        text(
            37.5,
            17.75,
            "Document",
            Align2::LEFT_CENTER,
            theme::body(),
            LABEL,
        );
        painter.rect_filled(
            Rect::from_min_size(at(0.0, 33.0), Vec2::new(content.width(), pt(1.0))),
            0,
            color::OPTIONS_SEPARATOR,
        );

        // The Canvas section, collapsible
        let id = ui.id().with("canvas-section");
        let mut open = ui.data_mut(|d| *d.get_temp_mut_or(id, true));
        let header = ui.interact(
            rect(0.0, 38.0, 120.0, 60.0),
            id.with("header"),
            Sense::click(),
        );
        if header.clicked() {
            open = !open;
        }
        ui.data_mut(|d| d.insert_temp(id, open));
        let c = at(11.0, 49.5);
        let chevron = if open {
            vec![c + v(-4.0, -2.0), c + v(0.0, 2.0), c + v(4.0, -2.0)]
        } else {
            vec![c + v(-2.0, -4.0), c + v(2.0, 0.0), c + v(-2.0, 4.0)]
        };
        painter.add(Shape::line(
            chevron,
            Stroke::new(pt(1.1), Color32::from_gray(0xe0)),
        ));
        text(
            21.0,
            49.25,
            "Canvas",
            Align2::LEFT_CENTER,
            theme::semibold(theme::font::BODY),
            VALUE,
        );
        if !open {
            return;
        }

        // W, H (with the constraint link) and X, Y
        link_icon(&painter, at(23.25, 84.25));
        let (w, h) = (format!("{} px", doc.width), format!("{} px", doc.height));
        for (row, (axis, value, pos_axis)) in
            [("W", &w, "X"), ("H", &h, "Y")].into_iter().enumerate()
        {
            let y = 64.0 + 24.0 * row as f32;
            text(
                59.5,
                y + 8.25,
                axis,
                Align2::RIGHT_CENTER,
                theme::body(),
                LABEL,
            );
            field(&painter, rect(64.0, y, 118.0, y + 17.0), value, true);
            text(
                138.0,
                y + 8.25,
                pos_axis,
                Align2::RIGHT_CENTER,
                theme::body(),
                LABEL,
            );
            field(&painter, rect(142.5, y, 196.5, y + 17.0), "0 px", false);
        }

        // Orientation: the current one has a box
        let portrait = doc.height >= doc.width;
        if portrait {
            painter.rect(
                rect(65.0, 115.0, 91.0, 141.0),
                0,
                BOX_FILL,
                Stroke::new(pt(1.0), BOX_BORDER),
                StrokeKind::Inside,
            );
        } else {
            painter.rect(
                rect(91.0, 115.0, 117.0, 141.0),
                0,
                BOX_FILL,
                Stroke::new(pt(1.0), BOX_BORDER),
                StrokeKind::Inside,
            );
        }
        orientation_icon(&painter, at(78.0, 128.0), true);
        orientation_icon(&painter, at(104.0, 128.0), false);

        text(
            64.5,
            156.75,
            &format!("Resolution: {} pixels/inch", doc.resolution.round()),
            Align2::LEFT_CENTER,
            theme::body(),
            LABEL,
        );

        // Mode and bit depth: only RGB, 8 bits is implemented so far
        text(
            59.0,
            183.25,
            "Mode",
            Align2::RIGHT_CENTER,
            theme::body(),
            LABEL,
        );
        let mode_rect = rect(65.0, 174.0, 197.0, 193.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(mode_rect), |ui| {
            widgets::dropdown(
                ui,
                "doc-mode",
                mode_rect.width(),
                doc.color_mode.label(),
                |ui| {
                    for m in ColorMode::ALL {
                        ui.add_enabled_ui(m == ColorMode::Rgb, |ui| {
                            ui.selectable_value(&mut doc.color_mode, m, m.label());
                        });
                    }
                },
            );
        });
        let depth_rect = rect(65.0, 200.0, 197.0, 219.0);
        ui.scope_builder(egui::UiBuilder::new().max_rect(depth_rect), |ui| {
            widgets::dropdown(
                ui,
                "doc-depth",
                depth_rect.width(),
                doc.bit_depth.label(),
                |ui| {
                    for d in BitDepth::ALL {
                        ui.add_enabled_ui(d == BitDepth::U8, |ui| {
                            ui.selectable_value(&mut doc.bit_depth, d, d.label());
                        });
                    }
                },
            );
        });
    });
}

/// Moves text so its capital letters' center lands on the measured line.
const TEXT_DY: f32 = 0.0;

fn v(x: f32, y: f32) -> Vec2 {
    Vec2::new(pt(x), pt(y))
}

/// A Photoshop text field: 1 pt `#666666` border on `#454545`, or the
/// dimmer disabled look.
fn field(painter: &egui::Painter, rect: Rect, value: &str, enabled: bool) {
    let (fill, border, text) = if enabled {
        (color::FIELD, color::DROPDOWN_BORDER, VALUE)
    } else {
        (
            Color32::from_gray(0x4d),
            Color32::from_gray(0x5e),
            Color32::from_gray(0x6a),
        )
    };
    painter.rect(
        rect,
        0,
        fill,
        Stroke::new(pt(1.0), border),
        StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(rect.left() + pt(4.0), rect.center().y + TEXT_DY),
        Align2::LEFT_CENTER,
        value,
        theme::body(),
        text,
    );
}

/// Traced at 2x around the box's center: a page with a folded corner.
fn document_icon(painter: &egui::Painter, c: Pos2) {
    let p = |x: f32, y: f32| c + v(x / 2.0, y / 2.0);
    let stroke = Stroke::new(pt(1.0), ICON);
    painter.add(Shape::closed_line(
        vec![
            p(-9.0, 11.0),
            p(-9.0, -13.0),
            p(5.0, -13.0),
            p(11.0, -7.0),
            p(11.0, 11.0),
        ],
        stroke,
    ));
    painter.add(Shape::line(
        vec![p(3.0, -13.0), p(3.0, -5.0), p(11.0, -5.0)],
        stroke,
    ));
}

/// The W/H constraint link, traced at 2x: two links of a chain, upright.
fn link_icon(painter: &egui::Painter, c: Pos2) {
    let p = |x: f32, y: f32| c + v(x / 2.0, y / 2.0);
    let stroke = Stroke::new(pt(1.0), ICON);
    for (cy, start) in [(-7.5f32, 120.0f32), (7.5, -60.0)] {
        let arc: Vec<Pos2> = (0..=16)
            .map(|k| {
                let a = (start + 300.0 * k as f32 / 16.0).to_radians();
                p(6.0 * a.cos(), cy + 6.0 * a.sin())
            })
            .collect();
        painter.add(Shape::line(arc, stroke));
    }
    painter.rect_filled(Rect::from_min_max(p(-1.5, -6.0), p(1.5, 6.0)), 0, ICON);
}

/// The orientation icons, traced at 2x: a frame with a person in it.
fn orientation_icon(painter: &egui::Painter, c: Pos2, portrait: bool) {
    let p = |x: f32, y: f32| c + v(x / 2.0, y / 2.0);
    let stroke = Stroke::new(pt(1.0), ICON);
    if portrait {
        painter.rect_stroke(
            Rect::from_min_max(p(-14.0, -16.0), p(14.0, 16.0)),
            0,
            stroke,
            StrokeKind::Inside,
        );
        painter.circle_filled(p(0.0, -7.5), pt(2.0), ICON);
        painter.rect_filled(
            Rect::from_min_max(p(-8.0, -2.0), p(8.0, 10.0)),
            pt(1.5),
            ICON,
        );
        painter.rect_filled(Rect::from_min_max(p(-6.0, 9.0), p(6.0, 14.0)), 0, ICON);
    } else {
        painter.rect_stroke(
            Rect::from_min_max(p(-16.0, -12.0), p(16.0, 12.0)),
            0,
            stroke,
            StrokeKind::Inside,
        );
        painter.circle_filled(p(0.0, -4.5), pt(1.75), ICON);
        painter.rect_filled(
            Rect::from_min_max(p(-8.0, 2.0), p(8.0, 10.0)),
            pt(1.5),
            ICON,
        );
    }
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
