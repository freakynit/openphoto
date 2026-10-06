//! The Move tool: shifting a layer's pixels, or only the selected ones.

use crate::document::Document;
use crate::layer::LayerId;
use crate::selection::Selection;
use crate::tile::TiledImage;

/// Why the active layer can't be moved; the text matches Photoshop's alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    NoLayer,
    Locked,
    Hidden,
    /// Moving selected pixels while a group is the active layer.
    Group,
}

impl MoveError {
    pub fn message(self) -> &'static str {
        match self {
            Self::NoLayer => "Could not use the move tool because there is no layer.",
            Self::Locked => "Could not use the move tool because the layer is locked.",
            Self::Hidden => "Could not use the move tool because the target layer is hidden.",
            Self::Group => "Could not use the move tool because the target layer is a group.",
        }
    }
}

/// A move in progress: the layer and selection as they were when it started,
/// so every offset is applied to the original pixels (no accumulated loss).
pub struct Move {
    layer: LayerId,
    base: TiledImage,
    selection: Option<Selection>,
    /// Background layer: the hole left behind gets the background color.
    background_fill: Option<[u8; 3]>,
    /// Other selected layers moving along (without a pixel selection),
    /// with their pixels when the move started.
    others: Vec<(LayerId, TiledImage)>,
}

impl Move {
    /// Starts moving the active layer (or its selected pixels).
    pub fn begin(doc: &Document, background: [u8; 3]) -> Result<Self, MoveError> {
        let id = doc.active_layer.ok_or(MoveError::NoLayer)?;
        let layer = doc.layer(id).ok_or(MoveError::NoLayer)?;
        if !doc.is_shown(id) {
            return Err(MoveError::Hidden);
        }
        let selection = doc.selection().cloned();
        // The background layer can only have selected pixels moved
        if layer.position_locked()
            || layer.pixels_locked()
            || (layer.is_background && selection.is_none())
        {
            return Err(MoveError::Locked);
        }
        // Selected pixels only move on a pixel layer
        if selection.is_some() && layer.is_group() {
            return Err(MoveError::Group);
        }
        // Without a pixel selection, the other selected layers (and the
        // layers in selected groups) that can move go along, as in Photoshop
        let mut moving: Vec<(LayerId, TiledImage)> = if selection.is_none() {
            doc.pixel_layers(&doc.selected_layers())
                .into_iter()
                .filter_map(|other| doc.layer(other))
                .filter(|l| {
                    doc.is_shown(l.id)
                        && !l.is_background
                        && !l.position_locked()
                        && !l.pixels_locked()
                })
                .filter_map(|l| Some((l.id, l.image()?.clone())))
                .collect()
        } else {
            Vec::new()
        };
        // The active layer leads (an empty group moves nothing)
        let (main, image) = match layer.image() {
            Some(image) => {
                moving.retain(|(other, _)| *other != id);
                (id, image.clone())
            }
            None if moving.is_empty() => return Err(MoveError::Locked),
            None => moving.remove(0),
        };
        let others = moving;
        Ok(Self {
            layer: main,
            base: image,
            selection,
            background_fill: layer.is_background.then_some(background),
            others,
        })
    }

