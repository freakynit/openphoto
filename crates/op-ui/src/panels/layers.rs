//! Layers panel.

use egui::{Align2, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use op_core::{BlendMode, Layer, LayerId, TiledImage};

use crate::icons;
use crate::ps_icons::Icon;
use crate::state::{AppState, DocState};
use crate::theme::{self, color, pt, size};
use crate::widgets;

/// Photoshop 2026's Layers panel, measured (points from the panel body's
/// top-left corner): three rows of controls above the list, which starts
/// at y 90, and a 25 pt footer.
const LIST_TOP: f32 = pt(90.0);
const FOOTER: f32 = pt(25.0);
/// The list keeps a scrollbar gutter on its right.
const GUTTER: f32 = pt(16.0);
/// Thumbnails fit in a box this wide and tall (Photoshop 2026), 4 pt below
/// the row's top; a row is the thumbnail's height plus 8 pt, and a 1 pt
/// `#454545` line under it.
const THUMB_W: f32 = pt(32.5);
const THUMB_H: f32 = pt(33.5);

/// The size of the document's thumbnails as drawn.
fn thumb_size(doc: &op_core::Document) -> Vec2 {
    let (w, h) = (doc.width.max(1) as f32, doc.height.max(1) as f32);
    let scale = (THUMB_W / w).min(THUMB_H / h);
    Vec2::new(w * scale, h * scale)
}

/// A row's height without its bottom line.
fn row_height(doc: &op_core::Document) -> f32 {
    thumb_size(doc).y + pt(8.0)
}

/// Space between the layer and mask thumbnails (the link icon sits in it).
const MASK_GAP: f32 = pt(10.0);
/// Group rows, without their 1 pt bottom line (Photoshop 2026).
const GROUP_ROW: f32 = pt(24.0);
/// Each level of grouping moves a row's contents right by this much.
const INDENT: f32 = pt(16.0);
/// The eye column, and the 1 pt line right of it.
const EYE_W: f32 = pt(29.5);
const LINE: egui::Color32 = egui::Color32::from_gray(0x45);
/// Disabled text and field colors (Photoshop greys the background
/// layer's blend mode, opacity and fill).
const TEXT_OFF: egui::Color32 = egui::Color32::from_gray(0x87);

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let full = ui.max_rect();
    let Some(state) = app.active() else {
        return;
    };
    let at = |x: f32, y: f32| full.min + Vec2::new(pt(x), pt(y));
    let painter = ui.painter().clone();
    for y in [31.0, 59.0] {
        painter.rect_filled(
            Rect::from_min_size(at(0.0, y), Vec2::new(full.width(), pt(1.0))),
            0,
            color::OPTIONS_SEPARATOR,
        );
    }
    filter_row(ui, full);
    blend_row(ui, state, full);
    lock_row(ui, state, full);

    let list_rect = Rect::from_min_max(
        Pos2::new(full.left(), full.top() + LIST_TOP),
        Pos2::new(full.right(), full.bottom() - FOOTER),
    );
    let bar_rect = Rect::from_min_max(Pos2::new(full.left(), list_rect.bottom()), full.max);
    painter.rect_filled(list_rect, 0, color::LIST_BG);
    painter.rect_filled(
        Rect::from_min_size(
            list_rect.min - Vec2::new(0.0, pt(0.5)),
            Vec2::new(full.width(), pt(0.5)),
        ),
        0,
        egui::Color32::from_gray(0x4a),
    );

    let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list_rect));
    let from_background = egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(&mut list_ui, |ui| layer_list(ui, state))
        .inner;
    if from_background {
        app.new_layer_dialog = Some(crate::dialogs::NewLayerDialog::from_background());
        return;
    }

    let (layer_dialog, group_dialog, delete) = bottom_bar(ui, state, bar_rect);
    if delete {
        app.delete_layers();
    } else if layer_dialog {
        let name = state.doc.next_layer_name();
        app.new_layer_dialog = Some(crate::dialogs::NewLayerDialog::new(name));
    } else if group_dialog {
        let name = state.doc.next_group_name();
        app.new_layer_dialog = Some(crate::dialogs::NewLayerDialog::group(
            name,
            crate::dialogs::NewLayerKind::Group,
        ));
    }
}

