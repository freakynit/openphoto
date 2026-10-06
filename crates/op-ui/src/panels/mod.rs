//! Right-hand panel column: tabbed panel groups stacked vertically, with draggable
//! separators between groups.
//!
//! The three groups are fixed for now; a full docking system with drag-to-dock
//! and collapse-to-icons comes later.

mod color_panel;
pub mod floating;
mod histogram;
mod info;
mod navigator;
pub use color_panel::DEFAULT_SWATCHES;
pub mod history;
mod layers;
pub use layers::{delete_active_layer, new_layer, toggle_active_visibility};
mod properties;

use egui::{Align2, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};

use crate::icons;
use crate::state::AppState;
use crate::theme::{self, color, size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Color,
    Swatches,
    Gradients,
    Patterns,
    Properties,
    Adjustments,
    Libraries,
    Layers,
    Channels,
    Paths,
}

impl PanelKind {
    fn title(self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Swatches => "Swatches",
            Self::Gradients => "Gradients",
            Self::Patterns => "Patterns",
            Self::Properties => "Properties",
            Self::Adjustments => "Adjustments",
            Self::Libraries => "Libraries",
            Self::Layers => "Layers",
            Self::Channels => "Channels",
            Self::Paths => "Paths",
        }
    }
}

struct Group {
    tabs: Vec<PanelKind>,
    active: usize,
}

pub struct Panels {
    groups: Vec<Group>,
    /// Group heights. The group at index `flex` ignores its value and takes the
    /// remaining space.
    heights: Vec<f32>,
    flex: usize,
}

impl Default for Panels {
    fn default() -> Self {
        use PanelKind::*;
        Self {
            groups: vec![
                Group {
                    tabs: vec![Color, Swatches, Gradients, Patterns],
                    active: 0,
                },
                Group {
                    tabs: vec![Properties, Adjustments, Libraries],
                    active: 0,
                },
                Group {
                    tabs: vec![Layers, Channels, Paths],
                    active: 0,
                },
            ],
            heights: vec![228.0, 0.0, 430.0],
            flex: 1,
        }
    }
}

const GROUP_GAP: f32 = 3.0;
const MIN_GROUP: f32 = size::PANEL_TAB_BAR + 40.0;

impl Panels {
    pub fn show(&mut self, ui: &mut Ui, app: &mut AppState) {
        crate::toolbar::header(ui, icons::CARET_DOUBLE_RIGHT, Align2::RIGHT_CENTER);

        let area = ui.available_rect_before_wrap();
        let n = self.groups.len();
        let total_gaps = GROUP_GAP * (n - 1) as f32;
        let fixed: f32 = (0..n)
            .filter(|&i| i != self.flex)
            .map(|i| self.heights[i])
            .sum();
        let flex_h = (area.height() - fixed - total_gaps).max(MIN_GROUP);

        let mut y = area.top();
        for i in 0..n {
            let h = if i == self.flex {
                flex_h
            } else {
                self.heights[i]
            };
            let rect = Rect::from_min_size(Pos2::new(area.left(), y), Vec2::new(area.width(), h));
            self.show_group(ui, i, rect, app);
            y += h;

            if i + 1 < n {
                let gap = Rect::from_min_size(
                    Pos2::new(area.left(), y),
                    Vec2::new(area.width(), GROUP_GAP),
                );
                ui.painter().rect_filled(gap, 0, color::SEPARATOR);
                let r = ui.interact(
                    gap.expand2(Vec2::new(0.0, 2.0)),
                    ui.id().with(("gap", i)),
                    Sense::drag(),
                );
                if r.hovered() || r.dragged() {
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                }
                if r.dragged() {
                    // Resize the non-flex side; the flex group absorbs the difference
                    let dy = r.drag_delta().y;
                    if i == self.flex {
                        self.heights[i + 1] = (self.heights[i + 1] - dy).max(MIN_GROUP);
                    } else {
                        self.heights[i] = (self.heights[i] + dy).max(MIN_GROUP);
                    }
                }
                y += GROUP_GAP;
            }
        }
        ui.advance_cursor_after_rect(area);
    }

