//! Edit > Free Transform and Edit > Transform: moving, scaling, rotating
//! and flipping the selected and linked layers (or the selected pixels) by an
//! affine transform, resampled bilinearly.

use crate::document::Document;
use crate::layer::LayerId;
use crate::selection::Selection;
use crate::tile::TiledImage;

/// A 2-D affine map: `x' = a·x + b·y + c`, `y' = d·x + e·y + f`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
}

impl Affine {
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 1.0,
        f: 0.0,
    };

    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            c: x,
            f: y,
            ..Self::IDENTITY
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            e: sy,
            ..Self::IDENTITY
        }
    }

    /// Rotation by `angle` radians (clockwise on screen, y pointing down).
    pub fn rotate(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            a: c,
            b: -s,
            d: s,
            e: c,
            ..Self::IDENTITY
        }
    }

    /// `self` applied after `first`.
    pub fn after(self, first: Self) -> Self {
        Self {
            a: self.a * first.a + self.b * first.d,
            b: self.a * first.b + self.b * first.e,
            c: self.a * first.c + self.b * first.f + self.c,
            d: self.d * first.a + self.e * first.d,
            e: self.d * first.b + self.e * first.e,
            f: self.d * first.c + self.e * first.f + self.f,
        }
    }

    pub fn apply(self, (x, y): (f32, f32)) -> (f32, f32) {
        (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        )
    }

    pub fn inverse(self) -> Option<Self> {
        let det = self.a * self.e - self.b * self.d;
        if det.abs() < 1e-9 {
            return None;
        }
        let (a, b, d, e) = (self.e / det, -self.b / det, -self.d / det, self.a / det);
        Some(Self {
            a,
            b,
            c: -(a * self.c + b * self.f),
            d,
            e,
            f: -(d * self.c + e * self.f),
        })
    }

    /// Scale `sx`, `sy` and rotate `angle` around `center`, then move by
    /// `offset`: what a Free Transform box describes.
    pub fn around(center: (f32, f32), sx: f32, sy: f32, angle: f32, offset: (f32, f32)) -> Self {
        Self::translate(center.0 + offset.0, center.1 + offset.1)
            .after(Self::rotate(angle))
            .after(Self::scale(sx, sy))
            .after(Self::translate(-center.0, -center.1))
    }
}

/// Why a transform can't be done; the messages match Photoshop's alerts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformError {
    NoLayer,
    Hidden,
    Locked,
    /// Nothing to transform: the layer (or the selection on it) is empty.
    Empty,
}

impl TransformError {
    pub fn message(self, command: &str) -> String {
        match self {
            Self::NoLayer => {
                format!("Could not complete the {command} command because there is no layer.")
            }
            Self::Hidden => format!(
                "Could not complete the {command} command because the target layer is hidden."
            ),
            Self::Locked => {
                format!("Could not complete the {command} command because the layer is locked.")
            }
            Self::Empty => format!(
                "Could not complete the {command} command because the selected area is empty."
            ),
        }
    }
}

/// The pixel layers a transform changes: with a selection, the active
/// layer (or the layers in the active group); without one, also the other
/// selected layers and the layers linked to them, as in Photoshop, leaving
/// out those that are hidden, locked or the background.
fn targets(doc: &Document) -> Vec<LayerId> {
    let Some(active) = doc.active_layer else {
        return Vec::new();
    };
    if doc.selection().is_some() {
        return doc.pixel_layers(&[active]);
    }
    doc.pixel_layers(&crate::link::with_linked(doc))
        .into_iter()
        .filter(|&id| {
            id == active
                || doc.layer(id).is_some_and(|l| {
                    doc.is_shown(id)
                        && !l.is_background
                        && !doc.pixels_locked(id)
                        && !doc.position_locked(id)
                })
        })
        .collect()
}

