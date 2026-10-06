//! Top options bar; its contents depend on the current tool.

use egui::{Align, Layout, Sense, Ui, Vec2};
use op_tools::Tool;

use crate::icons;
use crate::state::{AppState, MarqueeStyle, SelectionMode};
use crate::theme::{self, color, size};
use crate::widgets;

pub fn show(ui: &mut Ui, app: &mut AppState) {
    let h = size::OPTIONS_BAR;
    ui.spacing_mut().item_spacing.x = 6.0;
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), h),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.add_space(14.0);
            widgets::icon_button(ui, icons::HOUSE, 36.0, false).on_hover_text("Home");
            widgets::vseparator(ui, 34.0);

            // Tool presets
            widgets::icon_button(ui, icons::tool(app.tool), 36.0, false)
                .on_hover_text("Tool Presets");
            widgets::icon(ui, icons::CARET_DOWN, 13.0, color::ICON);
            widgets::vseparator(ui, 34.0);

            if app.transforming() {
                transform_options(ui, app);
            } else {
                tool_options(ui, app);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
                if app.transforming() {
                    transform_buttons(ui, app);
                    return;
                }
                if app.tool == Tool::Crop {
                    crop_buttons(ui, app);
                    return;
                }
                // Avatar placeholder
                let (r, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::click());
                ui.painter().circle_filled(
                    r.center(),
                    14.0,
                    egui::Color32::from_rgb(0x4f, 0x8f, 0xd9),
                );
                ui.add_space(6.0);
                widgets::icon(ui, icons::CARET_DOWN, 13.0, color::ICON);
                widgets::icon_button(ui, icons::SIDEBAR, 34.0, false).on_hover_text("Workspace");
                widgets::icon_button(ui, icons::LIGHTBULB, 34.0, false).on_hover_text("Discover");
                widgets::icon_button(ui, icons::MAGNIFYING_GLASS, 34.0, false)
                    .on_hover_text("Search");
                widgets::icon_button(ui, icons::BELL, 34.0, false).on_hover_text("Notifications");
                widgets::icon_button(ui, icons::EXPORT, 34.0, false).on_hover_text("Share");
            });
        },
    );
}

/// The four combine-mode buttons of the selection tools.
fn mode_buttons(ui: &mut Ui, current: &mut SelectionMode) {
    for (mode, icon, tip) in [
        (SelectionMode::New, icons::SQUARE, "New selection"),
        (
            SelectionMode::Add,
            icons::SELECTION_PLUS,
            "Add to selection",
        ),
        (
            SelectionMode::Subtract,
            icons::SELECTION_SLASH,
            "Subtract from selection",
        ),
        (
            SelectionMode::Intersect,
            icons::SELECTION_INVERSE,
            "Intersect with selection",
        ),
    ] {
        if widgets::icon_button(ui, icon, 34.0, *current == mode)
            .on_hover_text(tip)
            .clicked()
        {
            *current = mode;
        }
    }
    widgets::vseparator(ui, 34.0);
}

fn select_and_mask_button(ui: &mut Ui) {
    let button = egui::Button::new(egui::RichText::new("Select and Mask...").font(theme::body()))
        .fill(color::BUTTON)
        .min_size(Vec2::new(0.0, 32.0));
    ui.add(button);
}

/// Marquee and Lasso tools: mode, Feather, Anti-alias; the marquees also
/// have Style with Width and Height.
fn tool_options(ui: &mut Ui, app: &mut AppState) {
    match app.tool {
        Tool::RectangularMarquee
        | Tool::EllipticalMarquee
        | Tool::SingleRowMarquee
        | Tool::SingleColumnMarquee
        | Tool::Lasso
        | Tool::PolygonalLasso
        | Tool::MagneticLasso => marquee_options(ui, app),
        Tool::Move => move_options(ui),
        Tool::Brush
        | Tool::Pencil
        | Tool::Eraser
        | Tool::Dodge
        | Tool::Burn
        | Tool::Sponge
        | Tool::Blur
        | Tool::Sharpen
        | Tool::CloneStamp
        | Tool::HistoryBrush => paint_options(ui, app),
        Tool::PaintBucket => bucket_options(ui, app),
        Tool::Eyedropper => eyedropper_options(ui, app),
        Tool::Gradient => gradient_options(ui, app),
        Tool::Crop => crop_options(ui, app),
        Tool::MagicWand => wand_options(ui, app),
        Tool::Hand | Tool::Zoom => view_options(ui, app),
        _ => {}
    }
}

