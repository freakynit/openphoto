//! Image menu operations that reshape the canvas: Image Rotation, the
//! canvas flips, Crop and Trim.

use crate::document::{Document, Guide};

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
    let (w, h) = (w as f32, h as f32);
    doc.map_guides(|g| {
        let p = g.position;
        match (orientation, g.vertical) {
            (Orientation::Rotate180, v) => Guide {
                vertical: v,
                position: if v { w - p } else { h - p },
            },
            // Clockwise: a vertical guide at x becomes horizontal at x;
            // a horizontal guide at y becomes vertical at h - y
            (Orientation::Rotate90Clockwise, true) => Guide {
                vertical: false,
                position: p,
            },
            (Orientation::Rotate90Clockwise, false) => Guide {
                vertical: true,
                position: h - p,
            },
            (Orientation::Rotate90CounterClockwise, true) => Guide {
                vertical: false,
                position: w - p,
            },
            (Orientation::Rotate90CounterClockwise, false) => Guide {
                vertical: true,
                position: p,
            },
            (Orientation::FlipHorizontal, true) => Guide {
                vertical: true,
                position: w - p,
            },
            (Orientation::FlipVertical, false) => Guide {
                vertical: false,
                position: h - p,
            },
            _ => g,
        }
    });
}

/// Image > Reveal All: grows the canvas so every layer's pixels, including
/// the ones outside it, are on it. The background layer is extended with
/// `background`. Returns false (and changes nothing) when nothing lies
/// outside the canvas.
pub fn reveal_all(doc: &mut Document, background: crate::Color) -> bool {
    let (x0, y0, x1, y1) = doc.content_bounds();
    if (x0, y0, x1, y1) == (0, 0, doc.width as i64, doc.height as i64) {
        return false;
    }
    doc.place_canvas((x1 - x0) as u32, (y1 - y0) as u32, -x0, -y0, background);
    true
}

/// Cuts the canvas down to x0..x1 × y0..y1 (exclusive ends, inside the
/// canvas). The selection keeps its place on the image. Pixels outside the
/// new canvas are deleted (Photoshop's Delete Cropped Pixels).
pub fn crop(doc: &mut Document, x0: u32, y0: u32, x1: u32, y1: u32) {
    assert!(x0 < x1 && y0 < y1 && x1 <= doc.width && y1 <= doc.height);
    let (w, h) = (x1 - x0, y1 - y0);
    let (dx, dy) = (-(x0 as i64), -(y0 as i64));
    doc.transform_canvas(
        w,
        h,
        |image| image.with_canvas(w, h, dx, dy, [0; 4]).clipped(),
        |selection| selection.with_canvas(w, h, dx, dy),
    );
    doc.map_guides(|g| Guide {
        position: g.position - if g.vertical { x0 } else { y0 } as f32,
        ..g
    });
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

/// Image > Image Size's resampling methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Resample {
    /// Bicubic (smooth gradients): Photoshop's general-purpose default.
    #[default]
    Bicubic,
    Bilinear,
    NearestNeighbor,
}

impl Resample {
    pub const ALL: [Self; 3] = [Self::Bicubic, Self::Bilinear, Self::NearestNeighbor];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bicubic => "Bicubic (smooth gradients)",
            Self::Bilinear => "Bilinear",
            Self::NearestNeighbor => "Nearest Neighbor (hard edges)",
        }
    }

    /// Filter weight at distance `x` (in source pixels) and its radius.
    fn weight(self, x: f32) -> f32 {
        let x = x.abs();
        match self {
            // Catmull-Rom-like cubic (a = -0.5)
            Self::Bicubic => {
                let a = -0.5;
                if x < 1.0 {
                    (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0
                } else if x < 2.0 {
                    a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a
                } else {
                    0.0
                }
            }
            Self::Bilinear => (1.0 - x).max(0.0),
            Self::NearestNeighbor => unreachable!("sampled directly"),
        }
    }

    fn radius(self) -> f32 {
        match self {
            Self::Bicubic => 2.0,
            Self::Bilinear => 1.0,
            Self::NearestNeighbor => 0.5,
        }
    }
}

