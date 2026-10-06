//! UI layer: layout, panels, toolbar and document windows.
//!
//! It only turns user input into operations on `op-core` documents and draws
//! the resulting state.

mod actions;
mod commands;
mod dialogs;
mod document_view;
mod icons;
#[cfg(target_os = "macos")]
mod menu;
mod options_bar;
mod panels;
mod state;
mod theme;
mod titlebar;
mod toolbar;
mod widgets;

use std::path::PathBuf;

use egui::{Frame, Margin, Stroke};
use egui_dock::{DockArea, DockState, TabViewer};
use op_core::DocId;

use state::AppState;
use theme::{color, size};

pub struct OpenPhotoApp {
    state: AppState,
    dock: DockState<DocId>,
    panels: panels::Panels,
    #[cfg(target_os = "macos")]
    menu: menu::NativeMenu,
}

impl OpenPhotoApp {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx);
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("OpenPhoto requires the wgpu renderer");
        op_render::install(render_state);

        let mut app = Self {
            state: AppState::default(),
            dock: DockState::new(Vec::new()),
            panels: panels::Panels::default(),
            #[cfg(target_os = "macos")]
            menu: menu::NativeMenu::install(&cc.egui_ctx),
        };
        if files.is_empty() {
            actions::new_document(&mut app.state, &mut app.dock);
        } else {
            actions::open_paths(&mut app.state, &mut app.dock, files);
        }
        app
    }

    fn dock_style(&self, ui: &egui::Ui) -> egui_dock::Style {
        let mut s = egui_dock::Style::from_egui(ui.style());
        s.main_surface_border_stroke = Stroke::NONE;
        s.dock_area_padding = None;
        s.tab_bar.bg_fill = color::TAB_BAR;
        s.tab_bar.height = size::DOC_TAB_BAR;
        s.tab_bar.hline_color = color::TAB_BAR;
        s.tab_bar.corner_radius = 0.into();
        s.tab.spacing = 0.0;
        s.tab.hline_below_active_tab_name = false;
        s.tab.tab_body.bg_fill = color::PASTEBOARD;
        s.tab.tab_body.stroke = Stroke::NONE;
        s.tab.tab_body.inner_margin = Margin::ZERO;
        s.tab.tab_body.corner_radius = 0.into();
        for (style, fill, text) in [
            (&mut s.tab.active, color::TAB_ACTIVE, color::TEXT),
            (&mut s.tab.focused, color::TAB_ACTIVE, color::TEXT),
            (
                &mut s.tab.active_with_kb_focus,
                color::TAB_ACTIVE,
                color::TEXT,
            ),
            (
                &mut s.tab.focused_with_kb_focus,
                color::TAB_ACTIVE,
                color::TEXT,
            ),
            (&mut s.tab.inactive, color::TAB_BAR, color::TEXT_DIM),
            (
                &mut s.tab.inactive_with_kb_focus,
                color::TAB_BAR,
                color::TEXT_DIM,
            ),
            (&mut s.tab.hovered, color::TAB_INACTIVE, color::TEXT),
        ] {
            style.bg_fill = fill;
            style.text_color = text;
            style.outline_color = color::SEPARATOR;
            style.corner_radius = 0.into();
        }
        s.buttons.close_tab_color = color::TEXT_DIM;
        s.buttons.close_tab_active_color = color::TEXT;
        s.buttons.close_tab_bg_fill = color::HOVER;
        s.separator.color_idle = color::SEPARATOR;
        s.separator.width = 2.0;
        s.overlay.selection_color = color::ACCENT.gamma_multiply(0.4);
        s
    }
}

impl OpenPhotoApp {
    /// Runs commands from the native menu bar (macOS) or from egui shortcuts
    /// (other platforms, where there is no native menu).
    fn run_commands(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "macos")]
        let commands = {
            let mut c = self.menu.poll();
            c.extend(commands::from_shortcuts(ctx));
            c
        };
        #[cfg(not(target_os = "macos"))]
        let commands = commands::from_shortcuts(ctx);
        for command in commands {
            commands::run(command, ctx, &mut self.state, &mut self.dock);
        }
    }

    fn canvas_size_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.state.canvas_size_dialog.take() else {
            return;
        };
        let (fg, bg) = (self.state.foreground, self.state.background);
        match dialog.show(ctx, fg, bg) {
            dialogs::Outcome::Open => self.state.canvas_size_dialog = Some(dialog),
            dialogs::Outcome::Cancel => {}
            dialogs::Outcome::Apply {
                width,
                height,
                anchor,
                fill,
            } => {
                if let Some(state) = self.state.active()
                    && (width, height) != (state.doc.width, state.doc.height)
                {
                    state.doc.resize_canvas(width, height, anchor, fill);
                    state.record("Canvas Size");
                }
            }
        }
    }

    /// Shows the History panel to the left of the icon strip, the way
    /// Photoshop pops out a collapsed panel: its right edge touches the strip
    /// and its top sits 13 pt above the History button. Clicking anywhere else
    /// closes it (Photoshop's default "Auto-Collapse Iconic Panels").
    fn history_popout(&mut self, ctx: &egui::Context, strip: egui::Rect, button: egui::Rect) {
        let size = self.state.history_panel.size();
        let pos = egui::pos2(strip.left() - size.x, button.top() - theme::pt(13.0));
        let mut collapse = false;
        let area = egui::Area::new(egui::Id::new("history-popout"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(rect));
                        panel.set_clip_rect(rect.expand(1.0));
                        let action = panels::history::show(&mut panel, &mut self.state);
                        collapse = matches!(action, panels::history::Action::Collapse);
                    });
            });

        let clicked_outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !area.response.rect.contains(p) && !button.contains(p))
        });
        if clicked_outside || collapse {
            self.state.history_open = false;
        }
    }
}

