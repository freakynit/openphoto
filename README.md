# OpenPhoto

A Photoshop-style image editor written in Rust.

## Running

```bash
cargo run -- path/to/image.png
```

Without arguments, a blank 1920×1080 document is created. Images can also be dropped onto the window to open them.

## Structure

| crate | Responsibility |
|---|---|
| `op-core` | Document model: layer tree, 256×256 tiled pixel storage (copy-on-write), blend modes, CPU compositing |
| `op-render` | wgpu rendering: the canvas (zoom, mipmaps, checkerboard, pixel grid), embedded in the UI via egui-wgpu paint callbacks |
| `op-tools` | Tool definitions and shortcuts; will hold each tool's editing logic |
| `op-io` | File I/O (PNG/JPEG/WebP/TIFF/BMP/GIF); PSD is not implemented yet |
| `op-color` | Color model conversions (HSB, hex); ICC color management will go here |
| `op-ui` | egui interface: options bar, toolbar, document tabs (egui_dock), Color/Properties/Layers panels |
| `app` | Executable entry point |

UI dimensions are defined in `crates/op-ui/src/theme.rs`; `UI_SCALE` controls the overall scale.

## Shortcuts

| Action | Shortcut |
|---|---|
| New / Open / Export PNG / Close | ⌘N / ⌘O / ⌘S / ⌘W |
| Zoom in / Zoom out / Fit on screen / 100% | ⌘= / ⌘- / ⌘0 / ⌘1 |
| New layer | ⇧⌘N |
| Pan | Space-drag, Hand tool (H), trackpad scroll |
| Zoom | Trackpad pinch, ⌘+scroll, Zoom tool (Z, ⌥ to zoom out) |
| Tools | V M L W C K I J B S Y E G O P T A U H Z |
| Default colors / Swap colors | D / X |

## Third-party assets

- UI font Source Sans 3 (SIL Open Font License, see `crates/op-ui/assets/fonts/OFL.txt`)
- Icons from Phosphor Icons (MIT)
