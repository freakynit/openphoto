//! Application commands. Menu items and keyboard shortcuts both resolve to a
//! [`Command`], which is executed in one place.

use egui::{Key, Modifiers};

use crate::actions;
use crate::document_view;
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    New,
    Open,
    Close,
    CloseAll,
    CloseOthers,
    ExportAs,
    Undo,
    Redo,
    ToggleLastState,
    CanvasSize,
    NewLayer,
    DeleteLayer,
    /// Layer > Hide Layers / Show Layers for the active layer.
    ToggleLayerVisibility,
    /// Edit > Fill... (opens the dialog).
    Fill,
    /// Option+Delete: fill with the foreground color.
    FillForeground,
    /// Command+Delete: fill with the background color.
    FillBackground,
    /// Edit > Clear (Delete).
    Clear,
    SelectAll,
    Deselect,
    Reselect,
    SelectInverse,
    ZoomIn,
    ZoomOut,
    FitOnScreen,
    ActualPixels,
    ToggleHistory,
}

/// A keyboard shortcut. `cmd` is Command on macOS and Ctrl elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: Key,
}

const fn cmd(key: Key) -> Shortcut {
    Shortcut {
        cmd: true,
        shift: false,
        alt: false,
        key,
    }
}

const fn shift_cmd(key: Key) -> Shortcut {
    Shortcut {
        shift: true,
        ..cmd(key)
    }
}

const fn alt_cmd(key: Key) -> Shortcut {
    Shortcut {
        alt: true,
        ..cmd(key)
    }
}

impl Shortcut {
    fn modifiers(self) -> Modifiers {
        let mut m = Modifiers::NONE;
        m.command = self.cmd;
        m.shift = self.shift;
        m.alt = self.alt;
        m
    }

    /// Accelerator string for native menus, e.g. `CmdOrCtrl+Alt+C`.
    pub fn accelerator(self) -> String {
        let mut s = String::new();
        if self.cmd {
            s.push_str("CmdOrCtrl+");
        }
        if self.shift {
            s.push_str("Shift+");
        }
        if self.alt {
            s.push_str("Alt+");
        }
        s.push_str(self.key.name());
        s
    }
}

impl Command {
    /// Shortcuts match Photoshop's defaults (read from Photoshop 2026's menus).
    pub fn shortcut(self) -> Option<Shortcut> {
        Some(match self {
            Self::New => cmd(Key::N),
            Self::Open => cmd(Key::O),
            Self::Close => cmd(Key::W),
            Self::CloseAll => alt_cmd(Key::W),
            Self::CloseOthers => alt_cmd(Key::P),
            Self::ExportAs => Shortcut {
                shift: true,
                ..alt_cmd(Key::W)
            },
            Self::Undo => cmd(Key::Z),
            Self::Redo => shift_cmd(Key::Z),
            Self::ToggleLastState => alt_cmd(Key::Z),
            Self::CanvasSize => alt_cmd(Key::C),
            Self::NewLayer => shift_cmd(Key::N),
            Self::ToggleLayerVisibility => cmd(Key::Comma),
            Self::Fill => Shortcut {
                cmd: false,
                shift: true,
                alt: false,
                key: Key::F5,
            },
            Self::FillForeground => Shortcut {
                cmd: false,
                shift: false,
                alt: true,
                key: Key::Backspace,
            },
            Self::FillBackground => cmd(Key::Backspace),
            Self::Clear => Shortcut {
                cmd: false,
                shift: false,
                alt: false,
                key: Key::Backspace,
            },
            Self::SelectAll => cmd(Key::A),
            Self::Deselect => cmd(Key::D),
            Self::Reselect => shift_cmd(Key::D),
            Self::SelectInverse => shift_cmd(Key::I),
            // Photoshop shows Cmd++ and also accepts Cmd+=
            Self::ZoomIn => cmd(Key::Equals),
            Self::ZoomOut => cmd(Key::Minus),
            Self::FitOnScreen => cmd(Key::Num0),
            Self::ActualPixels => cmd(Key::Num1),
            Self::DeleteLayer | Self::ToggleHistory => return None,
        })
    }

    /// Whether the command can run in the current state (also drives menu
    /// item enabled states).
    pub fn enabled(self, app: &AppState) -> bool {
        // Menus are disabled while a modal dialog is open, as in Photoshop
        if app.modal_open() {
            return false;
        }
        let doc = app.active_doc.and_then(|id| app.docs.get(&id));
        match self {
            Self::New | Self::Open | Self::ToggleHistory => true,
            Self::Undo | Self::ToggleLastState => doc.is_some_and(|d| d.history.can_undo()),
            Self::Redo => doc.is_some_and(|d| d.history.can_redo()),
            Self::DeleteLayer => doc.is_some_and(|d| d.doc.layers.len() > 1),
            Self::CloseOthers => app.docs.len() > 1,
            Self::Deselect | Self::SelectInverse => {
                doc.is_some_and(|d| d.doc.selection().is_some())
            }
            Self::Reselect => doc.is_some_and(|d| d.doc.can_reselect()),
            Self::ToggleLayerVisibility => doc
                .and_then(|d| d.doc.active_layer.and_then(|id| d.doc.layer(id)))
                .is_some(),
            Self::Close
            | Self::Fill
            | Self::FillForeground
            | Self::FillBackground
            | Self::Clear
            | Self::SelectAll
            | Self::CloseAll
            | Self::ExportAs
            | Self::CanvasSize
            | Self::NewLayer
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::FitOnScreen
            | Self::ActualPixels => doc.is_some(),
        }
    }
}

