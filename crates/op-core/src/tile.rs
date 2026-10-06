use std::collections::HashMap;
use std::sync::Arc;

/// Tile edge length in pixels.
pub const TILE_SIZE: u32 = 256;
const TILE_BYTES: usize = (TILE_SIZE * TILE_SIZE * 4) as usize;

/// A 256×256 RGBA8 tile with straight alpha.
#[derive(Clone)]
pub struct Tile {
    pub data: Box<[u8]>,
}

impl Tile {
    pub fn filled(rgba: [u8; 4]) -> Self {
        let data = rgba.repeat(TILE_BYTES / 4).into_boxed_slice();
        Self { data }
    }

    fn is_transparent(&self) -> bool {
        self.data.as_chunks::<4>().0.iter().all(|p| p[3] == 0)
    }
}

/// A sparse tiled image. Missing tiles are fully transparent and use no memory.
///
/// Tiles are shared through `Arc`: cloning an image (e.g. for a history snapshot)
/// only copies pointers, and [`TiledImage::tile_mut`] copies on write.
#[derive(Clone)]
pub struct TiledImage {
    width: u32,
    height: u32,
    tiles: HashMap<(u32, u32), Arc<Tile>>,
}

impl TiledImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tiles: HashMap::new(),
        }
    }

    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut img = Self::new(width, height);
        if rgba[3] != 0 {
            let tile = Arc::new(Tile::filled(rgba));
            for ty in 0..img.tiles_y() {
                for tx in 0..img.tiles_x() {
                    img.tiles.insert((tx, ty), tile.clone());
                }
            }
        }
        img
    }

    /// Builds from a tightly packed RGBA8 buffer, dropping fully transparent tiles.
    pub fn from_rgba8(width: u32, height: u32, pixels: &[u8]) -> Self {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let mut img = Self::new(width, height);
        for ty in 0..img.tiles_y() {
            for tx in 0..img.tiles_x() {
                let mut tile = Tile::filled([0; 4]);
                let (x0, y0) = (tx * TILE_SIZE, ty * TILE_SIZE);
                let w = TILE_SIZE.min(width - x0) as usize;
                for row in 0..TILE_SIZE.min(height - y0) {
                    let src = (((y0 + row) * width + x0) * 4) as usize;
                    let dst = (row * TILE_SIZE * 4) as usize;
                    tile.data[dst..dst + w * 4].copy_from_slice(&pixels[src..src + w * 4]);
                }
                if !tile.is_transparent() {
                    img.tiles.insert((tx, ty), Arc::new(tile));
                }
            }
        }
        img
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn tiles_x(&self) -> u32 {
        self.width.div_ceil(TILE_SIZE)
    }

    pub fn tiles_y(&self) -> u32 {
        self.height.div_ceil(TILE_SIZE)
    }

    pub fn tile(&self, tx: u32, ty: u32) -> Option<&Tile> {
        self.tiles.get(&(tx, ty)).map(|t| t.as_ref())
    }

    /// Returns a writable tile, allocating a transparent one if missing and
    /// copying it first if shared.
    pub fn tile_mut(&mut self, tx: u32, ty: u32) -> &mut Tile {
        let tile = self
            .tiles
            .entry((tx, ty))
            .or_insert_with(|| Arc::new(Tile::filled([0; 4])));
        Arc::make_mut(tile)
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0; 4];
        }
        match self.tile(x / TILE_SIZE, y / TILE_SIZE) {
            Some(tile) => {
                let i = (((y % TILE_SIZE) * TILE_SIZE + x % TILE_SIZE) * 4) as usize;
                tile.data[i..i + 4].try_into().unwrap()
            }
            None => [0; 4],
        }
    }

    /// Copies the image into a tightly packed RGBA8 buffer.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0u8; w * h * 4];
        for (&(tx, ty), tile) in &self.tiles {
            let (x0, y0) = ((tx * TILE_SIZE) as usize, (ty * TILE_SIZE) as usize);
            let tw = (TILE_SIZE as usize).min(w - x0);
            let th = (TILE_SIZE as usize).min(h - y0);
            for row in 0..th {
                let src = row * TILE_SIZE as usize * 4;
                let dst = ((y0 + row) * w + x0) * 4;
                out[dst..dst + tw * 4].copy_from_slice(&tile.data[src..src + tw * 4]);
            }
        }
        out
    }

    /// Returns a `width`×`height` image with this one placed at (`dx`, `dy`).
    /// Pixels outside the original image are filled with `fill`.
    pub fn with_canvas(&self, width: u32, height: u32, dx: i64, dy: i64, fill: [u8; 4]) -> Self {
        let src = self.to_rgba8();
        let mut out = fill.repeat((width * height) as usize);
        // Overlap of the old image with the new canvas, in new-canvas coordinates
        let x0 = dx.max(0);
        let y0 = dy.max(0);
        let x1 = (dx + self.width as i64).min(width as i64);
        let y1 = (dy + self.height as i64).min(height as i64);
        if x0 < x1 {
            let len = ((x1 - x0) * 4) as usize;
            for y in y0..y1 {
                let s = (((y - dy) * self.width as i64 + (x0 - dx)) * 4) as usize;
                let d = ((y * width as i64 + x0) * 4) as usize;
                out[d..d + len].copy_from_slice(&src[s..s + len]);
            }
        }
        Self::from_rgba8(width, height, &out)
    }

    /// Number of allocated tiles (for debugging and memory stats).
    pub fn allocated_tiles(&self) -> usize {
        self.tiles.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_sparse() {
        let (w, h) = (300, 10);
        let mut px = vec![0u8; (w * h * 4) as usize];
        // A single opaque pixel, in the second tile only
        let i = ((5 * w + 280) * 4) as usize;
        px[i..i + 4].copy_from_slice(&[1, 2, 3, 255]);
        let img = TiledImage::from_rgba8(w, h, &px);
        assert_eq!(img.allocated_tiles(), 1);
        assert_eq!(img.pixel(280, 5), [1, 2, 3, 255]);
        assert_eq!(img.pixel(0, 0), [0; 4]);
    }

    #[test]
    fn with_canvas_grows_and_crops() {
        let img = TiledImage::filled(2, 2, [1, 1, 1, 255]);
        let grown = img.with_canvas(4, 4, 1, 1, [9, 9, 9, 255]);
        assert_eq!(grown.pixel(0, 0), [9, 9, 9, 255]);
        assert_eq!(grown.pixel(1, 1), [1, 1, 1, 255]);
        assert_eq!(grown.pixel(2, 2), [1, 1, 1, 255]);
        assert_eq!(grown.pixel(3, 3), [9, 9, 9, 255]);

        let cropped = img.with_canvas(1, 1, -1, -1, [0; 4]);
        assert_eq!(cropped.pixel(0, 0), [1, 1, 1, 255]);
    }

    #[test]
    fn copy_on_write() {
        let a = TiledImage::filled(10, 10, [9, 9, 9, 255]);
        let mut b = a.clone();
        b.tile_mut(0, 0).data[0] = 0;
        assert_eq!(a.pixel(0, 0), [9, 9, 9, 255]);
        assert_eq!(b.pixel(0, 0)[0], 0);
    }
}
