//! Image > Adjustments > Equalize... with a selection: Photoshop 2026's
//! classic dialog (409 × 127 pt) asking whether to equalize only the
//! selected area or the entire image based on it. Sizes are Photoshop
//! points from the dialog's top-left corner.

use egui::{Color32, Key, Rect, Sense, Ui, vec2};

use super::{appkit, common};
use crate::theme::{self, pt};

const SIZE: egui::Vec2 = vec2(pt(409.0), pt(127.0));

pub enum Outcome {
    Open,
    Cancel,
    /// Equalize; `entire_image` is the second choice.
    Apply {
        entire_image: bool,
    },
}

pub struct EqualizeDialog {
    /// Photoshop's default: the entire image based on the selected area.
    pub entire_image: bool,
}

impl Default for EqualizeDialog {
    fn default() -> Self {
        Self { entire_image: true }
    }
}

impl EqualizeDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("equalize-dialog"))
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
        let at = |x: f32, y: f32| frame.min + vec2(pt(x), pt(y));
        let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        common::frame(ui, frame, "Equalize", theme::dialog_bold(pt(13.0)));
        appkit::group(
            ui.painter(),
            r(11.0, 46.5, 322.0, 116.5),
            (at(25.5, 0.0).x, at(79.0, 0.0).x),
        );
        appkit::label(ui, at(30.5, 46.0), "Options");
        if appkit::radio(
            ui,
            at(27.0, 70.5),
            "Equalize selected area only",
            !self.entire_image,
        ) {
            self.entire_image = false;
        }
        if appkit::radio(
            ui,
            at(27.0, 97.0),
            "Equalize entire image based on selected area",
            self.entire_image,
        ) {
            self.entire_image = true;
        }
        let ok = appkit::button(ui, r(339.5, 45.0, 398.5, 71.0), "OK", true, true);
        let cancel = appkit::button(ui, r(339.5, 80.0, 398.5, 106.0), "Cancel", false, true);
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        if ok.clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
            return Outcome::Apply {
                entire_image: self.entire_image,
            };
        }
        Outcome::Open
    }
}
