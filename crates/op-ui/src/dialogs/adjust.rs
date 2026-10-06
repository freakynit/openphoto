//! Dialogs for adjustments and filters with settings: Image > Adjustments >
//! Threshold..., Posterize..., Levels..., Hue/Saturation..., Exposure...,
//! and the Filter menu's Gaussian Blur..., Box Blur..., Unsharp Mask...,
//! Add Noise..., Median..., Minimum..., Maximum..., High Pass..., Offset...
//! and Mosaic....
//!
//! Each setting is a parameter: a number with a range, a text field and
//! (mostly) a slider; a choice shown as radio buttons; or a checkbox. Laid
//! out after Photoshop's dialogs; sizes are in Photoshop points. With
//! Preview on, the document shows the result while the dialog is open.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, vec2};
use op_core::adjust::Adjustment;
use op_core::filter::{Filter, OffsetFill};

use super::{brightness_contrast, color_balance, common, curves, hue_saturation, levels, uxp};
use crate::theme::{self, color, pt};

const FONT: f32 = pt(12.5);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));
const FIELD_H: f32 = pt(22.0);
const FIELD_W: f32 = pt(56.0);
const LEFT: f32 = pt(20.0);
/// Width of the controls column (left of the buttons).
const COLUMN: f32 = pt(280.0);
const HISTOGRAM_H: f32 = pt(100.0);
const TRACK_H: f32 = pt(12.0);

/// What a dialog applies: an adjustment or a filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Adjustment(Adjustment),
    Filter(Filter),
}