/// Crop: the box's size and Clear (back to the whole canvas); Delete
/// Cropped Pixels is always on.
fn crop_options(ui: &mut Ui, app: &mut AppState) {
    let Some(state) = app.active() else {
        return;
    };
    let size = state.crop.map(|c| c.rect.size());
    ui.label("W:");
    let w = size.map_or(String::new(), |s| format!("{} px", s.x.round()));
    widgets::field(ui, &w, 76.0, true);
    widgets::icon(ui, icons::ARROWS_LEFT_RIGHT, 16.0, color::TEXT_DISABLED);
    ui.label("H:");
    let h = size.map_or(String::new(), |s| format!("{} px", s.y.round()));
    widgets::field(ui, &h, 76.0, true);
    ui.add_space(6.0);
    if ui.button("Clear").clicked() {
        state.crop = Some(crate::crop_tool::full(state));
    }
    widgets::vseparator(ui, 34.0);
    let mut delete = true;
    ui.add_enabled(
        false,
        egui::Checkbox::new(&mut delete, "Delete Cropped Pixels"),
    );
}

/// Crop's Cancel (reset the box) and Commit buttons.
fn crop_buttons(ui: &mut Ui, app: &mut AppState) {
    let Some(state) = app.active() else {
        return;
    };
    if widgets::icon_button(ui, icons::CHECK, 34.0, false)
        .on_hover_text("Commit current crop operation (Return)")
        .clicked()
    {
        crate::crop_tool::commit(state);
        return;
    }
    if widgets::icon_button(ui, icons::PROHIBIT, 34.0, false)
        .on_hover_text("Cancel current crop operation (Esc)")
        .clicked()
    {
        state.crop = Some(crate::crop_tool::full(state));
    }
}

/// Free Transform: the box's size (W, H in percent) and angle.
fn transform_options(ui: &mut Ui, app: &mut AppState) {
    let Some(t) = app.active().and_then(|s| s.free_transform.as_ref()) else {
        return;
    };
    let (sx, sy, angle) = (t.scale.0, t.scale.1, t.angle);
    let readout = |ui: &mut Ui, label: &str, value: String| {
        ui.label(label);
        widgets::field(ui, &value, 76.0, true);
        ui.add_space(6.0);
    };
    readout(ui, "W:", format!("{:.2}%", sx * 100.0));
    readout(ui, "H:", format!("{:.2}%", sy * 100.0));
    widgets::vseparator(ui, 34.0);
    widgets::icon(ui, icons::ANGLE, 16.0, color::ICON);
    readout(ui, "", format!("{:.2}°", angle.to_degrees()));
}

/// Free Transform's Cancel and Commit buttons (right-aligned).
fn transform_buttons(ui: &mut Ui, app: &mut AppState) {
    let Some(state) = app.active() else {
        return;
    };
    if widgets::icon_button(ui, icons::CHECK, 34.0, false)
        .on_hover_text("Commit transform (Return)")
        .clicked()
        && let Some(crate::free_transform::Outcome::Committed(m)) =
            crate::free_transform::commit(state)
    {
        app.last_transform = Some(m);
        return;
    }
    if widgets::icon_button(ui, icons::PROHIBIT, 34.0, false)
        .on_hover_text("Cancel transform (Esc)")
        .clicked()
    {
        crate::free_transform::cancel(state);
    }
}

fn marquee_options(ui: &mut Ui, app: &mut AppState) {
    let opts = &mut app.marquee;
    mode_buttons(ui, &mut opts.mode);

    ui.label("Feather:");
    ui.add_sized(
        [74.0, size::FIELD_HEIGHT],
        egui::DragValue::new(&mut opts.feather)
            .range(0.0..=1000.0)
            .speed(0.2)
            .custom_formatter(|v, _| format!("{}", (v * 10.0).round() / 10.0))
            .suffix(" px"),
    );
    ui.add_space(6.0);
    // Anti-alias only applies to curved edges, so it's disabled for the
    // rectangular and single row/column marquees, as in Photoshop
    ui.add_enabled(
        !matches!(
            app.tool,
            Tool::RectangularMarquee | Tool::SingleRowMarquee | Tool::SingleColumnMarquee
        ),
        egui::Checkbox::new(&mut opts.anti_alias, "Anti-alias"),
    );
    ui.add_space(6.0);
    if matches!(
        app.tool,
        Tool::Lasso | Tool::PolygonalLasso | Tool::MagneticLasso
    ) {
        select_and_mask_button(ui);
        return;
    }

    ui.label("Style:");
    egui::ComboBox::from_id_salt("marquee-style")
        .width(104.0)
        .selected_text(opts.style.label())
        .show_ui(ui, |ui| {
            for s in MarqueeStyle::ALL {
                ui.selectable_value(&mut opts.style, s, s.label());
            }
        });
    let fixed = opts.style != MarqueeStyle::Normal;
    ui.add_enabled_ui(fixed, |ui| {
        ui.label("Width:");
        widgets::field(ui, "", 58.0, fixed);
        widgets::icon(ui, icons::ARROWS_LEFT_RIGHT, 16.0, color::TEXT_DISABLED);
        ui.label("Height:");
        widgets::field(ui, "", 58.0, fixed);
    });
    ui.add_space(6.0);
    select_and_mask_button(ui);
}