/// Resamples `channels`-wide rows of `src` (sw × sh) to dw × dh, one axis
/// at a time. When shrinking, the filter widens to cover every source
/// pixel (area-correct downsampling).
fn resample_buffer(
    src: &[f32],
    sw: usize,
    sh: usize,
    dw: usize,
    dh: usize,
    ch: usize,
    method: Resample,
) -> Vec<f32> {
    let pass = |src: &[f32], sw: usize, sh: usize, dw: usize, horizontal: bool| -> Vec<f32> {
        // `n` = length along the resampled axis, `m` = the other axis
        let (n_src, n_dst, m) = if horizontal {
            (sw, dw, sh)
        } else {
            (sh, dw, sw)
        };
        let scale = n_dst as f32 / n_src as f32;
        let support = if scale < 1.0 { 1.0 / scale } else { 1.0 };
        let mut out = vec![0f32; ch * if horizontal { dw * sh } else { sw * dw }];
        for i in 0..n_dst {
            let center = (i as f32 + 0.5) / scale - 0.5;
            let taps: Vec<(usize, f32)> = if method == Resample::NearestNeighbor {
                vec![((center.round().max(0.0) as usize).min(n_src - 1), 1.0)]
            } else {
                let r = method.radius() * support;
                let lo = (center - r).floor() as isize;
                let hi = (center + r).ceil() as isize;
                let mut taps: Vec<(usize, f32)> = (lo..=hi)
                    .map(|j| {
                        let w = method.weight((j as f32 - center) / support);
                        (j.clamp(0, n_src as isize - 1) as usize, w)
                    })
                    .filter(|(_, w)| *w != 0.0)
                    .collect();
                let sum: f32 = taps.iter().map(|(_, w)| w).sum();
                taps.iter_mut().for_each(|(_, w)| *w /= sum);
                taps
            };
            for k in 0..m {
                let mut acc = vec![0f32; ch];
                for &(j, w) in &taps {
                    let idx = if horizontal { k * sw + j } else { j * sw + k };
                    for c in 0..ch {
                        acc[c] += src[idx * ch + c] * w;
                    }
                }
                let o = if horizontal { k * dw + i } else { i * sw + k };
                out[o * ch..o * ch + ch].copy_from_slice(&acc);
            }
        }
        out
    };
    let horizontal = pass(src, sw, sh, dw, true);
    pass(&horizontal, dw, sh, dh, false)
}

