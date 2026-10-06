//! Layer menu operations: duplicating, Layer Via Copy/Cut, converting the
//! background, arranging, merging and flattening.

use crate::clipboard::{self, ClipError};
use crate::document::Document;
use crate::layer::{BlendMode, Layer, LayerId};
use crate::tile::TiledImage;

/// Layer > Arrange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrange {
    BringToFront,
    BringForward,
    SendBackward,
    SendToBack,
}

fn active_index(doc: &Document) -> Option<usize> {
    let id = doc.active_layer?;
    doc.layers.iter().position(|l| l.id == id)
}

/// "Name copy", then "Name copy 2", "Name copy 3"... like Photoshop.
fn copy_name(doc: &Document, name: &str) -> String {
    let base = format!("{name} copy");
    if !doc.layers.iter().any(|l| l.name == base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !doc.layers.iter().any(|l| &l.name == candidate))
        .expect("some name is free")
}

/// A copy of `source` with a new id and name. A copy of the background is
/// a regular layer.
fn copy_of(doc: &Document, source: &Layer, name: String) -> Layer {
    Layer {
        id: doc.new_layer_id(),
        name,
        is_background: false,
        ..source.clone()
    }
}

/// Layer > Duplicate Layer: an identical layer named "Name copy" directly
/// above the active one, which becomes active.
pub fn duplicate(doc: &mut Document) -> Option<LayerId> {
    let source = doc.layers[active_index(doc)?].clone();
    let layer = copy_of(doc, &source, copy_name(doc, &source.name));
    let id = layer.id;
    doc.insert_above_active(layer);
    Some(id)
}

/// Layer > New > Layer Via Copy (Cmd+J). Without a selection the whole layer
/// is duplicated (named "Name copy", or "Layer N" for the background); with
/// one, the selected pixels go to a new "Layer N" in place. Either way the
/// new layer keeps the source's opacity and blend mode.
pub fn via_copy(doc: &mut Document) -> Result<LayerId, ClipError> {
    let index = active_index(doc).ok_or(ClipError::NoLayer)?;
    let source = doc.layers[index].clone();
    if doc.selection().is_none() {
        let name = if source.is_background {
            doc.next_layer_name()
        } else {
            copy_name(doc, &source.name)
        };
        let layer = copy_of(doc, &source, name);
        let id = layer.id;
        doc.insert_above_active(layer);
        return Ok(id);
    }
    let clip = clipboard::copy(doc)?;
    Ok(paste_like(doc, &clip, &source))
}

/// Layer > New > Layer Via Cut (Shift+Cmd+J): moves the selected pixels to
/// a new "Layer N" in place; the background is left with `background`.
pub fn via_cut(doc: &mut Document, background: [u8; 3]) -> Result<LayerId, ClipError> {
    let index = active_index(doc).ok_or(ClipError::NoLayer)?;
    let source = doc.layers[index].clone();
    let clip = clipboard::cut(doc, background)?;
    Ok(paste_like(doc, &clip, &source))
}

/// Puts `clip` back at its origin on a new layer that takes `source`'s
/// opacity and blend mode, keeping the selection (unlike Paste).
fn paste_like(doc: &mut Document, clip: &clipboard::Clip, source: &Layer) -> LayerId {
    let selection = doc.selection().cloned();
    let id = clipboard::paste(doc, clip, clip.origin.unwrap_or((0, 0)));
    doc.set_selection(selection);
    let layer = doc.layer_mut(id).expect("just added");
    if !source.is_background {
        layer.opacity = source.opacity;
        layer.fill = source.fill;
        layer.blend_mode = source.blend_mode;
    }
    id
}

/// Layer > New > Layer from Background: the background becomes a regular,
/// unlocked "Layer 0" that can hold transparency and be moved.
pub fn layer_from_background(doc: &mut Document) -> bool {
    let Some(layer) = doc.layers.iter_mut().find(|l| l.is_background) else {
        return false;
    };
    layer.is_background = false;
    layer.name = "Layer 0".into();
    layer.lock_pixels = false;
    layer.lock_position = false;
    layer.lock_transparency = false;
    doc.mark_dirty();
    true
}

