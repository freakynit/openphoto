//! Image > Canvas Size dialog.
//!
//! Layout is measured from Photoshop's dialog in reference-screenshot units
//! (scaled by `UI_SCALE` like the rest of the UI). All positions are relative
//! to the dialog's top-left corner.

use egui::{
    Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use op_core::{Anchor, Color, Document};

use super::common;
use crate::theme::{self, color};

/// Photoshop's maximum canvas dimension for regular documents.
pub const MAX_DIMENSION: u32 = 30_000;

const SIZE: Vec2 = vec2(681.0, 553.0);
const TITLE_BAR: f32 = 42.0;
const PAD: f32 = 29.0;
/// Right edge of the label column ("Width", "Height", "Anchor").
const LABEL_RIGHT: f32 = 81.0;
const FIELD_X: f32 = 95.0;
const FIELD_W: f32 = 119.0;
const UNIT_X: f32 = 228.0;
const UNIT_W: f32 = 237.0;
const CONTROL_H: f32 = 35.0;
/// Right edge of the left column (section rules, extension swatch).
const COLUMN_RIGHT: f32 = 529.0;
const BUTTON_X: f32 = 548.0;
const BUTTON_W: f32 = 103.0;
const ANCHOR_CELL: f32 = 34.4;
const FONT: f32 = 15.5;

const RULE: Color32 = Color32::from_gray(0x73);
const ANCHOR_LINE: Color32 = Color32::from_gray(0x78);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Pixels,
    Percent,
    Inches,
    Centimeters,
    Millimeters,
    Points,
    Picas,
}

impl Unit {
    const ALL: [Self; 7] = [
        Self::Pixels,
        Self::Percent,
        Self::Inches,
        Self::Centimeters,
        Self::Millimeters,
        Self::Points,
        Self::Picas,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Pixels => "Pixels",
            Self::Percent => "Percent",
            Self::Inches => "Inches",
            Self::Centimeters => "Centimeters",
            Self::Millimeters => "Millimeters",
            Self::Points => "Points",
            Self::Picas => "Picas",
        }
    }

    /// Pixels per unit. Percent is relative to the original dimension.
    fn pixels_per_unit(self, original: u32, resolution: f64) -> f64 {
        match self {
            Self::Pixels => 1.0,
            Self::Percent => original as f64 / 100.0,
            Self::Inches => resolution,
            Self::Centimeters => resolution / 2.54,
            Self::Millimeters => resolution / 25.4,
            Self::Points => resolution / 72.0,
            Self::Picas => resolution / 6.0,
        }
    }

    fn format(self, value: f64) -> String {
        let decimals = match self {
            Self::Pixels => 0,
            Self::Percent => 2,
            _ => 3,
        };
        let s = format!("{value:.decimals$}");
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    }
}

/// What fills the area added around the image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extension {
    Foreground,
    Background,
    White,
    Black,
    Gray,
    Other,
}

impl Extension {
    const ALL: [Self; 6] = [
        Self::Foreground,
        Self::Background,
        Self::White,
        Self::Black,
        Self::Gray,
        Self::Other,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Foreground => "Foreground",
            Self::Background => "Background",
            Self::White => "White",
            Self::Black => "Black",
            Self::Gray => "Gray",
            Self::Other => "Other...",
        }
    }
}

/// One dimension field (width or height) with its unit.
struct Dimension {
    text: String,
    unit: Unit,
    original: u32,
}

impl Dimension {
    fn new(original: u32) -> Self {
        Self {
            text: original.to_string(),
            unit: Unit::Pixels,
            original,
        }
    }

    /// The typed value in pixels (a delta when `relative`), if it parses.
    fn typed_pixels(&self, resolution: f64) -> Option<f64> {
        let v: f64 = self.text.trim().parse().ok()?;
        Some(v * self.unit.pixels_per_unit(self.original, resolution))
    }

