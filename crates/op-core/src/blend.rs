//! Blend mode math. Formulas follow the W3C Compositing and Blending spec,
//! which matches Photoshop's definitions of these modes. All values are
//! straight (non-premultiplied) and gamma-encoded, in 0..=1.

use crate::layer::BlendMode;

/// Composites a source pixel over a backdrop pixel (both straight alpha).
/// `src_alpha` already includes the layer's opacity and fill. `x`/`y` are the
/// pixel's document position (Dissolve's pattern depends on it).
pub fn composite(
    mode: BlendMode,
    dst: [f32; 4],
    src: [f32; 3],
    src_alpha: f32,
    x: u32,
    y: u32,
) -> [f32; 4] {
    let mut sa = src_alpha;
    if mode == BlendMode::Dissolve {
        // Each pixel is either fully shown or hidden, with probability equal
        // to its alpha
        sa = if dissolve_noise(x, y) < sa { 1.0 } else { 0.0 };
    }
    if sa <= 0.0 {
        return dst;
    }
    let da = dst[3];
    let cb = [dst[0], dst[1], dst[2]];
    let mixed = blend(mode, cb, src);
    let oa = sa + da * (1.0 - sa);
    let mut out = [0.0, 0.0, 0.0, oa];
    for c in 0..3 {
        // W3C general formula: where the backdrop is transparent the source
        // shows as is; where both exist the blend result is used
        let cs = (1.0 - da) * src[c] + da * mixed[c];
        out[c] = (cs * sa + cb[c] * da * (1.0 - sa)) / oa;
    }
    out
}

/// The blend result B(cb, cs) for fully opaque colors.
pub fn blend(mode: BlendMode, cb: [f32; 3], cs: [f32; 3]) -> [f32; 3] {
    use BlendMode::*;
    let sep = |f: fn(f32, f32) -> f32| [f(cb[0], cs[0]), f(cb[1], cs[1]), f(cb[2], cs[2])];
    match mode {
        // Pass Through only reaches here for a group composited as one
        Normal | Dissolve | PassThrough => cs,
        Darken => sep(f32::min),
        Multiply => sep(|b, s| b * s),
        ColorBurn => sep(color_burn),
        LinearBurn => sep(|b, s| (b + s - 1.0).max(0.0)),
        DarkerColor => {
            if sum(cs) < sum(cb) {
                cs
            } else {
                cb
            }
        }
        Lighten => sep(f32::max),
        Screen => sep(screen),
        ColorDodge => sep(color_dodge),
        LinearDodge => sep(|b, s| (b + s).min(1.0)),
        LighterColor => {
            if sum(cs) > sum(cb) {
                cs
            } else {
                cb
            }
        }
        Overlay => sep(|b, s| hard_light(s, b)),
        SoftLight => sep(soft_light),
        HardLight => sep(hard_light),
        VividLight => sep(|b, s| {
            if s <= 0.5 {
                color_burn(b, 2.0 * s)
            } else {
                color_dodge(b, 2.0 * s - 1.0)
            }
        }),
        LinearLight => sep(|b, s| (b + 2.0 * s - 1.0).clamp(0.0, 1.0)),
        PinLight => sep(|b, s| {
            if s <= 0.5 {
                b.min(2.0 * s)
            } else {
                b.max(2.0 * s - 1.0)
            }
        }),
        HardMix => sep(|b, s| if b + s >= 1.0 { 1.0 } else { 0.0 }),
        Difference => sep(|b, s| (b - s).abs()),
        Exclusion => sep(|b, s| b + s - 2.0 * b * s),
        Subtract => sep(|b, s| (b - s).max(0.0)),
        Divide => sep(|b, s| {
            if s <= 0.0 {
                if b <= 0.0 { 0.0 } else { 1.0 }
            } else {
                (b / s).min(1.0)
            }
        }),
        Hue => set_lum(set_sat(cs, sat(cb)), lum(cb)),
        Saturation => set_lum(set_sat(cb, sat(cs)), lum(cb)),
        Color => set_lum(cs, lum(cb)),
        Luminosity => set_lum(cb, lum(cs)),
    }
}

