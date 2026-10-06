//! Edit > Fill, Edit > Clear and the Paint Bucket.

use crate::blend;
use crate::document::Document;
use crate::layer::{BlendMode, LayerKind};
use crate::selection::Selection;
use crate::tile::TiledImage;

/// Why a fill can't be done; the messages match Photoshop's alerts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillError {
    NoLayer,
    Locked,
    Hidden,
}

impl FillError {
    pub fn message(self, command: &str) -> String {
        match self {
            Self::NoLayer => {
                format!("Could not complete the {command} command because there is no layer.")
            }
            Self::Locked => {
                format!("Could not complete the {command} command because the layer is locked.")
            }
            Self::Hidden => format!(
                "Could not complete the {command} command because the target layer is hidden."
            ),
        }
    }
}

/// Fill settings (the Blending section of the Fill dialog).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FillOptions {
    pub mode: BlendMode,
    pub opacity: f32,
    /// Keep each pixel's alpha (only recolor existing pixels).
    pub preserve_transparency: bool,
}

impl Default for FillOptions {
    fn default() -> Self {
        Self {
            mode: BlendMode::Normal,
            opacity: 1.0,
            preserve_transparency: false,
        }
    }
}

fn target(doc: &Document) -> Result<crate::layer::LayerId, FillError> {
    let id = doc.active_layer.ok_or(FillError::NoLayer)?;
    let layer = doc.layer(id).ok_or(FillError::NoLayer)?;
    if !layer.visible {
        return Err(FillError::Hidden);
    }
    if layer.lock_pixels {
        return Err(FillError::Locked);
    }
    Ok(id)
}

/// Fills the selection (or the whole layer without one) on the active layer
/// with `color`, using `mask` as an extra coverage limit (the Paint Bucket's
/// region). Returns whether anything changed.
fn fill_masked(
    doc: &mut Document,
    color: [u8; 3],
    options: FillOptions,
    mask: Option<&Selection>,
) -> Result<bool, FillError> {
    let id = target(doc)?;
    let selection = doc.selection().cloned();
    let (w, h) = (doc.width, doc.height);
    let layer = doc.layer_mut(id).expect("target layer exists");
    // The background layer is always opaque; locked transparency keeps alpha
    let keep_alpha =
        layer.is_background || layer.lock_transparency || options.preserve_transparency;
    let LayerKind::Raster(image) = &mut layer.kind;
    let src = color.map(|v| v as f32 / 255.0);
    let mut changed = false;
    for y in 0..h {
        for x in 0..w {
            let mut amount = options.opacity;
            if let Some(s) = &selection {
                amount *= s.get(x, y) as f32 / 255.0;
            }
            if let Some(m) = mask {
                amount *= m.get(x, y) as f32 / 255.0;
            }
            if amount <= 0.0 {
                continue;
            }
            let base = image.pixel(x, y);
            if keep_alpha && base[3] == 0 {
                continue;
            }
            let dst = base.map(|v| v as f32 / 255.0);
            let out = if keep_alpha {
                // Recolor in place: blend as if the pixel were opaque, keep alpha
                let mixed = blend::composite(
                    options.mode,
                    [dst[0], dst[1], dst[2], 1.0],
                    src,
                    amount,
                    x,
                    y,
                );
                [mixed[0], mixed[1], mixed[2], dst[3]]
            } else {
                blend::composite(options.mode, dst, src, amount, x, y)
            };
            let px = out.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
            if px != base {
                image.set_pixel(x, y, px);
                changed = true;
            }
        }
    }
    doc.mark_dirty();
    Ok(changed)
}

/// Edit > Fill with a solid color.
pub fn fill(doc: &mut Document, color: [u8; 3], options: FillOptions) -> Result<(), FillError> {
    fill_masked(doc, color, options, None).map(|_| ())
}