/// Paints one of the traced icons centered at `center`.
fn icon(
    painter: &egui::Painter,
    center: Pos2,
    icon: Icon,
    enabled: bool,
    background: egui::Color32,
) {
    let tint = if enabled {
        color::OPTIONS_ICON
    } else {
        color::OPTIONS_ICON_DISABLED
    };
    crate::ps_icons::paint(painter, center, icon, tint, background);
}

/// A Photoshop field box: enabled `#454545` with a `#666666` border, or
/// the disabled `#4d4d4d` with `#5e5e5e`.
fn field_box(painter: &egui::Painter, rect: Rect, enabled: bool) {
    let (fill, border) = if enabled {
        (color::FIELD, color::DROPDOWN_BORDER)
    } else {
        (color::LIST_BG, egui::Color32::from_gray(0x5e))
    };
    painter.rect(
        rect,
        0,
        fill,
        Stroke::new(pt(1.0), border),
        StrokeKind::Inside,
    );
}

fn text_color(enabled: bool) -> egui::Color32 {
    if enabled {
        color::TEXT_BRIGHT
    } else {
        TEXT_OFF
    }
}

/// Filter by kind: off, so dimmed as in Photoshop. Nothing filters yet.
fn filter_row(ui: &mut Ui, full: Rect) {
    let at = |x: f32, y: f32| full.min + Vec2::new(pt(x), pt(y));
    let painter = ui.painter();
    let kind = Rect::from_min_max(at(3.0, 8.5), at(92.0, 27.5));
    field_box(painter, kind, false);
    let search = at(17.0, 18.0);
    let p = |x: f32, y: f32| search + Vec2::new(pt(x / 2.0), pt(y / 2.0));
    painter.circle_stroke(p(-1.5, -1.5), pt(3.25), Stroke::new(pt(1.25), TEXT_OFF));
    painter.line_segment([p(3.5, 3.5), p(9.0, 9.0)], Stroke::new(pt(1.5), TEXT_OFF));
    painter.text(
        at(25.5, 18.0),
        Align2::LEFT_CENTER,
        "Kind",
        theme::body(),
        TEXT_OFF,
    );
    crate::ps_icons::paint(
        painter,
        at(84.75, 18.25),
        Icon::Caret,
        egui::Color32::from_gray(0x66),
        color::LIST_BG,
    );
    for (x, i, tip) in [
        (110.0, Icon::FilterPixel, "Filter for pixel layers"),
        (
            133.5,
            Icon::FilterAdjustment,
            "Filter for adjustment layers",
        ),
        (157.75, Icon::FilterType, "Filter for type layers"),
        (182.0, Icon::FilterShape, "Filter for shape layers"),
        (207.0, Icon::FilterSmartObject, "Filter for smart objects"),
    ] {
        icon(painter, at(x, 18.0), i, false, color::PANEL);
        ui.interact(
            Rect::from_center_size(at(x, 18.0), Vec2::splat(pt(20.0))),
            ui.id().with(tip),
            Sense::hover(),
        )
        .on_hover_text(tip);
    }
    // The filtering on/off switch (off)
    let switch = Rect::from_center_size(at(232.0, 18.0), Vec2::new(pt(10.0), pt(18.0)));
    painter.rect(
        switch,
        pt(5.0),
        color::LIST_BG,
        Stroke::new(pt(0.5), egui::Color32::from_gray(0x6a)),
        StrokeKind::Inside,
    );
    painter.circle_filled(at(232.0, 13.5), pt(4.0), egui::Color32::from_gray(0x99));
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

/// A percentage field with Photoshop's look and a chevron box; dragging
/// or typing changes the value. Returns the drag value's response.
fn percent_field(
    ui: &mut Ui,
    full: Rect,
    y: f32,
    value: &mut f32,
    enabled: bool,
) -> egui::Response {
    let at = |x: f32, y: f32| full.min + Vec2::new(pt(x), pt(y));
    let h = if y < 60.0 { 19.0 } else { 18.0 };
    let field = Rect::from_min_max(at(180.0, y), at(217.5, y + h));
    let chevron = Rect::from_min_max(at(216.5, y), at(232.0, y + h));
    field_box(ui.painter(), chevron, enabled);
    field_box(ui.painter(), field, enabled);
    crate::ps_icons::paint(
        ui.painter(),
        chevron.center() + Vec2::new(0.0, pt(0.25)),
        Icon::Caret,
        if enabled {
            color::OPTIONS_ICON
        } else {
            egui::Color32::from_gray(0x6a)
        },
        color::FIELD,
    );
    let mut pct = (*value * 100.0).round();
    // The value sits on the left like Photoshop's; the field's own colors
    // replace egui's (and its fading of disabled widgets)
    let r = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(field.shrink(pt(1.0)))
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                let style = ui.style_mut();
                style.spacing.button_padding = Vec2::new(pt(5.5), 0.0);
                style.spacing.interact_size = Vec2::new(pt(10.0), field.height() - pt(2.0));
                let v = &mut style.visuals;
                v.override_text_color = Some(text_color(enabled));
                for w in [
                    &mut v.widgets.noninteractive,
                    &mut v.widgets.inactive,
                    &mut v.widgets.hovered,
                    &mut v.widgets.active,
                ] {
                    w.weak_bg_fill = egui::Color32::TRANSPARENT;
                    w.bg_fill = egui::Color32::TRANSPARENT;
                    w.bg_stroke = Stroke::NONE;
                    w.fg_stroke.color = text_color(enabled);
                }
                ui.add_enabled(
                    enabled,
                    egui::DragValue::new(&mut pct)
                        .range(0.0..=100.0)
                        .speed(0.5)
                        .max_decimals(0)
                        .suffix("%"),
                )
            },
        )
        .inner;
    if r.changed() {
        *value = pct / 100.0;
    }
    r
}

