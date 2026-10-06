//! History panel, popped out from the icon strip like Photoshop's collapsed panels.

use egui::{Align2, Pos2, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons;
use crate::state::AppState;
use crate::theme::{self, color, size};
use crate::widgets;

const ROW: f32 = 40.0;
const BOTTOM_BAR: f32 = 38.0;
pub const SIZE: Vec2 = Vec2::new(420.0, 520.0);

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let full = ui.max_rect();
    let bar = Rect::from_min_size(full.min, Vec2::new(full.width(), size::PANEL_TAB_BAR));
    let bottom = Rect::from_min_max(Pos2::new(full.left(), full.bottom() - BOTTOM_BAR), full.max);
    let list = Rect::from_min_max(Pos2::new(full.left(), bar.bottom()), bottom.right_top());

    let painter = ui.painter();
    painter.rect_filled(full, 0, color::PANEL);
    painter.rect_filled(bar, 0, color::TAB_BAR);
    let font = theme::semibold(theme::font::BODY);
    let title = painter.layout_no_wrap("History".into(), font, color::TEXT);
    let tab = Rect::from_min_size(bar.min, Vec2::new(title.size().x + 30.0, bar.height()));
    painter.rect_filled(tab, 0, color::TAB_ACTIVE);
    painter.galley(tab.center() - title.size() / 2.0, title, color::TEXT);
    painter.text(
        bar.right_center() - Vec2::new(14.0, 0.0),
        Align2::CENTER_CENTER,
        icons::LIST,
        theme::icon(16.0),
        color::TEXT_DIM,
    );

    let Some(state) = app.active() else {
        return;
    };

    let mut jump = None;
    let mut list_ui = ui.new_child(egui::UiBuilder::new().max_rect(list));
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show(&mut list_ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let current = state.history.current();

            // Snapshot of the document as it was first opened
            if row(ui, icons::IMAGE_SQUARE, &state.doc.title, false, false).clicked() {
                jump = Some(0);
            }
            let (sep, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(sep, 0, color::SEPARATOR);

            for (i, h) in state.history.states().iter().enumerate() {
                let icon = if i == 0 { icons::FILE } else { icons::STACK };
                // States after the current one have been undone and are drawn dimmed
                if row(ui, icon, &h.name, i == current, i > current).clicked() {
                    jump = Some(i);
                }
            }
        });
    if let Some(i) = jump {
        state.jump_to_state(i);
    }

    ui.painter().rect_filled(bottom, 0, color::PANEL);
    ui.painter().line_segment(
        [bottom.left_top(), bottom.right_top()],
        Stroke::new(1.0, color::SEPARATOR),
    );
    let mut bar_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bottom.shrink2(Vec2::new(14.0, 0.0)))
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let current = state.history.current();
    let delete = bar_ui.add_enabled_ui(current > 0, |ui| {
        widgets::icon_button(ui, icons::TRASH, 30.0, false).on_hover_text("Delete current state")
    });
    if delete.inner.clicked() {
        state.delete_states_from(current);
    }
    bar_ui.add_enabled_ui(false, |ui| {
        widgets::icon_button(ui, icons::CAMERA, 30.0, false).on_hover_text("Create new snapshot");
        widgets::icon_button(ui, icons::COPY, 30.0, false)
            .on_hover_text("Create new document from current state");
    });
}

fn row(ui: &mut Ui, icon: &str, label: &str, selected: bool, undone: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 0, color::ROW_SELECTED);
    } else if response.hovered() {
        painter.rect_filled(rect, 0, color::HOVER);
    }
    let text = if undone {
        color::TEXT_DISABLED
    } else {
        color::TEXT
    };
    painter.text(
        rect.left_center() + Vec2::new(26.0, 0.0),
        Align2::CENTER_CENTER,
        icon,
        theme::icon(18.0),
        text,
    );
    painter.text(
        rect.left_center() + Vec2::new(50.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        theme::body(),
        text,
    );
    response
}
