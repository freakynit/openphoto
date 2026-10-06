//! Edit > Free Transform (Cmd+T) on the canvas: the transform box with its
//! handles, dragging to move, scale and rotate, and committing or
//! cancelling.
//!
//! Like Photoshop: dragging inside the box moves; a handle scales,
//! proportionally by default (Shift scales freely; side handles always
//! scale one axis), from the opposite handle (Alt: from the center);
//! dragging outside the box rotates (Shift: 15° steps). Cmd-dragging a
//! corner distorts it, Cmd-Shift a side skews, Cmd-Alt-Shift a corner puts
//! it in perspective; Edit > Transform > Skew, Distort and Perspective (and
//! the right-click menu) make those the plain drags. Enter or a
//! double-click inside commits, Escape cancels.

use egui::{Color32, CursorIcon, Key, Modifiers, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use op_core::transform::{self, Projective, TransformError};

use crate::document_view::{to_doc, to_screen};
use crate::state::{
    DocState, FreeTransform, TransformDrag, TransformHandle, TransformMode, WarpGrab,
};
use crate::theme::pt;
use op_core::transform::WarpMesh;

/// Handles closer than this to the pointer (in points) are grabbed.
const GRAB: f32 = pt(8.0);
const HANDLE: f32 = pt(7.0);

/// How a Free Transform session ended.
pub enum Outcome {
    Committed(Projective),
    Cancelled,
}

/// Starts Free Transform on the active layer (or the selected pixels).
pub fn start(state: &mut DocState) -> Result<(), TransformError> {
    start_in(state, TransformMode::Free)
}

/// Select > Transform Selection: the box around the selection, moving
/// only its outline. Returns false without a selection.
pub fn start_selection(state: &mut DocState) -> bool {
    let Some(bounds) = transform::selection_bounds(&state.doc) else {
        return false;
    };
    let mut t = FreeTransform::new(state.doc.snapshot(), bounds);
    t.selection_only = true;
    state.free_transform = Some(t);
    true
}

/// Starts a transform in one of Edit > Transform's modes (or switches the
/// running one to it, keeping the box).
pub fn start_in(state: &mut DocState, mode: TransformMode) -> Result<(), TransformError> {
    if let Some(t) = &mut state.free_transform {
        set_mode(t, mode);
        return Ok(());
    }
    let bounds = transform::bounds(&state.doc)?;
    let mut t = FreeTransform::new(state.doc.snapshot(), bounds);
    set_mode(&mut t, mode);
    state.free_transform = Some(t);
    Ok(())
}

/// Switches the box to `mode`. Warp starts from a flat mesh carried by
/// the box's current map, so what was done so far stays.
pub fn set_mode(t: &mut FreeTransform, mode: TransformMode) {
    t.mode = mode;
    if mode == TransformMode::Warp && t.warp.is_none() {
        let m = t.mapping();
        let mut mesh = WarpMesh::flat(t.bounds);
        for p in &mut mesh.points {
            *p = m.apply(*p);
        }
        t.warp = Some(mesh);
    }
}

/// Where a press at document point `p` grabs the warp mesh: a boundary
/// control point within reach, else the nearest surface point if the
/// press is on the surface.
fn warp_grab(state: &DocState, mesh: &WarpMesh, p: Pos2, ppp: f32) -> Option<WarpGrab> {
    let screen = to_screen(state, p, ppp);
    for (k, c) in mesh.points.iter().enumerate() {
        if to_screen(state, Pos2::new(c.0, c.1), ppp).distance(screen) <= GRAB {
            return Some(WarpGrab::Point(k));
        }
    }
    // The nearest of a fine grid of surface points, if close enough
    let mut best = (f32::MAX, 0.0, 0.0);
    for j in 0..=32 {
        for i in 0..=32 {
            let (u, v) = (i as f32 / 32.0, j as f32 / 32.0);
            let (x, y) = mesh.at(u, v);
            let d = (Pos2::new(x, y) - p).length_sq();
            if d < best.0 {
                best = (d, u, v);
            }
        }
    }
    let spacing = {
        let (a, b) = (mesh.at(0.0, 0.0), mesh.at(1.0, 1.0));
        ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() / 32.0
    };
    (best.0.sqrt() <= spacing.max(1.0) * 1.5).then_some(WarpGrab::Surface(best.1, best.2))
}

/// The box's corners in document pixels, clockwise from the top left.
fn corners(t: &FreeTransform) -> [Pos2; 4] {
    if let Some(q) = t.quad {
        return q;
    }
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
    let m = t.mapping();
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

/// What a handle drag does with the box's corners, from the mode and the
/// modifiers (Photoshop's: Cmd distorts, Cmd-Shift skews a side,
/// Cmd-Alt-Shift puts a corner in perspective).
fn corner_mode(mode: TransformMode, mods: Modifiers) -> Option<TransformMode> {
    if mods.command && mods.alt && mods.shift {
        return Some(TransformMode::Perspective);
    }
    if mods.command && mods.shift {
        return Some(TransformMode::Skew);
    }
    if mods.command {
        return Some(TransformMode::Distort);
    }
    (mode != TransformMode::Free).then_some(mode)
}

/// Moves corners of `q` for a handle drag by `d` (document pixels).
fn reshape(q: [Pos2; 4], (hx, hy): (i8, i8), d: Vec2, mode: TransformMode) -> [Pos2; 4] {
    // Corner indices: 0 top left, 1 top right, 2 bottom right, 3 bottom left
    let corner = |hx: i8, hy: i8| match (hx, hy) {
        (-1, -1) => 0,
        (1, -1) => 1,
        (1, 1) => 2,
        _ => 3,
    };
    let mut out = q;
    if hx != 0 && hy != 0 {
        let i = corner(hx, hy);
        match mode {
            TransformMode::Perspective => {
                // Along the stronger direction; the neighbor on that side
                // goes the other way
                if d.x.abs() >= d.y.abs() {
                    let j = corner(-hx, hy);
                    out[i].x += d.x;
                    out[j].x -= d.x;
                } else {
                    let j = corner(hx, -hy);
                    out[i].y += d.y;
                    out[j].y -= d.y;
                }
            }
            TransformMode::Skew => {
                // Along one axis only
                if d.x.abs() >= d.y.abs() {
                    out[i].x += d.x;
                } else {
                    out[i].y += d.y;
                }
            }
            _ => out[i] += d,
        }
    } else {
        // A side: its two corners
        let (i, j) = if hx == 0 {
            (corner(-1, hy), corner(1, hy))
        } else {
            (corner(hx, -1), corner(hx, 1))
        };
        let d = match mode {
            // Along the side
            TransformMode::Skew | TransformMode::Perspective => {
                let along = (q[j] - q[i]).normalized();
                along * d.dot(along)
            }
            _ => d,
        };
        out[i] += d;
        out[j] += d;
    }
    out
}

/// Updates the box for a drag to `p` (document pixels).
fn drag_to(t: &mut FreeTransform, drag: TransformDrag, p: Pos2, mods: Modifiers) {
    // Free corners (or a drag that makes them): reshape, move or turn them
    let reshaping = match drag.handle {
        TransformHandle::Scale(..) => corner_mode(t.mode, mods),
        _ => None,
    };
    if let (Some(q), TransformHandle::Move | TransformHandle::Rotate) = (drag.quad, drag.handle) {
        let c = q.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / 4.0;
        let c = c.to_pos2();
        t.quad = Some(match drag.handle {
            TransformHandle::Move => q.map(|v| v + (p - drag.start)),
            _ => {
                let mut a = (p - c).angle() - (drag.start - c).angle();
                if mods.shift {
                    let step = 15f32.to_radians();
                    a = (a / step).round() * step;
                }
                let (s, co) = a.sin_cos();
                q.map(|v| {
                    let r = v - c;
                    c + Vec2::new(r.x * co - r.y * s, r.x * s + r.y * co)
                })
            }
        });
        return;
    }
    if let TransformHandle::Scale(hx, hy) = drag.handle
        && (reshaping.is_some() || drag.quad.is_some())
    {
        let start = drag.quad.unwrap_or_else(|| corners(t));
        let mode = reshaping.unwrap_or(TransformMode::Distort);
        t.quad = Some(reshape(start, (hx, hy), p - drag.start, mode));
        return;
    }
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
        let quad = t.quad;
        let mesh = t.warp;
        let warp_grab = match (&t.warp, t.mode) {
            (Some(m), TransformMode::Warp) => warp_grab(state, m, start, ppp),
            _ => None,
        };
        let t = state.free_transform.as_mut()?;
        t.drag = Some(TransformDrag {
            handle,
            start,
            offset: t.offset,
            scale: t.scale,
            angle: t.angle,
            quad,
            mesh,
            warp_grab,
        });
    }
    let pointer = ui
        .input(|i| i.pointer.interact_pos())
        .map(|p| to_doc(state, p, ppp));
    let t = state.free_transform.as_mut()?;
    if let Some(drag) = t.drag {
        if let Some(p) = pointer {
            if t.mode == TransformMode::Warp {
                // Warp: a control point or the surface follows the pointer
                if let (Some(mut mesh), Some(grab)) = (drag.mesh, drag.warp_grab) {
                    let d = (p.x - drag.start.x, p.y - drag.start.y);
                    match grab {
                        WarpGrab::Point(k) => {
                            mesh.points[k].0 += d.0;
                            mesh.points[k].1 += d.1;
                        }
                        WarpGrab::Surface(u, v) => mesh.pull((u, v), d),
                    }
                    t.warp = Some(mesh);
                }
            } else {
                drag_to(t, drag, p, mods);
            }
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
    let m = t.mapping();
    let how = t.interpolation;
    if let Some(mesh) = t.warp {
        if Some(mesh) == t.applied_warp && how == t.applied_interpolation {
            return;
        }
        let bounds = t.bounds;
        state.doc.restore(&t.before);
        if transform::warp(&mut state.doc, bounds, &mesh, background, how).is_ok()
            && let Some(t) = &mut state.free_transform
        {
            t.applied_warp = Some(mesh);
            t.applied_interpolation = how;
        }
        return;
    }
    if m == t.applied && how == t.applied_interpolation {
        return;
    }
    state.doc.restore(&t.before);
    let done = if t.selection_only {
        transform::transform_selection(&mut state.doc, m)
    } else {
        transform::transform_with(&mut state.doc, m, background, how).is_ok()
    };
    if done && let Some(t) = &mut state.free_transform {
        t.applied = m;
        t.applied_interpolation = how;
    }
}

/// Ends the session keeping the result ("Free Transform" in the history).
pub fn commit(state: &mut DocState) -> Option<Outcome> {
    let t = state.free_transform.take()?;
    if let Some(mesh) = t.applied_warp {
        // Photoshop records a warp as "Warp" (it isn't repeated by
        // Transform Again here); an untouched mesh changes nothing
        if mesh != WarpMesh::flat(t.bounds) {
            state.record("Warp");
        } else {
            state.doc.restore(&t.before);
        }
        return Some(Outcome::Cancelled);
    }
    if t.applied == Projective::IDENTITY {
        return Some(Outcome::Cancelled);
    }
    if t.selection_only {
        state.record("Transform Selection");
        return Some(Outcome::Cancelled);
    }
    state.record("Free Transform");
    Some(Outcome::Committed(t.applied))
}

/// Ends the session putting the document back as it was.
pub fn cancel(state: &mut DocState) -> Outcome {
    if let Some(t) = state.free_transform.take()
        && (t.applied != Projective::IDENTITY || t.applied_warp.is_some())
    {
        state.doc.restore(&t.before);
    }
    Outcome::Cancelled
}

/// Turns or flips the box itself (the right-click menu's Rotate 180°,
/// Rotate 90°... and Flip while transforming).
pub fn turn_box(t: &mut FreeTransform, how: transform::FixedTransform) {
    use transform::FixedTransform as F;
    if let Some(q) = t.quad {
        let c = (q.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / 4.0).to_pos2();
        let f = |v: Pos2| -> Pos2 {
            let r = v - c;
            c + match how {
                F::Rotate180 => -r,
                F::Rotate90Clockwise => Vec2::new(-r.y, r.x),
                F::Rotate90CounterClockwise => Vec2::new(r.y, -r.x),
                F::FlipHorizontal => Vec2::new(-r.x, r.y),
                F::FlipVertical => Vec2::new(r.x, -r.y),
            }
        };
        t.quad = Some(q.map(f));
        return;
    }
    let quarter = std::f32::consts::FRAC_PI_2;
    match how {
        F::Rotate180 => t.angle += 2.0 * quarter,
        F::Rotate90Clockwise => t.angle += quarter,
        F::Rotate90CounterClockwise => t.angle -= quarter,
        F::FlipHorizontal => t.scale.0 = -t.scale.0,
        F::FlipVertical => t.scale.1 = -t.scale.1,
    }
}

/// The right-click menu while transforming, as in Photoshop: the modes,
/// then turning and flipping the box.
pub fn context_menu(ui: &mut Ui, t: &mut FreeTransform) {
    let modes = [
        ("Free Transform", TransformMode::Free),
        ("Scale", TransformMode::Free),
        ("Rotate", TransformMode::Free),
        ("Skew", TransformMode::Skew),
        ("Distort", TransformMode::Distort),
        ("Perspective", TransformMode::Perspective),
    ];
    for (label, mode) in modes {
        if ui.button(label).clicked() {
            set_mode(t, mode);
            ui.close();
        }
    }
    if ui.button("Warp").clicked() {
        set_mode(t, TransformMode::Warp);
        ui.close();
    }
    ui.separator();
    ui.add_enabled(false, egui::Button::new("Content-Aware Scale"));
    ui.add_enabled(false, egui::Button::new("Puppet Warp"));
    ui.separator();
    use transform::FixedTransform as F;
    for (label, how) in [
        ("Rotate 180°", F::Rotate180),
        ("Rotate 90° Clockwise", F::Rotate90Clockwise),
        ("Rotate 90° Counter Clockwise", F::Rotate90CounterClockwise),
    ] {
        if ui.button(label).clicked() {
            turn_box(t, how);
            ui.close();
        }
    }
    ui.separator();
    for (label, how) in [
        ("Flip Horizontal", F::FlipHorizontal),
        ("Flip Vertical", F::FlipVertical),
    ] {
        if ui.button(label).clicked() {
            turn_box(t, how);
            ui.close();
        }
    }
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

/// Warp's mesh as Photoshop 2026 draws it: blue outline and thirds lines
/// on the surface, the boundary's control points as blue dots (bigger at
/// the corners).
fn draw_warp(painter: &egui::Painter, state: &DocState, mesh: &WarpMesh, ppp: f32) {
    let blue = Color32::from_rgb(0x5b, 0x8b, 0xe6);
    let at = |u: f32, v: f32| {
        let (x, y) = mesh.at(u, v);
        to_screen(state, Pos2::new(x, y), ppp)
    };
    let curve = |f: &dyn Fn(f32) -> Pos2| (0..=24).map(|k| f(k as f32 / 24.0)).collect::<Vec<_>>();
    for k in 0..=3 {
        let t = k as f32 / 3.0;
        let width = if k == 0 || k == 3 { pt(1.5) } else { pt(0.75) };
        painter.add(egui::Shape::line(
            curve(&|s| at(t, s)),
            Stroke::new(width, blue),
        ));
        painter.add(egui::Shape::line(
            curve(&|s| at(s, t)),
            Stroke::new(width, blue),
        ));
    }
    for (k, p) in mesh.points.iter().enumerate() {
        let (i, j) = (k % 4, k / 4);
        let boundary = i == 0 || i == 3 || j == 0 || j == 3;
        if !boundary {
            continue;
        }
        let corner = (i == 0 || i == 3) && (j == 0 || j == 3);
        let c = to_screen(state, Pos2::new(p.0, p.1), ppp);
        painter.circle_filled(c, if corner { pt(5.0) } else { pt(3.5) }, blue);
    }
}

/// Draws the transform box: thin outline, square handles and the center
/// reference point.
pub fn draw(ui: &Ui, state: &DocState, canvas: Rect, ppp: f32) {
    let Some(t) = &state.free_transform else {
        return;
    };
    let painter = ui.painter_at(canvas);
    if let (Some(mesh), TransformMode::Warp) = (&t.warp, t.mode) {
        draw_warp(&painter, state, mesh, ppp);
        return;
    }
    let quad = corners(t).map(|c| to_screen(state, c, ppp));
    draw_box(
        &painter,
        quad,
        handles(t).map(|(h, _, _)| to_screen(state, h, ppp)),
    );
    // The reference point shows when its switch is on (off by default,
    // as in Photoshop 2026)
    if !t.show_reference {
        return;
    }
    let (cx, cy) = t.reference_now();
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
            quad: t.quad,
            mesh: t.warp,
            warp_grab: None,
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
    fn distort_skew_and_perspective() {
        let p = |x: f32, y: f32| Pos2::new(x, y);
        // Cmd-dragging the bottom-right corner moves only it
        let mut t = session();
        let d = drag(&t, TransformHandle::Scale(1, 1), p(4.0, 2.0));
        drag_to(&mut t, d, p(5.0, 4.0), Modifiers::COMMAND);
        assert_eq!(
            t.quad,
            Some([p(0.0, 0.0), p(4.0, 0.0), p(5.0, 4.0), p(0.0, 2.0)])
        );
        // A second drag goes on from those corners, a plain one too
        let d = drag(&t, TransformHandle::Scale(-1, -1), p(0.0, 0.0));
        drag_to(&mut t, d, p(1.0, 1.0), Modifiers::NONE);
        assert_eq!(t.quad.unwrap()[0], p(1.0, 1.0));
        assert_eq!(t.quad.unwrap()[2], p(5.0, 4.0));
        // Perspective: the top-right corner out, the top-left in
        let mut t = session();
        t.mode = TransformMode::Perspective;
        let d = drag(&t, TransformHandle::Scale(1, -1), p(4.0, 0.0));
        drag_to(&mut t, d, p(5.0, 0.2), Modifiers::NONE);
        assert_eq!(
            t.quad,
            Some([p(-1.0, 0.0), p(5.0, 0.0), p(4.0, 2.0), p(0.0, 2.0)])
        );
        // Skew a side: Cmd-Shift on the right side slides it along itself
        let mut t = session();
        let d = drag(&t, TransformHandle::Scale(1, 0), p(4.0, 1.0));
        drag_to(
            &mut t,
            d,
            p(6.0, 2.0),
            Modifiers::COMMAND | Modifiers::SHIFT,
        );
        assert_eq!(
            t.quad,
            Some([p(0.0, 0.0), p(4.0, 1.0), p(4.0, 3.0), p(0.0, 2.0)])
        );
        // The map follows the corners
        let (x, y) = t.mapping().apply((4.0, 0.0));
        assert!((x - 4.0).abs() < 1e-4 && (y - 1.0).abs() < 1e-4);
    }

    #[test]
    fn turning_and_flipping_the_box() {
        use transform::FixedTransform as F;
        let mut t = session();
        turn_box(&mut t, F::Rotate90Clockwise);
        assert!((t.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        turn_box(&mut t, F::FlipHorizontal);
        assert_eq!(t.scale, (-1.0, 1.0));
        // Free corners turn about their middle
        let mut t = session();
        t.quad = Some([
            Pos2::new(0.0, 0.0),
            Pos2::new(4.0, 0.0),
            Pos2::new(4.0, 2.0),
            Pos2::new(0.0, 2.0),
        ]);
        turn_box(&mut t, F::Rotate180);
        assert_eq!(t.quad.unwrap()[0], Pos2::new(4.0, 2.0));
    }

    #[test]
    fn options_bar_numbers_pivot_on_the_reference_point() {
        // W 50% with the top-left reference point: that corner stays
        let mut t = session();
        t.reference = (-1, -1);
        t.pivoting(|t| t.scale = (0.5, 0.5));
        let tl = t.mapping().apply((0.0, 0.0));
        assert!(tl.0.abs() < 1e-4 && tl.1.abs() < 1e-4, "{tl:?}");
        let br = t.mapping().apply((4.0, 2.0));
        assert!(
            (br.0 - 2.0).abs() < 1e-4 && (br.1 - 1.0).abs() < 1e-4,
            "{br:?}"
        );
        assert_eq!(t.reference_now(), (0.0, 0.0));
        // 90° about the center keeps the center
        let mut t = session();
        t.pivoting(|t| t.angle = std::f32::consts::FRAC_PI_2);
        let c = t.reference_now();
        assert!((c.0 - 2.0).abs() < 1e-4 && (c.1 - 1.0).abs() < 1e-4);
        // A 45° horizontal skew about the center slides the top left
        let mut t = session();
        t.pivoting(|t| t.skew.0 = std::f32::consts::FRAC_PI_4);
        let (x, y) = t.mapping().apply((0.0, 0.0));
        assert!((x + 1.0).abs() < 1e-4 && y.abs() < 1e-4, "{x} {y}");
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