    /// Shows the layer moved by (`dx`, `dy`) pixels from where it started.
    /// Pixels moved past the canvas edge stay on the layer, as in
    /// Photoshop, except on the background layer, which ends at the canvas.
    pub fn apply(&self, doc: &mut Document, dx: i64, dy: i64) {
        let (w, h) = (doc.width, doc.height);
        let moved = match &self.selection {
            None => self.base.with_canvas(w, h, dx, dy, [0; 4]),
            Some(sel) => {
                // Lift the selected pixels, leave the rest (or the background
                // color) behind, and drop the lifted pixels at the new place
                let mut out = self.base.clone();
                for y in 0..h {
                    for x in 0..w {
                        let s = sel.get(x, y) as f32 / 255.0;
                        if s <= 0.0 {
                            continue;
                        }
                        let px = self.base.pixel(x, y);
                        let hole = match self.background_fill {
                            Some([r, g, b]) => mix(px, [r, g, b, 255], s),
                            None => [
                                px[0],
                                px[1],
                                px[2],
                                (px[3] as f32 * (1.0 - s)).round() as u8,
                            ],
                        };
                        out.set_pixel(x, y, hole);
                    }
                }
                let clip = self.background_fill.is_some();
                for sy in 0..h {
                    for sx in 0..w {
                        let s = sel.get(sx, sy) as f32 / 255.0;
                        if s <= 0.0 {
                            continue;
                        }
                        let (x, y) = (sx as i64 + dx, sy as i64 + dy);
                        if clip && (x < 0 || y < 0 || x >= w as i64 || y >= h as i64) {
                            continue;
                        }
                        let src = self.base.pixel(sx, sy);
                        let lifted = [src[0], src[1], src[2], (src[3] as f32 * s).round() as u8];
                        let dst = out.pixel_at(x, y);
                        out.set_pixel_at(x, y, over(lifted, dst));
                    }
                }
                out
            }
        };
        if let Some(image) = doc.layer_mut(self.layer).and_then(|l| l.image_mut()) {
            *image = moved;
        }
        for (id, base) in &self.others {
            if let Some(image) = doc.layer_mut(*id).and_then(|l| l.image_mut()) {
                *image = base.with_canvas(w, h, dx, dy, [0; 4]);
            }
        }
        if let Some(sel) = &self.selection {
            doc.set_selection(Some(sel.with_canvas(w, h, dx, dy)));
        }
        doc.mark_dirty();
    }
}

fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

