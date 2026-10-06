//! Edit > Cut, Copy, Copy Merged and Paste: which pixels go to the
//! clipboard, and where pasted pixels land.

use crate::document::Document;
use crate::fill::{self, FillError};
use crate::layer::{Layer, LayerId};
use crate::tile::TiledImage;

/// Pixels on the clipboard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    pub width: u32,
    pub height: u32,
    /// Straight RGBA8, row by row.
    pub pixels: Vec<u8>,
    /// Top-left corner in the document it was copied from; images that come
    /// from other applications have no position.
    pub origin: Option<(i64, i64)>,
}

impl Clip {
    /// An image from outside OpenPhoto.
    pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        Self {
            width,
            height,
            pixels,
            origin: None,
        }
    }
}

/// Why a clipboard command can't be done; the messages match Photoshop's
/// alerts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipError {
    NoLayer,
    Hidden,
    Locked,
    /// Nothing but transparent pixels in the copied area.
    Empty,
    /// The active layer is a group.
    Group,
}

impl ClipError {
    pub fn message(self, command: &str) -> String {
        match self {
            Self::NoLayer => FillError::NoLayer.message(command),
            Self::Hidden => FillError::Hidden.message(command),
            Self::Locked => FillError::Locked.message(command),
            Self::Group => FillError::Group.message(command),
            Self::Empty => format!(
                "Could not complete the {command} command because the selected area is empty."
            ),
        }
    }
}

impl From<FillError> for ClipError {
    fn from(e: FillError) -> Self {
        match e {
            FillError::NoLayer => Self::NoLayer,
            FillError::Hidden => Self::Hidden,
            FillError::Group => Self::Group,
            FillError::Locked => Self::Locked,
        }
    }
}

