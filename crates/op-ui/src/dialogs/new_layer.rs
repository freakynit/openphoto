//! Layer > New > Layer... (Shift+Cmd+N): Name, Color, Mode and Opacity of
//! the new layer, laid out at the positions measured on Photoshop 2026's
//! dialog (552 × 186 pt). Sizes are in Photoshop points from the dialog's
//! top-left corner.

use egui::{Align2, Color32, CornerRadius, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use op_core::{BlendMode, LayerColor, neutral_color};

use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(552.0), pt(186.0));
const FONT: f32 = pt(12.0);
const TEXT: Color32 = Color32::from_gray(0xf1);
const TEXT_OFF: Color32 = Color32::from_gray(0x8e);
const BORDER: Color32 = Color32::from_gray(0x7a);
const CHECK_BORDER: Color32 = Color32::from_gray(0xa0);

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
        }
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
        let font = theme::dialog(FONT);
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
            theme::dialog(FONT),
            pt(10.0),
            self.first_frame,
        );

        label(280.5, 61.0, "Color", TEXT);
        let color_box = r(290.0, 48.0, 450.0, 72.0);
        let mut chosen = self.color;
        let shown = chosen;
        dialog_dropdown(
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
        checkbox(
            ui,
            at(58.0, 83.0),
            "Use previous layer to create clipping mask",
            &mut false,
            false,
        );

        label(49.0, 125.5, "Mode", TEXT);
        let mut mode = self.mode;
        let shown = mode;
        dialog_dropdown(
            ui,
            r(58.0, 113.0, 218.0, 137.0),
            "new-layer-mode",
            move |painter, rect| {
                painter.text(
                    rect.left_center() + vec2(pt(9.0), 0.0),
                    Align2::LEFT_CENTER,
                    shown.label(),
                    theme::dialog(FONT),
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
            theme::dialog(FONT),
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

        let neutral = neutral_color(self.mode);
        let neutral_label = format!("Fill with {}-neutral color", self.mode.label());
        checkbox(
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

        let layer = self.layer();
        let ok = button(ui, r(462.0, 48.0, 532.0, 72.0), "OK", true, layer.is_some());
        let cancel = button(ui, r(462.0, 84.0, 532.0, 108.0), "Cancel", false, true);
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
        theme::dialog(FONT),
        TEXT,
    );
}

/// A dialog dropdown: a rounded box with a 1 pt `#7a7a7a` border, content
/// drawn by `content`, a chevron 14 pt from the right; clicking opens `menu`.
fn dialog_dropdown(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    content: impl FnOnce(&egui::Painter, Rect),
    menu: impl FnOnce(&mut Ui),
) {
    let response = ui.interact(rect, ui.id().with(id), Sense::click());
    let fill = if response.hovered() {
        color::HOVER
    } else {
        color::PANEL
    };
    ui.painter().rect(
        rect,
        CornerRadius::same(pt(3.0) as u8),
        fill,
        Stroke::new(pt(1.0), BORDER),
        StrokeKind::Inside,
    );
    content(ui.painter(), rect);
    crate::ps_icons::paint(
        ui.painter(),
        Pos2::new(rect.right() - pt(13.5), rect.center().y),
        crate::ps_icons::Icon::DialogChevron,
        TEXT,
        fill,
    );
    egui::Popup::menu(&response)
        .id(ui.id().with((id, "menu")))
        .show(menu);
}

/// A 12 pt dialog checkbox with its label 9.5 pt to the right.
fn checkbox(ui: &mut Ui, min: Pos2, label: &str, checked: &mut bool, enabled: bool) {
    let font = theme::dialog(FONT);
    let text = if enabled { TEXT } else { TEXT_OFF };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, text);
    let b = Rect::from_min_size(min, vec2(pt(12.0), pt(12.0)));
    let hit = Rect::from_min_max(
        b.min,
        Pos2::new(b.right() + pt(9.5) + galley.size().x, b.bottom()),
    );
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(hit, ui.id().with(("check", label)), sense);
    if response.clicked() {
        *checked = !*checked;
    }
    let painter = ui.painter();
    let border = if enabled {
        CHECK_BORDER
    } else {
        Color32::from_gray(0x8e)
    };
    if *checked && enabled {
        painter.rect_filled(b, pt(2.5), color::CHECKBOX);
        let p = |x: f32, y: f32| b.min + vec2(pt(x), pt(y));
        painter.add(egui::Shape::line(
            vec![p(3.0, 6.0), p(5.25, 8.25), p(9.25, 3.75)],
            Stroke::new(pt(1.7), color::CHECK_MARK),
        ));
    } else {
        painter.rect_stroke(b, pt(2.5), Stroke::new(pt(1.0), border), StrokeKind::Inside);
    }
    painter.galley(
        Pos2::new(b.right() + pt(9.5), b.center().y - galley.size().y / 2.0),
        galley,
        text,
    );
}

/// Photoshop's dialog buttons: pills with a 1 pt border, bright for the
/// default button (bold label) and dim for the others.
fn button(ui: &mut Ui, rect: Rect, label: &str, default: bool, enabled: bool) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(("button", label)), sense);
    let fill = if enabled && response.is_pointer_button_down_on() {
        color::TOOL_ACTIVE
    } else if enabled && response.hovered() {
        color::HOVER
    } else {
        color::PANEL
    };
    let border = match (enabled, default) {
        (false, _) => Color32::from_gray(0x5e),
        (true, true) => TEXT,
        (true, false) => Color32::from_gray(0x72),
    };
    // Photoshop sets every dialog button's label in bold
    let _ = default;
    let font = theme::dialog_bold(pt(13.0));
    ui.painter().rect(
        rect,
        CornerRadius::same(255),
        fill,
        Stroke::new(pt(1.0), border),
        StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        font,
        if enabled { TEXT } else { TEXT_OFF },
    );
    response
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
