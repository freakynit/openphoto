//! Layers panel.

use egui::{Align, Align2, Layout, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use op_core::{BlendMode, Layer, LayerId, TiledImage};

use crate::icons;
use crate::state::{AppState, DocState};
use crate::theme::{self, color, size};
use crate::widgets;

const BOTTOM_BAR: f32 = 38.0;
const THUMB: f32 = 44.0;

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let full = ui.max_rect();
    let Some(state) = app.active() else {
        return;
    };

    ui.add_space(10.0);
    filter_row(ui);
    ui.add_space(6.0);
    blend_row(ui, state);
    ui.add_space(6.0);
    lock_row(ui, state);
    ui.add_space(6.0);

    let list_rect = Rect::from_min_max(
        Pos2::new(full.left(), ui.cursor().top()),
        Pos2::new(full.right(), full.bottom() - BOTTOM_BAR),
    );
    let bar_rect = Rect::from_min_max(Pos2::new(full.left(), list_rect.bottom()), full.max);
    ui.painter().rect_filled(list_rect, 0, color::LIST_BG);
    ui.painter().line_segment(
        [list_rect.left_top(), list_rect.right_top()],
        Stroke::new(1.0, color::SEPARATOR),
    );

    let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list_rect));
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut list_ui, |ui| layer_list(ui, state));

    bottom_bar(ui, state, bar_rect);
}

fn filter_row(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let (r, _) = ui.allocate_exact_size(Vec2::new(124.0, size::FIELD_HEIGHT), Sense::click());
        let painter = ui.painter();
        painter.rect(
            r,
            2,
            color::FIELD,
            Stroke::new(1.0, color::FIELD_BORDER),
            StrokeKind::Inside,
        );
        painter.text(
            r.left_center() + Vec2::new(8.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{}  Kind", icons::MAGNIFYING_GLASS),
            theme::body(),
            color::TEXT_DIM,
        );
        painter.text(
            r.right_center() - Vec2::new(8.0, 0.0),
            Align2::RIGHT_CENTER,
            icons::CARET_DOWN,
            theme::icon(12.0),
            color::TEXT_DIM,
        );
        ui.add_space(8.0);
        for (icon, tip) in [
            (icons::IMAGE, "Filter for pixel layers"),
            (icons::CIRCLE_HALF, "Filter for adjustment layers"),
            (icons::TEXT_T, "Filter for type layers"),
            (icons::BOUNDING_BOX, "Filter for shape layers"),
            (icons::FILE_IMAGE, "Filter for smart objects"),
        ] {
            widgets::icon_button(ui, icon, 30.0, false).on_hover_text(tip);
        }
        widgets::icon(ui, icons::TOGGLE_LEFT, 18.0, color::TEXT_DIM);
    });
}

/// What a row of controls did this frame, and how it goes into the history.
#[derive(Default)]
struct Edits {
    /// A layer property changed; the canvas must be redrawn.
    changed: bool,
    /// A discrete edit to record right away.
    record: Option<&'static str>,
    /// A continuous edit (drag or typed value) that is still in progress.
    pending: bool,
    /// A continuous edit finished; record it if anything changed.
    commit: Option<&'static str>,
}

impl Edits {
    /// Tracks a drag-value widget: changes are pending until the drag ends or
    /// the typed value is committed.
    fn track(&mut self, r: &egui::Response, name: &'static str) {
        if r.changed() {
            self.changed = true;
            self.pending = true;
        }
        if r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged()) {
            self.commit = Some(name);
        }
    }

    fn apply(self, state: &mut DocState) {
        if self.changed {
            state.doc.mark_dirty();
        }
        if self.pending {
            state.mark_pending();
        }
        if let Some(name) = self.record {
            state.record(name);
        }
        if let Some(name) = self.commit {
            state.commit_pending(name);
        }
    }
}

fn active_layer(state: &mut DocState) -> Option<&mut Layer> {
    let id = state.doc.active_layer?;
    state.doc.layer_mut(id)
}

