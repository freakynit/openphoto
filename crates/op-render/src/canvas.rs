use std::collections::HashMap;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use egui_wgpu::{CallbackResources, CallbackTrait, ScreenDescriptor};

/// Composite image to display (tightly packed straight RGBA8).
pub struct CanvasImage {
    /// Texture slot key, one per document.
    pub key: u64,
    /// Content revision; the texture is re-uploaded when it changes.
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// View transform.
#[derive(Clone, Copy, Debug)]
pub struct CanvasView {
    /// Screen position of the document's top-left corner, in points.
    pub origin: [f32; 2],
    /// Zoom: **physical pixels** per document pixel.
    /// As in Photoshop, 100% means one image pixel per screen pixel.
    pub zoom: f32,
    pub pixel_grid: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    origin_zoom: [f32; 4],
    doc: [f32; 4],
}

struct Slot {
    revision: u64,
    mip_count: u32,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

struct CanvasResources {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    slots: HashMap<u64, Slot>,
}

/// Creates the pipeline and registers it in egui-wgpu's callback resources.
/// Call once at startup.
pub fn install(render_state: &egui_wgpu::RenderState) {
    let device = &render_state.device;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("canvas"),
        source: wgpu::ShaderSource::Wgsl(include_str!("canvas.wgsl").into()),
    });

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("canvas"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("canvas"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("canvas"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(render_state.target_format.into())],
        }),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("canvas"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(CanvasResources {
            pipeline,
            layout,
            sampler,
            slots: HashMap::new(),
        });
}

/// Builds an egui paint command that draws the canvas within `rect`.
pub fn paint_callback(
    rect: egui::Rect,
    image: Arc<CanvasImage>,
    view: CanvasView,
) -> egui::PaintCallback {
    egui_wgpu::Callback::new_paint_callback(rect, CanvasCallback { image, view })
}

struct CanvasCallback {
    image: Arc<CanvasImage>,
    view: CanvasView,
}

impl CallbackTrait for CanvasCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let res: &mut CanvasResources = resources.get_mut().expect("canvas renderer installed");
        let img = &self.image;

        let stale = res
            .slots
            .get(&img.key)
            .is_none_or(|s| s.revision != img.revision);
        if stale {
            let slot = create_slot(device, queue, res, img);
            res.slots.insert(img.key, slot);
        }
        let slot = &res.slots[&img.key];

        let ppp = screen.pixels_per_point;
        let u = Uniforms {
            origin_zoom: [
                self.view.origin[0] * ppp,
                self.view.origin[1] * ppp,
                self.view.zoom,
                (8.0 * ppp).round(),
            ],
            doc: [
                img.width as f32,
                img.height as f32,
                (slot.mip_count - 1) as f32,
                self.view.pixel_grid as u32 as f32,
            ],
        };
        queue.write_buffer(&slot.uniforms, 0, bytemuck::bytes_of(&u));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let res: &CanvasResources = resources.get().expect("canvas renderer installed");
        let Some(slot) = res.slots.get(&self.image.key) else {
            return;
        };
        pass.set_pipeline(&res.pipeline);
        pass.set_bind_group(0, &slot.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

fn create_slot(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    res: &CanvasResources,
    img: &CanvasImage,
) -> Slot {
    let max_dim = device.limits().max_texture_dimension_2d;
    let mut levels = build_mips(img, max_dim);
    if levels.is_empty() {
        levels.push(MipLevel {
            width: 1,
            height: 1,
            data: vec![0; 4],
        });
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("canvas"),
        size: wgpu::Extent3d {
            width: levels[0].width,
            height: levels[0].height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Not an sRGB format: Photoshop blends and displays in gamma-encoded space
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, level) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: i as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &level.data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(level.width * 4),
                rows_per_image: Some(level.height),
            },
            wgpu::Extent3d {
                width: level.width,
                height: level.height,
                depth_or_array_layers: 1,
            },
        );
    }

    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("canvas uniforms"),
        size: size_of::<Uniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&Default::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("canvas"),
        layout: &res.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&res.sampler),
            },
        ],
    });

    Slot {
        revision: img.revision,
        mip_count: levels.len() as u32,
        uniforms,
        bind_group,
    }
}

struct MipLevel {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

/// Premultiplies, then builds mips with a 2×2 box filter. Levels larger than
/// the GPU texture limit are skipped.
fn build_mips(img: &CanvasImage, max_dim: u32) -> Vec<MipLevel> {
    if img.width == 0 || img.height == 0 {
        return Vec::new();
    }
    let premul: Vec<u8> = img
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3] as u32;
            let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            [m(p[0]), m(p[1]), m(p[2]), p[3]]
        })
        .collect();

    let mut levels = Vec::new();
    let mut cur = MipLevel {
        width: img.width,
        height: img.height,
        data: premul,
    };
    loop {
        let next = (cur.width > 1 || cur.height > 1).then(|| downsample(&cur));
        if cur.width <= max_dim && cur.height <= max_dim {
            levels.push(cur);
        }
        match next {
            Some(n) => cur = n,
            None => break,
        }
    }
    levels
}

fn downsample(src: &MipLevel) -> MipLevel {
    let (w, h) = ((src.width / 2).max(1), (src.height / 2).max(1));
    let mut data = vec![0u8; (w * h * 4) as usize];
    let sw = src.width as usize;
    for y in 0..h as usize {
        let y0 = (y * 2).min(src.height as usize - 1);
        let y1 = (y * 2 + 1).min(src.height as usize - 1);
        for x in 0..w as usize {
            let x0 = (x * 2).min(sw - 1);
            let x1 = (x * 2 + 1).min(sw - 1);
            for c in 0..4 {
                let s = |xx: usize, yy: usize| src.data[(yy * sw + xx) * 4 + c] as u32;
                let sum = s(x0, y0) + s(x1, y0) + s(x0, y1) + s(x1, y1);
                data[(y * w as usize + x) * 4 + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    MipLevel {
        width: w,
        height: h,
        data,
    }
}