impl Effect {
    pub fn name(self) -> &'static str {
        match self {
            Self::Adjustment(a) => a.name(),
            Self::Filter(f) => f.name(),
        }
    }

    /// Applies to the active layer; `background` is the background color
    /// (used by Offset on the background layer).
    pub fn apply(
        self,
        doc: &mut op_core::Document,
        background: [u8; 3],
    ) -> Result<(), op_core::fill::FillError> {
        match self {
            Self::Adjustment(a) => op_core::adjust::apply(doc, a),
            Self::Filter(f) => op_core::filter::apply(doc, f, background),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ParamKind {
    Number,
    /// Radio buttons; the value is the chosen index.
    Choice(&'static [&'static str]),
    /// A checkbox; the value is 0 or 1.
    Check,
}

/// One setting.
struct Param {
    label: &'static str,
    min: f32,
    max: f32,
    default: f32,
    decimals: usize,
    kind: ParamKind,
}

const fn param(label: &'static str, min: f32, max: f32, default: f32, decimals: usize) -> Param {
    Param {
        label,
        min,
        max,
        default,
        decimals,
        kind: ParamKind::Number,
    }
}

const fn choice(label: &'static str, options: &'static [&'static str], default: usize) -> Param {
    Param {
        label,
        min: 0.0,
        max: (options.len() - 1) as f32,
        default: default as f32,
        decimals: 0,
        kind: ParamKind::Choice(options),
    }
}

const fn check(label: &'static str, default: bool) -> Param {
    Param {
        label,
        min: 0.0,
        max: 1.0,
        default: default as u8 as f32,
        decimals: 0,
        kind: ParamKind::Check,
    }
}

const THRESHOLD: &[Param] = &[param("Threshold Level:", 1.0, 255.0, 128.0, 0)];
const POSTERIZE: &[Param] = &[param("Levels:", 2.0, 255.0, 4.0, 0)];
const EXPOSURE: &[Param] = &[
    param("Exposure:", -20.0, 20.0, 0.0, 2),
    param("Offset:", -0.5, 0.5, 0.0, 4),
    param("Gamma Correction:", 0.01, 9.99, 1.0, 2),
];
/// Photoshop's Black & White default preset.
const BLACK_WHITE: &[Param] = &[
    param("Reds (%):", -200.0, 300.0, 40.0, 0),
    param("Yellows (%):", -200.0, 300.0, 60.0, 0),
    param("Greens (%):", -200.0, 300.0, 40.0, 0),
    param("Cyans (%):", -200.0, 300.0, 60.0, 0),
    param("Blues (%):", -200.0, 300.0, 20.0, 0),
    param("Magentas (%):", -200.0, 300.0, 80.0, 0),
];
const VIBRANCE: &[Param] = &[
    param("Vibrance:", -100.0, 100.0, 0.0, 0),
    param("Saturation:", -100.0, 100.0, 0.0, 0),
];
/// Photoshop's Photo Filter presets and their colors.
const PHOTO_FILTERS: [(&str, [u8; 3]); 20] = [
    ("Warming Filter (85)", [0xec, 0x8a, 0x00]),
    ("Warming Filter (LBA)", [0xfa, 0x96, 0x00]),
    ("Warming Filter (81)", [0xeb, 0xb1, 0x13]),
    ("Cooling Filter (80)", [0x00, 0x6d, 0xff]),
    ("Cooling Filter (LBB)", [0x00, 0x5d, 0xff]),
    ("Cooling Filter (82)", [0x00, 0xb5, 0xff]),
    ("Red", [0xea, 0x1a, 0x1a]),
    ("Orange", [0xf3, 0x84, 0x17]),
    ("Yellow", [0xf9, 0xe3, 0x1c]),
    ("Green", [0x19, 0xc9, 0x19]),
    ("Cyan", [0x1d, 0xcb, 0xea]),
    ("Blue", [0x1d, 0x35, 0xea]),
    ("Violet", [0x9b, 0x1d, 0xea]),
    ("Magenta", [0xe3, 0x18, 0xe3]),
    ("Sepia", [0xac, 0x7a, 0x33]),
    ("Deep Red", [0xff, 0x00, 0x00]),
    ("Deep Blue", [0x00, 0x22, 0xcd]),
    ("Deep Emerald", [0x00, 0x8c, 0x00]),
    ("Deep Yellow", [0xff, 0xd5, 0x00]),
    ("Underwater", [0x00, 0xc1, 0xb1]),
];
const PHOTO_FILTER_NAMES: [&str; 20] = {
    let mut names = [""; 20];
    let mut i = 0;
    while i < 20 {
        names[i] = PHOTO_FILTERS[i].0;
        i += 1;
    }
    names
};
const PHOTO_FILTER: &[Param] = &[
    choice("Filter:", &PHOTO_FILTER_NAMES, 0),
    param("Density (%):", 1.0, 100.0, 25.0, 0),
    check("Preserve Luminosity", true),
];
const GRADIENT_MAP: &[Param] = &[check("Reverse", false)];
const GAUSSIAN_BLUR: &[Param] = &[param("Radius (pixels):", 0.1, 1000.0, 1.0, 1)];
const BOX_BLUR: &[Param] = &[param("Radius (pixels):", 1.0, 2000.0, 1.0, 0)];
const UNSHARP_MASK: &[Param] = &[
    param("Amount (%):", 1.0, 500.0, 50.0, 0),
    param("Radius (pixels):", 0.1, 1000.0, 1.0, 1),
    param("Threshold (levels):", 0.0, 255.0, 0.0, 0),
];
const ADD_NOISE: &[Param] = &[
    param("Amount (%):", 0.1, 400.0, 12.5, 2),
    choice("Distribution", &["Uniform", "Gaussian"], 0),
    check("Monochromatic", false),
];
const RADIUS: &[Param] = &[param("Radius (pixels):", 1.0, 500.0, 1.0, 0)];
const HIGH_PASS: &[Param] = &[param("Radius (pixels):", 0.1, 1000.0, 10.0, 1)];
const OFFSET: &[Param] = &[
    param("Horizontal (pixels right):", -30000.0, 30000.0, 0.0, 0),
    param("Vertical (pixels down):", -30000.0, 30000.0, 0.0, 0),
    choice(
        "Undefined Areas",
        &["Set to Transparent", "Repeat Edge Pixels", "Wrap Around"],
        0,
    ),
];
const MOSAIC: &[Param] = &[param("Cell Size (square):", 2.0, 200.0, 10.0, 0)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Threshold,
    Posterize,
    Levels,
    Curves,
    HueSaturation,
    Exposure,
    BrightnessContrast,
    ColorBalance,
    BlackWhite,
    Vibrance,
    PhotoFilter,
    GradientMap,
    GaussianBlur,
    BoxBlur,
    UnsharpMask,
    AddNoise,
    Median,
    Minimum,
    Maximum,
    HighPass,
    Offset,
    Mosaic,
}

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Threshold => "Threshold",
            Self::Posterize => "Posterize",
            Self::Levels => "Levels",
            Self::Curves => "Curves",
            Self::HueSaturation => "Hue/Saturation",
            Self::Exposure => "Exposure",
            Self::BrightnessContrast => "Brightness/Contrast",
            Self::ColorBalance => "Color Balance",
            Self::BlackWhite => "Black and White",
            Self::Vibrance => "Vibrance",
            Self::PhotoFilter => "Photo Filter",
            Self::GradientMap => "Gradient Map",
            Self::GaussianBlur => "Gaussian Blur",
            Self::BoxBlur => "Box Blur",
            Self::UnsharpMask => "Unsharp Mask",
            Self::AddNoise => "Add Noise",
            Self::Median => "Median",
            Self::Minimum => "Minimum",
            Self::Maximum => "Maximum",
            Self::HighPass => "High Pass",
            Self::Offset => "Offset",
            Self::Mosaic => "Mosaic",
        }
    }

    fn params(self) -> &'static [Param] {
        match self {
            Self::Threshold => THRESHOLD,
            Self::Posterize => POSTERIZE,
            // Their own dialogs keep their settings
            Self::Levels
            | Self::Curves
            | Self::HueSaturation
            | Self::BrightnessContrast
            | Self::ColorBalance => &[],
            Self::Exposure => EXPOSURE,
            Self::BlackWhite => BLACK_WHITE,
            Self::Vibrance => VIBRANCE,
            Self::PhotoFilter => PHOTO_FILTER,
            Self::GradientMap => GRADIENT_MAP,
            Self::GaussianBlur => GAUSSIAN_BLUR,
            Self::BoxBlur => BOX_BLUR,
            Self::UnsharpMask => UNSHARP_MASK,
            Self::AddNoise => ADD_NOISE,
            Self::Median | Self::Minimum | Self::Maximum => RADIUS,
            Self::HighPass => HIGH_PASS,
            Self::Offset => OFFSET,
            Self::Mosaic => MOSAIC,
        }
    }

    fn size(self) -> egui::Vec2 {
        match self {
            Self::Threshold => vec2(pt(400.0), pt(232.0)),
            Self::Posterize => vec2(pt(330.0), pt(132.0)),
            _ => {
                let rows: f32 = self.params().iter().map(row_height).sum();
                vec2(pt(400.0), (pt(36.0) + rows + pt(20.0)).max(pt(150.0)))
            }
        }
    }
}