/// Eyedropper: Sample Size, Sample, Show Sampling Ring.
fn eyedropper_options(ui: &mut Ui, app: &mut AppState) {
    use crate::state::EyedropperOptions;
    let opts = &mut app.eyedropper;
    ui.label("Sample Size:");
    let label = EyedropperOptions::SIZES
        .iter()
        .find(|(s, _)| *s == opts.size)
        .map_or("Point Sample", |(_, l)| l);
    egui::ComboBox::from_id_salt("eyedropper-size")
        .width(130.0)
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (size, label) in EyedropperOptions::SIZES {
                ui.selectable_value(&mut opts.size, size, label);
            }
        });
    ui.add_space(6.0);
    ui.label("Sample:");
    egui::ComboBox::from_id_salt("eyedropper-sample")
        .width(110.0)
        .selected_text(if opts.all_layers {
            "All Layers"
        } else {
            "Current Layer"
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut opts.all_layers, false, "Current Layer");
            ui.selectable_value(&mut opts.all_layers, true, "All Layers");
        });
    ui.add_space(6.0);
    let mut ring = true;
    ui.add_enabled(false, egui::Checkbox::new(&mut ring, "Show Sampling Ring"));
}

/// Gradient (classic): the gradient swatch (foreground to background), the
/// five kinds, Mode, Opacity and Reverse.
fn gradient_options(ui: &mut Ui, app: &mut AppState) {
    use op_core::gradient::GradientKind;
    // Swatch: the current two-color gradient
    let (r, _) = ui.allocate_exact_size(Vec2::new(110.0, 26.0), Sense::hover());
    let to32 = |c: op_core::Color| {
        let [r, g, b, _] = c.to_rgba8();
        egui::Color32::from_rgb(r, g, b)
    };
    let (start, end) = if app.gradient.reverse {
        (app.background, app.foreground)
    } else {
        (app.foreground, app.background)
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.left_top(), to32(start));
    mesh.colored_vertex(r.right_top(), to32(end));
    mesh.colored_vertex(r.right_bottom(), to32(end));
    mesh.colored_vertex(r.left_bottom(), to32(start));
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(egui::Shape::mesh(mesh));
    ui.painter().rect_stroke(
        r,
        0,
        egui::Stroke::new(1.0, color::SEPARATOR),
        egui::StrokeKind::Outside,
    );
    widgets::icon(ui, icons::CARET_DOWN, 13.0, color::ICON);
    widgets::vseparator(ui, 34.0);
    let opts = &mut app.gradient;
    for kind in GradientKind::ALL {
        let icon = match kind {
            GradientKind::Linear => icons::GRADIENT,
            GradientKind::Radial => icons::CIRCLE_HALF,
            GradientKind::Angle => icons::SPIRAL,
            GradientKind::Reflected => icons::ARROWS_IN_LINE_HORIZONTAL,
            GradientKind::Diamond => icons::DIAMOND,
        };
        if widgets::icon_button(ui, icon, 34.0, opts.kind == kind)
            .on_hover_text(kind.label())
            .clicked()
        {
            opts.kind = kind;
        }
    }
    widgets::vseparator(ui, 34.0);
    ui.label("Mode:");
    egui::ComboBox::from_id_salt("gradient-mode")
        .width(110.0)
        .selected_text(opts.mode.label())
        .show_ui(ui, |ui| {
            for (gi, group) in op_core::BlendMode::GROUPS.iter().enumerate() {
                if gi > 0 {
                    ui.separator();
                }
                for &m in *group {
                    ui.selectable_value(&mut opts.mode, m, m.label());
                }
            }
        });
    ui.add_space(6.0);
    ui.label("Opacity:");
    widgets::percent_drag(ui, &mut opts.opacity);
    ui.add_space(6.0);
    ui.checkbox(&mut opts.reverse, "Reverse");
}

