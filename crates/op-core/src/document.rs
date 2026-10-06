use std::sync::atomic::{AtomicU64, Ordering};

use crate::blend;
use crate::color::Color;
use crate::layer::{Layer, LayerId, LayerKind};
use crate::pixel::{BitDepth, ColorMode};
use crate::selection::Selection;
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

/// A ruler guide: a horizontal or vertical line at `position` document
/// pixels (it may lie outside the canvas).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub position: f32,
}

/// The undoable part of a document. Cheap to clone because tiles are shared.
#[derive(Clone)]
pub struct Snapshot {
    guides: Vec<Guide>,
    width: u32,
    height: u32,
    resolution: f32,
    layers: Vec<Layer>,
    active_layer: Option<LayerId>,
    selection: Option<Selection>,
    last_selection: Option<Selection>,
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
    /// The current selection; `None` means nothing is selected (Photoshop then
    /// treats the whole document as the target of edits).
    selection: Option<Selection>,
    /// The selection before the last Deselect, for Select > Reselect.
    last_selection: Option<Selection>,
    /// Bumped when the selection changes, so its outline can be cached.
    selection_revision: u64,
    /// Ruler guides; undoable like edits, as in Photoshop.
    pub guides: Vec<Guide>,
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
            selection: None,
            last_selection: None,
            selection_revision: 0,
            guides: Vec::new(),
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

    /// Name for a new layer, like Photoshop: one more than the highest
    /// existing "Layer N".
    pub fn next_layer_name(&self) -> String {
        let n = self
            .layers
            .iter()
            .filter_map(|l| l.name.strip_prefix("Layer ")?.parse::<u32>().ok())
            .max()
            .map_or(1, |max| max + 1);
        format!("Layer {n}")
    }

    /// Adds `layer` directly above the active layer (on top without one)
    /// and makes it active.
    pub fn insert_above_active(&mut self, layer: Layer) {
        let index = self
            .active_layer
            .and_then(|a| self.layers.iter().position(|l| l.id == a))
            .map_or(self.layers.len(), |i| i + 1);
        self.active_layer = Some(layer.id);
        self.layers.insert(index, layer);
        self.mark_dirty();
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn mark_dirty(&mut self) {
        self.revision += 1;
    }

    /// A layer as it is in `snapshot` (the History Brush paints from it).
    pub fn snapshot_layer(snapshot: &Snapshot, id: LayerId) -> Option<&Layer> {
        snapshot.layers.iter().find(|l| l.id == id)
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            guides: self.guides.clone(),
            width: self.width,
            height: self.height,
            resolution: self.resolution,
            layers: self.layers.clone(),
            active_layer: self.active_layer,
            selection: self.selection.clone(),
            last_selection: self.last_selection.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &Snapshot) {
        let s = snapshot.clone();
        self.width = s.width;
        self.height = s.height;
        self.resolution = s.resolution;
        self.layers = s.layers;
        self.active_layer = s.active_layer;
        self.selection = s.selection;
        self.last_selection = s.last_selection;
        self.guides = s.guides;
        self.selection_revision += 1;
        self.mark_dirty();
    }

    /// Moves each guide through `f` (document pixels, for the given
    /// orientation); used when the canvas is cropped, extended, resized,
    /// rotated or flipped. A rotation by 90° turns guides around, so `f`
    /// also returns the new orientation.
    pub fn map_guides(&mut self, f: impl Fn(Guide) -> Guide) {
        self.guides = self.guides.iter().map(|&g| f(g)).collect();
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
        for sel in [&mut self.selection, &mut self.last_selection]
            .into_iter()
            .flatten()
        {
            *sel = sel.with_canvas(width, height, dx, dy);
        }
        self.map_guides(|g| Guide {
            position: g.position + if g.vertical { dx } else { dy } as f32,
            ..g
        });
        self.selection_revision += 1;
        self.width = width;
        self.height = height;
        self.mark_dirty();
    }

    /// Replaces the canvas with a `width`×`height` one: every layer's image
    /// and both selections (current and the one Reselect brings back) are
    /// passed through `image` and `selection`. Used by Crop, Trim, Image
    /// Rotation and the canvas flips.
    pub fn transform_canvas(
        &mut self,
        width: u32,
        height: u32,
        image: impl Fn(&TiledImage) -> TiledImage,
        selection: impl Fn(&Selection) -> Selection,
    ) {
        for layer in &mut self.layers {
            let LayerKind::Raster(img) = &mut layer.kind;
            *img = image(img);
        }
        for sel in [&mut self.selection, &mut self.last_selection]
            .into_iter()
            .flatten()
        {
            *sel = selection(sel);
        }
        self.selection_revision += 1;
        self.width = width;
        self.height = height;
        self.mark_dirty();
    }

    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    pub fn selection_revision(&self) -> u64 {
        self.selection_revision
    }

    /// Replaces the selection. An empty selection counts as no selection.
    /// Clearing a selection remembers it for [`Self::reselect`].
    pub fn set_selection(&mut self, selection: Option<Selection>) {
        let selection = selection.filter(|s| !s.is_empty());
        if selection.is_none() && self.selection.is_some() {
            self.last_selection = self.selection.take();
        }
        self.selection = selection;
        self.selection_revision += 1;
    }

    /// Select > Reselect: restores the selection cleared by the last Deselect.
    pub fn reselect(&mut self) -> bool {
        match self.last_selection.take() {
            Some(s) => {
                self.selection = Some(s);
                self.selection_revision += 1;
                true
            }
            None => false,
        }
    }

    pub fn can_reselect(&self) -> bool {
        self.selection.is_none() && self.last_selection.is_some()
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
        self.composite_layers_rgba8(&self.layers)
    }

    /// Like [`Self::composite_rgba8`] for a given list of layers (bottom to
    /// top, each the size of the document), e.g. the layers a merge combines.
    pub fn composite_layers_rgba8(&self, layers: &[Layer]) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0f32; w * h * 4];

        for layer in layers.iter().filter(|l| l.visible) {
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
    fn selection_deselect_reselect_and_undo() {
        use crate::selection::Rect;
        let mut doc = Document::new_with_background("t", 10, 10, Color::WHITE);
        let snapshot = doc.snapshot();
        doc.set_selection(Some(Selection::rect(10, 10, Rect::new(1.0, 1.0, 4.0, 4.0))));
        assert!(doc.selection().is_some());
        doc.set_selection(None);
        assert!(doc.can_reselect());
        assert!(doc.reselect());
        assert_eq!(doc.selection().unwrap().bounds(), Some((1, 1, 4, 4)));
        // Restoring an earlier snapshot restores its (empty) selection
        doc.restore(&snapshot);
        assert!(doc.selection().is_none());
        // An empty selection counts as none
        doc.set_selection(Some(Selection::rect(10, 10, Rect::new(3.0, 3.0, 3.0, 3.0))));
        assert!(doc.selection().is_none());
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
