//! The shape tools (Rectangle, Ellipse, Triangle, Polygon, Line): a shape
//! filled with a color on a new layer, anti-aliased.

use crate::document::Document;
use crate::layer::{Layer, LayerId};
use crate::selection::{Rect, Selection};
use crate::tile::TiledImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
    Triangle,
    /// A regular polygon with this many sides, pointing up.
    Polygon(u32),
    Line,
}

impl ShapeKind {
    /// The base name of the layer a shape makes ("Rectangle 1", ...).
    pub fn layer_name(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Triangle => "Triangle",
            Self::Polygon(_) => "Polygon",
            Self::Line => "Line",
        }
    }
}

/// The corners of a regular polygon with `sides` sides inscribed in the
/// box, its first corner at the top center.
fn polygon_points((x0, y0, x1, y1): (f32, f32, f32, f32), sides: u32) -> Vec<(f32, f32)> {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (rx, ry) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    (0..sides.max(3))
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2
                + k as f32 * std::f32::consts::TAU / sides.max(3) as f32;
            (cx + rx * a.cos(), cy + ry * a.sin())
        })
        .collect()
}

/// The coverage of a shape on a `width` × `height` canvas. Box shapes fill
/// the rectangle between `a` and `b`; the Line runs from `a` to `b`,
/// `weight` pixels thick.
pub fn coverage(
    kind: ShapeKind,
    width: u32,
    height: u32,
    a: (f32, f32),
    b: (f32, f32),
    weight: f32,
) -> Selection {
    let bx = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
    match kind {
        ShapeKind::Rectangle => Selection::polygon(
            width,
            height,
            &[(bx.0, bx.1), (bx.2, bx.1), (bx.2, bx.3), (bx.0, bx.3)],
            true,
        ),
        ShapeKind::Ellipse => {
            Selection::ellipse(width, height, Rect::new(bx.0, bx.1, bx.2, bx.3), true)
        }
        ShapeKind::Triangle => {
            let points = [((bx.0 + bx.2) / 2.0, bx.1), (bx.2, bx.3), (bx.0, bx.3)];
            Selection::polygon(width, height, &points, true)
        }
        ShapeKind::Polygon(sides) => {
            Selection::polygon(width, height, &polygon_points(bx, sides), true)
        }
        ShapeKind::Line => {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len = (dx * dx + dy * dy).sqrt();
            if len == 0.0 {
                return Selection::polygon(width, height, &[], true);
            }
            // A rectangle along the line, `weight` wide
            let (nx, ny) = (-dy / len * weight / 2.0, dx / len * weight / 2.0);
            let points = [
                (a.0 + nx, a.1 + ny),
                (b.0 + nx, b.1 + ny),
                (b.0 - nx, b.1 - ny),
                (a.0 - nx, a.1 - ny),
            ];
            Selection::polygon(width, height, &points, true)
        }
    }
}

/// Adds a layer above the active one with the shape filled in `color`,
/// named after the shape and numbered like Photoshop ("Ellipse 2").
/// Returns `None` (and adds nothing) when the shape covers no pixel.
pub fn add_shape_layer(
    doc: &mut Document,
    kind: ShapeKind,
    a: (f32, f32),
    b: (f32, f32),
    weight: f32,
    color: [u8; 3],
) -> Option<LayerId> {
    let (w, h) = (doc.width, doc.height);
    let cov = coverage(kind, w, h, a, b, weight);
    if cov.is_empty() {
        return None;
    }
    let mut image = TiledImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = cov.get(x, y);
            if v > 0 {
                image.set_pixel(x, y, [color[0], color[1], color[2], v]);
            }
        }
    }
    let base = kind.layer_name();
    let n = doc
        .layers
        .iter()
        .filter_map(|l| l.name.strip_prefix(base)?.trim().parse::<u32>().ok())
        .max()
        .map_or(1, |m| m + 1);
    let id = doc.new_layer_id();
    doc.insert_above_active(Layer::raster(id, format!("{base} {n}"), image));
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::layer::LayerKind;

    #[test]
    fn shapes_cover_their_box() {
        let r = coverage(ShapeKind::Rectangle, 10, 10, (2.0, 2.0), (6.0, 5.0), 1.0);
        assert_eq!(r.bounds(), Some((2, 2, 6, 5)));
        assert_eq!(r.get(3, 3), 255);
        let e = coverage(ShapeKind::Ellipse, 10, 10, (0.0, 0.0), (10.0, 10.0), 1.0);
        assert_eq!(e.get(5, 5), 255);
        assert_eq!(e.get(0, 0), 0);
        let t = coverage(ShapeKind::Triangle, 10, 10, (0.0, 0.0), (10.0, 10.0), 1.0);
        assert_eq!((t.get(5, 8), t.get(1, 1)), (255, 0));
        let hex = coverage(ShapeKind::Polygon(6), 10, 10, (0.0, 0.0), (10.0, 10.0), 1.0);
        assert_eq!(hex.get(5, 5), 255);
        // A horizontal line 2 px thick along y = 5
        let l = coverage(ShapeKind::Line, 10, 10, (1.0, 5.0), (9.0, 5.0), 2.0);
        assert_eq!((l.get(5, 4), l.get(5, 5), l.get(5, 7)), (255, 255, 0));
    }

    #[test]
    fn shape_layers_are_numbered() {
        let mut doc = Document::new_with_background("t", 10, 10, Color::WHITE);
        add_shape_layer(
            &mut doc,
            ShapeKind::Rectangle,
            (1.0, 1.0),
            (4.0, 4.0),
            1.0,
            [255, 0, 0],
        );
        add_shape_layer(
            &mut doc,
            ShapeKind::Rectangle,
            (5.0, 5.0),
            (8.0, 8.0),
            1.0,
            [0, 0, 255],
        );
        let names: Vec<&str> = doc.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Background", "Rectangle 1", "Rectangle 2"]);
        let LayerKind::Raster(image) = &doc.layers[2].kind;
        assert_eq!(image.pixel(6, 6), [0, 0, 255, 255]);
        // A zero-size shape adds nothing
        assert!(
            add_shape_layer(
                &mut doc,
                ShapeKind::Ellipse,
                (3.0, 3.0),
                (3.0, 3.0),
                1.0,
                [0; 3]
            )
            .is_none()
        );
        assert_eq!(doc.layers.len(), 3);
    }
}
