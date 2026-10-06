//! Dialogs for adjustments with a single setting: Image > Adjustments >
//! Threshold... (histogram, slider and level) and Posterize... (levels).
//!
//! Laid out after Photoshop's dialogs; sizes are in Photoshop points. With
//! Preview on, the document shows the result while the dialog is open.

use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, vec2};
use op_core::adjust::Adjustment;

use super::common;
use crate::theme::{self, color, pt};

const FONT: f32 = pt(12.5);
const BUTTON: egui::Vec2 = vec2(pt(88.0), pt(24.0));
const FIELD_H: f32 = pt(22.0);
/// Threshold's histogram box.
const HISTOGRAM: Rect = Rect::from_min_max(
    Pos2::new(pt(20.0), pt(78.0)),
    Pos2::new(pt(278.0), pt(178.0)),
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Threshold,
    Posterize,
}

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Threshold => "Threshold",
            Self::Posterize => "Posterize",
        }
    }

    fn size(self) -> egui::Vec2 {
        match self {
            Self::Threshold => vec2(pt(400.0), pt(232.0)),
            Self::Posterize => vec2(pt(330.0), pt(132.0)),
        }
    }

    /// Photoshop's defaults and ranges.
    fn default_value(self) -> u8 {
        match self {
            Self::Threshold => 128,
            Self::Posterize => 4,
        }
    }

    fn range(self) -> (u8, u8) {
        match self {
            Self::Threshold => (1, 255),
            Self::Posterize => (2, 255),
        }
    }
}

pub enum Outcome {
    Open,
    Cancel,
    Apply(Adjustment),
}

pub struct AdjustDialog {
    pub kind: Kind,
    value: String,
    pub preview: bool,
    /// Luminosity histogram of the pixels being adjusted (Threshold).
    histogram: [u64; 256],
    first_frame: bool,
    /// The adjustment the document currently previews, if any.
    pub previewing: Option<Adjustment>,
    /// The document before any preview, restored on Cancel.
    pub before: op_core::Snapshot,
}

impl AdjustDialog {
    pub fn new(kind: Kind, histogram: [u64; 256], before: op_core::Snapshot) -> Self {
        Self {
            before,
            kind,
            value: kind.default_value().to_string(),
            preview: true,
            histogram,
            first_frame: true,
            previewing: None,
        }
    }

    fn value(&self) -> Option<u8> {
        let v: u8 = self.value.trim().parse().ok()?;
        let (lo, hi) = self.kind.range();
        (lo..=hi).contains(&v).then_some(v)
    }

    /// The adjustment as currently set, if the value is valid.
    pub fn adjustment(&self) -> Option<Adjustment> {
        let v = self.value()?;
        Some(match self.kind {
            Kind::Threshold => Adjustment::Threshold(v),
            Kind::Posterize => Adjustment::Posterize(v),
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("adjust-dialog"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(self.kind.size(), Sense::hover());
                outcome = self.ui(ui, rect);
            });
        self.first_frame = false;
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(x, y);
        let font = FontId::proportional(FONT);
        common::frame(ui, frame, self.kind.title(), theme::semibold(pt(13.0)));
        let button_x = frame.width() - pt(108.0);

        let (label, field_y) = match self.kind {
            Kind::Threshold => ("Threshold Level:", pt(52.0)),
            Kind::Posterize => ("Levels:", pt(58.0)),
        };
        let label_rect = ui.painter().text(
            at(pt(20.0), field_y),
            Align2::LEFT_CENTER,
            label,
            font.clone(),
            color::TEXT,
        );
        let field = Rect::from_min_size(
            Pos2::new(
                label_rect.right() + pt(8.0),
                label_rect.center().y - FIELD_H / 2.0,
            ),
            vec2(pt(52.0), FIELD_H),
        );
        common::number_field(
            ui,
            field,
            &mut self.value,
            "adjust-value",
            FONT,
            self.first_frame,
        );

        if self.kind == Kind::Threshold {
            self.histogram_ui(ui, frame);
        }

        let button_font = FontId::proportional(pt(13.0));
        let valid = self.value().is_some();
        let ok = common::pill_button(
            ui,
            Rect::from_min_size(at(button_x, pt(44.0)), BUTTON),
            "OK",
            button_font.clone(),
            valid,
        );
        let cancel = common::pill_button(
            ui,
            Rect::from_min_size(at(button_x, pt(76.0)), BUTTON),
            "Cancel",
            button_font,
            true,
        );
        let check =
            Rect::from_min_size(at(button_x, pt(112.0) - pt(9.0)), vec2(pt(96.0), pt(18.0)));
        let mut check_ui = ui.new_child(egui::UiBuilder::new().max_rect(check));
        check_ui.checkbox(&mut self.preview, egui::RichText::new("Preview").font(font));

        if cancel.clicked() {
            return Outcome::Cancel;
        }
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        if (ok.clicked() || enter)
            && let Some(adjustment) = self.adjustment()
        {
            return Outcome::Apply(adjustment);
        }
        Outcome::Open
    }

    /// The histogram with the threshold marker under it; dragging the marker
    /// sets the level.
    fn histogram_ui(&mut self, ui: &mut Ui, frame: Rect) {
        let rect = HISTOGRAM.translate(frame.min.to_vec2());
        let painter = ui.painter();
        painter.rect(
            rect,
            0,
            Color32::from_gray(0x3c),
            Stroke::new(1.0, Color32::from_gray(0x2c)),
            StrokeKind::Outside,
        );
        let max = self.histogram.iter().copied().max().unwrap_or(0).max(1) as f32;
        let bar = rect.width() / 256.0;
        for (i, &count) in self.histogram.iter().enumerate() {
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

        let track = Rect::from_min_max(
            Pos2::new(rect.left(), rect.bottom() + pt(2.0)),
            Pos2::new(rect.right(), rect.bottom() + pt(14.0)),
        );
        let response = ui.interact(
            track,
            ui.id().with("threshold-marker"),
            Sense::click_and_drag(),
        );
        if (response.dragged() || response.clicked())
            && let Some(p) = response.interact_pointer_pos()
        {
            let v = ((p.x - rect.left()) / rect.width() * 255.0)
                .round()
                .clamp(1.0, 255.0);
            self.value = (v as u8).to_string();
        }
        let level = self.value().unwrap_or(128) as f32;
        let x = rect.left() + level / 255.0 * rect.width();
        let tip = Pos2::new(x, track.top());
        ui.painter().add(Shape::convex_polygon(
            vec![
                tip,
                Pos2::new(x + pt(6.0), track.bottom()),
                Pos2::new(x - pt(6.0), track.bottom()),
            ],
            color::TEXT,
            Stroke::NONE,
        ));
    }
}