fn blend_row(ui: &mut Ui, state: &mut DocState, full: Rect) {
    let at = |x: f32, y: f32| full.min + Vec2::new(pt(x), pt(y));
    let mut edits = Edits::default();
    let Some(layer) = active_layer(state) else {
        return;
    };
    // The background layer's blend mode and opacity can't be changed
    let editable = !layer.is_background;
    let mode = Rect::from_min_max(at(3.0, 36.5), at(134.5, 55.5));
    ui.scope_builder(egui::UiBuilder::new().max_rect(mode), |ui| {
        let label = layer.blend_mode.label();
        let is_group = layer.is_group();
        widgets::dropdown_with(ui, "blend-mode", mode.width(), label, editable, |ui| {
            // A group can also pass through
            if is_group
                && ui
                    .selectable_value(
                        &mut layer.blend_mode,
                        BlendMode::PassThrough,
                        "Pass Through",
                    )
                    .changed()
            {
                edits.changed = true;
                edits.record = Some("Blending Change");
            }
            for (gi, group) in BlendMode::GROUPS.iter().enumerate() {
                if gi > 0 {
                    ui.separator();
                }
                for &m in *group {
                    if ui
                        .selectable_value(&mut layer.blend_mode, m, m.label())
                        .changed()
                    {
                        edits.changed = true;
                        edits.record = Some("Blending Change");
                    }
                }
            }
        });
    });
    ui.painter().text(
        at(177.0, 46.0),
        Align2::RIGHT_CENTER,
        "Opacity:",
        theme::body(),
        text_color(editable),
    );
    let r = percent_field(ui, full, 36.5, &mut layer.opacity, editable);
    edits.track(&r, "Opacity Change");
    edits.apply(state);
}

fn lock_row(ui: &mut Ui, state: &mut DocState, full: Rect) {
    let at = |x: f32, y: f32| full.min + Vec2::new(pt(x), pt(y));
    let mut edits = Edits::default();
    let Some(layer) = active_layer(state) else {
        return;
    };
    let editable = !layer.is_background;
    ui.painter().text(
        at(6.0, 73.5),
        Align2::LEFT_CENTER,
        "Lock:",
        theme::body(),
        text_color(editable),
    );
    let all = layer.is_locked();
    let flags: [(f32, Icon, &str, Option<&mut bool>); 5] = [
        (
            43.5,
            Icon::LockTransparent,
            "Lock transparent pixels",
            Some(&mut layer.lock_transparency),
        ),
        (
            67.0,
            Icon::LockPixels,
            "Lock image pixels",
            Some(&mut layer.lock_pixels),
        ),
        (
            88.0,
            Icon::LockPosition,
            "Lock position",
            Some(&mut layer.lock_position),
        ),
        (
            110.5,
            Icon::LockArtboards,
            "Prevent auto-nesting into and out of Artboards and Frames",
            None,
        ),
        (129.0, Icon::LockAll, "Lock all", None),
    ];
    for (x, i, tip, flag) in flags {
        let center = at(x, 73.75);
        let rect = Rect::from_center_size(center, Vec2::splat(pt(20.0)));
        let response = ui.interact(rect, ui.id().with(tip), Sense::click());
        let on = flag
            .as_deref()
            .copied()
            .unwrap_or(i == Icon::LockAll && all && editable);
        if on {
            ui.painter().rect_filled(rect, pt(2.0), color::TOOL_ACTIVE);
        } else if editable && response.hovered() {
            ui.painter().rect_filled(rect, pt(2.0), color::HOVER);
        }
        icon(ui.painter(), center, i, editable, color::PANEL);
        let response = response.on_hover_text(tip);
        if editable
            && response.clicked()
            && let Some(flag) = flag
        {
            *flag = !*flag;
            edits.record = Some("Lock Change");
        }
    }
    ui.painter().text(
        at(177.0, 73.5),
        Align2::RIGHT_CENTER,
        "Fill:",
        theme::body(),
        text_color(editable),
    );
    let r = percent_field(ui, full, 64.5, &mut layer.fill, editable);
    edits.track(&r, "Fill Opacity Change");
    edits.apply(state);
}

