//! File operations and global shortcuts.

use std::path::PathBuf;

use egui::Key;
use egui_dock::DockState;
use op_core::{Color, DocId, Document};
use op_tools::Tool;

use crate::state::{AppState, DocState};

/// `initial` names the document's first history state ("Open" or "New").
pub fn add_document(app: &mut AppState, dock: &mut DockState<DocId>, doc: Document, initial: &str) {
    let id = doc.id;
    app.docs.insert(id, DocState::new(doc, initial));
    dock.push_to_focused_leaf(id);
    app.active_doc = Some(id);
}

pub fn open_paths(app: &mut AppState, dock: &mut DockState<DocId>, paths: Vec<PathBuf>) {
    for path in paths {
        match op_io::open(&path) {
            Ok(doc) => add_document(app, dock, doc, "Open"),
            Err(e) => {
                log::error!("{}: {e}", path.display());
                app.alert = Some(format!("Could not open “{}”: {e}", path.display()));
            }
        }
    }
}

pub fn open_dialog(app: &mut AppState, dock: &mut DockState<DocId>) {
    let paths = rfd::FileDialog::new()
        .add_filter("Images", op_io::OPEN_EXTENSIONS)
        .pick_files();
    if let Some(paths) = paths {
        open_paths(app, dock, paths);
    }
}

pub fn new_document(app: &mut AppState, dock: &mut DockState<DocId>) {
    app.untitled_counter += 1;
    let doc = Document::new_with_background(
        format!("Untitled-{}", app.untitled_counter),
        1920,
        1080,
        Color::WHITE,
    );
    add_document(app, dock, doc, "New");
}

pub fn export_dialog(app: &mut AppState) {
    let Some(state) = app.active() else {
        return;
    };
    let stem = state
        .doc
        .title
        .rsplit_once('.')
        .map_or(state.doc.title.as_str(), |(s, _)| s)
        .to_owned();
    let path = rfd::FileDialog::new()
        .set_file_name(format!("{stem}.png"))
        .add_filter("PNG", &["png"])
        .add_filter("JPEG", &["jpg", "jpeg"])
        .save_file();
    if let Some(path) = path
        && let Err(e) = op_io::export_composite(&state.doc, &path)
    {
        app.alert = Some(format!("Could not export: {e}"));
    }
}

pub fn close_active(app: &mut AppState, dock: &mut DockState<DocId>) {
    let Some(id) = app.active_doc else {
        return;
    };
    if let Some(path) = dock.find_tab(&id) {
        dock.remove_tab(path);
    }
    app.docs.remove(&id);
    app.active_doc = None;
}

/// Single-key tool and color shortcuts. Modifier shortcuts are commands; see
/// [`crate::commands`].
pub fn handle_tool_keys(ctx: &egui::Context, app: &mut AppState) {
    if app.modal_open() {
        return;
    }
    // Single-key shortcuts are ignored while a text field has focus
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let keys: Vec<Key> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if !modifiers.command && !modifiers.ctrl && !modifiers.alt => Some(*key),
                _ => None,
            })
            .collect()
    });
    for key in keys {
        match key {
            Key::D => crate::toolbar::reset_colors(app),
            Key::X => crate::toolbar::swap_colors(app),
            _ => {
                let name = key.name();
                if name.len() == 1
                    && let Some(tool) = Tool::from_shortcut(name.chars().next().unwrap())
                {
                    app.tool = tool;
                }
            }
        }
    }
}