/// Where Layer > Arrange would move the active layer, if anywhere. Nothing
/// moves the background, and no layer goes below it.
pub fn arrange_target(doc: &Document, arrange: Arrange) -> Option<usize> {
    let index = active_index(doc)?;
    if doc.layers[index].is_background {
        return None;
    }
    let lowest = usize::from(doc.layers.first().is_some_and(|l| l.is_background));
    let top = doc.layers.len() - 1;
    let to = match arrange {
        Arrange::BringToFront => top,
        Arrange::BringForward => (index + 1).min(top),
        Arrange::SendBackward => index.saturating_sub(1).max(lowest),
        Arrange::SendToBack => lowest,
    };
    (to != index).then_some(to)
}

/// Layer > Arrange. Returns whether the layer moved.
pub fn arrange(doc: &mut Document, arrange: Arrange) -> bool {
    let (Some(from), Some(to)) = (active_index(doc), arrange_target(doc, arrange)) else {
        return false;
    };
    move_layer(doc, from, to)
}

/// Moves the layer at index `from` so it ends up at index `to` (both
/// bottom-up), as when dragging it in the Layers panel. The background
/// can't move and no layer goes below it. Returns whether anything moved.
pub fn move_layer(doc: &mut Document, from: usize, to: usize) -> bool {
    let len = doc.layers.len();
    if from == to || from >= len || to >= len || doc.layers[from].is_background {
        return false;
    }
    if to == 0 && doc.layers[0].is_background {
        return false;
    }
    let layer = doc.layers.remove(from);
    doc.layers.insert(to, layer);
    doc.mark_dirty();
    true
}

/// Layer > Rename Layer. Returns whether the name changed; empty names are
/// refused.
pub fn rename(doc: &mut Document, id: LayerId, name: &str) -> bool {
    let name = name.trim();
    let Some(layer) = doc.layer_mut(id) else {
        return false;
    };
    if name.is_empty() || layer.name == name {
        return false;
    }
    layer.name = name.into();
    doc.mark_dirty();
    true
}

/// The visible pixels of `layers` merged, as one image.
fn merged(doc: &Document, layers: &[Layer]) -> TiledImage {
    TiledImage::from_rgba8(doc.width, doc.height, &doc.composite_layers_rgba8(layers))
}

/// Whether Layer > Merge Down can run: the active layer and the one below
/// it are both visible.
pub fn can_merge_down(doc: &Document) -> bool {
    active_index(doc).is_some_and(|i| i > 0 && doc.layers[i].visible && doc.layers[i - 1].visible)
}

/// Layer > Merge Down (Cmd+E): merges the active layer into the one below,
/// which keeps its name and properties and becomes active.
pub fn merge_down(doc: &mut Document) -> bool {
    if !can_merge_down(doc) {
        return false;
    }
    let i = active_index(doc).expect("checked");
    let mut lower = doc.layers[i - 1].clone();
    // The upper layer is merged into the lower layer's own pixels
    lower.opacity = 1.0;
    lower.fill = 1.0;
    lower.blend_mode = BlendMode::Normal;
    let image = merged(doc, &[lower, doc.layers[i].clone()]);
    doc.layers.remove(i);
    let target = &mut doc.layers[i - 1];
    target.kind = crate::layer::LayerKind::Raster(image);
    doc.active_layer = Some(target.id);
    doc.mark_dirty();
    true
}

/// Whether Layer > Merge Visible has anything to merge.
pub fn can_merge_visible(doc: &Document) -> bool {
    doc.layers.iter().filter(|l| l.visible).count() > 1
}

