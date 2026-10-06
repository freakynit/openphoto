//! Document model: layer tree, tiled pixel storage and pixel formats.
//!
//! This crate has no GPU or UI dependencies; every edit ends up as a change to
//! the data structures defined here.

pub mod blend;
pub mod clipboard;
pub mod color;
pub mod document;
pub mod fill;
pub mod history;
pub mod image_ops;
pub mod layer;
pub mod layer_ops;
pub mod move_tool;
pub mod paint;
pub mod pixel;
pub mod selection;
pub mod tile;

pub use color::Color;
pub use document::{Anchor, DocId, Document, Snapshot};
pub use history::History;
pub use layer::{BlendMode, Layer, LayerId, LayerKind};
pub use pixel::{BitDepth, ColorMode};
pub use selection::{Selection, SelectionOp};
pub use tile::{TILE_SIZE, Tile, TiledImage};
