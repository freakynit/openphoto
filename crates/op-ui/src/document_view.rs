//! Document window: the canvas (zoom/pan/tool input) and the status bar.

use egui::{Align2, Color32, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Ui, Vec2};
use op_core::DocId;
use op_tools::Tool;

use crate::state::{AppState, DocState};
use crate::theme::{self, color, pt, size};

/// Photoshop's preset zoom levels, in percent.
const ZOOM_STEPS: &[f32] = &[
    1.0, 2.0, 3.0, 4.0, 5.0, 6.25, 8.33, 12.5, 16.67, 25.0, 33.33, 50.0, 66.67, 100.0, 150.0,
    200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1200.0, 1600.0, 2400.0, 3200.0, 6400.0,
    12800.0,
];
const MIN_ZOOM: f32 = 0.01;
const MAX_ZOOM: f32 = 128.0;

pub fn zoom_label(zoom: f32) -> String {
    let pct = zoom * 100.0;
    if (pct - pct.round()).abs() < 0.01 {
        format!("{pct:.0}%")
    } else {
        format!("{pct:.2}%")
    }
}

pub fn next_zoom_step(zoom: f32, zoom_in: bool) -> f32 {
    let pct = zoom * 100.0;
    let step = if zoom_in {
        ZOOM_STEPS.iter().copied().find(|s| *s > pct * 1.001)
    } else {
        ZOOM_STEPS.iter().rev().copied().find(|s| *s < pct * 0.999)
    };
    step.map_or(zoom, |s| s / 100.0)
}

fn doc_size_pt(state: &DocState, zoom: f32, ppp: f32) -> Vec2 {
    Vec2::new(state.doc.width as f32, state.doc.height as f32) * zoom / ppp
}

/// Screen position of the document's top-left corner, in points.
pub fn origin(state: &DocState, ppp: f32) -> Pos2 {
    let v = &state.view;
    v.viewport.center() + v.offset - doc_size_pt(state, v.zoom, ppp) / 2.0
}

/// Zooms around `anchor` (a screen point), keeping the document point under it fixed.
pub fn zoom_at(state: &mut DocState, new_zoom: f32, anchor: Pos2, ppp: f32) {
    let new_zoom = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let doc_pt = (anchor - origin(state, ppp)) * ppp / state.view.zoom;
    let new_origin = anchor - doc_pt * new_zoom / ppp;
    let new_center = new_origin + doc_size_pt(state, new_zoom, ppp) / 2.0;
    state.view.offset = new_center - state.view.viewport.center();
    state.view.zoom = new_zoom;
}

pub fn zoom_step(state: &mut DocState, zoom_in: bool, ppp: f32) {
    let z = next_zoom_step(state.view.zoom, zoom_in);
    zoom_at(state, z, state.view.viewport.center(), ppp);
}

/// View > Fit on Screen.
pub fn fit_on_screen(state: &mut DocState, ppp: f32) {
    let avail = state.view.viewport.size() * ppp;
    let z = (avail.x / state.doc.width as f32).min(avail.y / state.doc.height as f32);
    state.view.zoom = z.clamp(MIN_ZOOM, MAX_ZOOM);
    state.view.offset = Vec2::ZERO;
}

/// View > 100%.
pub fn actual_pixels(state: &mut DocState, _ppp: f32) {
    state.view.zoom = 1.0;
    state.view.offset = Vec2::ZERO;
}