/// Magic Wand: mode, Tolerance, Anti-alias, Contiguous, Sample All Layers.
fn wand_options(ui: &mut Ui, app: &mut AppState) {
    let opts = &mut app.wand;
    mode_buttons(ui, &mut opts.mode);
    ui.label("Tolerance:");
    ui.add_sized(
        [48.0, size::FIELD_HEIGHT],
        egui::DragValue::new(&mut opts.region.tolerance).range(0..=255),
    );
    ui.add_space(6.0);
    ui.checkbox(&mut opts.region.anti_alias, "Anti-alias");
    ui.checkbox(&mut opts.region.contiguous, "Contiguous");
    ui.checkbox(&mut opts.region.all_layers, "Sample All Layers");
    ui.add_space(6.0);
    select_and_mask_button(ui);
}

/// Brush, Pencil and Eraser: the brush preset picker (a dot with the size
/// below it; clicking opens Size and Hardness), Mode, Opacity and Flow.
fn paint_options(ui: &mut Ui, app: &mut AppState) {
    let tool = app.tool;
    let mut retouch = app.retouch;
    let Some(opts) = app.paint_options(tool) else {
        return;
    };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(44.0, size::OPTIONS_BAR), Sense::click());
    let dot = rect.center_top() + Vec2::new(0.0, size::OPTIONS_BAR * 0.36);
    ui.painter().circle_filled(dot, 9.0, color::TEXT);
    ui.painter().text(
        rect.center_bottom() - Vec2::new(0.0, 6.0),
        egui::Align2::CENTER_BOTTOM,
        format!("{:.0}", opts.size),
        theme::small(),
        color::TEXT,
    );
    widgets::icon(ui, icons::CARET_DOWN, 13.0, color::ICON);
    egui::Popup::from_response(&response)
        .open_memory(response.clicked().then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(240.0);
            ui.label("Size:");
            ui.add(
                egui::Slider::new(&mut opts.size, 1.0..=crate::state::PaintOptions::MAX_SIZE)
                    .logarithmic(true)
                    .max_decimals(0)
                    .suffix(" px"),
            );
            if tool != Tool::Pencil {
                ui.label("Hardness:");
                let mut pct = opts.hardness * 100.0;
                if ui
                    .add(
                        egui::Slider::new(&mut pct, 0.0..=100.0)
                            .max_decimals(0)
                            .suffix("%"),
                    )
                    .changed()
                {
                    opts.hardness = pct / 100.0;
                }
            }
        });
    widgets::vseparator(ui, 34.0);
    retouch_options(ui, tool, opts, &mut retouch);
    app.retouch = retouch;
}