fn blend_row(ui: &mut Ui, state: &mut DocState) {
    let mut edits = Edits::default();
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let Some(layer) = active_layer(state) else {
            return;
        };
        // The background layer's blend mode and opacity can't be changed
        let editable = !layer.is_background;
        ui.add_enabled_ui(editable, |ui| {
            egui::ComboBox::from_id_salt("blend-mode")
                .width(184.0)
                .selected_text(layer.blend_mode.label())
                .show_ui(ui, |ui| {
                    for (gi, group) in BlendMode::GROUPS.iter().enumerate() {
                        if gi > 0 {
                            ui.separator();
                        }
                        for &mode in *group {
                            if ui
                                .selectable_value(&mut layer.blend_mode, mode, mode.label())
                                .changed()
                            {
                                edits.changed = true;
                                edits.record = Some("Blending Change");
                            }
                        }
                    }
                });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                let r = widgets::percent_drag(ui, &mut layer.opacity);
                edits.track(&r, "Opacity Change");
                ui.label("Opacity:");
            });
        });
    });
    edits.apply(state);
}

fn lock_row(ui: &mut Ui, state: &mut DocState) {
    let mut edits = Edits::default();
    ui.horizontal(|ui| {
        ui.add_space(14.0);
        let Some(layer) = active_layer(state) else {
            return;
        };
        ui.label("Lock:");
        let editable = !layer.is_background;
        ui.add_enabled_ui(editable, |ui| {
            let flags: [(&str, &str, &mut bool); 3] = [
                (
                    icons::CHECKERBOARD,
                    "Lock transparent pixels",
                    &mut layer.lock_transparency,
                ),
                (
                    icons::PAINT_BRUSH,
                    "Lock image pixels",
                    &mut layer.lock_pixels,
                ),
                (
                    icons::ARROWS_OUT_CARDINAL,
                    "Lock position",
                    &mut layer.lock_position,
                ),
            ];
            for (icon, tip, flag) in flags {
                if widgets::icon_button(ui, icon, 28.0, *flag)
                    .on_hover_text(tip)
                    .clicked()
                {
                    *flag = !*flag;
                    edits.record = Some("Lock Change");
                }
            }
            widgets::icon_button(ui, icons::FRAME_CORNERS, 28.0, false)
                .on_hover_text("Prevent auto-nesting into and out of Artboards and Frames");
            widgets::icon_button(ui, icons::LOCK_SIMPLE, 28.0, layer.is_locked())
                .on_hover_text("Lock all");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(14.0);
                let r = widgets::percent_drag(ui, &mut layer.fill);
                edits.track(&r, "Fill Opacity Change");
                ui.label("Fill:");
            });
        });
    });
    edits.apply(state);
}