pub fn show(ui: &mut Ui, app: &mut AppState, id: DocId) {
    let tool = app.tool;
    let Some(state) = app.docs.get_mut(&id) else {
        return;
    };
    let ppp = ui.ctx().pixels_per_point();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0, color::PASTEBOARD);

    // Photoshop's document window: canvas, a vertical scrollbar column on
    // the right, and the status bar (with the horizontal scrollbar) below
    let status_rect = Rect::from_min_max(
        Pos2::new(full.left(), full.bottom() - size::STATUS_BAR),
        full.max,
    );
    let vscroll_rect = Rect::from_min_max(
        Pos2::new(full.right() - VSCROLL_W, full.top()),
        Pos2::new(full.right(), status_rect.top()),
    );
    let canvas_rect =
        Rect::from_min_max(full.min, Pos2::new(vscroll_rect.left(), status_rect.top()));

    // Layout may not be settled in the first frames; pick the initial zoom only
    // once the viewport size is the same for two consecutive frames
    let stable = state.view.viewport == canvas_rect;
    state.view.viewport = canvas_rect;
    if !state.view.initialized && !stable {
        ui.ctx().request_repaint();
    }
    if !state.view.initialized && stable && canvas_rect.width() > 1.0 {
        state.view.initialized = true;
        let fits = state.doc.width as f32 <= canvas_rect.width() * ppp
            && state.doc.height as f32 <= canvas_rect.height() * ppp;
        if fits {
            actual_pixels(state, ppp);
        } else {
            fit_on_screen(state, ppp);
        }
    }

    let response = ui.allocate_rect(canvas_rect, Sense::click_and_drag());
    let (space, alt, zoom_delta, scroll) = ui.input(|i| {
        (
            i.key_down(Key::Space),
            i.modifiers.alt,
            i.zoom_delta(),
            i.smooth_scroll_delta,
        )
    });
    let hover = response.hover_pos();

    // Pinch / Cmd+scroll zooms; plain scrolling pans
    if let Some(p) = hover {
        if zoom_delta != 1.0 {
            let z = state.view.zoom * zoom_delta;
            zoom_at(state, z, p, ppp);
        }
        if scroll != Vec2::ZERO {
            state.view.offset += scroll;
        }
    }

    let panning = space || tool == Tool::Hand;
    let middle_drag = response.dragged_by(PointerButton::Middle);
    if (panning && response.dragged_by(PointerButton::Primary)) || middle_drag {
        state.view.offset += response.drag_delta();
    } else if !space {
        match tool {
            Tool::Zoom if response.clicked() => {
                if let Some(p) = response.interact_pointer_pos() {
                    let z = next_zoom_step(state.view.zoom, !alt);
                    zoom_at(state, z, p, ppp);
                }
            }
            Tool::Eyedropper if response.is_pointer_button_down_on() => {
                if let Some(p) = response.interact_pointer_pos() {
                    let d = (p - origin(state, ppp)) * ppp / state.view.zoom;
                    if d.x >= 0.0
                        && d.y >= 0.0
                        && let Some(c) = state.sample(d.x as u32, d.y as u32)
                    {
                        app.foreground = c;
                    }
                }
            }
            _ => {}
        }
    }

    let state = app.docs.get_mut(&id).unwrap();
    clamp_offset(state, ppp);
    if response.hovered() {
        let cursor = if panning || middle_drag {
            if response.dragged() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            }
        } else {
            match tool {
                Tool::Zoom if alt => CursorIcon::ZoomOut,
                Tool::Zoom => CursorIcon::ZoomIn,
                Tool::Eyedropper | Tool::Brush | Tool::Eraser | Tool::RectangularMarquee => {
                    CursorIcon::Crosshair
                }
                Tool::Move => CursorIcon::Move,
                Tool::HorizontalType => CursorIcon::Text,
                _ => CursorIcon::Default,
            }
        };
        ui.ctx().set_cursor_icon(cursor);
    }

    // Snap to physical pixels so 100% stays crisp
    let origin = origin(state, ppp);
    let origin = Pos2::new(
        (origin.x * ppp).round() / ppp,
        (origin.y * ppp).round() / ppp,
    );
    let image = state.canvas_image();
    let view = op_render::CanvasView {
        origin: [origin.x, origin.y],
        zoom: state.view.zoom,
        pixel_grid: true,
    };
    ui.painter()
        .with_clip_rect(canvas_rect)
        .add(op_render::paint_callback(canvas_rect, image, view));

    status_bar(ui, state, status_rect, ppp);
    vertical_scrollbar(ui, state, vscroll_rect, ppp);
}

/// Width of the vertical scrollbar column, and the status bar's layout.
const VSCROLL_W: f32 = pt(17.0);
const ZOOM_BOX_W: f32 = pt(60.0);
const INFO_X: f32 = pt(89.0);
const CARET_X: f32 = pt(237.0);
const HSCROLL_X: f32 = pt(243.0);
const TRACK: Color32 = Color32::from_gray(0x4a);
const THUMB: Color32 = Color32::from_gray(0x69);
const THUMB_THICKNESS: f32 = pt(10.0);
const VTHUMB_THICKNESS: f32 = pt(12.0);

/// The scrollable extent along one axis: the document plus one viewport, so
/// the document's edge can be scrolled to the middle of the window. Returns
/// (visible start, visible length) as fractions of the extent.
fn scroll_fraction(doc_pt: f32, viewport: f32, offset: f32) -> (f32, f32) {
    let extent = doc_pt + viewport;
    ((doc_pt / 2.0 - offset) / extent, viewport / extent)
}

/// Keeps the document's edge from being scrolled past the middle of the
/// viewport (the same extent the scrollbars show).
fn clamp_offset(state: &mut DocState, ppp: f32) {
    let size = doc_size_pt(state, state.view.zoom, ppp);
    let o = &mut state.view.offset;
    o.x = o.x.clamp(-size.x / 2.0, size.x / 2.0);
    o.y = o.y.clamp(-size.y / 2.0, size.y / 2.0);
}

