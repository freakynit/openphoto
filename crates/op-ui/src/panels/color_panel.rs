//! Color panel (hue strip + saturation/brightness field) and Swatches panel.

use egui::{Color32, Mesh, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use op_color::Hsb;
use op_core::Color;

use crate::state::AppState;
use crate::toolbar::swatch;

fn to_c32(c: Color) -> Color32 {
    let [r, g, b, _] = c.to_rgba8();
    Color32::from_rgb(r, g, b)
}

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let area = ui.max_rect().shrink2(Vec2::new(14.0, 14.0));

    // Re-sync the cached HSB when the color was changed elsewhere (eyedropper, swatches)
    let current = app.editing_color();
    if app.picker_hsb.to_color().to_rgba8() != current.to_rgba8() {
        app.picker_hsb = Hsb::from_color(current);
    }

    // Foreground/background swatches in the top-left corner
    let fg_rect = Rect::from_min_size(area.min, Vec2::splat(30.0));
    let bg_rect = fg_rect.translate(Vec2::splat(15.0));
    if ui
        .interact(bg_rect, ui.id().with("bg"), Sense::click())
        .clicked()
    {
        app.editing_background = true;
        app.picker_hsb = Hsb::from_color(app.background);
    }
    if ui
        .interact(fg_rect, ui.id().with("fg"), Sense::click())
        .clicked()
    {
        app.editing_background = false;
        app.picker_hsb = Hsb::from_color(app.foreground);
    }
    let painter = ui.painter();
    if app.editing_background {
        swatch(painter, fg_rect, app.foreground);
        swatch(painter, bg_rect, app.background);
    } else {
        swatch(painter, bg_rect, app.background);
        swatch(painter, fg_rect, app.foreground);
    }

    let field_h = (area.height()).max(40.0);
    let sv_rect = Rect::from_min_max(
        Pos2::new(area.left() + 54.0, area.top()),
        Pos2::new(area.right() - 56.0, area.top() + field_h),
    );
    let hue_rect = Rect::from_min_max(
        Pos2::new(area.right() - 28.0, area.top()),
        Pos2::new(area.right(), area.top() + field_h),
    );

    let mut hsb = app.picker_hsb;
    let sv = ui.interact(sv_rect, ui.id().with("sv"), Sense::click_and_drag());
    if let Some(p) = sv
        .interact_pointer_pos()
        .filter(|_| sv.is_pointer_button_down_on())
    {
        hsb.s = ((p.x - sv_rect.left()) / sv_rect.width()).clamp(0.0, 1.0);
        hsb.b = 1.0 - ((p.y - sv_rect.top()) / sv_rect.height()).clamp(0.0, 1.0);
    }
    let hue = ui.interact(
        hue_rect.expand2(Vec2::new(10.0, 0.0)),
        ui.id().with("hue"),
        Sense::click_and_drag(),
    );
    if let Some(p) = hue
        .interact_pointer_pos()
        .filter(|_| hue.is_pointer_button_down_on())
    {
        // 360° at the top, 0° at the bottom
        hsb.h = 360.0 * (1.0 - ((p.y - hue_rect.top()) / hue_rect.height()).clamp(0.0, 1.0));
    }
    if hsb != app.picker_hsb {
        app.picker_hsb = hsb;
        app.set_editing_color(hsb.to_color());
    }

    let painter = ui.painter();
    painter.add(sv_mesh(sv_rect, hsb.h));
    painter.add(hue_mesh(hue_rect));

    let marker = Pos2::new(
        sv_rect.left() + hsb.s * sv_rect.width(),
        sv_rect.top() + (1.0 - hsb.b) * sv_rect.height(),
    );
    let ring = if hsb.b > 0.6 && hsb.s < 0.5 {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    painter.circle_stroke(marker, 6.0, Stroke::new(1.5, ring));

    let y = hue_rect.top() + (1.0 - hsb.h / 360.0) * hue_rect.height();
    let tip = Pos2::new(hue_rect.left() - 2.0, y);
    painter.add(Shape::convex_polygon(
        vec![tip, tip + Vec2::new(-9.0, -6.0), tip + Vec2::new(-9.0, 6.0)],
        Color32::WHITE,
        Stroke::new(1.0, Color32::from_gray(0x30)),
    ));
}

/// Saturation/brightness field: the top row goes from white to the pure hue and
/// is scaled by brightness downwards. A subdivided grid keeps interpolation accurate.
fn sv_mesh(rect: Rect, hue: f32) -> Shape {
    const N: usize = 32;
    let mut mesh = Mesh::default();
    for row in 0..=N {
        let v = 1.0 - row as f32 / N as f32;
        for col in 0..=N {
            let s = col as f32 / N as f32;
            let c = Hsb { h: hue, s, b: v }.to_color();
            let pos = Pos2::new(
                rect.left() + s * rect.width(),
                rect.top() + (1.0 - v) * rect.height(),
            );
            mesh.colored_vertex(pos, to_c32(c));
        }
    }
    let w = (N + 1) as u32;
    for row in 0..N as u32 {
        for col in 0..N as u32 {
            let i = row * w + col;
            mesh.add_triangle(i, i + 1, i + w);
            mesh.add_triangle(i + 1, i + w + 1, i + w);
        }
    }
    Shape::mesh(mesh)
}

fn hue_mesh(rect: Rect) -> Shape {
    const N: usize = 36;
    let mut mesh = Mesh::default();
    for i in 0..=N {
        let t = i as f32 / N as f32;
        let c = to_c32(
            Hsb {
                h: 360.0 * (1.0 - t),
                s: 1.0,
                b: 1.0,
            }
            .to_color(),
        );
        let y = rect.top() + t * rect.height();
        mesh.colored_vertex(Pos2::new(rect.left(), y), c);
        mesh.colored_vertex(Pos2::new(rect.right(), y), c);
    }
    for i in 0..N as u32 {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    Shape::mesh(mesh)
}

const SWATCHES: &[u32] = &[
    0xffffff, 0xd9d9d9, 0xa6a6a6, 0x737373, 0x404040, 0x000000, 0xff0000, 0xffff00, 0x00ff00,
    0x00ffff, 0x0000ff, 0xff00ff, 0xf26c4f, 0xf68e55, 0xfbaf5c, 0xfff467, 0xacd372, 0x7cc576,
    0x3bb878, 0x1abbb4, 0x00bff3, 0x438cca, 0x5574b9, 0x605ca8, 0x855fa8, 0xa763a8, 0xf06eaa,
    0xf26d7d, 0xed1c24, 0xf26522, 0xf7941d, 0xfff200, 0x8dc73f, 0x39b54a, 0x00a651, 0x00a99d,
];

pub fn swatches(ui: &mut Ui, app: &mut AppState) {
    let area = ui.max_rect().shrink(14.0);
    let cell = 24.0;
    let gap = 4.0;
    let cols = ((area.width() + gap) / (cell + gap)).floor().max(1.0) as usize;
    for (i, &hex) in SWATCHES.iter().enumerate() {
        let (col, row) = (i % cols, i / cols);
        let rect = Rect::from_min_size(
            area.min + Vec2::new(col as f32 * (cell + gap), row as f32 * (cell + gap)),
            Vec2::splat(cell),
        );
        let c = Color::from_rgba8([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]);
        let r = ui.interact(rect, ui.id().with(("swatch", i)), Sense::click());
        ui.painter().rect_filled(rect, 1, to_c32(c));
        if r.hovered() {
            ui.painter().rect_stroke(
                rect,
                1,
                Stroke::new(1.0, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        }
        if r.clicked() {
            app.set_editing_color(c);
        }
    }
}
