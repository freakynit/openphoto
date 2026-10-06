//! Visual spec: colors, sizes and fonts, taken from Photoshop's default
//! medium-gray interface.
//!
//! Sizes are measured in reference-screenshot pixels and scaled by [`UI_SCALE`]
//! to match Photoshop's actual on-screen size.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Stroke};

pub mod color {
    use egui::Color32;

    const fn gray(v: u8) -> Color32 {
        Color32::from_rgb(v, v, v)
    }

    /// Background of panels, the toolbar and the options bar.
    pub const PANEL: Color32 = gray(0x53);
    /// Pasteboard around the canvas.
    pub const PASTEBOARD: Color32 = gray(0x28);
    /// Tab bar background (document and panel tabs).
    pub const TAB_BAR: Color32 = gray(0x42);
    pub const TAB_INACTIVE: Color32 = gray(0x4c);
    pub const TAB_ACTIVE: Color32 = PANEL;
    /// Text field background.
    pub const FIELD: Color32 = gray(0x45);
    pub const FIELD_BORDER: Color32 = gray(0x3a);
    /// Buttons (e.g. "Select and Mask...").
    pub const BUTTON: Color32 = gray(0x63);
    /// Selected tool in the toolbar.
    pub const TOOL_ACTIVE: Color32 = gray(0x38);
    pub const HOVER: Color32 = gray(0x5e);
    /// Layers panel list background and selected row.
    pub const LIST_BG: Color32 = gray(0x4d);
    pub const ROW_SELECTED: Color32 = gray(0x6b);
    /// Dark separator between sections.
    pub const SEPARATOR: Color32 = gray(0x39);
    pub const SEPARATOR_LIGHT: Color32 = gray(0x44);

    pub const TEXT: Color32 = gray(0xea);
    pub const TEXT_DIM: Color32 = gray(0xbc);
    pub const TEXT_DISABLED: Color32 = gray(0x7c);
    pub const ICON: Color32 = gray(0xd8);
    pub const ACCENT: Color32 = Color32::from_rgb(0x2c, 0x8b, 0xe8);
}

pub mod size {
    use super::pt;

    // Window frame, measured in Photoshop at 1:1 (points)
    /// Title bar including its 1 pt bottom line.
    pub const TITLE_BAR: f32 = pt(29.0);
    pub const OPTIONS_BAR: f32 = pt(33.0);
    /// Toolbar including its 3 pt dark right border.
    pub const TOOLBAR: f32 = pt(42.0);
    pub const STATUS_BAR: f32 = pt(16.0);
    /// Icon strip including the 3 pt border on its left and the divider on its right.
    pub const ICON_STRIP: f32 = pt(44.0);
    pub const PANEL_COLUMN: f32 = pt(321.0);

    // Measured on the reference screenshot (scaled by UI_SCALE)
    pub const TOOL_BUTTON: f32 = 38.0;
    pub const PANEL_TAB_BAR: f32 = 42.0;
    pub const FIELD_HEIGHT: f32 = 26.0;
    pub const LAYER_ROW: f32 = 60.0;
}

pub mod font {
    use super::pt;

    /// Matches the cap height of Photoshop's panel text (8 pt).
    pub const BODY: f32 = pt(11.5);
    pub const SMALL: f32 = pt(10.5);
    pub const ICON: f32 = 20.0;
}

/// Global UI scale (like Photoshop's UI Scaling preference).
/// Affects the UI only; canvas zoom is always in physical pixels.
pub const UI_SCALE: f32 = 0.675;

/// Converts a size measured in points on Photoshop's screen into egui units
/// (which [`UI_SCALE`] scales back down). Newer UI is measured directly in
/// Photoshop at 1:1, so it uses this instead of reference-screenshot pixels.
pub const fn pt(points: f32) -> f32 {
    points / UI_SCALE
}

/// Name of the semibold font family (panel tabs, headings).
pub const SEMIBOLD: &str = "semibold";

