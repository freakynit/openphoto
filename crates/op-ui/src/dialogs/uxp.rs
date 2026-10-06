//! Controls of Photoshop 2026's UXP adjustment dialogs (Brightness/Contrast,
//! Color Balance, Hue/Saturation), measured on Photoshop: 12 pt Adobe Clean
//! labels (Source Sans 3 here), 24 pt number boxes, Spectrum sliders and a
//! column of pill buttons 109 pt wide, 20 pt from the right edge.

use egui::{Align2, Color32, Key, Mesh, Pos2, Rect, Sense, Stroke, Ui, vec2};

use super::common;
use crate::theme::{self, pt};

pub const TEXT: Color32 = Color32::from_gray(0xf0);
/// The plain slider track.
pub const TRACK: Color32 = Color32::from_gray(0x73);
const THUMB: Color32 = Color32::from_gray(0xd0);
const THUMB_DRAGGED: Color32 = Color32::from_gray(0xf0);
/// The thumb's ring: 13 pt across, 1 pt thick, hollow; the track stops
/// 4.5 pt short of it on either side.
const THUMB_RADIUS: f32 = pt(6.5);
const TRACK_GAP: f32 = pt(4.5);

pub fn font() -> egui::FontId {
    theme::uxp(pt(12.0))
}

/// A label with its capitals centered on `left_center`.
pub fn label(ui: &Ui, left_center: Pos2, text: &str) -> Rect {
    // Source Sans sits a little higher than Adobe Clean
    ui.painter().text(
        left_center + vec2(0.0, pt(0.75)),
        Align2::LEFT_CENTER,
        text,
        font(),
        TEXT,
    )
}

/// A number box (24 pt high): the value 11 pt in, a blue ring when it has
/// the keyboard focus. Up and Down arrows step the value by 1 (10 with
/// Shift) within `min..=max`. With `focus`, it takes the focus with its
/// text selected.
pub fn number_box(
    ui: &mut Ui,
    rect: Rect,
    text: &mut String,
    id: impl egui::AsIdSalt + Copy,
    range: (f32, f32),
    focus: bool,
) -> egui::Response {
    let response = common::text_field(ui, rect, text, id, font(), pt(11.0), focus);
    if response.has_focus() {
        let (up, down, shift) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.modifiers.shift,
            )
        });
        let step = if shift { 10.0 } else { 1.0 };
        let delta = if up {
            step
        } else if down {
            -step
        } else {
            0.0
        };
        if delta != 0.0
            && let Ok(v) = text.trim().parse::<f32>()
        {
            *text = format!("{}", (v + delta).clamp(range.0, range.1).round());
        }
    }
    response
}

/// A Spectrum slider along `x0..x1` at height `y`: a 2 pt track colored
/// by `colors` (0 at the left end, 1 at the right), broken around a hollow
/// ring thumb. The value runs from `min` to `max` with `zero` at the middle
/// of the track (Photoshop centers 0 even when the range is lopsided, as
/// Contrast's −50–100). Pressing anywhere on the track moves the thumb
/// there; returns the new value while dragging.
#[allow(clippy::too_many_arguments)]
pub fn slider(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    x0: f32,
    x1: f32,
    y: f32,
    value: f32,
    (min, max): (f32, f32),
    colors: &dyn Fn(f32) -> Color32,
) -> Option<f32> {
    // A range starting at 0 (Colorize's hue and saturation) runs from the
    // left end; otherwise 0 is in the middle
    let mid = if min >= 0.0 { x0 } else { (x0 + x1) / 2.0 };
    let to_x = |v: f32| {
        if v >= 0.0 || min >= 0.0 {
            mid + (x1 - mid) * (v / max).clamp(0.0, 1.0)
        } else {
            mid - (mid - x0) * (v / min).min(1.0)
        }
    };
    let to_value = |x: f32| {
        if x >= mid || min >= 0.0 {
            max * ((x - mid) / (x1 - mid)).clamp(0.0, 1.0)
        } else {
            min * ((mid - x) / (mid - x0)).clamp(0.0, 1.0)
        }
    };
    let hit = Rect::from_min_max(
        Pos2::new(x0 - THUMB_RADIUS, y - pt(9.0)),
        Pos2::new(x1 + THUMB_RADIUS, y + pt(9.0)),
    );
    let response = ui.interact(
        hit,
        ui.id().with(("uxp-slider", id)),
        Sense::click_and_drag(),
    );
    let mut changed = None;
    if (response.dragged() || response.clicked() || response.drag_started())
        && let Some(p) = response.interact_pointer_pos()
    {
        changed = Some(to_value(p.x).round());
    }
    let value = changed.unwrap_or(value);
    let x = to_x(value);
    let painter = ui.painter();
    let half = pt(1.0);
    let segment = |a: f32, b: f32| {
        if b <= a {
            return;
        }
        // Thin quads, each with the colors at its ends
        let mut mesh = Mesh::default();
        let steps = ((b - a) / pt(4.0)).ceil().max(1.0) as usize;
        for k in 0..=steps {
            let px = a + (b - a) * k as f32 / steps as f32;
            let c = colors((px - x0) / (x1 - x0));
            mesh.colored_vertex(Pos2::new(px, y - half), c);
            mesh.colored_vertex(Pos2::new(px, y + half), c);
            if k > 0 {
                let i = (2 * k) as u32;
                mesh.add_triangle(i - 2, i - 1, i);
                mesh.add_triangle(i - 1, i + 1, i);
            }
        }
        painter.add(egui::Shape::mesh(mesh));
    };
    segment(x0, x - THUMB_RADIUS - TRACK_GAP);
    segment(x + THUMB_RADIUS + TRACK_GAP, x1);
    let ring = if response.dragged() {
        THUMB_DRAGGED
    } else {
        THUMB
    };
    painter.circle_stroke(
        Pos2::new(x, y),
        THUMB_RADIUS - pt(0.75),
        Stroke::new(pt(1.0), ring),
    );
    changed
}

