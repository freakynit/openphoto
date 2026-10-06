//! Modal dialogs.

mod canvas_size;
mod color_picker;
mod common;

pub use canvas_size::{CanvasSizeDialog, Outcome};
pub use color_picker::{ColorPicker, Outcome as ColorPickerOutcome};
