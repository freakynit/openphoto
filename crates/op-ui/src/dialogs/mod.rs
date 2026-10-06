//! Modal dialogs.

mod canvas_size;
mod color_picker;
mod common;
mod fill;

pub use canvas_size::{CanvasSizeDialog, Outcome};
pub use color_picker::{ColorPicker, Outcome as ColorPickerOutcome};
pub use fill::{FillDialog, Outcome as FillOutcome};
