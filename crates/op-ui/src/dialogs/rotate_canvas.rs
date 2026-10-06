//! Image > Image Rotation > Arbitrary...: Photoshop 2026's Rotate Canvas
//! dialog (a UXP dialog, 350 × 128 pt): the angle and its direction.
//! Sizes are in Photoshop points from the dialog's top-left corner.

use egui::{Align2, Color32, Key, Pos2, Rect, Sense, Stroke, Ui, vec2};

use super::common;
use crate::theme::{self, pt};

const SIZE: egui::Vec2 = vec2(pt(350.0), pt(128.0));
const TEXT: Color32 = Color32::from_gray(0xf0);

pub enum Outcome {
    Open,
    Cancel,
    /// Degrees, clockwise when positive.
    Rotate(f32),
}

pub struct RotateCanvasDialog {
    angle: String,
    clockwise: bool,
    first_frame: bool,
}

impl Default for RotateCanvasDialog {
    fn default() -> Self {
        Self {
            angle: "0".into(),
            clockwise: true,
            first_frame: true,
        }
    }
}

impl RotateCanvasDialog {
    /// The turn the dialog asks for: -359.99 ... 359.99 degrees as typed,
    /// signed by the direction.
    pub fn degrees(&self) -> Option<f32> {
        let v: f32 = self
            .angle
            .trim()
            .trim_end_matches('°')
            .trim()
            .parse()
            .ok()?;
        (v.abs() < 360.0).then_some(if self.clockwise { v } else { -v })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("rotate-canvas"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(SIZE, Sense::hover());
                outcome = self.ui(ui, rect);
            });
        self.first_frame = false;
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(pt(x), pt(y));
        let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        let font = theme::uxp(pt(12.0));
        common::frame(ui, frame, "Rotate Canvas", theme::dialog_bold(pt(13.0)));
        let painter = ui.painter().clone();
        painter.text(
            at(20.0, 60.0),
            Align2::LEFT_CENTER,
            "Angle",
            font.clone(),
            TEXT,
        );
        let field = r(55.0, 47.0, 126.0, 73.0);
        common::text_field(
            ui,
            field,
            &mut self.angle,
            "rotate-angle",
            font.clone(),
            pt(12.0),
            self.first_frame,
        );
        // The degree sign follows the number
        let shown = painter.layout_no_wrap(self.angle.clone(), font.clone(), TEXT);
        painter.text(
            Pos2::new(field.left() + pt(12.0) + shown.size().x, field.center().y),
            Align2::LEFT_CENTER,
            "°",
            font.clone(),
            TEXT,
        );

        for (cy, label, value) in [
            (59.75, "Clockwise", true),
            (83.75, "Counter Clockwise", false),
        ] {
            let center = at(142.75, cy);
            let hit = Rect::from_min_max(center - vec2(pt(7.0), pt(9.0)), at(260.0, cy + 9.0));
            if ui
                .interact(hit, ui.id().with(label), Sense::click())
                .clicked()
            {
                self.clockwise = value;
            }
            radio(&painter, center, self.clockwise == value);
            painter.text(
                at(158.5, cy),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                TEXT,
            );
        }

        let degrees = self.degrees();
        let ok = common::ps_button(
            ui,
            r(260.0, 48.0, 330.0, 72.0),
            "OK",
            true,
            degrees.is_some(),
            true,
        );
        let cancel = common::ps_button(
            ui,
            r(260.0, 84.0, 330.0, 108.0),
            "Cancel",
            false,
            true,
            true,
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        if (ok.clicked() || ui.input(|i| i.key_pressed(Key::Enter)))
            && let Some(d) = degrees
        {
            return Outcome::Rotate(d);
        }
        Outcome::Open
    }
}

/// A Spectrum radio button, 12 pt: chosen, a light disc with a dark hole;
/// otherwise a light ring.
fn radio(painter: &egui::Painter, center: Pos2, on: bool) {
    let light = Color32::from_gray(0xd4);
    if on {
        painter.circle_filled(center, pt(6.0), light);
        painter.circle_filled(center, pt(2.0), Color32::from_gray(0x53));
    } else {
        painter.circle_stroke(
            center,
            pt(5.5),
            Stroke::new(pt(1.0), Color32::from_gray(0xa0)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_and_direction() {
        let mut d = RotateCanvasDialog::default();
        assert_eq!(d.degrees(), Some(0.0));
        d.angle = "30°".into();
        assert_eq!(d.degrees(), Some(30.0));
        d.clockwise = false;
        assert_eq!(d.degrees(), Some(-30.0));
        d.angle = "360".into();
        assert_eq!(d.degrees(), None);
        d.angle = "x".into();
        assert_eq!(d.degrees(), None);
    }
}