/// Height of a parameter's row in the generic layout.
fn row_height(p: &Param) -> f32 {
    match p.kind {
        ParamKind::Number => pt(52.0),
        // Long lists are a dropdown, short ones radio buttons
        ParamKind::Choice(options) if options.len() > 4 => pt(36.0),
        ParamKind::Choice(options) => pt(24.0) * (options.len() + 1) as f32,
        ParamKind::Check => pt(28.0),
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Apply(Effect),
}

pub struct AdjustDialog {
    pub kind: Kind,
    values: Vec<String>,
    pub preview: bool,
    /// Histogram of the pixels being adjusted (Threshold and Levels).
    histogram: [u64; 256],
    first_frame: bool,
    /// The effect the document currently previews, if any.
    pub previewing: Option<Effect>,
    /// The document before any preview, restored on Cancel.
    pub before: op_core::Snapshot,
    /// Gradient Map's colors (the foreground and background colors).
    pub colors: ([u8; 3], [u8; 3]),
    /// The dialogs rebuilt after Photoshop 2026, which keep their own
    /// settings.
    custom: Option<Custom>,
}

/// A dialog with its own layout and settings.
enum Custom {
    BrightnessContrast(brightness_contrast::Dialog),
    ColorBalance(color_balance::Dialog),
    HueSaturation(Box<hue_saturation::Dialog>),
    Levels(Box<levels::Dialog>),
    Curves(Box<curves::Dialog>),
}

fn format(v: f32, decimals: usize) -> String {
    format!("{v:.decimals$}")
}

impl AdjustDialog {
    pub fn new(kind: Kind, histogram: [u64; 256], before: op_core::Snapshot) -> Self {
        Self {
            kind,
            values: kind
                .params()
                .iter()
                .map(|p| format(p.default, p.decimals))
                .collect(),
            preview: true,
            histogram,
            first_frame: true,
            previewing: None,
            before,
            colors: ([0; 3], [255; 3]),
            custom: match kind {
                Kind::BrightnessContrast => Some(Custom::BrightnessContrast(Default::default())),
                Kind::ColorBalance => Some(Custom::ColorBalance(Default::default())),
                Kind::HueSaturation => Some(Custom::HueSaturation(Box::new(
                    hue_saturation::Dialog::new(0),
                ))),
                Kind::Levels => Some(Custom::Levels(Box::new(levels::Dialog::new([[0; 256]; 3])))),
                Kind::Curves => Some(Custom::Curves(Box::new(curves::Dialog::new([[0; 256]; 3])))),
                _ => None,
            },
        }
    }

    /// Levels' (and Curves') per-channel histograms.
    pub fn set_channel_histograms(&mut self, histograms: [[u64; 256]; 3]) {
        match &mut self.custom {
            Some(Custom::Levels(d)) => **d = levels::Dialog::new(histograms),
            Some(Custom::Curves(d)) => **d = curves::Dialog::new(histograms),
            _ => {}
        }
    }

    /// Hue/Saturation's Colorize starts from this hue.
    pub fn set_colorize_hue(&mut self, hue: i32) {
        if let Some(Custom::HueSaturation(d)) = &mut self.custom {
            d.colorize_values[0] = hue.to_string();
        }
    }

    fn value(&self, i: usize) -> Option<f32> {
        let p = &self.kind.params()[i];
        let v: f32 = self.values[i].trim().parse().ok()?;
        (p.min..=p.max).contains(&v).then_some(v)
    }

    fn set(&mut self, i: usize, v: f32) {
        let p = &self.kind.params()[i];
        self.values[i] = format(v.clamp(p.min, p.max), p.decimals);
    }

    /// The effect as currently set, if every value is valid.
    pub fn effect(&self) -> Option<Effect> {
        match &self.custom {
            Some(Custom::BrightnessContrast(d)) => return d.adjustment().map(Effect::Adjustment),
            Some(Custom::ColorBalance(d)) => return d.adjustment().map(Effect::Adjustment),
            Some(Custom::HueSaturation(d)) => return d.adjustment().map(Effect::Adjustment),
            Some(Custom::Levels(d)) => return d.adjustment().map(Effect::Adjustment),
            Some(Custom::Curves(d)) => return Some(Effect::Adjustment(d.adjustment())),
            None => {}
        }
        let v: Vec<f32> = (0..self.values.len())
            .map(|i| self.value(i))
            .collect::<Option<_>>()?;
        let filter = match self.kind {
            Kind::GaussianBlur => Filter::GaussianBlur { radius: v[0] },
            Kind::BoxBlur => Filter::BoxBlur {
                radius: v[0] as u32,
            },
            Kind::UnsharpMask => Filter::UnsharpMask {
                amount: v[0],
                radius: v[1],
                threshold: v[2] as u8,
            },
            Kind::AddNoise => Filter::AddNoise {
                amount: v[0],
                gaussian: v[1] == 1.0,
                monochromatic: v[2] == 1.0,
            },
            Kind::Median => Filter::Median {
                radius: v[0] as u32,
            },
            Kind::Minimum => Filter::Minimum {
                radius: v[0] as u32,
            },
            Kind::Maximum => Filter::Maximum {
                radius: v[0] as u32,
            },
            Kind::HighPass => Filter::HighPass { radius: v[0] },
            Kind::Offset => Filter::Offset {
                dx: v[0] as i32,
                dy: v[1] as i32,
                fill: [
                    OffsetFill::Background,
                    OffsetFill::RepeatEdges,
                    OffsetFill::Wrap,
                ][v[2] as usize],
            },
            Kind::Mosaic => Filter::Mosaic { cell: v[0] as u32 },
            _ => return self.adjustment(&v).map(Effect::Adjustment),
        };
        Some(Effect::Filter(filter))
    }

    fn adjustment(&self, v: &[f32]) -> Option<Adjustment> {
        Some(match self.kind {
            Kind::Threshold => Adjustment::Threshold(v[0] as u8),
            Kind::Posterize => Adjustment::Posterize(v[0] as u8),
            Kind::Exposure => Adjustment::Exposure {
                exposure: v[0],
                offset: v[1],
                gamma: v[2],
            },
            Kind::BlackWhite => Adjustment::BlackWhite {
                weights: [0, 1, 2, 3, 4, 5].map(|i| v[i] as i32),
            },
            Kind::Vibrance => Adjustment::Vibrance {
                vibrance: v[0] as i32,
                saturation: v[1] as i32,
            },
            Kind::PhotoFilter => Adjustment::PhotoFilter {
                color: PHOTO_FILTERS[(v[0] as usize).min(PHOTO_FILTERS.len() - 1)].1,
                density: v[1] as u8,
                preserve_luminosity: v[2] == 1.0,
            },
            Kind::GradientMap => {
                let (a, b) = self.colors;
                let (from, to) = if v[0] == 1.0 { (b, a) } else { (a, b) };
                Adjustment::GradientMap { from, to }
            }
            _ => return None,
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("adjust-dialog"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let size = match self.custom {
                    Some(Custom::BrightnessContrast(_)) => brightness_contrast::SIZE,
                    Some(Custom::ColorBalance(_)) => color_balance::SIZE,
                    Some(Custom::HueSaturation(_)) => hue_saturation::SIZE,
                    Some(Custom::Levels(_)) => levels::SIZE,
                    Some(Custom::Curves(_)) => curves::SIZE,
                    None => self.kind.size(),
                };
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                outcome = if self.custom.is_some() {
                    self.custom_ui(ui, rect)
                } else {
                    self.ui(ui, rect)
                };
            });
        self.first_frame = false;
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    /// The dialogs rebuilt after Photoshop 2026: their own layout, with the
    /// AppKit title bar.
    fn custom_ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        common::frame(ui, frame, self.kind.title(), theme::dialog_bold(pt(13.0)));
        let button = match self.custom.as_mut() {
            Some(Custom::BrightnessContrast(d)) => {
                let pressed = d.ui(ui, frame, self.first_frame, &mut self.preview);
                if pressed == Some(uxp::Button::Third) {
                    d.auto(auto_brightness_contrast(&self.histogram));
                }
                pressed
            }
            Some(Custom::ColorBalance(d)) => d.ui(ui, frame, self.first_frame, &mut self.preview),
            Some(Custom::HueSaturation(d)) => d.ui(ui, frame, self.first_frame, &mut self.preview),
            Some(Custom::Levels(d)) => d.ui(ui, frame, self.first_frame, &mut self.preview),
            Some(Custom::Curves(d)) => d.ui(ui, frame, &mut self.preview),
            None => None,
        };
        match button {
            Some(uxp::Button::Ok) => self.effect().map_or(Outcome::Open, Outcome::Apply),
            Some(uxp::Button::Cancel) => Outcome::Cancel,
            _ => Outcome::Open,
        }
    }

    fn ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        common::frame(ui, frame, self.kind.title(), theme::semibold(pt(13.0)));
        match self.kind {
            Kind::Threshold => {
                self.labeled_field(ui, 0, at(LEFT, pt(52.0)));
                let hist = Rect::from_min_size(at(LEFT, pt(78.0)), vec2(pt(258.0), HISTOGRAM_H));
                self.histogram_ui(ui, hist);
                self.track(ui, hist, 0);
            }
            Kind::Posterize => self.labeled_field(ui, 0, at(LEFT, pt(58.0))),
            _ => {
                let mut y = pt(52.0);
                for (i, p) in self.kind.params().iter().enumerate() {
                    match p.kind {
                        ParamKind::Number => self.slider_row(ui, i, at(LEFT, y), COLUMN - LEFT),
                        ParamKind::Choice(options) => {
                            self.choice_row(ui, i, options, at(LEFT, y));
                        }
                        ParamKind::Check => self.check_row(ui, i, at(LEFT, y)),
                    }
                    y += row_height(p);
                }
            }
        }
        self.buttons(ui, frame)
    }

    /// A label with radio buttons under it.
    fn choice_row(&mut self, ui: &mut Ui, i: usize, options: &[&str], left_center: Pos2) {
        let label = self.label(ui, self.kind.params()[i].label, left_center);
        let mut chosen = self.value(i).unwrap_or(0.0) as usize;
        if options.len() > 4 {
            let rect = Rect::from_min_size(
                Pos2::new(label.right() + pt(8.0), left_center.y - FIELD_H / 2.0),
                vec2(pt(200.0), FIELD_H),
            );
            common::dropdown(
                ui,
                rect,
                ("adjust-choice", i),
                options[chosen.min(options.len() - 1)],
                FONT,
                true,
                |ui| {
                    for (k, option) in options.iter().enumerate() {
                        ui.selectable_value(&mut chosen, k, *option);
                    }
                },
            );
            self.values[i] = chosen.to_string();
            return;
        }
        for (k, option) in options.iter().enumerate() {
            let center = left_center + vec2(pt(12.0), pt(24.0) * (k + 1) as f32);
            let rect = Rect::from_min_size(center - vec2(0.0, pt(9.0)), vec2(pt(220.0), pt(18.0)));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
            child.radio_value(
                &mut chosen,
                k,
                egui::RichText::new(*option).font(FontId::proportional(FONT)),
            );
        }
        self.values[i] = chosen.to_string();
    }

    fn check_row(&mut self, ui: &mut Ui, i: usize, left_center: Pos2) {
        let mut on = self.value(i) == Some(1.0);
        let rect = Rect::from_min_size(left_center - vec2(0.0, pt(9.0)), vec2(pt(220.0), pt(18.0)));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.checkbox(
            &mut on,
            egui::RichText::new(self.kind.params()[i].label).font(FontId::proportional(FONT)),
        );
        self.values[i] = (on as u8).to_string();
    }

    fn label(&self, ui: &Ui, text: &str, left_center: Pos2) -> Rect {
        ui.painter().text(
            left_center,
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(FONT),
            color::TEXT,
        )
    }

    fn field(&mut self, ui: &mut Ui, i: usize, rect: Rect) {
        // The first field takes focus with its text selected when the dialog opens
        let select = self.first_frame && i == 0;
        common::number_field(
            ui,
            rect,
            &mut self.values[i],
            ("adjust-value", i),
            FONT,
            select,
        );
    }

    /// "Label: [field]" starting at `left_center`.
    fn labeled_field(&mut self, ui: &mut Ui, i: usize, left_center: Pos2) {
        let label = self.label(ui, self.kind.params()[i].label, left_center);
        let field = Rect::from_min_size(
            Pos2::new(label.right() + pt(8.0), left_center.y - FIELD_H / 2.0),
            vec2(FIELD_W, FIELD_H),
        );
        self.field(ui, i, field);
    }

    /// Label on the left, field on the right, slider underneath.
    fn slider_row(&mut self, ui: &mut Ui, i: usize, left_center: Pos2, width: f32) {
        self.label(ui, self.kind.params()[i].label, left_center);
        let field = Rect::from_min_size(
            Pos2::new(
                left_center.x + width - FIELD_W,
                left_center.y - FIELD_H / 2.0,
            ),
            vec2(FIELD_W, FIELD_H),
        );
        self.field(ui, i, field);
        let line = Rect::from_min_size(
            Pos2::new(left_center.x, left_center.y + pt(14.0)),
            vec2(width, pt(2.0)),
        );
        ui.painter().rect_filled(line, 0, Color32::from_gray(0x3a));
        self.track(ui, line, i);
    }

    /// A triangle marker under `above` for parameter `i`; pressing or
    /// dragging along the track sets its value in proportion.
    fn track(&mut self, ui: &mut Ui, above: Rect, i: usize) {
        let track = Rect::from_min_max(
            Pos2::new(above.left(), above.bottom() + pt(2.0)),
            Pos2::new(above.right(), above.bottom() + pt(2.0) + TRACK_H),
        );
        let response = ui.interact(
            track,
            ui.id().with(("adjust-track", i)),
            Sense::click_and_drag(),
        );
        let p = &self.kind.params()[i];
        if let Some(pointer) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            let t = ((pointer.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let v = p.min + t * (p.max - p.min);
            let scale = 10f32.powi(p.decimals as i32);
            self.set(i, (v * scale).round() / scale);
        }
        let p = &self.kind.params()[i];
        let v = self.value(i).unwrap_or(p.default);
        let x = track.left() + ((v - p.min) / (p.max - p.min)).clamp(0.0, 1.0) * track.width();
        ui.painter().add(Shape::convex_polygon(
            vec![
                Pos2::new(x, track.top()),
                Pos2::new(x + pt(6.0), track.bottom()),
                Pos2::new(x - pt(6.0), track.bottom()),
            ],
            color::TEXT,
            Stroke::new(1.0, Color32::from_gray(0x9a)),
        ));
    }

    fn histogram_ui(&self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect(
            rect,
            0,
            Color32::from_gray(0x3c),
            Stroke::new(1.0, Color32::from_gray(0x2c)),
            StrokeKind::Outside,
        );
        let max = self.histogram.iter().copied().max().unwrap_or(0).max(1) as f32;
        let bar = rect.width() / 256.0;
        for (i, &count) in self.histogram.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let h = count as f32 / max * rect.height();
            let x = rect.left() + (i as f32 + 0.5) * bar;
            painter.line_segment(
                [Pos2::new(x, rect.bottom()), Pos2::new(x, rect.bottom() - h)],
                Stroke::new(bar.max(1.0), color::TEXT),
            );
        }
    }

    fn buttons(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let x = frame.width() - pt(108.0);
        let at = |y: f32| frame.min + vec2(x, y);
        let button_font = FontId::proportional(pt(13.0));
        let valid = self.effect().is_some();
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            valid,
        );
        let cancel = common::pill_button(
            ui,
            Rect::from_min_size(at(pt(76.0)), BUTTON),
            "Cancel",
            button_font,
            true,
        );
        let check = Rect::from_min_size(at(pt(112.0) - pt(9.0)), vec2(pt(96.0), pt(18.0)));
        let mut check_ui = ui.new_child(egui::UiBuilder::new().max_rect(check));
        check_ui.checkbox(
            &mut self.preview,
            egui::RichText::new("Preview").font(FontId::proportional(FONT)),
        );

        if cancel.clicked() {
            return Outcome::Cancel;
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if (ok.clicked() || enter)
            && let Some(effect) = self.effect()
        {
            return Outcome::Apply(effect);
        }
        Outcome::Open
    }
}

/// Brightness/Contrast's Auto: the brightness and contrast whose curve
/// comes closest (over the image's histogram) to Auto Contrast's stretch,
/// the 0.1% darkest and lightest values clipped. Photoshop's own choice
/// is not reproduced exactly.
fn auto_brightness_contrast(histogram: &[u64; 256]) -> (i32, i32) {
    let total: u64 = histogram.iter().sum();
    if total == 0 {
        return (0, 0);
    }
    let clip = total / 1000;
    let mut acc = 0;
    let lo = (0..256).find(|&i| {
        acc += histogram[i];
        acc > clip
    });
    acc = 0;
    let hi = (0..256).rev().find(|&i| {
        acc += histogram[i];
        acc > clip
    });
    let (lo, hi) = (lo.unwrap_or(0) as f32, hi.unwrap_or(255) as f32);
    if hi <= lo {
        return (0, 0);
    }
    let target = |v: usize| ((v as f32 - lo) / (hi - lo) * 255.0).clamp(0.0, 255.0);
    let mut best = (f64::MAX, (0, 0));
    for b in (-150..=150).step_by(2) {
        for c in (-50..=100).step_by(2) {
            let adj = Adjustment::BrightnessContrast {
                brightness: b,
                contrast: c,
                legacy: false,
            };
            let table = adj.tables().expect("a tone curve")[0];
            let err: f64 = (0..256)
                .filter(|&v| histogram[v] > 0)
                .map(|v| {
                    let d = table[v] as f32 - target(v);
                    histogram[v] as f64 * (d * d) as f64
                })
                .sum();
            if err < best.0 {
                best = (err, (b, c));
            }
        }
    }
    best.1
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_core::adjust::{HueSaturation, Levels};

    fn dialog(kind: Kind) -> AdjustDialog {
        let doc = op_core::Document::new_with_background("t", 1, 1, op_core::Color::WHITE);
        AdjustDialog::new(kind, [0; 256], doc.snapshot())
    }

    #[test]
    fn defaults_match_photoshop() {
        assert_eq!(
            dialog(Kind::Levels).effect(),
            Some(Effect::Adjustment(Adjustment::Levels(
                Levels::IDENTITY.composite()
            )))
        );
        assert_eq!(dialog(Kind::Exposure).values, ["0.00", "0.0000", "1.00"]);
        assert_eq!(
            dialog(Kind::HueSaturation).effect(),
            Some(Effect::Adjustment(Adjustment::HueSaturation(
                HueSaturation::master(0, 0, 0)
            )))
        );
    }

    #[test]
    fn filters_read_choices_and_checkboxes() {
        let mut d = dialog(Kind::AddNoise);
        d.values[1] = "1".into();
        d.values[2] = "1".into();
        assert_eq!(
            d.effect(),
            Some(Effect::Filter(Filter::AddNoise {
                amount: 12.5,
                gaussian: true,
                monochromatic: true
            }))
        );
        let mut d = dialog(Kind::Offset);
        d.values[2] = "2".into();
        assert_eq!(
            d.effect(),
            Some(Effect::Filter(Filter::Offset {
                dx: 0,
                dy: 0,
                fill: OffsetFill::Wrap
            }))
        );
    }
}