/// A gradient through evenly spaced color stops, blended in sRGB between
/// neighbors (Photoshop's tracks, sampled from its dialogs).
pub fn stops(colors: &'static [[u8; 3]]) -> impl Fn(f32) -> Color32 {
    move |t| {
        let f = t.clamp(0.0, 1.0) * (colors.len() - 1) as f32;
        let i = (f as usize).min(colors.len() - 2);
        let [a, b] = [colors[i], colors[i + 1]].map(|[r, g, b]| Color32::from_rgb(r, g, b));
        a.lerp_to_gamma(b, f - i as f32)
    }
}

/// What the button column was asked to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Button {
    Ok,
    Cancel,
    /// The third button (Auto), when there is one.
    Third,
}

/// OK, Cancel and an optional third button down the right side, 36 pt
/// apart from 48 pt down. OK is the default button; with `ok_focused` it
/// also holds the keyboard focus (no number box has it).
pub fn buttons(
    ui: &mut Ui,
    frame: Rect,
    third: Option<&str>,
    ok_enabled: bool,
    ok_focused: bool,
) -> Option<Button> {
    let x = frame.right() - pt(129.0);
    let at = |i: f32| {
        Rect::from_min_size(
            Pos2::new(x, frame.top() + pt(48.0 + 36.0 * i)),
            vec2(pt(109.0), pt(24.0)),
        )
    };
    let bold = theme::uxp_bold(pt(12.0));
    let ok = if ok_focused && ok_enabled {
        common::ps_focused_button(ui, at(0.0), "OK", bold.clone())
    } else {
        common::ps_button_with(ui, at(0.0), "OK", true, ok_enabled, bold.clone())
    };
    let cancel = common::ps_button_with(ui, at(1.0), "Cancel", false, true, bold.clone());
    let third = third.map(|label| common::ps_button_with(ui, at(2.0), label, false, true, bold));
    if cancel.clicked() {
        return Some(Button::Cancel);
    }
    if third.is_some_and(|r| r.clicked()) {
        return Some(Button::Third);
    }
    let enter = ui.input(|i| i.key_pressed(Key::Enter));
    ((ok.clicked() || enter) && ok_enabled).then_some(Button::Ok)
}

/// The "Preview (Opt+P)" checkbox with its box's top-left at `min`; ⌥P
/// toggles it too.
pub fn preview(ui: &mut Ui, min: Pos2, preview: &mut bool) {
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::ALT, Key::P)) {
        *preview = !*preview;
    }
    checkbox(ui, min, "Preview (Opt+P)", preview);
}

/// A 12 pt checkbox with its label 9.5 pt to the right.
pub fn checkbox(ui: &mut Ui, min: Pos2, label: &str, checked: &mut bool) {
    common::ps_checkbox_with(ui, min, label, checked, true, font(), pt(1.0));
}