/// The rows. Returns true when double-clicking the background asks for
/// the Layer from Background dialog.
fn layer_list(ui: &mut Ui, state: &mut DocState) -> bool {
    let mut from_background = false;
    ui.spacing_mut().item_spacing.y = 0.0;
    // Top to bottom, as listed; layers in collapsed groups aren't
    let rows = visible_rows(&state.doc);
    // Each row's top and bottom, for dropping a dragged row
    let mut spans: Vec<(f32, f32)> = Vec::new();
    // The row being dragged (its position in the list) and where it is
    let mut dragged: Option<(usize, Pos2, bool)> = None;
    let ts = thumb_size(&state.doc);
    for (row, &(id, depth)) in rows.iter().enumerate() {
        let is_group = state.doc.layer(id).is_some_and(|l| l.is_group());
        // Group rows are 24 pt (Photoshop 2026), layers fit their thumbnail
        let row_h = if is_group {
            GROUP_ROW
        } else {
            row_height(&state.doc)
        };
        let indent = INDENT * depth as f32;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), row_h + pt(1.0)),
            Sense::click_and_drag(),
        );
        spans.push((rect.top(), rect.bottom()));
        // The row proper, left of the scrollbar gutter
        let row_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right() - GUTTER, rect.top() + row_h),
        );
        let eye_rect = Rect::from_min_size(rect.min, Vec2::new(EYE_W, row_h));
        let eye = ui.interact(eye_rect, ui.id().with(("eye", id.0)), Sense::click());
        let lock_center = Pos2::new(row_rect.right() - pt(21.5), rect.top() + row_h / 2.0);
        let lock_rect = Rect::from_center_size(lock_center, Vec2::splat(pt(16.0)));
        let is_background = state.doc.layer(id).is_some_and(|l| l.is_background);
        let has_mask = state.doc.layer(id).is_some_and(|l| l.mask.is_some());
        // The layer thumbnail, then the mask's (with a link icon between)
        // The layer thumbnail as drawn, then the mask's (with a link icon
        // between); the name starts 8 pt right of the last one
        let thumb_box = Rect::from_min_size(
            Pos2::new(rect.left() + pt(34.0) + indent, rect.top() + pt(4.0)),
            ts,
        );
        let mask_box = has_mask.then(|| thumb_box.translate(Vec2::new(ts.x + MASK_GAP, 0.0)));
        let name_x = if is_group {
            rect.left() + pt(71.0) + indent
        } else {
            mask_box.unwrap_or(thumb_box).right() + pt(8.0)
        };
        // A group's expand/collapse arrow
        let arrow_center = Pos2::new(rect.left() + pt(38.0) + indent, rect.top() + pt(11.75));
        let arrow = is_group.then(|| {
            ui.interact(
                Rect::from_center_size(arrow_center, Vec2::splat(pt(16.0))),
                ui.id().with(("arrow", id.0)),
                Sense::click(),
            )
        });
        // Clicking the background's lock turns it into a regular layer
        let lock = is_background
            .then(|| ui.interact(lock_rect, ui.id().with(("lock", id.0)), Sense::click()));

        if arrow.as_ref().is_some_and(|a| a.clicked()) {
            // Not a history state, as in Photoshop
            if let Some(op_core::LayerKind::Group { collapsed }) =
                state.doc.layer_mut(id).map(|l| &mut l.kind)
            {
                *collapsed = !*collapsed;
            }
        } else if eye.clicked() {
            if let Some(l) = state.doc.layer_mut(id) {
                l.visible = !l.visible;
            }
            state.doc.mark_dirty();
        } else if lock.as_ref().is_some_and(|l| l.clicked()) {
            if op_core::layer_ops::layer_from_background(&mut state.doc) {
                state.record("Layer From Background");
            }
        } else if response.double_clicked()
            && response
                .interact_pointer_pos()
                .is_some_and(|p| p.x > name_x - 16.0)
        {
            // Double-clicking the name renames the layer; on the background
            // it makes it a regular layer (Photoshop asks for a name first)
            if is_background {
                from_background = true;
            } else if let Some(l) = state.doc.layer(id) {
                state.renaming = Some((id, l.name.clone()));
            }
        } else if response.clicked() {
            let p = response.interact_pointer_pos().unwrap_or_default();
            let (shift, cmd) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
            let on_mask = mask_box.is_some_and(|b| b.contains(p));
            // Cmd-click adds or removes a layer, Shift-click selects a range
            // (Shift on a mask thumbnail turns the mask off instead)
            if cmd {
                state.doc.toggle_layer_selection(id);
            } else if shift && !on_mask {
                state.doc.select_layer_range(id);
            } else if !(shift && on_mask && state.doc.active_layer == Some(id)) {
                state.doc.select_layer(id);
            }
            if on_mask {
                if shift {
                    // Shift-click turns the mask off and on
                    match op_core::layer_ops::toggle_mask(&mut state.doc) {
                        Some(true) => state.record("Enable Layer Mask"),
                        Some(false) => state.record("Disable Layer Mask"),
                        None => {}
                    }
                } else {
                    state.doc.mask_target = true;
                }
            } else {
                // The layer itself (its thumbnail or name) is the target
                state.doc.mask_target = false;
            }
        }
        if (response.dragged() || response.drag_stopped())
            && let Some(p) = ui.ctx().pointer_latest_pos()
        {
            dragged = Some((row, p, response.drag_stopped()));
        }

        let thumb = state.layer_thumbnail(ui.ctx(), id, (THUMB_H * 3.0) as u32);
        let selected = state.doc.is_layer_selected(id);
        let active = state.doc.active_layer == Some(id);
        let Some(layer) = state.doc.layer(id) else {
            continue;
        };
        let painter = ui.painter();
        // The eye column keeps the panel's color; the rest of the row is
        // highlighted when selected (Photoshop 2026)
        let label = state
            .doc
            .layer(id)
            .and_then(|l| layer_color(l.color))
            .unwrap_or(color::PANEL);
        painter.rect_filled(eye_rect, 0, label);
        let body = Rect::from_min_max(
            Pos2::new(eye_rect.right() + pt(1.0), rect.top()),
            row_rect.max,
        );
        if selected {
            painter.rect_filled(body, 0, color::ROW_SELECTED);
        } else if response.hovered() {
            painter.rect_filled(body, 0, color::HOVER);
        } else {
            painter.rect_filled(body, 0, color::PANEL);
        }
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(eye_rect.right(), rect.top()),
                Vec2::new(pt(1.0), row_h),
            ),
            0,
            LINE,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + row_h),
                Vec2::new(rect.width(), pt(1.0)),
            ),
            0,
            LINE,
        );
        if layer.visible {
            icon(
                painter,
                Pos2::new(rect.left() + pt(15.0), rect.top() + row_h / 2.0 + pt(0.25)),
                Icon::Eye,
                true,
                color::PANEL,
            );
        }

        let mask_thumb =
            mask_box.and_then(|_| state.mask_thumbnail(ui.ctx(), id, (THUMB_H * 2.0) as u32));
        let Some(layer) = state.doc.layer(id) else {
            continue;
        };
        if let Some(tex) = thumb {
            // With a 1 pt dark frame inside
            let r = thumb_box;
            widgets::checkerboard(painter, r, pt(2.0));
            painter.image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            painter.rect_stroke(
                r,
                0,
                Stroke::new(pt(1.0), egui::Color32::from_gray(0x2e)),
                StrokeKind::Inside,
            );
        }
        if let (Some(mbox), Some(tex)) = (mask_box, mask_thumb) {
            let r = mbox;
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
            painter.text(
                Pos2::new(thumb_box.right() + MASK_GAP / 2.0, rect.center().y),
                Align2::CENTER_CENTER,
                icons::LINK_SIMPLE,
                theme::icon(12.0),
                color::ICON,
            );
            // A disabled mask is crossed out in red
            if layer.mask.as_ref().is_some_and(|m| !m.enabled) {
                let red = Stroke::new(2.0, egui::Color32::from_rgb(0xe0, 0x30, 0x30));
                painter.line_segment([r.left_top(), r.right_bottom()], red);
                painter.line_segment([r.right_top(), r.left_bottom()], red);
            }
        }
        // The edit target (pixels or mask) of the selected layer gets a
        // 1.5 pt white frame 0.5 pt outside its thumbnail (Photoshop 2026
        // leaves it off the background layer)
        if active && !layer.is_background {
            let target = match mask_box {
                Some(mbox) if state.doc.mask_target => mbox,
                _ => thumb_box,
            };
            painter.rect_stroke(
                target.expand(pt(0.5)),
                0,
                Stroke::new(pt(1.5), egui::Color32::WHITE),
                StrokeKind::Outside,
            );
        }

        if let op_core::LayerKind::Group { collapsed } = layer.kind {
            let arrow_icon = if collapsed {
                Icon::GroupCollapsed
            } else {
                Icon::GroupExpanded
            };
            crate::ps_icons::paint(
                painter,
                arrow_center,
                arrow_icon,
                egui::Color32::from_gray(0xe0),
                color::PANEL,
            );
            icon(
                painter,
                Pos2::new(rect.left() + pt(53.0) + indent, rect.top() + pt(11.5)),
                Icon::Folder,
                true,
                color::PANEL,
            );
        }
        let font = if layer.is_background {
            egui::FontId::new(theme::font::BODY, egui::FontFamily::Proportional)
        } else {
            theme::body()
        };
        let name_pos = Pos2::new(name_x, rect.top() + row_h / 2.0);
        if state.renaming.as_ref().is_some_and(|(r, _)| *r == id) {
            rename_field(ui, state, id, name_pos, rect);
            continue;
        }
        painter.text(
            name_pos,
            Align2::LEFT_CENTER,
            &layer.name,
            font,
            color::TEXT_BRIGHT,
        );
        if layer.is_background {
            let bg = if selected {
                color::ROW_SELECTED
            } else {
                color::PANEL
            };
            icon(painter, lock_center, Icon::LayerLock, true, bg);
        }
    }
    if let Some((from_row, pointer, released)) = dragged {
        drop_layer(ui, state, &rows, &spans, from_row, pointer, released);
    }
    from_background
}

