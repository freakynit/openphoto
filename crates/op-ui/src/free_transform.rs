//! Edit > Free Transform (Cmd+T) on the canvas: the transform box with its
//! handles, dragging to move, scale and rotate, and committing or
//! cancelling.
//!
//! Like Photoshop: dragging inside the box moves; a handle scales,
//! proportionally by default (Shift scales freely; side handles always
//! scale one axis), from the opposite handle (Alt: from the center);
//! dragging outside the box rotates (Shift: 15° steps). Enter or a
//! double-click inside commits, Escape cancels.

use egui::{Color32, CursorIcon, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use op_core::transform::{self, Affine, TransformError};

use crate::document_view::{to_doc, to_screen};
use crate::state::{DocState, FreeTransform, TransformDrag, TransformHandle};
use crate::theme::pt;

/// Handles closer than this to the pointer (in points) are grabbed.
const GRAB: f32 = pt(8.0);
const HANDLE: f32 = pt(7.0);

/// How a Free Transform session ended.
pub enum Outcome {
    Committed(Affine),
    Cancelled,
}

/// Starts Free Transform on the active layer (or the selected pixels).
pub fn start(state: &mut DocState) -> Result<(), TransformError> {
    let bounds = transform::bounds(&state.doc)?;
    state.free_transform = Some(FreeTransform::new(state.doc.snapshot(), bounds));
    Ok(())
}

/// The box's corners in document pixels, clockwise from the top left.
fn corners(t: &FreeTransform) -> [Pos2; 4] {
    let (x0, y0, x1, y1) = t.bounds;
    let m = t.affine();
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|p| {
        let (x, y) = m.apply(p);
        Pos2::new(x, y)
    })
}

/// The eight scale handles in document pixels, with their (hx, hy).
fn handles(t: &FreeTransform) -> [(Pos2, i8, i8); 8] {
    let (x0, y0, x1, y1) = t.bounds;
    let (cx, cy) = t.center();
    let m = t.affine();
    let at = |x: f32, y: f32| {
        let (x, y) = m.apply((x, y));
        Pos2::new(x, y)
    };
    [
        (at(x0, y0), -1, -1),
        (at(cx, y0), 0, -1),
        (at(x1, y0), 1, -1),
        (at(x1, cy), 1, 0),
        (at(x1, y1), 1, 1),
        (at(cx, y1), 0, 1),
        (at(x0, y1), -1, 1),
        (at(x0, cy), -1, 0),
    ]
}

/// Whether `p` lies inside the convex quad `q`.
fn inside(q: &[Pos2; 4], p: Pos2) -> bool {
    let mut sign = 0.0f32;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let cross = (b - a).x * (p - a).y - (b - a).y * (p - a).x;
        if cross != 0.0 {
            if sign != 0.0 && cross.signum() != sign {
                return false;
            }
            sign = cross.signum();
        }
    }
    true
}

/// The handle under the screen point `p`.
fn hit(state: &DocState, t: &FreeTransform, p: Pos2, ppp: f32) -> TransformHandle {
    for (h, hx, hy) in handles(t) {
        if to_screen(state, h, ppp).distance(p) <= GRAB {
            return TransformHandle::Scale(hx, hy);
        }
    }
    let quad = corners(t).map(|c| to_screen(state, c, ppp));
    if inside(&quad, p) {
        TransformHandle::Move
    } else {
        TransformHandle::Rotate
    }
}

