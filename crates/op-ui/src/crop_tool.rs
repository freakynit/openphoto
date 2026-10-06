//! The Crop tool (C): a crop box over the canvas.
//!
//! As in Photoshop, the box covers the whole canvas when the tool is
//! picked. Its handles resize it (Shift keeps the proportions, Alt resizes
//! around the center), dragging inside moves it, and dragging outside it
//! draws a new box. The area outside is shaded, with a rule-of-thirds grid
//! while dragging. Enter or a double-click inside crops; Escape resets the
//! box.

use egui::{Color32, CursorIcon, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use crate::document_view::{to_doc, to_screen};
use crate::state::{CropBox, CropDrag, DocState};
use crate::theme::pt;

const GRAB: f32 = pt(8.0);
const SHIELD: Color32 = Color32::from_black_alpha(140);

/// The box covering the whole canvas.
pub fn full(state: &DocState) -> CropBox {
    CropBox {
        rect: Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(state.doc.width as f32, state.doc.height as f32),
        ),
        drag: None,
    }
}

/// The handle (−1, 0 or 1 per axis) under screen point `p`, if any.
fn handle_at(state: &DocState, rect: Rect, p: Pos2, ppp: f32) -> Option<(i8, i8)> {
    let screen = Rect::from_two_pos(
        to_screen(state, rect.min, ppp),
        to_screen(state, rect.max, ppp),
    );
    for hy in -1i8..=1 {
        for hx in -1i8..=1 {
            if hx == 0 && hy == 0 {
                continue;
            }
            let x = match hx {
                -1 => screen.left(),
                0 => screen.center().x,
                _ => screen.right(),
            };
            let y = match hy {
                -1 => screen.top(),
                0 => screen.center().y,
                _ => screen.bottom(),
            };
            if Pos2::new(x, y).distance(p) <= GRAB {
                return Some((hx, hy));
            }
        }
    }
    None
}

/// The box for a drag to `p` (document pixels).
fn dragged(drag: CropDrag, p: Pos2, mods: Modifiers) -> Rect {
    let start = drag.rect;
    match drag.handle {
        None => start.translate(p - drag.pointer),
        Some((hx, hy)) => {
            let (mut x0, mut y0, mut x1, mut y1) =
                (start.left(), start.top(), start.right(), start.bottom());
            let d = p - drag.pointer;
            if hx < 0 {
                x0 += d.x;
            } else if hx > 0 {
                x1 += d.x;
            }
            if hy < 0 {
                y0 += d.y;
            } else if hy > 0 {
                y1 += d.y;
            }
            if mods.alt {
                // Around the center: the opposite side moves the other way
                if hx < 0 {
                    x1 -= d.x;
                } else if hx > 0 {
                    x0 -= d.x;
                }
                if hy < 0 {
                    y1 -= d.y;
                } else if hy > 0 {
                    y0 -= d.y;
                }
            }
            let mut r = Rect::from_two_pos(Pos2::new(x0, y0), Pos2::new(x1, y1));
            if mods.shift && hx != 0 && hy != 0 && start.height() > 0.0 {
                // Keep the starting aspect ratio, sized by the wider change
                let ratio = start.width() / start.height();
                let (w, h) = if r.width() / ratio > r.height() {
                    (r.width(), r.width() / ratio)
                } else {
                    (r.height() * ratio, r.height())
                };
                let anchor = Pos2::new(
                    if hx < 0 { start.right() } else { start.left() },
                    if hy < 0 { start.bottom() } else { start.top() },
                );
                let corner = anchor + Vec2::new(hx as f32 * w, hy as f32 * h);
                r = Rect::from_two_pos(anchor, corner);
            }
            r
        }
    }
}

