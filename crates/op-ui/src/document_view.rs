//! Document window: the canvas (zoom/pan/tool input) and the status bar.

use egui::{Align2, CursorIcon, Key, PointerButton, Pos2, Rect, Sense, Ui, Vec2};
use op_core::DocId;
use op_tools::Tool;

use crate::state::{AppState, DocState};
use crate::theme::{self, color, size};

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

    let (canvas_rect, status_rect) = {
        let mut c = full;
        c.max.y -= size::STATUS_BAR;
        (
            c,
            Rect::from_min_max(Pos2::new(full.min.x, c.max.y), full.max),
        )
    };

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

    status_bar(ui, state, status_rect);
}

fn status_bar(ui: &mut Ui, state: &mut DocState, rect: Rect) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0, color::PANEL);
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        egui::Stroke::new(1.0, color::SEPARATOR),
    );

    let zoom_rect = Rect::from_min_size(
        rect.min + Vec2::new(10.0, 3.0),
        Vec2::new(64.0, rect.height() - 6.0),
    );
    painter.rect_filled(zoom_rect, 2, color::FIELD);
    painter.text(
        zoom_rect.left_center() + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
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
    let info_rect = painter.text(
        Pos2::new(zoom_rect.right() + 20.0, rect.center().y),
        Align2::LEFT_CENTER,
        info,
        theme::body(),
        color::TEXT_DIM,
    );
    painter.text(
        Pos2::new(info_rect.right() + 40.0, rect.center().y),
        Align2::LEFT_CENTER,
        crate::icons::CARET_RIGHT,
        theme::icon(14.0),
        color::TEXT_DIM,
    );
}