/// The inline text field for renaming a layer: Enter or clicking elsewhere
/// commits, Escape cancels.
fn rename_field(ui: &mut Ui, state: &mut DocState, id: LayerId, pos: Pos2, row: Rect) {
    let Some((_, text)) = &mut state.renaming else {
        return;
    };
    let field = Rect::from_min_max(
        Pos2::new(pos.x - 4.0, row.center().y - size::FIELD_HEIGHT / 2.0),
        Pos2::new(
            row.right() - 50.0,
            row.center().y + size::FIELD_HEIGHT / 2.0,
        ),
    );
    let edit = egui::TextEdit::singleline(text)
        .font(theme::body())
        .margin(Vec2::new(4.0, 2.0));
    let r = ui.put(field, edit);
    if !r.has_focus() && !r.lost_focus() {
        // First frame: focus the field with the whole name selected
        r.request_focus();
        if let Some(mut s) = egui::TextEdit::load_state(ui.ctx(), r.id) {
            let all = egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(text.chars().count()),
            );
            s.cursor.set_char_range(Some(all));
            s.store(ui.ctx(), r.id);
        }
        return;
    }
    if r.lost_focus() {
        let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let (_, name) = state.renaming.take().expect("renaming");
        if !cancelled && op_core::layer_ops::rename(&mut state.doc, id, &name) {
            state.record("Rename Layer");
        }
    }
}

