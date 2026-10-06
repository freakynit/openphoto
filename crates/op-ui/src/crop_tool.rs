//! The Crop tool (C): a crop box over the canvas, as in Photoshop 2026.
//!
//! The box covers the whole canvas when the tool is picked. Its handles
//! resize it (Shift keeps the proportions, Alt resizes around the center;
//! a ratio or size from the options bar keeps its proportions), and it may
//! reach past the canvas, which then grows. By default (not Classic Mode)
//! the box stays in the middle of the view and the image moves under it:
//! dragging inside pans the image, and a handle's opposite side stays put
//! on the image. In Classic Mode the box itself moves. Outside the box the
//! image is shielded; an overlay (rule of thirds...) is drawn inside.
//! Enter or a double-click inside crops; Escape resets the box.

use egui::{Color32, CursorIcon, Key, Modifiers, Pos2, Rect, Shape, Stroke, Ui, Vec2};

use crate::document_view::{to_doc, to_screen};
use crate::state::{CropBox, CropDrag, DocState};
use crate::theme::{color, pt};

const GRAB: f32 = pt(10.0);
/// The box's lines and handles (Photoshop 2026, measured).
const LIGHT: Color32 = Color32::from_gray(0xf4);
const DARK: Color32 = Color32::from_gray(0x32);
const HANDLE_THICK: f32 = pt(4.0);
const CORNER_ARM: f32 = pt(23.0);
const SIDE_BAR: f32 = pt(47.0);

/// Units of the options bar's Width and Height in W x H x Resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeUnit {
    Pixels,
    Inches,
    Centimeters,
    Millimeters,
}

impl SizeUnit {
    fn suffix(self) -> &'static str {
        match self {
            Self::Pixels => "px",
            Self::Inches => "in",
            Self::Centimeters => "cm",
            Self::Millimeters => "mm",
        }
    }

    /// Units per inch, for the printed units.
    fn per_inch(self) -> Option<f32> {
        match self {
            Self::Pixels => None,
            Self::Inches => Some(1.0),
            Self::Centimeters => Some(2.54),
            Self::Millimeters => Some(25.4),
        }
    }
}

/// The options bar's ratio menu, in Photoshop 2026's order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CropPreset {
    /// Free, or the ratio typed in Width and Height.
    Ratio,
    /// Free, or the size (and resolution) typed: the crop is resampled to it.
    SizeResolution,
    OriginalRatio,
    FixedRatio(&'static str, f32, f32),
    FrontImage,
    /// Width × height in a unit, at a resolution (ppi).
    Size(&'static str, f32, f32, SizeUnit, f32),
}

/// The menu's groups (separators between them).
pub const PRESET_GROUPS: [&[CropPreset]; 4] = [
    &[CropPreset::Ratio, CropPreset::SizeResolution],
    &[CropPreset::OriginalRatio],
    &[
        CropPreset::FixedRatio("1 : 1 (Square)", 1.0, 1.0),
        CropPreset::FixedRatio("4 : 5 (8 : 10)", 4.0, 5.0),
        CropPreset::FixedRatio("5 : 7", 5.0, 7.0),
        CropPreset::FixedRatio("2 : 3 (4 : 6)", 2.0, 3.0),
        CropPreset::FixedRatio("16 : 9", 16.0, 9.0),
    ],
    &[
        CropPreset::FrontImage,
        CropPreset::Size("4 x 5 in 300 ppi", 4.0, 5.0, SizeUnit::Inches, 300.0),
        CropPreset::Size("8.5 x 11 in 300 ppi", 8.5, 11.0, SizeUnit::Inches, 300.0),
        CropPreset::Size(
            "1024 x 768 px 92 ppi",
            1024.0,
            768.0,
            SizeUnit::Pixels,
            92.0,
        ),
        CropPreset::Size(
            "1280 x 800 px 113 ppi",
            1280.0,
            800.0,
            SizeUnit::Pixels,
            113.0,
        ),
        CropPreset::Size(
            "1366 x 768 px 135 ppi",
            1366.0,
            768.0,
            SizeUnit::Pixels,
            135.0,
        ),
    ],
];

impl CropPreset {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ratio => "Ratio",
            Self::SizeResolution => "W x H x Resolution",
            Self::OriginalRatio => "Original Ratio",
            Self::FixedRatio(name, ..) | Self::Size(name, ..) => name,
            Self::FrontImage => "Front Image",
        }
    }

    /// Whether the fields hold a size (with a resolution) rather than a
    /// ratio.
    pub fn sized(self) -> bool {
        matches!(
            self,
            Self::SizeResolution | Self::Size(..) | Self::FrontImage
        )
    }
}