/// Image > Image Size with resampling: every layer and the selection are
/// scaled to `width` × `height`. Colors are resampled premultiplied by
/// alpha; results are clamped (bicubic can overshoot).
pub fn resize(doc: &mut Document, width: u32, height: u32, method: Resample) {
    assert!(width > 0 && height > 0);
    let (sw, sh) = (doc.width as usize, doc.height as usize);
    let (kx, ky) = (width as f32 / sw as f32, height as f32 / sh as f32);
    doc.map_guides(|g| Guide {
        position: g.position * if g.vertical { kx } else { ky },
        ..g
    });
    let (dw, dh) = (width as usize, height as usize);
    doc.transform_canvas(
        width,
        height,
        |image| {
            let raw = image.to_rgba8();
            let src: Vec<f32> = raw
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|&[r, g, b, a]| {
                    let k = a as f32 / 255.0;
                    [r as f32 * k, g as f32 * k, b as f32 * k, a as f32]
                })
                .collect();
            let out = resample_buffer(&src, sw, sh, dw, dh, 4, method);
            let pixels: Vec<u8> = out
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|&[r, g, b, a]| {
                    let a = a.clamp(0.0, 255.0);
                    if a < 0.5 {
                        return [0; 4];
                    }
                    let k = 255.0 / a;
                    [
                        (r * k).round().clamp(0.0, 255.0) as u8,
                        (g * k).round().clamp(0.0, 255.0) as u8,
                        (b * k).round().clamp(0.0, 255.0) as u8,
                        a.round() as u8,
                    ]
                })
                .collect();
            crate::tile::TiledImage::from_rgba8(width, height, &pixels)
        },
        |selection| {
            let src: Vec<f32> = (0..sw * sh)
                .map(|i| selection.get((i % sw) as u32, (i / sw) as u32) as f32)
                .collect();
            let out = resample_buffer(&src, sw, sh, dw, dh, 1, method);
            let mask = out
                .iter()
                .map(|v| v.round().clamp(0.0, 255.0) as u8)
                .collect();
            crate::selection::Selection::from_mask(width, height, mask, false)
        },
    );
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
    fn guides_follow_the_canvas() {
        let mut d = doc();
        d.guides = vec![
            Guide {
                vertical: true,
                position: 1.0,
            },
            Guide {
                vertical: false,
                position: 0.5,
            },
        ];
        // 3×2 turned clockwise: x = 1 stays 1 as a row; y = 0.5 becomes column 1.5
        reorient(&mut d, Orientation::Rotate90Clockwise);
        assert_eq!(
            d.guides,
            [
                Guide {
                    vertical: false,
                    position: 1.0
                },
                Guide {
                    vertical: true,
                    position: 1.5
                }
            ]
        );
        crop(&mut d, 1, 0, 2, 3);
        assert_eq!(d.guides[1].position, 0.5);
        resize(&mut d, 2, 6, Resample::NearestNeighbor);
        assert_eq!((d.guides[0].position, d.guides[1].position), (2.0, 1.0));
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
    fn resize_scales_layers_and_selection() {
        let mut d = Document::new_with_background("t", 4, 2, Color::WHITE);
        let id = d.active_layer.unwrap();
        let LayerKind::Raster(image) = &mut d.layer_mut(id).unwrap().kind;
        image.set_pixel(0, 0, RED);
        image.set_pixel(1, 0, RED);
        image.set_pixel(0, 1, RED);
        image.set_pixel(1, 1, RED);
        d.set_selection(Some(Selection::rect(4, 2, Rect::new(0.0, 0.0, 2.0, 2.0))));
        // Halving: the red half becomes one (mostly) red pixel; bicubic
        // takes in a little of the white next to it
        resize(&mut d, 2, 1, Resample::Bicubic);
        assert_eq!((d.width, d.height), (2, 1));
        let red = pixel(&d, 0, 0);
        assert!(red[0] == 255 && red[1] < 40, "{red:?}");
        assert!(pixel(&d, 1, 0)[1] > 215);
        assert!(d.selection().unwrap().get(0, 0) > 215);
        // Nearest neighbor doubling keeps hard edges
        let mut d = Document::new_with_background("t", 2, 1, Color::WHITE);
        let id = d.active_layer.unwrap();
        let LayerKind::Raster(image) = &mut d.layer_mut(id).unwrap().kind;
        image.set_pixel(0, 0, RED);
        resize(&mut d, 4, 2, Resample::NearestNeighbor);
        assert_eq!(pixel(&d, 1, 1), RED);
        assert_eq!(pixel(&d, 2, 1), [255, 255, 255, 255]);
        // Bilinear doubling blends at the edge
        let mut e = doc();
        resize(&mut e, 6, 4, Resample::Bilinear);
        let edge = pixel(&e, 1, 0);
        assert!(edge[1] > 0 && edge[1] < 255, "{edge:?}");
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

    #[test]
    fn reveal_all_grows_the_canvas_to_the_hidden_pixels() {
        let mut doc = Document::new_with_background("t", 4, 4, Color::WHITE);
        let id = doc.new_layer_id();
        let mut image = crate::TiledImage::new(4, 4);
        image.set_pixel_at(-2, 1, [255, 0, 0, 255]);
        image.set_pixel_at(5, 6, [0, 0, 255, 255]);
        doc.layers.push(crate::Layer::raster(id, "L", image));
        assert!(reveal_all(&mut doc, Color::BLACK));
        assert_eq!((doc.width, doc.height), (8, 7));
        let LayerKind::Raster(image) = &doc.layer(id).unwrap().kind;
        assert_eq!(image.pixel(0, 1), [255, 0, 0, 255]);
        assert_eq!(image.pixel(7, 6), [0, 0, 255, 255]);
        // The background is extended with the background color
        let LayerKind::Raster(bg) = &doc.layers[0].kind;
        assert_eq!(bg.pixel(0, 0), [0, 0, 0, 255]);
        assert_eq!(bg.pixel(2, 0), [255, 255, 255, 255]);
        assert!(!reveal_all(&mut doc, Color::BLACK));
    }
}
