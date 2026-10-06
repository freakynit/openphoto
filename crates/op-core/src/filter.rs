//! Filter menu filters applied to the active layer, limited to the
//! selection.
//!
//! Filters read the whole layer (a blur near the selection edge sees the
//! pixels outside it) and write only the selected pixels. Colors are
//! filtered premultiplied by alpha, so transparent pixels don't bleed their
//! (meaningless) color into their neighbors.

use crate::document::Document;
use crate::fill::FillError;
use crate::tile::TiledImage;

/// What Filter > Other > Offset puts in the area uncovered by the shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OffsetFill {
    /// Transparent, or the background color on the background layer.
    Background,
    /// The nearest edge pixels.
    RepeatEdges,
    /// The pixels pushed off the other side.
    Wrap,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    /// Blur > Gaussian Blur; the radius is the standard deviation in pixels.
    GaussianBlur { radius: f32 },
    /// Blur > Box Blur: the average of a (2r + 1)² square.
    BoxBlur { radius: u32 },
    /// Blur > Average: the selection filled with its average color.
    Average,
    /// Sharpen > Unsharp Mask: amount in percent, Gaussian radius, and the
    /// threshold (0–255) a difference must exceed to be sharpened.
    UnsharpMask {
        amount: f32,
        radius: f32,
        threshold: u8,
    },
    /// Noise > Add Noise: amount in percent; uniform or Gaussian; the same
    /// noise on all channels when monochromatic.
    AddNoise {
        amount: f32,
        gaussian: bool,
        monochromatic: bool,
    },
    /// Noise > Median over a (2r + 1)² square, per channel.
    Median { radius: u32 },
    /// Other > Minimum: each channel's smallest value in a (2r + 1)² square.
    Minimum { radius: u32 },
    /// Other > Maximum: the largest value.
    Maximum { radius: u32 },
    /// Other > High Pass: the image minus its Gaussian blur, around gray.
    HighPass { radius: f32 },
    /// Other > Offset: shifts the layer by (dx, dy) pixels.
    Offset { dx: i32, dy: i32, fill: OffsetFill },
    /// Pixelate > Mosaic: squares of `cell` pixels filled with their average.
    Mosaic { cell: u32 },
    /// Stylize > Solarize: values above 127 are inverted.
    Solarize,
}

impl Filter {
    /// The menu and history name.
    pub fn name(self) -> &'static str {
        match self {
            Self::GaussianBlur { .. } => "Gaussian Blur",
            Self::BoxBlur { .. } => "Box Blur",
            Self::Average => "Average",
            Self::UnsharpMask { .. } => "Unsharp Mask",
            Self::AddNoise { .. } => "Add Noise",
            Self::Median { .. } => "Median",
            Self::Minimum { .. } => "Minimum",
            Self::Maximum { .. } => "Maximum",
            Self::HighPass { .. } => "High Pass",
            Self::Offset { .. } => "Offset",
            Self::Mosaic { .. } => "Mosaic",
            Self::Solarize => "Solarize",
        }
    }
}

