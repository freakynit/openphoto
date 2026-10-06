//! The clipboard behind Edit > Cut, Copy and Paste: copied pixels are kept
//! here with their position, and also placed on the system clipboard so
//! other applications can paste them (and images copied elsewhere can be
//! pasted here).

use op_core::clipboard::Clip;

pub struct Clipboard {
    /// The last pixels copied in OpenPhoto.
    internal: Option<Clip>,
    /// Whether `internal` was also put on the system clipboard.
    on_system: bool,
    /// `None` in headless tests, or where there is no system clipboard.
    system: Option<arboard::Clipboard>,
}

impl Clipboard {
    pub fn new(use_system: bool) -> Self {
        Self {
            internal: None,
            on_system: false,
            system: use_system.then(|| arboard::Clipboard::new().ok()).flatten(),
        }
    }

    pub fn set(&mut self, clip: Clip) {
        self.on_system = self.system.as_mut().is_some_and(|system| {
            let image = arboard::ImageData {
                width: clip.width as usize,
                height: clip.height as usize,
                bytes: clip.pixels.as_slice().into(),
            };
            system.set_image(image).is_ok()
        });
        self.internal = Some(clip);
    }

    /// What Paste pastes. An image on the system clipboard wins unless it is
    /// the one copied here, in which case the internal copy (which knows
    /// where it came from) is used.
    pub fn get(&mut self) -> Option<Clip> {
        let Some(system) = &mut self.system else {
            return self.internal.clone();
        };
        match system.get_image() {
            Ok(image) => {
                let (w, h) = (image.width as u32, image.height as u32);
                if let Some(internal) = &self.internal
                    && self.on_system
                    && same_image(internal, w, h, &image.bytes)
                {
                    return Some(internal.clone());
                }
                Some(Clip::from_rgba8(w, h, image.bytes.into_owned()))
            }
            // Something else (or nothing) was copied since
            Err(_) if self.on_system => None,
            Err(_) => self.internal.clone(),
        }
    }

    /// Text on the system clipboard, for pasting into a text field.
    pub fn text(&mut self) -> Option<String> {
        self.system.as_mut()?.get_text().ok()
    }
}

/// Whether the system clipboard still holds `clip`. The round trip through
/// the system's image formats can change values slightly (premultiplied
/// alpha, color conversion), so this allows a small average difference.
fn same_image(clip: &Clip, width: u32, height: u32, bytes: &[u8]) -> bool {
    if (clip.width, clip.height) != (width, height) || clip.pixels.len() != bytes.len() {
        return false;
    }
    let diff: u64 = clip
        .pixels
        .iter()
        .zip(bytes)
        .map(|(&a, &b)| a.abs_diff(b) as u64)
        .sum();
    diff <= 3 * bytes.len() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_clipboard_keeps_the_position() {
        let mut clipboard = Clipboard::new(false);
        assert_eq!(clipboard.get(), None);
        let clip = Clip {
            width: 1,
            height: 1,
            pixels: vec![1, 2, 3, 255],
            origin: Some((5, 6)),
        };
        clipboard.set(clip.clone());
        assert_eq!(clipboard.get(), Some(clip));
    }

    #[test]
    fn round_tripped_images_are_recognized() {
        let clip = Clip::from_rgba8(2, 1, vec![10, 20, 30, 255, 0, 0, 0, 128]);
        assert!(same_image(&clip, 2, 1, &[11, 19, 30, 255, 1, 0, 0, 128]));
        assert!(!same_image(&clip, 1, 2, &clip.pixels));
        assert!(!same_image(&clip, 2, 1, &[200, 200, 200, 255, 0, 0, 0, 0]));
    }
}