/// Handles the Crop tool's input on the canvas. Returns true when the
/// image was cropped.
pub fn input(ui: &Ui, response: &egui::Response, state: &mut DocState, ppp: f32) -> bool {
    if state.crop.is_none() {
        state.crop = Some(full(state));
    }
    // Keys typed into a text field are not for the canvas
    let typing = ui.ctx().egui_wants_keyboard_input();
    let (enter, escape) = ui.input_mut(|i| {
        (
            !typing && i.consume_key(Modifiers::NONE, Key::Enter),
            !typing && i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    if escape {
        state.crop = Some(full(state));
        return false;
    }
    let mods = ui.input(|i| i.modifiers);
    let rect = state.crop.as_ref().map(|c| c.rect).unwrap_or(Rect::NOTHING);
    let double_inside = response.double_clicked()
        && response
            .interact_pointer_pos()
            .is_some_and(|p| rect.contains(to_doc(state, p, ppp)));
    if enter || double_inside {
        return commit(state);
    }

    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        let d = to_doc(state, p, ppp);
        let handle = handle_at(state, rect, p, ppp);
        let drag = match handle {
            Some(h) => CropDrag {
                handle: Some(h),
                pointer: d,
                rect,
            },
            None if rect.contains(d) => CropDrag {
                handle: None,
                pointer: d,
                rect,
            },
            // Outside the box: draw a new one from here
            None => CropDrag {
                handle: Some((1, 1)),
                pointer: d,
                rect: Rect::from_min_max(d, d),
            },
        };
        if let Some(c) = &mut state.crop {
            c.drag = Some(drag);
        }
    }
    let pointer = ui
        .input(|i| i.pointer.interact_pos())
        .map(|p| to_doc(state, p, ppp));
    if let Some(c) = &mut state.crop
        && let Some(drag) = c.drag
    {
        if let Some(p) = pointer {
            c.rect = dragged(drag, p, mods);
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            c.drag = None;
            if c.rect.width() < 1.0 || c.rect.height() < 1.0 {
                c.rect = drag.rect;
            }
        }
        ui.ctx().request_repaint();
    }
    false
}

/// Crops to the box (whole pixels, within the canvas). Returns true when
/// the image changed; the box then covers the new canvas.
pub fn commit(state: &mut DocState) -> bool {
    let Some(c) = state.crop.take() else {
        return false;
    };
    let (w, h) = (state.doc.width as f32, state.doc.height as f32);
    let x0 = c.rect.left().round().clamp(0.0, w) as u32;
    let y0 = c.rect.top().round().clamp(0.0, h) as u32;
    let x1 = c.rect.right().round().clamp(0.0, w) as u32;
    let y1 = c.rect.bottom().round().clamp(0.0, h) as u32;
    let changed = x1 > x0 && y1 > y0 && (x0, y0, x1, y1) != (0, 0, w as u32, h as u32);
    if changed {
        op_core::image_ops::crop(&mut state.doc, x0, y0, x1, y1);
        state.record("Crop");
    }
    state.crop = Some(full(state));
    changed
}

pub fn cursor(state: &DocState, p: Pos2, ppp: f32) -> CursorIcon {
    let Some(c) = &state.crop else {
        return CursorIcon::Crosshair;
    };
    let handle = c
        .drag
        .map_or_else(|| handle_at(state, c.rect, p, ppp), |d| d.handle);
    match handle {
        Some((0, _)) => CursorIcon::ResizeVertical,
        Some((_, 0)) => CursorIcon::ResizeHorizontal,
        Some((hx, hy)) if hx == hy => CursorIcon::ResizeNwSe,
        Some(_) => CursorIcon::ResizeNeSw,
        None if c.rect.contains(to_doc(state, p, ppp)) => CursorIcon::Move,
        None => CursorIcon::Crosshair,
    }
}

/// The shaded area outside the box, the box with its handles, and the
/// rule-of-thirds grid while dragging.
pub fn draw(ui: &Ui, state: &DocState, canvas: Rect, ppp: f32) {
    let Some(c) = &state.crop else {
        return;
    };
    let painter = ui.painter_at(canvas);
    let image = Rect::from_two_pos(
        to_screen(state, Pos2::ZERO, ppp),
        to_screen(
            state,
            Pos2::new(state.doc.width as f32, state.doc.height as f32),
            ppp,
        ),
    );
    let r = Rect::from_two_pos(
        to_screen(state, c.rect.min, ppp),
        to_screen(state, c.rect.max, ppp),
    );
    // Shield: the image outside the box, in four bands
    for band in [
        Rect::from_min_max(image.min, Pos2::new(image.right(), r.top())),
        Rect::from_min_max(Pos2::new(image.left(), r.bottom()), image.max),
        Rect::from_min_max(
            Pos2::new(image.left(), r.top()),
            Pos2::new(r.left(), r.bottom()),
        ),
        Rect::from_min_max(
            Pos2::new(r.right(), r.top()),
            Pos2::new(image.right(), r.bottom()),
        ),
    ] {
        let band = band.intersect(image);
        if band.is_positive() {
            painter.rect_filled(band, 0, SHIELD);
        }
    }
    let line = Stroke::new(1.0, Color32::WHITE);
    if c.drag.is_some() {
        let thin = Stroke::new(1.0, Color32::from_white_alpha(140));
        for k in 1..3 {
            let x = r.left() + r.width() * k as f32 / 3.0;
            let y = r.top() + r.height() * k as f32 / 3.0;
            painter.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], thin);
            painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], thin);
        }
    }
    painter.rect_stroke(r, 0, line, StrokeKind::Middle);
    // Handles: L-shaped corners and bars on the sides, like Photoshop's
    let (len, thick) = (pt(14.0), pt(3.0));
    for (hx, hy) in [(-1, -1), (1, -1), (1, 1), (-1, 1)] {
        let corner = Pos2::new(
            if hx < 0 { r.left() } else { r.right() },
            if hy < 0 { r.top() } else { r.bottom() },
        );
        let (dx, dy) = (-(hx as f32), -(hy as f32));
        let h = Rect::from_two_pos(corner, corner + Vec2::new(dx * len, dy * thick));
        let v = Rect::from_two_pos(corner, corner + Vec2::new(dx * thick, dy * len));
        painter.rect_filled(h, 0, Color32::WHITE);
        painter.rect_filled(v, 0, Color32::WHITE);
    }
    for (center, horizontal) in [
        (Pos2::new(r.center().x, r.top()), true),
        (Pos2::new(r.center().x, r.bottom()), true),
        (Pos2::new(r.left(), r.center().y), false),
        (Pos2::new(r.right(), r.center().y), false),
    ] {
        let size = if horizontal {
            Vec2::new(len, thick)
        } else {
            Vec2::new(thick, len)
        };
        painter.rect_filled(Rect::from_center_size(center, size), 0, Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(handle: Option<(i8, i8)>) -> CropDrag {
        CropDrag {
            handle,
            pointer: Pos2::new(100.0, 50.0),
            rect: Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 50.0)),
        }
    }

    #[test]
    fn handles_resize_and_inside_moves() {
        let r = dragged(drag(Some((1, 1))), Pos2::new(80.0, 30.0), Modifiers::NONE);
        assert_eq!(r, Rect::from_min_max(Pos2::ZERO, Pos2::new(80.0, 30.0)));
        // Shift keeps 2:1
        let r = dragged(drag(Some((1, 1))), Pos2::new(80.0, 20.0), Modifiers::SHIFT);
        assert_eq!(r, Rect::from_min_max(Pos2::ZERO, Pos2::new(80.0, 40.0)));
        // Alt resizes around the center
        let r = dragged(drag(Some((1, 0))), Pos2::new(90.0, 50.0), Modifiers::ALT);
        assert_eq!(
            r,
            Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(90.0, 50.0))
        );
        let r = dragged(drag(None), Pos2::new(110.0, 55.0), Modifiers::NONE);
        assert_eq!(r.min, Pos2::new(10.0, 5.0));
    }
}
