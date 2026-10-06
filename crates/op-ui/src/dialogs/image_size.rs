//! Image > Image Size (Alt+Cmd+I).
//!
//! Laid out after the right-hand column of Photoshop's dialog: the image's
//! memory size and dimensions, Width and Height in pixels linked by the
//! "constrain proportions" chain, Resolution, and Resample with its method.
//! Sizes are in Photoshop points.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Ui, vec2};
use op_core::image_ops::Resample;

use super::canvas_size::MAX_DIMENSION;
use super::common;
use crate::icons;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(440.0), pt(290.0));
const FONT: f32 = pt(12.5);
const LABEL_RIGHT: f32 = pt(96.0);
const FIELD_X: f32 = pt(104.0);
const FIELD_W: f32 = pt(80.0);
const FIELD_H: f32 = pt(22.0);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));

pub enum Outcome {
    Open,
    Cancel,
    Apply {
        width: u32,
        height: u32,
        resolution: f32,
        /// `None` when Resample is off (only the resolution changes).
        resample: Option<Resample>,
    },
}

pub struct ImageSizeDialog {
    /// The document's size when the dialog opened.
    original: (u32, u32),
    width: String,
    height: String,
    resolution: String,
    constrain: bool,
    resample: bool,
    method: Resample,
    first_frame: bool,
}

impl ImageSizeDialog {
    pub fn new(width: u32, height: u32, resolution: f32) -> Self {
        Self {
            original: (width, height),
            width: width.to_string(),
            height: height.to_string(),
            resolution: format_resolution(resolution),
            constrain: true,
            resample: true,
            method: Resample::Bicubic,
            first_frame: true,
        }
    }

    fn dimension(text: &str) -> Option<u32> {
        let v: u32 = text.trim().parse().ok()?;
        (1..=MAX_DIMENSION).contains(&v).then_some(v)
    }

    fn values(&self) -> Option<(u32, u32, f32)> {
        let resolution: f32 = self.resolution.trim().parse().ok()?;
        if !(1.0..=10_000.0).contains(&resolution) {
            return None;
        }
        Some((
            Self::dimension(&self.width)?,
            Self::dimension(&self.height)?,
            resolution,
        ))
    }