/// The copied area: the selection's bounds, or the whole canvas without a
/// selection. Each pixel's alpha is scaled by the selection.
fn extract(doc: &Document, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Result<Clip, ClipError> {
    let selection = doc.selection();
    let (x0, y0, x1, y1) = match selection {
        Some(s) => s.bounds().ok_or(ClipError::Empty)?,
        None => (0, 0, doc.width, doc.height),
    };
    let (width, height) = (x1 - x0, y1 - y0);
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    let mut any = false;
    for y in y0..y1 {
        for x in x0..x1 {
            let [r, g, b, a] = pixel(x, y);
            let a = match selection {
                Some(s) => (a as u32 * s.get(x, y) as u32 + 127) / 255,
                None => a as u32,
            } as u8;
            any |= a > 0;
            pixels.extend_from_slice(&[r, g, b, a]);
        }
    }
    if !any {
        return Err(ClipError::Empty);
    }
    Ok(Clip {
        width,
        height,
        pixels,
        origin: Some((x0 as i64, y0 as i64)),
    })
}

fn active_image(doc: &Document) -> Result<&TiledImage, ClipError> {
    let layer = doc
        .active_layer
        .and_then(|id| doc.layer(id))
        .ok_or(ClipError::NoLayer)?;
    layer.image().ok_or(ClipError::Group)
}

/// Edit > Copy: the selected pixels of the active layer.
pub fn copy(doc: &Document) -> Result<Clip, ClipError> {
    let image = active_image(doc)?;
    extract(doc, |x, y| image.pixel(x, y))
}

/// Edit > Copy Merged: the selected pixels of the visible layers merged.
pub fn copy_merged(doc: &Document) -> Result<Clip, ClipError> {
    let merged = doc.composite_rgba8();
    let w = doc.width as usize;
    extract(doc, |x, y| {
        let i = (y as usize * w + x as usize) * 4;
        merged[i..i + 4].try_into().unwrap()
    })
}

/// Edit > Cut: copies, then clears the selection like Edit > Clear (the
/// background layer is filled with `background`).
pub fn cut(doc: &mut Document, background: [u8; 3]) -> Result<Clip, ClipError> {
    let layer = doc
        .active_layer
        .and_then(|id| doc.layer(id))
        .ok_or(ClipError::NoLayer)?;
    if !layer.visible {
        return Err(ClipError::Hidden);
    }
    if doc.pixels_locked(layer.id) {
        return Err(ClipError::Locked);
    }
    let clip = copy(doc)?;
    fill::clear(doc, background)?;
    Ok(clip)
}

/// Where Edit > Paste puts the top-left corner of `clip` in a document of
/// `width` × `height`. `visible` is the part of the document shown in the
/// window (x0, y0, x1, y1 in document pixels).
///
/// Paste in Place keeps the copied position. Plain Paste keeps it too when
/// that area lies within the canvas and can be seen, so copying and pasting
/// in one document stacks the copy on the original; otherwise the pixels
/// are centered in the visible part of the canvas.
pub fn placement(
    clip: &Clip,
    width: u32,
    height: u32,
    visible: [f32; 4],
    in_place: bool,
) -> (i64, i64) {
    let (cw, ch) = (clip.width as i64, clip.height as i64);
    if let Some((ox, oy)) = clip.origin {
        let inside = ox >= 0 && oy >= 0 && ox + cw <= width as i64 && oy + ch <= height as i64;
        let seen = (ox as f32) < visible[2]
            && ((ox + cw) as f32) > visible[0]
            && (oy as f32) < visible[3]
            && ((oy + ch) as f32) > visible[1];
        if in_place || (inside && seen) {
            return (ox, oy);
        }
    }
    let x0 = visible[0].max(0.0);
    let y0 = visible[1].max(0.0);
    let x1 = visible[2].min(width as f32);
    let y1 = visible[3].min(height as f32);
    // The canvas is scrolled out of view: center on the canvas instead
    let (cx, cy) = if x1 > x0 && y1 > y0 {
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
    } else {
        (width as f32 / 2.0, height as f32 / 2.0)
    };
    (
        (cx - cw as f32 / 2.0).round() as i64,
        (cy - ch as f32 / 2.0).round() as i64,
    )
}

/// Edit > Paste: puts `clip` on a new layer above the active one with its
/// top-left corner at `at`, and deselects. Pixels outside the canvas are
/// kept on the layer, as in Photoshop (Image > Reveal All shows them).
pub fn paste(doc: &mut Document, clip: &Clip, at: (i64, i64)) -> LayerId {
    let mut image = TiledImage::new(doc.width, doc.height);
    for y in 0..clip.height {
        for x in 0..clip.width {
            let i = ((y * clip.width + x) * 4) as usize;
            let px: [u8; 4] = clip.pixels[i..i + 4].try_into().unwrap();
            if px[3] > 0 {
                image.set_pixel_at(at.0 + x as i64, at.1 + y as i64, px);
            }
        }
    }
    let id = doc.new_layer_id();
    doc.insert_above_active(Layer::raster(id, doc.next_layer_name(), image));
    doc.set_selection(None);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::selection::{Rect, Selection};

    fn doc() -> Document {
        // 4×4 white background, with a red pixel at (1, 1)
        let mut doc = Document::new_with_background("t", 4, 4, Color::WHITE);
        let id = doc.active_layer.unwrap();
        let image = doc.layer_mut(id).unwrap().image_mut().unwrap();
        image.set_pixel(1, 1, [255, 0, 0, 255]);
        doc
    }

    fn select(doc: &mut Document, x0: f32, y0: f32, x1: f32, y1: f32) {
        let (w, h) = (doc.width, doc.height);
        let s = Selection::rect(w, h, Rect::new(x0, y0, x1, y1));
        doc.set_selection(Some(s));
    }

    #[test]
    fn copy_takes_the_selection_bounds() {
        let mut doc = doc();
        select(&mut doc, 1.0, 1.0, 3.0, 2.0);
        let clip = copy(&doc).unwrap();
        assert_eq!((clip.width, clip.height, clip.origin), (2, 1, Some((1, 1))));
        assert_eq!(clip.pixels, [255, 0, 0, 255, 255, 255, 255, 255]);
        // Without a selection the whole layer is copied
        doc.set_selection(None);
        let clip = copy(&doc).unwrap();
        assert_eq!((clip.width, clip.height, clip.origin), (4, 4, Some((0, 0))));
    }

    #[test]
    fn copying_transparent_pixels_fails() {
        let mut doc = doc();
        let id = doc.new_layer_id();
        doc.insert_above_active(Layer::raster(id, "Layer 1", TiledImage::new(4, 4)));
        assert_eq!(copy(&doc), Err(ClipError::Empty));
        assert_eq!(
            ClipError::Empty.message("Copy"),
            "Could not complete the Copy command because the selected area is empty."
        );
        // Copy Merged sees the layers below
        assert!(copy_merged(&doc).is_ok());
    }

    #[test]
    fn cut_clears_and_paste_stacks_on_the_original() {
        let mut doc = doc();
        select(&mut doc, 0.0, 0.0, 2.0, 2.0);
        // On the background layer, Cut leaves the background color
        let clip = cut(&mut doc, [0, 0, 255]).unwrap();
        assert_eq!(&doc.composite_rgba8()[20..24], [0, 0, 255, 255]);
        let at = placement(&clip, 4, 4, [0.0, 0.0, 4.0, 4.0], false);
        assert_eq!(at, (0, 0));
        let id = paste(&mut doc, &clip, at);
        assert_eq!(doc.active_layer, Some(id));
        assert_eq!(doc.layers.len(), 2);
        assert_eq!(doc.layers[1].name, "Layer 1");
        assert!(doc.selection().is_none());
        // The red pixel is back at (1, 1)
        assert_eq!(&doc.composite_rgba8()[20..24], [255, 0, 0, 255]);
    }

    #[test]
    fn paste_centers_when_the_origin_is_out_of_view() {
        let clip = Clip {
            width: 2,
            height: 2,
            pixels: vec![255; 16],
            origin: Some((8, 8)),
        };
        // Doesn't fit in a 6×6 canvas: centered in the visible area
        assert_eq!(placement(&clip, 6, 6, [0.0, 0.0, 6.0, 6.0], false), (2, 2));
        // Paste in Place keeps the position regardless
        assert_eq!(placement(&clip, 6, 6, [0.0, 0.0, 6.0, 6.0], true), (8, 8));
        // Images from other applications are centered in the view
        let outside = Clip::from_rgba8(2, 2, vec![255; 16]);
        assert_eq!(
            placement(&outside, 100, 100, [10.0, 20.0, 30.0, 40.0], false),
            (19, 29)
        );
    }

    #[test]
    fn paste_keeps_pixels_outside_the_canvas() {
        let mut doc = Document::new_with_background("t", 2, 2, Color::WHITE);
        let clip = Clip::from_rgba8(2, 1, vec![0, 0, 0, 255, 9, 9, 9, 255]);
        let id = paste(&mut doc, &clip, (1, 1));
        let px = doc.composite_rgba8();
        assert_eq!(&px[12..16], [0, 0, 0, 255]);
        assert_eq!(&px[8..12], [255, 255, 255, 255]);
        // The second pixel landed past the right edge and is kept
        let image = doc.layer(id).unwrap().image().unwrap();
        assert_eq!(image.pixel_at(2, 1), [9, 9, 9, 255]);
        assert_eq!(image.content_bounds(), Some((1, 1, 3, 2)));
    }
}
