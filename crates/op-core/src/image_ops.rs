//! Image menu operations that reshape the canvas: Image Rotation, the
//! canvas flips, Crop and Trim.

use crate::document::Document;

/// Image > Image Rotation (fixed angles and flips).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Rotate180,
    Rotate90Clockwise,
    Rotate90CounterClockwise,
    FlipHorizontal,
    FlipVertical,
}

impl Orientation {
    /// The history name Photoshop uses.
    pub fn history_name(self) -> &'static str {
        match self {
            Self::Rotate180 | Self::Rotate90Clockwise | Self::Rotate90CounterClockwise => {
                "Rotate Canvas"
            }
            Self::FlipHorizontal => "Flip Canvas Horizontal",
            Self::FlipVertical => "Flip Canvas Vertical",
        }
    }
}

/// Rotates or flips the whole document: every layer and the selection.
pub fn reorient(doc: &mut Document, orientation: Orientation) {
    let (w, h) = (doc.width, doc.height);
    let (nw, nh) = match orientation {
        Orientation::Rotate90Clockwise | Orientation::Rotate90CounterClockwise => (h, w),
        _ => (w, h),
    };
    // Source pixel of each destination pixel
    let source = move |x: u32, y: u32| match orientation {
        Orientation::Rotate180 => (w - 1 - x, h - 1 - y),
        Orientation::Rotate90Clockwise => (y, h - 1 - x),
        Orientation::Rotate90CounterClockwise => (w - 1 - y, x),
        Orientation::FlipHorizontal => (w - 1 - x, y),
        Orientation::FlipVertical => (x, h - 1 - y),
    };
    doc.transform_canvas(
        nw,
        nh,
        |image| image.remapped(nw, nh, source),
        |selection| selection.remapped(nw, nh, source),
    );
}

/// Cuts the canvas down to x0..x1 × y0..y1 (exclusive ends, inside the
/// canvas). The selection keeps its place on the image.
pub fn crop(doc: &mut Document, x0: u32, y0: u32, x1: u32, y1: u32) {
    assert!(x0 < x1 && y0 < y1 && x1 <= doc.width && y1 <= doc.height);
    let (w, h) = (x1 - x0, y1 - y0);
    let (dx, dy) = (-(x0 as i64), -(y0 as i64));
    doc.transform_canvas(
        w,
        h,
        |image| image.with_canvas(w, h, dx, dy, [0; 4]),
        |selection| selection.with_canvas(w, h, dx, dy),
    );
}

/// Image > Crop: crops to the selection's bounding box. Returns false
/// without a selection.
pub fn crop_to_selection(doc: &mut Document) -> bool {
    let Some((x0, y0, x1, y1)) = doc.selection().and_then(|s| s.bounds()) else {
        return false;
    };
    crop(doc, x0, y0, x1, y1);
    true
}

/// What Image > Trim removes ("Based On").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrimBasis {
    /// Fully transparent pixels.
    Transparent,
    /// Pixels the color of the top-left pixel.
    TopLeftColor,
    /// Pixels the color of the bottom-right pixel.
    BottomRightColor,
}

/// Which sides Image > Trim cuts ("Trim Away").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrimSides {
    pub top: bool,
    pub left: bool,
    pub bottom: bool,
    pub right: bool,
}

impl Default for TrimSides {
    fn default() -> Self {
        Self {
            top: true,
            left: true,
            bottom: true,
            right: true,
        }
    }
}

/// The area Image > Trim keeps (x0, y0, x1, y1; exclusive ends), judged on
/// the merged image. `None` when every pixel would be trimmed away.
pub fn trim_bounds(
    doc: &Document,
    basis: TrimBasis,
    sides: TrimSides,
) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = (doc.width, doc.height);
    if w == 0 || h == 0 {
        return None;
    }
    let pixels = doc.composite_rgba8();
    let at = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        pixels[i..i + 4].try_into().unwrap()
    };
    let reference = match basis {
        TrimBasis::Transparent => None,
        TrimBasis::TopLeftColor => Some(at(0, 0)),
        TrimBasis::BottomRightColor => Some(at(w - 1, h - 1)),
    };
    let trimmed = |p: [u8; 4]| match reference {
        None => p[3] == 0,
        Some(r) => p == r,
    };
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if !trimmed(at(x, y)) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x0 >= x1 {
        return None;
    }
    Some((
        if sides.left { x0 } else { 0 },
        if sides.top { y0 } else { 0 },
        if sides.right { x1 } else { w },
        if sides.bottom { y1 } else { h },
    ))
}

