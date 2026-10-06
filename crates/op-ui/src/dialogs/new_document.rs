//! File > New... (Cmd+N), laid out after Photoshop's New dialog (the legacy
//! one): Name, Width, Height, Resolution, Color Mode and Background
//! Contents, with OK and Cancel on the right. Sizes are in Photoshop points.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Ui, vec2};

use super::canvas_size::MAX_DIMENSION;
use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(470.0), pt(300.0));
const FONT: f32 = pt(12.5);
const LABEL_RIGHT: f32 = pt(150.0);
const FIELD_X: f32 = pt(158.0);
const FIELD_W: f32 = pt(90.0);
const FIELD_H: f32 = pt(22.0);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));

/// "Background Contents".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Contents {
    #[default]
    White,
    Black,
    BackgroundColor,
    Transparent,
}

impl Contents {
    const ALL: [Self; 4] = [
        Self::White,
        Self::Black,
        Self::BackgroundColor,
        Self::Transparent,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::White => "White",
            Self::Black => "Black",
            Self::BackgroundColor => "Background Color",
            Self::Transparent => "Transparent",
        }
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Create {
        name: String,
        width: u32,
        height: u32,
        resolution: f32,
        contents: Contents,
    },
}

pub struct NewDocumentDialog {
    name: String,
    width: String,
    height: String,
    resolution: String,
    contents: Contents,
    first_frame: bool,
}

impl NewDocumentDialog {
    /// `name` is the next "Untitled-N"; `size` is the clipboard image's
    /// size when there is one (Photoshop's default), else 1920 × 1080.
    pub fn new(name: String, size: Option<(u32, u32)>) -> Self {
        let (w, h) = size.unwrap_or((1920, 1080));
        Self {
            name,
            width: w.to_string(),
            height: h.to_string(),
            resolution: "72".into(),
            contents: Contents::White,
            first_frame: true,
        }
    }

    fn values(&self) -> Option<(u32, u32, f32)> {
        let dim = |s: &str| -> Option<u32> {
            let v: u32 = s.trim().parse().ok()?;
            (1..=MAX_DIMENSION).contains(&v).then_some(v)
        };
        let r: f32 = self.resolution.trim().parse().ok()?;
        (1.0..=10_000.0).contains(&r).then_some(())?;
        Some((dim(&self.width)?, dim(&self.height)?, r))
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("new-document"))
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
        common::frame(ui, frame, "New", theme::semibold(pt(13.0)));
        let painter = ui.painter().clone();
        let label = |text: &str, y: f32| {
            painter.text(
                at(LABEL_RIGHT, y),
                Align2::RIGHT_CENTER,
                text,
                font.clone(),
                color::TEXT,
            );
        };
        let unit = |text: &str, x: f32, y: f32| {
            painter.text(
                at(x, y),
                Align2::LEFT_CENTER,
                text,
                font.clone(),
                color::TEXT_DIM,
            );
        };
        let field = |x: f32, y: f32, w: f32| {
            Rect::from_min_size(
                Pos2::new(frame.left() + x, frame.top() + y - FIELD_H / 2.0),
                vec2(w, FIELD_H),
            )
        };

        label("Name:", pt(52.0));
        common::number_field(
            ui,
            field(FIELD_X, pt(52.0), pt(180.0)),
            &mut self.name,
            "new-name",
            FONT,
            self.first_frame,
        );

        label("Document Type:", pt(88.0));
        unit("Custom", FIELD_X + pt(4.0), pt(88.0));

        let rows = [
            ("Width:", pt(120.0), "Pixels"),
            ("Height:", pt(150.0), "Pixels"),
            ("Resolution:", pt(180.0), "Pixels/Inch"),
        ];
        let fields = [&mut self.width, &mut self.height, &mut self.resolution];
        for (k, (text, y, u)) in rows.into_iter().enumerate() {
            label(text, y);
            common::number_field(
                ui,
                field(FIELD_X, y, FIELD_W),
                fields[k],
                ("new-value", k),
                FONT,
                false,
            );
            unit(u, FIELD_X + FIELD_W + pt(8.0), y);
        }

        label("Color Mode:", pt(212.0));
        unit("RGB Color    8 bit", FIELD_X + pt(4.0), pt(212.0));

        label("Background Contents:", pt(244.0));
        let mut contents = self.contents;
        common::dropdown(
            ui,
            field(FIELD_X, pt(244.0), pt(180.0)),
            "new-contents",
            contents.label(),
            FONT,
            true,
            |ui| {
                for c in Contents::ALL {
                    ui.selectable_value(&mut contents, c, c.label());
                }
            },
        );
        self.contents = contents;

        let x = frame.width() - pt(108.0);
        let button_font = FontId::proportional(pt(13.0));
        let values = self.values();
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(x, pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            values.is_some(),
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
            && let Some((width, height, resolution)) = values
        {
            let name = if self.name.trim().is_empty() {
                "Untitled".into()
            } else {
                self.name.trim().to_string()
            };
            return Outcome::Create {
                name,
                width,
                height,
                resolution,
                contents: self.contents,
            };
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let d = NewDocumentDialog::new("Untitled-1".into(), None);
        assert_eq!(d.values(), Some((1920, 1080, 72.0)));
        let mut d = NewDocumentDialog::new("Untitled-2".into(), Some((640, 480)));
        assert_eq!(d.values(), Some((640, 480, 72.0)));
        d.width = "0".into();
        assert_eq!(d.values(), None);
    }
}
