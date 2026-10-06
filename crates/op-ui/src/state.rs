use std::collections::HashMap;
use std::sync::Arc;

use op_color::Hsb;
use op_core::{Color, DocId, Document, History, LayerId};
use op_render::CanvasImage;
use op_tools::Tool;

/// How a document is shown in its viewport.
#[derive(Clone, Copy, Debug)]
pub struct View {
    /// Physical pixels per document pixel; 1.0 = 100%.
    pub zoom: f32,
    /// Offset of the document center from the viewport center, in points.
    /// Keeps the document centered when the window is resized.
    pub offset: egui::Vec2,
    /// The viewport size is unknown on first show; the initial zoom is picked once it is.
    pub initialized: bool,
    /// Viewport of the last frame, in points; used by keyboard zoom.
    pub viewport: egui::Rect,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            offset: egui::Vec2::ZERO,
            initialized: false,
            viewport: egui::Rect::NOTHING,
        }
    }
}

pub struct DocState {
    pub doc: Document,
    pub history: History,
    pub view: View,
    /// Set by continuous edits (e.g. dragging opacity) that haven't been
    /// recorded in the history yet.
    pending_edit: bool,
    canvas: Option<Arc<CanvasImage>>,
    thumbs: HashMap<LayerId, (u64, egui::TextureHandle)>,
}

impl DocState {
    /// `initial` names the first history state, e.g. "Open" or "New".
    pub fn new(doc: Document, initial: &str) -> Self {
        Self {
            history: History::new(&doc, initial),
            doc,
            view: View::default(),
            pending_edit: false,
            canvas: None,
            thumbs: HashMap::new(),
        }
    }

    /// Records the current document as a new history state.
    pub fn record(&mut self, name: &str) {
        self.history.record(&self.doc, name);
        self.pending_edit = false;
    }

    /// Marks an in-progress edit that will be recorded by [`Self::commit_pending`].
    pub fn mark_pending(&mut self) {
        self.pending_edit = true;
    }

    /// Records a pending continuous edit, if there is one.
    pub fn commit_pending(&mut self, name: &str) {
        if self.pending_edit {
            self.record(name);
        }
    }

    pub fn undo(&mut self) -> bool {
        self.pending_edit = false;
        self.history.undo(&mut self.doc)
    }

    pub fn redo(&mut self) -> bool {
        self.pending_edit = false;
        self.history.redo(&mut self.doc)
    }

    pub fn jump_to_state(&mut self, index: usize) -> bool {
        self.pending_edit = false;
        self.history.jump(index, &mut self.doc)
    }

    pub fn delete_states_from(&mut self, index: usize) -> bool {
        self.pending_edit = false;
        self.history.delete_from(index, &mut self.doc)
    }

    /// The current composite, recomputed when the document changes.
    pub fn canvas_image(&mut self) -> Arc<CanvasImage> {
        let rev = self.doc.revision();
        match &self.canvas {
            Some(img) if img.revision == rev => img.clone(),
            _ => {
                let img = Arc::new(CanvasImage {
                    key: self.doc.id.0,
                    revision: rev,
                    width: self.doc.width,
                    height: self.doc.height,
                    pixels: self.doc.composite_rgba8(),
                });
                self.canvas = Some(img.clone());
                img
            }
        }
    }

    /// Color of a composite pixel (for the eyedropper).
    pub fn sample(&mut self, x: u32, y: u32) -> Option<Color> {
        if x >= self.doc.width || y >= self.doc.height {
            return None;
        }
        let img = self.canvas_image();
        let i = ((y * img.width + x) * 4) as usize;
        Some(Color::from_rgba8(img.pixels[i..i + 4].try_into().ok()?))
    }

    /// Layer thumbnail, cached per document revision.
    pub fn layer_thumbnail(
        &mut self,
        ctx: &egui::Context,
        layer: LayerId,
        max_px: u32,
    ) -> Option<egui::TextureHandle> {
        let rev = self.doc.revision();
        if let Some((r, tex)) = self.thumbs.get(&layer)
            && *r == rev
        {
            return Some(tex.clone());
        }
        let l = self.doc.layer(layer)?;
        let op_core::LayerKind::Raster(img) = &l.kind;
        let scale = (max_px as f32 / img.width().max(img.height()) as f32).min(1.0);
        let tw = ((img.width() as f32 * scale).round() as u32).max(1);
        let th = ((img.height() as f32 * scale).round() as u32).max(1);
        let mut pixels = Vec::with_capacity((tw * th) as usize);
        for y in 0..th {
            for x in 0..tw {
                let sx = ((x as f32 + 0.5) / scale) as u32;
                let sy = ((y as f32 + 0.5) / scale) as u32;
                let [r, g, b, a] = img.pixel(sx.min(img.width() - 1), sy.min(img.height() - 1));
                pixels.push(egui::Color32::from_rgba_unmultiplied(r, g, b, a));
            }
        }
        let image = egui::ColorImage::new([tw as usize, th as usize], pixels);
        let tex = ctx.load_texture(
            format!("thumb-{}-{}", self.doc.id.0, layer.0),
            image,
            egui::TextureOptions::LINEAR,
        );
        self.thumbs.insert(layer, (rev, tex.clone()));
        Some(tex)
    }
}

/// Selection combine mode (the four buttons in the options bar).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectionMode {
    #[default]
    New,
    Add,
    Subtract,
    Intersect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MarqueeStyle {
    #[default]
    Normal,
    FixedRatio,
    FixedSize,
}

impl MarqueeStyle {
    pub const ALL: [Self; 3] = [Self::Normal, Self::FixedRatio, Self::FixedSize];

    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::FixedRatio => "Fixed Ratio",
            Self::FixedSize => "Fixed Size",
        }
    }
}

#[derive(Default)]
pub struct MarqueeOptions {
    pub mode: SelectionMode,
    pub feather: f32,
    pub anti_alias: bool,
    pub style: MarqueeStyle,
}

pub struct AppState {
    pub docs: HashMap<DocId, DocState>,
    pub active_doc: Option<DocId>,
    pub tool: Tool,
    pub foreground: Color,
    pub background: Color,
    pub marquee: MarqueeOptions,
    /// Whether the Color panel edits the background or the foreground color.
    pub editing_background: bool,
    /// Cached HSB so the hue doesn't snap back to 0 for grays.
    pub picker_hsb: Hsb,
    pub untitled_counter: u32,
    /// Error message to show to the user.
    pub alert: Option<String>,
    /// Whether the History panel is popped out from the icon strip.
    pub history_open: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let foreground = Color::from_rgba8([0x14, 0xa5, 0xdc, 0xff]);
        Self {
            docs: HashMap::new(),
            active_doc: None,
            tool: Tool::RectangularMarquee,
            foreground,
            background: Color::WHITE,
            marquee: MarqueeOptions::default(),
            editing_background: false,
            picker_hsb: Hsb::from_color(foreground),
            untitled_counter: 0,
            alert: None,
            history_open: false,
        }
    }
}

impl AppState {
    pub fn active(&mut self) -> Option<&mut DocState> {
        self.docs.get_mut(&self.active_doc?)
    }

    pub fn editing_color(&self) -> Color {
        if self.editing_background {
            self.background
        } else {
            self.foreground
        }
    }

    pub fn set_editing_color(&mut self, c: Color) {
        if self.editing_background {
            self.background = c;
        } else {
            self.foreground = c;
        }
    }
}