/// Draws a scrollbar thumb in `track` and returns the drag, as a change of
/// the visible start in fractions of the extent.
fn scrollbar(
    ui: &mut Ui,
    track: Rect,
    vertical: bool,
    (start, len): (f32, f32),
    thickness: f32,
    id: &str,
) -> f32 {
    let along = |r: Rect| if vertical { r.height() } else { r.width() };
    let length = along(track);
    let t0 = start.clamp(0.0, 1.0 - len.min(1.0)) * length;
    let t1 = t0 + len.min(1.0) * length;
    let thumb = if vertical {
        Rect::from_min_max(
            Pos2::new(track.center().x - thickness / 2.0, track.top() + t0),
            Pos2::new(track.center().x + thickness / 2.0, track.top() + t1),
        )
    } else {
        Rect::from_min_max(
            Pos2::new(track.left() + t0, track.center().y - thickness / 2.0),
            Pos2::new(track.left() + t1, track.center().y + thickness / 2.0),
        )
    };
    let response = ui.interact(thumb, ui.id().with(id), Sense::drag());
    ui.painter().rect_filled(
        thumb,
        egui::CornerRadius::same((thickness / 2.0) as u8),
        THUMB,
    );
    if response.dragged() && length > 0.0 {
        let d = response.drag_delta();
        (if vertical { d.y } else { d.x }) / length
    } else {
        0.0
    }
}

fn vertical_scrollbar(ui: &mut Ui, state: &mut DocState, column: Rect, ppp: f32) {
    let painter = ui.painter();
    painter.rect_filled(column, 0, TRACK);
    // 1 pt edges: the canvas border, then a lighter and a darker line
    let edge = |x: f32, c: u8| {
        let r = Rect::from_min_max(
            Pos2::new(x, column.top()),
            Pos2::new(x + pt(1.0), column.bottom()),
        );
        painter.rect_filled(r, 0, Color32::from_gray(c));
    };
    edge(column.left(), 0x2e);
    edge(column.left() + pt(1.0), 0x46);
    edge(column.right() - pt(1.0), 0x45);

    let doc = doc_size_pt(state, state.view.zoom, ppp);
    let fraction = scroll_fraction(doc.y, state.view.viewport.height(), state.view.offset.y);
    let extent = doc.y + state.view.viewport.height();
    let track = column.shrink2(Vec2::new(0.0, pt(1.0)));
    let d = scrollbar(ui, track, true, fraction, VTHUMB_THICKNESS, "vscroll");
    state.view.offset.y -= d * extent;
    clamp_offset(state, ppp);
}

/// Photoshop's status bar: zoom box, document info, the info menu caret,
/// then the horizontal scrollbar filling the rest.
fn status_bar(ui: &mut Ui, state: &mut DocState, rect: Rect, ppp: f32) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0, color::PANEL);
    let line = Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + pt(1.0)));
    painter.rect_filled(line, 0, Color32::from_gray(0x44));
    let body = Rect::from_min_max(Pos2::new(rect.left(), line.bottom()), rect.max);

    let zoom_rect = Rect::from_min_size(body.min, Vec2::new(ZOOM_BOX_W, body.height()));
    painter.rect_filled(zoom_rect, 0, Color32::from_gray(0x41));
    painter.text(
        zoom_rect.center(),
        Align2::CENTER_CENTER,
        zoom_label(state.view.zoom),
        theme::body(),
        color::TEXT,
    );

    let doc = &state.doc;
    let info = format!(
        "{} px x {} px ({} ppi)",
        doc.width,
        doc.height,
        doc.resolution.round()
    );
    painter.text(
        Pos2::new(body.left() + INFO_X, body.center().y),
        Align2::LEFT_CENTER,
        info,
        theme::body(),
        color::TEXT_DIM,
    );
    painter.text(
        Pos2::new(body.left() + CARET_X, body.center().y),
        Align2::CENTER_CENTER,
        crate::icons::CARET_RIGHT,
        theme::icon(pt(9.0)),
        color::TEXT_DIM,
    );

    // The horizontal scrollbar stops where the vertical one's column starts;
    // the corner below that column stays panel-colored
    let track = Rect::from_min_max(
        Pos2::new(body.left() + HSCROLL_X, body.top()),
        Pos2::new(body.right() - VSCROLL_W, body.bottom()),
    );
    if track.width() > 0.0 {
        painter.rect_filled(track, 0, TRACK);
        let doc = doc_size_pt(state, state.view.zoom, ppp);
        let fraction = scroll_fraction(doc.x, state.view.viewport.width(), state.view.offset.x);
        let extent = doc.x + state.view.viewport.width();
        let d = scrollbar(ui, track, false, fraction, THUMB_THICKNESS, "hscroll");
        state.view.offset.x -= d * extent;
        clamp_offset(state, ppp);
    }
}
