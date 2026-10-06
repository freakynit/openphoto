//! File I/O. Bitmap formats go through the `image` crate.

use std::io::Cursor;
use std::path::Path;

use image::{DynamicImage, ImageFormat, RgbImage, RgbaImage};
use op_core::Document;

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("could not read image: {0}")]
    Read(image::ImageError),
    #[error("could not write image: {0}")]
    Write(image::ImageError),
    #[error("could not save file: {0}")]
    File(#[from] std::io::Error),
    #[error("unsupported file format: {0}")]
    Unsupported(String),
}

/// Extensions offered in the Open dialog.
pub const OPEN_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif"];

/// Background that transparent pixels are flattened onto for formats without
/// alpha. White, like the default matte of Photoshop's Export As.
const MATTE: [u8; 3] = [255, 255, 255];

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

pub fn open(path: &Path) -> Result<Document, IoError> {
    let ext = extension(path);
    if !OPEN_EXTENSIONS.contains(&ext.as_str()) {
        return Err(IoError::Unsupported(ext));
    }

    let img = image::open(path).map_err(IoError::Read)?.into_rgba8();
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".into());
    let (w, h) = img.dimensions();
    Ok(Document::from_rgba8(title, w, h, img.as_raw()))
}

/// Exports the composite as PNG/JPEG/etc., chosen by file extension.
///
/// JPEG has no alpha channel, so the composite is flattened onto a white
/// matte first. The file is encoded in memory and only written once encoding
/// succeeded, so a failed export never leaves a partial file behind.
pub fn export_composite(doc: &Document, path: &Path) -> Result<(), IoError> {
    let format = ImageFormat::from_path(path).map_err(|_| IoError::Unsupported(extension(path)))?;

    let pixels = doc.composite_rgba8();
    let rgba = RgbaImage::from_raw(doc.width, doc.height, pixels)
        .expect("composite buffer size matches document");
    let image = if format == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(flatten(&rgba, MATTE))
    } else {
        DynamicImage::ImageRgba8(rgba)
    };

    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).map_err(IoError::Write)?;
    std::fs::write(path, bytes.into_inner())?;
    Ok(())
}

/// Composites straight-alpha RGBA over an opaque `matte` color, in the same
/// gamma-encoded space the document is composited in.
fn flatten(rgba: &RgbaImage, matte: [u8; 3]) -> RgbImage {
    let (w, h) = rgba.dimensions();
    let mut out = RgbImage::new(w, h);
    for (src, dst) in rgba.pixels().zip(out.pixels_mut()) {
        let a = src[3] as u32;
        for c in 0..3 {
            dst[c] = ((src[c] as u32 * a + matte[c] as u32 * (255 - a) + 127) / 255) as u8;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_core::{Color, Layer, TiledImage};

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("op-io-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    /// 2×1 document: an opaque red pixel and a fully transparent pixel.
    fn half_transparent_doc() -> Document {
        Document::from_rgba8("t", 2, 1, &[255, 0, 0, 255, 0, 0, 0, 0])
    }

    #[test]
    fn jpeg_export_flattens_onto_white() {
        let path = temp_path("out.jpg");
        export_composite(&half_transparent_doc(), &path).unwrap();
        let back = image::open(&path).unwrap().into_rgb8();
        assert_eq!(back.dimensions(), (2, 1));
        // JPEG is lossy, so compare loosely
        let near =
            |p: &image::Rgb<u8>, e: [u8; 3]| p.0.iter().zip(e).all(|(a, b)| a.abs_diff(b) < 40);
        assert!(near(back.get_pixel(0, 0), [255, 0, 0]));
        assert!(near(back.get_pixel(1, 0), [255, 255, 255]));
    }

    #[test]
    fn png_export_keeps_transparency() {
        let path = temp_path("out.png");
        export_composite(&half_transparent_doc(), &path).unwrap();
        let back = image::open(&path).unwrap().into_rgba8();
        assert_eq!(back.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(back.get_pixel(1, 0).0[3], 0);
    }

    #[test]
    fn unknown_extension_writes_nothing() {
        let path = temp_path("out.xyz");
        let doc = Document::new_with_background("t", 1, 1, Color::WHITE);
        assert!(matches!(
            export_composite(&doc, &path),
            Err(IoError::Unsupported(_))
        ));
        assert!(!path.exists());
    }

    #[test]
    fn transparent_png_opens_as_regular_layer() {
        let path = temp_path("in.png");
        let mut doc = Document::new_with_background("t", 2, 2, Color::WHITE);
        // Replace the background with a half-transparent layer before saving
        let id = doc.new_layer_id();
        doc.layers = vec![Layer::raster(
            id,
            "x",
            TiledImage::filled(2, 2, [0, 0, 255, 128]),
        )];
        export_composite(&doc, &path).unwrap();

        let opened = open(&path).unwrap();
        assert_eq!(opened.layers[0].name, "Layer 0");
        assert!(!opened.layers[0].is_background);
    }

    #[test]
    fn flatten_blends_alpha() {
        let rgba = RgbaImage::from_raw(1, 1, vec![0, 0, 0, 128]).unwrap();
        assert_eq!(
            flatten(&rgba, [255, 255, 255]).get_pixel(0, 0).0,
            [127, 127, 127]
        );
    }
}
