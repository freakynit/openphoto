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
    /// Levels on the composite RGB channel: input black and white points,
    /// midtone gamma (0.01–9.99), output black and white points.
    Levels {
        input_black: u8,
        input_white: u8,
        gamma: f32,
        output_black: u8,
        output_white: u8,
    },
    /// Hue/Saturation on the master range: hue shift −180–180°, saturation
    /// and lightness −100–100.
    HueSaturation {
        hue: i32,
        saturation: i32,
        lightness: i32,
    },
    /// Exposure in stops (−20–20), offset (−0.5–0.5) and gamma correction
    /// (0.01–9.99), computed in linear light.
    Exposure {
        exposure: f32,
        offset: f32,
        gamma: f32,
    },
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
            Self::Levels { .. } => "Levels",
            Self::HueSaturation { .. } => "Hue/Saturation",
            Self::Exposure { .. } => "Exposure",
        }
    }

    /// Per-channel lookup table for adjustments that treat each channel the
    /// same way on its own.
    fn table(self) -> Option<[u8; 256]> {
        let f: Box<dyn Fn(f32) -> f32> = match self {
            Self::Levels {
                input_black,
                input_white,
                gamma,
                output_black,
                output_white,
            } => {
                let (ib, iw) = (
                    input_black as f32,
                    (input_white.max(input_black + 1)) as f32,
                );
                let (ob, ow) = (output_black as f32, output_white as f32);
                Box::new(move |v| {
                    let t = ((v - ib) / (iw - ib)).clamp(0.0, 1.0).powf(1.0 / gamma);
                    ob + t * (ow - ob)
                })
            }
            Self::Exposure {
                exposure,
                offset,
                gamma,
            } => Box::new(move |v| {
                let linear = srgb_to_linear(v / 255.0) * 2f32.powf(exposure) + offset;
                linear_to_srgb(linear.max(0.0).powf(1.0 / gamma)) * 255.0
            }),
            _ => return None,
        };
        let mut table = [0u8; 256];
        for (i, t) in table.iter_mut().enumerate() {
            *t = f(i as f32).round().clamp(0.0, 255.0) as u8;
        }
        Some(table)
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// RGB (0–1) to hue (0–360), saturation and lightness (0–1).
pub(crate) fn rgb_to_hsl([r, g, b]: [f32; 3]) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return [0.0, 0.0, l];
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    [h * 60.0, s, l]
}

pub(crate) fn hsl_to_rgb([h, s, l]: [f32; 3]) -> [f32; 3] {
    if s == 0.0 {
        return [l; 3];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let h = h / 360.0;
    [channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0)]
}

/// Hue/Saturation on one pixel: hue rotates and saturation scales in HSL;
/// lightness then mixes toward white (positive) or black (negative), as in
/// Photoshop.
fn hue_saturation(px: [u8; 4], hue: i32, saturation: i32, lightness: i32) -> [u8; 4] {
    let rgb = [px[0], px[1], px[2]].map(|v| v as f32 / 255.0);
    let [h, s, l] = rgb_to_hsl(rgb);
    let s = (s * (1.0 + saturation as f32 / 100.0)).clamp(0.0, 1.0);
    let mut out = hsl_to_rgb([h + hue as f32, s, l]);
    let k = lightness as f32 / 100.0;
    for c in &mut out {
        *c = if k >= 0.0 {
            *c + (1.0 - *c) * k
        } else {
            *c * (1.0 + k)
        };
    }
    let [r, g, b] = out.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
    [r, g, b, px[3]]
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
pub fn channel_histogram(doc: &Document) -> [u64; 256] {
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
        other => other.table(),
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
            Adjustment::Equalize | Adjustment::Levels { .. } | Adjustment::Exposure { .. } => {
                let t = table.as_ref().expect("computed above");
                [t[r as usize], t[g as usize], t[b as usize], a]
            }
            Adjustment::HueSaturation {
                hue,
                saturation,
                lightness,
            } => hue_saturation(px, hue, saturation, lightness),
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
    fn levels_hue_saturation_and_exposure() {
        let levels = Adjustment::Levels {
            input_black: 50,
            input_white: 200,
            gamma: 1.0,
            output_black: 0,
            output_white: 255,
        };
        let mut d = doc([125, 50, 220, 255]);
        apply(&mut d, levels).unwrap();
        assert_eq!(first(&d), [128, 0, 255, 255]);
        // Gamma 2 brightens the midtones: (128/255)^(1/2) * 255 = 180.7
        let mut d = doc([128, 128, 128, 255]);
        let gamma = Adjustment::Levels {
            input_black: 0,
            input_white: 255,
            gamma: 2.0,
            output_black: 0,
            output_white: 255,
        };
        apply(&mut d, gamma).unwrap();
        assert_eq!(first(&d), [181, 181, 181, 255]);

        // Red turned by 120° is green; saturation −100 is gray
        let mut d = doc([255, 0, 0, 255]);
        let hue = Adjustment::HueSaturation {
            hue: 120,
            saturation: 0,
            lightness: 0,
        };
        apply(&mut d, hue).unwrap();
        assert_eq!(first(&d), [0, 255, 0, 255]);
        let mut d = doc([255, 0, 0, 255]);
        let gray = Adjustment::HueSaturation {
            hue: 0,
            saturation: -100,
            lightness: 50,
        };
        apply(&mut d, gray).unwrap();
        assert_eq!(first(&d), [191, 191, 191, 255]);

        // One stop up doubles linear light: sRGB 128 (0.216) -> 0.432 -> 175.5
        let mut d = doc([128, 128, 128, 255]);
        let exposure = Adjustment::Exposure {
            exposure: 1.0,
            offset: 0.0,
            gamma: 1.0,
        };
        apply(&mut d, exposure).unwrap();
        assert_eq!(first(&d), [176, 176, 176, 255]);
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
