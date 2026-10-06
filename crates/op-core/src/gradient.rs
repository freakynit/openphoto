//! The Gradient tool (classic gradient): a two-color gradient painted on
//! the active layer, limited to the selection.

use crate::blend;
use crate::document::Document;
use crate::fill::FillError;
use crate::layer::BlendMode;

/// The gradient shape (the five buttons in the options bar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GradientKind {
    #[default]
    Linear,
    Radial,
    Angle,
    Reflected,
    Diamond,
}

impl GradientKind {
    pub const ALL: [Self; 5] = [
        Self::Linear,
        Self::Radial,
        Self::Angle,
        Self::Reflected,
        Self::Diamond,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear Gradient",
            Self::Radial => "Radial Gradient",
            Self::Angle => "Angle Gradient",
            Self::Reflected => "Reflected Gradient",
            Self::Diamond => "Diamond Gradient",
        }
    }

    /// Position along the gradient (0 at the start color, 1 at the end) of
    /// the point (`x`, `y`) for a drag from `a` to `b`.
    pub fn position(self, a: (f32, f32), b: (f32, f32), x: f32, y: f32) -> f32 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = dx * dx + dy * dy;
        if len2 <= 0.0 {
            return 0.0;
        }
        let len = len2.sqrt();
        let (px, py) = (x - a.0, y - a.1);
        let along = (px * dx + py * dy) / len2;
        match self {
            Self::Linear => along.clamp(0.0, 1.0),
            Self::Radial => ((px * px + py * py).sqrt() / len).min(1.0),
            Self::Reflected => along.abs().min(1.0),
            Self::Diamond => {
                // Distance in the frame turned to the drag direction
                let (ux, uy) = (dx / len, dy / len);
                let u = (px * ux + py * uy).abs();
                let v = (-px * uy + py * ux).abs();
                ((u + v) / len).min(1.0)
            }
            Self::Angle => {
                // Sweeps once around the start point, from the drag's direction
                let start = dy.atan2(dx);
                let angle = py.atan2(px);
                let turn = (start - angle).rem_euclid(std::f32::consts::TAU);
                turn / std::f32::consts::TAU
            }
        }
    }
}

/// How a gradient blends between its colors (Photoshop 2026's Method).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Method {
    /// In OKLab, with the classic easing.
    Perceptual,
    /// In linear light, with the classic easing.
    Linear,
    /// In sRGB values, with the classic easing (Photoshop before 2023).
    Classic,
    /// In OKLab, with a lighter easing (the dialogs' default).
    #[default]
    Smooth,
}

impl Method {
    pub const ALL: [Self; 4] = [Self::Perceptual, Self::Linear, Self::Classic, Self::Smooth];

    pub fn label(self) -> &'static str {
        match self {
            Self::Perceptual => "Perceptual",
            Self::Linear => "Linear",
            Self::Classic => "Classic",
            Self::Smooth => "Smooth",
        }
    }
}

/// The color `t` (0–1) of the way from `a` to `b` (measured against
/// Photoshop 2026's Gradient Map): Photoshop's gradients ease between two
/// stops — the classic easing is the mean of `t` and smoothstep — and
/// Smooth eases 40% as much.
pub fn blend_colors(a: [u8; 3], b: [u8; 3], t: f32, method: Method) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let smoothstep = t * t * (3.0 - 2.0 * t);
    let classic = (t + smoothstep) / 2.0;
    let round = |v: f32| (v * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
    match method {
        Method::Classic => {
            [0, 1, 2].map(|c| round((a[c] as f32 + (b[c] as f32 - a[c] as f32) * classic) / 255.0))
        }
        Method::Linear => {
            let (la, lb) = (a.map(srgb_to_linear), b.map(srgb_to_linear));
            [0, 1, 2].map(|c| round(linear_to_srgb(la[c] + (lb[c] - la[c]) * classic)))
        }
        Method::Perceptual | Method::Smooth => {
            let k = if method == Method::Perceptual {
                classic
            } else {
                t + (classic - t) * 0.4
            };
            let (oa, ob) = (oklab(a), oklab(b));
            let mixed = [0, 1, 2].map(|i| oa[i] + (ob[i] - oa[i]) * k);
            from_oklab(mixed).map(round)
        }
    }
}

