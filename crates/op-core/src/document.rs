use std::sync::atomic::{AtomicU64, Ordering};

use crate::blend;
use crate::color::Color;
use crate::layer::{Layer, LayerId, LayerKind};
use crate::pixel::{BitDepth, ColorMode};
use crate::tile::{TILE_SIZE, TiledImage};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DocId(pub u64);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Where the existing image is pinned when the canvas size changes
/// (the 3×3 grid in the Canvas Size dialog).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    /// 0 = left, 1 = center, 2 = right.
    pub x: u8,
    /// 0 = top, 1 = middle, 2 = bottom.
    pub y: u8,
}

impl Anchor {
    pub const CENTER: Self = Self { x: 1, y: 1 };

    /// Offset of the old image inside the new canvas along one axis.
    fn offset(pos: u8, old: u32, new: u32) -> i64 {
        let diff = new as i64 - old as i64;
        match pos {
            0 => 0,
            // Odd differences put the extra pixel on the right/bottom
            1 => diff.div_euclid(2),
            _ => diff,
        }
    }
}

/// The undoable part of a document. Cheap to clone because tiles are shared.
#[derive(Clone)]
pub struct Snapshot {
    width: u32,
    height: u32,
    resolution: f32,
    layers: Vec<Layer>,
    active_layer: Option<LayerId>,
}

pub struct Document {
    pub id: DocId,
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Resolution in pixels per inch.
    pub resolution: f32,
    pub color_mode: ColorMode,
    pub bit_depth: BitDepth,
    /// Ordered bottom to top.
    pub layers: Vec<Layer>,
    pub active_layer: Option<LayerId>,
    /// Bumped on every pixel or layer property change; the renderer uses it to
    /// decide whether to re-upload.
    revision: u64,
}

