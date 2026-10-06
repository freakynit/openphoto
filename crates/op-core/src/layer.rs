use crate::tile::TiledImage;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(pub u64);

/// All Photoshop layer blend modes, ordered and grouped as in the blend mode menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Dissolve,

    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,

    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    LighterColor,

    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,

    Difference,
    Exclusion,
    Subtract,
    Divide,

    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    /// Menu groups; a separator is drawn between groups.
    pub const GROUPS: &[&[Self]] = &[
        &[Self::Normal, Self::Dissolve],
        &[
            Self::Darken,
            Self::Multiply,
            Self::ColorBurn,
            Self::LinearBurn,
            Self::DarkerColor,
        ],
        &[
            Self::Lighten,
            Self::Screen,
            Self::ColorDodge,
            Self::LinearDodge,
            Self::LighterColor,
        ],
        &[
            Self::Overlay,
            Self::SoftLight,
            Self::HardLight,
            Self::VividLight,
            Self::LinearLight,
            Self::PinLight,
            Self::HardMix,
        ],
        &[
            Self::Difference,
            Self::Exclusion,
            Self::Subtract,
            Self::Divide,
        ],
        &[Self::Hue, Self::Saturation, Self::Color, Self::Luminosity],
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Dissolve => "Dissolve",
            Self::Darken => "Darken",
            Self::Multiply => "Multiply",
            Self::ColorBurn => "Color Burn",
            Self::LinearBurn => "Linear Burn",
            Self::DarkerColor => "Darker Color",
            Self::Lighten => "Lighten",
            Self::Screen => "Screen",
            Self::ColorDodge => "Color Dodge",
            Self::LinearDodge => "Linear Dodge (Add)",
            Self::LighterColor => "Lighter Color",
            Self::Overlay => "Overlay",
            Self::SoftLight => "Soft Light",
            Self::HardLight => "Hard Light",
            Self::VividLight => "Vivid Light",
            Self::LinearLight => "Linear Light",
            Self::PinLight => "Pin Light",
            Self::HardMix => "Hard Mix",
            Self::Difference => "Difference",
            Self::Exclusion => "Exclusion",
            Self::Subtract => "Subtract",
            Self::Divide => "Divide",
            Self::Hue => "Hue",
            Self::Saturation => "Saturation",
            Self::Color => "Color",
            Self::Luminosity => "Luminosity",
        }
    }
}

#[derive(Clone)]
pub enum LayerKind {
    Raster(TiledImage),
}

#[derive(Clone)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub visible: bool,
    /// Layer opacity, applied to the whole layer including layer styles.
    pub opacity: f32,
    /// Fill opacity, applied to the pixels only (not to layer styles).
    pub fill: f32,
    pub blend_mode: BlendMode,
    /// The "Background" layer: locked, opaque and always at the bottom.
    pub is_background: bool,
    pub lock_transparency: bool,
    pub lock_pixels: bool,
    pub lock_position: bool,
    pub kind: LayerKind,
}

impl Layer {
    pub fn raster(id: LayerId, name: impl Into<String>, image: TiledImage) -> Self {
        Self {
            id,
            name: name.into(),
            visible: true,
            opacity: 1.0,
            fill: 1.0,
            blend_mode: BlendMode::Normal,
            is_background: false,
            lock_transparency: false,
            lock_pixels: false,
            lock_position: false,
            kind: LayerKind::Raster(image),
        }
    }

    pub fn is_locked(&self) -> bool {
        self.is_background || self.lock_pixels || self.lock_position
    }
}