/// A layer as premultiplied RGBA floats (0–255).
struct Buffer {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

impl Buffer {
    fn from_image(image: &TiledImage) -> Self {
        let raw = image.to_rgba8();
        let px = raw
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, a]| {
                let k = a as f32 / 255.0;
                [r as f32 * k, g as f32 * k, b as f32 * k, a as f32]
            })
            .collect();
        Self {
            w: image.width() as usize,
            h: image.height() as usize,
            px,
        }
    }

    fn at(&self, x: isize, y: isize) -> [f32; 4] {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.px[y * self.w + x]
    }

    fn straight(p: [f32; 4]) -> [u8; 4] {
        let a = p[3].clamp(0.0, 255.0);
        if a <= 0.0 {
            return [0; 4];
        }
        let k = 255.0 / a;
        [
            (p[0] * k).round().clamp(0.0, 255.0) as u8,
            (p[1] * k).round().clamp(0.0, 255.0) as u8,
            (p[2] * k).round().clamp(0.0, 255.0) as u8,
            a.round() as u8,
        ]
    }

    /// Convolves rows, then columns, with a symmetric 1-D kernel.
    fn separable(&self, kernel: &[f32]) -> Self {
        let r = (kernel.len() / 2) as isize;
        let pass = |src: &Buffer, horizontal: bool| -> Buffer {
            let mut out = vec![[0f32; 4]; src.w * src.h];
            for y in 0..src.h as isize {
                for x in 0..src.w as isize {
                    let mut acc = [0f32; 4];
                    for (k, &wgt) in kernel.iter().enumerate() {
                        let d = k as isize - r;
                        let p = if horizontal {
                            src.at(x + d, y)
                        } else {
                            src.at(x, y + d)
                        };
                        for c in 0..4 {
                            acc[c] += p[c] * wgt;
                        }
                    }
                    out[y as usize * src.w + x as usize] = acc;
                }
            }
            Buffer {
                w: src.w,
                h: src.h,
                px: out,
            }
        };
        pass(&pass(self, true), false)
    }

    fn gaussian(&self, sigma: f32) -> Self {
        if sigma <= 0.0 {
            return Self {
                w: self.w,
                h: self.h,
                px: self.px.clone(),
            };
        }
        let r = (sigma * 3.0).ceil() as isize;
        let mut kernel: Vec<f32> = (-r..=r)
            .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
            .collect();
        let sum: f32 = kernel.iter().sum();
        kernel.iter_mut().for_each(|k| *k /= sum);
        self.separable(&kernel)
    }

    /// Applies `f` to each channel's values in the (2r + 1)² square.
    fn rank(&self, radius: u32, f: impl Fn(&mut Vec<f32>) -> f32) -> Self {
        let r = radius as isize;
        let mut out = vec![[0f32; 4]; self.w * self.h];
        let mut values = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
        for y in 0..self.h as isize {
            for x in 0..self.w as isize {
                let mut p = [0f32; 4];
                for (c, v) in p.iter_mut().enumerate() {
                    values.clear();
                    for dy in -r..=r {
                        for dx in -r..=r {
                            values.push(self.at(x + dx, y + dy)[c]);
                        }
                    }
                    *v = f(&mut values);
                }
                out[y as usize * self.w + x as usize] = p;
            }
        }
        Self {
            w: self.w,
            h: self.h,
            px: out,
        }
    }
}