    fn show_group(&mut self, ui: &mut Ui, index: usize, rect: Rect, app: &mut AppState) {
        let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), size::PANEL_TAB_BAR));
        let body = Rect::from_min_max(Pos2::new(rect.left(), bar.bottom()), rect.max);
        let group = &mut self.groups[index];

        let painter = ui.painter_at(rect);
        painter.rect_filled(bar, 0, color::TAB_BAR);
        painter.rect_filled(body, 0, color::PANEL);

        // Tabs
        let mut x = bar.left();
        for (i, &tab) in group.tabs.iter().enumerate() {
            let font = theme::semibold(theme::font::BODY);
            let galley = ui
                .painter()
                .layout_no_wrap(tab.title().into(), font, Color32::WHITE);
            let w = galley.size().x + 30.0;
            let tab_rect = Rect::from_min_size(Pos2::new(x, bar.top()), Vec2::new(w, bar.height()));
            let response = ui.interact(tab_rect, ui.id().with(("tab", index, i)), Sense::click());
            if response.clicked() {
                group.active = i;
            }
            let active = group.active == i;
            if active {
                painter.rect_filled(tab_rect, 0, color::TAB_ACTIVE);
            } else {
                painter.line_segment(
                    [tab_rect.right_top(), tab_rect.right_bottom()],
                    Stroke::new(1.0, color::SEPARATOR),
                );
            }
            let text_color = if active || response.hovered() {
                color::TEXT
            } else {
                color::TEXT_DIM
            };
            painter.galley(tab_rect.center() - galley.size() / 2.0, galley, text_color);
            x += w;
        }
        // Panel menu
        painter.text(
            bar.right_center() - Vec2::new(14.0, 0.0),
            Align2::CENTER_CENTER,
            icons::LIST,
            theme::icon(16.0),
            color::TEXT_DIM,
        );

        let kind = group.tabs[group.active];
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(body)
                .id_salt(("panel-body", index)),
        );
        child.set_clip_rect(body);
        match kind {
            PanelKind::Color => color_panel::show(&mut child, app),
            PanelKind::Swatches => color_panel::swatches(&mut child, app),
            PanelKind::Properties => properties::show(&mut child, app),
            PanelKind::Layers => layers::show(&mut child, app),
            other => placeholder(&mut child, other),
        }
    }
}

fn placeholder(ui: &mut Ui, kind: PanelKind) {
    ui.painter().text(
        ui.max_rect().center(),
        Align2::CENTER_CENTER,
        format!("{} — not implemented yet", kind.title()),
        theme::small(),
        color::TEXT_DISABLED,
    );
}

/// The icon column between the canvas and the panels (collapsed History, Comments, ...).
/// Returns the rect of the History button, which the History popout is anchored to.
pub fn icon_strip(ui: &mut Ui, app: &mut AppState) -> Rect {
    use crate::theme::pt;
    // Photoshop: a 3 pt dark border on the left (next to the document's
    // scrollbar) and a 5 pt divider on the right (next to the panels)
    let full = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(
        Rect::from_min_max(full.min, Pos2::new(full.left() + pt(3.0), full.bottom())),
        0,
        egui::Color32::from_gray(0x3f),
    );
    let divider = Rect::from_min_max(Pos2::new(full.right() - pt(5.0), full.top()), full.max);
    painter.rect_filled(divider, 0, egui::Color32::from_gray(0x4d));
    painter.rect_filled(
        divider.shrink2(Vec2::new(pt(1.0), 0.0)),
        0,
        egui::Color32::from_gray(0x41),
    );
    let content = Rect::from_min_max(
        Pos2::new(full.left() + pt(3.0), full.top()),
        Pos2::new(full.right() - pt(5.0), full.bottom()),
    );
    let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(content));
    let ui = &mut inner;
    crate::toolbar::header(ui, icons::CARET_DOUBLE_LEFT, Align2::RIGHT_CENTER);
    ui.add_space(4.0);
    let history = ui.vertical_centered(|ui| {
        let r =
            crate::widgets::icon_button(ui, icons::CLOCK_COUNTER_CLOCKWISE, 38.0, app.history_open)
                .on_hover_text("History");
        if r.clicked() {
            app.history_open = !app.history_open;
        }
        crate::widgets::icon_button(ui, icons::CHAT_TEXT, 38.0, false).on_hover_text("Comments");
        r.rect
    });
    let r = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [
            r.left_top() + Vec2::new(8.0, 6.0),
            r.right_top() + Vec2::new(-8.0, 6.0),
        ],
        Stroke::new(1.0, color::SEPARATOR_LIGHT),
    );
    history.inner
}