    /// The resulting dimension in pixels, if valid.
    fn pixels(&self, relative: bool, resolution: f64) -> Option<u32> {
        let typed = self.typed_pixels(resolution)?;
        let px = if relative {
            self.original as f64 + typed
        } else {
            typed
        }
        .round();
        (px >= 1.0 && px <= MAX_DIMENSION as f64).then_some(px as u32)
    }

    fn show_pixels(&mut self, px: f64, resolution: f64) {
        let per_unit = self.unit.pixels_per_unit(self.original, resolution);
        self.text = self.unit.format(px / per_unit);
    }

    /// Changes the unit, keeping the value the field represents.
    fn set_unit(&mut self, unit: Unit, resolution: f64) {
        let px = self.typed_pixels(resolution);
        self.unit = unit;
        if let Some(px) = px {
            self.show_pixels(px, resolution);
        }
    }

    /// Switches between absolute and relative entry, keeping the result.
    fn set_relative(&mut self, relative: bool, resolution: f64) {
        let Some(typed) = self.typed_pixels(resolution) else {
            return;
        };
        let original = self.original as f64;
        let px = if relative {
            typed - original
        } else {
            original + typed
        };
        self.show_pixels(px, resolution);
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Apply {
        width: u32,
        height: u32,
        anchor: Anchor,
        fill: Color,
    },
}

pub struct CanvasSizeDialog {
    width: Dimension,
    height: Dimension,
    resolution: f64,
    /// Bytes per pixel used for the "Current Size: 1.70M" readout.
    bytes_per_pixel: u64,
    relative: bool,
    anchor: Anchor,
    extension: Extension,
    other: Color,
    /// Set when "Other..." or the swatch asks for the Color Picker.
    wants_color_picker: bool,
    /// The extension color only applies to a background layer.
    has_background: bool,
    first_frame: bool,
}

impl CanvasSizeDialog {
    pub fn new(doc: &Document) -> Self {
        Self {
            width: Dimension::new(doc.width),
            height: Dimension::new(doc.height),
            resolution: doc.resolution as f64,
            // Photoshop counts the color channels only (RGB, 8 bits each)
            bytes_per_pixel: 3 * doc.bit_depth.bits() as u64 / 8,
            relative: false,
            anchor: Anchor::CENTER,
            extension: Extension::Background,
            other: Color::WHITE,
            wants_color_picker: false,
            has_background: doc.has_background(),
            first_frame: true,
        }
    }

    fn new_size(&self) -> Option<(u32, u32)> {
        Some((
            self.width.pixels(self.relative, self.resolution)?,
            self.height.pixels(self.relative, self.resolution)?,
        ))
    }

    fn extension_color(&self, foreground: Color, background: Color) -> Color {
        match self.extension {
            Extension::Foreground => foreground,
            Extension::Background => background,
            Extension::White => Color::WHITE,
            Extension::Black => Color::BLACK,
            Extension::Gray => Color::from_rgba8([128, 128, 128, 255]),
            Extension::Other => self.other,
        }
    }

    /// The Color Picker the dialog asked for, with the color to start from.
    /// Returns it once per request.
    pub fn take_color_picker_request(
        &mut self,
        foreground: Color,
        background: Color,
    ) -> Option<Color> {
        std::mem::take(&mut self.wants_color_picker)
            .then(|| self.extension_color(foreground, background))
    }

    /// The Color Picker was confirmed: the extension becomes "Other..." with
    /// that color. (Cancelling leaves the previous choice in place.)
    pub fn set_other_color(&mut self, color: Color) {
        self.extension = Extension::Other;
        self.other = color;
    }

