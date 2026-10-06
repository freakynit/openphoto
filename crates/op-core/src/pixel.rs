/// Document color mode (Image > Mode). Only RGB is implemented so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorMode {
    Bitmap,
    Grayscale,
    Indexed,
    #[default]
    Rgb,
    Cmyk,
    Lab,
    Multichannel,
}

impl ColorMode {
    pub const ALL: [Self; 7] = [
        Self::Bitmap,
        Self::Grayscale,
        Self::Indexed,
        Self::Rgb,
        Self::Cmyk,
        Self::Lab,
        Self::Multichannel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Bitmap => "Bitmap",
            Self::Grayscale => "Grayscale",
            Self::Indexed => "Indexed Color",
            Self::Rgb => "RGB Color",
            Self::Cmyk => "CMYK Color",
            Self::Lab => "Lab Color",
            Self::Multichannel => "Multichannel",
        }
    }

    /// Short form shown in the document tab, e.g. `RGB/8`.
    pub fn short(self) -> &'static str {
        match self {
            Self::Bitmap => "Bitmap",
            Self::Grayscale => "Gray",
            Self::Indexed => "Index",
            Self::Rgb => "RGB",
            Self::Cmyk => "CMYK",
            Self::Lab => "Lab",
            Self::Multichannel => "Multichannel",
        }
    }
}

/// Bits per channel. Pixel storage is 8-bit for now; 16/32-bit support will be
/// added to the tile layer via generics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BitDepth {
    #[default]
    U8,
    U16,
    F32,
}

impl BitDepth {
    pub const ALL: [Self; 3] = [Self::U8, Self::U16, Self::F32];

    pub fn bits(self) -> u32 {
        match self {
            Self::U8 => 8,
            Self::U16 => 16,
            Self::F32 => 32,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::U8 => "8 Bits/Channel",
            Self::U16 => "16 Bits/Channel",
            Self::F32 => "32 Bits/Channel",
        }
    }
}
