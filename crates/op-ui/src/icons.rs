//! Icon mapping. Uses Phosphor icons (MIT), not Adobe's icon assets.

pub use egui_phosphor::regular::*;

use op_tools::Tool;

pub fn tool(tool: Tool) -> &'static str {
    match tool {
        Tool::Move => ARROWS_OUT_CARDINAL,
        Tool::RectangularMarquee => SELECTION,
        Tool::Lasso => LASSO,
        Tool::ObjectSelection => SELECTION_PLUS,
        Tool::Crop => CROP,
        Tool::Frame => FRAME_CORNERS,
        Tool::Eyedropper => EYEDROPPER,
        Tool::SpotHealingBrush => BANDAIDS,
        Tool::Brush => PAINT_BRUSH,
        Tool::CloneStamp => STAMP,
        Tool::HistoryBrush => PAINT_BRUSH_BROAD,
        Tool::Eraser => ERASER,
        Tool::PaintBucket => PAINT_BUCKET,
        Tool::Blur => DROP,
        Tool::Dodge => DROP_HALF,
        Tool::Pen => PEN_NIB,
        Tool::HorizontalType => TEXT_T,
        Tool::PathSelection => NAVIGATION_ARROW,
        Tool::Rectangle => RECTANGLE,
        Tool::Hand => HAND,
        Tool::Zoom => MAGNIFYING_GLASS,
    }
}
