//! Histogram panel (compact view): the merged image's luminosity histogram.

use egui::{Pos2, Stroke, Ui};

use crate::state::AppState;
use crate::theme::color;

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let rect = ui.max_rect();
    let Some(state) = app.active() else {
        return;
    };
    let histogram = state.composite_histogram();
    let painter = ui.painter();
    painter.rect_filled(rect, 0, egui::Color32::from_gray(0x3c));
    let max = histogram.iter().copied().max().unwrap_or(0).max(1) as f32;
    let bar = rect.width() / 256.0;
    for (i, &count) in histogram.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let h = count as f32 / max * rect.height();
        let x = rect.left() + (i as f32 + 0.5) * bar;
        painter.line_segment(
            [Pos2::new(x, rect.bottom()), Pos2::new(x, rect.bottom() - h)],
            Stroke::new(bar.max(1.0), color::TEXT),
        );
    }
}
