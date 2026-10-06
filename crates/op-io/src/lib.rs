//! File I/O. Bitmap formats go through the `image` crate; PSD will be implemented here.

use std::path::Path;

use op_core::Document;

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("could not read image: {0}")]
    Image(#[from] image::ImageError),
    #[error("unsupported file format: {0}")]
    Unsupported(String),
}

/// Extensions offered in the Open dialog.
pub const OPEN_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"];

pub fn open(path: &Path) -> Result<Document, IoError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !OPEN_EXTENSIONS.contains(&ext.as_str()) {
        return Err(IoError::Unsupported(ext));
    }

    let img = image::open(path)?.into_rgba8();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into());
    let (w, h) = img.dimensions();
    Ok(Document::from_rgba8(title, w, h, img.as_raw()))
}

/// Exports the composite as PNG/JPEG/etc., chosen by file extension.
pub fn export_composite(doc: &Document, path: &Path) -> Result<(), IoError> {
    let pixels = doc.composite_rgba8();
    let img = image::RgbaImage::from_raw(doc.width, doc.height, pixels)
        .expect("composite buffer size matches document");
    img.save(path)?;
    Ok(())
}