/// Layer > Merge Visible (Shift+Cmd+E): merges every visible layer into one.
/// The result takes the background's place if it is visible, otherwise the
/// active layer's (or the top visible layer's when the active one is
/// hidden). Hidden layers stay as they are.
pub fn merge_visible(doc: &mut Document) -> bool {
    if !can_merge_visible(doc) {
        return false;
    }
    let active = active_index(doc).filter(|&i| doc.layers[i].visible);
    let target = doc
        .layers
        .iter()
        .position(|l| l.visible && l.is_background)
        .or(active)
        .or_else(|| doc.layers.iter().rposition(|l| l.visible))
        .expect("there are visible layers");
    let image = merged(doc, &doc.layers);
    let target_id = doc.layers[target].id;
    doc.layers.retain(|l| !l.visible || l.id == target_id);
    let layer = doc.layer_mut(target_id).expect("kept");
    layer.kind = crate::layer::LayerKind::Raster(image);
    layer.opacity = 1.0;
    layer.fill = 1.0;
    layer.blend_mode = BlendMode::Normal;
    doc.active_layer = Some(target_id);
    doc.mark_dirty();
    true
}

/// Layer > Flatten Image: the visible layers merged onto white become a
/// single "Background" layer; hidden layers are discarded.
pub fn flatten(doc: &mut Document) {
    let mut pixels = doc.composite_rgba8();
    for px in pixels.as_chunks_mut::<4>().0 {
        let a = px[3] as u32;
        for c in &mut px[..3] {
            *c = ((*c as u32 * a + 255 * (255 - a) + 127) / 255) as u8;
        }
        px[3] = 255;
    }
    let image = TiledImage::from_rgba8(doc.width, doc.height, &pixels);
    let mut layer = Layer::raster(doc.new_layer_id(), "Background", image);
    layer.is_background = true;
    doc.active_layer = Some(layer.id);
    doc.layers = vec![layer];
    doc.mark_dirty();
}

