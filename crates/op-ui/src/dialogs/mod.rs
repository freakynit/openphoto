//! Modal dialogs.

mod canvas_size;
mod color_picker;
mod common;
mod fill;
mod trim;

pub use canvas_size::{CanvasSizeDialog, Outcome};
pub use color_picker::{ColorPicker, Outcome as ColorPickerOutcome};
pub use fill::{FillDialog, Outcome as FillOutcome};
pub use trim::{Outcome as TrimOutcome, TrimDialog};
