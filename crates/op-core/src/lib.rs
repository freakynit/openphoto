//! Document model: layer tree, tiled pixel storage and pixel formats.
//!
//! This crate has no GPU or UI dependencies; every edit ends up as a change to
//! the data structures defined here.

pub mod blend;
pub mod color;
pub mod document;
pub mod history;
pub mod layer;
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
