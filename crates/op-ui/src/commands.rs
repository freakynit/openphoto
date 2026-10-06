//! Application commands. Menu items and keyboard shortcuts both resolve to a
//! [`Command`], which is executed in one place.

use egui::{Key, Modifiers};

use op_core::adjust::{self, Adjustment};
use op_core::filter::Filter;
use op_core::image_ops::{self, Orientation};
use op_core::layer_ops::{self, Arrange};
use op_core::transform::{self, FixedTransform};

use crate::actions;
use crate::dialogs::{AdjustDialog, AdjustKind, ModifyKind};
use crate::document_view;
use crate::state::AppState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    New,
    Open,
    Close,
    CloseAll,
    CloseOthers,
    Save,
    SaveAs,
    SaveACopy,
    /// File > Revert (F12).
    Revert,
    /// Quit OpenPhoto (asks about unsaved changes first).
    Quit,
    ExportAs,
    Undo,
    Redo,
    ToggleLastState,
    CanvasSize,
    ImageSize,
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
    Levels,
    HueSaturation,
    Exposure,
    /// Filter > Last Filter: the last filter again, with its settings.
    LastFilter,
    Average,
    Solarize,
    GaussianBlur,
    BoxBlur,
    UnsharpMask,
    AddNoise,
    Median,
    Minimum,
    Maximum,
    HighPass,
    Offset,
    Mosaic,
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
    FreeTransform,
    /// Edit > Transform > Again: the last transform once more.
    TransformAgain,
    TransformRotate180,
    TransformRotate90Clockwise,
    TransformRotate90CounterClockwise,
    TransformFlipHorizontal,
    TransformFlipVertical,
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
    /// Select > Modify > Border... (and the next four: dialogs).
    ModifyBorder,
    ModifySmooth,
    ModifyExpand,
    ModifyContract,
    /// Select > Modify > Feather... (Shift+F6).
    ModifyFeather,
    Grow,
    Similar,
    ZoomIn,
    ZoomOut,
    FitOnScreen,
    FitLayers,
    ActualPixels,
    Zoom200,
    PrintSize,
    /// View > Rulers.
    ToggleRulers,
    /// View > Extras.
    ToggleExtras,
    /// View > Show > Guides.
    ToggleGuides,
    /// View > Show > Grid.
    ToggleGrid,
    /// View > Guides > Lock Guides.
    LockGuides,
    ClearGuides,
    /// View > Guides > New Guide... (opens the dialog).
    NewGuide,
    /// Hide OpenPhoto (Ctrl+Cmd+H, as Photoshop: Cmd+H is Extras).
    HideApp,
    ToggleHistory,
}

/// A keyboard shortcut. `cmd` is Command on macOS and Ctrl elsewhere.
#[derive(Clone, Copy, Debug)]
pub struct Shortcut {
    pub cmd: bool,
    pub shift: bool,
    pub alt: bool,
    /// The Control key on macOS (where `cmd` is Command).
    pub ctrl: bool,
    pub key: Key,
}