/// Updates the box for a drag to `p` (document pixels).
fn drag_to(t: &mut FreeTransform, drag: TransformDrag, p: Pos2, mods: Modifiers) {
    let center0 = t.center();
    let (x0, y0, x1, y1) = t.bounds;
    let (ow, oh) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let (sin, cos) = drag.angle.sin_cos();
    let to_local = |v: Vec2| Vec2::new(v.x * cos + v.y * sin, -v.x * sin + v.y * cos);
    let to_world = |v: Vec2| Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
    let c = Pos2::new(center0.0 + drag.offset.0, center0.1 + drag.offset.1);
    match drag.handle {
        TransformHandle::Move => {
            let d = p - drag.start;
            t.offset = (drag.offset.0 + d.x, drag.offset.1 + d.y);
        }
        TransformHandle::Rotate => {
            let a0 = (drag.start - c).angle();
            let a1 = (p - c).angle();
            let mut angle = drag.angle + (a1 - a0);
            if mods.shift {
                let step = 15f32.to_radians();
                angle = (angle / step).round() * step;
            }
            t.angle = angle;
        }
        TransformHandle::Scale(hx, hy) => {
            let (hx, hy) = (hx as f32, hy as f32);
            let (cw, ch) = (ow * drag.scale.0, oh * drag.scale.1);
            // The fixed point: the opposite handle, or the center with Alt
            let from_center = mods.alt;
            let fixed = if from_center {
                c
            } else {
                c + to_world(Vec2::new(-hx * cw, -hy * ch))
            };
            let d = to_local(p - fixed);
            // Distance from the fixed point to the dragged handle, in
            // multiples of the half size
            let span = if from_center { 1.0 } else { 2.0 };
            let mut sx = if hx != 0.0 {
                d.x / (span * hx) / ow
            } else {
                drag.scale.0
            };
            let mut sy = if hy != 0.0 {
                d.y / (span * hy) / oh
            } else {
                drag.scale.1
            };
            if hx != 0.0 && hy != 0.0 && !mods.shift {
                // Proportional: project onto the box's diagonal
                let diag = Vec2::new(hx * cw * span, hy * ch * span);
                let f = d.dot(diag) / diag.length_sq().max(1e-6);
                sx = drag.scale.0 * f;
                sy = drag.scale.1 * f;
            }
            // Keep the box from collapsing to nothing
            let min = 1.0 / ow.max(oh).max(1.0);
            let clamp = |s: f32| if s.abs() < min { min.copysign(s) } else { s };
            t.scale = (clamp(sx), clamp(sy));
            let (nw, nh) = (ow * t.scale.0, oh * t.scale.1);
            let new_center = if from_center {
                c
            } else {
                fixed + to_world(Vec2::new(hx * nw, hy * nh))
            };
            // A side handle leaves the other axis' center where it was
            let new_center = if hx == 0.0 || hy == 0.0 {
                let shift = to_local(new_center - c);
                c + to_world(Vec2::new(
                    if hx == 0.0 { 0.0 } else { shift.x },
                    if hy == 0.0 { 0.0 } else { shift.y },
                ))
            } else {
                new_center
            };
            t.offset = (new_center.x - center0.0, new_center.y - center0.1);
        }
    }
}

/// Handles input on the canvas while transforming. Returns how the session
/// ended, if it did.
pub fn input(
    ui: &Ui,
    response: &egui::Response,
    state: &mut DocState,
    background: [u8; 3],
    ppp: f32,
) -> Option<Outcome> {
    // Keys typed into a text field are not for the canvas
    let typing = ui.ctx().egui_wants_keyboard_input();
    let (enter, escape) = ui.input_mut(|i| {
        (
            !typing && i.consume_key(Modifiers::NONE, Key::Enter),
            !typing && i.consume_key(Modifiers::NONE, Key::Escape),
        )
    });
    if escape {
        return Some(cancel(state));
    }
    let mods = ui.input(|i| i.modifiers);
    let t = state.free_transform.as_ref()?;
    let double_inside = response.double_clicked()
        && response
            .interact_pointer_pos()
            .is_some_and(|p| hit(state, t, p, ppp) == TransformHandle::Move);
    if enter || double_inside {
        return commit(state);
    }

    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        let handle = hit(state, t, p, ppp);
        let start = to_doc(state, p, ppp);
        let t = state.free_transform.as_mut()?;
        t.drag = Some(TransformDrag {
            handle,
            start,
            offset: t.offset,
            scale: t.scale,
            angle: t.angle,
        });
    }
    let pointer = ui
        .input(|i| i.pointer.interact_pos())
        .map(|p| to_doc(state, p, ppp));
    let t = state.free_transform.as_mut()?;
    if let Some(drag) = t.drag {
        if let Some(p) = pointer {
            drag_to(t, drag, p, mods);
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            t.drag = None;
        }
    }
    preview(state, background);
    None
}