/// Layer > Delete > Hidden Layers. Returns whether any were deleted; the
/// last layer is never deleted.
pub fn delete_hidden(doc: &mut Document) -> bool {
    let before = doc.layers.len();
    if doc.layers.iter().all(|l| !l.visible) {
        return false;
    }
    doc.layers.retain(|l| l.visible);
    if doc.layers.len() == before {
        return false;
    }
    if active_index(doc).is_none() {
        doc.active_layer = doc.layers.last().map(|l| l.id);
    }
    doc.mark_dirty();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::layer::LayerKind;
    use crate::selection::{Rect, Selection};

    /// 4×4 white background plus "Layer 1" with a red pixel at (1, 1).
    fn doc() -> Document {
        let mut doc = Document::new_with_background("t", 4, 4, Color::WHITE);
        let mut image = TiledImage::new(4, 4);
        image.set_pixel(1, 1, [255, 0, 0, 255]);
        doc.insert_above_active(Layer::raster(doc.new_layer_id(), "Layer 1", image));
        doc
    }

    fn names(doc: &Document) -> Vec<&str> {
        doc.layers.iter().map(|l| l.name.as_str()).collect()
    }

    fn pixel(doc: &Document, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * doc.width + x) * 4) as usize;
        doc.composite_rgba8()[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn duplicates_are_named_like_photoshop() {
        let mut doc = doc();
        duplicate(&mut doc);
        doc.active_layer = Some(doc.layers[1].id);
        duplicate(&mut doc);
        assert_eq!(
            names(&doc),
            ["Background", "Layer 1", "Layer 1 copy 2", "Layer 1 copy"]
        );
        // Cmd+J on the background makes "Layer N", not a second background
        doc.active_layer = Some(doc.layers[0].id);
        via_copy(&mut doc).unwrap();
        assert_eq!(doc.layers[1].name, "Layer 2");
        assert!(!doc.layers[1].is_background);
    }

    #[test]
    fn via_copy_and_cut_move_the_selection_in_place() {
        let mut doc = doc();
        doc.layers[1].opacity = 0.5;
        let s = Selection::rect(4, 4, Rect::new(0.0, 0.0, 2.0, 2.0));
        doc.set_selection(Some(s));
        let id = via_copy(&mut doc).unwrap();
        assert_eq!(doc.active_layer, Some(id));
        assert_eq!(doc.layers[2].name, "Layer 2");
        assert_eq!(doc.layers[2].opacity, 0.5);
        assert!(doc.selection().is_some());

        doc.active_layer = Some(doc.layers[1].id);
        via_cut(&mut doc, [0, 0, 0]).unwrap();
        let LayerKind::Raster(image) = &doc.layers[1].kind;
        assert_eq!(image.pixel(1, 1)[3], 0);
        assert_eq!(doc.layers[2].name, "Layer 3");
    }

    #[test]
    fn arrange_keeps_the_background_at_the_bottom() {
        let mut doc = doc();
        duplicate(&mut doc);
        assert!(arrange(&mut doc, Arrange::SendToBack));
        assert_eq!(names(&doc), ["Background", "Layer 1 copy", "Layer 1"]);
        assert!(!arrange(&mut doc, Arrange::SendBackward));
        assert!(arrange(&mut doc, Arrange::BringToFront));
        assert_eq!(names(&doc), ["Background", "Layer 1", "Layer 1 copy"]);
        doc.active_layer = Some(doc.layers[0].id);
        assert!(!arrange(&mut doc, Arrange::BringForward));
    }

    #[test]
    fn dragging_and_renaming() {
        let mut doc = doc();
        duplicate(&mut doc);
        assert!(move_layer(&mut doc, 2, 1));
        assert_eq!(names(&doc), ["Background", "Layer 1 copy", "Layer 1"]);
        assert!(!move_layer(&mut doc, 1, 0));
        assert!(!move_layer(&mut doc, 0, 2));
        let id = doc.layers[2].id;
        assert!(rename(&mut doc, id, "  Sky "));
        assert_eq!(doc.layers[2].name, "Sky");
        assert!(!rename(&mut doc, id, ""));
        assert!(!rename(&mut doc, id, "Sky"));
    }

    #[test]
    fn merge_down_keeps_the_lower_layer() {
        let mut doc = doc();
        doc.layers[1].opacity = 0.5;
        assert!(merge_down(&mut doc));
        assert_eq!(names(&doc), ["Background"]);
        assert!(doc.layers[0].is_background);
        assert_eq!(pixel(&doc, 1, 1), [255, 128, 128, 255]);
        assert!(!can_merge_down(&doc));
    }

    #[test]
    fn merge_visible_leaves_hidden_layers() {
        let mut doc = doc();
        duplicate(&mut doc);
        doc.layers[2].visible = false;
        assert!(merge_visible(&mut doc));
        assert_eq!(names(&doc), ["Background", "Layer 1 copy"]);
        assert_eq!(pixel(&doc, 1, 1), [255, 0, 0, 255]);
        assert_eq!(doc.active_layer, Some(doc.layers[0].id));
    }

    #[test]
    fn flatten_fills_transparency_with_white() {
        let mut doc = doc();
        layer_from_background(&mut doc);
        assert_eq!(doc.layers[0].name, "Layer 0");
        doc.layers[0].visible = false;
        flatten(&mut doc);
        assert_eq!(names(&doc), ["Background"]);
        assert_eq!(pixel(&doc, 0, 0), [255, 255, 255, 255]);
        assert_eq!(pixel(&doc, 1, 1), [255, 0, 0, 255]);
    }

    #[test]
    fn delete_hidden_keeps_visible_layers() {
        let mut doc = doc();
        doc.layers[1].visible = false;
        assert!(delete_hidden(&mut doc));
        assert_eq!(names(&doc), ["Background"]);
        assert_eq!(doc.active_layer, Some(doc.layers[0].id));
        assert!(!delete_hidden(&mut doc));
    }
}