fn layer_list(ui: &mut Ui, state: &mut DocState) {
    ui.spacing_mut().item_spacing.y = 0.0;
    let ids: Vec<LayerId> = state.doc.layers.iter().rev().map(|l| l.id).collect();
    for id in ids {
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), size::LAYER_ROW),
            Sense::click(),
        );
        let eye_rect = Rect::from_min_size(rect.min, Vec2::new(44.0, rect.height()));
        let eye = ui.interact(eye_rect, ui.id().with(("eye", id.0)), Sense::click());

        if eye.clicked() {
            if let Some(l) = state.doc.layer_mut(id) {
                l.visible = !l.visible;
            }
            state.doc.mark_dirty();
        } else if response.clicked() {
            state.doc.active_layer = Some(id);
        }

        let thumb = state.layer_thumbnail(ui.ctx(), id, (THUMB * 2.0) as u32);
        let selected = state.doc.active_layer == Some(id);
        let Some(layer) = state.doc.layer(id) else {
            continue;
        };
        let painter = ui.painter();
        if selected {
            painter.rect_filled(rect, 0, color::ROW_SELECTED);
        } else if response.hovered() {
            painter.rect_filled(rect, 0, color::HOVER);
        }
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, color::SEPARATOR_LIGHT),
        );
        painter.line_segment(
            [eye_rect.right_top(), eye_rect.right_bottom()],
            Stroke::new(1.0, color::SEPARATOR_LIGHT),
        );
        if layer.visible {
            painter.text(
                eye_rect.center(),
                Align2::CENTER_CENTER,
                icons::EYE,
                theme::icon(18.0),
                color::ICON,
            );
        }

        let thumb_box = Rect::from_center_size(
            Pos2::new(eye_rect.right() + 10.0 + THUMB / 2.0, rect.center().y),
            Vec2::splat(THUMB),
        );
        if let Some(tex) = thumb {
            let size = tex.size_vec2();
            let scale = (THUMB / size.x).min(THUMB / size.y);
            let r = Rect::from_center_size(thumb_box.center(), size * scale);
            widgets::checkerboard(painter, r, 4.0);
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            painter.rect_stroke(
                r,
                0,
                Stroke::new(1.0, color::SEPARATOR),
                StrokeKind::Outside,
            );
        }

        let font = if layer.is_background {
            egui::FontId::new(theme::font::BODY, egui::FontFamily::Proportional)
        } else {
            theme::body()
        };
        painter.text(
            Pos2::new(thumb_box.right() + 16.0, rect.center().y),
            Align2::LEFT_CENTER,
            &layer.name,
            font,
            color::TEXT,
        );
        if layer.is_background {
            painter.text(
                rect.right_center() - Vec2::new(30.0, 0.0),
                Align2::CENTER_CENTER,
                icons::LOCK_SIMPLE,
                theme::icon(16.0),
                color::ICON,
            );
        }
    }
}

fn bottom_bar(ui: &mut Ui, state: &mut DocState, rect: Rect) {
    ui.painter().rect_filled(rect, 0, color::PANEL);
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, color::SEPARATOR),
    );
    let mut bar = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(14.0, 0.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    bar.spacing_mut().item_spacing.x = 4.0;

    let can_delete = state.doc.layers.len() > 1 && state.doc.active_layer.is_some();
    let delete = bar.add_enabled_ui(can_delete, |ui| {
        widgets::icon_button(ui, icons::TRASH, 30.0, false).on_hover_text("Delete layer")
    });
    if delete.inner.clicked() {
        delete_active_layer(state);
    }
    if widgets::icon_button(&mut bar, icons::PLUS_SQUARE, 30.0, false)
        .on_hover_text("Create a new layer")
        .clicked()
    {
        new_layer(state);
    }
    for (icon, tip) in [
        (icons::FOLDER_SIMPLE, "Create a new group"),
        (icons::CIRCLE_HALF, "Create new fill or adjustment layer"),
        (icons::SELECTION_BACKGROUND, "Add a mask"),
        (icons::SPARKLE, "Add a layer style"),
        (icons::LINK_SIMPLE, "Link layers"),
    ] {
        widgets::icon_button(&mut bar, icon, 30.0, false).on_hover_text(tip);
    }
}

pub fn new_layer(state: &mut DocState) {
    let doc = &mut state.doc;
    let n = doc.layers.iter().filter(|l| !l.is_background).count() + 1;
    let id = doc.new_layer_id();
    let layer = Layer::raster(
        id,
        format!("Layer {n}"),
        TiledImage::new(doc.width, doc.height),
    );
    let index = doc
        .active_layer
        .and_then(|a| doc.layers.iter().position(|l| l.id == a))
        .map_or(doc.layers.len(), |i| i + 1);
    doc.layers.insert(index, layer);
    doc.active_layer = Some(id);
    doc.mark_dirty();
    state.record("New Layer");
}

fn delete_active_layer(state: &mut DocState) {
    let doc = &mut state.doc;
    let Some(active) = doc.active_layer else {
        return;
    };
    let Some(i) = doc.layers.iter().position(|l| l.id == active) else {
        return;
    };
    doc.layers.remove(i);
    doc.active_layer = doc
        .layers
        .get(i.saturating_sub(1))
        .or(doc.layers.first())
        .map(|l| l.id);
    doc.mark_dirty();
    state.record("Delete Layer");
}
