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

    /// Copies `dst.len() / 4` pixels of row `y`, starting at column `x`, into
    /// `dst`. Pixels in missing tiles are transparent. The span must lie
    /// inside the image.
    fn read_span(&self, x: u32, y: u32, dst: &mut [u8]) {
        let (ty, row) = (y / TILE_SIZE, y % TILE_SIZE);
        let mut done = 0;
        while done < dst.len() / 4 {
            let sx = x + done as u32;
            let (tx, col) = (sx / TILE_SIZE, sx % TILE_SIZE);
            let n = ((TILE_SIZE - col) as usize).min(dst.len() / 4 - done);
            let out = &mut dst[done * 4..(done + n) * 4];
            match self.tile(tx, ty) {
                Some(tile) => {
                    let i = ((row * TILE_SIZE + col) * 4) as usize;
                    out.copy_from_slice(&tile.data[i..i + n * 4]);
                }
                None => out.fill(0),
            }
            done += n;
        }
    }

    /// Returns a `width`×`height` image with this one placed at (`dx`, `dy`).
    /// Pixels outside the original image are filled with `fill`.
    ///
    /// Works one destination tile at a time, so memory stays proportional to
    /// the allocated tiles, and fully transparent tiles stay unallocated.
    pub fn with_canvas(&self, width: u32, height: u32, dx: i64, dy: i64, fill: [u8; 4]) -> Self {
        let mut out = Self::new(width, height);
        let (src_w, src_h) = (self.width as i64, self.height as i64);
        for ty in 0..out.tiles_y() {
            for tx in 0..out.tiles_x() {
                let (x0, y0) = ((tx * TILE_SIZE) as i64, (ty * TILE_SIZE) as i64);
                let tw = (TILE_SIZE as i64).min(width as i64 - x0);
                let th = (TILE_SIZE as i64).min(height as i64 - y0);

                // Columns of this tile covered by the old image, in source coordinates
                let sx0 = (x0 - dx).max(0);
                let sx1 = (x0 + tw - dx).min(src_w);
                let sy0 = (y0 - dy).max(0);
                let sy1 = (y0 + th - dy).min(src_h);
                let covered = sx0 < sx1 && sy0 < sy1;
                if !covered && fill[3] == 0 {
                    continue;
                }

                let mut tile = Tile::filled(fill);
                if covered {
                    let col = (sx0 + dx - x0) as usize;
                    let len = (sx1 - sx0) as usize;
                    for sy in sy0..sy1 {
                        let row = (sy + dy - y0) as usize;
                        let i = (row * TILE_SIZE as usize + col) * 4;
                        self.read_span(sx0 as u32, sy as u32, &mut tile.data[i..i + len * 4]);
                    }
                }
                if !tile.is_transparent() {
                    out.tiles.insert((tx, ty), Arc::new(tile));
                }
            }
        }
        out
    }

    /// Writes one pixel (ignored outside the image). Writing a transparent
    /// pixel into a missing tile allocates nothing.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let (tx, ty) = (x / TILE_SIZE, y / TILE_SIZE);
        if rgba[3] == 0 && self.tile(tx, ty).is_none() {
            return;
        }
        let i = (((y % TILE_SIZE) * TILE_SIZE + x % TILE_SIZE) * 4) as usize;
        self.tile_mut(tx, ty).data[i..i + 4].copy_from_slice(&rgba);
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
    fn with_canvas_across_tiles() {
        let w = TILE_SIZE + 10;
        let px: Vec<u8> = (0..w * 3)
            .flat_map(|i| [(i % 251) as u8, 0, 0, 255])
            .collect();
        let img = TiledImage::from_rgba8(w, 3, &px);
        let moved = img.with_canvas(w + 300, 5, 300, 1, [0; 4]);
        for x in 0..w {
            assert_eq!(moved.pixel(x + 300, 2), img.pixel(x, 1));
        }
        assert_eq!(moved.pixel(299, 2), [0; 4]);
        assert_eq!(moved.pixel(300, 0), [0; 4]);
        // The untouched first tile column stays unallocated
        assert!(moved.tile(0, 0).is_none());
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