fn sum(c: [f32; 3]) -> f32 {
    c[0] + c[1] + c[2]
}

fn screen(b: f32, s: f32) -> f32 {
    b + s - b * s
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b * 2.0 * s
    } else {
        screen(b, 2.0 * s - 1.0)
    }
}

fn color_dodge(b: f32, s: f32) -> f32 {
    if b <= 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        (b / (1.0 - s)).min(1.0)
    }
}

fn color_burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 {
        1.0
    } else if s <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - b) / s).min(1.0)
    }
}

fn soft_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b - (1.0 - 2.0 * s) * b * (1.0 - b)
    } else {
        let d = if b <= 0.25 {
            ((16.0 * b - 12.0) * b + 4.0) * b
        } else {
            b.sqrt()
        };
        b + (2.0 * s - 1.0) * (d - b)
    }
}

fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut c = c;
    if n < 0.0 {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 {
        c = c.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    c
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    if max <= min {
        return [0.0; 3];
    }
    c.map(|v| (v - min) * s / (max - min))
}

/// Deterministic per-pixel noise in 0..1 for Dissolve.
fn dissolve_noise(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use BlendMode::*;

    fn g(v: f32) -> [f32; 3] {
        [v, v, v]
    }

    fn q(c: [f32; 3]) -> [u8; 3] {
        c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8)
    }

    #[test]
    fn separable_modes() {
        let b = g(128.0 / 255.0);
        assert_eq!(q(blend(Multiply, b, b)), [64, 64, 64]);
        assert_eq!(q(blend(Screen, b, b)), [192, 192, 192]);
        assert_eq!(q(blend(Difference, g(1.0), g(0.25))), [191, 191, 191]);
        assert_eq!(q(blend(Darken, g(0.2), g(0.6))), q(g(0.2)));
        assert_eq!(q(blend(Lighten, g(0.2), g(0.6))), q(g(0.6)));
        assert_eq!(q(blend(LinearDodge, g(0.6), g(0.6))), [255, 255, 255]);
        assert_eq!(q(blend(Subtract, g(0.6), g(0.2))), q(g(0.4)));
        // Multiplying by white and screening with black change nothing
        let c = [0.1, 0.5, 0.9];
        assert_eq!(q(blend(Multiply, c, g(1.0))), q(c));
        assert_eq!(q(blend(Screen, c, g(0.0))), q(c));
        // Overlay keeps black and white backdrops
        assert_eq!(q(blend(Overlay, g(0.0), g(0.7))), [0, 0, 0]);
        assert_eq!(q(blend(Overlay, g(1.0), g(0.3))), [255, 255, 255]);
    }

    #[test]
    fn non_separable_modes() {
        let red = [1.0, 0.0, 0.0];
        let gray = g(0.5);
        // Color takes hue and saturation from the source, luminosity from the backdrop
        let c = blend(Color, gray, red);
        assert!((lum(c) - 0.5).abs() < 1e-4);
        assert!(c[0] > c[1] && (c[1] - c[2]).abs() < 1e-4);
        // Luminosity of a gray source on a color backdrop keeps the hue
        let l = blend(Luminosity, red, g(0.8));
        assert!((lum(l) - 0.8).abs() < 1e-3);
        assert!(l[0] >= l[1]);
        // Saturation from a gray source desaturates
        assert_eq!(sat(blend(Saturation, red, gray)), 0.0);
    }

    #[test]
    fn composite_over_transparent_shows_source() {
        let out = composite(Multiply, [0.0; 4], [0.2, 0.4, 0.6], 1.0, 0, 0);
        assert_eq!(q([out[0], out[1], out[2]]), q([0.2, 0.4, 0.6]));
        assert_eq!(out[3], 1.0);
    }

    #[test]
    fn dissolve_is_all_or_nothing() {
        let mut shown = 0;
        for i in 0..1000 {
            let out = composite(Dissolve, [0.0, 0.0, 0.0, 1.0], g(1.0), 0.3, i, i * 7);
            assert!(out[0] == 0.0 || out[0] == 1.0);
            shown += (out[0] == 1.0) as u32;
        }
        assert!((200..400).contains(&shown), "{shown}");
    }
}