/// The options after the brush picker, per tool, as in Photoshop. Disabled
/// controls are shown for what isn't implemented.
fn retouch_options(
    ui: &mut Ui,
    tool: Tool,
    opts: &mut crate::state::PaintOptions,
    retouch: &mut crate::state::RetouchOptions,
) {
    use op_core::paint::ToneRange;
    let disabled_combo = |ui: &mut Ui, id: &str, text: &str, width: f32| {
        ui.add_enabled_ui(false, |ui| {
            egui::ComboBox::from_id_salt(id)
                .width(width)
                .selected_text(text)
                .show_ui(ui, |_| {});
        });
    };
    let disabled_check = |ui: &mut Ui, label: &str, on: bool| {
        let mut on = on;
        ui.add_enabled(false, egui::Checkbox::new(&mut on, label));
    };
    match tool {
        Tool::Dodge | Tool::Burn => {
            let range = if tool == Tool::Dodge {
                &mut retouch.dodge_range
            } else {
                &mut retouch.burn_range
            };
            ui.label("Range:");
            egui::ComboBox::from_id_salt("tone-range")
                .width(100.0)
                .selected_text(range.label())
                .show_ui(ui, |ui| {
                    for r in ToneRange::ALL {
                        ui.selectable_value(range, r, r.label());
                    }
                });
            ui.add_space(8.0);
            ui.label("Exposure:");
            widgets::percent_drag(ui, &mut opts.opacity);
            ui.add_space(8.0);
            disabled_check(ui, "Protect Tones", true);
        }
        Tool::Sponge => {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("sponge-mode")
                .width(110.0)
                .selected_text(if retouch.sponge_saturate {
                    "Saturate"
                } else {
                    "Desaturate"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut retouch.sponge_saturate, false, "Desaturate");
                    ui.selectable_value(&mut retouch.sponge_saturate, true, "Saturate");
                });
            ui.add_space(8.0);
            ui.label("Flow:");
            widgets::percent_drag(ui, &mut opts.flow);
            ui.add_space(8.0);
            disabled_check(ui, "Vibrance", true);
        }
        Tool::Blur | Tool::Sharpen => {
            ui.label("Mode:");
            disabled_combo(ui, "retouch-mode", "Normal", 110.0);
            ui.add_space(8.0);
            ui.label("Strength:");
            widgets::percent_drag(ui, &mut opts.opacity);
            ui.add_space(8.0);
            disabled_check(ui, "Sample All Layers", false);
            if tool == Tool::Sharpen {
                disabled_check(ui, "Protect Detail", true);
            }
        }
        _ => {
            ui.label("Mode:");
            let mode = if tool == Tool::Eraser {
                "Brush"
            } else {
                "Normal"
            };
            disabled_combo(ui, "paint-mode", mode, 110.0);
            ui.add_space(8.0);
            ui.label("Opacity:");
            widgets::percent_drag(ui, &mut opts.opacity);
            if tool != Tool::Pencil {
                ui.add_space(8.0);
                ui.label("Flow:");
                widgets::percent_drag(ui, &mut opts.flow);
            }
            if tool == Tool::CloneStamp {
                ui.add_space(8.0);
                ui.checkbox(&mut retouch.clone_aligned, "Aligned");
                ui.label("Sample:");
                disabled_combo(ui, "clone-sample", "Current Layer", 110.0);
            }
        }
    }
}

/// Paint Bucket: Fill source, Mode, Opacity, Tolerance, Anti-alias,
/// Contiguous, All Layers.
fn bucket_options(ui: &mut Ui, app: &mut AppState) {
    let opts = &mut app.bucket;
    ui.label("Fill:");
    ui.add_enabled_ui(false, |ui| {
        egui::ComboBox::from_id_salt("bucket-fill")
            .width(100.0)
            .selected_text("Foreground")
            .show_ui(ui, |_| {});
    });
    ui.add_space(6.0);
    ui.label("Mode:");
    egui::ComboBox::from_id_salt("bucket-mode")
        .width(110.0)
        .selected_text(opts.fill.mode.label())
        .show_ui(ui, |ui| {
            for (gi, group) in op_core::BlendMode::GROUPS.iter().enumerate() {
                if gi > 0 {
                    ui.separator();
                }
                for &m in *group {
                    ui.selectable_value(&mut opts.fill.mode, m, m.label());
                }
            }
        });
    ui.add_space(6.0);
    ui.label("Opacity:");
    widgets::percent_drag(ui, &mut opts.fill.opacity);
    ui.add_space(6.0);
    ui.label("Tolerance:");
    ui.add_sized(
        [48.0, size::FIELD_HEIGHT],
        egui::DragValue::new(&mut opts.tolerance).range(0..=255),
    );
    ui.add_space(6.0);
    ui.checkbox(&mut opts.anti_alias, "Anti-alias");
    ui.checkbox(&mut opts.contiguous, "Contiguous");
    ui.checkbox(&mut opts.all_layers, "All Layers");
}

fn move_options(ui: &mut Ui) {
    let mut auto_select = false;
    let mut show_transform = false;
    ui.checkbox(&mut auto_select, "Auto-Select:");
    widgets::field(ui, "Layer", 80.0, true);
    ui.add_space(8.0);
    ui.checkbox(&mut show_transform, "Show Transform Controls");
}

fn view_options(ui: &mut Ui, app: &mut AppState) {
    let ppp = ui.ctx().pixels_per_point();
    let Some(doc) = app.active() else {
        return;
    };
    for (label, f) in [
        (
            "100%",
            crate::document_view::actual_pixels as fn(&mut _, f32),
        ),
        ("Fit Screen", crate::document_view::fit_on_screen),
    ] {
        let b = egui::Button::new(label)
            .fill(color::BUTTON)
            .min_size(Vec2::new(0.0, 30.0));
        if ui.add(b).clicked() {
            f(doc, ppp);
        }
    }
}
