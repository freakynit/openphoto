//! The Move tool: shifting a layer's pixels, or only the selected ones.

use crate::document::Document;
use crate::layer::{LayerId, LayerKind};
use crate::selection::Selection;
use crate::tile::TiledImage;

/// Why the active layer can't be moved; the text matches Photoshop's alert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    NoLayer,
    Locked,
    Hidden,
}

impl MoveError {
    pub fn message(self) -> &'static str {
        match self {
            Self::NoLayer => "Could not use the move tool because there is no layer.",
            Self::Locked => "Could not use the move tool because the layer is locked.",
            Self::Hidden => "Could not use the move tool because the target layer is hidden.",
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
}

impl Move {
    /// Starts moving the active layer (or its selected pixels).
    pub fn begin(doc: &Document, background: [u8; 3]) -> Result<Self, MoveError> {
        let id = doc.active_layer.ok_or(MoveError::NoLayer)?;
        let layer = doc.layer(id).ok_or(MoveError::NoLayer)?;
        if !layer.visible {
            return Err(MoveError::Hidden);
        }
        let selection = doc.selection().cloned();
        // The background layer can only have selected pixels moved
        if layer.lock_position || layer.lock_pixels || (layer.is_background && selection.is_none())
        {
            return Err(MoveError::Locked);
        }
        let LayerKind::Raster(image) = &layer.kind;
        Ok(Self {
            layer: id,
            base: image.clone(),
            selection,
            background_fill: layer.is_background.then_some(background),
        })
    }

    /// Shows the layer moved by (`dx`, `dy`) pixels from where it started.
    /// Pixels moved past the canvas edge are lost.
    pub fn apply(&self, doc: &mut Document, dx: i64, dy: i64) {
        let (w, h) = (doc.width, doc.height);
        let moved = match &self.selection {
            None => self.base.with_canvas(w, h, dx, dy, [0; 4]),
            Some(sel) => {
                // Lift the selected pixels, leave the rest (or the background
                // color) behind, and drop the lifted pixels at the new place
                let mut out = TiledImage::new(w, h);
                for y in 0..h {
                    for x in 0..w {
                        let px = self.base.pixel(x, y);
                        let s = sel.get(x, y) as f32 / 255.0;
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
                for y in 0..h as i64 {
                    let sy = y - dy;
                    if sy < 0 || sy >= h as i64 {
                        continue;
                    }
                    for x in 0..w as i64 {
                        let sx = x - dx;
                        if sx < 0 || sx >= w as i64 {
                            continue;
                        }
                        let s = sel.get(sx as u32, sy as u32) as f32 / 255.0;
                        if s <= 0.0 {
                            continue;
                        }
                        let src = self.base.pixel(sx as u32, sy as u32);
                        let lifted = [src[0], src[1], src[2], (src[3] as f32 * s).round() as u8];
                        let dst = out.pixel(x as u32, y as u32);
                        out.set_pixel(x as u32, y as u32, over(lifted, dst));
                    }
                }
                out
            }
        };
        if let Some(layer) = doc.layer_mut(self.layer) {
            let LayerKind::Raster(image) = &mut layer.kind;
            *image = moved;
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
        let LayerKind::Raster(img) = &doc.layer(id).unwrap().kind;
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
}