/// The overlay drawn in the box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    RuleOfThirds,
    Grid,
    Diagonal,
    Triangle,
    GoldenRatio,
    GoldenSpiral,
}

impl Overlay {
    pub const ALL: [Self; 6] = [
        Self::RuleOfThirds,
        Self::Grid,
        Self::Diagonal,
        Self::Triangle,
        Self::GoldenRatio,
        Self::GoldenSpiral,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::RuleOfThirds => "Rule of Thirds",
            Self::Grid => "Grid",
            Self::Diagonal => "Diagonal",
            Self::Triangle => "Triangle",
            Self::GoldenRatio => "Golden Ratio",
            Self::GoldenSpiral => "Golden Spiral",
        }
    }
}

/// When the overlay shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayShow {
    /// While the box is being changed.
    Auto,
    Always,
    Never,
}

/// What fills the canvas the box adds past the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CropFill {
    Background,
    GenerativeExpand,
    ContentAware,
}

impl CropFill {
    pub fn label(self) -> &'static str {
        match self {
            Self::Background => "Background (default)",
            Self::GenerativeExpand => "Generative Expand",
            Self::ContentAware => "Content-Aware Fill",
        }
    }
}

/// The Crop tool's options (the options bar and its gear menu), with
/// Photoshop 2026's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct CropOptions {
    pub preset: CropPreset,
    pub width: String,
    pub height: String,
    pub resolution: String,
    /// The resolution unit: pixels per centimeter instead of per inch.
    pub per_cm: bool,
    pub delete_cropped: bool,
    pub fill: CropFill,
    pub overlay: Overlay,
    pub overlay_show: OverlayShow,
    pub classic: bool,
    pub show_cropped_area: bool,
    pub auto_center: bool,
    pub shield: bool,
    /// `None`: Match Canvas.
    pub shield_color: Option<Color32>,
    pub shield_opacity: f32,
    pub auto_adjust_opacity: bool,
}

impl Default for CropOptions {
    fn default() -> Self {
        Self {
            preset: CropPreset::SizeResolution,
            width: String::new(),
            height: String::new(),
            resolution: String::new(),
            per_cm: false,
            delete_cropped: true,
            fill: CropFill::Background,
            overlay: Overlay::RuleOfThirds,
            overlay_show: OverlayShow::Always,
            classic: false,
            show_cropped_area: true,
            auto_center: true,
            shield: true,
            shield_color: None,
            shield_opacity: 0.75,
            auto_adjust_opacity: true,
        }
    }
}

/// A value typed in Width or Height: a number with an optional unit.
fn parse_length(text: &str) -> Option<(f32, Option<SizeUnit>)> {
    let t = text.trim();
    let (number, unit) = [
        ("px", SizeUnit::Pixels),
        ("in", SizeUnit::Inches),
        ("cm", SizeUnit::Centimeters),
        ("mm", SizeUnit::Millimeters),
    ]
    .into_iter()
    .find_map(|(s, u)| t.strip_suffix(s).map(|n| (n, Some(u))))
    .unwrap_or((t, None));
    let v: f32 = number.trim().parse().ok()?;
    (v > 0.0).then_some((v, unit))
}

fn format_length(v: f32, unit: SizeUnit) -> String {
    let n = format!("{v:.3}");
    let n = n.trim_end_matches('0').trim_end_matches('.');
    format!("{n} {}", unit.suffix())
}

impl CropOptions {
    /// Pixels per inch typed, if any.
    fn ppi(&self) -> Option<f32> {
        let v: f32 = self.resolution.trim().parse().ok()?;
        (v > 0.0).then_some(if self.per_cm { v * 2.54 } else { v })
    }