/// What a transform acts on, as (x0, y0, x1, y1): the selection's bounds,
/// or the box around the target layers' non-transparent pixels. Also checks
/// that the active layer can be transformed: the background only with a
/// selection.
pub fn bounds(doc: &Document) -> Result<(f32, f32, f32, f32), TransformError> {
    let layer = doc
        .active_layer
        .and_then(|id| doc.layer(id))
        .ok_or(TransformError::NoLayer)?;
    if !doc.is_shown(layer.id) {
        return Err(TransformError::Hidden);
    }
    let selection = doc.selection();
    if doc.pixels_locked(layer.id)
        || doc.position_locked(layer.id)
        || (layer.is_background && selection.is_none())
    {
        return Err(TransformError::Locked);
    }
    // Without a selection the box is around all the target layers' pixels
    // (a group's layers, the other selected and the linked layers)
    if selection.is_none() {
        let (x0, y0, x1, y1) = targets(doc)
            .into_iter()
            .filter_map(|p| doc.layer(p)?.image()?.content_bounds())
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            .ok_or(TransformError::Empty)?;
        return Ok((x0 as f32, y0 as f32, x1 as f32, y1 as f32));
    }
    let Some(image) = layer.image() else {
        return Err(TransformError::Empty);
    };
    let (w, h) = (doc.width, doc.height);
    let b = match selection {
        // With a selection, the box is the selection's (as in Photoshop),
        // once it covers some of the layer's pixels
        Some(s) => {
            let covered =
                (0..h).any(|y| (0..w).any(|x| image.pixel(x, y)[3] > 0 && s.get(x, y) > 0));
            covered
                .then(|| s.bounds())
                .flatten()
                .map(|(x0, y0, x1, y1)| (x0 as i64, y0 as i64, x1 as i64, y1 as i64))
        }
        // Otherwise all of the layer's pixels, including those outside
        // the canvas
        None => image.content_bounds(),
    };
    let (x0, y0, x1, y1) = b.ok_or(TransformError::Empty)?;
    Ok((x0 as f32, y0 as f32, x1 as f32, y1 as f32))
}

