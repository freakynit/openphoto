//! Image > Adjustments applied to the active layer's pixels, limited to the
//! selection.

use crate::document::Document;
use crate::fill::FillError;
use crate::layer::LayerKind;

/// A pixel adjustment with its settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Adjustment {
    Invert,
    /// Gray at each pixel's HSL lightness, (max + min) / 2, like Photoshop.
    Desaturate,
    /// Pixels at least this bright (luminosity 0–255) become white, the
    /// rest black. Photoshop's range is 1–255.
    Threshold(u8),
    /// The number of tonal levels per channel, 2–255.
    Posterize(u8),
    /// Spreads the brightness values evenly (histogram equalization).
    Equalize,
}

impl Adjustment {
    /// The menu and history name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Invert => "Invert",
            Self::Desaturate => "Desaturate",
            Self::Threshold(_) => "Threshold",
            Self::Posterize(_) => "Posterize",
            Self::Equalize => "Equalize",
        }
    }
}

/// Luminosity on Photoshop's 0–255 scale (Rec. 601 weights).
pub fn luminosity([r, g, b, _]: [u8; 4]) -> u8 {
    ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114 + 500) / 1000) as u8
}

fn posterize(v: u8, levels: u8) -> u8 {
    let steps = (levels.max(2) - 1) as f32;
    let level = (v as f32 * steps / 255.0).round();
    (level * 255.0 / steps).round() as u8
}

/// Histogram of the brightness values (all three channels together) of the
/// active layer's selected, non-transparent pixels.
fn channel_histogram(doc: &Document) -> [u64; 256] {
    let mut hist = [0u64; 256];
    let Some(layer) = doc.active_layer.and_then(|id| doc.layer(id)) else {
        return hist;
    };
    let LayerKind::Raster(image) = &layer.kind;
    let selection = doc.selection();
    for y in 0..doc.height {
        for x in 0..doc.width {
            if selection.is_some_and(|s| s.get(x, y) == 0) {
                continue;
            }
            let px = image.pixel(x, y);
            if px[3] == 0 {
                continue;
            }
            for &c in &px[..3] {
                hist[c as usize] += 1;
            }
        }
    }
    hist
}

/// Equalize's lookup table: each value maps to its place in the cumulative
/// histogram.
fn equalize_table(hist: &[u64; 256]) -> [u8; 256] {
    let total: u64 = hist.iter().sum();
    let mut table = [0u8; 256];
    if total == 0 {
        for (i, t) in table.iter_mut().enumerate() {
            *t = i as u8;
        }
        return table;
    }
    let mut cumulative = 0u64;
    for (i, &count) in hist.iter().enumerate() {
        cumulative += count;
        table[i] = ((cumulative * 255 + total / 2) / total) as u8;
    }
    table
}

/// Histogram of the active layer's luminosity inside the selection, as the
/// Threshold dialog shows it.
pub fn luminosity_histogram(doc: &Document) -> [u64; 256] {
    let mut hist = [0u64; 256];
    let Some(layer) = doc.active_layer.and_then(|id| doc.layer(id)) else {
        return hist;
    };
    let LayerKind::Raster(image) = &layer.kind;
    let selection = doc.selection();
    for y in 0..doc.height {
        for x in 0..doc.width {
            if selection.is_some_and(|s| s.get(x, y) == 0) {
                continue;
            }
            let px = image.pixel(x, y);
            if px[3] > 0 {
                hist[luminosity(px) as usize] += 1;
            }
        }
    }
    hist
}

/// The checks every adjustment makes before changing pixels; the messages
/// match Photoshop's ("Could not complete the Invert command because the
/// target layer is hidden.").
pub fn check(doc: &Document) -> Result<(), FillError> {
    let layer = doc
        .active_layer
        .and_then(|id| doc.layer(id))
        .ok_or(FillError::NoLayer)?;
    if !layer.visible {
        return Err(FillError::Hidden);
    }
    if layer.lock_pixels {
        return Err(FillError::Locked);
    }
    Ok(())
}

