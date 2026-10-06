//! Edit > Fill (Shift+F5).
//!
//! Laid out after Photoshop's Fill dialog (Contents, then Blending with Mode,
//! Opacity and Preserve Transparency; OK and Cancel on the right). Sizes are
//! in Photoshop points.

use egui::{Align2, Color32, FontId, Key, Rect, Sense, Stroke, Ui, vec2};
use op_core::fill::FillOptions;
use op_core::{BlendMode, Color};

use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(420.0), pt(222.0));
const FONT: f32 = pt(12.5);
const LABEL_RIGHT: f32 = pt(84.0);
const CONTROL_X: f32 = pt(92.0);
const CONTROL_W: f32 = pt(200.0);
const CONTROL_H: f32 = pt(22.0);
const BUTTON_X: f32 = pt(316.0);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));

/// What the dialog fills with ("Contents").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    Foreground,
    Background,
    Color,
    ContentAware,
    Pattern,
    History,
    Black,
    Gray,
    White,
}

impl Contents {
    const ALL: [Self; 9] = [
        Self::Foreground,
        Self::Background,
        Self::Color,
        Self::ContentAware,
        Self::Pattern,
        Self::History,
        Self::Black,
        Self::Gray,
        Self::White,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Foreground => "Foreground Color",
            Self::Background => "Background Color",
            Self::Color => "Color...",
            Self::ContentAware => "Content-Aware",
            Self::Pattern => "Pattern",
            Self::History => "History",
            Self::Black => "Black",
            Self::Gray => "50% Gray",
            Self::White => "White",
        }
    }

    /// Content-Aware, Pattern and History aren't implemented.
    fn available(self) -> bool {
        !matches!(self, Self::ContentAware | Self::Pattern | Self::History)
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Apply { color: Color, options: FillOptions },
}

pub struct FillDialog {
    contents: Contents,
    /// The color chosen with "Color...".
    color: Color,
    mode: BlendMode,
    opacity: String,
    preserve_transparency: bool,
    wants_color_picker: bool,
    first_frame: bool,
}

impl Default for FillDialog {
    fn default() -> Self {
        Self {
            contents: Contents::Foreground,
            color: Color::WHITE,
            mode: BlendMode::Normal,
            opacity: "100".into(),
            preserve_transparency: false,
            wants_color_picker: false,
            first_frame: true,
        }
    }
}

impl FillDialog {
    /// "Color..." asks for the Color Picker; returns its starting color once.
    pub fn take_color_picker_request(&mut self) -> Option<Color> {
        std::mem::take(&mut self.wants_color_picker).then_some(self.color)
    }

    /// The Color Picker was confirmed for "Color...".
    pub fn set_color(&mut self, color: Color) {
        self.contents = Contents::Color;
        self.color = color;
    }

    fn fill_color(&self, foreground: Color, background: Color) -> Color {
        match self.contents {
            Contents::Foreground => foreground,
            Contents::Background => background,
            Contents::Color => self.color,
            Contents::Black => Color::BLACK,
            Contents::Gray => Color::from_rgba8([128, 128, 128, 255]),
            Contents::White | Contents::ContentAware | Contents::Pattern | Contents::History => {
                Color::WHITE
            }
        }
    }

    fn opacity(&self) -> Option<f32> {
        let v: f32 = self.opacity.trim().parse().ok()?;
        (0.0..=100.0).contains(&v).then_some(v / 100.0)
    }