/// Bilinear sample of premultiplied RGBA at (x, y) in pixel-center
/// coordinates; transparent outside the image.
fn sample(px: &[[f32; 4]], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    let (fx, fy) = (x - 0.5, y - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let at = |ix: f32, iy: f32| -> [f32; 4] {
        if ix < 0.0 || iy < 0.0 || ix >= w as f32 || iy >= h as f32 {
            [0.0; 4]
        } else {
            px[iy as usize * w + ix as usize]
        }
    };
    let (p00, p10, p01, p11) = (
        at(x0, y0),
        at(x0 + 1.0, y0),
        at(x0, y0 + 1.0),
        at(x0 + 1.0, y0 + 1.0),
    );
    let mut out = [0.0; 4];
    for c in 0..4 {
        let top = p00[c] + (p10[c] - p00[c]) * tx;
        let bottom = p01[c] + (p11[c] - p01[c]) * tx;
        out[c] = top + (bottom - top) * ty;
    }
    out
}

/// Applies `m` (source → destination, in document pixels) to the active
/// layer. With a selection only the selected pixels move: the place they
/// leave becomes transparent (the background color on the background
/// layer), and the selection moves with them. Pixels outside the canvas
/// move too and stay on the layer, as in Photoshop; the background layer
/// ends at the canvas.
pub fn transform(doc: &mut Document, m: Affine, background: [u8; 3]) -> Result<(), TransformError> {
    let src_box = bounds(doc)?;
    let inverse = m.inverse().ok_or(TransformError::Empty)?;
    let selection = doc.selection().cloned();
    let (cw, ch) = (doc.width as i64, doc.height as i64);
    // Every target layer, with the same map
    for id in targets(doc) {
        let layer = doc.layer_mut(id).expect("checked");
        let is_background = layer.is_background;
        let Some(image) = layer.image_mut() else {
            continue;
        };

        // The region worked on: the canvas, the layer's pixels and where the
        // box lands
        let (bx0, by0, bx1, by1) = src_box;
        let corners = [(bx0, by0), (bx1, by0), (bx1, by1), (bx0, by1)].map(|p| m.apply(p));
        let fx0 = corners.iter().map(|c| c.0).fold(f32::MAX, f32::min).floor() as i64;
        let fy0 = corners.iter().map(|c| c.1).fold(f32::MAX, f32::min).floor() as i64;
        let fx1 = corners.iter().map(|c| c.0).fold(f32::MIN, f32::max).ceil() as i64;
        let fy1 = corners.iter().map(|c| c.1).fold(f32::MIN, f32::max).ceil() as i64;
        let (cx0, cy0, cx1, cy1) = image.content_bounds().unwrap_or((0, 0, cw, ch));
        let (ux0, uy0) = (0.min(cx0).min(fx0), 0.min(cy0).min(fy0));
        let (ux1, uy1) = (cw.max(cx1).max(fx1), ch.max(cy1).max(fy1));
        let (w, h) = ((ux1 - ux0) as usize, (uy1 - uy0) as usize);

        // The moving pixels (premultiplied) and what stays behind (straight)
        let raw = image.region_rgba8(ux0, uy0, w as u32, h as u32);
        let mut moving = vec![[0f32; 4]; w * h];
        let mut staying = raw.clone();
        for i in 0..w * h {
            let (x, y) = (ux0 + (i % w) as i64, uy0 + (i / w) as i64);
            let m = match &selection {
                None => 1.0,
                Some(s) if x >= 0 && y >= 0 && x < cw && y < ch => {
                    s.get(x as u32, y as u32) as f32 / 255.0
                }
                Some(_) => 0.0,
            };
            if m <= 0.0 {
                continue;
            }
            let p = &raw[i * 4..i * 4 + 4];
            let a = p[3] as f32 * m / 255.0;
            moving[i] = [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, a * 255.0];
            let left = &mut staying[i * 4..i * 4 + 4];
            if is_background {
                for c in 0..3 {
                    left[c] = (left[c] as f32 * (1.0 - m) + background[c] as f32 * m).round() as u8;
                }
            } else {
                left[3] = (left[3] as f32 * (1.0 - m)).round() as u8;
            }
        }

        // Composite the moved pixels over what stayed
        let mut out = staying;
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = ((ux0 + x as i64) as f32 + 0.5, (uy0 + y as i64) as f32 + 0.5);
                let (sx, sy) = inverse.apply((dx, dy));
                let s = sample(&moving, w, h, sx - ux0 as f32, sy - uy0 as f32);
                let sa = s[3] / 255.0;
                if sa <= 0.0 {
                    continue;
                }
                let i = (y * w + x) * 4;
                let d = &mut out[i..i + 4];
                let da = d[3] as f32 / 255.0;
                let oa = sa + da * (1.0 - sa);
                for c in 0..3 {
                    let v = (s[c] + d[c] as f32 * da * (1.0 - sa)) / oa;
                    d[c] = v.round().clamp(0.0, 255.0) as u8;
                }
                d[3] = (oa * 255.0).round() as u8;
            }
        }
        *image = TiledImage::from_region(cw as u32, ch as u32, ux0, uy0, w as u32, h as u32, &out);
        if is_background {
            *image = image.clipped();
        }
    }

    if let Some(s) = selection {
        let (w, h) = (cw as usize, ch as usize);
        let mut mask = vec![0u8; w * h];
        let src: Vec<[f32; 4]> = (0..w * h)
            .map(|i| {
                let v = s.get((i % w) as u32, (i / w) as u32) as f32;
                [v, 0.0, 0.0, 0.0]
            })
            .collect();
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = inverse.apply((x as f32 + 0.5, y as f32 + 0.5));
                mask[y * w + x] = sample(&src, w, h, sx, sy)[0].round().clamp(0.0, 255.0) as u8;
            }
        }
        doc.set_selection(Some(Selection::from_mask(w as u32, h as u32, mask, false)));
    }
    doc.mark_dirty();
    Ok(())
}

/// The fixed transforms of Edit > Transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixedTransform {
    Rotate180,
    Rotate90Clockwise,
    Rotate90CounterClockwise,
    FlipHorizontal,
    FlipVertical,
}