/// Edit > Clear (Delete): erases the selection on a regular layer. On the
/// background layer (or with transparent pixels locked) Photoshop fills with
/// the background color instead.
pub fn clear(doc: &mut Document, background: [u8; 3]) -> Result<(), FillError> {
    let id = target(doc)?;
    let layer = doc.layer(id).expect("target layer exists");
    if layer.is_background || layer.lock_transparency {
        return fill(doc, background, FillOptions::default());
    }
    let selection = doc.selection().cloned();
    let (w, h) = (doc.width, doc.height);
    let layer = doc.layer_mut(id).expect("target layer exists");
    let LayerKind::Raster(image) = &mut layer.kind;
    for y in 0..h {
        for x in 0..w {
            let amount = selection
                .as_ref()
                .map_or(1.0, |s| s.get(x, y) as f32 / 255.0);
            if amount <= 0.0 {
                continue;
            }
            let [r, g, b, a] = image.pixel(x, y);
            if a > 0 {
                image.set_pixel(x, y, [r, g, b, (a as f32 * (1.0 - amount)).round() as u8]);
            }
        }
    }
    doc.mark_dirty();
    Ok(())
}

/// Paint Bucket settings (its options bar).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BucketOptions {
    pub fill: FillOptions,
    /// 0–255: how far a channel may differ from the clicked color.
    pub tolerance: u8,
    pub anti_alias: bool,
    pub contiguous: bool,
    /// Sample the merged image instead of the active layer.
    pub all_layers: bool,
}

impl Default for BucketOptions {
    /// Photoshop's defaults: tolerance 32, anti-alias and contiguous on.
    fn default() -> Self {
        Self {
            fill: FillOptions::default(),
            tolerance: 32,
            anti_alias: true,
            contiguous: true,
            all_layers: false,
        }
    }
}

/// The region a Paint Bucket click at (`x`, `y`) fills: pixels whose
/// channels all lie within `tolerance` of the clicked pixel, either
/// connected to it (4-neighborhood) or anywhere.
fn bucket_region(source: &TiledImage, x: u32, y: u32, options: &BucketOptions) -> Selection {
    let (w, h) = (source.width(), source.height());
    let seed = source.pixel(x, y);
    let tol = options.tolerance as i32;
    let similar = |px: [u8; 4]| (0..4).all(|c| (px[c] as i32 - seed[c] as i32).abs() <= tol);
    let mut mask = vec![0u8; (w * h) as usize];
    if options.contiguous {
        let mut stack = vec![(x, y)];
        while let Some((px, py)) = stack.pop() {
            let i = (py * w + px) as usize;
            if mask[i] != 0 || !similar(source.pixel(px, py)) {
                continue;
            }
            mask[i] = 255;
            if px > 0 {
                stack.push((px - 1, py));
            }
            if px + 1 < w {
                stack.push((px + 1, py));
            }
            if py > 0 {
                stack.push((px, py - 1));
            }
            if py + 1 < h {
                stack.push((px, py + 1));
            }
        }
    } else {
        for py in 0..h {
            for px in 0..w {
                if similar(source.pixel(px, py)) {
                    mask[(py * w + px) as usize] = 255;
                }
            }
        }
    }
    Selection::from_mask(w, h, mask, options.anti_alias)
}

