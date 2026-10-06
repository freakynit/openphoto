//! Custom title bar used on macOS once the system one is hidden (the traffic
//! light buttons are still drawn by the system).

use egui::{Align2, Sense, Ui, ViewportCommand};

use crate::theme::{self, color};

pub fn show(ui: &mut Ui) {
    let rect = ui.max_rect();
    let response = ui.interact(rect, ui.id().with("titlebar"), Sense::click_and_drag());
    if response.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        "OpenPhoto",
        theme::body(),
        color::TEXT,
    );
}
