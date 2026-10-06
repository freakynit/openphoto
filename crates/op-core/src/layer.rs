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

/// A layer mask: white shows the layer, black hides it, grays partly.
/// Stored as an opaque gray image (the value is in every color channel), so
/// a plain white or black mask shares one tile across the whole canvas.
#[derive(Clone)]
pub struct LayerMask {
    pub image: TiledImage,
    /// Layer > Layer Mask > Disable: a disabled mask is kept but ignored.
    pub enabled: bool,
}

impl LayerMask {
    /// A mask of one value: 255 reveals everything, 0 hides everything.
    pub fn filled(width: u32, height: u32, value: u8) -> Self {
        Self {
            image: TiledImage::filled(width, height, [value, value, value, 255]),
            enabled: true,
        }
    }

    /// A mask from per-pixel values (a selection's mask), row by row.
    pub fn from_values(width: u32, height: u32, values: impl Fn(u32, u32) -> u8) -> Self {
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let v = values(x, y);
                pixels.extend_from_slice(&[v, v, v, 255]);
            }
        }
        Self {
            image: TiledImage::from_rgba8(width, height, &pixels),
            enabled: true,
        }
    }

    /// The mask's value at a pixel (0 outside the image).
    pub fn value(&self, x: u32, y: u32) -> u8 {
        self.image.pixel(x, y)[0]
    }
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
    pub mask: Option<LayerMask>,
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
            mask: None,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.is_background || self.lock_pixels || self.lock_position
    }
}