/// Applies `adjustment` to the active layer. Partly selected pixels get a
/// proportional mix of the old and new color; alpha is never changed.
pub fn apply(doc: &mut Document, adjustment: Adjustment) -> Result<(), FillError> {
    check(doc)?;
    let table = match adjustment {
        Adjustment::Equalize => Some(equalize_table(&channel_histogram(doc))),
        _ => None,
    };
    let map = |px: [u8; 4]| -> [u8; 4] {
        let [r, g, b, a] = px;
        match adjustment {
            Adjustment::Invert => [255 - r, 255 - g, 255 - b, a],
            Adjustment::Desaturate => {
                let l = (r.max(g).max(b) as u16 + r.min(g).min(b) as u16).div_ceil(2) as u8;
                [l, l, l, a]
            }
            Adjustment::Threshold(level) => {
                let v = if luminosity(px) >= level { 255 } else { 0 };
                [v, v, v, a]
            }
            Adjustment::Posterize(levels) => [
                posterize(r, levels),
                posterize(g, levels),
                posterize(b, levels),
                a,
            ],
            Adjustment::Equalize => {
                let t = table.as_ref().expect("computed above");
                [t[r as usize], t[g as usize], t[b as usize], a]
            }
        }
    };
    let selection = doc.selection().cloned();
    let (w, h) = (doc.width, doc.height);
    let id = doc.active_layer.expect("checked");
    let LayerKind::Raster(image) = &mut doc.layer_mut(id).expect("checked").kind;
    for y in 0..h {
        for x in 0..w {
            let amount = selection.as_ref().map_or(255, |s| s.get(x, y));
            if amount == 0 {
                continue;
            }
            let old = image.pixel(x, y);
            if old[3] == 0 {
                continue;
            }
            let mut new = map(old);
            if amount < 255 {
                for c in 0..3 {
                    let (o, n) = (old[c] as u32, new[c] as u32);
                    new[c] = ((o * (255 - amount as u32) + n * amount as u32 + 127) / 255) as u8;
                }
            }
            if new != old {
                image.set_pixel(x, y, new);
            }
        }
    }
    doc.mark_dirty();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::selection::{Rect, Selection};

    fn doc(px: [u8; 4]) -> Document {
        let mut doc = Document::new_with_background("t", 2, 1, Color::WHITE);
        let id = doc.active_layer.unwrap();
        let LayerKind::Raster(image) = &mut doc.layer_mut(id).unwrap().kind;
        image.set_pixel(0, 0, px);
        doc
    }

    fn first(doc: &Document) -> [u8; 4] {
        doc.composite_rgba8()[..4].try_into().unwrap()
    }

    #[test]
    fn invert_desaturate_threshold_posterize() {
        let mut d = doc([200, 100, 0, 255]);
        apply(&mut d, Adjustment::Invert).unwrap();
        assert_eq!(first(&d), [55, 155, 255, 255]);

        let mut d = doc([200, 100, 0, 255]);
        apply(&mut d, Adjustment::Desaturate).unwrap();
        assert_eq!(first(&d), [100, 100, 100, 255]);

        // Luminosity of (200, 100, 0) is 118.5 -> 119
        let mut d = doc([200, 100, 0, 255]);
        apply(&mut d, Adjustment::Threshold(119)).unwrap();
        assert_eq!(first(&d), [255, 255, 255, 255]);
        let mut d = doc([200, 100, 0, 255]);
        apply(&mut d, Adjustment::Threshold(120)).unwrap();
        assert_eq!(first(&d), [0, 0, 0, 255]);

        let mut d = doc([200, 100, 30, 255]);
        apply(&mut d, Adjustment::Posterize(2)).unwrap();
        assert_eq!(first(&d), [255, 0, 0, 255]);
        let mut d = doc([200, 100, 30, 255]);
        apply(&mut d, Adjustment::Posterize(4)).unwrap();
        assert_eq!(first(&d), [170, 85, 0, 255]);
    }

    #[test]
    fn equalize_stretches_the_range() {
        // Values 100 and 200 only: equalized to the middle and the top
        let mut d = doc([100, 100, 100, 255]);
        let id = d.active_layer.unwrap();
        let LayerKind::Raster(image) = &mut d.layer_mut(id).unwrap().kind;
        image.set_pixel(1, 0, [200, 200, 200, 255]);
        apply(&mut d, Adjustment::Equalize).unwrap();
        assert_eq!(first(&d), [128, 128, 128, 255]);
        assert_eq!(&d.composite_rgba8()[4..8], [255, 255, 255, 255]);
    }

    #[test]
    fn only_the_selection_changes() {
        let mut d = doc([0, 0, 0, 255]);
        d.set_selection(Some(Selection::rect(2, 1, Rect::new(1.0, 0.0, 2.0, 1.0))));
        apply(&mut d, Adjustment::Invert).unwrap();
        assert_eq!(first(&d), [0, 0, 0, 255]);
        assert_eq!(&d.composite_rgba8()[4..8], [0, 0, 0, 255]);
    }

    #[test]
    fn hidden_layers_are_refused() {
        let mut d = doc([0, 0, 0, 255]);
        d.layers[0].visible = false;
        let e = apply(&mut d, Adjustment::Invert).unwrap_err();
        assert_eq!(
            e.message("Invert"),
            "Could not complete the Invert command because the target layer is hidden."
        );
    }
}
