//! Layer > Lock Layers... (Cmd+/): the locks of the selected layers, laid
//! out at the positions measured on Photoshop 2026's dialog (269 × 206 pt).
//! Sizes are in Photoshop points from the dialog's top-left corner.

use egui::{Color32, Key, Rect, Sense, Ui, vec2};

use op_core::Locks;

use super::common;
use crate::ps_icons::Icon;
use crate::theme::{self, pt};

const SIZE: egui::Vec2 = vec2(pt(269.0), pt(206.0));

/// A row: its center, the icon's x, y offset, kind and scale, the label,
/// and the lock it sets (none for All).
type Row<'a> = (f32, f32, f32, Icon, f32, &'a str, Option<&'a mut bool>);

pub enum Outcome {
    Open,
    Cancel,
    Apply(Locks),
}

pub struct LockLayersDialog {
    locks: Locks,
}

impl LockLayersDialog {
    /// Starts from the locks every selected layer has.
    pub fn new(locks: Locks) -> Self {
        Self { locks }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("lock-layers"))
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
        common::frame(ui, frame, "Lock Layers", theme::dialog_bold(pt(14.0)));
        let all = self.locks.all;
        // Rows: center, icon (x, y offset, scale), label; "All" ticks (and
        // greys) the rest
        let rows: [Row; 5] = [
            (
                60.0,
                27.75,
                0.0,
                Icon::LockTransparent,
                0.5,
                "Transparency",
                Some(&mut self.locks.transparency),
            ),
            (
                86.0,
                27.75,
                1.0,
                Icon::LockPixels,
                0.5,
                "Image",
                Some(&mut self.locks.pixels),
            ),
            (
                112.0,
                27.75,
                0.0,
                Icon::LockPosition,
                0.45,
                "Position",
                Some(&mut self.locks.position),
            ),
            (
                138.0,
                25.0,
                0.75,
                Icon::LockArtboards,
                0.45,
                "Prevent auto-nest",
                Some(&mut self.locks.nesting),
            ),
            (174.0, 28.0, 0.0, Icon::LockAll, 0.5, "All", None),
        ];
        let mut toggle_all = false;
        for (cy, icon_x, icon_dy, icon, scale, label, flag) in rows {
            crate::ps_icons::paint_scaled(
                ui.painter(),
                at(icon_x, cy - 4.25 + icon_dy),
                icon,
                Color32::from_gray(0xf1),
                theme::color::PANEL,
                scale,
            );
            let min = at(52.0, cy - 6.0);
            let font = theme::body();
            // The panel font's labels sit 1 pt lower than the box's middle
            let drop = pt(1.0);
            match flag {
                Some(flag) => {
                    let mut on = *flag || all;
                    common::ps_checkbox_with(ui, min, label, &mut on, !all, font, drop);
                    if !all {
                        *flag = on;
                    }
                }
                None => {
                    let mut on = all;
                    common::ps_checkbox_with(ui, min, label, &mut on, true, font, drop);
                    toggle_all = on != all;
                }
            }
        }
        if toggle_all {
            self.locks.all = !all;
        }

        let ok = common::ps_focused_button(
            ui,
            Rect::from_min_max(at(179.0, 48.0), at(249.0, 72.0)),
            "OK",
            theme::semibold(pt(12.0)),
        );
        let cancel = common::ps_button_with(
            ui,
            Rect::from_min_max(at(179.0, 84.0), at(249.0, 108.0)),
            "Cancel",
            false,
            true,
            theme::semibold(pt(12.0)),
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        if ok.clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
            return Outcome::Apply(self.locks);
        }
        Outcome::Open
    }
}
