//! Navigator panel: a thumbnail of the document with the part shown in the
//! window outlined in red; dragging on the thumbnail moves the view. Below,
//! the zoom percentage and a zoom slider between the zoom-out and zoom-in
//! buttons.

use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::floating::small;
use crate::document_view;
use crate::icons;
use crate::state::AppState;
use crate::theme::{self, color, pt};

/// Photoshop draws the view box in red.
const VIEW_BOX: Color32 = Color32::from_rgb(0xff, 0x00, 0x00);

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let rect = ui.max_rect();
    let ppp = ui.ctx().pixels_per_point();
    let Some(state) = app.active() else {
        return;
    };
    let area = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.bottom() - pt(32.0)));
    let (w, h) = (state.doc.width as f32, state.doc.height as f32);
    let scale = (area.width() / w).min(area.height() / h);
    let image = Rect::from_center_size(area.center(), Vec2::new(w, h) * scale);
    if let Some(tex) = state.composite_texture(ui.ctx(), 256) {
        crate::widgets::checkerboard(ui.painter(), image, 4.0);
        ui.painter().image(
            tex.id(),
            image,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    // The visible part of the document, clipped to the thumbnail
    let [x0, y0, x1, y1] = document_view::visible_rect(state, ppp);
    let view = Rect::from_min_max(
        image.min + Vec2::new(x0, y0) * scale,
        image.min + Vec2::new(x1, y1) * scale,
    )
    .intersect(image);
    ui.painter()
        .rect_stroke(view, 0, Stroke::new(1.0, VIEW_BOX), StrokeKind::Inside);
    // Dragging (or clicking) centers the view there
    let response = ui.interact(area, ui.id().with("navigator"), Sense::click_and_drag());
    if (response.dragged() || response.clicked())
        && let Some(p) = response.interact_pointer_pos()
    {
        let doc = (p - image.min) / scale;
        document_view::center_on(state, Pos2::new(doc.x, doc.y), ppp);
    }

    // Zoom percentage, zoom out, slider, zoom in
    let row = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - pt(24.0)), rect.max);
    ui.painter().text(
        row.left_center(),
        Align2::LEFT_CENTER,
        document_view::zoom_label(state.view.zoom),
        small(),
        color::TEXT,
    );
    let mut row_ui = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_max(
        Pos2::new(row.left() + pt(60.0), row.top()),
        row.max,
    )));
    row_ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icons::MOUNTAINS).font(theme::icon(pt(10.0))));
        // Logarithmic slider over Photoshop's zoom range
        let mut log = state.view.zoom.ln();
        let slider = egui::Slider::new(&mut log, 0.01f32.ln()..=128f32.ln()).show_value(false);
        if ui.add_sized([pt(120.0), pt(18.0)], slider).changed() {
            document_view::zoom_to(state, log.exp(), ppp);
        }
        ui.label(egui::RichText::new(icons::MOUNTAINS).font(theme::icon(pt(15.0))));
    });
}
