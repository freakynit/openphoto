//! Tool definitions. Each tool's behavior (pointer events → document edits)
//! will be implemented here; the UI only forwards events in canvas coordinates.

/// Toolbar tools, in the order of Photoshop's default toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Move,
    RectangularMarquee,
    Lasso,
    ObjectSelection,
    Crop,
    Frame,
    Eyedropper,
    SpotHealingBrush,
    Brush,
    CloneStamp,
    HistoryBrush,
    Eraser,
    PaintBucket,
    Blur,
    Dodge,
    Pen,
    HorizontalType,
    PathSelection,
    Rectangle,
    Hand,
    Zoom,
}

/// Toolbar order; the grouping determines the toolbar layout.
pub const TOOLBAR: &[Tool] = &[
    Tool::Move,
    Tool::RectangularMarquee,
    Tool::Lasso,
    Tool::ObjectSelection,
    Tool::Crop,
    Tool::Frame,
    Tool::Eyedropper,
    Tool::SpotHealingBrush,
    Tool::Brush,
    Tool::CloneStamp,
    Tool::HistoryBrush,
    Tool::Eraser,
    Tool::PaintBucket,
    Tool::Blur,
    Tool::Dodge,
    Tool::Pen,
    Tool::HorizontalType,
    Tool::PathSelection,
    Tool::Rectangle,
    Tool::Hand,
    Tool::Zoom,
];

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Move => "Move Tool",
            Self::RectangularMarquee => "Rectangular Marquee Tool",
            Self::Lasso => "Lasso Tool",
            Self::ObjectSelection => "Object Selection Tool",
            Self::Crop => "Crop Tool",
            Self::Frame => "Frame Tool",
            Self::Eyedropper => "Eyedropper Tool",
            Self::SpotHealingBrush => "Spot Healing Brush Tool",
            Self::Brush => "Brush Tool",
            Self::CloneStamp => "Clone Stamp Tool",
            Self::HistoryBrush => "History Brush Tool",
            Self::Eraser => "Eraser Tool",
            Self::PaintBucket => "Paint Bucket Tool",
            Self::Blur => "Blur Tool",
            Self::Dodge => "Dodge Tool",
            Self::Pen => "Pen Tool",
            Self::HorizontalType => "Horizontal Type Tool",
            Self::PathSelection => "Path Selection Tool",
            Self::Rectangle => "Rectangle Tool",
            Self::Hand => "Hand Tool",
            Self::Zoom => "Zoom Tool",
        }
    }

    /// Single-key shortcut (Shift-cycling within a tool group is not included).
    pub fn shortcut(self) -> Option<char> {
        Some(match self {
            Self::Move => 'V',
            Self::RectangularMarquee => 'M',
            Self::Lasso => 'L',
            Self::ObjectSelection => 'W',
            Self::Crop => 'C',
            Self::Frame => 'K',
            Self::Eyedropper => 'I',
            Self::SpotHealingBrush => 'J',
            Self::Brush => 'B',
            Self::CloneStamp => 'S',
            Self::HistoryBrush => 'Y',
            Self::Eraser => 'E',
            Self::PaintBucket => 'G',
            Self::Blur => return None,
            Self::Dodge => 'O',
            Self::Pen => 'P',
            Self::HorizontalType => 'T',
            Self::PathSelection => 'A',
            Self::Rectangle => 'U',
            Self::Hand => 'H',
            Self::Zoom => 'Z',
        })
    }

    pub fn from_shortcut(c: char) -> Option<Self> {
        let c = c.to_ascii_uppercase();
        TOOLBAR.iter().copied().find(|t| t.shortcut() == Some(c))
    }

    /// Whether to draw the small corner triangle indicating more tools in the group.
    pub fn has_group(self) -> bool {
        !matches!(self, Self::Hand | Self::Zoom | Self::Frame)
    }
}
