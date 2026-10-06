//! Modal dialogs.

mod adjust;
pub mod alert;
mod canvas_size;
mod color_picker;
mod common;
mod duplicate_layer;
mod fill;
mod image_size;
mod modify_selection;
mod new_document;
mod new_guide;
mod new_layer;
pub mod save_changes;
mod trim;

pub use adjust::{AdjustDialog, Effect, Kind as AdjustKind, Outcome as AdjustOutcome};
pub use canvas_size::{CanvasSizeDialog, Outcome};
pub use color_picker::{ColorPicker, Outcome as ColorPickerOutcome};
pub use duplicate_layer::{Destination, DuplicateLayerDialog, Outcome as DuplicateOutcome};
pub use fill::{FillDialog, Outcome as FillOutcome};
pub use image_size::{ImageSizeDialog, Outcome as ImageSizeOutcome};
pub use modify_selection::{ModifyDialog, ModifyKind, Outcome as ModifyOutcome};
pub use new_document::{Contents as NewContents, NewDocumentDialog, Outcome as NewDocumentOutcome};
pub use new_guide::{NewGuideDialog, Outcome as NewGuideOutcome};
pub use new_layer::{Kind as NewLayerKind, NewLayer, NewLayerDialog, Outcome as NewLayerOutcome};
pub use save_changes::SaveChoice;
pub use trim::{Outcome as TrimOutcome, TrimDialog};