/// Shows the current box's result on the document.
pub fn preview(state: &mut DocState, background: [u8; 3]) {
    let Some(t) = &state.free_transform else {
        return;
    };
    let m = t.affine();
    if m == t.applied {
        return;
    }
    state.doc.restore(&t.before);
    if transform::transform(&mut state.doc, m, background).is_ok()
        && let Some(t) = &mut state.free_transform
    {
        t.applied = m;
    }
}

/// Ends the session keeping the result ("Free Transform" in the history).
pub fn commit(state: &mut DocState) -> Option<Outcome> {
    let t = state.free_transform.take()?;
    if t.applied == Affine::IDENTITY {
        return Some(Outcome::Cancelled);
    }
    state.record("Free Transform");
    Some(Outcome::Committed(t.applied))
}

/// Ends the session putting the document back as it was.
pub fn cancel(state: &mut DocState) -> Outcome {
    if let Some(t) = state.free_transform.take()
        && t.applied != Affine::IDENTITY
    {
        state.doc.restore(&t.before);
    }
    Outcome::Cancelled
}

/// The cursor over the canvas while transforming.
pub fn cursor(state: &DocState, p: Pos2, ppp: f32) -> CursorIcon {
    let Some(t) = &state.free_transform else {
        return CursorIcon::Default;
    };
    let handle = t.drag.map_or_else(|| hit(state, t, p, ppp), |d| d.handle);
    match handle {
        TransformHandle::Move => CursorIcon::Move,
        TransformHandle::Rotate => CursorIcon::Alias,
        TransformHandle::Scale(hx, hy) => {
            // The handle's direction on screen, rotated with the box
            let dir = Vec2::new(hx as f32, hy as f32).normalized();
            let (sin, cos) = t.angle.sin_cos();
            let d = Vec2::new(dir.x * cos - dir.y * sin, dir.x * sin + dir.y * cos);
            let deg = d.y.atan2(d.x).to_degrees().rem_euclid(180.0);
            match ((deg + 22.5) / 45.0) as u32 % 4 {
                0 => CursorIcon::ResizeHorizontal,
                1 => CursorIcon::ResizeNwSe,
                2 => CursorIcon::ResizeVertical,
                _ => CursorIcon::ResizeNeSw,
            }
        }
    }
}

/// The Move tool's handles for an untransformed box, in document pixels.
fn controls_handles(bounds: (f32, f32, f32, f32)) -> [Pos2; 8] {
    let (x0, y0, x1, y1) = bounds;
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        (x0, y0),
        (cx, y0),
        (x1, y0),
        (x1, cy),
        (x1, y1),
        (cx, y1),
        (x0, y1),
        (x0, cy),
    ]
    .map(|(x, y)| Pos2::new(x, y))
}

/// Whether the screen point `p` grabs a handle of Show Transform Controls'
/// box around `bounds`.
pub fn controls_handle_at(
    state: &DocState,
    bounds: (f32, f32, f32, f32),
    p: Pos2,
    ppp: f32,
) -> bool {
    controls_handles(bounds)
        .iter()
        .any(|&h| to_screen(state, h, ppp).distance(p) <= GRAB)
}

/// Draws the Move tool's Show Transform Controls box: the Free Transform
/// box without the reference point.
pub fn draw_controls(
    ui: &Ui,
    state: &DocState,
    bounds: (f32, f32, f32, f32),
    canvas: Rect,
    ppp: f32,
) {
    let (x0, y0, x1, y1) = bounds;
    let quad = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .map(|(x, y)| to_screen(state, Pos2::new(x, y), ppp));
    let handles = controls_handles(bounds).map(|h| to_screen(state, h, ppp));
    draw_box(&ui.painter_at(canvas), quad, handles);
}

fn draw_box(painter: &egui::Painter, quad: [Pos2; 4], handles: [Pos2; 8]) {
    let line = Stroke::new(1.0, Color32::from_rgb(0x2c, 0x8b, 0xe8));
    for i in 0..4 {
        painter.line_segment([quad[i], quad[(i + 1) % 4]], line);
    }
    for h in handles {
        let r = Rect::from_center_size(h, Vec2::splat(HANDLE));
        painter.rect(
            r,
            0,
            Color32::WHITE,
            Stroke::new(1.0, Color32::from_gray(0x40)),
            StrokeKind::Inside,
        );
    }
}