/// Image > Trim. Returns whether the canvas changed.
pub fn trim(doc: &mut Document, basis: TrimBasis, sides: TrimSides) -> bool {
    match trim_bounds(doc, basis, sides) {
        Some(b) if b != (0, 0, doc.width, doc.height) => {
            crop(doc, b.0, b.1, b.2, b.3);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::layer::LayerKind;
    use crate::selection::{Rect, Selection};

    /// 3×2 white background with red at (0, 0) and blue at (2, 1).
    fn doc() -> Document {
        let mut doc = Document::new_with_background("t", 3, 2, Color::WHITE);
        let id = doc.active_layer.unwrap();
        let LayerKind::Raster(image) = &mut doc.layer_mut(id).unwrap().kind;
        image.set_pixel(0, 0, [255, 0, 0, 255]);
        image.set_pixel(2, 1, [0, 0, 255, 255]);
        doc
    }

    fn pixel(doc: &Document, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * doc.width + x) * 4) as usize;
        doc.composite_rgba8()[i..i + 4].try_into().unwrap()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    #[test]
    fn rotations_and_flips_move_the_corners() {
        let mut d = doc();
        reorient(&mut d, Orientation::Rotate90Clockwise);
        assert_eq!((d.width, d.height), (2, 3));
        assert_eq!(pixel(&d, 1, 0), RED);
        assert_eq!(pixel(&d, 0, 2), BLUE);

        let mut d = doc();
        reorient(&mut d, Orientation::Rotate90CounterClockwise);
        assert_eq!(pixel(&d, 0, 2), RED);
        assert_eq!(pixel(&d, 1, 0), BLUE);

        let mut d = doc();
        reorient(&mut d, Orientation::Rotate180);
        assert_eq!(pixel(&d, 2, 1), RED);
        assert_eq!(pixel(&d, 0, 0), BLUE);

        let mut d = doc();
        reorient(&mut d, Orientation::FlipHorizontal);
        assert_eq!(pixel(&d, 2, 0), RED);
        reorient(&mut d, Orientation::FlipVertical);
        assert_eq!(pixel(&d, 2, 1), RED);
    }

    #[test]
    fn the_selection_turns_with_the_canvas() {
        let mut d = doc();
        let s = Selection::rect(3, 2, Rect::new(0.0, 0.0, 1.0, 1.0));
        d.set_selection(Some(s));
        reorient(&mut d, Orientation::Rotate90Clockwise);
        assert_eq!(d.selection().unwrap().bounds(), Some((1, 0, 2, 1)));
    }

    #[test]
    fn crop_keeps_the_selected_area() {
        let mut d = doc();
        let s = Selection::rect(3, 2, Rect::new(1.0, 1.0, 3.0, 2.0));
        d.set_selection(Some(s));
        assert!(crop_to_selection(&mut d));
        assert_eq!((d.width, d.height), (2, 1));
        assert_eq!(pixel(&d, 1, 0), BLUE);
        assert_eq!(d.selection().unwrap().bounds(), Some((0, 0, 2, 1)));
        d.set_selection(None);
        assert!(!crop_to_selection(&mut d));
    }

    #[test]
    fn trim_removes_borders_of_the_corner_color() {
        let mut d = Document::new_with_background("t", 5, 5, Color::WHITE);
        let id = d.active_layer.unwrap();
        let LayerKind::Raster(image) = &mut d.layer_mut(id).unwrap().kind;
        image.set_pixel(2, 1, RED);
        image.set_pixel(3, 3, RED);
        assert_eq!(
            trim_bounds(&d, TrimBasis::TopLeftColor, TrimSides::default()),
            Some((2, 1, 4, 4))
        );
        let only_top = TrimSides {
            top: true,
            left: false,
            bottom: false,
            right: false,
        };
        assert_eq!(
            trim_bounds(&d, TrimBasis::BottomRightColor, only_top),
            Some((0, 1, 5, 5))
        );
        // Nothing transparent on an opaque background
        assert!(!trim(&mut d, TrimBasis::Transparent, TrimSides::default()));
        assert!(trim(&mut d, TrimBasis::TopLeftColor, TrimSides::default()));
        assert_eq!((d.width, d.height), (2, 3));
    }
}
