//! Layer > New > Layer... (Shift+Cmd+N): Name, Color, Mode and Opacity of
//! the new layer, laid out at the positions measured on Photoshop 2026's
//! dialog (552 × 186 pt). Sizes are in Photoshop points from the dialog's
//! top-left corner.

use egui::{Align2, Color32, CornerRadius, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use op_core::{BlendMode, LayerColor, neutral_color};

use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(552.0), pt(186.0));
/// Layer from Background's version has no neutral-color row.
const SIZE_FROM_BACKGROUND: egui::Vec2 = vec2(pt(552.0), pt(157.0));
const FONT: f32 = pt(12.0);
const TEXT: Color32 = Color32::from_gray(0xf1);
const BORDER: Color32 = Color32::from_gray(0x7a);

/// What the dialog creates.
#[derive(Clone, Debug, PartialEq)]
pub struct NewLayer {
    pub name: String,
    pub color: LayerColor,
    pub mode: BlendMode,
    pub opacity: f32,
    /// Fill with the mode's neutral color (only offered when it has one).
    pub fill_neutral: bool,
}

pub enum Outcome {
    Open,
    Cancel,
    Create(NewLayer),
}

pub struct NewLayerDialog {
    name: String,
    color: LayerColor,
    mode: BlendMode,
    opacity: String,
    fill_neutral: bool,
    first_frame: bool,
    /// Layer > New > Layer from Background...: the same dialog, turning the
    /// background into a regular layer.
    from_background: bool,
}

impl NewLayerDialog {
    /// `name` is the next "Layer N".
    pub fn new(name: String) -> Self {
        Self {
            name,
            color: LayerColor::None,
            mode: BlendMode::Normal,
            opacity: "100%".into(),
            fill_neutral: false,
            first_frame: true,
            from_background: false,
        }
    }

    /// The dialog of Layer > New > Layer from Background... ("Layer 0").
    pub fn from_background() -> Self {
        Self {
            from_background: true,
            ..Self::new("Layer 0".into())
        }
    }

    pub fn is_from_background(&self) -> bool {
        self.from_background
    }