pub fn body() -> FontId {
    FontId::proportional(font::BODY)
}

pub fn small() -> FontId {
    FontId::proportional(font::SMALL)
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

pub fn icon(size: f32) -> FontId {
    FontId::proportional(size)
}

/// The bundled interface fonts, also the Type tool's fonts.
pub const SOURCE_SANS_REGULAR: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Regular.ttf");
pub const SOURCE_SANS_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/SourceSans3-Semibold.ttf");

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "source-sans".into(),
        FontData::from_static(SOURCE_SANS_REGULAR).into(),
    );
    fonts.font_data.insert(
        "source-sans-semibold".into(),
        FontData::from_static(SOURCE_SANS_SEMIBOLD).into(),
    );

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "source-sans".into());

    // The icon font is appended to Proportional as a fallback
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    let mut semibold = vec!["source-sans-semibold".to_owned()];
    semibold.extend(
        fonts.families[&FontFamily::Proportional]
            .iter()
            .skip(1)
            .cloned(),
    );
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), semibold);

    ctx.set_fonts(fonts);
}

pub fn apply_style(ctx: &egui::Context) {
    use egui::{CornerRadius, TextStyle};

    ctx.set_theme(egui::Theme::Dark);
    ctx.set_zoom_factor(UI_SCALE);
    ctx.options_mut(|o| {
        // Keep Cmd +/- for canvas zoom instead of scaling the whole UI
        o.zoom_with_keyboard = false;
    });

    ctx.style_mut_of(egui::Theme::Dark, |style| {
        style.text_styles = [
            (TextStyle::Heading, semibold(16.0)),
            (TextStyle::Body, body()),
            (TextStyle::Button, body()),
            (TextStyle::Small, small()),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into();

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style.spacing.interact_size.y = size::FIELD_HEIGHT;
        style.spacing.combo_height = 400.0;

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(color::TEXT);
        v.panel_fill = color::PANEL;
        v.window_fill = color::PANEL;
        v.window_stroke = Stroke::new(1.0, color::SEPARATOR);
        v.window_corner_radius = CornerRadius::same(4);
        v.menu_corner_radius = CornerRadius::same(4);
        v.extreme_bg_color = color::FIELD;
        v.text_edit_bg_color = Some(color::FIELD);
        v.faint_bg_color = color::LIST_BG;
        v.selection.bg_fill = color::ACCENT;
        v.selection.stroke = Stroke::new(1.0, color::TEXT);

        let r = CornerRadius::same(3);
        let w = &mut v.widgets;
        w.noninteractive.bg_fill = color::PANEL;
        w.noninteractive.weak_bg_fill = color::PANEL;
        w.noninteractive.bg_stroke = Stroke::new(1.0, color::SEPARATOR_LIGHT);
        w.noninteractive.fg_stroke = Stroke::new(1.0, color::TEXT);

        w.inactive.bg_fill = color::FIELD;
        w.inactive.weak_bg_fill = color::FIELD;
        w.inactive.bg_stroke = Stroke::new(1.0, color::FIELD_BORDER);
        w.inactive.fg_stroke = Stroke::new(1.0, color::ICON);
        w.inactive.corner_radius = r;

        w.hovered.bg_fill = color::HOVER;
        w.hovered.weak_bg_fill = color::HOVER;
        w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_gray(0x80));
        w.hovered.fg_stroke = Stroke::new(1.0, color::TEXT);
        w.hovered.corner_radius = r;
        w.hovered.expansion = 0.0;

        w.active.bg_fill = color::TOOL_ACTIVE;
        w.active.weak_bg_fill = color::TOOL_ACTIVE;
        w.active.bg_stroke = Stroke::new(1.0, Color32::from_gray(0x90));
        w.active.fg_stroke = Stroke::new(1.0, color::TEXT);
        w.active.corner_radius = r;
        w.active.expansion = 0.0;

        w.open = w.active;
    });
}
