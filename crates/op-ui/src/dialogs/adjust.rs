//! Dialogs for Image > Adjustments with numeric settings: Threshold...,
//! Posterize..., Levels..., Hue/Saturation... and Exposure....
//!
//! Each setting is a parameter with a range, a text field and (mostly) a
//! slider. Laid out after Photoshop's dialogs; sizes are in Photoshop points.
//! With Preview on, the document shows the result while the dialog is open.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, vec2};
use op_core::adjust::Adjustment;

use super::common;
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

/// One numeric setting.
struct Param {
    label: &'static str,
    min: f32,
    max: f32,
    default: f32,
    decimals: usize,
}

const fn param(label: &'static str, min: f32, max: f32, default: f32, decimals: usize) -> Param {
    Param {
        label,
        min,
        max,
        default,
        decimals,
    }
}

const THRESHOLD: &[Param] = &[param("Threshold Level:", 1.0, 255.0, 128.0, 0)];
const POSTERIZE: &[Param] = &[param("Levels:", 2.0, 255.0, 4.0, 0)];
/// Input black, gamma, input white, output black, output white.
const LEVELS: &[Param] = &[
    param("", 0.0, 253.0, 0.0, 0),
    param("", 0.01, 9.99, 1.0, 2),
    param("", 2.0, 255.0, 255.0, 0),
    param("", 0.0, 255.0, 0.0, 0),
    param("", 0.0, 255.0, 255.0, 0),
];
const HUE_SATURATION: &[Param] = &[
    param("Hue:", -180.0, 180.0, 0.0, 0),
    param("Saturation:", -100.0, 100.0, 0.0, 0),
    param("Lightness:", -100.0, 100.0, 0.0, 0),
];
const EXPOSURE: &[Param] = &[
    param("Exposure:", -20.0, 20.0, 0.0, 2),
    param("Offset:", -0.5, 0.5, 0.0, 4),
    param("Gamma Correction:", 0.01, 9.99, 1.0, 2),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Threshold,
    Posterize,
    Levels,
    HueSaturation,
    Exposure,
}

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Threshold => "Threshold",
            Self::Posterize => "Posterize",
            Self::Levels => "Levels",
            Self::HueSaturation => "Hue/Saturation",
            Self::Exposure => "Exposure",
        }
    }

    fn params(self) -> &'static [Param] {
        match self {
            Self::Threshold => THRESHOLD,
            Self::Posterize => POSTERIZE,
            Self::Levels => LEVELS,
            Self::HueSaturation => HUE_SATURATION,
            Self::Exposure => EXPOSURE,
        }
    }

    fn size(self) -> egui::Vec2 {
        match self {
            Self::Threshold => vec2(pt(400.0), pt(232.0)),
            Self::Posterize => vec2(pt(330.0), pt(132.0)),
            Self::Levels => vec2(pt(400.0), pt(330.0)),
            Self::HueSaturation | Self::Exposure => vec2(pt(400.0), pt(220.0)),
        }
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Apply(Adjustment),
}

pub struct AdjustDialog {
    pub kind: Kind,
    values: Vec<String>,
    pub preview: bool,
    /// Histogram of the pixels being adjusted (Threshold and Levels).
    histogram: [u64; 256],
    first_frame: bool,
    /// The adjustment the document currently previews, if any.
    pub previewing: Option<Adjustment>,
    /// The document before any preview, restored on Cancel.
    pub before: op_core::Snapshot,
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