impl Document {
    fn empty(title: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            id: DocId(next_id()),
            title: title.into(),
            width,
            height,
            resolution: 72.0,
            color_mode: ColorMode::Rgb,
            bit_depth: BitDepth::U8,
            layers: Vec::new(),
            active_layer: None,
            revision: 0,
        }
    }

    /// File > New: a single background layer filled with `background`.
    pub fn new_with_background(
        title: impl Into<String>,
        width: u32,
        height: u32,
        background: Color,
    ) -> Self {
        let mut doc = Self::empty(title, width, height);
        let image = TiledImage::filled(width, height, background.to_rgba8());
        let mut layer = Layer::raster(doc.new_layer_id(), "Background", image);
        layer.is_background = true;
        doc.active_layer = Some(layer.id);
        doc.layers.push(layer);
        doc
    }

    /// Opens a flat bitmap file as a single layer, like Photoshop: an opaque
    /// image becomes the locked "Background" layer, while an image with any
    /// transparency becomes a regular "Layer 0".
    pub fn from_rgba8(title: impl Into<String>, width: u32, height: u32, pixels: &[u8]) -> Self {
        let mut doc = Self::empty(title, width, height);
        let opaque = pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 255);
        let image = TiledImage::from_rgba8(width, height, pixels);
        let name = if opaque { "Background" } else { "Layer 0" };
        let mut layer = Layer::raster(doc.new_layer_id(), name, image);
        layer.is_background = opaque;
        doc.active_layer = Some(layer.id);
        doc.layers.push(layer);
        doc
    }

    pub fn new_layer_id(&self) -> LayerId {
        LayerId(next_id())
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn mark_dirty(&mut self) {
        self.revision += 1;
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            width: self.width,
            height: self.height,
            resolution: self.resolution,
            layers: self.layers.clone(),
            active_layer: self.active_layer,
        }
    }

    pub fn restore(&mut self, snapshot: &Snapshot) {
        let s = snapshot.clone();
        self.width = s.width;
        self.height = s.height;
        self.resolution = s.resolution;
        self.layers = s.layers;
        self.active_layer = s.active_layer;
        self.mark_dirty();
    }

    /// Image > Canvas Size. The background layer is extended with `fill`;
    /// other layers are extended with transparency.
    pub fn resize_canvas(&mut self, width: u32, height: u32, anchor: Anchor, fill: Color) {
        let dx = Anchor::offset(anchor.x, self.width, width);
        let dy = Anchor::offset(anchor.y, self.height, height);
        let fill = fill.to_rgba8();
        for layer in &mut self.layers {
            let LayerKind::Raster(image) = &mut layer.kind;
            let extension = if layer.is_background {
                [fill[0], fill[1], fill[2], 255]
            } else {
                [0; 4]
            };
            *image = image.with_canvas(width, height, dx, dy, extension);
        }
        self.width = width;
        self.height = height;
        self.mark_dirty();
    }

    /// Whether the document has a background layer (which decides whether the
    /// canvas extension color applies).
    pub fn has_background(&self) -> bool {
        self.layers.iter().any(|l| l.is_background)
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Composites all visible layers on the CPU into tightly packed straight RGBA8.
    ///
    /// Blending happens in gamma-encoded space, which is Photoshop's default,
    /// with each layer's blend mode (see [`crate::blend`]).
    pub fn composite_rgba8(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0f32; w * h * 4];

        for layer in self.layers.iter().filter(|l| l.visible) {
            let LayerKind::Raster(image) = &layer.kind;
            let layer_alpha = layer.opacity * layer.fill;
            if layer_alpha <= 0.0 {
                continue;
            }

            for ty in 0..image.tiles_y() {
                for tx in 0..image.tiles_x() {
                    let Some(tile) = image.tile(tx, ty) else {
                        continue;
                    };
                    let (x0, y0) = ((tx * TILE_SIZE) as usize, (ty * TILE_SIZE) as usize);
                    let tw = (TILE_SIZE as usize).min(w - x0);
                    let th = (TILE_SIZE as usize).min(h - y0);
                    for row in 0..th {
                        let src_row = &tile.data[row * TILE_SIZE as usize * 4..];
                        let dst_row = &mut out[((y0 + row) * w + x0) * 4..];
                        for col in 0..tw {
                            let s = &src_row[col * 4..col * 4 + 4];
                            let sa = s[3] as f32 / 255.0 * layer_alpha;
                            if sa <= 0.0 {
                                continue;
                            }
                            let d = &mut dst_row[col * 4..col * 4 + 4];
                            let src = [s[0], s[1], s[2]].map(|v| v as f32 / 255.0);
                            let (x, y) = ((x0 + col) as u32, (y0 + row) as u32);
                            let out = blend::composite(
                                layer.blend_mode,
                                [d[0], d[1], d[2], d[3]],
                                src,
                                sa,
                                x,
                                y,
                            );
                            d.copy_from_slice(&out);
                        }
                    }
                }
            }
        }

        out.iter()
            .map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_bitmap_opens_as_background() {
        let doc = Document::from_rgba8("t", 2, 1, &[1, 2, 3, 255, 4, 5, 6, 255]);
        assert_eq!(doc.layers.len(), 1);
        assert_eq!(doc.layers[0].name, "Background");
        assert!(doc.layers[0].is_background);
        assert!(doc.has_background());
    }

    #[test]
    fn transparent_bitmap_opens_as_regular_layer() {
        let doc = Document::from_rgba8("t", 2, 1, &[1, 2, 3, 255, 4, 5, 6, 128]);
        assert_eq!(doc.layers[0].name, "Layer 0");
        assert!(!doc.layers[0].is_background);
        assert!(!doc.has_background());
        assert_eq!(doc.active_layer, Some(doc.layers[0].id));
    }

    #[test]
    fn resize_canvas_centered() {
        let mut doc = Document::new_with_background("t", 2, 2, Color::WHITE);
        doc.resize_canvas(4, 5, Anchor::CENTER, Color::BLACK);
        assert_eq!((doc.width, doc.height), (4, 5));
        let px = doc.composite_rgba8();
        let at = |x: usize, y: usize| &px[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(at(0, 0), &[0, 0, 0, 255]);
        assert_eq!(at(1, 1), &[255, 255, 255, 255]);
        assert_eq!(at(2, 2), &[255, 255, 255, 255]);
        assert_eq!(at(2, 3), &[0, 0, 0, 255]);
    }

    #[test]
    fn resize_canvas_keeps_layers_transparent() {
        let mut doc = Document::new_with_background("t", 2, 2, Color::WHITE);
        let id = doc.new_layer_id();
        doc.layers.push(Layer::raster(
            id,
            "Layer 1",
            TiledImage::filled(2, 2, [255, 0, 0, 255]),
        ));
        doc.resize_canvas(3, 3, Anchor { x: 0, y: 0 }, Color::BLACK);
        let LayerKind::Raster(img) = &doc.layer(id).unwrap().kind;
        assert_eq!(img.pixel(2, 2), [0; 4]);
        assert_eq!(img.pixel(1, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn composite_uses_blend_mode() {
        let mut doc =
            Document::new_with_background("t", 1, 1, Color::from_rgba8([128, 128, 128, 255]));
        let id = doc.new_layer_id();
        let mut layer = Layer::raster(
            id,
            "Layer 1",
            TiledImage::filled(1, 1, [128, 128, 128, 255]),
        );
        layer.blend_mode = crate::BlendMode::Multiply;
        doc.layers.push(layer);
        assert_eq!(&doc.composite_rgba8()[..4], &[64, 64, 64, 255]);
    }

    #[test]
    fn composite_half_opacity_over_white() {
        let mut doc = Document::new_with_background("t", 4, 4, Color::WHITE);
        let id = doc.new_layer_id();
        let mut layer = Layer::raster(id, "Layer 1", TiledImage::filled(4, 4, [0, 0, 0, 255]));
        layer.opacity = 0.5;
        doc.layers.push(layer);
        let px = doc.composite_rgba8();
        assert_eq!(&px[0..4], &[128, 128, 128, 255]);
    }
}
