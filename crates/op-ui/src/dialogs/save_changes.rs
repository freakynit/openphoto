//! The "Save changes?" prompt shown when closing a document (or quitting)
//! with unsaved changes. Like Photoshop's: the question, then Don't Save on
//! the left and Cancel and Save on the right. Enter means Save, Escape
//! Cancel and Cmd+D Don't Save.

use egui::{Color32, FontId, Key, Modifiers, Rect, Sense, Ui, vec2};

use super::common;
use crate::theme::{self, color, pt};

const SIZE: egui::Vec2 = vec2(pt(420.0), pt(150.0));
const FONT: f32 = pt(13.0);
const BUTTON: egui::Vec2 = vec2(pt(96.0), pt(24.0));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveChoice {
    Save,
    DontSave,
    Cancel,
}

/// Shows the prompt for the document titled `title`; returns the answer
/// once given.
pub fn show(ctx: &egui::Context, title: &str) -> Option<SaveChoice> {
    let mut choice = None;
    egui::Modal::new(egui::Id::new("save-changes"))
        .frame(egui::Frame::NONE)
        .backdrop_color(Color32::TRANSPARENT)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(SIZE, Sense::hover());
            choice = prompt(ui, rect, title);
        });
    ctx.input_mut(|i| {
        if i.consume_key(Modifiers::NONE, Key::Escape) {
            choice = Some(SaveChoice::Cancel);
        } else if i.consume_key(Modifiers::NONE, Key::Enter) {
            choice = Some(SaveChoice::Save);
        } else if i.consume_key(Modifiers::COMMAND, Key::D) {
            // macOS's shortcut for "Don't Save"
            choice = Some(SaveChoice::DontSave);
        }
    });
    choice
}

fn prompt(ui: &mut Ui, frame: Rect, title: &str) -> Option<SaveChoice> {
    let at = |x: f32, y: f32| frame.min + vec2(x, y);
    common::frame(ui, frame, "OpenPhoto", theme::semibold(pt(13.0)));
    let text = format!("Save changes to the OpenPhoto document “{title}” before closing?");
    let galley = ui.painter().layout(
        text,
        FontId::proportional(FONT),
        color::TEXT,
        frame.width() - pt(48.0),
    );
    ui.painter()
        .galley(at(pt(24.0), pt(48.0)), galley, color::TEXT);

    let y = frame.height() - pt(24.0) - BUTTON.y;
    let font = FontId::proportional(FONT);
    let dont = common::pill_button(
        ui,
        Rect::from_min_size(at(pt(24.0), y), BUTTON),
        "Don't Save",
        font.clone(),
        true,
    );
    let cancel = common::pill_button(
        ui,
        Rect::from_min_size(
            at(frame.width() - pt(24.0) - BUTTON.x * 2.0 - pt(10.0), y),
            BUTTON,
        ),
        "Cancel",
        font.clone(),
        true,
    );
    let save = common::pill_button(
        ui,
        Rect::from_min_size(at(frame.width() - pt(24.0) - BUTTON.x, y), BUTTON),
        "Save",
        font,
        true,
    );
    if dont.clicked() {
        Some(SaveChoice::DontSave)
    } else if cancel.clicked() {
        Some(SaveChoice::Cancel)
    } else if save.clicked() {
        Some(SaveChoice::Save)
    } else {
        None
    }
}