    /// `active` is false while a dialog on top (the Color Picker) has the
    /// keyboard, so Enter and Esc go to that dialog only.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        foreground: Color,
        background: Color,
        active: bool,
    ) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("canvas-size"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(SIZE, Sense::hover());
                outcome = self.ui(ui, rect, foreground, background, active);
            });
        self.first_frame = false;

        if active && ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn ui(
        &mut self,
        ui: &mut Ui,
        frame: Rect,
        foreground: Color,
        background: Color,
        active: bool,
    ) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let font = FontId::proportional(FONT);
        let bold = theme::semibold(FONT);

        let painter = ui.painter().clone();
        common::frame(ui, frame, "Canvas Size", theme::semibold(17.0));

        let body = TITLE_BAR;
        let new_size = self.new_size();

        // Current size
        section(
            &painter,
            at(PAD, body + 43.0),
            &format!(
                "Current Size: {}",
                self.size_label(self.width.original, self.height.original)
            ),
            &bold,
        );
        for (label, value, y) in [
            ("Width", self.width.original, 78.0),
            ("Height", self.height.original, 109.0),
        ] {
            painter.text(
                at(LABEL_RIGHT, body + y),
                Align2::RIGHT_CENTER,
                label,
                font.clone(),
                color::TEXT,
            );
            painter.text(
                at(FIELD_X, body + y),
                Align2::LEFT_CENTER,
                format!("{value} px"),
                font.clone(),
                color::TEXT,
            );
        }

        // New size
        let new_label = match new_size {
            Some((w, h)) => self.size_label(w, h),
            None => "—".into(),
        };
        section(
            &painter,
            at(PAD, body + 151.0),
            &format!("New Size: {new_label}"),
            &bold,
        );

        let resolution = self.resolution;
        let first_frame = self.first_frame;
        for (i, (label, y)) in [("Width", 192.0), ("Height", 236.0)]
            .into_iter()
            .enumerate()
        {
            painter.text(
                at(LABEL_RIGHT, body + y),
                Align2::RIGHT_CENTER,
                label,
                font.clone(),
                color::TEXT,
            );
            let dim = if i == 0 {
                &mut self.width
            } else {
                &mut self.height
            };
            let field = Rect::from_min_size(
                at(FIELD_X, body + y - CONTROL_H / 2.0),
                vec2(FIELD_W, CONTROL_H),
            );
            common::number_field(
                ui,
                field,
                &mut dim.text,
                ("canvas-size-field", i),
                FONT,
                first_frame && i == 0,
            );

            let unit_rect = Rect::from_min_size(
                at(UNIT_X, body + y - CONTROL_H / 2.0),
                vec2(UNIT_W, CONTROL_H),
            );
            let mut unit = dim.unit;
            common::dropdown(
                ui,
                unit_rect,
                ("canvas-size-unit", i),
                unit.label(),
                FONT,
                true,
                |ui| {
                    for u in Unit::ALL {
                        ui.selectable_value(&mut unit, u, u.label());
                    }
                },
            );
            if unit != dim.unit {
                dim.set_unit(unit, resolution);
            }
        }

        // Relative checkbox
        let mut relative = self.relative;
        let check = Rect::from_min_size(at(FIELD_X, body + 289.0 - 12.0), vec2(300.0, 24.0));
        ui.put(
            check,
            egui::Checkbox::new(
                &mut relative,
                egui::RichText::new("Relative to current dimension").font(font.clone()),
            ),
        );
        if relative != self.relative {
            self.width.set_relative(relative, resolution);
            self.height.set_relative(relative, resolution);
            self.relative = relative;
        }

        // Anchor
        painter.text(
            at(LABEL_RIGHT, body + 337.0),
            Align2::RIGHT_CENTER,
            "Anchor",
            font.clone(),
            color::TEXT,
        );
        let grid = Rect::from_min_size(at(FIELD_X, body + 324.0), Vec2::splat(ANCHOR_CELL * 3.0));
        let growth = match new_size {
            Some((w, h)) => (
                (w as i64 - self.width.original as i64).signum(),
                (h as i64 - self.height.original as i64).signum(),
            ),
            None => (1, 1),
        };
        self.anchor_grid(ui, grid, growth);

        // Canvas extension color
        let y = body + 463.0;
        painter.text(
            at(PAD, y),
            Align2::LEFT_CENTER,
            "Canvas extension color",
            font.clone(),
            if self.has_background {
                color::TEXT
            } else {
                color::TEXT_DISABLED
            },
        );
        let ext_rect = Rect::from_min_size(at(208.0, y - CONTROL_H / 2.0), vec2(237.0, CONTROL_H));
        let mut extension = self.extension;
        common::dropdown(
            ui,
            ext_rect,
            "canvas-size-extension",
            extension.label(),
            FONT,
            self.has_background,
            |ui| {
                for (i, e) in Extension::ALL.into_iter().enumerate() {
                    if i == 5 {
                        ui.separator();
                    }
                    ui.selectable_value(&mut extension, e, e.label());
                }
            },
        );
        if extension == Extension::Other {
            // "Other..." opens the Color Picker; the choice only changes once
            // a color is confirmed there, as in Photoshop
            self.wants_color_picker = true;
        } else {
            self.extension = extension;
        }

        let swatch = Rect::from_min_max(
            at(459.0, y - CONTROL_H / 2.0),
            at(COLUMN_RIGHT, y + CONTROL_H / 2.0),
        );
        if self.has_background {
            let c = self.extension_color(foreground, background);
            let [r, g, b, _] = c.to_rgba8();
            painter.rect(
                swatch,
                5,
                Color32::from_rgb(r, g, b),
                Stroke::new(1.5, Color32::from_gray(0xc0)),
                StrokeKind::Inside,
            );
            if ui
                .interact(swatch, ui.id().with("canvas-size-swatch"), Sense::click())
                .clicked()
            {
                self.wants_color_picker = true;
            }
        } else {
            painter.rect_stroke(
                swatch,
                5,
                Stroke::new(1.0, color::SEPARATOR_LIGHT),
                StrokeKind::Inside,
            );
        }

        // Buttons
        let ok_rect = Rect::from_min_size(at(BUTTON_X, body + 29.0), vec2(BUTTON_W, CONTROL_H));
        let cancel_rect = Rect::from_min_size(at(BUTTON_X, body + 82.0), vec2(BUTTON_W, CONTROL_H));
        let ok = common::pill_button(
            ui,
            ok_rect,
            "OK",
            theme::semibold(FONT + 0.5),
            new_size.is_some(),
        );
        let cancel =
            common::pill_button(ui, cancel_rect, "Cancel", theme::semibold(FONT + 0.5), true);

        let enter = active && ui.input(|i| i.key_pressed(Key::Enter));
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        if (ok.clicked() || enter)
            && let Some((width, height)) = new_size
        {
            return Outcome::Apply {
                width,
                height,
                anchor: self.anchor,
                fill: self.extension_color(foreground, background),
            };
        }
        Outcome::Open
    }

    /// "1.70M" style size readout, as in Photoshop.
    fn size_label(&self, width: u32, height: u32) -> String {
        let bytes = width as f64 * height as f64 * self.bytes_per_pixel as f64;
        const K: f64 = 1024.0;
        if bytes >= K * K * K {
            format!("{:.2}G", bytes / (K * K * K))
        } else if bytes >= K * K {
            format!("{:.2}M", bytes / (K * K))
        } else {
            format!("{:.1}K", bytes / K)
        }
    }

    /// 3×3 anchor picker. Arrows point away from the anchor when the canvas
    /// grows along that axis and toward it when it shrinks.
    fn anchor_grid(&mut self, ui: &mut Ui, grid: Rect, growth: (i64, i64)) {
        let painter = ui.painter();
        painter.rect_stroke(grid, 0, Stroke::new(1.0, ANCHOR_LINE), StrokeKind::Inside);
        for i in 1..3 {
            let d = i as f32 * ANCHOR_CELL;
            painter.line_segment(
                [
                    pos2(grid.left() + d, grid.top()),
                    pos2(grid.left() + d, grid.bottom()),
                ],
                Stroke::new(1.0, ANCHOR_LINE),
            );
            painter.line_segment(
                [
                    pos2(grid.left(), grid.top() + d),
                    pos2(grid.right(), grid.top() + d),
                ],
                Stroke::new(1.0, ANCHOR_LINE),
            );
        }

        for cy in 0..3u8 {
            for cx in 0..3u8 {
                let cell = Rect::from_min_size(
                    grid.min + vec2(cx as f32 * ANCHOR_CELL, cy as f32 * ANCHOR_CELL),
                    Vec2::splat(ANCHOR_CELL),
                );
                let response = ui.interact(cell, ui.id().with(("anchor", cx, cy)), Sense::click());
                if response.clicked() {
                    self.anchor = Anchor { x: cx, y: cy };
                }
                if response.hovered() {
                    ui.painter().rect_filled(cell.shrink(1.0), 0, color::HOVER);
                }

                let dx = cx as i64 - self.anchor.x as i64;
                let dy = cy as i64 - self.anchor.y as i64;
                let c = cell.center();
                if dx == 0 && dy == 0 {
                    ui.painter().circle_filled(c, 5.0, color::TEXT);
                } else if dx.abs() <= 1 && dy.abs() <= 1 {
                    let sx = if growth.0 < 0 { -1 } else { 1 };
                    let sy = if growth.1 < 0 { -1 } else { 1 };
                    let dir = vec2((dx * sx) as f32, (dy * sy) as f32).normalized();
                    arrow(ui.painter(), c, dir);
                }
            }
        }
    }
}

