//! Icons traced from Photoshop 2026's options bar at 2x and drawn as vector
//! shapes, so they match its pixels instead of approximating them with an
//! icon font. Coordinates are Photoshop device pixels at 2x (half points),
//! relative to the icon's center.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::theme::pt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Home,
    Move,
    Caret,
    Share,
    Bell,
    Search,
    Lightbulb,
    Workspace,
    AlignLeft,
    AlignHorizontalCenter,
    AlignRight,
    DistributeVertically,
    AlignTop,
    AlignVerticalCenter,
    AlignBottom,
    DistributeHorizontally,
    More,
    Gear,
    /// The toolbar's collapse "»": pixel-aligned and bold.
    CollapseToolbar,
    /// The panel column's thin collapse "»".
    CollapseRight,
    /// The icon strip's thin expand "«".
    CollapseLeft,
    /// The History panel's icon: three stacked squares and a curved arrow.
    History,
    /// The Comments panel's icon: a filled speech bubble.
    Comments,
}

struct Pen<'a> {
    painter: &'a Painter,
    center: Pos2,
    color: Color32,
}

impl Pen<'_> {
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.center + Vec2::new(pt(x / 2.0), pt(y / 2.0))
    }

    fn w(w: f32) -> f32 {
        pt(w / 2.0)
    }

    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.painter.rect_filled(
            Rect::from_min_max(self.p(x0, y0), self.p(x1, y1)),
            0,
            self.color,
        );
    }

    fn poly(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, self.color, Stroke::NONE));
    }

    /// A filled outline given as rows of (y, left, right), top to bottom.
    fn profile(&self, rows: &[(f32, f32, f32)]) {
        for pair in rows.windows(2) {
            let ((y0, l0, r0), (y1, l1, r1)) = (pair[0], pair[1]);
            self.poly(&[(l0, y0), (r0, y0), (r1, y1), (l1, y1)]);
        }
    }

    fn line(&self, points: &[(f32, f32)], width: f32) {
        let points = points.iter().map(|&(x, y)| self.p(x, y)).collect();
        self.painter
            .add(Shape::line(points, Stroke::new(Self::w(width), self.color)));
    }

    fn round_line(&self, a: (f32, f32), b: (f32, f32), width: f32) {
        self.line(&[a, b], width);
        for (x, y) in [a, b] {
            self.painter
                .circle_filled(self.p(x, y), Self::w(width) / 2.0, self.color);
        }
    }

    fn ring(&self, x: f32, y: f32, r: f32, width: f32) {
        self.painter.circle_stroke(
            self.p(x, y),
            Self::w(r),
            Stroke::new(Self::w(width), self.color),
        );
    }

    fn dot(&self, x: f32, y: f32, r: f32) {
        self.painter
            .circle_filled(self.p(x, y), Self::w(r), self.color);
    }
}

