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
    /// The file the document was opened from or last saved to.
    pub path: Option<std::path::PathBuf>,
    /// History state that matches the file on disk (or the new document);
    /// any other current state means there are unsaved changes.
    saved_state: u64,
    /// The document has no embedded color profile (shown as "#" in its tab).
    /// Opened files are untagged because profiles aren't read; new documents
    /// are sRGB.
    pub untagged: bool,
    pub history: History,
    pub view: View,
    /// Set by continuous edits (e.g. dragging opacity) that haven't been
    /// recorded in the history yet.
    pending_edit: bool,
    /// A marquee being dragged on the canvas.
    pub marquee_drag: Option<MarqueeDrag>,
    /// A Move tool drag: the move and where it started (document pixels).
    pub move_drag: Option<(op_core::move_tool::Move, egui::Pos2)>,
    /// The paint stroke in progress, and the tool painting it.
    pub stroke: Option<(op_core::paint::Stroke, Tool)>,
    /// Where the last stroke ended; Shift-click draws a line from here.
    pub last_paint_point: Option<(f32, f32)>,
    /// A lasso outline being drawn.
    pub lasso: Option<LassoPath>,
    /// The Crop tool's box, while the Crop tool is in use.
    pub crop: Option<CropBox>,
    /// Edit > Free Transform, while in progress.
    pub free_transform: Option<FreeTransform>,
    /// A Gradient tool drag: start and current point, in document pixels.
    pub gradient_drag: Option<(egui::Pos2, egui::Pos2)>,
    /// A layer name being edited in the Layers panel, and the text so far.
    pub renaming: Option<(LayerId, String)>,
    /// Marching-ants outline of the selection, cached per selection revision.
    outline: Option<(u64, Arc<Vec<[u32; 4]>>)>,
    canvas: Option<Arc<CanvasImage>>,
    thumbs: HashMap<LayerId, (u64, egui::TextureHandle)>,
    /// Thumbnail of the document as opened, for the History panel's snapshot
    /// row. Kept as pixels until a texture can be created.
    snapshot_thumb: Option<egui::ColorImage>,
    snapshot_texture: Option<egui::TextureHandle>,
}

impl DocState {
    /// `initial` names the first history state, e.g. "Open" or "New".
    pub fn new(doc: Document, initial: &str) -> Self {
        let snapshot_thumb = Some(composite_thumbnail(&doc, SNAPSHOT_THUMB_PX));
        let history = History::new(&doc, initial);
        Self {
            untagged: initial == "Open",
            path: None,
            saved_state: history.current_id(),
            history,
            snapshot_thumb,
            snapshot_texture: None,
            doc,
            view: View::default(),
            pending_edit: false,
            marquee_drag: None,
            move_drag: None,
            stroke: None,
            last_paint_point: None,
            renaming: None,
            lasso: None,
            free_transform: None,
            crop: None,
            gradient_drag: None,
            outline: None,
            canvas: None,
            thumbs: HashMap::new(),
        }
    }

    /// Whether the document differs from its file (or, for a new document,
    /// from how it was created). Undoing back to the saved state counts as
    /// unchanged, as in Photoshop.
    pub fn is_dirty(&self) -> bool {
        self.history.current_id() != self.saved_state
    }