fn section(painter: &egui::Painter, left_center: Pos2, text: &str, font: &FontId) {
    let r = painter.text(
        left_center,
        Align2::LEFT_CENTER,
        text,
        font.clone(),
        color::TEXT,
    );
    let x0 = r.right() + 14.0;
    let x1 = left_center.x - PAD + COLUMN_RIGHT;
    painter.line_segment(
        [pos2(x0, left_center.y), pos2(x1, left_center.y)],
        Stroke::new(1.0, RULE),
    );
}

/// An arrow centered on `c`, pointing along `dir`.
fn arrow(painter: &egui::Painter, c: Pos2, dir: Vec2) {
    let len = 16.0;
    let tip = c + dir * len / 2.0;
    let tail = c - dir * len / 2.0;
    let stroke = Stroke::new(2.0, color::TEXT);
    painter.line_segment([tail, tip - dir * 3.0], stroke);
    let side = vec2(-dir.y, dir.x);
    let head = 6.0;
    painter.add(egui::Shape::convex_polygon(
        vec![
            tip,
            tip - dir * head + side * head * 0.6,
            tip - dir * head - side * head * 0.6,
        ],
        color::TEXT,
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_round_trip() {
        let mut d = Dimension::new(734);
        d.set_unit(Unit::Inches, 96.0);
        assert_eq!(d.text, "7.646");
        d.set_unit(Unit::Percent, 96.0);
        assert_eq!(d.text, "100");
        d.set_unit(Unit::Pixels, 96.0);
        assert_eq!(d.text, "734");
        assert_eq!(d.pixels(false, 96.0), Some(734));
    }

    #[test]
    fn relative_entry() {
        let mut d = Dimension::new(734);
        d.set_relative(true, 96.0);
        assert_eq!(d.text, "0");
        d.text = "266".into();
        assert_eq!(d.pixels(true, 96.0), Some(1000));
        d.set_relative(false, 96.0);
        assert_eq!(d.text, "1000");
    }

    #[test]
    fn rejects_out_of_range() {
        let mut d = Dimension::new(10);
        d.text = "0".into();
        assert_eq!(d.pixels(false, 72.0), None);
        d.text = "abc".into();
        assert_eq!(d.pixels(false, 72.0), None);
        d.text = (MAX_DIMENSION + 1).to_string();
        assert_eq!(d.pixels(false, 72.0), None);
    }
}