    /// `active` is false while the Color Picker is open on top.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        foreground: Color,
        background: Color,
        active: bool,
    ) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("fill-dialog"))
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
        common::frame(ui, frame, "Fill", theme::semibold(pt(13.0)));
        let painter = ui.painter().clone();
        let row = |y: f32| {
            Rect::from_min_size(
                at(CONTROL_X, y - CONTROL_H / 2.0),
                vec2(CONTROL_W, CONTROL_H),
            )
        };
        let label = |text: &str, y: f32| {
            painter.text(
                at(LABEL_RIGHT, y),
                Align2::RIGHT_CENTER,
                text,
                font.clone(),
                color::TEXT,
            );
        };

        // Contents
        label("Contents:", pt(56.0));
        let mut contents = self.contents;
        common::dropdown(
            ui,
            row(pt(56.0)),
            "fill-contents",
            contents.label(),
            FONT,
            true,
            |ui| {
                for (i, c) in Contents::ALL.into_iter().enumerate() {
                    if i == 3 || i == 6 {
                        ui.separator();
                    }
                    ui.add_enabled_ui(c.available(), |ui| {
                        ui.selectable_value(&mut contents, c, c.label());
                    });
                }
            },
        );
        if contents == Contents::Color && self.contents != Contents::Color {
            // "Color..." opens the Color Picker; the choice applies once a
            // color is confirmed there
            self.wants_color_picker = true;
        } else {
            self.contents = contents;
        }

        // Blending section with its rule
        let header_y = pt(96.0);
        let r = painter.text(
            at(pt(20.0), header_y),
            Align2::LEFT_CENTER,
            "Blending",
            theme::semibold(FONT),
            color::TEXT,
        );
        painter.line_segment(
            [
                egui::pos2(r.right() + pt(8.0), r.center().y),
                at(pt(296.0), header_y),
            ],
            Stroke::new(1.0, Color32::from_gray(0x73)),
        );

        label("Mode:", pt(126.0));
        let mut mode = self.mode;
        common::dropdown(
            ui,
            row(pt(126.0)),
            "fill-mode",
            mode.label(),
            FONT,
            true,
            |ui| {
                for (gi, group) in BlendMode::GROUPS.iter().enumerate() {
                    if gi > 0 {
                        ui.separator();
                    }
                    for &m in *group {
                        ui.selectable_value(&mut mode, m, m.label());
                    }
                }
            },
        );
        self.mode = mode;

        label("Opacity:", pt(156.0));
        let field = Rect::from_min_size(
            at(CONTROL_X, pt(156.0) - CONTROL_H / 2.0),
            vec2(pt(52.0), CONTROL_H),
        );
        common::number_field(
            ui,
            field,
            &mut self.opacity,
            "fill-opacity",
            FONT,
            self.first_frame,
        );
        painter.text(
            field.right_center() + vec2(pt(6.0), 0.0),
            Align2::LEFT_CENTER,
            "%",
            font.clone(),
            color::TEXT,
        );

        // Left-aligned under the fields (ui.put would center it)
        let check = Rect::from_min_size(
            at(CONTROL_X, pt(186.0) - pt(9.0)),
            vec2(pt(200.0), pt(18.0)),
        );
        let mut check_ui = ui.new_child(egui::UiBuilder::new().max_rect(check));
        check_ui.checkbox(
            &mut self.preserve_transparency,
            egui::RichText::new("Preserve Transparency").font(font.clone()),
        );

        // Buttons
        let button_font = FontId::proportional(pt(13.0));
        let opacity = self.opacity();
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(BUTTON_X, pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            opacity.is_some(),
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
        let enter = active && ui.input(|i| i.key_pressed(Key::Enter));
        if (ok.clicked() || enter)
            && let Some(opacity) = opacity
        {
            return Outcome::Apply {
                color: self.fill_color(foreground, background),
                options: FillOptions {
                    mode: self.mode,
                    opacity,
                    preserve_transparency: self.preserve_transparency,
                },
            };
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contents_pick_the_color() {
        let mut d = FillDialog::default();
        let (fg, bg) = (
            Color::from_rgba8([1, 2, 3, 255]),
            Color::from_rgba8([4, 5, 6, 255]),
        );
        assert_eq!(d.fill_color(fg, bg), fg);
        d.contents = Contents::Gray;
        assert_eq!(d.fill_color(fg, bg).to_rgba8(), [128, 128, 128, 255]);
        d.set_color(Color::from_rgba8([7, 8, 9, 255]));
        assert_eq!(d.fill_color(fg, bg).to_rgba8(), [7, 8, 9, 255]);
    }

    #[test]
    fn opacity_must_be_a_percentage() {
        let mut d = FillDialog::default();
        assert_eq!(d.opacity(), Some(1.0));
        d.opacity = "150".into();
        assert_eq!(d.opacity(), None);
        d.opacity = "x".into();
        assert_eq!(d.opacity(), None);
    }
}
