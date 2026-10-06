//! Image > Trim.
//!
//! Laid out after Photoshop's Trim dialog: "Based On" with three choices,
//! "Trim Away" with a checkbox per side, OK and Cancel on the right. Sizes
//! are in Photoshop points.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, Ui, vec2};
use op_core::image_ops::{TrimBasis, TrimSides};

use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(380.0), pt(236.0));
const FONT: f32 = pt(12.5);
const LEFT: f32 = pt(20.0);
const RULE_RIGHT: f32 = pt(256.0);
const BUTTON_X: f32 = pt(276.0);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));

pub enum Outcome {
    Open,
    Cancel,
    Apply { basis: TrimBasis, sides: TrimSides },
}

pub struct TrimDialog {
    basis: TrimBasis,
    sides: TrimSides,
    /// "Transparent Pixels" needs a document without a background layer.
    can_trim_transparent: bool,
}

impl TrimDialog {
    pub fn new(has_background: bool) -> Self {
        Self {
            basis: if has_background {
                TrimBasis::TopLeftColor
            } else {
                TrimBasis::Transparent
            },
            sides: TrimSides::default(),
            can_trim_transparent: !has_background,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("trim-dialog"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(SIZE, Sense::hover());
                outcome = self.ui(ui, rect);
            });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let font = FontId::proportional(FONT);
        common::frame(ui, frame, "Trim", theme::semibold(pt(13.0)));
        let painter = ui.painter().clone();
        let header = |text: &str, y: f32| {
            let r = painter.text(
                at(LEFT, y),
                Align2::LEFT_CENTER,
                text,
                theme::semibold(FONT),
                color::TEXT,
            );
            painter.line_segment(
                [
                    Pos2::new(r.right() + pt(8.0), r.center().y),
                    at(RULE_RIGHT, y),
                ],
                Stroke::new(1.0, Color32::from_gray(0x73)),
            );
        };
        // A control left-aligned in a row (ui.put would center it)
        let row = |ui: &mut Ui, x: f32, y: f32, w: f32| {
            let r = Rect::from_min_size(at(x, y - pt(9.0)), vec2(w, pt(18.0)));
            ui.new_child(egui::UiBuilder::new().max_rect(r))
        };

        header("Based On", pt(52.0));
        let choices = [
            (TrimBasis::Transparent, "Transparent Pixels"),
            (TrimBasis::TopLeftColor, "Top Left Pixel Color"),
            (TrimBasis::BottomRightColor, "Bottom Right Pixel Color"),
        ];
        for (i, (basis, label)) in choices.into_iter().enumerate() {
            let mut child = row(ui, pt(32.0), pt(78.0 + 24.0 * i as f32), pt(220.0));
            let enabled = basis != TrimBasis::Transparent || self.can_trim_transparent;
            child.add_enabled_ui(enabled, |ui| {
                ui.radio_value(
                    &mut self.basis,
                    basis,
                    egui::RichText::new(label).font(font.clone()),
                );
            });
        }

        header("Trim Away", pt(164.0));
        let sides = [
            (&mut self.sides.top, "Top", pt(32.0), pt(190.0)),
            (&mut self.sides.left, "Left", pt(132.0), pt(190.0)),
            (&mut self.sides.bottom, "Bottom", pt(32.0), pt(214.0)),
            (&mut self.sides.right, "Right", pt(132.0), pt(214.0)),
        ];
        for (flag, label, x, y) in sides {
            let mut child = row(ui, x, y, pt(96.0));
            child.checkbox(flag, egui::RichText::new(label).font(font.clone()));
        }

        let button_font = FontId::proportional(pt(13.0));
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(BUTTON_X, pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            true,
        );
        let cancel = common::pill_button(
            ui,
            Rect::from_min_size(at(BUTTON_X, pt(76.0)), BUTTON),
            "Cancel",
            button_font,
            true,
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        if ok.clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
            return Outcome::Apply {
                basis: self.basis,
                sides: self.sides,
            };
        }
        Outcome::Open
    }
}
