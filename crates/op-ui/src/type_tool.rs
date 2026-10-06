//! The Horizontal Type tool (T) on the canvas.
//!
//! A click places the insertion point (the first line's baseline); typed
//! text shows on the document right away as a new layer in the foreground
//! color. Enter starts a new line, Backspace deletes, Escape cancels, and
//! Cmd+Enter (or a click elsewhere on the canvas) commits, as in Photoshop.

use egui::{Color32, Key, Modifiers, Pos2, Rect, Stroke, Ui};

use crate::document_view::{to_doc, to_screen};
use crate::state::{DocState, TextEdit, TypeOptions};

/// Text size in document pixels: points at the document's resolution.
fn size_px(state: &DocState, options: TypeOptions) -> f32 {
    options.size_pt * state.doc.resolution / 72.0
}

/// Handles the Type tool's input. Returns true when text was committed.
pub fn input(
    ui: &Ui,
    response: &egui::Response,
    state: &mut DocState,
    options: TypeOptions,
    color: [u8; 3],
    ppp: f32,
) -> bool {
    let mut committed = false;
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        if state.text_edit.is_some() {
            // A click elsewhere ends the text being typed
            committed = commit(state);
        } else {
            state.text_edit = Some(TextEdit {
                origin: to_doc(state, p, ppp),
                text: String::new(),
                before: state.doc.snapshot(),
                shown: String::new(),
            });
        }
    }
    if state.text_edit.is_none() {
        return committed;
    }

    // Keys typed into a text field (such as the size in the options bar)
    // are not for the text on the canvas
    if ui.ctx().egui_wants_keyboard_input() {
        preview(state, options, color);
        return committed;
    }
    // Typed characters and editing keys go into the text
    let (typed, backspace, newline, escape, commit_key) = ui.input_mut(|i| {
        let mut typed = String::new();
        i.events.retain(|e| match e {
            egui::Event::Text(t) => {
                typed.push_str(t);
                false
            }
            _ => true,
        });
        (
            typed,
            i.consume_key(Modifiers::NONE, Key::Backspace),
            i.consume_key(Modifiers::NONE, Key::Enter),
            i.consume_key(Modifiers::NONE, Key::Escape),
            i.consume_key(Modifiers::COMMAND, Key::Enter),
        )
    });
    if escape {
        cancel(state);
        return committed;
    }
    if commit_key {
        return commit(state) || committed;
    }
    if let Some(edit) = &mut state.text_edit {
        edit.text.push_str(&typed);
        if backspace {
            edit.text.pop();
        }
        if newline {
            edit.text.push('\n');
        }
    }
    preview(state, options, color);
    committed
}

/// Shows the current text on the document as a layer.
pub fn preview(state: &mut DocState, options: TypeOptions, color: [u8; 3]) {
    let size = size_px(state, options);
    let Some(edit) = &state.text_edit else {
        return;
    };
    if edit.text == edit.shown {
        return;
    }
    let (text, origin) = (edit.text.clone(), edit.origin);
    state.doc.restore(&edit.before);
    let _ = op_core::text::add_text_layer(
        &mut state.doc,
        options.font(),
        &text,
        size,
        (origin.x, origin.y),
        color,
    );
    if let Some(edit) = &mut state.text_edit {
        edit.shown = text;
    }
}

/// Keeps the text ("Type Tool" in the history). Empty text leaves nothing.
pub fn commit(state: &mut DocState) -> bool {
    let Some(edit) = state.text_edit.take() else {
        return false;
    };
    if edit.shown.trim().is_empty() {
        state.doc.restore(&edit.before);
        return false;
    }
    state.record("Type Tool");
    true
}

/// Drops the text being typed.
pub fn cancel(state: &mut DocState) {
    if let Some(edit) = state.text_edit.take() {
        state.doc.restore(&edit.before);
    }
}

/// The blinking caret after the last character.
pub fn draw_caret(ui: &Ui, state: &DocState, options: TypeOptions, canvas: Rect, ppp: f32) {
    let Some(edit) = &state.text_edit else {
        return;
    };
    let size = size_px(state, options);
    let Some(((x, y), line)) = op_core::text::caret(
        options.font(),
        &edit.text,
        size,
        (edit.origin.x, edit.origin.y),
    ) else {
        return;
    };
    // Blink every half second
    let time = ui.input(|i| i.time);
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(500));
    if (time * 2.0) as i64 % 2 == 1 {
        return;
    }
    let top = to_screen(state, Pos2::new(x, y - line * 0.75), ppp);
    let bottom = to_screen(state, Pos2::new(x, y + line * 0.2), ppp);
    // Black with a white core, visible on any color
    let painter = ui.painter_at(canvas);
    painter.line_segment([top, bottom], Stroke::new(3.0, Color32::BLACK));
    painter.line_segment([top, bottom], Stroke::new(1.0, Color32::WHITE));
}
