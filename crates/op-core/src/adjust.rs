//! Image > Adjustments applied to the active layer's pixels, limited to the
//! selection.

use crate::document::Document;
use crate::fill::FillError;

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
    /// Brightness −150–150 and contrast −50–100 (the non-legacy behavior,
    /// approximated without clipping: brightness bends the tones with a
    /// gamma, contrast with an S-curve).
    BrightnessContrast {
        brightness: i32,
        contrast: i32,
    },
    /// Color Balance on the midtones: cyan–red, magenta–green and
    /// yellow–blue shifts (−100–100); Preserve Luminosity keeps each pixel's
    /// luminosity.
    ColorBalance {
        midtones: [i32; 3],
        preserve_luminosity: bool,
    },
    /// Black & White: how bright reds, yellows, greens, cyans, blues and
    /// magentas turn (percent, −200–300).
    BlackWhite {
        weights: [i32; 6],
    },
    /// Vibrance (boosts muted colors most) and Saturation, −100–100 each.
    Vibrance {
        vibrance: i32,
        saturation: i32,
    },
    /// Photo Filter: a color multiplied in at `density` percent.
    PhotoFilter {
        color: [u8; 3],
        density: u8,
        preserve_luminosity: bool,
    },
    /// Gradient Map: luminosity mapped from `from` (shadows) to `to`.
    GradientMap {
        from: [u8; 3],
        to: [u8; 3],
    },
    /// Image > Auto Tone: each channel stretched to the full range,
    /// ignoring the darkest and lightest 0.1%.
    AutoTone,
    /// Image > Auto Contrast: all channels stretched together, so colors
    /// keep their balance.
    AutoContrast,
    /// Curves on the composite RGB channel: up to 16 (input, output)
    /// points, the first `count` of `points` used, sorted by input.
    Curves {
        points: [(u8, u8); 16],
        count: u8,
    },
    /// Image > Auto Color: here each channel stretched like Auto Tone
    /// (Photoshop also neutralizes the midtones).
    AutoColor,
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
            Self::BrightnessContrast { .. } => "Brightness/Contrast",
            Self::ColorBalance { .. } => "Color Balance",
            Self::BlackWhite { .. } => "Black & White",
            Self::Vibrance { .. } => "Vibrance",
            Self::PhotoFilter { .. } => "Photo Filter",
            Self::GradientMap { .. } => "Gradient Map",
            Self::AutoTone => "Auto Tone",
            Self::AutoContrast => "Auto Contrast",
            Self::AutoColor => "Auto Color",
            Self::Curves { .. } => "Curves",
        }
    }

    /// Curves from a list of (input, output) points.
    pub fn curves(points: &[(u8, u8)]) -> Self {
        let mut array = [(0u8, 0u8); 16];
        let count = points.len().min(16);
        array[..count].copy_from_slice(&points[..count]);
        Self::Curves {
            points: array,
            count: count as u8,
        }
    }

    /// Per-channel lookup table for adjustments that treat each channel the
    /// same way on its own.
    fn table(self) -> Option<[u8; 256]> {
        if let Self::Curves { points, count } = self {
            return Some(curve_table(&points[..count as usize]));
        }
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
            Self::BrightnessContrast {
                brightness,
                contrast,
            } => {
                let g = 2f32.powf(-brightness as f32 / 100.0);
                let k = contrast as f32 / 100.0;
                Box::new(move |v| {
                    let mut t = (v / 255.0).powf(g);
                    t = if k >= 0.0 {
                        // Toward a smooth S-curve: darker darks, lighter lights
                        let s = t * t * (3.0 - 2.0 * t);
                        t + (s - t) * k
                    } else {
                        0.5 + (t - 0.5) * (1.0 + k)
                    };
                    t * 255.0
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

/// The curve through `points` (sorted by input) as a 256-entry table: a
/// natural cubic spline, flat beyond the first and last points, clamped to
/// 0–255.
pub fn curve_table(points: &[(u8, u8)]) -> [u8; 256] {
    let mut pts: Vec<(f32, f32)> = points.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts.dedup_by(|a, b| a.0 == b.0);
    let mut table = [0u8; 256];
    match pts.len() {
        0 => {
            for (i, t) in table.iter_mut().enumerate() {
                *t = i as u8;
            }
            return table;
        }
        1 => {
            table.fill(pts[0].1.round() as u8);
            return table;
        }
        _ => {}
    }
    // Second derivatives of the natural spline (tridiagonal solve)
    let n = pts.len();
    let mut m = vec![0f32; n];
    if n > 2 {
        let h: Vec<f32> = (0..n - 1).map(|i| pts[i + 1].0 - pts[i].0).collect();
        let mut a = vec![0f32; n];
        let mut b = vec![0f32; n];
        let mut c = vec![0f32; n];
        let mut d = vec![0f32; n];
        for i in 1..n - 1 {
            a[i] = h[i - 1];
            b[i] = 2.0 * (h[i - 1] + h[i]);
            c[i] = h[i];
            d[i] = 6.0 * ((pts[i + 1].1 - pts[i].1) / h[i] - (pts[i].1 - pts[i - 1].1) / h[i - 1]);
        }
        // Thomas algorithm on rows 1..n-1 (m[0] = m[n-1] = 0)
        for i in 2..n - 1 {
            let w = a[i] / b[i - 1];
            b[i] -= w * c[i - 1];
            d[i] -= w * d[i - 1];
        }
        for i in (1..n - 1).rev() {
            let next = if i + 1 < n - 1 { m[i + 1] } else { 0.0 };
            m[i] = (d[i] - c[i] * next) / b[i];
        }
    }
    for (x, t) in table.iter_mut().enumerate() {
        let x = x as f32;
        let y = if x <= pts[0].0 {
            pts[0].1
        } else if x >= pts[n - 1].0 {
            pts[n - 1].1
        } else {
            let i = (0..n - 1).find(|&i| x <= pts[i + 1].0).unwrap_or(n - 2);
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[i + 1];
            let h = x1 - x0;
            let (s, u) = ((x1 - x) / h, (x - x0) / h);
            s * y0 + u * y1 + ((s * s * s - s) * m[i] + (u * u * u - u) * m[i + 1]) * h * h / 6.0
        };
        *t = y.round().clamp(0.0, 255.0) as u8;
    }
    table
}

/// Moves every channel by the same amount so the pixel's luminosity is
/// `target` again.
fn keep_luminosity(rgb: [f32; 3], target: f32) -> [f32; 3] {
    let l = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
    rgb.map(|v| v + (target - l))
}

fn to_u8(rgb: [f32; 3], a: u8) -> [u8; 4] {
    let [r, g, b] = rgb.map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
    [r, g, b, a]
}

/// The per-pixel adjustments that mix channels.
fn color_adjust(adjustment: Adjustment, px: [u8; 4]) -> [u8; 4] {
    let rgb = [px[0], px[1], px[2]].map(|v| v as f32 / 255.0);
    let lum = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
    match adjustment {
        Adjustment::ColorBalance {
            midtones,
            preserve_luminosity,
        } => {
            // Each channel moves most in the midtones
            let mut out = [0, 1, 2].map(|c| {
                let v = rgb[c];
                v + midtones[c] as f32 / 100.0 * 0.4 * 4.0 * v * (1.0 - v)
            });
            if preserve_luminosity {
                out = keep_luminosity(out, lum);
            }
            to_u8(out, px[3])
        }
        Adjustment::BlackWhite { weights } => {
            // The gray is the darkest channel plus the primary and the
            // secondary hue's shares, each weighted
            let (r, g, b) = (rgb[0], rgb[1], rgb[2]);
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let mid = r + g + b - max - min;
            let w = |i: usize| weights[i] as f32 / 100.0;
            let primary = if max == r {
                w(0)
            } else if max == g {
                w(2)
            } else {
                w(4)
            };
            // Yellow (red+green), cyan (green+blue) or magenta (red+blue)
            let secondary = if min == b {
                w(1)
            } else if min == r {
                w(3)
            } else {
                w(5)
            };
            let gray = min + (mid - min) * secondary + (max - mid) * primary;
            to_u8([gray; 3], px[3])
        }
        Adjustment::Vibrance {
            vibrance,
            saturation,
        } => {
            let [h, s, l] = rgb_to_hsl(rgb);
            let v = vibrance as f32 / 100.0;
            // Vibrance acts most on muted colors
            let s = (s * (1.0 + v * (1.0 - s))).clamp(0.0, 1.0);
            let s = (s * (1.0 + saturation as f32 / 100.0)).clamp(0.0, 1.0);
            to_u8(hsl_to_rgb([h, s, l]), px[3])
        }
        Adjustment::PhotoFilter {
            color,
            density,
            preserve_luminosity,
        } => {
            let d = density as f32 / 100.0;
            let mut out = [0, 1, 2].map(|c| {
                let v = rgb[c];
                v + (v * color[c] as f32 / 255.0 - v) * d
            });
            if preserve_luminosity {
                out = keep_luminosity(out, lum);
            }
            to_u8(out, px[3])
        }
        Adjustment::GradientMap { from, to } => {
            let out =
                [0, 1, 2].map(|c| (from[c] as f32 + (to[c] as f32 - from[c] as f32) * lum) / 255.0);
            to_u8(out, px[3])
        }
        _ => px,
    }
}

/// Auto Tone / Auto Contrast lookup tables: a channel's range without its
/// darkest and lightest 0.1% stretched to 0–255.
fn auto_tables(doc: &Document, per_channel: bool) -> [[u8; 256]; 3] {
    let mut hists = [[0u64; 256]; 3];
    if let Some(image) = doc
        .active_layer
        .and_then(|id| doc.layer(id))
        .and_then(|l| l.image())
    {
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
                for c in 0..3 {
                    hists[c][px[c] as usize] += 1;
                }
            }
        }
    }
    let range = |hist: &[u64; 256]| -> (usize, usize) {
        let total: u64 = hist.iter().sum();
        let clip = total / 1000;
        let mut acc = 0;
        let lo = (0..256).find(|&i| {
            acc += hist[i];
            acc > clip
        });
        acc = 0;
        let hi = (0..256).rev().find(|&i| {
            acc += hist[i];
            acc > clip
        });
        (lo.unwrap_or(0), hi.unwrap_or(255))
    };
    let table = |(lo, hi): (usize, usize)| {
        let mut t = [0u8; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = if hi <= lo {
                i as u8
            } else {
                ((i as f32 - lo as f32) / (hi - lo) as f32 * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
        }
        t
    };
    if per_channel {
        hists.map(|h| table(range(&h)))
    } else {
        let mut all = [0u64; 256];
        for h in &hists {
            for (a, v) in all.iter_mut().zip(h) {
                *a += v;
            }
        }
        [table(range(&all)); 3]
    }
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

/// The gray a color paints on a layer mask (its luminosity).
pub fn mask_gray([r, g, b]: [u8; 3]) -> [u8; 3] {
    let l = luminosity([r, g, b, 255]);
    [l, l, l]
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
    let Some(image) = layer.image() else {
        return hist;
    };
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
    let Some(image) = layer.image() else {
        return hist;
    };
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
    if doc.pixels_locked(layer.id) {
        return Err(FillError::Locked);
    }
    if layer.is_group() {
        return Err(FillError::Group);
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
    let auto = match adjustment {
        Adjustment::AutoTone | Adjustment::AutoColor => Some(auto_tables(doc, true)),
        Adjustment::AutoContrast => Some(auto_tables(doc, false)),
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
            Adjustment::Equalize
            | Adjustment::Levels { .. }
            | Adjustment::Exposure { .. }
            | Adjustment::BrightnessContrast { .. }
            | Adjustment::Curves { .. } => {
                let t = table.as_ref().expect("computed above");
                [t[r as usize], t[g as usize], t[b as usize], a]
            }
            Adjustment::AutoTone | Adjustment::AutoContrast | Adjustment::AutoColor => {
                let t = auto.as_ref().expect("computed above");
                [t[0][r as usize], t[1][g as usize], t[2][b as usize], a]
            }
            Adjustment::ColorBalance { .. }
            | Adjustment::BlackWhite { .. }
            | Adjustment::Vibrance { .. }
            | Adjustment::PhotoFilter { .. }
            | Adjustment::GradientMap { .. } => color_adjust(adjustment, px),
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
    let image = doc
        .layer_mut(id)
        .and_then(|l| l.image_mut())
        .expect("checked: not a group");
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
        let image = doc.layer_mut(id).unwrap().image_mut().unwrap();
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
        let image = d.layer_mut(id).unwrap().image_mut().unwrap();
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
    fn color_adjustments() {
        // Brightness up lightens the midtones, contrast spreads them
        let mut d = doc([128, 128, 128, 255]);
        let bc = Adjustment::BrightnessContrast {
            brightness: 100,
            contrast: 0,
        };
        apply(&mut d, bc).unwrap();
        assert!(first(&d)[0] > 170);
        let mut d = doc([200, 60, 60, 255]);
        let bc = Adjustment::BrightnessContrast {
            brightness: 0,
            contrast: 100,
        };
        apply(&mut d, bc).unwrap();
        assert!(first(&d)[0] > 200 && first(&d)[1] < 60);

        // Black & White with the default weights: pure red at 40% is 102
        let mut d = doc([255, 0, 0, 255]);
        let bw = Adjustment::BlackWhite {
            weights: [40, 60, 40, 60, 20, 80],
        };
        apply(&mut d, bw).unwrap();
        assert_eq!(first(&d), [102, 102, 102, 255]);
        // Yellow uses the yellows weight
        let mut d = doc([255, 255, 0, 255]);
        apply(&mut d, bw).unwrap();
        assert_eq!(first(&d)[0], 153);

        // Gradient map: black to red
        let mut d = doc([255, 255, 255, 255]);
        let gm = Adjustment::GradientMap {
            from: [0, 0, 0],
            to: [255, 0, 0],
        };
        apply(&mut d, gm).unwrap();
        assert_eq!(first(&d), [255, 0, 0, 255]);

        // Color balance toward red keeps luminosity when asked
        let mut d = doc([128, 128, 128, 255]);
        let cb = Adjustment::ColorBalance {
            midtones: [100, 0, 0],
            preserve_luminosity: true,
        };
        apply(&mut d, cb).unwrap();
        let px = first(&d);
        assert!(px[0] > px[1]);
        assert!((luminosity(px) as i32 - 128).abs() <= 1);

        // Vibrance saturates a muted color more than a vivid one
        let mut muted = doc([140, 120, 120, 255]);
        let vib = Adjustment::Vibrance {
            vibrance: 100,
            saturation: 0,
        };
        apply(&mut muted, vib).unwrap();
        assert!(first(&muted)[0] - first(&muted)[1] > 20);

        // Photo filter: a blue filter takes red out
        let mut d = doc([200, 200, 200, 255]);
        let pf = Adjustment::PhotoFilter {
            color: [0, 0, 255],
            density: 50,
            preserve_luminosity: false,
        };
        apply(&mut d, pf).unwrap();
        assert_eq!(first(&d), [100, 100, 200, 255]);
    }

    #[test]
    fn curves_pass_through_their_points() {
        let identity = curve_table(&[(0, 0), (255, 255)]);
        assert!(identity.iter().enumerate().all(|(i, &v)| v == i as u8));
        // An S-curve through (64, 40) and (192, 215) keeps its points and
        // stays smooth and increasing
        let s = curve_table(&[(0, 0), (64, 40), (192, 215), (255, 255)]);
        assert_eq!((s[64], s[192]), (40, 215));
        assert!(s.windows(2).all(|w| w[1] >= w[0]));
        // Beyond the end points the curve is flat
        let clipped = curve_table(&[(30, 0), (220, 255)]);
        assert_eq!((clipped[10], clipped[240]), (0, 255));
        let mut d = doc([64, 64, 64, 255]);
        apply(&mut d, Adjustment::curves(&[(0, 0), (64, 128), (255, 255)])).unwrap();
        assert_eq!(first(&d), [128, 128, 128, 255]);
    }

    #[test]
    fn auto_tone_stretches_each_channel() {
        let mut d = doc([50, 100, 150, 255]);
        let id = d.active_layer.unwrap();
        let image = d.layer_mut(id).unwrap().image_mut().unwrap();
        image.set_pixel(1, 0, [100, 200, 250, 255]);
        apply(&mut d, Adjustment::AutoTone).unwrap();
        assert_eq!(first(&d), [0, 0, 0, 255]);
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