/// Draws the transform box: thin outline, square handles and the center
/// reference point.
pub fn draw(ui: &Ui, state: &DocState, canvas: Rect, ppp: f32) {
    let Some(t) = &state.free_transform else {
        return;
    };
    let painter = ui.painter_at(canvas);
    let quad = corners(t).map(|c| to_screen(state, c, ppp));
    draw_box(
        &painter,
        quad,
        handles(t).map(|(h, _, _)| to_screen(state, h, ppp)),
    );
    let (cx, cy) = t.affine().apply(t.center());
    let c = to_screen(state, Pos2::new(cx, cy), ppp);
    painter.circle_stroke(c, pt(4.0), Stroke::new(1.0, Color32::from_gray(0x40)));
    painter.line_segment(
        [c - Vec2::new(pt(6.0), 0.0), c + Vec2::new(pt(6.0), 0.0)],
        Stroke::new(1.0, Color32::from_gray(0x40)),
    );
    painter.line_segment(
        [c - Vec2::new(0.0, pt(6.0)), c + Vec2::new(0.0, pt(6.0))],
        Stroke::new(1.0, Color32::from_gray(0x40)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> FreeTransform {
        let doc = op_core::Document::new_with_background("t", 10, 10, op_core::Color::WHITE);
        FreeTransform::new(doc.snapshot(), (0.0, 0.0, 4.0, 2.0))
    }

    fn drag(t: &FreeTransform, handle: TransformHandle, start: Pos2) -> TransformDrag {
        TransformDrag {
            handle,
            start,
            offset: t.offset,
            scale: t.scale,
            angle: t.angle,
        }
    }

    #[test]
    fn corner_scales_proportionally_from_the_opposite_corner() {
        let mut t = session();
        let d = drag(&t, TransformHandle::Scale(1, 1), Pos2::new(4.0, 2.0));
        drag_to(&mut t, d, Pos2::new(8.0, 4.0), Modifiers::NONE);
        assert!((t.scale.0 - 2.0).abs() < 1e-4 && (t.scale.1 - 2.0).abs() < 1e-4);
        // The top-left corner stays put
        let tl = t.affine().apply((0.0, 0.0));
        assert!(tl.0.abs() < 1e-4 && tl.1.abs() < 1e-4, "{tl:?}");
        // Shift scales freely
        let mut t = session();
        let d = drag(&t, TransformHandle::Scale(1, 1), Pos2::new(4.0, 2.0));
        drag_to(&mut t, d, Pos2::new(8.0, 2.0), Modifiers::SHIFT);
        assert!((t.scale.0 - 2.0).abs() < 1e-4 && (t.scale.1 - 1.0).abs() < 1e-4);
    }

    #[test]
    fn side_handles_move_and_rotate() {
        let mut t = session();
        let d = drag(&t, TransformHandle::Scale(1, 0), Pos2::new(4.0, 1.0));
        drag_to(&mut t, d, Pos2::new(6.0, 5.0), Modifiers::NONE);
        assert!((t.scale.0 - 1.5).abs() < 1e-4 && t.scale.1 == 1.0);
        let left = t.affine().apply((0.0, 1.0));
        assert!(
            left.0.abs() < 1e-4 && (left.1 - 1.0).abs() < 1e-4,
            "{left:?}"
        );

        let mut t = session();
        let d = drag(&t, TransformHandle::Move, Pos2::new(1.0, 1.0));
        drag_to(&mut t, d, Pos2::new(4.0, 3.0), Modifiers::NONE);
        assert_eq!(t.offset, (3.0, 2.0));

        // Rotating a quarter turn around the center (2, 1); Shift snaps
        let mut t = session();
        let d = drag(&t, TransformHandle::Rotate, Pos2::new(6.0, 1.0));
        drag_to(&mut t, d, Pos2::new(2.2, 5.0), Modifiers::SHIFT);
        assert!(
            (t.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
            "{}",
            t.angle
        );
    }

    #[test]
    fn hit_testing_the_quad() {
        let q = [
            Pos2::new(0.0, 0.0),
            Pos2::new(4.0, 0.0),
            Pos2::new(4.0, 2.0),
            Pos2::new(0.0, 2.0),
        ];
        assert!(inside(&q, Pos2::new(1.0, 1.0)));
        assert!(!inside(&q, Pos2::new(5.0, 1.0)));
    }
}
