//! Application commands. Menu items and keyboard shortcuts both resolve to a
//! [`Command`], which is executed in one place.

use egui::{Key, Modifiers};

use op_core::adjust::{self, Adjustment};
use op_core::image_ops::{self, Orientation};
use op_core::layer_ops::{self, Arrange};

use crate::actions;
use crate::dialogs::{AdjustDialog, AdjustKind};
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
    Rotate180,
    Rotate90Clockwise,
    Rotate90CounterClockwise,
    FlipCanvasHorizontal,
    FlipCanvasVertical,
    /// Image > Crop (to the selection).
    Crop,
    /// Image > Trim... (opens the dialog).
    Trim,
    Invert,
    Desaturate,
    Equalize,
    /// Image > Adjustments > Threshold... (opens the dialog).
    Threshold,
    /// Image > Adjustments > Posterize... (opens the dialog).
    Posterize,
    NewLayer,
    DeleteLayer,
    /// Layer > Hide Layers / Show Layers for the active layer.
    ToggleLayerVisibility,
    DuplicateLayer,
    LayerViaCopy,
    LayerViaCut,
    LayerFromBackground,
    DeleteHiddenLayers,
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
    MergeDown,
    MergeVisible,
    FlattenImage,
    /// Edit > Fill... (opens the dialog).
    Fill,
    /// Option+Delete: fill with the foreground color.
    FillForeground,
    /// Command+Delete: fill with the background color.
    FillBackground,
    /// Edit > Clear (Delete).
    Clear,
    Cut,
    Copy,
    CopyMerged,
    Paste,
    /// Edit > Paste Special > Paste in Place.
    PasteInPlace,
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
        // muda knows the bracket keys by their symbols
        s.push_str(match self.key {
            Key::OpenBracket => "[",
            Key::CloseBracket => "]",
            key => key.name(),
        });
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
            Self::LayerViaCopy => cmd(Key::J),
            Self::LayerViaCut => shift_cmd(Key::J),
            Self::BringToFront => shift_cmd(Key::CloseBracket),
            Self::BringForward => cmd(Key::CloseBracket),
            Self::SendBackward => cmd(Key::OpenBracket),
            Self::SendToBack => shift_cmd(Key::OpenBracket),
            Self::MergeDown => cmd(Key::E),
            Self::MergeVisible => shift_cmd(Key::E),
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
            Self::Cut => cmd(Key::X),
            Self::Copy => cmd(Key::C),
            Self::CopyMerged => shift_cmd(Key::C),
            Self::Paste => cmd(Key::V),
            Self::PasteInPlace => shift_cmd(Key::V),
            Self::SelectAll => cmd(Key::A),
            Self::Deselect => cmd(Key::D),
            Self::Reselect => shift_cmd(Key::D),
            Self::SelectInverse => shift_cmd(Key::I),
            // Photoshop shows Cmd++ and also accepts Cmd+=
            Self::ZoomIn => cmd(Key::Equals),
            Self::ZoomOut => cmd(Key::Minus),
            Self::FitOnScreen => cmd(Key::Num0),
            Self::ActualPixels => cmd(Key::Num1),
            Self::Rotate180
            | Self::Rotate90Clockwise
            | Self::Rotate90CounterClockwise
            | Self::FlipCanvasHorizontal
            | Self::FlipCanvasVertical
            | Self::Crop
            | Self::Trim
            | Self::Equalize
            | Self::Threshold
            | Self::Posterize => return None,
            Self::Invert => cmd(Key::I),
            Self::Desaturate => shift_cmd(Key::U),
            Self::DeleteLayer
            | Self::ToggleHistory
            | Self::DuplicateLayer
            | Self::LayerFromBackground
            | Self::DeleteHiddenLayers
            | Self::FlattenImage => return None,
        })
    }

    fn orientation(self) -> Option<Orientation> {
        Some(match self {
            Self::Rotate180 => Orientation::Rotate180,
            Self::Rotate90Clockwise => Orientation::Rotate90Clockwise,
            Self::Rotate90CounterClockwise => Orientation::Rotate90CounterClockwise,
            Self::FlipCanvasHorizontal => Orientation::FlipHorizontal,
            Self::FlipCanvasVertical => Orientation::FlipVertical,
            _ => return None,
        })
    }

    fn arrange(self) -> Option<Arrange> {
        Some(match self {
            Self::BringToFront => Arrange::BringToFront,
            Self::BringForward => Arrange::BringForward,
            Self::SendBackward => Arrange::SendBackward,
            Self::SendToBack => Arrange::SendToBack,
            _ => return None,
        })
    }

    fn is_clipboard(self) -> bool {
        matches!(
            self,
            Self::Cut | Self::Copy | Self::CopyMerged | Self::Paste | Self::PasteInPlace
        )
    }

    /// Whether the command can run in the current state (also drives menu
    /// item enabled states).
    pub fn enabled(self, app: &AppState) -> bool {
        // With a text field focused, Cut/Copy/Paste edit its text (even in
        // a dialog)
        if app.typing && self.is_clipboard() {
            return true;
        }
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
            Self::ToggleLayerVisibility | Self::DuplicateLayer | Self::LayerViaCopy => doc
                .and_then(|d| d.doc.active_layer.and_then(|id| d.doc.layer(id)))
                .is_some(),
            Self::LayerViaCut => {
                doc.is_some_and(|d| d.doc.selection().is_some() && d.doc.active_layer.is_some())
            }
            Self::Crop => doc.is_some_and(|d| d.doc.selection().is_some()),
            Self::LayerFromBackground => doc.is_some_and(|d| d.doc.has_background()),
            Self::DeleteHiddenLayers => doc.is_some_and(|d| {
                let layers = &d.doc.layers;
                layers.iter().any(|l| !l.visible) && layers.iter().any(|l| l.visible)
            }),
            Self::BringToFront | Self::BringForward | Self::SendBackward | Self::SendToBack => doc
                .is_some_and(|d| {
                    let arrange = self.arrange().expect("arrange command");
                    layer_ops::arrange_target(&d.doc, arrange).is_some()
                }),
            Self::MergeDown => doc.is_some_and(|d| layer_ops::can_merge_down(&d.doc)),
            Self::MergeVisible => doc.is_some_and(|d| layer_ops::can_merge_visible(&d.doc)),
            Self::FlattenImage => doc.is_some_and(|d| {
                let layers = &d.doc.layers;
                !(layers.len() == 1 && layers[0].is_background)
            }),
            Self::Close
            | Self::Fill
            | Self::FillForeground
            | Self::FillBackground
            | Self::Clear
            | Self::Cut
            | Self::Copy
            | Self::CopyMerged
            | Self::Paste
            | Self::PasteInPlace
            | Self::SelectAll
            | Self::CloseAll
            | Self::ExportAs
            | Self::CanvasSize
            | Self::Rotate180
            | Self::Rotate90Clockwise
            | Self::Rotate90CounterClockwise
            | Self::FlipCanvasHorizontal
            | Self::FlipCanvasVertical
            | Self::Trim
            | Self::Invert
            | Self::Desaturate
            | Self::Equalize
            | Self::Threshold
            | Self::Posterize
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
    Command::CopyMerged,
    Command::PasteInPlace,
    Command::LayerViaCut,
    Command::BringToFront,
    Command::SendToBack,
    Command::MergeVisible,
    Command::Fill,
    Command::FillForeground,
    Command::FillBackground,
    Command::Reselect,
    Command::SelectInverse,
    Command::Desaturate,
    Command::Redo,
    Command::ToggleLastState,
    Command::NewLayer,
    Command::CanvasSize,
    Command::CloseAll,
    Command::CloseOthers,
    Command::Undo,
    Command::Invert,
    Command::LayerViaCopy,
    Command::BringForward,
    Command::SendBackward,
    Command::MergeDown,
    Command::Cut,
    Command::Copy,
    Command::Paste,
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
            // Delete-key fills would eat text editing keys, and text fields
            // handle their own Cut/Copy/Paste
            if typing
                && (command.is_clipboard()
                    || matches!(command, Command::FillForeground | Command::FillBackground))
            {
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
        // egui-winit turns Cmd+X/C/V into these events instead of key presses
        if !typing {
            let shift = i.modifiers.shift;
            for event in &i.events {
                out.push(match event {
                    egui::Event::Cut => Command::Cut,
                    egui::Event::Copy if shift => Command::CopyMerged,
                    egui::Event::Copy => Command::Copy,
                    egui::Event::Paste(_) if shift => Command::PasteInPlace,
                    egui::Event::Paste(_) => Command::Paste,
                    _ => continue,
                });
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
        Command::Cut
        | Command::Copy
        | Command::CopyMerged
        | Command::Paste
        | Command::PasteInPlace => actions::clipboard(command, app, ppp),
        Command::LayerViaCopy | Command::LayerViaCut => {
            let [r, g, b, _] = app.background.to_rgba8();
            let Some(state) = app.active_doc.and_then(|id| app.docs.get_mut(&id)) else {
                return;
            };
            let (name, result) = if command == Command::LayerViaCopy {
                ("Layer Via Copy", layer_ops::via_copy(&mut state.doc))
            } else {
                (
                    "Layer Via Cut",
                    layer_ops::via_cut(&mut state.doc, [r, g, b]),
                )
            };
            match result {
                Ok(_) => state.record(name),
                Err(e) => app.alert = Some(e.message(name)),
            }
        }
        Command::Invert | Command::Desaturate | Command::Equalize => {
            let adjustment = match command {
                Command::Invert => Adjustment::Invert,
                Command::Desaturate => Adjustment::Desaturate,
                _ => Adjustment::Equalize,
            };
            if let Some(state) = app.active_doc.and_then(|id| app.docs.get_mut(&id)) {
                match adjust::apply(&mut state.doc, adjustment) {
                    Ok(()) => state.record(adjustment.name()),
                    Err(e) => app.alert = Some(e.message(adjustment.name())),
                }
            }
        }
        Command::Threshold | Command::Posterize => {
            let (kind, name) = if command == Command::Threshold {
                (AdjustKind::Threshold, "Threshold")
            } else {
                (AdjustKind::Posterize, "Posterize")
            };
            if let Some(state) = app.active_doc.and_then(|id| app.docs.get_mut(&id)) {
                match adjust::check(&state.doc) {
                    Ok(()) => {
                        let histogram = adjust::luminosity_histogram(&state.doc);
                        let before = state.doc.snapshot();
                        app.adjust_dialog = Some(AdjustDialog::new(kind, histogram, before));
                    }
                    Err(e) => app.alert = Some(e.message(name)),
                }
            }
        }
        Command::Trim => {
            if let Some(state) = app.active() {
                let dialog = crate::dialogs::TrimDialog::new(state.doc.has_background());
                app.trim_dialog = Some(dialog);
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
                Command::Rotate180
                | Command::Rotate90Clockwise
                | Command::Rotate90CounterClockwise
                | Command::FlipCanvasHorizontal
                | Command::FlipCanvasVertical => {
                    let orientation = command.orientation().expect("rotation command");
                    image_ops::reorient(&mut state.doc, orientation);
                    state.record(orientation.history_name());
                }
                Command::Crop => {
                    if image_ops::crop_to_selection(&mut state.doc) {
                        state.record("Crop");
                    }
                }
                Command::NewLayer => crate::panels::new_layer(state),
                Command::DuplicateLayer => {
                    if layer_ops::duplicate(&mut state.doc).is_some() {
                        state.record("Duplicate Layer");
                    }
                }
                Command::LayerFromBackground => {
                    if layer_ops::layer_from_background(&mut state.doc) {
                        state.record("Layer From Background");
                    }
                }
                Command::DeleteHiddenLayers => {
                    if layer_ops::delete_hidden(&mut state.doc) {
                        state.record("Delete Hidden Layers");
                    }
                }
                Command::BringToFront
                | Command::BringForward
                | Command::SendBackward
                | Command::SendToBack => {
                    let arrange = command.arrange().expect("arrange command");
                    if layer_ops::arrange(&mut state.doc, arrange) {
                        state.record("Layer Order");
                    }
                }
                Command::MergeDown => {
                    if layer_ops::merge_down(&mut state.doc) {
                        state.record("Merge Down");
                    }
                }
                Command::MergeVisible => {
                    if layer_ops::merge_visible(&mut state.doc) {
                        state.record("Merge Visible");
                    }
                }
                Command::FlattenImage => {
                    layer_ops::flatten(&mut state.doc);
                    state.record("Flatten Image");
                }
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