/// Commands that have keyboard shortcuts, most specific first: egui ignores
/// extra Shift/Alt when matching, so Shift+Cmd+Z must be checked before Cmd+Z.
const SHORTCUT_ORDER: &[Command] = &[
    Command::ExportAs,
    Command::Fill,
    Command::FillForeground,
    Command::FillBackground,
    Command::Reselect,
    Command::SelectInverse,
    Command::Redo,
    Command::ToggleLastState,
    Command::NewLayer,
    Command::CanvasSize,
    Command::CloseAll,
    Command::CloseOthers,
    Command::Undo,
    Command::New,
    Command::Open,
    Command::Close,
    Command::ToggleLayerVisibility,
    Command::SelectAll,
    Command::Deselect,
    Command::ZoomIn,
    Command::ZoomOut,
    Command::FitOnScreen,
    Command::ActualPixels,
];

/// Shortcuts the macOS menu bar can't catch. Zoom In's menu item shows Cmd++
/// like Photoshop, which macOS only matches with Shift held, so the plain
/// Cmd+= key is handled here.
#[cfg(target_os = "macos")]
pub fn from_shortcuts_beside_menu(ctx: &egui::Context) -> Vec<Command> {
    let hit = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Equals));
    if hit {
        vec![Command::ZoomIn]
    } else {
        Vec::new()
    }
}

/// Command shortcuts typed into egui, used where there is no native menu bar
/// (other platforms, and headless tests on macOS). On macOS the native menu bar handles
/// these instead, so this is only used on other platforms.
pub fn from_shortcuts(ctx: &egui::Context) -> Vec<Command> {
    let mut out = Vec::new();
    let typing = ctx.egui_wants_keyboard_input();
    ctx.input_mut(|i| {
        for &command in SHORTCUT_ORDER {
            // Delete-key fills would eat text editing keys
            if typing && matches!(command, Command::FillForeground | Command::FillBackground) {
                continue;
            }
            let s = command.shortcut().expect("listed commands have shortcuts");
            let mut hit = i.consume_key(s.modifiers(), s.key);
            // Cmd+Shift+= arrives as Cmd+Plus on some layouts
            if command == Command::ZoomIn {
                hit |= i.consume_key(s.modifiers(), Key::Plus);
            }
            if hit {
                out.push(command);
            }
        }
        if !typing
            && (i.consume_key(Modifiers::NONE, Key::Backspace)
                || i.consume_key(Modifiers::NONE, Key::Delete))
        {
            out.push(Command::Clear);
        }
    });
    out
}

pub fn run(command: Command, ctx: &egui::Context, app: &mut AppState) {
    if !command.enabled(app) {
        return;
    }
    let ppp = ctx.pixels_per_point();
    match command {
        Command::New => actions::new_document(app),
        Command::Open => actions::open_dialog(app),
        Command::Close => actions::close_active(app),
        Command::CloseAll => actions::close_all(app),
        Command::CloseOthers => actions::close_others(app),
        Command::ExportAs => actions::export_dialog(app),
        Command::ToggleHistory => app.history_open = !app.history_open,
        Command::Fill => app.fill_dialog = Some(Default::default()),
        Command::FillForeground | Command::FillBackground => {
            let color = if command == Command::FillForeground {
                app.foreground
            } else {
                app.background
            };
            let [r, g, b, _] = color.to_rgba8();
            if let Some(state) = app.active() {
                let result = op_core::fill::fill(&mut state.doc, [r, g, b], Default::default());
                match result {
                    Ok(()) => state.record("Fill"),
                    Err(e) => app.alert = Some(e.message("Fill")),
                }
            }
        }
        Command::Clear => {
            let [r, g, b, _] = app.background.to_rgba8();
            if let Some(state) = app.active() {
                if state.doc.selection().is_none() {
                    // Without a pixel selection, Delete removes the layer
                    if state.doc.layers.len() > 1 {
                        crate::panels::delete_active_layer(state);
                    }
                } else {
                    match op_core::fill::clear(&mut state.doc, [r, g, b]) {
                        Ok(()) => state.record("Clear"),
                        Err(e) => app.alert = Some(e.message("Clear")),
                    }
                }
            }
        }
        Command::CanvasSize => {
            if let Some(state) = app.active() {
                let dialog = crate::dialogs::CanvasSizeDialog::new(&state.doc);
                app.canvas_size_dialog = Some(dialog);
            }
        }
        _ => {
            let Some(state) = app.active() else {
                return;
            };
            match command {
                Command::Undo => {
                    state.undo();
                }
                Command::Redo => {
                    state.redo();
                }
                Command::ToggleLastState => {
                    state.toggle_last_state();
                }
                Command::ToggleLayerVisibility => crate::panels::toggle_active_visibility(state),
                Command::SelectAll => {
                    let (w, h) = (state.doc.width, state.doc.height);
                    state.doc.set_selection(Some(op_core::Selection::all(w, h)));
                    state.record("Select All");
                }
                Command::Deselect => {
                    state.doc.set_selection(None);
                    state.record("Deselect");
                }
                Command::Reselect => {
                    state.doc.reselect();
                    state.record("Reselect");
                }
                Command::SelectInverse => {
                    let inverse = state.doc.selection().map(|s| s.inverse());
                    state.doc.set_selection(inverse);
                    state.record("Select Inverse");
                }
                Command::NewLayer => crate::panels::new_layer(state),
                Command::DeleteLayer => crate::panels::delete_active_layer(state),
                Command::ZoomIn => document_view::zoom_step(state, true, ppp),
                Command::ZoomOut => document_view::zoom_step(state, false, ppp),
                Command::FitOnScreen => document_view::fit_on_screen(state, ppp),
                Command::ActualPixels => document_view::actual_pixels(state, ppp),
                _ => unreachable!("handled above"),
            }
        }
    }
}