    /// The layer to create, or `None` while the opacity isn't a number.
    fn layer(&self) -> Option<NewLayer> {
        let pct: f32 = self
            .opacity
            .trim()
            .trim_end_matches('%')
            .trim()
            .parse()
            .ok()?;
        let name = self.name.trim();
        Some(NewLayer {
            name: if name.is_empty() {
                "Layer".into()
            } else {
                name.into()
            },
            color: self.color,
            mode: self.mode,
            opacity: pct.clamp(0.0, 100.0) / 100.0,
            fill_neutral: self.fill_neutral && neutral_color(self.mode).is_some(),
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("new-layer"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let size = if self.from_background {
                    SIZE_FROM_BACKGROUND
                } else {
                    SIZE
                };
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
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
        let font = theme::dialog_medium(FONT);
        common::frame(ui, frame, "New Layer", theme::dialog_bold(pt(14.0)));
        let painter = ui.painter().clone();
        let label = |right: f32, cy: f32, text: &str, c: Color32| {
            painter.text(at(right, cy), Align2::RIGHT_CENTER, text, font.clone(), c);
        };

        label(49.0, 61.0, "Name", TEXT);
        common::text_field(
            ui,
            r(59.0, 49.0, 243.0, 71.0),
            &mut self.name,
            "new-layer-name",
            theme::dialog_medium(FONT),
            pt(10.0),
            self.first_frame,
        );

        label(280.5, 61.0, "Color", TEXT);
        let color_box = r(290.0, 48.0, 450.0, 72.0);
        let mut chosen = self.color;
        let shown = chosen;
        common::ps_dropdown(
            ui,
            color_box,
            "new-layer-color",
            move |painter, rect| {
                color_label(painter, rect, shown);
            },
            |ui| {
                for c in LayerColor::ALL {
                    ui.selectable_value(&mut chosen, c, c.label());
                }
            },
        );
        self.color = chosen;

        // Clipping masks aren't supported yet, so this stays off
        common::ps_checkbox(
            ui,
            at(58.0, 83.0),
            "Use previous layer to create clipping mask",
            &mut false,
            false,
        );

        label(49.0, 125.5, "Mode", TEXT);
        let mut mode = self.mode;
        let shown = mode;
        common::ps_dropdown(
            ui,
            r(58.0, 113.0, 218.0, 137.0),
            "new-layer-mode",
            move |painter, rect| {
                painter.text(
                    rect.left_center() + vec2(pt(9.0), 0.0),
                    Align2::LEFT_CENTER,
                    shown.label(),
                    theme::dialog_medium(FONT),
                    TEXT,
                );
            },
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

        label(280.5, 125.5, "Opacity", TEXT);
        let opacity_box = r(290.0, 113.0, 340.0, 137.0);
        let chevron_box = r(339.0, 113.0, 358.0, 137.0);
        ui.painter().rect(
            chevron_box,
            CornerRadius {
                nw: 0,
                sw: 0,
                ne: 3,
                se: 3,
            },
            color::PANEL,
            Stroke::new(pt(1.0), BORDER),
            StrokeKind::Inside,
        );
        common::text_field(
            ui,
            opacity_box,
            &mut self.opacity,
            "new-layer-opacity",
            theme::dialog_medium(FONT),
            pt(11.5),
            false,
        );
        let chevron = ui.interact(chevron_box, ui.id().with("opacity-slider"), Sense::click());
        crate::ps_icons::paint(
            ui.painter(),
            chevron_box.center(),
            crate::ps_icons::Icon::DialogChevron,
            TEXT,
            color::PANEL,
        );
        egui::Popup::from_toggle_button_response(&chevron).show(|ui| {
            let mut pct = self.layer().map_or(100.0, |l| l.opacity * 100.0);
            if ui
                .add(egui::Slider::new(&mut pct, 0.0..=100.0).show_value(false))
                .changed()
            {
                self.opacity = format!("{}%", pct.round());
            }
        });

        let neutral = neutral_color(self.mode).filter(|_| !self.from_background);
        if !self.from_background {
            let neutral_label = format!("Fill with {}-neutral color", self.mode.label());
            common::ps_checkbox(
                ui,
                at(58.0, 148.0),
                if neutral.is_some() {
                    &neutral_label
                } else {
                    "Fill with neutral color"
                },
                &mut self.fill_neutral,
                neutral.is_some(),
            );
            let swatch = r(192.0, 142.0, 216.0, 166.0);
            let swatch = if neutral.is_some() {
                // The swatch follows the label's length
                let galley_w = ui
                    .painter()
                    .layout_no_wrap(neutral_label.clone(), font.clone(), TEXT)
                    .size()
                    .x;
                Rect::from_min_size(
                    Pos2::new(at(79.5, 0.0).x + galley_w + pt(12.0), swatch.top()),
                    swatch.size(),
                )
            } else {
                swatch
            };
            let (fill, border) = match neutral {
                Some([r, g, b]) => (Color32::from_rgb(r, g, b), BORDER),
                None => (Color32::from_gray(0x76), Color32::from_gray(0x8e)),
            };
            ui.painter().rect(
                swatch,
                CornerRadius::same(pt(3.0) as u8),
                fill,
                Stroke::new(pt(1.0), border),
                StrokeKind::Inside,
            );
        }

        let layer = self.layer();
        let ok = common::ps_button(
            ui,
            r(462.0, 48.0, 532.0, 72.0),
            "OK",
            true,
            layer.is_some(),
            true,
        );
        let cancel = common::ps_button(
            ui,
            r(462.0, 84.0, 532.0, 108.0),
            "Cancel",
            false,
            true,
            true,
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if (ok.clicked() || enter)
            && let Some(layer) = layer
        {
            return Outcome::Create(layer);
        }
        Outcome::Open
    }
}

/// The color label's swatch (an X in a box for None) and name.
fn color_label(painter: &egui::Painter, rect: Rect, c: LayerColor) {
    let icon = Rect::from_min_size(rect.min + vec2(pt(8.0), pt(5.0)), vec2(pt(14.0), pt(14.0)));
    match crate::panels::layer_color(c) {
        None => {
            painter.rect_stroke(
                icon,
                pt(2.0),
                Stroke::new(pt(1.0), TEXT),
                StrokeKind::Inside,
            );
            let i = icon.shrink(pt(3.5));
            painter.line_segment([i.left_top(), i.right_bottom()], Stroke::new(pt(1.0), TEXT));
            painter.line_segment([i.right_top(), i.left_bottom()], Stroke::new(pt(1.0), TEXT));
        }
        Some(fill) => {
            painter.rect(
                icon,
                pt(2.0),
                fill,
                Stroke::new(pt(1.0), TEXT),
                StrokeKind::Inside,
            );
        }
    }
    painter.text(
        Pos2::new(rect.left() + pt(31.0), rect.center().y),
        Align2::LEFT_CENTER,
        c.label(),
        theme::dialog_medium(FONT),
        TEXT,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_values() {
        let mut d = NewLayerDialog::new("Layer 3".into());
        let l = d.layer().unwrap();
        assert_eq!(
            (l.name.as_str(), l.mode, l.opacity, l.color),
            ("Layer 3", BlendMode::Normal, 1.0, LayerColor::None)
        );
        assert!(!l.fill_neutral);
        d.opacity = "40".into();
        assert_eq!(d.layer().unwrap().opacity, 0.4);
        d.opacity = "abc".into();
        assert!(d.layer().is_none());
        d.opacity = "250%".into();
        assert_eq!(d.layer().unwrap().opacity, 1.0);
        // Fill with neutral color only counts in modes that have one
        d.fill_neutral = true;
        assert!(!d.layer().unwrap().fill_neutral);
        d.mode = BlendMode::Overlay;
        assert!(d.layer().unwrap().fill_neutral);
    }
}
