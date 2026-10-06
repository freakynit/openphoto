//! View > Guides > New Guide...: Orientation (Horizontal or Vertical) and
//! Position in pixels, laid out after Photoshop's dialog. Sizes are in
//! Photoshop points.

use egui::{Align2, Color32, FontId, Key, Rect, Sense, Ui, vec2};
use op_core::Guide;

use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(330.0), pt(170.0));
const FONT: f32 = pt(12.5);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));

pub enum Outcome {
    Open,
    Cancel,
    Apply(Guide),
}

pub struct NewGuideDialog {
    vertical: bool,
    position: String,
    first_frame: bool,
}

impl Default for NewGuideDialog {
    /// Photoshop starts with a horizontal guide at 0.
    fn default() -> Self {
        Self {
            vertical: false,
            position: "0".into(),
            first_frame: true,
        }
    }
}

impl NewGuideDialog {
    fn guide(&self) -> Option<Guide> {
        let position: f32 = self
            .position
            .trim()
            .trim_end_matches("px")
            .trim()
            .parse()
            .ok()?;
        (position.abs() <= 30_000.0).then_some(Guide {
            vertical: self.vertical,
            position,
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("new-guide"))
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
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let font = FontId::proportional(FONT);
        common::frame(ui, frame, "New Guide", theme::semibold(pt(13.0)));
        ui.painter().text(
            at(pt(20.0), pt(50.0)),
            Align2::LEFT_CENTER,
            "Orientation",
            theme::semibold(FONT),
            color::TEXT,
        );
        for (k, (vertical, label)) in [(false, "Horizontal"), (true, "Vertical")]
            .into_iter()
            .enumerate()
        {
            let r = Rect::from_min_size(
                at(pt(32.0), pt(66.0) + pt(22.0) * k as f32),
                vec2(pt(150.0), pt(18.0)),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            child.radio_value(
                &mut self.vertical,
                vertical,
                egui::RichText::new(label).font(font.clone()),
            );
        }
        ui.painter().text(
            at(pt(20.0), pt(132.0)),
            Align2::LEFT_CENTER,
            "Position:",
            font.clone(),
            color::TEXT,
        );
        let field = Rect::from_min_size(at(pt(84.0), pt(121.0)), vec2(pt(80.0), pt(22.0)));
        common::number_field(
            ui,
            field,
            &mut self.position,
            "guide-position",
            FONT,
            self.first_frame,
        );
        ui.painter().text(
            at(pt(172.0), pt(132.0)),
            Align2::LEFT_CENTER,
            "px",
            font,
            color::TEXT_DIM,
        );

        let x = frame.width() - pt(108.0);
        let button_font = FontId::proportional(pt(13.0));
        let guide = self.guide();
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(x, pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            guide.is_some(),
        );
        let cancel = common::pill_button(
            ui,
            Rect::from_min_size(at(x, pt(76.0)), BUTTON),
            "Cancel",
            button_font,
            true,
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if (ok.clicked() || enter)
            && let Some(guide) = guide
        {
            return Outcome::Apply(guide);
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_accepts_a_px_suffix() {
        let mut d = NewGuideDialog {
            position: "120 px".into(),
            vertical: true,
            ..Default::default()
        };
        assert_eq!(
            d.guide(),
            Some(Guide {
                vertical: true,
                position: 120.0
            })
        );
        d.position = "abc".into();
        assert_eq!(d.guide(), None);
    }
}
