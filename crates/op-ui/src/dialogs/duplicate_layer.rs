//! Layer > Duplicate Layer...: the copy's name and where it goes (this
//! document, another open one, or a new one), laid out at the positions
//! measured on Photoshop 2026's dialog (447 × 208 pt). Sizes are in
//! Photoshop points from the dialog's top-left corner.

use egui::{Align2, Color32, Key, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, vec2};
use op_core::DocId;

use super::common;
use crate::theme::{self, pt};

const SIZE: egui::Vec2 = vec2(pt(447.0), pt(208.0));
const FONT: f32 = pt(12.0);
const LABEL: Color32 = Color32::from_gray(0xd7);
const VALUE: Color32 = Color32::from_gray(0xf1);
const OFF: Color32 = Color32::from_gray(0x88);

/// Where the copy goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Document(DocId),
    /// A new document with this title.
    New(String),
}

pub enum Outcome {
    Open,
    Cancel,
    Duplicate { name: String, to: Destination },
}

pub struct DuplicateLayerDialog {
    source: String,
    name: String,
    /// The open documents (id and title), the current one first.
    documents: Vec<(DocId, String)>,
    /// Index into `documents`, or `documents.len()` for "New".
    target: usize,
    new_title: String,
    first_frame: bool,
}

impl DuplicateLayerDialog {
    /// `source` is the active layer's name, `name` the default copy name;
    /// `documents` lists the open documents with the current one first;
    /// `new_title` is the next "Untitled-N".
    pub fn new(
        source: String,
        name: String,
        documents: Vec<(DocId, String)>,
        new_title: String,
    ) -> Self {
        Self {
            source,
            name,
            documents,
            target: 0,
            new_title,
            first_frame: true,
        }
    }

    fn destination(&self) -> Destination {
        match self.documents.get(self.target) {
            Some((id, _)) => Destination::Document(*id),
            None => Destination::New(self.new_title.clone()),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        egui::Modal::new(egui::Id::new("duplicate-layer"))
            .frame(egui::Frame::NONE)
            .backdrop_color(Color32::TRANSPARENT)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(SIZE, Sense::hover());
                outcome = self.ui(ui, rect);
            });
        self.first_frame = false;
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            outcome = Outcome::Cancel;
        }
        outcome
    }

    fn ui(&mut self, ui: &mut Ui, frame: Rect) -> Outcome {
        let at = |x: f32, y: f32| frame.min + vec2(pt(x), pt(y));
        let r = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        let font = theme::dialog(FONT);
        common::frame(ui, frame, "Duplicate Layer", theme::dialog_bold(pt(13.0)));
        let painter = ui.painter().clone();
        let label = |cy: f32, text: &str, c: Color32| {
            painter.text(at(81.5, cy), Align2::RIGHT_CENTER, text, font.clone(), c);
        };

        label(45.5, "Duplicate:", LABEL);
        painter.text(
            at(89.5, 45.5),
            Align2::LEFT_CENTER,
            &self.source,
            font.clone(),
            VALUE,
        );
        label(70.5, "As:", LABEL);
        common::text_field(
            ui,
            r(86.0, 61.0, 361.0, 80.0),
            &mut self.name,
            "duplicate-as",
            font.clone(),
            pt(4.0),
            self.first_frame,
        );

        // The Destination group: a 1 pt frame with its title in a gap
        let group = r(10.5, 96.5, 361.0, 197.5);
        let line = Stroke::new(pt(1.0), Color32::from_gray(0x42));
        painter.line_segment(
            [group.left_top(), Pos2::new(at(25.5, 0.0).x, group.top())],
            line,
        );
        painter.line_segment(
            [Pos2::new(at(99.0, 0.0).x, group.top()), group.right_top()],
            line,
        );
        painter.line_segment([group.left_top(), group.left_bottom()], line);
        painter.line_segment([group.right_top(), group.right_bottom()], line);
        painter.line_segment([group.left_bottom(), group.right_bottom()], line);
        painter.text(
            at(30.0, 95.25),
            Align2::LEFT_CENTER,
            "Destination",
            font.clone(),
            VALUE,
        );

        label(121.5, "Document:", LABEL);
        let names: Vec<String> = self
            .documents
            .iter()
            .map(|(_, t)| t.clone())
            .chain(std::iter::once("New".to_string()))
            .collect();
        let mut target = self.target;
        common::field_dropdown(
            ui,
            r(86.0, 111.0, 351.5, 132.0),
            "duplicate-document",
            &names[target],
            true,
            |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut target, i, n);
                }
            },
        );
        self.target = target;

        label(150.5, "Artboard:", OFF);
        common::field_dropdown(
            ui,
            r(86.0, 140.0, 351.5, 161.0),
            "duplicate-artboard",
            "Canvas",
            false,
            |_| {},
        );

        let new = self.target == self.documents.len();
        label(178.75, "Name:", if new { LABEL } else { OFF });
        let name_box = r(86.0, 169.0, 351.5, 188.0);
        if new {
            common::text_field(
                ui,
                name_box,
                &mut self.new_title,
                "duplicate-new-title",
                font.clone(),
                pt(4.0),
                false,
            );
        } else {
            painter.rect(
                name_box,
                0,
                Color32::from_gray(0x4d),
                Stroke::new(pt(1.0), Color32::from_gray(0x5e)),
                StrokeKind::Inside,
            );
        }

        let ok = common::ps_button(ui, r(377.5, 39.0, 437.0, 64.0), "OK", true, true, false);
        let cancel = common::ps_button(
            ui,
            r(377.5, 74.0, 437.0, 99.0),
            "Cancel",
            false,
            true,
            false,
        );
        if cancel.clicked() {
            return Outcome::Cancel;
        }
        if ok.clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
            let name = self.name.trim();
            return Outcome::Duplicate {
                name: if name.is_empty() {
                    self.source.clone()
                } else {
                    name.to_string()
                },
                to: self.destination(),
            };
        }
        Outcome::Open
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations() {
        let a = DocId(1);
        let mut d = DuplicateLayerDialog::new(
            "Background".into(),
            "Background copy".into(),
            vec![(a, "a.png".into())],
            "Untitled-1".into(),
        );
        assert_eq!(d.destination(), Destination::Document(a));
        d.target = 1;
        assert_eq!(d.destination(), Destination::New("Untitled-1".into()));
    }
}