/// While a row is dragged, shows where it would go; on release, moves it
/// (with everything in it, for a group) and records "Layer Order".
fn drop_layer(
    ui: &Ui,
    state: &mut DocState,
    rows: &[(LayerId, usize)],
    spans: &[(f32, f32)],
    from_row: usize,
    pointer: Pos2,
    released: bool,
) {
    // The boundary between rows the pointer is closest to (0 = above the
    // top row)
    let edges: Vec<f32> = spans
        .iter()
        .map(|s| s.0)
        .chain(spans.last().map(|s| s.1))
        .collect();
    let Some(gap) = (0..edges.len()).min_by(|&a, &b| {
        (edges[a] - pointer.y)
            .abs()
            .total_cmp(&(edges[b] - pointer.y).abs())
    }) else {
        return;
    };
    // In layer order: above the row under the boundary (below the last
    // row: under that row and everything in it)
    let doc = &state.doc;
    let index = |id| doc.layers.iter().position(|l| l.id == id);
    let flat = match rows.get(gap) {
        Some(&(id, _)) => index(id).map(|i| i + 1),
        None => rows
            .last()
            .and_then(|&(id, _)| doc.block(id))
            .map(|b| b.start),
    };
    let moving = rows[from_row].0;
    let Some(flat) = flat.filter(|&f| op_core::layer_ops::can_move_block(doc, moving, f)) else {
        return;
    };
    if released {
        if op_core::layer_ops::move_block(&mut state.doc, moving, flat) {
            state.record("Layer Order");
        }
        return;
    }
    let y = edges[gap];
    let clip = ui.clip_rect();
    ui.painter().line_segment(
        [Pos2::new(clip.left(), y), Pos2::new(clip.right(), y)],
        Stroke::new(2.0, color::ACCENT),
    );
}