impl FixedTransform {
    /// The map around the center of `bounds`.
    pub fn affine(self, (x0, y0, x1, y1): (f32, f32, f32, f32)) -> Affine {
        let center = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (sx, sy, angle) = match self {
            Self::Rotate180 => (1.0, 1.0, std::f32::consts::PI),
            Self::Rotate90Clockwise => (1.0, 1.0, std::f32::consts::FRAC_PI_2),
            Self::Rotate90CounterClockwise => (1.0, 1.0, -std::f32::consts::FRAC_PI_2),
            Self::FlipHorizontal => (-1.0, 1.0, 0.0),
            Self::FlipVertical => (1.0, -1.0, 0.0),
        };
        Affine::around(center, sx, sy, angle, (0.0, 0.0))
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Rotate180 => "Rotate 180°",
            Self::Rotate90Clockwise => "Rotate 90° Clockwise",
            Self::Rotate90CounterClockwise => "Rotate 90° Counter Clockwise",
            Self::FlipHorizontal => "Flip Horizontal",
            Self::FlipVertical => "Flip Vertical",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::layer::Layer;
    use crate::selection::Rect;

    /// 6×6 white background plus a layer with a 2×2 red square at (1, 1).
    fn doc() -> Document {
        let mut doc = Document::new_with_background("t", 6, 6, Color::WHITE);
        let mut image = TiledImage::new(6, 6);
        for y in 1..3 {
            for x in 1..3 {
                image.set_pixel(x, y, [255, 0, 0, 255]);
            }
        }
        doc.insert_above_active(Layer::raster(doc.new_layer_id(), "Layer 1", image));
        doc
    }

    fn layer_px(doc: &Document, x: u32, y: u32) -> [u8; 4] {
        let image = doc.layers[1].image().unwrap();
        image.pixel(x, y)
    }

    #[test]
    fn affine_math() {
        let m = Affine::around((1.0, 1.0), 2.0, 2.0, 0.0, (3.0, 0.0));
        assert_eq!(m.apply((2.0, 1.0)), (6.0, 1.0));
        let back = m.inverse().unwrap().apply((6.0, 1.0));
        assert!((back.0 - 2.0).abs() < 1e-5 && (back.1 - 1.0).abs() < 1e-5);
        let r = Affine::rotate(std::f32::consts::FRAC_PI_2).apply((1.0, 0.0));
        assert!(r.0.abs() < 1e-6 && (r.1 - 1.0).abs() < 1e-6);
        assert!(Affine::scale(0.0, 1.0).inverse().is_none());
    }

    #[test]
    fn bounds_and_locks() {
        let mut d = doc();
        assert_eq!(bounds(&d), Ok((1.0, 1.0, 3.0, 3.0)));
        d.active_layer = Some(d.layers[0].id);
        assert_eq!(bounds(&d), Err(TransformError::Locked));
        d.set_selection(Some(Selection::rect(6, 6, Rect::new(0.0, 0.0, 4.0, 4.0))));
        assert_eq!(bounds(&d), Ok((0.0, 0.0, 4.0, 4.0)));
    }

    /// Layer 1 and a second layer selected, a third linked to the second
    /// and a fourth locked but linked: the box spans the three movable
    /// ones and they move together; the locked one stays.
    #[test]
    fn selected_and_linked_layers_transform_together() {
        let mut d = doc();
        let one = d.layers[1].id;
        let dot = |d: &mut Document, x: u32, y: u32| {
            let mut image = TiledImage::new(6, 6);
            image.set_pixel(x, y, [0, 0, 255, 255]);
            let id = d.new_layer_id();
            d.layers.push(Layer::raster(id, "dot", image));
            id
        };
        let two = dot(&mut d, 4, 4);
        let three = dot(&mut d, 5, 0);
        let locked = dot(&mut d, 0, 5);
        d.layer_mut(locked).unwrap().lock_position = true;
        d.set_selected_layers(vec![two, three, locked]);
        crate::link::link_selected(&mut d);
        d.set_selected_layers(vec![two, one]);
        assert_eq!(bounds(&d).unwrap(), (1.0, 0.0, 6.0, 5.0));
        transform(&mut d, Affine::translate(0.0, 1.0), [255; 3]).unwrap();
        let px = |d: &Document, id, x, y| d.layer(id).unwrap().image().unwrap().pixel(x, y);
        assert_eq!(px(&d, one, 1, 2), [255, 0, 0, 255]);
        assert_eq!(px(&d, two, 4, 5), [0, 0, 255, 255]);
        assert_eq!(px(&d, three, 5, 1), [0, 0, 255, 255]);
        assert_eq!(px(&d, locked, 0, 5), [0, 0, 255, 255]);
        // With a selection only the active layer's selected pixels move
        d.set_selection(Some(Selection::rect(6, 6, Rect::new(0.0, 0.0, 6.0, 6.0))));
        transform(&mut d, Affine::translate(0.0, -1.0), [255; 3]).unwrap();
        assert_eq!(px(&d, one, 1, 1), [255, 0, 0, 255]);
        assert_eq!(px(&d, two, 4, 5), [0, 0, 255, 255]);
    }

    #[test]
    fn move_scale_and_flip_the_layer() {
        let mut d = doc();
        transform(&mut d, Affine::translate(3.0, 2.0), [255; 3]).unwrap();
        assert_eq!(layer_px(&d, 4, 3), [255, 0, 0, 255]);
        assert_eq!(layer_px(&d, 1, 1)[3], 0);

        let mut d = doc();
        let double = Affine::around((2.0, 2.0), 2.0, 2.0, 0.0, (0.0, 0.0));
        transform(&mut d, double, [255; 3]).unwrap();
        // 0..4 after scaling; the outer ring is softened by interpolation
        assert_eq!(layer_px(&d, 1, 1), [255, 0, 0, 255]);
        assert_eq!(layer_px(&d, 2, 2), [255, 0, 0, 255]);
        assert!(layer_px(&d, 0, 0)[3] > 0 && layer_px(&d, 4, 4)[3] < 255);

        let mut d = doc();
        let b = bounds(&d).unwrap();
        let flip = FixedTransform::FlipHorizontal.affine((b.0, b.1, b.2 + 2.0, b.3));
        transform(&mut d, flip, [255; 3]).unwrap();
        assert_eq!(layer_px(&d, 3, 1), [255, 0, 0, 255]);
        assert_eq!(layer_px(&d, 1, 1)[3], 0);
    }

    #[test]
    fn selected_pixels_move_and_leave_the_background_color() {
        let mut d = doc();
        d.active_layer = Some(d.layers[0].id);
        let image = d.layers[0].image_mut().unwrap();
        image.set_pixel(0, 0, [0, 0, 255, 255]);
        d.set_selection(Some(Selection::rect(6, 6, Rect::new(0.0, 0.0, 1.0, 1.0))));
        transform(&mut d, Affine::translate(5.0, 0.0), [0, 255, 0]).unwrap();
        let image = d.layers[0].image().unwrap();
        assert_eq!(image.pixel(5, 0), [0, 0, 255, 255]);
        assert_eq!(image.pixel(0, 0), [0, 255, 0, 255]);
        assert_eq!(d.selection().unwrap().bounds(), Some((5, 0, 6, 1)));
    }

    #[test]
    fn transforming_takes_pixels_outside_the_canvas_along() {
        let mut doc = Document::new_with_background("t", 10, 10, crate::Color::WHITE);
        let id = doc.new_layer_id();
        let mut image = TiledImage::new(10, 10);
        // A 2 × 1 bar half outside the left edge
        image.set_pixel_at(-1, 4, [255, 0, 0, 255]);
        image.set_pixel_at(0, 4, [255, 0, 0, 255]);
        doc.layers.push(crate::Layer::raster(id, "L", image));
        doc.active_layer = Some(id);
        assert_eq!(bounds(&doc), Ok((-1.0, 4.0, 1.0, 5.0)));
        // Moving it right by 3 brings the outside pixel in
        transform(&mut doc, Affine::translate(3.0, 0.0), [0; 3]).unwrap();
        let image = doc.layer(id).unwrap().image().unwrap();
        assert_eq!(image.pixel(2, 4), [255, 0, 0, 255]);
        assert_eq!(image.pixel(3, 4), [255, 0, 0, 255]);
        assert!(!image.has_pixels_outside());
        // Moving it up by 8 puts it outside, where it is kept
        transform(&mut doc, Affine::translate(0.0, -8.0), [0; 3]).unwrap();
        let image = doc.layer(id).unwrap().image().unwrap();
        assert_eq!(image.content_bounds(), Some((2, -4, 4, -3)));
    }
}
