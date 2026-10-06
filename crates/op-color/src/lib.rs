//! Color model conversions. ICC color management will live here as well.

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
    fn hex() {
        let c = from_hex("#1fa3e0").unwrap();
        assert_eq!(to_hex(c), "1fa3e0");
    }
}