/// The rows of the list, top to bottom, with their depth in groups:
/// layers inside collapsed groups are left out.
fn visible_rows(doc: &op_core::Document) -> Vec<(LayerId, usize)> {
    doc.layers
        .iter()
        .rev()
        .filter_map(|l| {
            let mut depth = 0;
            let mut parent = l.parent;
            while let Some(p) = parent.and_then(|p| doc.layer(p)) {
                if matches!(p.kind, op_core::LayerKind::Group { collapsed: true }) {
                    return None;
                }
                depth += 1;
                parent = p.parent;
            }
            Some((l.id, depth))
        })
        .collect()
}

/// The footer: eight buttons at Photoshop 2026's positions (centers
/// measured from the panel's right edge). Returns true when Alt-clicking
/// "Create a new layer" asks for the New Layer dialog, as in Photoshop.
fn bottom_bar(ui: &mut Ui, state: &mut DocState, rect: Rect) -> (bool, bool, bool) {
    let mut open_dialog = false;
    let mut open_group_dialog = false;
    let mut ask_delete = false;
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0, color::PANEL);
    painter.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), pt(1.0))),
        0,
        color::OPTIONS_SEPARATOR,
    );
    let is_background = state
        .doc
        .active_layer
        .and_then(|id| state.doc.layer(id))
        .is_some_and(|l| l.is_background);
    let selected = state.doc.selected_layers().len();
    let can_delete = selected > 0 && selected < state.doc.layers.len();
    let can_mask = op_core::layer_ops::can_add_mask(&state.doc);
    let buttons = [
        (222.5, Icon::FooterBrush, "", false),
        (196.5, Icon::LinkLayers, "Link layers", false),
        (169.5, Icon::LayerStyle, "Add a layer style", !is_background),
        (139.75, Icon::LayerMask, "Add a mask", can_mask),
        (
            113.5,
            Icon::NewAdjustment,
            "Create new fill or adjustment layer",
            true,
        ),
        (86.5, Icon::NewGroup, "Create a new group", true),
        (58.25, Icon::NewLayer, "Create a new layer", true),
        (30.25, Icon::DeleteLayer, "Delete layer", can_delete),
    ];
    for (x, i, tip, enabled) in buttons {
        let center = Pos2::new(rect.right() - pt(x), rect.top() + pt(12.5));
        let hit = Rect::from_center_size(center, Vec2::new(pt(24.0), pt(22.0)));
        let mut response = ui.interact(hit, ui.id().with(("footer", x.to_bits())), Sense::click());
        if enabled && response.hovered() {
            painter.rect_filled(hit, pt(3.0), color::HOVER);
        }
        icon(&painter, center, i, enabled, color::PANEL);
        if !tip.is_empty() {
            response = response.on_hover_text(tip);
        }
        if !enabled || !response.clicked() {
            continue;
        }
        match i {
            Icon::DeleteLayer => ask_delete = true,
            Icon::NewLayer if ui.input(|i| i.modifiers.alt) => open_dialog = true,
            Icon::NewGroup if ui.input(|i| i.modifiers.alt) => open_group_dialog = true,
            Icon::NewGroup => {
                op_core::layer_ops::new_group(&mut state.doc);
                state.record("New Group");
            }
            Icon::NewLayer => new_layer(state),
            Icon::LayerMask => {
                // From the selection when there is one, as in Photoshop
                let kind = if state.doc.selection().is_some() {
                    op_core::layer_ops::NewMask::RevealSelection
                } else {
                    op_core::layer_ops::NewMask::RevealAll
                };
                if op_core::layer_ops::add_mask(&mut state.doc, kind) {
                    state.record("Add Layer Mask");
                }
            }
            _ => {}
        }
    }
    (open_dialog, open_group_dialog, ask_delete)
}

