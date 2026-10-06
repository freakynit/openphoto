//! GPU rendering. Currently draws the document composite on the canvas
//! (zoom, pan, checkerboard, pixel grid).
//!
//! Embedded in the UI through egui-wgpu paint callbacks, sharing egui's wgpu
//! device and render pass.
//! Layer compositing, blend modes and filters will move into compute/fragment
//! shaders here.

mod canvas;

pub use canvas::{CanvasImage, CanvasView, Shield, install, paint_callback};