const fn cmd(key: Key) -> Shortcut {
    Shortcut {
        cmd: true,
        shift: false,
        alt: false,
        ctrl: false,
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
        m.ctrl = self.ctrl;
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
        if self.ctrl {
            s.push_str("Ctrl+");
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
            Self::Save => cmd(Key::S),
            Self::SaveAs => shift_cmd(Key::S),
            Self::SaveACopy => alt_cmd(Key::S),
            Self::Revert => Shortcut {
                cmd: false,
                shift: false,
                alt: false,
                ctrl: false,
                key: Key::F12,
            },
            Self::Quit => cmd(Key::Q),
            Self::ExportAs => Shortcut {
                shift: true,
                ..alt_cmd(Key::W)
            },
            Self::Undo => cmd(Key::Z),
            Self::Redo => shift_cmd(Key::Z),
            Self::ToggleLastState => alt_cmd(Key::Z),
            Self::CanvasSize => alt_cmd(Key::C),
            Self::ImageSize => alt_cmd(Key::I),
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
                ctrl: false,
                key: Key::F5,
            },
            Self::FillForeground => Shortcut {
                cmd: false,
                shift: false,
                alt: true,
                ctrl: false,
                key: Key::Backspace,
            },
            Self::FillBackground => cmd(Key::Backspace),
            Self::Clear => Shortcut {
                cmd: false,
                shift: false,
                alt: false,
                ctrl: false,
                key: Key::Backspace,
            },
            Self::FreeTransform => cmd(Key::T),
            Self::TransformAgain => shift_cmd(Key::T),
            Self::Cut => cmd(Key::X),
            Self::Copy => cmd(Key::C),
            Self::CopyMerged => shift_cmd(Key::C),
            Self::Paste => cmd(Key::V),
            Self::PasteInPlace => shift_cmd(Key::V),
            Self::SelectAll => cmd(Key::A),
            Self::Deselect => cmd(Key::D),
            Self::Reselect => shift_cmd(Key::D),
            Self::SelectInverse => shift_cmd(Key::I),
            Self::ModifyFeather => Shortcut {
                cmd: false,
                shift: true,
                alt: false,
                ctrl: false,
                key: Key::F6,
            },
            Self::ModifyBorder
            | Self::ModifySmooth
            | Self::ModifyExpand
            | Self::ModifyContract
            | Self::Grow
            | Self::Similar => return None,
            // Photoshop shows Cmd++ and also accepts Cmd+=
            Self::ZoomIn => cmd(Key::Equals),
            Self::ZoomOut => cmd(Key::Minus),
            Self::FitOnScreen => cmd(Key::Num0),
            Self::ActualPixels => cmd(Key::Num1),
            Self::ToggleRulers => cmd(Key::R),
            Self::ToggleExtras => cmd(Key::H),
            Self::ToggleGuides => cmd(Key::Semicolon),
            Self::ToggleGrid => cmd(Key::Quote),
            Self::LockGuides => alt_cmd(Key::Semicolon),
            Self::HideApp => Shortcut {
                ctrl: true,
                ..cmd(Key::H)
            },
            Self::TransformRotate180
            | Self::TransformRotate90Clockwise
            | Self::TransformRotate90CounterClockwise
            | Self::TransformFlipHorizontal
            | Self::TransformFlipVertical => return None,
            Self::Rotate180
            | Self::Rotate90Clockwise
            | Self::Rotate90CounterClockwise
            | Self::FlipCanvasHorizontal
            | Self::FlipCanvasVertical
            | Self::Crop
            | Self::Trim
            | Self::Equalize
            | Self::Threshold
            | Self::Posterize
            | Self::Exposure
            | Self::Average
            | Self::Solarize
            | Self::GaussianBlur
            | Self::BoxBlur
            | Self::UnsharpMask
            | Self::AddNoise
            | Self::Median
            | Self::Minimum
            | Self::Maximum
            | Self::HighPass
            | Self::Offset
            | Self::Mosaic => return None,
            Self::Invert => cmd(Key::I),
            Self::Levels => cmd(Key::L),
            Self::HueSaturation => cmd(Key::U),
            Self::LastFilter => Shortcut {
                cmd: true,
                shift: false,
                alt: false,
                ctrl: true,
                key: Key::F,
            },
            Self::Desaturate => shift_cmd(Key::U),
            Self::FitLayers
            | Self::Zoom200
            | Self::PrintSize
            | Self::ClearGuides
            | Self::NewGuide => {
                return None;
            }
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

    /// The check mark of View menu switches.
    pub fn checked(self, app: &AppState) -> Option<bool> {
        let v = &app.view;
        Some(match self {
            Self::ToggleRulers => v.rulers,
            Self::ToggleExtras => v.extras,
            Self::ToggleGuides => v.guides,
            Self::ToggleGrid => v.grid,
            Self::LockGuides => v.lock_guides,
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
            Self::New
            | Self::Open
            | Self::ToggleHistory
            | Self::Quit
            | Self::HideApp
            | Self::ToggleRulers
            | Self::ToggleExtras
            | Self::ToggleGuides
            | Self::ToggleGrid
            | Self::LockGuides => true,
            Self::ClearGuides => doc.is_some_and(|d| !d.doc.guides.is_empty()),
            Self::Revert => doc.is_some_and(|d| d.path.is_some() && d.is_dirty()),
            Self::Undo | Self::ToggleLastState => doc.is_some_and(|d| d.history.can_undo()),
            Self::Redo => doc.is_some_and(|d| d.history.can_redo()),
            Self::DeleteLayer => doc.is_some_and(|d| d.doc.layers.len() > 1),
            Self::CloseOthers => app.docs.len() > 1,
            Self::Deselect
            | Self::SelectInverse
            | Self::ModifyBorder
            | Self::ModifySmooth
            | Self::ModifyExpand
            | Self::ModifyContract
            | Self::ModifyFeather
            | Self::Grow
            | Self::Similar => doc.is_some_and(|d| d.doc.selection().is_some()),
            Self::Reselect => doc.is_some_and(|d| d.doc.can_reselect()),
            Self::ToggleLayerVisibility | Self::DuplicateLayer | Self::LayerViaCopy => doc
                .and_then(|d| d.doc.active_layer.and_then(|id| d.doc.layer(id)))
                .is_some(),
            Self::LayerViaCut => {
                doc.is_some_and(|d| d.doc.selection().is_some() && d.doc.active_layer.is_some())
            }
            Self::Crop => doc.is_some_and(|d| d.doc.selection().is_some()),
            Self::TransformAgain => doc.is_some() && app.last_transform.is_some(),
            Self::LastFilter => doc.is_some() && app.last_filter.is_some(),
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
            | Self::Save
            | Self::SaveAs
            | Self::SaveACopy
            | Self::Fill
            | Self::FillForeground
            | Self::FillBackground
            | Self::Clear
            | Self::FreeTransform
            | Self::TransformRotate180
            | Self::TransformRotate90Clockwise
            | Self::TransformRotate90CounterClockwise
            | Self::TransformFlipHorizontal
            | Self::TransformFlipVertical
            | Self::Cut
            | Self::Copy
            | Self::CopyMerged
            | Self::Paste
            | Self::PasteInPlace
            | Self::SelectAll
            | Self::CloseAll
            | Self::ExportAs
            | Self::CanvasSize
            | Self::ImageSize
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
            | Self::Levels
            | Self::HueSaturation
            | Self::Exposure
            | Self::Average
            | Self::Solarize
            | Self::GaussianBlur
            | Self::BoxBlur
            | Self::UnsharpMask
            | Self::AddNoise
            | Self::Median
            | Self::Minimum
            | Self::Maximum
            | Self::HighPass
            | Self::Offset
            | Self::Mosaic
            | Self::NewLayer
            | Self::ZoomIn
            | Self::ZoomOut
            | Self::FitOnScreen
            | Self::FitLayers
            | Self::ActualPixels
            | Self::Zoom200
            | Self::PrintSize
            | Self::NewGuide => doc.is_some(),
        }
    }
}

/// Commands that have keyboard shortcuts, most specific first: egui ignores
/// extra Shift/Alt when matching, so Shift+Cmd+Z must be checked before Cmd+Z.
const SHORTCUT_ORDER: &[Command] = &[
    Command::ExportAs,
    Command::ModifyFeather,
    Command::HideApp,
    Command::LockGuides,
    Command::SaveAs,
    Command::SaveACopy,
    Command::Revert,
    Command::LastFilter,
    Command::CopyMerged,
    Command::PasteInPlace,
    Command::LayerViaCut,
    Command::TransformAgain,
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
    Command::ImageSize,
    Command::CloseAll,
    Command::CloseOthers,
    Command::Undo,
    Command::FreeTransform,
    Command::Save,
    Command::Quit,
    Command::Invert,
    Command::Levels,
    Command::HueSaturation,
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
    Command::ToggleRulers,
    Command::ToggleExtras,
    Command::ToggleGuides,
    Command::ToggleGrid,
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
pub fn from_shortcuts(ctx: &egui::Context, app: &AppState) -> Vec<Command> {
    let mut out = Vec::new();
    // Every command is disabled under a modal dialog; leave its keys (such
    // as Cmd+D for "Don't Save") to the dialog
    if app.modal_open() {
        return out;
    }
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
        Command::Save | Command::SaveAs | Command::SaveACopy => {
            if let Some(id) = app.active_doc {
                match command {
                    Command::Save => actions::save(app, id),
                    Command::SaveAs => actions::save_as(app, id, false),
                    _ => actions::save_as(app, id, true),
                };
            }
        }
        Command::Revert => actions::revert(app),
        Command::Quit => actions::quit(app),
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
        Command::Average | Command::Solarize | Command::LastFilter => {
            let filter = match command {
                Command::Average => Some(Filter::Average),
                Command::Solarize => Some(Filter::Solarize),
                _ => app.last_filter,
            };
            let [r, g, b, _] = app.background.to_rgba8();
            if let Some(filter) = filter
                && let Some(state) = app.active_doc.and_then(|id| app.docs.get_mut(&id))
            {
                match op_core::filter::apply(&mut state.doc, filter, [r, g, b]) {
                    Ok(()) => {
                        state.record(filter.name());
                        app.last_filter = Some(filter);
                    }
                    Err(e) => app.alert = Some(e.message(filter.name())),
                }
            }
        }
        Command::Threshold
        | Command::Posterize
        | Command::Levels
        | Command::HueSaturation
        | Command::Exposure
        | Command::GaussianBlur
        | Command::BoxBlur
        | Command::UnsharpMask
        | Command::AddNoise
        | Command::Median
        | Command::Minimum
        | Command::Maximum
        | Command::HighPass
        | Command::Offset
        | Command::Mosaic => {
            let (kind, name) = match command {
                Command::Threshold => (AdjustKind::Threshold, "Threshold"),
                Command::Posterize => (AdjustKind::Posterize, "Posterize"),
                Command::Levels => (AdjustKind::Levels, "Levels"),
                Command::HueSaturation => (AdjustKind::HueSaturation, "Hue/Saturation"),
                Command::Exposure => (AdjustKind::Exposure, "Exposure"),
                Command::GaussianBlur => (AdjustKind::GaussianBlur, "Gaussian Blur"),
                Command::BoxBlur => (AdjustKind::BoxBlur, "Box Blur"),
                Command::UnsharpMask => (AdjustKind::UnsharpMask, "Unsharp Mask"),
                Command::AddNoise => (AdjustKind::AddNoise, "Add Noise"),
                Command::Median => (AdjustKind::Median, "Median"),
                Command::Minimum => (AdjustKind::Minimum, "Minimum"),
                Command::Maximum => (AdjustKind::Maximum, "Maximum"),
                Command::HighPass => (AdjustKind::HighPass, "High Pass"),
                Command::Offset => (AdjustKind::Offset, "Offset"),
                _ => (AdjustKind::Mosaic, "Mosaic"),
            };
            if let Some(state) = app.active_doc.and_then(|id| app.docs.get_mut(&id)) {
                match adjust::check(&state.doc) {
                    Ok(()) => {
                        let histogram = if kind == AdjustKind::Levels {
                            adjust::channel_histogram(&state.doc)
                        } else {
                            adjust::luminosity_histogram(&state.doc)
                        };
                        let before = state.doc.snapshot();
                        app.adjust_dialog = Some(AdjustDialog::new(kind, histogram, before));
                    }
                    Err(e) => app.alert = Some(e.message(name)),
                }
            }
        }
        Command::FreeTransform
        | Command::TransformAgain
        | Command::TransformRotate180
        | Command::TransformRotate90Clockwise
        | Command::TransformRotate90CounterClockwise
        | Command::TransformFlipHorizontal
        | Command::TransformFlipVertical => {
            let [r, g, b, _] = app.background.to_rgba8();
            let last = app.last_transform;
            let Some(state) = app.active_doc.and_then(|id| app.docs.get_mut(&id)) else {
                return;
            };
            if command == Command::FreeTransform {
                if let Err(e) = crate::free_transform::start(state) {
                    app.alert = Some(e.message("Free Transform"));
                }
                return;
            }
            let fixed = match command {
                Command::TransformRotate180 => Some(FixedTransform::Rotate180),
                Command::TransformRotate90Clockwise => Some(FixedTransform::Rotate90Clockwise),
                Command::TransformRotate90CounterClockwise => {
                    Some(FixedTransform::Rotate90CounterClockwise)
                }
                Command::TransformFlipHorizontal => Some(FixedTransform::FlipHorizontal),
                Command::TransformFlipVertical => Some(FixedTransform::FlipVertical),
                _ => None,
            };
            let (name, affine) = match (fixed, last) {
                (Some(f), _) => match transform::bounds(&state.doc) {
                    Ok(b) => (f.name(), f.affine(b)),
                    Err(e) => {
                        app.alert = Some(e.message(f.name()));
                        return;
                    }
                },
                (None, Some(m)) => ("Transform Again", m),
                (None, None) => return,
            };
            match transform::transform(&mut state.doc, affine, [r, g, b]) {
                Ok(()) => {
                    state.record(name);
                    app.last_transform = Some(affine);
                }
                Err(e) => app.alert = Some(e.message(name)),
            }
        }
        Command::ImageSize => {
            if let Some(state) = app.active() {
                let d = &state.doc;
                let dialog = crate::dialogs::ImageSizeDialog::new(d.width, d.height, d.resolution);
                app.image_size_dialog = Some(dialog);
            }
        }
        Command::ToggleRulers => app.view.rulers = !app.view.rulers,
        Command::ToggleExtras => app.view.extras = !app.view.extras,
        Command::ToggleGuides => app.view.guides = !app.view.guides,
        Command::ToggleGrid => app.view.grid = !app.view.grid,
        Command::LockGuides => app.view.lock_guides = !app.view.lock_guides,
        Command::NewGuide => app.new_guide_dialog = Some(Default::default()),
        Command::HideApp => {
            #[cfg(target_os = "macos")]
            crate::app_kit::hide_app();
        }
        Command::ModifyBorder
        | Command::ModifySmooth
        | Command::ModifyExpand
        | Command::ModifyContract
        | Command::ModifyFeather => {
            let kind = match command {
                Command::ModifyBorder => ModifyKind::Border,
                Command::ModifySmooth => ModifyKind::Smooth,
                Command::ModifyExpand => ModifyKind::Expand,
                Command::ModifyContract => ModifyKind::Contract,
                _ => ModifyKind::Feather,
            };
            app.modify_dialog = Some(crate::dialogs::ModifyDialog::new(kind));
        }
        Command::Grow | Command::Similar => {
            let options = app.wand.region;
            if let Some(state) = app.active()
                && let Some(s) = op_core::fill::grow(&state.doc, &options, command == Command::Grow)
            {
                state.doc.set_selection(Some(s));
                state.record(if command == Command::Grow {
                    "Grow"
                } else {
                    "Similar"
                });
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
                Command::Zoom200 => document_view::zoom_to(state, 2.0, ppp),
                Command::PrintSize => document_view::print_size(state, ppp),
                Command::FitLayers => document_view::fit_layers(state, ppp),
                Command::ClearGuides => {
                    state.doc.guides.clear();
                    state.record("Clear Guides");
                }
                _ => unreachable!("handled above"),
            }
        }
    }
}