/// Paint Bucket click at (`x`, `y`) with `color`. Returns `Ok(false)` when
/// the click is outside the document.
pub fn bucket(
    doc: &mut Document,
    x: u32,
    y: u32,
    color: [u8; 3],
    options: BucketOptions,
) -> Result<bool, FillError> {
    let id = target(doc)?;
    if x >= doc.width || y >= doc.height {
        return Ok(false);
    }
    let source = if options.all_layers {
        TiledImage::from_rgba8(doc.width, doc.height, &doc.composite_rgba8())
    } else {
        let LayerKind::Raster(image) = &doc.layer(id).expect("target layer exists").kind;
        image.clone()
    };
    let region = bucket_region(&source, x, y, &options);
    fill_masked(doc, color, options.fill, Some(&region))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::Rect;
    use crate::{Color, Layer};

    fn doc() -> Document {
        Document::new_with_background("t", 10, 10, Color::WHITE)
    }

    fn px(doc: &Document, x: u32, y: u32) -> [u8; 4] {
        let LayerKind::Raster(img) = &doc.layer(doc.active_layer.unwrap()).unwrap().kind;
        img.pixel(x, y)
    }

    #[test]
    fn fill_respects_selection_opacity_and_mode() {
        let mut d = doc();
        d.set_selection(Some(Selection::rect(
            10,
            10,
            Rect::new(0.0, 0.0, 5.0, 10.0),
        )));
        fill(
            &mut d,
            [0, 0, 0],
            FillOptions {
                opacity: 0.5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(px(&d, 2, 2), [128, 128, 128, 255]);
        assert_eq!(px(&d, 7, 2), [255, 255, 255, 255]);
        d.set_selection(None);
        fill(
            &mut d,
            [128, 128, 128],
            FillOptions {
                mode: BlendMode::Multiply,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(px(&d, 7, 2), [128, 128, 128, 255]);
    }

    #[test]
    fn clear_erases_or_fills_background() {
        let mut d = doc();
        d.set_selection(Some(Selection::rect(10, 10, Rect::new(0.0, 0.0, 5.0, 5.0))));
        clear(&mut d, [9, 9, 9]).unwrap();
        assert_eq!(px(&d, 1, 1), [9, 9, 9, 255]);
        let id = d.new_layer_id();
        d.layers.push(Layer::raster(
            id,
            "L",
            TiledImage::filled(10, 10, [1, 2, 3, 255]),
        ));
        d.active_layer = Some(id);
        clear(&mut d, [9, 9, 9]).unwrap();
        assert_eq!(px(&d, 1, 1)[3], 0);
        assert_eq!(px(&d, 8, 8), [1, 2, 3, 255]);
    }

    #[test]
    fn preserve_transparency_keeps_alpha() {
        let mut d = doc();
        let id = d.new_layer_id();
        let mut img = TiledImage::new(10, 10);
        img.set_pixel(3, 3, [0, 0, 0, 100]);
        d.layers.push(Layer::raster(id, "L", img));
        d.active_layer = Some(id);
        fill(
            &mut d,
            [255, 0, 0],
            FillOptions {
                preserve_transparency: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(px(&d, 3, 3), [255, 0, 0, 100]);
        assert_eq!(px(&d, 4, 4)[3], 0);
    }

    #[test]
    fn bucket_fills_contiguous_region() {
        let mut d = doc();
        // A black wall at x = 5 splits the white image
        for y in 0..10 {
            set(&mut d, 5, y, [0, 0, 0, 255]);
        }
        let opts = BucketOptions {
            anti_alias: false,
            ..Default::default()
        };
        bucket(&mut d, 1, 1, [255, 0, 0], opts).unwrap();
        assert_eq!(px(&d, 4, 9), [255, 0, 0, 255]);
        assert_eq!(px(&d, 5, 5), [0, 0, 0, 255]);
        assert_eq!(px(&d, 8, 8), [255, 255, 255, 255]);
        // Not contiguous: all similar pixels, on both sides of the wall
        let opts = BucketOptions {
            contiguous: false,
            anti_alias: false,
            ..Default::default()
        };
        bucket(&mut d, 8, 8, [0, 255, 0], opts).unwrap();
        assert_eq!(px(&d, 8, 8), [0, 255, 0, 255]);
        assert_eq!(px(&d, 1, 1), [255, 0, 0, 255]);
    }

    pub(super) fn set(d: &mut Document, x: u32, y: u32, rgba: [u8; 4]) {
        let id = d.active_layer.unwrap();
        let LayerKind::Raster(img) = &mut d.layer_mut(id).unwrap().kind;
        img.set_pixel(x, y, rgba);
    }

    #[test]
    fn locked_layer_refuses() {
        let mut d = doc();
        let id = d.active_layer.unwrap();
        d.layer_mut(id).unwrap().lock_pixels = true;
        assert_eq!(
            fill(&mut d, [0; 3], FillOptions::default()),
            Err(FillError::Locked)
        );
        assert_eq!(
            FillError::Locked.message("Fill"),
            "Could not complete the Fill command because the layer is locked."
        );
    }
}