    /// The size typed, in pixels (W x H x Resolution and the sizes).
    pub fn output_size(&self) -> Option<(f32, f32)> {
        if !self.preset.sized() {
            return None;
        }
        let px = |text: &str| {
            let (v, unit) = parse_length(text)?;
            match unit.unwrap_or(SizeUnit::Pixels).per_inch() {
                None => Some(v),
                Some(k) => Some(v / k * self.ppi()?),
            }
        };
        Some((px(&self.width)?, px(&self.height)?))
    }

    /// The width : height the box keeps, if any.
    pub fn aspect(&self, doc: (u32, u32)) -> Option<f32> {
        match self.preset {
            CropPreset::OriginalRatio => Some(doc.0 as f32 / doc.1 as f32),
            p if p.sized() => {
                // A printed size keeps its ratio even without a resolution
                let (w, wu) = parse_length(&self.width)?;
                let (h, hu) = parse_length(&self.height)?;
                let inch = |v: f32, u: Option<SizeUnit>| {
                    u.and_then(|u| u.per_inch()).map_or(v, |k| v / k * 1000.0)
                };
                Some(inch(w, wu) / inch(h, hu))
            }
            _ => {
                let (w, _) = parse_length(&self.width)?;
                let (h, _) = parse_length(&self.height)?;
                Some(w / h)
            }
        }
    }

    /// Picks a preset: its numbers go into the fields.
    pub fn choose(&mut self, preset: CropPreset) {
        self.preset = preset;
        match preset {
            CropPreset::Ratio | CropPreset::SizeResolution | CropPreset::OriginalRatio => {
                self.width.clear();
                self.height.clear();
                self.resolution.clear();
            }
            CropPreset::FixedRatio(_, w, h) => {
                self.width = format!("{w}");
                self.height = format!("{h}");
                self.resolution.clear();
            }
            CropPreset::Size(_, w, h, unit, ppi) => {
                self.width = format_length(w, unit);
                self.height = format_length(h, unit);
                self.resolution = format!("{ppi}");
                self.per_cm = false;
            }
            CropPreset::FrontImage => {}
        }
    }

    /// The options bar's swap button.
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.width, &mut self.height);
    }

    /// The options bar's Clear: no ratio or size.
    pub fn clear(&mut self) {
        self.width.clear();
        self.height.clear();
        self.resolution.clear();
        if !self.preset.sized() {
            self.preset = CropPreset::Ratio;
        } else {
            self.preset = CropPreset::SizeResolution;
        }
    }
}

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

/// The largest box of `aspect` on the canvas, centered.
pub fn fitted(state: &DocState, aspect: f32) -> CropBox {
    let (w, h) = (state.doc.width as f32, state.doc.height as f32);
    let (bw, bh) = if w / h > aspect {
        (h * aspect, h)
    } else {
        (w, w / aspect)
    };
    CropBox {
        rect: Rect::from_center_size(Pos2::new(w / 2.0, h / 2.0), Vec2::new(bw, bh)),
        drag: None,
    }
}

