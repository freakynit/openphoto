//! Native macOS menu bar, laid out like Photoshop's. Items that aren't
//! implemented yet are shown disabled so the menus read like the real thing.

use std::str::FromStr;
use std::sync::mpsc::{Receiver, channel};

use muda::accelerator::{Accelerator, KeyAccelerator};
use muda::{AboutMetadata, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

use crate::commands::Command;
use crate::state::AppState;

const ALL_COMMANDS: &[Command] = &[
    Command::New,
    Command::Open,
    Command::Close,
    Command::CloseAll,
    Command::CloseOthers,
    Command::ExportAs,
    Command::Undo,
    Command::Redo,
    Command::ToggleLastState,
    Command::CanvasSize,
    Command::NewLayer,
    Command::DeleteLayer,
    Command::ToggleLayerVisibility,
    Command::ZoomIn,
    Command::ZoomOut,
    Command::FitOnScreen,
    Command::ActualPixels,
    Command::ToggleHistory,
];

fn id(command: Command) -> String {
    format!("{command:?}")
}

fn accelerator(text: &str) -> Option<Accelerator> {
    Accelerator::from_str(text).ok()
}

pub struct NativeMenu {
    /// Keeps the native menu alive.
    _menu: Menu,
    items: Vec<(Command, MenuItem)>,
    events: Receiver<Command>,
    /// Last applied (enabled, dynamic label) per item, to avoid redundant
    /// native calls.
    applied: Vec<(bool, Option<String>)>,
}

impl NativeMenu {
    pub fn install(ctx: &egui::Context) -> Self {
        let mut items = Vec::new();
        let mut item = |label: &str, command: Command| {
            let accel = command
                .shortcut()
                .and_then(|s| accelerator(&s.accelerator()));
            let item = MenuItem::with_id(id(command), label, false, accel);
            if command == Command::ZoomIn {
                // Shown as Cmd++ like Photoshop; Cmd+= is handled in egui
                let plus = KeyAccelerator::from_str("CmdOrCtrl++").ok();
                let _ = item.set_key_accelerator(plus);
            }
            items.push((command, item.clone()));
            item
        };
        // Not implemented yet: shown disabled
        let todo = |label: &str, accel: Option<&str>| {
            MenuItem::new(label, false, accel.and_then(accelerator))
        };
        let todo_sub = |label: &str| Submenu::new(label, false);
        let sep = PredefinedMenuItem::separator;

        let app_menu = Submenu::with_items(
            "OpenPhoto",
            true,
            &[
                &PredefinedMenuItem::about(
                    Some("About OpenPhoto"),
                    Some(AboutMetadata {
                        name: Some("OpenPhoto".into()),
                        version: Some(env!("CARGO_PKG_VERSION").into()),
                        ..Default::default()
                    }),
                ),
                &sep(),
                &PredefinedMenuItem::services(None),
                &sep(),
                &PredefinedMenuItem::hide(Some("Hide OpenPhoto")),
                &PredefinedMenuItem::hide_others(None),
                &PredefinedMenuItem::show_all(None),
                &sep(),
                &PredefinedMenuItem::quit(Some("Quit OpenPhoto")),
            ],
        );

        let file = Submenu::with_items(
            "File",
            true,
            &[
                &item("New...", Command::New) as &dyn IsMenuItem,
                &item("Open...", Command::Open),
                &sep(),
                &item("Close", Command::Close),
                &item("Close All", Command::CloseAll),
                &item("Close Others", Command::CloseOthers),
                &sep(),
                &todo("Save", Some("CmdOrCtrl+S")),
                &todo("Save As...", Some("CmdOrCtrl+Shift+S")),
                &sep(),
                &Submenu::with_items("Export", true, &[&item("Export As...", Command::ExportAs)])
                    .expect("static menu definition is valid"),
            ],
        );

        let edit = Submenu::with_items(
            "Edit",
            true,
            &[&item("Undo", Command::Undo), &item("Redo", Command::Redo)],
        );

        let image = Submenu::with_items(
            "Image",
            true,
            &[
                &todo_sub("Mode") as &dyn IsMenuItem,
                &sep(),
                &todo_sub("Adjustments"),
                &sep(),
                &todo("Auto Tone", Some("CmdOrCtrl+Shift+L")),
                &todo("Auto Contrast", Some("CmdOrCtrl+Shift+Alt+L")),
                &todo("Auto Color", Some("CmdOrCtrl+Shift+B")),
                &sep(),
                &todo("Image Size...", Some("CmdOrCtrl+Alt+I")),
                &item("Canvas Size...", Command::CanvasSize),
                &todo_sub("Image Rotation"),
                &todo("Crop", None),
                &todo("Trim...", None),
                &todo("Reveal All", None),
                &sep(),
                &todo("Duplicate...", None),
                &todo("Apply Image...", None),
                &todo("Calculations...", None),
                &sep(),
                &todo_sub("Variables"),
                &todo("Apply Data Set...", None),
                &sep(),
                &todo("Trap...", None),
                &sep(),
                &todo_sub("Analysis"),
            ],
        );

        let layer = Submenu::with_items(
            "Layer",
            true,
            &[
                // Photoshop's "Layer..." opens a dialog; ours creates the layer
                // directly, so the label has no ellipsis.
                &Submenu::with_items("New", true, &[&item("Layer", Command::NewLayer)])
                    .expect("static menu definition is valid") as &dyn IsMenuItem,
                &sep(),
                &Submenu::with_items("Delete", true, &[&item("Layer", Command::DeleteLayer)])
                    .expect("static menu definition is valid"),
                &sep(),
                &item("Hide Layers", Command::ToggleLayerVisibility),
            ],
        );

        let view = Submenu::with_items(
            "View",
            true,
            &[
                &item("Zoom In", Command::ZoomIn),
                &item("Zoom Out", Command::ZoomOut),
                &item("Fit on Screen", Command::FitOnScreen),
                &item("100%", Command::ActualPixels),
            ],
        );

        let window =
            Submenu::with_items("Window", true, &[&item("History", Command::ToggleHistory)]);
        let help = Submenu::with_items("Help", true, &[&todo("OpenPhoto Help", None)]);

        let menu = Menu::new();
        let submenus: Vec<Submenu> = [app_menu, file, edit, image, layer, view, window, help]
            .into_iter()
            .map(|m| m.expect("static menu definition is valid"))
            .collect();
        for submenu in &submenus {
            menu.append(submenu).expect("append submenu");
        }
        menu.init_for_nsapp();
        // "Window" also gets the standard window list from macOS
        submenus[6].set_as_windows_menu_for_nsapp();

        // Menu events arrive on the main thread outside egui's frame; queue
        // them and wake egui up.
        let (tx, events) = channel();
        let ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some(&command) = ALL_COMMANDS.iter().find(|c| event.id == id(**c).as_str()) {
                let _ = tx.send(command);
                ctx.request_repaint();
            }
        }));

        let applied = vec![(false, None); items.len()];
        Self {
            _menu: menu,
            items,
            events,
            applied,
        }
    }

    /// Commands chosen from the menu (or via their shortcuts) since the last call.
    pub fn poll(&self) -> Vec<Command> {
        self.events.try_iter().collect()
    }

    /// Syncs enabled states and dynamic labels ("Undo Canvas Size") with the app.
    pub fn update(&mut self, app: &AppState) {
        let doc = app.active_doc.and_then(|id| app.docs.get(&id));
        for (i, (command, item)) in self.items.iter().enumerate() {
            let enabled = command.enabled(app);
            let label = match command {
                Command::Undo => Some(match doc.and_then(|d| d.history.undo_name()) {
                    Some(name) => format!("Undo {name}"),
                    None => "Undo".into(),
                }),
                Command::Redo => Some(match doc.and_then(|d| d.history.redo_name()) {
                    Some(name) => format!("Redo {name}"),
                    None => "Redo".into(),
                }),
                Command::ToggleLayerVisibility => {
                    let layer = doc.and_then(|d| d.doc.active_layer.and_then(|id| d.doc.layer(id)));
                    Some(
                        if layer.is_some_and(|l| !l.visible) {
                            "Show Layers"
                        } else {
                            "Hide Layers"
                        }
                        .into(),
                    )
                }
                _ => None,
            };
            let applied = &mut self.applied[i];
            if applied.0 != enabled {
                item.set_enabled(enabled);
                applied.0 = enabled;
            }
            if let Some(label) = label
                && applied.1.as_ref() != Some(&label)
            {
                item.set_text(&label);
                applied.1 = Some(label);
            }
        }
    }
}
