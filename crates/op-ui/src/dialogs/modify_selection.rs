//! Select > Modify: Border..., Smooth..., Expand..., Contract... and
//! Feather... (Shift+F6). One value in pixels each, plus "Apply effect at
//! canvas bounds" for Smooth, Contract and Feather, as in Photoshop.

use egui::{Align2, Color32, FontId, Key, Rect, Sense, Ui, vec2};
use op_core::Selection;

use super::common;
use crate::theme::{self, color, pt};

const FONT: f32 = pt(12.5);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifyKind {
    Border,
    Smooth,
    Expand,
    Contract,
    Feather,
}

impl ModifyKind {
    fn title(self) -> &'static str {
        match self {
            Self::Border => "Border Selection",
            Self::Smooth => "Smooth Selection",
            Self::Expand => "Expand Selection",
            Self::Contract => "Contract Selection",
            Self::Feather => "Feather Selection",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Border => "Width:",
            Self::Smooth => "Sample Radius:",
            Self::Expand => "Expand By:",
            Self::Contract => "Contract By:",
            Self::Feather => "Feather Radius:",
        }
    }

    /// The history name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Border => "Border",
            Self::Smooth => "Smooth",
            Self::Expand => "Expand",
            Self::Contract => "Contract",
            Self::Feather => "Feather",
        }
    }

    fn range(self) -> (f32, f32) {
        match self {
            Self::Border => (1.0, 200.0),
            Self::Smooth | Self::Expand | Self::Contract => (1.0, 500.0),
            Self::Feather => (0.1, 1000.0),
        }
    }

    fn has_bounds_option(self) -> bool {
        matches!(self, Self::Smooth | Self::Contract | Self::Feather)
    }

    /// The modified selection.
    pub fn apply(self, s: &Selection, value: f32, at_bounds: bool) -> Selection {
        match self {
            Self::Border => s.border(value),
            Self::Smooth => s.smooth(value.round() as u32),
            Self::Expand => s.expand(value.round()),
            Self::Contract => s.contract(value.round(), at_bounds),
            Self::Feather => s.feather(value),
        }
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Apply { value: f32, at_bounds: bool },
}

pub struct ModifyDialog {
    pub kind: ModifyKind,
    value: String,
    at_bounds: bool,
    first_frame: bool,
}

impl ModifyDialog {
    pub fn new(kind: ModifyKind) -> Self {
        Self {
            kind,
            value: "1".into(),
            at_bounds: false,
            first_frame: true,
        }
    }

    fn value(&self) -> Option<f32> {
        let v: f32 = self.value.trim().parse().ok()?;
        let (lo, hi) = self.kind.range();
        (lo..=hi).contains(&v).then_some(v)
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let height = if self.kind.has_bounds_option() {
            pt(140.0)
        } else {
            pt(116.0)
        };
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("modify-selection"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(pt(380.0), height), Sense::hover());
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
        common::frame(ui, frame, self.kind.title(), theme::semibold(pt(13.0)));
        let label = ui.painter().text(
            at(pt(20.0), pt(56.0)),
            Align2::LEFT_CENTER,
            self.kind.label(),
            font.clone(),
            color::TEXT,
        );
        let field = Rect::from_min_size(
            egui::pos2(label.right() + pt(8.0), label.center().y - pt(11.0)),
            vec2(pt(60.0), pt(22.0)),
        );
        common::number_field(
            ui,
            field,
            &mut self.value,
            "modify-value",
            FONT,
            self.first_frame,
        );
        ui.painter().text(
            field.right_center() + vec2(pt(6.0), 0.0),
            Align2::LEFT_CENTER,
            "pixels",
            font.clone(),
            color::TEXT,
        );
        if self.kind.has_bounds_option() {
            let r = Rect::from_min_size(at(pt(20.0), pt(88.0)), vec2(pt(240.0), pt(18.0)));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r));
            child.checkbox(
                &mut self.at_bounds,
                egui::RichText::new("Apply effect at canvas bounds").font(font),
            );
        }

        let x = frame.width() - pt(108.0);
        let button_font = FontId::proportional(pt(13.0));
        let value = self.value();
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(x, pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            value.is_some(),
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
            && let Some(value) = value
        {
            return Outcome::Apply {
                value,
                at_bounds: self.at_bounds,
            };
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        let mut d = ModifyDialog::new(ModifyKind::Border);
        assert_eq!(d.value(), Some(1.0));
        d.value = "201".into();
        assert_eq!(d.value(), None);
        let mut f = ModifyDialog::new(ModifyKind::Feather);
        f.value = "0.5".into();
        assert_eq!(f.value(), Some(0.5));
    }
}