    /// The adjustment as currently set, if every value is valid.
    pub fn adjustment(&self) -> Option<Adjustment> {
        let v: Vec<f32> = (0..self.values.len())
            .map(|i| self.value(i))
            .collect::<Option<_>>()?;
        Some(match self.kind {
            Kind::Threshold => Adjustment::Threshold(v[0] as u8),
            Kind::Posterize => Adjustment::Posterize(v[0] as u8),
            Kind::Levels => {
                // The black point must stay below the white point
                if v[0] + 2.0 > v[2] {
                    return None;
                }
                Adjustment::Levels {
                    input_black: v[0] as u8,
                    gamma: v[1],
                    input_white: v[2] as u8,
                    output_black: v[3] as u8,
                    output_white: v[4] as u8,
                }
            }
            Kind::HueSaturation => Adjustment::HueSaturation {
                hue: v[0] as i32,
                saturation: v[1] as i32,
                lightness: v[2] as i32,
            },
            Kind::Exposure => Adjustment::Exposure {
                exposure: v[0],
                offset: v[1],
                gamma: v[2],
            },
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("adjust-dialog"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(self.kind.size(), Sense::hover());
                outcome = self.ui(ui, rect);
            });
        self.first_frame = false;
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        common::frame(ui, frame, self.kind.title(), theme::semibold(pt(13.0)));
        match self.kind {
            Kind::Threshold => {
                self.labeled_field(ui, 0, at(LEFT, pt(52.0)));
                let hist = Rect::from_min_size(at(LEFT, pt(78.0)), vec2(pt(258.0), HISTOGRAM_H));
                self.histogram_ui(ui, hist);
                self.track(ui, hist, &[0], false);
            }
            Kind::Posterize => self.labeled_field(ui, 0, at(LEFT, pt(58.0))),
            Kind::Levels => self.levels_ui(ui, frame),
            Kind::HueSaturation | Kind::Exposure => {
                for i in 0..self.values.len() {
                    let y = pt(52.0) + pt(52.0) * i as f32;
                    self.slider_row(ui, i, at(LEFT, y), COLUMN - LEFT);
                }
            }
        }
        self.buttons(ui, frame)
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
        self.track(ui, line, &[i], false);
    }

    /// Triangle markers under `above` for the given parameters; dragging
    /// one sets its value. With `gamma_between`, the middle parameter is
    /// Levels' gamma, placed between the other two.
    fn track(&mut self, ui: &mut Ui, above: Rect, params: &[usize], gamma_between: bool) {
        let track = Rect::from_min_max(
            Pos2::new(above.left(), above.bottom() + pt(2.0)),
            Pos2::new(above.right(), above.bottom() + pt(2.0) + TRACK_H),
        );
        let id = ui.id().with(("adjust-track", params[0]));
        let response = ui.interact(track, id, Sense::click_and_drag());
        let frac = |p: &Param, v: f32| (v - p.min) / (p.max - p.min);

        // Marker positions as 0–1 along the track
        let positions: Vec<f32> = params
            .iter()
            .map(|&i| {
                let p = &self.kind.params()[i];
                let v = self.value(i).unwrap_or(p.default);
                if gamma_between && i == params[1] {
                    let lo = self.value(params[0]).unwrap_or(0.0) / 255.0;
                    let hi = self.value(params[2]).unwrap_or(255.0) / 255.0;
                    lo + (hi - lo) * 0.5f32.powf(v)
                } else if self.kind == Kind::Levels {
                    v / 255.0
                } else {
                    frac(p, v)
                }
            })
            .collect();

        if let Some(p) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            let t = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            // The marker being dragged: remembered from the press
            let grabbed = ui.memory_mut(|m| {
                let key = id.with("grabbed");
                if response.drag_started() || response.clicked() {
                    let nearest = positions
                        .iter()
                        .enumerate()
                        .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs()))
                        .map_or(0, |(k, _)| k);
                    m.data.insert_temp(key, nearest);
                }
                m.data.get_temp::<usize>(key).unwrap_or(0)
            });
            let i = params[grabbed];
            let p = &self.kind.params()[i];
            if gamma_between && grabbed == 1 {
                let lo = self.value(params[0]).unwrap_or(0.0) / 255.0;
                let hi = self.value(params[2]).unwrap_or(255.0) / 255.0;
                let u = ((t - lo) / (hi - lo)).clamp(0.001, 0.999);
                self.set(i, u.ln() / 0.5f32.ln());
            } else if self.kind == Kind::Levels {
                self.set(i, (t * 255.0).round());
            } else {
                let v = p.min + t * (p.max - p.min);
                let scale = 10f32.powi(p.decimals as i32);
                self.set(i, (v * scale).round() / scale);
            }
        }

        for (k, &pos) in positions.iter().enumerate() {
            let x = track.left() + pos.clamp(0.0, 1.0) * track.width();
            let fill = if params.len() == 1 {
                color::TEXT
            } else if gamma_between && k == 1 {
                Color32::from_gray(0x80)
            } else if k == 0 {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            ui.painter().add(Shape::convex_polygon(
                vec![
                    Pos2::new(x, track.top()),
                    Pos2::new(x + pt(6.0), track.bottom()),
                    Pos2::new(x - pt(6.0), track.bottom()),
                ],
                fill,
                Stroke::new(1.0, Color32::from_gray(0x9a)),
            ));
        }
    }

    fn levels_ui(&mut self, ui: &mut Ui, frame: Rect) {
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let width = pt(258.0);
        self.label(ui, "Channel:   RGB", at(LEFT, pt(48.0)));
        self.label(ui, "Input Levels:", at(LEFT, pt(76.0)));
        let hist = Rect::from_min_size(at(LEFT, pt(90.0)), vec2(width, HISTOGRAM_H));
        self.histogram_ui(ui, hist);
        self.track(ui, hist, &[0, 1, 2], true);
        let fields_y = hist.bottom() + pt(32.0);
        let center = LEFT + (width - FIELD_W) / 2.0;
        for (i, x) in [(0, LEFT), (1, center), (2, LEFT + width - FIELD_W)] {
            let r = Rect::from_min_size(
                Pos2::new(frame.left() + x, fields_y - FIELD_H / 2.0),
                vec2(FIELD_W, FIELD_H),
            );
            self.field(ui, i, r);
        }
        let out_y = fields_y + pt(30.0);
        self.label(ui, "Output Levels:", Pos2::new(frame.left() + LEFT, out_y));
        let ramp = Rect::from_min_size(
            Pos2::new(frame.left() + LEFT, out_y + pt(12.0)),
            vec2(width, pt(10.0)),
        );
        // Black-to-white ramp
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(ramp.left_top(), Color32::BLACK);
        mesh.colored_vertex(ramp.right_top(), Color32::WHITE);
        mesh.colored_vertex(ramp.right_bottom(), Color32::WHITE);
        mesh.colored_vertex(ramp.left_bottom(), Color32::BLACK);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        ui.painter().add(Shape::mesh(mesh));
        self.track(ui, ramp, &[3, 4], false);
        let out_fields_y = ramp.bottom() + pt(30.0);
        for (i, x) in [(3, LEFT), (4, LEFT + width - FIELD_W)] {
            let r = Rect::from_min_size(
                Pos2::new(frame.left() + x, out_fields_y - FIELD_H / 2.0),
                vec2(FIELD_W, FIELD_H),
            );
            self.field(ui, i, r);
        }
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
        let valid = self.adjustment().is_some();
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
            && let Some(adjustment) = self.adjustment()
        {
            return Outcome::Apply(adjustment);
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog(kind: Kind) -> AdjustDialog {
        let doc = op_core::Document::new_with_background("t", 1, 1, op_core::Color::WHITE);
        AdjustDialog::new(kind, [0; 256], doc.snapshot())
    }

    #[test]
    fn defaults_match_photoshop() {
        assert_eq!(
            dialog(Kind::Levels).adjustment(),
            Some(Adjustment::Levels {
                input_black: 0,
                input_white: 255,
                gamma: 1.0,
                output_black: 0,
                output_white: 255,
            })
        );
        assert_eq!(dialog(Kind::Exposure).values, ["0.00", "0.0000", "1.00"]);
        assert_eq!(
            dialog(Kind::HueSaturation).adjustment(),
            Some(Adjustment::HueSaturation {
                hue: 0,
                saturation: 0,
                lightness: 0
            })
        );
    }

    #[test]
    fn invalid_values_disable_the_dialog() {
        let mut d = dialog(Kind::Levels);
        d.values[0] = "250".into();
        d.values[2] = "251".into();
        assert_eq!(d.adjustment(), None);
        let mut d = dialog(Kind::HueSaturation);
        d.values[0] = "181".into();
        assert_eq!(d.adjustment(), None);
    }
}