    /// The document now matches its file.
    pub fn mark_saved(&mut self) {
        self.saved_state = self.history.current_id();
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

    pub fn toggle_last_state(&mut self) -> bool {
        self.pending_edit = false;
        self.history.toggle_last_state(&mut self.doc)
    }

    pub fn jump_to_state(&mut self, index: usize) -> bool {
        self.pending_edit = false;
        self.history.jump(index, &mut self.doc)
    }

    pub fn delete_states_from(&mut self, index: usize) -> bool {
        self.pending_edit = false;
        self.history.delete_from(index, &mut self.doc)
    }

    /// Outline of the current selection (see `Selection::outline`), cached.
    pub fn selection_outline(&mut self) -> Option<Arc<Vec<[u32; 4]>>> {
        let selection = self.doc.selection()?;
        let rev = self.doc.selection_revision();
        if let Some((r, segs)) = &self.outline
            && *r == rev
        {
            return Some(segs.clone());
        }
        let segs = Arc::new(selection.outline());
        self.outline = Some((rev, segs.clone()));
        Some(segs)
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
    /// The Eyedropper's color at (`x`, `y`): the average of a `size` ×
    /// `size` square (clipped to the canvas) of the merged image or, without
    /// `all_layers`, of the active layer. Averaged with alpha weighting, and
    /// opaque; `None` outside the canvas or where everything sampled is
    /// transparent.
    pub fn sample_average(&mut self, x: u32, y: u32, size: u32, all_layers: bool) -> Option<Color> {
        let (w, h) = (self.doc.width, self.doc.height);
        if x >= w || y >= h {
            return None;
        }
        let merged = all_layers.then(|| self.canvas_image());
        let layer = if all_layers {
            None
        } else {
            let id = self.doc.active_layer?;
            let op_core::LayerKind::Raster(image) = &self.doc.layer(id)?.kind;
            Some(image)
        };
        let half = (size / 2) as i64;
        let mut sum = [0u64; 3];
        let mut alpha = 0u64;
        for sy in (y as i64 - half).max(0)..=(y as i64 + half).min(h as i64 - 1) {
            for sx in (x as i64 - half).max(0)..=(x as i64 + half).min(w as i64 - 1) {
                let (sx, sy) = (sx as u32, sy as u32);
                let px: [u8; 4] = match (&merged, layer) {
                    (Some(img), _) => {
                        let i = ((sy * img.width + sx) * 4) as usize;
                        img.pixels[i..i + 4].try_into().ok()?
                    }
                    (None, Some(image)) => image.pixel(sx, sy),
                    (None, None) => return None,
                };
                for c in 0..3 {
                    sum[c] += px[c] as u64 * px[3] as u64;
                }
                alpha += px[3] as u64;
            }
        }
        if alpha == 0 {
            return None;
        }
        // Picked colors are always opaque
        let [r, g, b] = sum.map(|v| ((v + alpha / 2) / alpha) as u8);
        Some(Color::from_rgba8([r, g, b, 255]))
    }

    /// The merged image's pixel at (`x`, `y`).
    #[cfg(test)]
    pub fn sample(&mut self, x: u32, y: u32) -> Option<Color> {
        if x >= self.doc.width || y >= self.doc.height {
            return None;
        }
        let img = self.canvas_image();
        let i = ((y * img.width + x) * 4) as usize;
        Some(Color::from_rgba8(img.pixels[i..i + 4].try_into().ok()?))
    }

    /// Thumbnail of the document as it was opened (History panel snapshot).
    pub fn snapshot_thumbnail(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if let Some(image) = self.snapshot_thumb.take() {
            let name = format!("snapshot-{}", self.doc.id.0);
            self.snapshot_texture =
                Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
        }
        self.snapshot_texture.clone()
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

const SNAPSHOT_THUMB_PX: u32 = 96;

/// Downscaled composite of the whole document (nearest-neighbor).
fn composite_thumbnail(doc: &Document, max_px: u32) -> egui::ColorImage {
    let pixels = doc.composite_rgba8();
    let scale = (max_px as f32 / doc.width.max(doc.height) as f32).min(1.0);
    let tw = ((doc.width as f32 * scale).round() as u32).max(1);
    let th = ((doc.height as f32 * scale).round() as u32).max(1);
    let mut out = Vec::with_capacity((tw * th) as usize);
    for y in 0..th {
        for x in 0..tw {
            let sx = (((x as f32 + 0.5) / scale) as u32).min(doc.width - 1);
            let sy = (((y as f32 + 0.5) / scale) as u32).min(doc.height - 1);
            let i = ((sy * doc.width + sx) * 4) as usize;
            out.push(egui::Color32::from_rgba_unmultiplied(
                pixels[i],
                pixels[i + 1],
                pixels[i + 2],
                pixels[i + 3],
            ));
        }
    }
    egui::ColorImage::new([tw as usize, th as usize], out)
}

/// A marquee drag in progress, in document pixels.
#[derive(Clone, Copy, Debug)]
pub struct MarqueeDrag {
    pub start: egui::Pos2,
    pub current: egui::Pos2,
    pub op: op_core::SelectionOp,
    /// Shift/Alt were held when the drag started with an existing selection,
    /// so they chose the combine mode and don't constrain the shape.
    pub shift_for_op: bool,
    pub alt_for_op: bool,
}

/// The Crop tool's box, in document pixels.
#[derive(Clone, Copy, Debug)]
pub struct CropBox {
    pub rect: egui::Rect,
    pub drag: Option<CropDrag>,
}

/// A crop box drag: the handle (−1, 0 or 1 per axis; `None` moves the
/// box), where it started and the box at that time.
#[derive(Clone, Copy, Debug)]
pub struct CropDrag {
    pub handle: Option<(i8, i8)>,
    pub pointer: egui::Pos2,
    pub rect: egui::Rect,
}

/// What a Free Transform drag grabbed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformHandle {
    Move,
    Rotate,
    /// A scale handle: −1, 0 or 1 along each axis of the box (0 for the
    /// middle of a side).
    Scale(i8, i8),
}

/// A Free Transform drag: the handle and the box as it was at mouse-down.
#[derive(Clone, Copy, Debug)]
pub struct TransformDrag {
    pub handle: TransformHandle,
    pub start: egui::Pos2,
    pub offset: (f32, f32),
    pub scale: (f32, f32),
    pub angle: f32,
}

/// Edit > Free Transform in progress: the box over the original bounds,
/// scaled and rotated around its center and moved by `offset`. The
/// document shows the result live; `before` is restored to cancel.
pub struct FreeTransform {
    pub before: op_core::Snapshot,
    pub bounds: (f32, f32, f32, f32),
    pub offset: (f32, f32),
    pub scale: (f32, f32),
    pub angle: f32,
    pub drag: Option<TransformDrag>,
    /// The transform the document currently shows.
    pub applied: op_core::transform::Affine,
}

impl FreeTransform {
    pub fn new(before: op_core::Snapshot, bounds: (f32, f32, f32, f32)) -> Self {
        Self {
            before,
            bounds,
            offset: (0.0, 0.0),
            scale: (1.0, 1.0),
            angle: 0.0,
            drag: None,
            applied: op_core::transform::Affine::IDENTITY,
        }
    }

    pub fn center(&self) -> (f32, f32) {
        let (x0, y0, x1, y1) = self.bounds;
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
    }

    pub fn affine(&self) -> op_core::transform::Affine {
        op_core::transform::Affine::around(
            self.center(),
            self.scale.0,
            self.scale.1,
            self.angle,
            self.offset,
        )
    }
}

/// A Lasso or Polygonal Lasso outline being drawn, in document pixels.
#[derive(Clone, Debug)]
pub struct LassoPath {
    pub points: Vec<egui::Pos2>,
    pub op: op_core::SelectionOp,
    pub polygonal: bool,
}

/// Eyedropper options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EyedropperOptions {
    /// Side of the averaged square in pixels: 1 is "Point Sample".
    pub size: u32,
    /// "Sample: All Layers" (the merged image) or "Current Layer".
    pub all_layers: bool,
}

impl EyedropperOptions {
    /// Photoshop's "Sample Size" choices.
    pub const SIZES: [(u32, &'static str); 7] = [
        (1, "Point Sample"),
        (3, "3 by 3 Average"),
        (5, "5 by 5 Average"),
        (11, "11 by 11 Average"),
        (31, "31 by 31 Average"),
        (51, "51 by 51 Average"),
        (101, "101 by 101 Average"),
    ];
}

impl Default for EyedropperOptions {
    fn default() -> Self {
        Self {
            size: 1,
            all_layers: true,
        }
    }
}

/// Magic Wand options: combine mode plus the region rule it shares with the
/// Paint Bucket (tolerance 32, anti-alias and contiguous on by default).
#[derive(Clone, Copy, Debug, Default)]
pub struct WandOptions {
    pub mode: SelectionMode,
    pub region: op_core::fill::BucketOptions,
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

impl SelectionMode {
    pub fn op(self) -> op_core::SelectionOp {
        match self {
            Self::New => op_core::SelectionOp::Replace,
            Self::Add => op_core::SelectionOp::Add,
            Self::Subtract => op_core::SelectionOp::Subtract,
            Self::Intersect => op_core::SelectionOp::Intersect,
        }
    }
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

pub struct MarqueeOptions {
    pub mode: SelectionMode,
    pub feather: f32,
    /// Photoshop has Anti-alias on by default (it applies to the elliptical
    /// marquee).
    pub anti_alias: bool,
    pub style: MarqueeStyle,
}

impl Default for MarqueeOptions {
    fn default() -> Self {
        Self {
            mode: SelectionMode::New,
            feather: 0.0,
            anti_alias: true,
            style: MarqueeStyle::Normal,
        }
    }
}

/// What a confirmed Color Picker color is applied to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerTarget {
    Foreground,
    Background,
    /// The "Other..." canvas extension color in the Canvas Size dialog.
    CanvasExtension,
    /// "Color..." in the Fill dialog.
    FillColor,
}

pub struct PickerSession {
    pub picker: crate::dialogs::ColorPicker,
    pub target: PickerTarget,
}

/// Settings of a painting tool. Photoshop keeps them per tool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintOptions {
    /// Brush diameter in pixels (1–5000).
    pub size: f32,
    /// 0..=1; not used by the Pencil.
    pub hardness: f32,
    pub opacity: f32,
    /// 0..=1; not used by the Pencil.
    pub flow: f32,
}

impl PaintOptions {
    pub const MAX_SIZE: f32 = 5000.0;

    /// Photoshop's default round brush: 30 px, 0% hardness.
    const fn brush() -> Self {
        Self {
            size: 30.0,
            hardness: 0.0,
            opacity: 1.0,
            flow: 1.0,
        }
    }

    const fn pencil() -> Self {
        Self {
            size: 1.0,
            hardness: 1.0,
            opacity: 1.0,
            flow: 1.0,
        }
    }

    /// The `]` step at this size, as in Photoshop.
    pub fn size_step(size: f32) -> f32 {
        match size {
            s if s < 10.0 => 1.0,
            s if s < 50.0 => 5.0,
            s if s < 100.0 => 10.0,
            s if s < 200.0 => 25.0,
            s if s < 300.0 => 50.0,
            _ => 100.0,
        }
    }
}

pub struct AppState {
    pub docs: HashMap<DocId, DocState>,
    /// Tab order of the open documents.
    pub doc_order: Vec<DocId>,
    pub active_doc: Option<DocId>,
    pub tool: Tool,
    /// The tool each toolbar slot shows (the last one used in its group).
    pub tool_slots: Vec<Tool>,
    pub foreground: Color,
    pub background: Color,
    pub marquee: MarqueeOptions,
    pub eyedropper: EyedropperOptions,
    pub gradient: op_core::gradient::GradientOptions,
    pub wand: WandOptions,
    pub brush: PaintOptions,
    pub pencil: PaintOptions,
    pub eraser: PaintOptions,
    /// Whether the Color panel edits the background or the foreground color.
    pub editing_background: bool,
    /// Cached HSB so the hue doesn't snap back to 0 for grays.
    pub picker_hsb: Hsb,
    pub untitled_counter: u32,
    /// Error message to show to the user.
    pub alert: Option<String>,
    /// Whether the History panel is popped out from the icon strip.
    pub history_open: bool,
    pub history_panel: crate::panels::history::PanelState,
    /// Image > Canvas Size, while open.
    pub canvas_size_dialog: Option<crate::dialogs::CanvasSizeDialog>,
    /// Edit > Fill, while open.
    pub fill_dialog: Option<crate::dialogs::FillDialog>,
    /// The last transform applied, for Edit > Transform > Again.
    pub last_transform: Option<op_core::transform::Affine>,
    /// The last filter applied, for Filter > Last Filter.
    pub last_filter: Option<op_core::filter::Filter>,
    /// An adjustment or filter dialog, while open.
    pub adjust_dialog: Option<crate::dialogs::AdjustDialog>,
    /// Image > Trim, while open.
    pub trim_dialog: Option<crate::dialogs::TrimDialog>,
    /// Paint Bucket options.
    pub bucket: op_core::fill::BucketOptions,
    /// The Color Picker, while open. It can sit on top of Canvas Size.
    pub color_picker: Option<PickerSession>,
    /// Swatches panel contents; "Add to Swatches" appends here.
    pub swatches: Vec<Color>,
    /// Documents waiting to be closed (Close All, quitting); each one with
    /// unsaved changes asks first.
    pub close_queue: Vec<DocId>,
    /// The document whose "Save changes?" prompt is showing.
    pub save_prompt: Option<DocId>,
    /// Quit once the close queue is done (it was started by quitting).
    pub quit_after_close: bool,
    /// Every document was dealt with; the window may close now.
    pub quit_approved: bool,
    pub clipboard: crate::clipboard::Clipboard,
    /// A text field has keyboard focus (as of the last frame).
    pub typing: bool,
    /// Input events for egui's next frame: menu Cut/Copy/Paste passed on to
    /// the focused text field.
    pub forward_events: Vec<egui::Event>,
}

impl Default for AppState {
    fn default() -> Self {
        let foreground = Color::from_rgba8([0x14, 0xa5, 0xdc, 0xff]);
        Self {
            docs: HashMap::new(),
            doc_order: Vec::new(),
            active_doc: None,
            tool: Tool::RectangularMarquee,
            tool_slots: op_tools::TOOLBAR.iter().map(|group| group[0]).collect(),
            foreground,
            background: Color::WHITE,
            marquee: MarqueeOptions::default(),
            eyedropper: EyedropperOptions::default(),
            gradient: Default::default(),
            wand: WandOptions::default(),
            brush: PaintOptions::brush(),
            pencil: PaintOptions::pencil(),
            eraser: PaintOptions::brush(),
            editing_background: false,
            picker_hsb: Hsb::from_color(foreground),
            untitled_counter: 0,
            alert: None,
            history_open: false,
            history_panel: Default::default(),
            canvas_size_dialog: None,
            fill_dialog: None,
            trim_dialog: None,
            adjust_dialog: None,
            last_filter: None,
            last_transform: None,
            bucket: Default::default(),
            color_picker: None,
            swatches: crate::panels::DEFAULT_SWATCHES
                .iter()
                .map(|&hex| {
                    Color::from_rgba8([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255])
                })
                .collect(),
            close_queue: Vec::new(),
            save_prompt: None,
            quit_after_close: false,
            quit_approved: false,
            clipboard: crate::clipboard::Clipboard::new(false),
            typing: false,
            forward_events: Vec::new(),
        }
    }
}

impl AppState {
    /// Options of the painting tool `tool`, if it is one.
    pub fn paint_options(&mut self, tool: Tool) -> Option<&mut PaintOptions> {
        match tool {
            Tool::Brush => Some(&mut self.brush),
            Tool::Pencil => Some(&mut self.pencil),
            Tool::Eraser => Some(&mut self.eraser),
            _ => None,
        }
    }

    /// Makes `tool` current and the tool shown in its toolbar slot.
    pub fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.tool_slots[tool.slot()] = tool;
    }

    /// Adds a document as the last tab and makes it active.
    pub fn add_document(&mut self, doc: Document, initial: &str) {
        let id = doc.id;
        self.docs.insert(id, DocState::new(doc, initial));
        self.doc_order.push(id);
        self.active_doc = Some(id);
    }

    /// Closes a document; the tab to its left becomes active if it was active.
    pub fn close_document(&mut self, id: DocId) {
        let index = self.doc_order.iter().position(|d| *d == id);
        self.doc_order.retain(|d| *d != id);
        self.docs.remove(&id);
        if self.active_doc == Some(id) {
            self.active_doc = index.and_then(|i| {
                self.doc_order
                    .get(i.saturating_sub(1))
                    .or(self.doc_order.first())
                    .copied()
            });
        }
    }

    /// Whether a modal dialog is open; menus and shortcuts are disabled meanwhile.
    pub fn modal_open(&self) -> bool {
        self.canvas_size_dialog.is_some()
            || self.fill_dialog.is_some()
            || self.trim_dialog.is_some()
            || self.adjust_dialog.is_some()
            || self.color_picker.is_some()
            || self.save_prompt.is_some()
            || self.alert.is_some()
            || self.transforming()
    }

    /// Whether the active document is in Free Transform (menus and tool
    /// keys are off meanwhile, as in Photoshop).
    pub fn transforming(&self) -> bool {
        self.active_doc
            .and_then(|id| self.docs.get(&id))
            .is_some_and(|d| d.free_transform.is_some())
    }

    /// Opens the Color Picker for the foreground or background color, titled
    /// like Photoshop's ("Color Picker (Foreground Color)").
    pub fn open_color_picker(&mut self, target: PickerTarget) {
        let (title, color) = match target {
            PickerTarget::Foreground => ("Color Picker (Foreground Color)", self.foreground),
            PickerTarget::Background => ("Color Picker (Background Color)", self.background),
            PickerTarget::CanvasExtension => ("Color Picker", self.background),
            PickerTarget::FillColor => ("Color Picker (Fill Color)", self.foreground),
        };
        self.color_picker = Some(PickerSession {
            picker: crate::dialogs::ColorPicker::new(title, color),
            target,
        });
    }

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