/// Whether the box differs from the whole canvas (the options bar then
/// shows Cancel and Commit).
pub fn modified(state: &DocState) -> bool {
    state
        .crop
        .as_ref()
        .is_some_and(|c| c.rect != full(state).rect)
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

/// The box for a handle moved by `d` (document pixels) from where the
/// drag started. `aspect` (or Shift, the starting one) keeps proportions.
fn dragged(drag: CropDrag, d: Vec2, mods: Modifiers, aspect: Option<f32>) -> Rect {
    let start = drag.rect;
    let Some((hx, hy)) = drag.handle else {
        return start.translate(d);
    };
    let (mut x0, mut y0, mut x1, mut y1) =
        (start.left(), start.top(), start.right(), start.bottom());
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
    let r = Rect::from_two_pos(Pos2::new(x0, y0), Pos2::new(x1, y1));
    let ratio = aspect.or_else(|| {
        (mods.shift && hx != 0 && hy != 0 && start.height() > 0.0)
            .then(|| start.width() / start.height())
    });
    let Some(ratio) = ratio else {
        return r;
    };
    if hx != 0 && hy != 0 {
        // A corner: sized by the wider change, the opposite corner fixed
        let (w, h) = if r.width() / ratio > r.height() {
            (r.width(), r.width() / ratio)
        } else {
            (r.height() * ratio, r.height())
        };
        if mods.alt {
            return Rect::from_center_size(start.center(), Vec2::new(w, h));
        }
        let anchor = Pos2::new(
            if hx < 0 { start.right() } else { start.left() },
            if hy < 0 { start.bottom() } else { start.top() },
        );
        Rect::from_two_pos(anchor, anchor + Vec2::new(hx as f32 * w, hy as f32 * h))
    } else if hx != 0 {
        // A side: the other dimension follows, about the middle
        let h = r.width() / ratio;
        Rect::from_min_max(
            Pos2::new(r.left(), start.center().y - h / 2.0),
            Pos2::new(r.right(), start.center().y + h / 2.0),
        )
    } else {
        let w = r.height() * ratio;
        Rect::from_min_max(
            Pos2::new(start.center().x - w / 2.0, r.top()),
            Pos2::new(start.center().x + w / 2.0, r.bottom()),
        )
    }
}

/// Pans the view so the box's center is in the middle of the window.
pub fn center_on_box(state: &mut DocState, ppp: f32) {
    let Some(c) = state.crop.as_ref() else {
        return;
    };
    let on_screen = to_screen(state, c.rect.center(), ppp);
    state.view.offset += state.view.viewport.center() - on_screen;
}

/// Handles the Crop tool's input on the canvas. Returns true when the
/// image was cropped.
pub fn input(
    ui: &Ui,
    response: &egui::Response,
    state: &mut DocState,
    options: &CropOptions,
    background: op_core::Color,
    ppp: f32,
) -> bool {
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
        center_on_box(state, ppp);
        return false;
    }
    let mods = ui.input(|i| i.modifiers);
    let rect = state.crop.as_ref().map(|c| c.rect).unwrap_or(Rect::NOTHING);
    let double_inside = response.double_clicked()
        && response
            .interact_pointer_pos()
            .is_some_and(|p| rect.contains(to_doc(state, p, ppp)));
    if enter || double_inside {
        let changed = commit(state, options, background);
        center_on_box(state, ppp);
        return changed;
    }
    let aspect = options.aspect((state.doc.width, state.doc.height));
    // The box stays in the view's middle unless in Classic Mode
    let centered = !options.classic && options.auto_center;

    if response.drag_started_by(egui::PointerButton::Primary)
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        let d = to_doc(state, p, ppp);
        let handle = handle_at(state, rect, p, ppp);
        let drag = match handle {
            Some(h) => CropDrag {
                handle: Some(h),
                pointer: p,
                rect,
            },
            None if rect.contains(d) => CropDrag {
                handle: None,
                pointer: p,
                rect,
            },
            // Outside the box: draw a new one from here
            None => CropDrag {
                handle: Some((1, 1)),
                pointer: p,
                rect: Rect::from_min_max(d, d),
            },
        };
        if let Some(c) = &mut state.crop {
            c.drag = Some(drag);
        }
    }
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let zoom = state.view.zoom;
    if let Some(drag) = state.crop.as_ref().and_then(|c| c.drag) {
        if let Some(p) = pointer {
            // Screen movement since the press, in document pixels
            let d = (p - drag.pointer) * ppp / zoom;
            let new_rect = match (drag.handle, centered) {
                // The image moves under the box
                (None, true) => drag.rect.translate(-d),
                // The box stays centered, so a side moves twice as far on
                // the image (its opposite side stays put there)
                (Some(_), true) if !mods.alt => dragged(drag, d * 2.0, mods, aspect),
                _ => dragged(drag, d, mods, aspect),
            };
            if let Some(c) = &mut state.crop {
                c.rect = new_rect;
            }
            if centered && drag.rect.area() > 0.0 {
                center_on_box(state, ppp);
            }
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            if let Some(c) = &mut state.crop {
                c.drag = None;
                if c.rect.width() < 1.0 || c.rect.height() < 1.0 {
                    c.rect = drag.rect;
                }
            }
            if centered {
                center_on_box(state, ppp);
            }
        }
        ui.ctx().request_repaint();
    }
    false
}