/// Paints `icon` centered on `center`. `background` is the color behind it,
/// used for the gear's hole.
pub fn paint(painter: &Painter, center: Pos2, icon: Icon, color: Color32, background: Color32) {
    let pen = Pen {
        painter,
        center,
        color,
    };
    match icon {
        Icon::Home => {
            pen.profile(&[(-14.5, 0.0, 0.0), (2.0, -16.5, 16.5), (4.5, -14.5, 14.5)]);
            pen.rect(-12.0, 2.0, -4.0, 14.5);
            pen.rect(4.0, 2.0, 12.0, 14.5);
        }
        Icon::Move => {
            pen.rect(-1.0, -9.0, 1.0, 9.0);
            pen.rect(-15.0, -1.0, 15.0, 1.0);
            pen.poly(&[(0.0, -15.0), (5.0, -9.0), (-5.0, -9.0)]);
            pen.poly(&[(0.0, 15.0), (-5.0, 9.0), (5.0, 9.0)]);
            pen.poly(&[(-15.5, 0.0), (-9.0, -5.0), (-9.0, 5.0)]);
            pen.poly(&[(15.5, 0.0), (9.0, 5.0), (9.0, -5.0)]);
        }
        Icon::Caret => pen.line(&[(-6.0, -3.0), (0.0, 3.0), (6.0, -3.0)], 2.2),
        Icon::Share => {
            // The tray, open at the top around the arrow
            pen.line(
                &[
                    (-8.0, -5.0),
                    (-14.0, -5.0),
                    (-14.0, 15.0),
                    (14.0, 15.0),
                    (14.0, -5.0),
                    (8.0, -5.0),
                ],
                4.0,
            );
            pen.rect(-2.0, -9.5, 2.0, 2.0);
            pen.poly(&[(0.0, -17.5), (8.0, -9.5), (-8.0, -9.5)]);
        }
        Icon::Bell => {
            pen.rect(-2.0, -17.0, 2.0, -13.0);
            pen.profile(&[
                (-13.0, -5.0, 5.0),
                (-10.0, -9.0, 9.0),
                (-5.0, -9.5, 9.5),
                (0.0, -10.0, 10.0),
                (4.0, -11.0, 11.0),
                (8.0, -14.0, 14.0),
                (11.0, -14.0, 14.0),
            ]);
            pen.profile(&[(13.0, -4.0, 4.0), (15.0, -3.0, 3.0), (16.5, -1.0, 1.0)]);
        }
        Icon::Search => {
            pen.ring(-2.5, -2.25, 10.5, 3.0);
            pen.round_line((5.0, 6.0), (12.0, 12.0), 4.5);
        }
        Icon::Lightbulb => {
            pen.ring(-0.5, -0.5, 7.5, 3.0);
            pen.line(&[(-4.5, 6.0), (-4.0, 10.0)], 3.0);
            pen.line(&[(3.5, 6.0), (3.0, 10.0)], 3.0);
            pen.profile(&[(10.0, -5.0, 4.0), (14.5, -5.0, 4.0), (16.5, -2.5, 1.5)]);
            pen.line(&[(-0.5, -16.5), (-0.5, -11.5)], 3.0);
            pen.line(&[(-12.0, -10.5), (-9.5, -8.0)], 3.0);
            pen.line(&[(11.0, -10.5), (8.5, -8.0)], 3.0);
            pen.line(&[(-17.0, 0.0), (-11.5, 0.0)], 3.0);
            pen.line(&[(10.5, 0.0), (16.0, 0.0)], 3.0);
        }
        Icon::Workspace => {
            pen.rect(-16.0, -13.0, 16.0, -9.0);
            pen.rect(-16.0, -9.0, -14.0, 13.0);
            pen.rect(14.0, -9.0, 16.0, 13.0);
            pen.rect(-16.0, 11.0, 16.0, 13.0);
            // The panel strip: three stacked cells
            for k in 0..4 {
                let y = -7.0 + 4.0 * k as f32;
                pen.rect(-12.0, y, -6.0, y + 2.0);
            }
            pen.rect(-12.0, -7.0, -10.0, 7.0);
            pen.rect(-8.0, -7.0, -6.0, 7.0);
        }
        Icon::AlignLeft => {
            pen.rect(-12.5, -15.0, -10.5, 15.0);
            pen.rect(-8.5, -9.0, 5.5, -1.0);
            pen.rect(-8.5, 3.0, 12.5, 11.0);
        }
        Icon::AlignHorizontalCenter => {
            pen.rect(-1.0, -15.0, 1.0, 15.0);
            pen.rect(-7.0, -10.0, 7.0, -2.0);
            pen.rect(-11.0, 2.0, 10.0, 10.0);
        }
        Icon::AlignRight => {
            pen.rect(10.5, -15.0, 12.5, 15.0);
            pen.rect(-5.5, -9.0, 8.5, -1.0);
            pen.rect(-12.5, 3.0, 8.5, 11.0);
        }
        Icon::DistributeVertically => {
            pen.rect(-16.0, -10.0, 16.0, -8.0);
            pen.rect(-10.0, -4.0, 10.0, 4.0);
            pen.rect(-16.0, 8.0, 16.0, 10.0);
        }
        Icon::AlignTop => {
            pen.rect(-14.0, -12.5, 14.0, -10.5);
            pen.rect(-10.0, -8.5, -2.0, 12.5);
            pen.rect(2.0, -8.5, 10.0, 5.5);
        }
        Icon::AlignVerticalCenter => {
            pen.rect(-14.0, -1.0, 14.0, 1.0);
            pen.rect(-10.0, -11.0, -2.0, 10.0);
            pen.rect(2.0, -7.0, 10.0, 7.0);
        }
        Icon::AlignBottom => {
            pen.rect(-14.0, 10.5, 14.0, 12.5);
            pen.rect(-10.0, -12.5, -2.0, 8.5);
            pen.rect(2.0, -5.5, 10.0, 8.5);
        }
        Icon::DistributeHorizontally => {
            pen.rect(-10.0, -16.0, -8.0, 16.0);
            pen.rect(8.0, -16.0, 10.0, 16.0);
            pen.rect(-4.0, -10.0, 4.0, 10.0);
        }
        Icon::More => {
            for x in [-12.0, 0.0, 12.0] {
                pen.dot(x, 0.0, 4.0);
            }
        }
        Icon::CollapseToolbar => {
            for left in [-6.5, 1.5] {
                let rows = [
                    (-5.0, 0.0, 1.0),
                    (-3.0, 0.0, 3.0),
                    (-1.0, 2.0, 5.0),
                    (1.0, 2.0, 5.0),
                    (3.0, 0.0, 3.0),
                    (5.0, 0.0, 1.0),
                ]
                .map(|(y, l, r)| (y, left + l, left + r));
                pen.profile(&rows);
            }
        }
        Icon::CollapseRight | Icon::CollapseLeft => {
            let flip = if icon == Icon::CollapseLeft {
                -1.0
            } else {
                1.0
            };
            for dx in [-8.0, 0.0] {
                pen.line(
                    &[
                        (flip * (dx + 2.5), -4.0),
                        (flip * (dx + 5.5), 0.0),
                        (flip * (dx + 2.5), 4.0),
                    ],
                    1.3,
                );
            }
        }
        Icon::History => {
            // Traced around (2013, 195): two outlined squares over a filled
            // one, and an arrow curving from the top right down to the left
            for y in [-17.0, -5.0] {
                pen.rect(-14.0, y, -4.0, y + 2.0);
                pen.rect(-14.0, y + 8.0, -4.0, y + 10.0);
                pen.rect(-14.0, y, -12.0, y + 10.0);
                pen.rect(-6.0, y, -4.0, y + 10.0);
            }
            pen.rect(-14.0, 7.0, -4.0, 17.0);
            let arc: Vec<(f32, f32)> = (0..=12)
                .map(|k| {
                    let a = (-40.0 + 135.0 * k as f32 / 12.0).to_radians();
                    (11.0 * a.cos(), 1.0 + 11.0 * a.sin())
                })
                .collect();
            pen.line(&arc, 4.0);
            pen.poly(&[(2.0, -14.0), (13.5, -14.0), (2.0, -2.5)]);
        }
        Icon::Comments => {
            // Traced around (2013, 251)
            pen.painter.rect_filled(
                Rect::from_min_max(pen.p(-15.0, -15.0), pen.p(15.0, 6.0)),
                pt(1.0),
                pen.color,
            );
            pen.poly(&[(-10.0, 5.0), (0.0, 5.0), (-9.5, 14.5)]);
        }
        Icon::Gear => {
            pen.dot(0.0, 0.0, 10.0);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (s, c) = a.sin_cos();
                let at = |r: f32, t: f32| (c * r - s * t, s * r + c * t);
                pen.poly(&[at(8.0, -2.5), at(13.0, -2.0), at(13.0, 2.0), at(8.0, 2.5)]);
            }
            Pen {
                color: background,
                ..pen
            }
            .dot(0.0, 0.0, 4.0);
            Pen {
                color: Color32::WHITE,
                ..pen
            }
            .poly(&[(10.0, 15.5), (18.0, 15.5), (14.0, 19.5)]);
        }
    }
}
