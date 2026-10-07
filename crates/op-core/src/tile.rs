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
///
/// `width` × `height` is the canvas. A layer's pixels can also lie outside
/// it, like Photoshop's: tiles left of or above the canvas (negative tile
/// indices), right of or below it, and the part of an edge tile past the
/// canvas edge all hold pixels outside the canvas. So nothing may write
/// into an edge tile's part past the canvas unless it means to put pixels
/// there.
#[derive(Clone)]
pub struct TiledImage {
    width: u32,
    height: u32,
    tiles: HashMap<(i32, i32), Arc<Tile>>,
}

impl std::fmt::Debug for TiledImage {
    /// Size and allocated tiles only; the pixels would be far too much.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TiledImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("tiles", &self.tiles.len())
            .finish()
    }
}

impl TiledImage {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            tiles: HashMap::new(),
        }
    }

    /// The canvas filled with `rgba`; nothing outside the canvas.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut img = Self::new(width, height);
        if rgba[3] != 0 {
            let tile = Arc::new(Tile::filled(rgba));
            for ty in 0..img.tiles_y() {
                for tx in 0..img.tiles_x() {
                    img.tiles.insert((tx as i32, ty as i32), tile.clone());
                }
            }
            img.clear_edge_padding();
        }
        img
    }

    /// Clears the part of the edge tiles past the canvas (after filling
    /// whole tiles), so it holds no pixels outside the canvas.
    fn clear_edge_padding(&mut self) {
        let (w, h) = (self.width, self.height);
        let edge_x = w % TILE_SIZE != 0;
        let edge_y = h % TILE_SIZE != 0;
        if !edge_x && !edge_y {
            return;
        }
        let (last_tx, last_ty) = (self.tiles_x() - 1, self.tiles_y() - 1);
        for ty in 0..self.tiles_y() {
            for tx in 0..self.tiles_x() {
                if !(edge_x && tx == last_tx) && !(edge_y && ty == last_ty) {
                    continue;
                }
                if !self.tiles.contains_key(&(tx as i32, ty as i32)) {
                    continue;
                }
                let tile = self.tile_mut(tx, ty);
                for row in 0..TILE_SIZE {
                    for col in 0..TILE_SIZE {
                        if tx * TILE_SIZE + col >= w || ty * TILE_SIZE + row >= h {
                            let i = ((row * TILE_SIZE + col) * 4) as usize;
                            tile.data[i..i + 4].fill(0);
                        }
                    }
                }
            }
        }
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
                    img.tiles.insert((tx as i32, ty as i32), Arc::new(tile));
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

    /// A tile of the canvas.
    pub fn tile(&self, tx: u32, ty: u32) -> Option<&Tile> {
        self.tile_at(tx as i32, ty as i32)
    }

    /// Any tile, inside the canvas or not.
    pub fn tile_at(&self, tx: i32, ty: i32) -> Option<&Tile> {
        self.tiles.get(&(tx, ty)).map(|t| t.as_ref())
    }

    /// Returns a writable tile of the canvas, allocating a transparent one
    /// if missing and copying it first if shared.
    pub fn tile_mut(&mut self, tx: u32, ty: u32) -> &mut Tile {
        self.tile_at_mut(tx as i32, ty as i32)
    }

    /// [`Self::tile_mut`] for any tile, inside the canvas or not.
    pub fn tile_at_mut(&mut self, tx: i32, ty: i32) -> &mut Tile {
        let tile = self
            .tiles
            .entry((tx, ty))
            .or_insert_with(|| Arc::new(Tile::filled([0; 4])));
        Arc::make_mut(tile)
    }

    /// Reads a pixel anywhere, inside the canvas or outside it.
    pub fn pixel_at(&self, x: i64, y: i64) -> [u8; 4] {
        let ts = TILE_SIZE as i64;
        let (tx, ty) = (x.div_euclid(ts) as i32, y.div_euclid(ts) as i32);
        match self.tile_at(tx, ty) {
            Some(tile) => {
                let (col, row) = (x.rem_euclid(ts), y.rem_euclid(ts));
                let i = ((row * ts + col) * 4) as usize;
                tile.data[i..i + 4].try_into().unwrap()
            }
            None => [0; 4],
        }
    }

    /// Writes a pixel anywhere, inside the canvas or outside it. Writing a
    /// transparent pixel into a missing tile allocates nothing.
    pub fn set_pixel_at(&mut self, x: i64, y: i64, rgba: [u8; 4]) {
        let ts = TILE_SIZE as i64;
        let (tx, ty) = (x.div_euclid(ts) as i32, y.div_euclid(ts) as i32);
        if rgba[3] == 0 && self.tile_at(tx, ty).is_none() {
            return;
        }
        let (col, row) = (x.rem_euclid(ts), y.rem_euclid(ts));
        let i = ((row * ts + col) * 4) as usize;
        self.tile_at_mut(tx, ty).data[i..i + 4].copy_from_slice(&rgba);
    }

    /// The box around all non-transparent pixels, inside the canvas or
    /// not, as (x0, y0, x1, y1); `None` when there are none.
    pub fn content_bounds(&self) -> Option<(i64, i64, i64, i64)> {
        let ts = TILE_SIZE as i64;
        // Tiles on the edge of the tile grid first: once they set the
        // bounds, tiles wholly inside them are skipped, so a large layer
        // costs its border tiles, not every pixel (this runs every frame
        // for the Move tool's controls and the align buttons)
        let (mut tx0, mut ty0, mut tx1, mut ty1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(tx, ty) in self.tiles.keys() {
            (tx0, ty0, tx1, ty1) = (tx0.min(tx), ty0.min(ty), tx1.max(tx), ty1.max(ty));
        }
        let mut order: Vec<_> = self.tiles.iter().collect();
        order.sort_by_key(|(k, _)| !(k.0 == tx0 || k.0 == tx1 || k.1 == ty0 || k.1 == ty1));
        let mut b: Option<(i64, i64, i64, i64)> = None;
        for (&(tx, ty), tile) in order {
            let (ox, oy) = (tx as i64 * ts, ty as i64 * ts);
            if let Some((x0, y0, x1, y1)) = b
                && ox >= x0
                && oy >= y0
                && ox + ts <= x1
                && oy + ts <= y1
            {
                continue;
            }
            let Some((cx0, cy0, cx1, cy1)) = tile_bounds(&tile.data) else {
                continue;
            };
            let (x0, y0, x1, y1) = (ox + cx0, oy + cy0, ox + cx1, oy + cy1);
            b = Some(match b {
                None => (x0, y0, x1, y1),
                Some(c) => (c.0.min(x0), c.1.min(y0), c.2.max(x1), c.3.max(y1)),
            });
        }
        b
    }

    /// Whether any pixel lies outside the canvas.
    pub fn has_pixels_outside(&self) -> bool {
        self.content_bounds().is_some_and(|(x0, y0, x1, y1)| {
            x0 < 0 || y0 < 0 || x1 > self.width as i64 || y1 > self.height as i64
        })
    }

    /// This image with the pixels outside the canvas deleted (Photoshop's
    /// background layer, and Crop with Delete Cropped Pixels on).
    pub fn clipped(&self) -> Self {
        let mut out = Self::new(self.width, self.height);
        for (&(tx, ty), tile) in &self.tiles {
            if tx < 0 || ty < 0 || tx as u32 >= self.tiles_x() || ty as u32 >= self.tiles_y() {
                continue;
            }
            out.tiles.insert((tx, ty), tile.clone());
        }
        out.clear_edge_padding();
        out.drop_transparent_tiles();
        out
    }

    fn drop_transparent_tiles(&mut self) {
        self.tiles.retain(|_, t| !t.is_transparent());
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

    /// Writes `src` (whole pixels) into row `y` starting at column `x`,
    /// anywhere, allocating tiles as needed; spans of transparent pixels
    /// over missing tiles allocate nothing.
    fn write_span_at(&mut self, x: i64, y: i64, src: &[u8]) {
        let ts = TILE_SIZE as i64;
        let (ty, row) = (y.div_euclid(ts) as i32, y.rem_euclid(ts));
        let mut done = 0usize;
        let n_px = src.len() / 4;
        while done < n_px {
            let sx = x + done as i64;
            let (tx, col) = (sx.div_euclid(ts) as i32, sx.rem_euclid(ts));
            let n = ((ts - col) as usize).min(n_px - done);
            let part = &src[done * 4..(done + n) * 4];
            if self.tile_at(tx, ty).is_some() || part.as_chunks::<4>().0.iter().any(|p| p[3] != 0) {
                let i = ((row * ts + col) * 4) as usize;
                self.tile_at_mut(tx, ty).data[i..i + n * 4].copy_from_slice(part);
            }
            done += n;
        }
    }

    /// Returns a `width`×`height` canvas with this image's pixels moved by
    /// (`dx`, `dy`), keeping the ones that end up outside it (like a
    /// Photoshop layer's). Canvas pixels that the old canvas didn't cover are
    /// filled with `fill` when it's opaque, e.g. the background color.
    ///
    /// Works one allocated tile row at a time, so memory stays proportional
    /// to the allocated tiles, and fully transparent tiles stay unallocated.
    pub fn with_canvas(&self, width: u32, height: u32, dx: i64, dy: i64, fill: [u8; 4]) -> Self {
        let mut out = Self::new(width, height);
        let ts = TILE_SIZE as i64;
        let mut keys: Vec<(i32, i32)> = self.tiles.keys().copied().collect();
        keys.sort_unstable();
        for (tx, ty) in keys {
            let tile = &self.tiles[&(tx, ty)];
            for row in 0..ts {
                let src = &tile.data[(row * ts * 4) as usize..((row + 1) * ts * 4) as usize];
                out.write_span_at(tx as i64 * ts + dx, ty as i64 * ts + row + dy, src);
            }
        }
        if fill[3] != 0 {
            // The new canvas area outside the old canvas
            let (ox0, oy0) = (dx, dy);
            let (ox1, oy1) = (dx + self.width as i64, dy + self.height as i64);
            let w = width as i64;
            let row = fill.repeat(width as usize);
            for y in 0..height as i64 {
                if y < oy0 || y >= oy1 {
                    out.write_span_at(0, y, &row);
                    continue;
                }
                let left = ox0.clamp(0, w);
                let right = ox1.clamp(0, w);
                if left > 0 {
                    out.write_span_at(0, y, &row[..left as usize * 4]);
                }
                if right < w {
                    out.write_span_at(right, y, &row[..(w - right) as usize * 4]);
                }
            }
        }
        out.drop_transparent_tiles();
        out
    }

    /// The pixels of the `w` × `h` region at (`x0`, `y0`), which may reach
    /// outside the canvas, as a tightly packed RGBA8 buffer.
    pub fn region_rgba8(&self, x0: i64, y0: i64, w: u32, h: u32) -> Vec<u8> {
        let ts = TILE_SIZE as i64;
        let (w, h) = (w as usize, h as usize);
        let mut out = vec![0; w * h * 4];
        for row in 0..h {
            let y = y0 + row as i64;
            let (ty, ry) = (y.div_euclid(ts) as i32, y.rem_euclid(ts));
            let mut done = 0usize;
            while done < w {
                let x = x0 + done as i64;
                let (tx, col) = (x.div_euclid(ts) as i32, x.rem_euclid(ts));
                let n = ((ts - col) as usize).min(w - done);
                if let Some(tile) = self.tile_at(tx, ty) {
                    let i = ((ry * ts + col) * 4) as usize;
                    let d = (row * w + done) * 4;
                    out[d..d + n * 4].copy_from_slice(&tile.data[i..i + n * 4]);
                }
                done += n;
            }
        }
        out
    }

    /// A `width` × `height` canvas holding only `pixels`, a tightly packed
    /// RGBA8 buffer of the `w` × `h` region at (`x0`, `y0`), which may reach
    /// outside the canvas.
    pub fn from_region(
        width: u32,
        height: u32,
        x0: i64,
        y0: i64,
        w: u32,
        h: u32,
        pixels: &[u8],
    ) -> Self {
        assert_eq!(pixels.len(), (w as usize) * (h as usize) * 4);
        let mut out = Self::new(width, height);
        let row = w as usize * 4;
        for y in 0..h as usize {
            out.write_span_at(x0, y0 + y as i64, &pixels[y * row..(y + 1) * row]);
        }
        out.drop_transparent_tiles();
        out
    }

    /// The whole image as a tightly packed RGBA8 buffer.
    pub fn to_rgba8(&self) -> Vec<u8> {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = vec![0; w * h * 4];
        for y in 0..h {
            self.read_span(0, y as u32, &mut out[y * w * 4..(y + 1) * w * 4]);
        }
        out
    }

    /// A `width`×`height` image whose pixel (x, y) is this image's pixel
    /// `source(x, y)`, e.g. a rotated or mirrored copy.
    pub fn remapped(
        &self,
        width: u32,
        height: u32,
        source: impl Fn(u32, u32) -> (u32, u32),
    ) -> Self {
        let src = self.to_rgba8();
        let sw = self.width as usize;
        let mut out = vec![0; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let (sx, sy) = source(x, y);
                let s = (sy as usize * sw + sx as usize) * 4;
                let d = ((y * width + x) * 4) as usize;
                out[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
        Self::from_rgba8(width, height, &out)
    }

    /// Writes one pixel (ignored outside the image). Writing a transparent
    /// pixel into a missing tile allocates nothing.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        self.set_pixel_at(x as i64, y as i64, rgba);
    }

    /// Number of allocated tiles (for debugging and memory stats).
    pub fn allocated_tiles(&self) -> usize {
        self.tiles.len()
    }
}

/// The box around a tile's pixels with any alpha (tile coordinates, end
/// exclusive): rows scanned in from the top and bottom, then columns in
/// from the sides between them, each stopping at the first pixel found.
fn tile_bounds(data: &[u8]) -> Option<(i64, i64, i64, i64)> {
    let ts = TILE_SIZE as usize;
    let alpha = |x: usize, y: usize| data[(y * ts + x) * 4 + 3] != 0;
    let row_has = |y: usize| (0..ts).any(|x| alpha(x, y));
    let top = (0..ts).find(|&y| row_has(y))?;
    let bottom = (top..ts).rev().find(|&y| row_has(y))?;
    let col_has = |x: usize| (top..=bottom).any(|y| alpha(x, y));
    let left = (0..ts).find(|&x| col_has(x))?;
    let right = (left..ts).rev().find(|&x| col_has(x))?;
    Some((left as i64, top as i64, right as i64 + 1, bottom as i64 + 1))
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

    #[test]
    fn filled_leaves_nothing_past_the_canvas() {
        // 10 × 10 inside a 256 × 256 tile: the rest of the tile is outside
        let img = TiledImage::filled(10, 10, [9, 9, 9, 255]);
        assert_eq!(img.pixel_at(9, 9), [9, 9, 9, 255]);
        assert_eq!(img.pixel_at(10, 0), [0; 4]);
        assert_eq!(img.content_bounds(), Some((0, 0, 10, 10)));
        assert!(!img.has_pixels_outside());
    }

    #[test]
    fn moving_keeps_pixels_outside_the_canvas() {
        let mut img = TiledImage::new(10, 10);
        img.set_pixel(1, 1, [1, 2, 3, 255]);
        img.set_pixel(8, 8, [4, 5, 6, 255]);
        let moved = img.with_canvas(10, 10, -300, 5, [0; 4]);
        assert_eq!(moved.pixel_at(-299, 6), [1, 2, 3, 255]);
        assert_eq!(moved.content_bounds(), Some((-299, 6, -291, 14)));
        assert!(moved.has_pixels_outside());
        // Moving back restores both
        let back = moved.with_canvas(10, 10, 300, -5, [0; 4]);
        assert_eq!(back.pixel(1, 1), [1, 2, 3, 255]);
        assert_eq!(back.pixel(8, 8), [4, 5, 6, 255]);
        assert!(!back.has_pixels_outside());
    }

    #[test]
    fn clipped_drops_what_is_outside() {
        let mut img = TiledImage::new(10, 10);
        img.set_pixel(1, 1, [1, 2, 3, 255]);
        img.set_pixel_at(-1, 0, [7, 7, 7, 255]);
        img.set_pixel_at(12, 3, [7, 7, 7, 255]);
        img.set_pixel_at(3, 400, [7, 7, 7, 255]);
        let c = img.clipped();
        assert_eq!(c.content_bounds(), Some((1, 1, 2, 2)));
        assert_eq!(c.allocated_tiles(), 1);
    }

    #[test]
    fn opaque_fill_only_covers_new_canvas_area() {
        let mut img = TiledImage::filled(2, 2, [1, 1, 1, 255]);
        img.set_pixel(0, 0, [0, 0, 0, 0]);
        let grown = img.with_canvas(4, 3, 1, 0, [9, 9, 9, 255]);
        // The transparent pixel of the old canvas stays transparent
        assert_eq!(grown.pixel(1, 0), [0; 4]);
        assert_eq!(grown.pixel(0, 0), [9, 9, 9, 255]);
        assert_eq!(grown.pixel(3, 2), [9, 9, 9, 255]);
        assert_eq!(grown.pixel(2, 1), [1, 1, 1, 255]);
        assert!(!grown.has_pixels_outside());
    }

    #[test]
    fn regions_reach_outside_the_canvas() {
        let mut img = TiledImage::new(4, 4);
        img.set_pixel_at(-1, -1, [1, 1, 1, 255]);
        img.set_pixel_at(300, 2, [2, 2, 2, 255]);
        let px = img.region_rgba8(-1, -1, 302, 4);
        assert_eq!(&px[0..4], &[1, 1, 1, 255]);
        let i = (3 * 302 + 301) * 4;
        assert_eq!(&px[i..i + 4], &[2, 2, 2, 255]);
        let back = TiledImage::from_region(4, 4, -1, -1, 302, 4, &px);
        assert_eq!(back.content_bounds(), img.content_bounds());
        assert_eq!(back.pixel_at(300, 2), [2, 2, 2, 255]);
    }

    #[test]
    fn content_bounds_match_a_full_scan() {
        // Sparse dots, some past the canvas on every side, in many tiles
        for seed in 0u64..40 {
            let mut img = TiledImage::new(700, 500);
            let mut v = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut dots = Vec::new();
            for _ in 0..(seed % 7) {
                v ^= v << 13;
                v ^= v >> 7;
                v ^= v << 17;
                let x = (v % 1100) as i64 - 200;
                let y = ((v >> 20) % 900) as i64 - 200;
                img.set_pixel_at(x, y, [1, 2, 3, (v >> 40) as u8 | 1]);
                dots.push((x, y));
            }
            let want = dots
                .iter()
                .fold(None, |b: Option<(i64, i64, i64, i64)>, &(x, y)| {
                    Some(match b {
                        None => (x, y, x + 1, y + 1),
                        Some(c) => (c.0.min(x), c.1.min(y), c.2.max(x + 1), c.3.max(y + 1)),
                    })
                });
            assert_eq!(img.content_bounds(), want, "seed {seed}");
        }
        // A large filled block: interior tiles are skipped, edges exact
        let mut img = TiledImage::new(3000, 1080);
        for y in 101..899 {
            for x in 503..2497 {
                img.set_pixel(x, y, [9, 9, 9, 255]);
            }
        }
        assert_eq!(img.content_bounds(), Some((503, 101, 2497, 899)));
        // and it is quick: it runs several times a frame (the old full
        // scan took tens of milliseconds)
        let t = std::time::Instant::now();
        for _ in 0..10 {
            std::hint::black_box(img.content_bounds());
        }
        assert!(
            t.elapsed() < std::time::Duration::from_millis(20),
            "{:?}",
            t.elapsed()
        );
    }
}