/// A repeatable per-pixel random value in 0..1.
fn noise(x: usize, y: usize, channel: usize) -> f32 {
    let mut v = (x as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        .wrapping_add(channel as u64 * 0x1656_67B1_9E37_79F9);
    v ^= v >> 33;
    v = v.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    v ^= v >> 33;
    (v >> 40) as f32 / (1u64 << 24) as f32
}

/// The filtered layer as straight RGBA8, computed from the whole layer.
fn filtered(
    image: &TiledImage,
    filter: Filter,
    selection: Option<&crate::selection::Selection>,
    background: Option<[u8; 3]>,
) -> Vec<[u8; 4]> {
    let src = Buffer::from_image(image);
    let (w, h) = (src.w, src.h);
    let per_pixel = |f: &dyn Fn(usize, usize, [u8; 4]) -> [u8; 4]| -> Vec<[u8; 4]> {
        (0..w * h)
            .map(|i| f(i % w, i / w, Buffer::straight(src.px[i])))
            .collect()
    };
    match filter {
        Filter::GaussianBlur { radius } => src
            .gaussian(radius)
            .px
            .into_iter()
            .map(Buffer::straight)
            .collect(),
        Filter::BoxBlur { radius } => {
            let n = 2 * radius as usize + 1;
            src.separable(&vec![1.0 / n as f32; n])
                .px
                .into_iter()
                .map(Buffer::straight)
                .collect()
        }
        Filter::Average => {
            // Premultiplied mean of the selected (or all) pixels
            let mut sum = [0f64; 4];
            let mut weight = 0f64;
            for (i, p) in src.px.iter().enumerate() {
                let m = selection.map_or(1.0, |s| {
                    s.get((i % w) as u32, (i / w) as u32) as f64 / 255.0
                });
                for c in 0..4 {
                    sum[c] += p[c] as f64 * m;
                }
                weight += m;
            }
            let mean = sum.map(|v| (v / weight.max(1e-9)) as f32);
            vec![Buffer::straight(mean); w * h]
        }
        Filter::UnsharpMask {
            amount,
            radius,
            threshold,
        } => {
            let blurred = src.gaussian(radius);
            (0..w * h)
                .map(|i| {
                    let orig = Buffer::straight(src.px[i]);
                    let blur = Buffer::straight(blurred.px[i]);
                    let mut out = orig;
                    for c in 0..3 {
                        let diff = orig[c] as f32 - blur[c] as f32;
                        if diff.abs() > threshold as f32 {
                            out[c] = (orig[c] as f32 + diff * amount / 100.0)
                                .round()
                                .clamp(0.0, 255.0) as u8;
                        }
                    }
                    out
                })
                .collect()
        }
        Filter::AddNoise {
            amount,
            gaussian,
            monochromatic,
        } => per_pixel(&|x, y, px| {
            let mut out = px;
            for c in 0..3 {
                let channel = if monochromatic { 0 } else { c };
                let u = noise(x, y, channel);
                let n = if gaussian {
                    // Sum of uniforms: roughly normal with unit variance
                    let s: f32 = (0..4).map(|k| noise(x, y, channel * 4 + k + 8)).sum();
                    (s - 2.0) * 3f32.sqrt()
                } else {
                    u * 2.0 - 1.0
                };
                let v = px[c] as f32 + n * amount / 100.0 * 127.5;
                out[c] = v.round().clamp(0.0, 255.0) as u8;
            }
            out
        }),
        Filter::Median { radius } => src
            .rank(radius, |v| {
                let mid = v.len() / 2;
                *v.select_nth_unstable_by(mid, f32::total_cmp).1
            })
            .px
            .into_iter()
            .map(Buffer::straight)
            .collect(),
        Filter::Minimum { radius } => src
            .rank(radius, |v| v.iter().copied().fold(f32::MAX, f32::min))
            .px
            .into_iter()
            .map(Buffer::straight)
            .collect(),
        Filter::Maximum { radius } => src
            .rank(radius, |v| v.iter().copied().fold(f32::MIN, f32::max))
            .px
            .into_iter()
            .map(Buffer::straight)
            .collect(),
        Filter::HighPass { radius } => {
            let blurred = src.gaussian(radius);
            (0..w * h)
                .map(|i| {
                    let orig = Buffer::straight(src.px[i]);
                    let blur = Buffer::straight(blurred.px[i]);
                    let mut out = orig;
                    for c in 0..3 {
                        out[c] = (orig[c] as f32 - blur[c] as f32 + 128.0)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                    out
                })
                .collect()
        }
        Filter::Offset { dx, dy, fill } => {
            let empty = background.map_or([0; 4], |[r, g, b]| [r, g, b, 255]);
            per_pixel(&|x, y, _| {
                let sx = x as i64 - dx as i64;
                let sy = y as i64 - dy as i64;
                let (wi, hi) = (w as i64, h as i64);
                let inside = (0..wi).contains(&sx) && (0..hi).contains(&sy);
                let (sx, sy) = match fill {
                    _ if inside => (sx, sy),
                    OffsetFill::Background => return empty,
                    OffsetFill::RepeatEdges => (sx.clamp(0, wi - 1), sy.clamp(0, hi - 1)),
                    OffsetFill::Wrap => (sx.rem_euclid(wi), sy.rem_euclid(hi)),
                };
                Buffer::straight(src.px[(sy * wi + sx) as usize])
            })
        }
        Filter::Mosaic { cell } => {
            let cell = cell.max(1) as usize;
            let mut out = vec![[0u8; 4]; w * h];
            for cy in (0..h).step_by(cell) {
                for cx in (0..w).step_by(cell) {
                    let (x1, y1) = ((cx + cell).min(w), (cy + cell).min(h));
                    let mut sum = [0f32; 4];
                    for y in cy..y1 {
                        for x in cx..x1 {
                            let p = src.px[y * w + x];
                            for c in 0..4 {
                                sum[c] += p[c];
                            }
                        }
                    }
                    let n = ((x1 - cx) * (y1 - cy)) as f32;
                    let mean = Buffer::straight(sum.map(|v| v / n));
                    for y in cy..y1 {
                        out[y * w + cx..y * w + x1].fill(mean);
                    }
                }
            }
            out
        }
        Filter::Solarize => per_pixel(&|_, _, px| {
            let s = |v: u8| if v > 127 { 255 - v } else { v };
            [s(px[0]), s(px[1]), s(px[2]), px[3]]
        }),
    }
}

/// Applies `filter` to the active layer. The background layer and layers
/// with locked transparency keep their alpha; partly selected pixels mix
/// the old and new values.
pub fn apply(doc: &mut Document, filter: Filter, background: [u8; 3]) -> Result<(), FillError> {
    crate::adjust::check(doc)?;
    let selection = doc.selection().cloned();
    let (w, h) = (doc.width, doc.height);
    let id = doc.active_layer.expect("checked");
    let layer = doc.layer_mut(id).expect("checked");
    let keep_alpha = layer.is_background || layer.transparency_locked();
    let is_background = layer.is_background;
    let image = layer.image_mut().expect("checked: not a group");
    let out = filtered(
        image,
        filter,
        selection.as_ref(),
        is_background.then_some(background),
    );
    for y in 0..h {
        for x in 0..w {
            let m = selection.as_ref().map_or(255, |s| s.get(x, y)) as u32;
            if m == 0 {
                continue;
            }
            let old = image.pixel(x, y);
            let mut new = out[(y * w + x) as usize];
            if keep_alpha {
                new[3] = old[3];
            }
            if m < 255 {
                for c in 0..4 {
                    new[c] = ((old[c] as u32 * (255 - m) + new[c] as u32 * m + 127) / 255) as u8;
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
    use crate::layer::Layer;

    /// A 5×1 white background with a black pixel in the middle.
    fn doc() -> Document {
        let mut doc = Document::new_with_background("t", 5, 1, Color::WHITE);
        let id = doc.active_layer.unwrap();
        let image = doc.layer_mut(id).unwrap().image_mut().unwrap();
        image.set_pixel(2, 0, [0, 0, 0, 255]);
        doc
    }

    fn row(doc: &Document) -> Vec<u8> {
        doc.composite_rgba8().chunks(4).map(|p| p[0]).collect()
    }

    fn run(filter: Filter) -> Vec<u8> {
        let mut d = doc();
        apply(&mut d, filter, [255, 255, 255]).unwrap();
        row(&d)
    }

    #[test]
    fn blurs_spread_the_dark_pixel() {
        assert_eq!(
            run(Filter::BoxBlur { radius: 1 }),
            [255, 170, 170, 170, 255]
        );
        let g = run(Filter::GaussianBlur { radius: 1.0 });
        assert!(g[2] > 0 && g[2] < g[1] && g[1] < g[0], "{g:?}");
        assert_eq!(g[1], g[3]);
        assert_eq!(run(Filter::Average), [204; 5]);
    }

    #[test]
    fn rank_filters() {
        assert_eq!(run(Filter::Median { radius: 1 }), [255; 5]);
        assert_eq!(run(Filter::Minimum { radius: 1 }), [255, 0, 0, 0, 255]);
        assert_eq!(run(Filter::Maximum { radius: 1 }), [255; 5]);
    }

    #[test]
    fn sharpen_high_pass_and_solarize() {
        // Unsharp Mask darkens the dark pixel's neighbors' contrast edge
        let u = run(Filter::UnsharpMask {
            amount: 100.0,
            radius: 1.0,
            threshold: 0,
        });
        assert_eq!(u[2], 0);
        assert_eq!(u[0], 255);
        let hp = run(Filter::HighPass { radius: 1.0 });
        assert!(hp[2] < 128 && hp[0] > 128, "{hp:?}");
        assert_eq!(run(Filter::Solarize), [0, 0, 0, 0, 0]);
    }

    #[test]
    fn offset_wraps_or_fills_with_the_background() {
        let wrap = Filter::Offset {
            dx: 3,
            dy: 0,
            fill: OffsetFill::Wrap,
        };
        assert_eq!(run(wrap), [0, 255, 255, 255, 255]);
        let mut d = doc();
        let back = Filter::Offset {
            dx: -2,
            dy: 0,
            fill: OffsetFill::Background,
        };
        apply(&mut d, back, [9, 9, 9]).unwrap();
        assert_eq!(row(&d), [0, 255, 255, 9, 9]);
    }

    #[test]
    fn mosaic_and_noise() {
        assert_eq!(run(Filter::Mosaic { cell: 5 }), [204; 5]);
        let noisy = Filter::AddNoise {
            amount: 50.0,
            gaussian: false,
            monochromatic: true,
        };
        let mut a = doc();
        apply(&mut a, noisy, [255; 3]).unwrap();
        let mut b = doc();
        apply(&mut b, noisy, [255; 3]).unwrap();
        // Repeatable, so previews and the final result agree
        assert_eq!(a.composite_rgba8(), b.composite_rgba8());
        // Monochromatic noise keeps pixels gray
        let px = &a.composite_rgba8()[4..8];
        assert_eq!((px[0], px[0]), (px[1], px[2]));
    }

    #[test]
    fn transparent_pixels_do_not_bleed_color() {
        // Red pixel on a transparent layer: the blur stays red, fading out
        let mut d = Document::new_with_background("t", 3, 1, Color::WHITE);
        let mut image = TiledImage::new(3, 1);
        image.set_pixel(1, 0, [255, 0, 0, 255]);
        d.insert_above_active(Layer::raster(d.new_layer_id(), "Layer 1", image));
        apply(&mut d, Filter::BoxBlur { radius: 1 }, [255; 3]).unwrap();
        let image = d.layers[1].image().unwrap();
        assert_eq!(image.pixel(0, 0), [255, 0, 0, 85]);
    }
}