/// Straight-alpha source-over.
fn over(s: [u8; 4], d: [u8; 4]) -> [u8; 4] {
    let sa = s[3] as f32 / 255.0;
    let da = d[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        return [0; 4];
    }
    let c = |i: usize| ((s[i] as f32 * sa + d[i] as f32 * da * (1.0 - sa)) / oa).round() as u8;
    [c(0), c(1), c(2), (oa * 255.0).round() as u8]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::Rect;
    use crate::{Color, Layer};

    fn doc_with_dot() -> (Document, LayerId) {
        let mut doc = Document::new_with_background("t", 10, 10, Color::WHITE);
        let id = doc.new_layer_id();
        let mut img = TiledImage::new(10, 10);
        img.set_pixel(2, 2, [255, 0, 0, 255]);
        img.set_pixel(7, 7, [0, 0, 255, 255]);
        doc.layers.push(Layer::raster(id, "L", img));
        doc.active_layer = Some(id);
        (doc, id)
    }

    fn px(doc: &Document, id: LayerId, x: u32, y: u32) -> [u8; 4] {
        let img = doc.layer(id).unwrap().image().unwrap();
        img.pixel(x, y)
    }

    #[test]
    fn moves_whole_layer_from_its_start() {
        let (mut doc, id) = doc_with_dot();
        let m = Move::begin(&doc, [0; 3]).unwrap();
        m.apply(&mut doc, 1, 1);
        m.apply(&mut doc, 3, 0);
        assert_eq!(px(&doc, id, 5, 2), [255, 0, 0, 255]);
        assert_eq!(px(&doc, id, 2, 2)[3], 0);
    }

    #[test]
    fn moves_only_selected_pixels() {
        let (mut doc, id) = doc_with_dot();
        doc.set_selection(Some(Selection::rect(10, 10, Rect::new(0.0, 0.0, 5.0, 5.0))));
        let m = Move::begin(&doc, [0; 3]).unwrap();
        m.apply(&mut doc, 1, 0);
        assert_eq!(px(&doc, id, 3, 2), [255, 0, 0, 255]);
        assert_eq!(px(&doc, id, 2, 2)[3], 0);
        // Unselected pixels stay
        assert_eq!(px(&doc, id, 7, 7), [0, 0, 255, 255]);
        // The selection moved with them
        assert_eq!(doc.selection().unwrap().bounds(), Some((1, 0, 6, 5)));
    }

    #[test]
    fn pixels_moved_off_the_canvas_come_back() {
        let (mut doc, id) = doc_with_dot();
        let m = Move::begin(&doc, [0; 3]).unwrap();
        // The red dot at (2, 2) goes 5 px past the left edge...
        m.apply(&mut doc, -7, 0);
        assert_eq!(px(&doc, id, 0, 2)[3], 0);
        let img = doc.layer(id).unwrap().image().unwrap();
        assert_eq!(img.pixel_at(-5, 2), [255, 0, 0, 255]);
        // ...and a second move brings it back
        let m = Move::begin(&doc, [0; 3]).unwrap();
        m.apply(&mut doc, 7, 0);
        assert_eq!(px(&doc, id, 2, 2), [255, 0, 0, 255]);
        assert_eq!(px(&doc, id, 7, 7), [0, 0, 255, 255]);
    }

    #[test]
    fn selected_pixels_moved_off_the_canvas_are_kept_except_on_the_background() {
        let (mut doc, id) = doc_with_dot();
        doc.set_selection(Some(Selection::rect(10, 10, Rect::new(0.0, 0.0, 5.0, 5.0))));
        let m = Move::begin(&doc, [0; 3]).unwrap();
        m.apply(&mut doc, 0, -4);
        let img = doc.layer(id).unwrap().image().unwrap();
        assert_eq!(img.pixel_at(2, -2), [255, 0, 0, 255]);

        let mut doc = Document::new_with_background("t", 10, 10, Color::WHITE);
        doc.set_selection(Some(Selection::rect(10, 10, Rect::new(0.0, 0.0, 2.0, 2.0))));
        let m = Move::begin(&doc, [9, 9, 9]).unwrap();
        m.apply(&mut doc, -1, 0);
        let img = doc.layers[0].image().unwrap();
        assert!(!img.has_pixels_outside());
    }

    #[test]
    fn background_needs_a_selection_and_leaves_background_color() {
        let mut doc = Document::new_with_background("t", 10, 10, Color::WHITE);
        assert_eq!(Move::begin(&doc, [0; 3]).err(), Some(MoveError::Locked));
        doc.set_selection(Some(Selection::rect(10, 10, Rect::new(0.0, 0.0, 2.0, 2.0))));
        let m = Move::begin(&doc, [9, 9, 9]).unwrap();
        m.apply(&mut doc, 5, 5);
        let bg = doc.layers[0].id;
        assert_eq!(px(&doc, bg, 0, 0), [9, 9, 9, 255]);
        assert_eq!(px(&doc, bg, 5, 5), [255, 255, 255, 255]);
    }

    #[test]
    fn selected_layers_move_together() {
        let (mut doc, id) = doc_with_dot();
        let other = doc.new_layer_id();
        let mut img = TiledImage::new(10, 10);
        img.set_pixel(5, 5, [0, 255, 0, 255]);
        doc.layers.push(Layer::raster(other, "M", img));
        // Select M and the background, then the dot's layer (active)
        let bg = doc.layers[0].id;
        doc.select_layer(other);
        doc.toggle_layer_selection(bg);
        doc.toggle_layer_selection(id);
        assert_eq!(doc.active_layer, Some(id));
        let m = Move::begin(&doc, [0; 3]).unwrap();
        m.apply(&mut doc, 1, 2);
        assert_eq!(px(&doc, id, 3, 4), [255, 0, 0, 255]);
        assert_eq!(px(&doc, other, 6, 7), [0, 255, 0, 255]);
        // The background is selected too but stays put
        assert_eq!(px(&doc, bg, 0, 0), [255, 255, 255, 255]);
    }
}
