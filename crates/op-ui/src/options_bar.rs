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

            match app.tool {
                Tool::RectangularMarquee
                | Tool::EllipticalMarquee
                | Tool::SingleRowMarquee
                | Tool::SingleColumnMarquee
                | Tool::Lasso
                | Tool::PolygonalLasso
                | Tool::MagneticLasso => marquee_options(ui, app),
                Tool::Move => move_options(ui),
                Tool::Hand | Tool::Zoom => view_options(ui, app),
                _ => {}
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(12.0);
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

fn marquee_options(ui: &mut Ui, app: &mut AppState) {
    let opts = &mut app.marquee;
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
        if widgets::icon_button(ui, icon, 34.0, opts.mode == mode)
            .on_hover_text(tip)
            .clicked()
        {
            opts.mode = mode;
        }
    }
    widgets::vseparator(ui, 34.0);

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
    let button = egui::Button::new(egui::RichText::new("Select and Mask...").font(theme::body()))
        .fill(color::BUTTON)
        .min_size(Vec2::new(0.0, 32.0));
    ui.add(button);
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