/// Layer > Hide Layers / Show Layers. Like Photoshop's default, visibility
/// changes are not recorded in the history.
pub fn toggle_active_visibility(state: &mut DocState) {
    op_core::layer_ops::toggle_selected_visibility(&mut state.doc);
}

/// The color a layer's color label shows in the Layers panel (behind the
/// eye); `None` for no label.
pub fn layer_color(c: op_core::LayerColor) -> Option<egui::Color32> {
    use op_core::LayerColor::*;
    let rgb = |r, g, b| Some(egui::Color32::from_rgb(r, g, b));
    // Measured on Photoshop 2026
    match c {
        None => Option::None,
        Red => rgb(0xa3, 0x49, 0x43),
        Orange => rgb(0x9c, 0x65, 0x24),
        Yellow => rgb(0xa5, 0x8a, 0x2e),
        Green => rgb(0x66, 0x81, 0x45),
        Blue => rgb(0x58, 0x6e, 0x96),
        Violet => rgb(0x6d, 0x56, 0x9b),
        Gray => rgb(0x6a, 0x6a, 0x6a),
    }
}

/// Layer > New > Layer... confirmed: a layer with the dialog's name, color
/// label, blend mode and opacity above the active one, filled with the
/// mode's neutral color when asked. Recorded as "New Layer".
pub fn new_layer_from(state: &mut DocState, new: crate::dialogs::NewLayer) {
    let doc = &mut state.doc;
    let image = match new
        .fill_neutral
        .then(|| op_core::neutral_color(new.mode))
        .flatten()
    {
        Some([r, g, b]) => TiledImage::filled(doc.width, doc.height, [r, g, b, 255]),
        None => TiledImage::new(doc.width, doc.height),
    };
    let mut layer = Layer::raster(doc.new_layer_id(), new.name, image);
    layer.color = new.color;
    layer.blend_mode = new.mode;
    layer.opacity = new.opacity;
    doc.insert_above_active(layer);
    state.record("New Layer");
}

/// Layer > New > Layer from Background... confirmed: the background
/// becomes a regular layer with the dialog's name, color label, blend mode
/// and opacity. Recorded as "Layer From Background".
pub fn layer_from_background_with(state: &mut DocState, new: crate::dialogs::NewLayer) {
    if !op_core::layer_ops::layer_from_background(&mut state.doc) {
        return;
    }
    if let Some(layer) = state.doc.layers.first_mut() {
        layer.name = new.name;
        layer.color = new.color;
        layer.blend_mode = new.mode;
        layer.opacity = new.opacity;
    }
    state.record("Layer From Background");
}

/// Layer > New > Group... (or Group from Layers...) confirmed: a group with
/// the dialog's name, color label, blend mode and opacity, empty above the
/// active layer or holding the selected layers. Recorded as "New Group"
/// or "Group Layers".
pub fn new_group_from(state: &mut DocState, new: crate::dialogs::NewLayer, from_layers: bool) {
    let doc = &mut state.doc;
    let id = if from_layers {
        match op_core::layer_ops::group_selected(doc) {
            Some(id) => id,
            None => return,
        }
    } else {
        op_core::layer_ops::new_group(doc)
    };
    if let Some(group) = doc.layer_mut(id) {
        group.name = new.name;
        group.color = new.color;
        group.blend_mode = new.mode;
        group.opacity = new.opacity;
    }
    state.record(if from_layers {
        "Group Layers"
    } else {
        "New Group"
    });
}

pub fn new_layer(state: &mut DocState) {
    let doc = &mut state.doc;
    let layer = Layer::raster(
        doc.new_layer_id(),
        doc.next_layer_name(),
        TiledImage::new(doc.width, doc.height),
    );
    doc.insert_above_active(layer);
    state.record("New Layer");
}

/// Deletes the selected layers (all of them when several are selected).
pub fn delete_active_layer(state: &mut DocState) {
    if op_core::layer_ops::delete_selected(&mut state.doc) {
        state.record("Delete Layer");
    }
}