struct DocTabs<'a> {
    state: &'a mut AppState,
}

impl TabViewer for DocTabs<'_> {
    type Tab = DocId;

    fn id(&mut self, tab: &mut DocId) -> egui::Id {
        egui::Id::new(("doc", tab.0))
    }

    fn title(&mut self, tab: &mut DocId) -> egui::WidgetText {
        let Some(s) = self.state.docs.get(tab) else {
            return "".into();
        };
        let d = &s.doc;
        let text = format!(
            "{} @ {} ({}/{})",
            d.title,
            document_view::zoom_label(s.view.zoom),
            d.color_mode.short(),
            d.bit_depth.bits()
        );
        egui::RichText::new(text)
            .font(theme::semibold(theme::font::BODY))
            .into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut DocId) {
        document_view::show(ui, self.state, *tab);
    }

    fn on_close(&mut self, tab: &mut DocId) -> egui_dock::widgets::tab_viewer::OnCloseResponse {
        self.state.docs.remove(tab);
        if self.state.active_doc == Some(*tab) {
            self.state.active_doc = None;
        }
        egui_dock::widgets::tab_viewer::OnCloseResponse::Close
    }

    fn scroll_bars(&self, _tab: &DocId) -> [bool; 2] {
        [false, false]
    }

    fn allowed_in_windows(&self, _tab: &mut DocId) -> bool {
        false
    }
}

impl eframe::App for OpenPhotoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if !dropped.is_empty() {
            actions::open_paths(&mut self.state, &mut self.dock, dropped);
        }
        self.run_commands(&ctx);
        actions::handle_tool_keys(&ctx, &mut self.state);

        let bar_frame = Frame::NONE.fill(color::PANEL);

        if cfg!(target_os = "macos") {
            egui::Panel::top("titlebar")
                .exact_size(size::TITLE_BAR)
                .resizable(false)
                .frame(bar_frame)
                .show(ui, titlebar::show);
        }
        egui::Panel::top("options-bar")
            .exact_size(size::OPTIONS_BAR)
            .resizable(false)
            .frame(bar_frame)
            .show(ui, |ui| options_bar::show(ui, &mut self.state));
        egui::Panel::left("toolbar")
            .exact_size(size::TOOLBAR)
            .resizable(false)
            .frame(bar_frame)
            .show(ui, |ui| toolbar::show(ui, &mut self.state));
        egui::Panel::right("panels")
            .exact_size(size::PANEL_COLUMN)
            .resizable(false)
            .frame(bar_frame)
            .show(ui, |ui| self.panels.show(ui, &mut self.state));
        let strip = egui::Panel::right("icon-strip")
            .exact_size(size::ICON_STRIP)
            .resizable(false)
            .frame(bar_frame)
            .show(ui, |ui| panels::icon_strip(ui, &mut self.state));
        let (strip_rect, history_button) = (strip.response.rect, strip.inner);

        egui::CentralPanel::no_frame()
            .frame(Frame::NONE.fill(color::PASTEBOARD))
            .show(ui, |ui| {
                if self.dock.iter_all_tabs().next().is_none() {
                    return;
                }
                let style = self.dock_style(ui);
                DockArea::new(&mut self.dock)
                    .style(style)
                    .show_add_buttons(false)
                    .show_leaf_collapse_buttons(false)
                    .show_leaf_close_all_buttons(false)
                    .show_inside(
                        ui,
                        &mut DocTabs {
                            state: &mut self.state,
                        },
                    );
            });

        if let Some((_, tab)) = self.dock.find_active_focused() {
            self.state.active_doc = Some(*tab);
        } else if self
            .state
            .active_doc
            .is_none_or(|id| !self.state.docs.contains_key(&id))
        {
            self.state.active_doc = self.dock.iter_all_tabs().next().map(|(_, id)| *id);
        }

        if self.state.history_open {
            self.history_popout(&ctx, strip_rect, history_button);
        }

        self.canvas_size_dialog(&ctx);

        #[cfg(target_os = "macos")]
        self.menu.update(&self.state);

        if let Some(msg) = self.state.alert.clone() {
            egui::Modal::new(egui::Id::new("alert")).show(&ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(msg);
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    self.state.alert = None;
                }
            });
        }
    }
}