    /// With the chain on, editing one dimension updates the other.
    fn constrain_from(&mut self, width_changed: bool) {
        let (w0, h0) = (self.original.0 as f32, self.original.1 as f32);
        if width_changed {
            if let Some(w) = Self::dimension(&self.width) {
                self.height = ((w as f32 * h0 / w0).round().max(1.0) as u32).to_string();
            }
        } else if let Some(h) = Self::dimension(&self.height) {
            self.width = ((h as f32 * w0 / h0).round().max(1.0) as u32).to_string();
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("image-size"))
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
        common::frame(ui, frame, "Image Size", theme::semibold(pt(13.0)));
        let painter = ui.painter().clone();
        let text = |s: &str, p: Pos2, align: Align2, c: Color32| {
            painter.text(p, align, s, font.clone(), c);
        };

        // Memory size and dimensions of the result
        let (w, h) = (
            Self::dimension(&self.width).unwrap_or(self.original.0),
            Self::dimension(&self.height).unwrap_or(self.original.1),
        );
        let bytes = w as f64 * h as f64 * 3.0;
        let size = if bytes >= 1024.0 * 1024.0 {
            format!("{:.1}M", bytes / 1024.0 / 1024.0)
        } else {
            format!("{:.1}K", bytes / 1024.0)
        };
        text(
            &format!("Image Size: {size}"),
            at(pt(20.0), pt(52.0)),
            Align2::LEFT_CENTER,
            color::TEXT,
        );
        text(
            &format!("Dimensions: {w} px × {h} px"),
            at(pt(20.0), pt(76.0)),
            Align2::LEFT_CENTER,
            color::TEXT_DIM,
        );

        let row =
            |y: f32| Rect::from_min_size(at(FIELD_X, y - FIELD_H / 2.0), vec2(FIELD_W, FIELD_H));
        let label = |s: &str, y: f32, enabled: bool| {
            let c = if enabled {
                color::TEXT
            } else {
                color::TEXT_DISABLED
            };
            text(s, at(LABEL_RIGHT, y), Align2::RIGHT_CENTER, c);
        };
        let unit = |s: &str, y: f32| {
            text(
                s,
                at(FIELD_X + FIELD_W + pt(8.0), y),
                Align2::LEFT_CENTER,
                color::TEXT_DIM,
            );
        };

        let dims = self.resample;
        let (wy, hy, ry) = (pt(116.0), pt(146.0), pt(184.0));
        label("Width:", wy, dims);
        label("Height:", hy, dims);
        let before = (self.width.clone(), self.height.clone());
        ui.add_enabled_ui(dims, |ui| {
            common::number_field(
                ui,
                row(wy),
                &mut self.width,
                "image-width",
                FONT,
                self.first_frame,
            );
            common::number_field(ui, row(hy), &mut self.height, "image-height", FONT, false);
        });
        if self.constrain {
            if self.width != before.0 {
                self.constrain_from(true);
            } else if self.height != before.1 {
                self.constrain_from(false);
            }
        }
        unit("Pixels", wy);
        unit("Pixels", hy);
        // The chain linking Width and Height
        let chain = Rect::from_center_size(at(pt(44.0), (wy + hy) / 2.0), vec2(pt(18.0), pt(30.0)));
        let response = ui.interact(chain, ui.id().with("constrain"), Sense::click());
        if response.clicked() {
            self.constrain = !self.constrain;
            if self.constrain {
                self.constrain_from(true);
            }
        }
        let icon = if self.constrain {
            icons::LINK_SIMPLE
        } else {
            icons::LINK_SIMPLE_BREAK
        };
        painter.text(
            chain.center(),
            Align2::CENTER_CENTER,
            icon,
            theme::icon(pt(14.0)),
            color::ICON,
        );

        label("Resolution:", ry, true);
        common::number_field(
            ui,
            row(ry),
            &mut self.resolution,
            "image-resolution",
            FONT,
            false,
        );
        unit("Pixels/Inch", ry);

        // Resample
        let check =
            Rect::from_min_size(at(pt(28.0), pt(222.0) - pt(9.0)), vec2(pt(90.0), pt(18.0)));
        let mut check_ui = ui.new_child(egui::UiBuilder::new().max_rect(check));
        let toggled = check_ui
            .checkbox(
                &mut self.resample,
                egui::RichText::new("Resample:").font(font.clone()),
            )
            .changed();
        if toggled && !self.resample {
            // Without resampling the pixel dimensions can't change
            self.width = self.original.0.to_string();
            self.height = self.original.1.to_string();
        }
        let mut method = self.method;
        ui.add_enabled_ui(self.resample, |ui| {
            common::dropdown(
                ui,
                Rect::from_min_size(
                    at(FIELD_X + pt(20.0), pt(222.0) - FIELD_H / 2.0),
                    vec2(pt(200.0), FIELD_H),
                ),
                "image-resample",
                method.label(),
                FONT,
                true,
                |ui| {
                    for m in Resample::ALL {
                        ui.selectable_value(&mut method, m, m.label());
                    }
                },
            );
        });
        self.method = method;

        // OK and Cancel, bottom right as in Photoshop's dialog
        let button_font = FontId::proportional(pt(13.0));
        let y = frame.height() - pt(20.0) - BUTTON.y;
        let values = self.values();
        let cancel = common::pill_button(
            ui,
            Rect::from_min_size(
                at(frame.width() - pt(20.0) - BUTTON.x * 2.0 - pt(10.0), y),
                BUTTON,
            ),
            "Cancel",
            button_font.clone(),
            true,
        );
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(frame.width() - pt(20.0) - BUTTON.x, y), BUTTON),
            "OK",
            button_font,
            values.is_some(),
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if (ok.clicked() || enter)
            && let Some((width, height, resolution)) = values
        {
            return Outcome::Apply {
                width,
                height,
                resolution,
                resample: self.resample.then_some(self.method),
            };
        }
        Outcome::Open
    }
}

fn format_resolution(r: f32) -> String {
    if r.fract() == 0.0 {
        format!("{r:.0}")
    } else {
        format!("{r:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chain_keeps_proportions() {
        let mut d = ImageSizeDialog::new(800, 600, 72.0);
        d.width = "400".into();
        d.constrain_from(true);
        assert_eq!(d.height, "300");
        d.height = "150".into();
        d.constrain_from(false);
        assert_eq!(d.width, "200");
        assert_eq!(d.values(), Some((200, 150, 72.0)));
        d.width = "0".into();
        assert_eq!(d.values(), None);
    }
}