fn srgb_to_linear(v: u8) -> f32 {
    let v = v as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB to OKLab (Björn Ottosson's).
fn oklab(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(srgb_to_linear);
    let l = 0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let [l, m, s] = [l, m, s].map(f32::cbrt);
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

/// OKLab back to sRGB (0–1, clipped).
fn from_oklab([ll, a, b]: [f32; 3]) -> [f32; 3] {
    let l = ll + 0.396_337_78 * a + 0.215_803_76 * b;
    let m = ll - 0.105_561_346 * a - 0.063_854_17 * b;
    let s = ll - 0.089_484_18 * a - 1.291_485_5 * b;
    let [l, m, s] = [l, m, s].map(|v| v * v * v);
    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_93 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
    .map(linear_to_srgb)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientOptions {
    pub kind: GradientKind,
    pub mode: BlendMode,
    pub opacity: f32,
    /// Swaps the start and end colors.
    pub reverse: bool,
}

impl Default for GradientOptions {
    fn default() -> Self {
        Self {
            kind: GradientKind::Linear,
            mode: BlendMode::Normal,
            opacity: 1.0,
            reverse: false,
        }
    }
}

/// Paints a gradient from `from` (at `a`) to `to` (at `b`) on the active
/// layer. Colors are interpolated in RGB; partly selected pixels get a
/// proportional amount; the background layer and layers with locked
/// transparency keep their alpha.
pub fn gradient(
    doc: &mut Document,
    a: (f32, f32),
    b: (f32, f32),
    colors: ([u8; 3], [u8; 3]),
    options: GradientOptions,
) -> Result<(), FillError> {
    // Quick Mask can be painted whatever the layer's state
    if doc.quick_mask.is_none() {
        crate::adjust::check(doc)?;
    }
    let (c0, c1) = if options.reverse {
        (colors.1, colors.0)
    } else {
        colors
    };
    let selection = doc.selection().cloned();
    let (w, h) = (doc.width, doc.height);
    let target = doc.edit_target().expect("checked");
    let keep_alpha = target.keep_alpha;
    // On a mask the gradient runs between the colors' grays
    let (c0, c1) = if target.mask {
        (crate::adjust::mask_gray(c0), crate::adjust::mask_gray(c1))
    } else {
        (c0, c1)
    };
    let (c0, c1) = (c0.map(|v| v as f32 / 255.0), c1.map(|v| v as f32 / 255.0));
    let image = target.image;
    for y in 0..h {
        for x in 0..w {
            let amount = options.opacity
                * selection
                    .as_ref()
                    .map_or(1.0, |s| s.get(x, y) as f32 / 255.0);
            if amount <= 0.0 {
                continue;
            }
            let base = image.pixel(x, y);
            if keep_alpha && base[3] == 0 {
                continue;
            }
            let t = options.kind.position(a, b, x as f32 + 0.5, y as f32 + 0.5);
            let src = [0, 1, 2].map(|c| c0[c] + (c1[c] - c0[c]) * t);
            let dst = base.map(|v| v as f32 / 255.0);
            let out = if keep_alpha {
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

    fn row(doc: &Document) -> Vec<u8> {
        doc.composite_rgba8().chunks(4).map(|p| p[0]).collect()
    }

    #[test]
    fn positions_of_each_kind() {
        let (a, b) = ((0.0, 0.0), (10.0, 0.0));
        assert_eq!(GradientKind::Linear.position(a, b, 5.0, 3.0), 0.5);
        assert_eq!(GradientKind::Linear.position(a, b, -4.0, 0.0), 0.0);
        assert_eq!(GradientKind::Radial.position(a, b, 0.0, 5.0), 0.5);
        assert_eq!(GradientKind::Reflected.position(a, b, -5.0, 0.0), 0.5);
        assert_eq!(GradientKind::Diamond.position(a, b, 2.5, 2.5), 0.5);
        assert_eq!(GradientKind::Angle.position(a, b, 5.0, 0.0), 0.0);
        let quarter = GradientKind::Angle.position(a, b, 0.0, -5.0);
        assert!((quarter - 0.25).abs() < 1e-6, "{quarter}");
    }

    #[test]
    fn paints_black_to_white() {
        let mut d = Document::new_with_background("t", 5, 1, Color::WHITE);
        let g = GradientOptions::default();
        gradient(&mut d, (0.5, 0.5), (4.5, 0.5), ([0; 3], [255; 3]), g).unwrap();
        assert_eq!(row(&d), [0, 64, 128, 191, 255]);
        let reversed = GradientOptions { reverse: true, ..g };
        gradient(&mut d, (0.5, 0.5), (4.5, 0.5), ([0; 3], [255; 3]), reversed).unwrap();
        assert_eq!(row(&d), [255, 191, 128, 64, 0]);
    }

    #[test]
    fn half_opacity_and_selection() {
        let mut d = Document::new_with_background("t", 2, 1, Color::WHITE);
        d.set_selection(Some(crate::selection::Selection::rect(
            2,
            1,
            crate::selection::Rect::new(0.0, 0.0, 1.0, 1.0),
        )));
        let g = GradientOptions {
            opacity: 0.5,
            ..Default::default()
        };
        gradient(&mut d, (0.0, 0.0), (1.0, 0.0), ([0; 3], [0; 3]), g).unwrap();
        assert_eq!(row(&d), [128, 255]);
    }
}
