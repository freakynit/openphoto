//! Color model conversions used by the color picker.

use op_core::Color;

/// HSB (as named in the Photoshop color picker): h ∈ 0..360, s/b ∈ 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsb {
    pub h: f32,
    pub s: f32,
    pub b: f32,
}

impl Hsb {
    pub fn from_color(c: Color) -> Self {
        let max = c.r.max(c.g).max(c.b);
        let min = c.r.min(c.g).min(c.b);
        let d = max - min;
        let h = if d == 0.0 {
            0.0
        } else if max == c.r {
            60.0 * ((c.g - c.b) / d).rem_euclid(6.0)
        } else if max == c.g {
            60.0 * ((c.b - c.r) / d + 2.0)
        } else {
            60.0 * ((c.r - c.g) / d + 4.0)
        };
        let s = if max == 0.0 { 0.0 } else { d / max };
        Self { h, s, b: max }
    }

    pub fn to_color(self) -> Color {
        let c = self.b * self.s;
        let hp = (self.h.rem_euclid(360.0)) / 60.0;
        let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
        let (r, g, b) = match hp as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = self.b - c;
        Color::rgb(r + m, g + m, b + m)
    }
}

/// CIELAB relative to D50, as Photoshop's color picker shows it.
/// `l` ∈ 0..=100, `a`/`b` roughly −128..=127.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

/// D50 reference white.
const WHITE_D50: [f32; 3] = [0.96422, 1.0, 0.82521];

/// Linear sRGB → XYZ (D65), then Bradford-adapted to D50.
const RGB_TO_XYZ_D50: [[f32; 3]; 3] = [
    [0.4360747, 0.3850649, 0.1430804],
    [0.2225045, 0.7168786, 0.0606169],
    [0.0139322, 0.0971045, 0.7141733],
];

/// Inverse of [`RGB_TO_XYZ_D50`].
const XYZ_D50_TO_RGB: [[f32; 3]; 3] = [
    [3.133_856, -1.616_867, -0.490_614_6],
    [-0.9787684, 1.9161415, 0.0334540],
    [0.0719453, -0.2289914, 1.4052427],
];

fn mul(m: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

const EPSILON: f32 = 216.0 / 24389.0;
const KAPPA: f32 = 24389.0 / 27.0;

impl Lab {
    pub fn from_color(c: Color) -> Self {
        let xyz = mul(&RGB_TO_XYZ_D50, [c.r, c.g, c.b].map(srgb_to_linear));
        let f = |t: f32| {
            if t > EPSILON {
                t.cbrt()
            } else {
                (KAPPA * t + 16.0) / 116.0
            }
        };
        let [fx, fy, fz] = [0, 1, 2].map(|i| f(xyz[i] / WHITE_D50[i]));
        Self {
            l: 116.0 * fy - 16.0,
            a: 500.0 * (fx - fy),
            b: 200.0 * (fy - fz),
        }
    }

    /// Converts to sRGB, clamping colors outside the sRGB gamut.
    pub fn to_color(self) -> Color {
        let fy = (self.l + 16.0) / 116.0;
        let fx = fy + self.a / 500.0;
        let fz = fy - self.b / 200.0;
        let inv = |f: f32| {
            let t = f * f * f;
            if t > EPSILON {
                t
            } else {
                (116.0 * f - 16.0) / KAPPA
            }
        };
        let xyz = [inv(fx), inv(fy), inv(fz)];
        let xyz = [0, 1, 2].map(|i| xyz[i] * WHITE_D50[i]);
        let [r, g, b] = mul(&XYZ_D50_TO_RGB, xyz).map(|v| linear_to_srgb(v.clamp(0.0, 1.0)));
        Color::rgb(r, g, b)
    }
}

/// CMYK fractions (0..=1).
///
/// This is the device-independent formula, not Photoshop's: Photoshop
/// converts through the working CMYK profile (U.S. Web Coated SWOP by
/// default), so its numbers differ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cmyk {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}

impl Cmyk {
    pub fn from_color(c: Color) -> Self {
        let k = 1.0 - c.r.max(c.g).max(c.b);
        if k >= 1.0 {
            return Self {
                c: 0.0,
                m: 0.0,
                y: 0.0,
                k: 1.0,
            };
        }
        let f = |v: f32| (1.0 - v - k) / (1.0 - k);
        Self {
            c: f(c.r),
            m: f(c.g),
            y: f(c.b),
            k,
        }
    }

    pub fn to_color(self) -> Color {
        let f = |v: f32| (1.0 - v) * (1.0 - self.k);
        Color::rgb(f(self.c), f(self.m), f(self.y))
    }
}

/// Snaps to the nearest web-safe color (each channel a multiple of 0x33).
pub fn web_safe(c: Color) -> Color {
    let [r, g, b, a] = c.to_rgba8();
    let snap = |v: u8| ((v as f32 / 51.0).round() * 51.0) as u8;
    Color::from_rgba8([snap(r), snap(g), snap(b), a])
}

pub fn is_web_safe(c: Color) -> bool {
    c.to_rgba8()[..3].iter().all(|v| v % 51 == 0)
}

/// Hex color without the leading `#`, e.g. `1fa3e0`.
pub fn to_hex(c: Color) -> String {
    let [r, g, b, _] = c.to_rgba8();
    format!("{r:02x}{g:02x}{b:02x}")
}

pub fn from_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color::from_rgba8([
        (v >> 16) as u8,
        (v >> 8) as u8,
        v as u8,
        255,
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsb_round_trip() {
        for rgb in [[255, 0, 0], [12, 160, 220], [128, 128, 128], [0, 0, 0]] {
            let c = Color::from_rgba8([rgb[0], rgb[1], rgb[2], 255]);
            assert_eq!(Hsb::from_color(c).to_color().to_rgba8(), c.to_rgba8());
        }
    }

    #[test]
    fn lab_matches_photoshop() {
        // Photoshop's color picker shows #00afdc as L 66, a -27, b -34
        let lab = Lab::from_color(from_hex("00afdc").unwrap());
        assert_eq!(
            (lab.l.round(), lab.a.round(), lab.b.round()),
            (66.0, -27.0, -34.0)
        );
        let white = Lab::from_color(Color::WHITE);
        assert!((white.l - 100.0).abs() < 0.01 && white.a.abs() < 0.01 && white.b.abs() < 0.01);
    }

    #[test]
    fn lab_round_trip() {
        for hex in ["00afdc", "ff0000", "808080", "123456", "000000"] {
            let c = from_hex(hex).unwrap();
            assert_eq!(to_hex(Lab::from_color(c).to_color()), hex);
        }
    }

    #[test]
    fn cmyk_round_trip() {
        let c = from_hex("00afdc").unwrap();
        let cmyk = Cmyk::from_color(c);
        assert_eq!(to_hex(cmyk.to_color()), "00afdc");
        assert_eq!(Cmyk::from_color(Color::BLACK).k, 1.0);
    }

    #[test]
    fn web_safe_snapping() {
        let c = from_hex("00afdc").unwrap();
        assert!(!is_web_safe(c));
        assert_eq!(to_hex(web_safe(c)), "0099cc");
        assert!(is_web_safe(web_safe(c)));
    }

    #[test]
    fn hex() {
        let c = from_hex("#1fa3e0").unwrap();
        assert_eq!(to_hex(c), "1fa3e0");
    }
}