/// Crops to the box (whole pixels; past the canvas the canvas grows, filled
/// per Fill), then resamples to the size typed in W x H x Resolution.
/// Returns true when the image changed; the box then covers the new canvas.
pub fn commit(state: &mut DocState, options: &CropOptions, background: op_core::Color) -> bool {
    let Some(c) = state.crop.take() else {
        return false;
    };
    let r = (
        c.rect.left().round() as i64,
        c.rect.top().round() as i64,
        c.rect.right().round() as i64,
        c.rect.bottom().round() as i64,
    );
    let (w, h) = (state.doc.width as i64, state.doc.height as i64);
    let mut changed = false;
    if r.2 > r.0 && r.3 > r.1 && r != (0, 0, w, h) {
        changed = op_core::image_ops::crop_extended(
            &mut state.doc,
            r,
            options.delete_cropped,
            background,
        );
    }
    if let Some((tw, th)) = options.output_size() {
        let (tw, th) = (tw.round().max(1.0) as u32, th.round().max(1.0) as u32);
        if (tw, th) != (state.doc.width, state.doc.height) {
            op_core::image_ops::resize(
                &mut state.doc,
                tw,
                th,
                op_core::image_ops::Resample::Automatic,
            );
            changed = true;
        }
        if let Some(ppi) = options.ppi() {
            state.doc.resolution = ppi;
        }
    }
    if changed {
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

/// The shield for the canvas renderer: the image outside the box, mixed
/// with the shield color (the pasteboard's for Match Canvas) in linear
/// light. With Show Cropped Area off, the outside is covered entirely.
pub fn shield(state: &DocState, options: &CropOptions) -> Option<op_render::Shield> {
    let c = state.crop.as_ref()?;
    if !options.shield && options.show_cropped_area {
        return None;
    }
    let rgb = options.shield_color.unwrap_or(color::PASTEBOARD);
    let opacity = if options.show_cropped_area {
        options.shield_opacity
    } else {
        1.0
    };
    Some(op_render::Shield {
        center: [c.rect.center().x, c.rect.center().y],
        half: [c.rect.width() / 2.0, c.rect.height() / 2.0],
        angle: 0.0,
        color: [
            rgb.r() as f32 / 255.0,
            rgb.g() as f32 / 255.0,
            rgb.b() as f32 / 255.0,
        ],
        opacity,
    })
}

/// The box's lines, its overlay and handles (Photoshop 2026: a 1 pt dark
/// and a 1 pt light line just outside the box, 4 pt light handles outside
/// it: L-shaped corners with 23 pt arms, 47 pt bars on the sides).
pub fn draw(ui: &Ui, state: &DocState, options: &CropOptions, canvas: Rect, ppp: f32) {
    let Some(c) = &state.crop else {
        return;
    };
    let painter = ui.painter_at(canvas);
    let r = Rect::from_two_pos(
        to_screen(state, c.rect.min, ppp),
        to_screen(state, c.rect.max, ppp),
    );
    let show = match options.overlay_show {
        OverlayShow::Always => true,
        OverlayShow::Auto => c.drag.is_some(),
        OverlayShow::Never => false,
    };
    if show {
        overlay(&painter, r, options.overlay);
    }
    let one = pt(1.0);
    painter.rect_stroke(r, 0, Stroke::new(one, DARK), egui::StrokeKind::Outside);
    painter.rect_stroke(
        r.expand(one),
        0,
        Stroke::new(one, LIGHT),
        egui::StrokeKind::Outside,
    );
    let t = HANDLE_THICK;
    let outer = r.expand(t);
    for (hx, hy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = Pos2::new(
            if hx < 0.0 {
                outer.left()
            } else {
                outer.right()
            },
            if hy < 0.0 {
                outer.top()
            } else {
                outer.bottom()
            },
        );
        let arm_h = Rect::from_two_pos(corner, corner + Vec2::new(-hx * CORNER_ARM, -hy * t));
        let arm_v = Rect::from_two_pos(corner, corner + Vec2::new(-hx * t, -hy * CORNER_ARM));
        painter.rect_filled(arm_h, pt(2.5), LIGHT);
        painter.rect_filled(arm_v, pt(2.5), LIGHT);
    }
    for (center, horizontal) in [
        (Pos2::new(r.center().x, r.top() - t / 2.0), true),
        (Pos2::new(r.center().x, r.bottom() + t / 2.0), true),
        (Pos2::new(r.left() - t / 2.0, r.center().y), false),
        (Pos2::new(r.right() + t / 2.0, r.center().y), false),
    ] {
        let size = if horizontal {
            Vec2::new(SIDE_BAR, t)
        } else {
            Vec2::new(t, SIDE_BAR)
        };
        painter.rect_filled(Rect::from_center_size(center, size), pt(2.5), LIGHT);
    }
}

/// The overlay's lines in `r` (screen): light 1 pt lines with a faint
/// shadow, which shows on light images, as Photoshop draws them.
fn overlay(painter: &egui::Painter, r: Rect, kind: Overlay) {
    let mut lines: Vec<[Pos2; 2]> = Vec::new();
    let at = |fx: f32, fy: f32| Pos2::new(r.left() + r.width() * fx, r.top() + r.height() * fy);
    let verticals = |fs: &[f32], lines: &mut Vec<[Pos2; 2]>| {
        for &f in fs {
            lines.push([at(f, 0.0), at(f, 1.0)]);
            lines.push([at(0.0, f), at(1.0, f)]);
        }
    };
    let mut curves: Vec<Vec<Pos2>> = Vec::new();
    match kind {
        Overlay::RuleOfThirds => verticals(&[1.0 / 3.0, 2.0 / 3.0], &mut lines),
        Overlay::GoldenRatio => verticals(&[0.382, 0.618], &mut lines),
        Overlay::Grid => {
            // Squares of about 40 pt
            let step = pt(40.0);
            let mut x = r.left() + step;
            while x < r.right() - 1.0 {
                lines.push([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())]);
                x += step;
            }
            let mut y = r.top() + step;
            while y < r.bottom() - 1.0 {
                lines.push([Pos2::new(r.left(), y), Pos2::new(r.right(), y)]);
                y += step;
            }
        }
        Overlay::Diagonal => {
            // 45° lines from each corner
            let s = r.width().min(r.height());
            for (corner, dir) in [
                (r.left_top(), Vec2::new(1.0, 1.0)),
                (r.right_top(), Vec2::new(-1.0, 1.0)),
                (r.left_bottom(), Vec2::new(1.0, -1.0)),
                (r.right_bottom(), Vec2::new(-1.0, -1.0)),
            ] {
                lines.push([corner, corner + dir * s]);
            }
        }
        Overlay::Triangle => {
            // A diagonal and the perpendiculars to it from the other corners
            let (a, b) = (r.left_bottom(), r.right_top());
            lines.push([a, b]);
            let dir = (b - a).normalized();
            for p in [r.left_top(), r.right_bottom()] {
                let foot = a + dir * (p - a).dot(dir);
                lines.push([p, foot]);
            }
        }
        Overlay::GoldenSpiral => {
            // Quarter turns in shrinking golden rectangles
            let phi = 0.618_034f32;
            let mut box_ = r;
            let mut points = Vec::new();
            for k in 0..8 {
                let (center, radius, from) = match k % 4 {
                    0 => {
                        let w = box_.width() * phi;
                        let sq = Rect::from_min_size(box_.min, Vec2::new(w, box_.height()));
                        box_ = Rect::from_min_max(Pos2::new(sq.right(), box_.top()), box_.max);
                        lines.push([
                            Pos2::new(sq.right(), sq.top()),
                            Pos2::new(sq.right(), sq.bottom()),
                        ]);
                        (sq.right_bottom(), Vec2::new(w, sq.height()), 180.0f32)
                    }
                    1 => {
                        let h = box_.height() * phi;
                        let sq = Rect::from_min_size(box_.min, Vec2::new(box_.width(), h));
                        box_ = Rect::from_min_max(Pos2::new(box_.left(), sq.bottom()), box_.max);
                        lines.push([sq.left_bottom(), sq.right_bottom()]);
                        (sq.left_bottom(), Vec2::new(sq.width(), h), 270.0)
                    }
                    2 => {
                        let w = box_.width() * phi;
                        let sq =
                            Rect::from_min_max(Pos2::new(box_.right() - w, box_.top()), box_.max);
                        box_ = Rect::from_min_max(box_.min, Pos2::new(sq.left(), box_.bottom()));
                        lines.push([sq.left_top(), sq.left_bottom()]);
                        (sq.left_top(), Vec2::new(w, sq.height()), 0.0)
                    }
                    _ => {
                        let h = box_.height() * phi;
                        let sq =
                            Rect::from_min_max(Pos2::new(box_.left(), box_.bottom() - h), box_.max);
                        box_ = Rect::from_min_max(box_.min, Pos2::new(box_.right(), sq.top()));
                        lines.push([sq.left_top(), sq.right_top()]);
                        (sq.right_top(), Vec2::new(sq.width(), h), 90.0)
                    }
                };
                for s in 0..=12 {
                    let a = (from + 90.0 * s as f32 / 12.0).to_radians();
                    points.push(center + Vec2::new(radius.x * a.cos(), radius.y * a.sin()));
                }
            }
            curves.push(points);
        }
    }
    let shadow = Stroke::new(pt(2.0), Color32::from_black_alpha(28));
    let line = Stroke::new(pt(1.0), LIGHT);
    for l in &lines {
        painter.line_segment(*l, shadow);
    }
    for c in &curves {
        painter.add(Shape::line(c.clone(), shadow));
    }
    for l in &lines {
        painter.line_segment(*l, line);
    }
    for c in curves {
        painter.add(Shape::line(c, line));
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
        let d = |x: f32, y: f32| Vec2::new(x, y);
        let r = dragged(drag(Some((1, 1))), d(-20.0, -20.0), Modifiers::NONE, None);
        assert_eq!(r, Rect::from_min_max(Pos2::ZERO, Pos2::new(80.0, 30.0)));
        // Shift keeps 2:1
        let r = dragged(drag(Some((1, 1))), d(-20.0, -30.0), Modifiers::SHIFT, None);
        assert_eq!(r, Rect::from_min_max(Pos2::ZERO, Pos2::new(80.0, 40.0)));
        // Alt resizes around the center
        let r = dragged(drag(Some((1, 0))), d(-10.0, 0.0), Modifiers::ALT, None);
        assert_eq!(
            r,
            Rect::from_min_max(Pos2::new(10.0, 0.0), Pos2::new(90.0, 50.0))
        );
        let r = dragged(drag(None), d(10.0, 5.0), Modifiers::NONE, None);
        assert_eq!(r.min, Pos2::new(10.0, 5.0));
        // A 1:1 ratio: a side drag keeps a square about the middle
        let r = dragged(
            drag(Some((1, 0))),
            d(-50.0, 0.0),
            Modifiers::NONE,
            Some(1.0),
        );
        assert_eq!(
            r,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(50.0, 50.0))
        );
    }

    #[test]
    fn presets_ratios_and_sizes() {
        let mut o = CropOptions::default();
        assert_eq!(o.preset, CropPreset::SizeResolution);
        assert_eq!(o.aspect((734, 811)), None);
        o.choose(PRESET_GROUPS[2][4]);
        assert_eq!((o.width.as_str(), o.height.as_str()), ("16", "9"));
        assert_eq!(o.aspect((734, 811)), Some(16.0 / 9.0));
        o.swap();
        assert_eq!(o.aspect((734, 811)), Some(9.0 / 16.0));
        o.choose(CropPreset::OriginalRatio);
        assert_eq!(o.aspect((734, 811)), Some(734.0 / 811.0));
        // 4 x 5 in at 300 ppi: 1200 x 1500 pixels
        o.choose(PRESET_GROUPS[3][1]);
        assert_eq!(o.width, "4 in");
        assert_eq!(o.output_size(), Some((1200.0, 1500.0)));
        assert_eq!(o.aspect((1, 1)), Some(0.8));
        // Typed pixels need no resolution
        o.choose(CropPreset::SizeResolution);
        o.width = "800".into();
        o.height = "600 px".into();
        assert_eq!(o.output_size(), Some((800.0, 600.0)));
        o.clear();
        assert_eq!(o.output_size(), None);
        assert_eq!(o.preset, CropPreset::SizeResolution);
    }
}
