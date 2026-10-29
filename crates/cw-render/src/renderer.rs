use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::data::Scene;
use crate::mesh::{self, Mesh, Vertex, VERTEX_BYTES};
use crate::palette::{ramp, Palette, COLOURS, STEPS};

const SHADER: &str = include_str!("shaders/scene.wgsl");
const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Everything that changes how a frame looks, apart from the scene and the camera.
#[derive(Clone, Debug)]
pub struct Style {
    pub night: bool,
    /// Render at 1/n size and repeat each pixel n times, for chunky 8-bit pixels.
    pub pixel_scale: u32,
    pub shadow_size: u32,
    /// Compass bearing the light comes from, and its height above the horizon, in degrees.
    pub sun_azimuth_deg: f32,
    pub sun_elevation_deg: f32,
    pub clouds: bool,
    pub buildings: bool,
    /// Cloud image alpha above which a pixel is cloud, and above which it is the bright core.
    pub cloud_threshold: f32,
    pub cloud_core: f32,
    pub cloud_altitude_km: f32,
    /// Ordered dither strength between ground classes (0 is off).
    pub dither: f32,
}

impl Default for Style {
    fn default() -> Style {
        Style {
            night: false,
            pixel_scale: 1,
            shadow_size: 4096,
            sun_azimuth_deg: 305.0,
            sun_elevation_deg: 34.0,
            clouds: true,
            buildings: true,
            cloud_threshold: 0.86,
            cloud_core: 0.97,
            cloud_altitude_km: 18.0,
            dither: 0.12,
        }
    }
}

impl Style {
    pub fn night() -> Style {
        Style { night: true, sun_azimuth_deg: 40.0, sun_elevation_deg: 38.0, ..Style::default() }
    }
    fn light_dir(&self) -> Vec3 {
        let (az, el) = (self.sun_azimuth_deg.to_radians(), self.sun_elevation_deg.to_radians());
        Vec3::new(az.sin() * el.cos(), az.cos() * el.cos(), el.sin())
    }
}

#[derive(Debug)]
pub struct NoAdapter(pub String);

impl std::fmt::Display for NoAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no usable graphics adapter: {}", self.0)
    }
}
impl std::error::Error for NoAdapter {}

#[derive(Clone, Debug)]
pub struct AdapterSummary {
    pub name: String,
    pub backend: String,
    pub device_type: String,
    pub driver: String,
}

/// Counts and timing of one frame. Times are wall clock around submit and wait, so on a
/// software adapter they are only good for relative comparison.
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    pub width: u32,
    pub height: u32,
    pub vertices: usize,
    pub triangles: usize,
    pub draw_calls: u32,
    pub gpu_wait: Duration,
    pub readback: Duration,
}

pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, top row first. Alpha is always 255.
    pub rgba: Vec<u8>,
    pub stats: FrameStats,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_vp: [[f32; 4]; 4],
    sun: [f32; 4],
    proj: [f32; 4],
    earth: [f32; 4],
    mask_b: [f32; 4],
    mask_dim: [f32; 4],
    cloud: [f32; 4],
    cloud2: [f32; 4],
    palette: [[f32; 4]; COLOURS],
    day: [[f32; 4]; COLOURS],
}

struct GpuMesh {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    indices: u32,
    vertices: usize,
    triangles: usize,
}

struct GpuScene {
    id: *const Scene,
    terrain: GpuMesh,
    buildings: GpuMesh,
    clouds: GpuMesh,
    cloud_altitude: f32,
    earth: wgpu::TextureView,
    cloud_tex: wgpu::TextureView,
    mask: wgpu::TextureView,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: AdapterSummary,
    layout: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    compare: wgpu::Sampler,
    shadow_pipeline: wgpu::RenderPipeline,
    main_pipeline: wgpu::RenderPipeline,
    dummy_shadow: wgpu::TextureView,
    uniforms: wgpu::Buffer,
    cache: Option<GpuScene>,
}

fn wgsl_prelude() -> String {
    format!(
        "const STEPS: u32 = {STEPS}u;\nconst PALETTE_LEN: u32 = {COLOURS}u;\n\
         const RAMP_INK: u32 = {}u;\nconst RAMP_WATER: u32 = {}u;\nconst RAMP_FOREST: u32 = {}u;\n\
         const RAMP_FIELD: u32 = {}u;\nconst RAMP_SOIL: u32 = {}u;\nconst RAMP_ROCK: u32 = {}u;\n\
         const RAMP_SNOW: u32 = {}u;\nconst RAMP_URBAN: u32 = {}u;\nconst RAMP_GOLD: u32 = {}u;\n\
         const RAMP_CLOUD: u32 = {}u;\nconst RAMP_BRICK: u32 = {}u;\nconst RAMP_SIGNAL: u32 = {}u;\n",
        ramp::INK, ramp::WATER, ramp::FOREST, ramp::FIELD, ramp::SOIL, ramp::ROCK, ramp::SNOW,
        ramp::URBAN, ramp::GOLD, ramp::CLOUD, ramp::BRICK, ramp::SIGNAL
    )
}

impl Renderer {
    /// Find an adapter and build the pipelines. Fails cleanly when the machine has no
    /// graphics adapter at all (CI), so callers and tests can skip.
    pub fn new() -> Result<Renderer, NoAdapter> {
        let instance = wgpu::Instance::default();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(|e| NoAdapter(e.to_string()))?;
        let ai = adapter.get_info();
        let info = AdapterSummary {
            name: ai.name.clone(),
            backend: format!("{:?}", ai.backend),
            device_type: format!("{:?}", ai.device_type),
            driver: format!("{} {}", ai.driver, ai.driver_info).trim().to_string(),
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("cw-render"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| NoAdapter(e.to_string()))?;

        let source = format!("{}{}", wgsl_prelude(), SHADER);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let tex_entry = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty,
            count: None,
        };
        let tex2d = |sample_type| wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene"),
            entries: &[
                tex_entry(
                    0,
                    wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                ),
                tex_entry(1, tex2d(wgpu::TextureSampleType::Float { filterable: true })),
                tex_entry(2, tex2d(wgpu::TextureSampleType::Float { filterable: true })),
                tex_entry(3, tex2d(wgpu::TextureSampleType::Float { filterable: false })),
                tex_entry(4, tex2d(wgpu::TextureSampleType::Depth)),
                tex_entry(5, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
                tex_entry(6, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Float32x3];
        let vbl = wgpu::VertexBufferLayout {
            array_stride: VERTEX_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        };
        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        };
        let depth = |bias: wgpu::DepthBiasState| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias,
        };
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: &[Some(vbl.clone())],
            },
            primitive,
            depth_stencil: Some(depth(wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 })),
            multisample: wgpu::MultisampleState::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        let main_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("main"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vbl)],
            },
            primitive,
            depth_stencil: Some(depth(wgpu::DepthBiasState::default())),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOUR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let compare = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let dummy_shadow = depth_texture(&device, 1, "dummy shadow").create_view(&Default::default());
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(Renderer {
            device,
            queue,
            info,
            layout,
            nearest,
            compare,
            shadow_pipeline,
            main_pipeline,
            dummy_shadow,
            uniforms,
            cache: None,
        })
    }

    pub fn adapter(&self) -> &AdapterSummary {
        &self.info
    }

    fn upload_mesh(&self, m: &Mesh, label: &str) -> GpuMesh {
        // Empty meshes still get a one-vertex buffer so buffers are never zero sized.
        let vertices = if m.vertices.is_empty() { vec![Vertex::zeroed()] } else { m.vertices.clone() };
        let indices = if m.indices.is_empty() { vec![0u32] } else { m.indices.clone() };
        GpuMesh {
            vb: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            ib: self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            indices: m.indices.len() as u32,
            vertices: m.vertices.len(),
            triangles: m.triangles(),
        }
    }

    fn rgba_texture(&self, w: u32, h: u32, data: &[u8], label: &str) -> wgpu::TextureView {
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        );
        tex.create_view(&Default::default())
    }

    fn prepare(&mut self, scene: &Scene, style: &Style) {
        let id = scene as *const Scene;
        if let Some(c) = &self.cache {
            if c.id == id && (c.cloud_altitude - style.cloud_altitude_km).abs() < 1e-6 {
                return;
            }
        }
        let gpu = GpuScene {
            id,
            terrain: self.upload_mesh(&mesh::terrain(scene), "terrain"),
            buildings: self.upload_mesh(&mesh::buildings(scene), "buildings"),
            clouds: self.upload_mesh(&mesh::cloud_sheet(scene, style.cloud_altitude_km), "clouds"),
            cloud_altitude: style.cloud_altitude_km,
            earth: self.rgba_texture(scene.earth.w, scene.earth.h, &scene.earth.rgba, "earth"),
            cloud_tex: self.rgba_texture(scene.clouds.w, scene.clouds.h, &scene.clouds.rgba, "clouds"),
            mask: self.rgba_texture(scene.mask.w as u32, scene.mask.h as u32, &scene.mask.rgba, "mask"),
        };
        self.cache = Some(gpu);
    }

    /// Render the scene from the camera and read it back. The result is deterministic for a
    /// given adapter and driver.
    pub fn render_to_rgba(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
    ) -> Frame {
        self.prepare(scene, style);
        let ps = style.pixel_scale.max(1);
        let (rw, rh) = ((width / ps).max(1), (height / ps).max(1));
        let aspect = rw as f32 / rh as f32;

        let (tx, ty) = scene.projection.to_km(camera.lon, camera.lat);
        let view = camera.view((tx, ty), 0.0, aspect);
        let light = style.light_dir();
        let (light_vp, texel_km) = light_matrix(scene, &view, light, style.shadow_size);
        let palette = if style.night { Palette::night() } else { Palette::day() };
        let km_lon = scene.projection.km_per_deg_lon();
        let em = scene.earth.bounds;
        let mb = scene.mask.bounds;
        // The cloud shadow lands displaced from the cloud, away from the light.
        let fall = style.cloud_altitude_km / light.z.max(0.2);
        let (sx, sy) = (-light.x * fall, -light.y * fall);
        let globals = Globals {
            view_proj: view.view_proj.to_cols_array_2d(),
            light_vp: light_vp.to_cols_array_2d(),
            sun: [light.x, light.y, light.z, if style.night { 1.0 } else { 0.0 }],
            proj: [scene.projection.lon0, scene.projection.lat0, km_lon, crate::data::KM_PER_DEG_LAT],
            earth: [em.west, em.south, em.east, em.north],
            mask_b: [mb.west, mb.south, mb.east, mb.north],
            mask_dim: [scene.mask.w as f32, scene.mask.h as f32, scene.mask.px_per_degree(), style.dither],
            cloud: [style.cloud_threshold, style.cloud_core, style.cloud_altitude_km, if style.clouds { 1.0 } else { 0.0 }],
            cloud2: [sx / km_lon, sy / crate::data::KM_PER_DEG_LAT, texel_km, 0.0],
            palette: palette.flat(),
            day: Palette::day().flat(),
        };
        self.queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&globals));

        let gpu = self.cache.as_ref().expect("prepared");
        let shadow_tex = depth_texture(&self.device, style.shadow_size, "shadow");
        let shadow_view = shadow_tex.create_view(&Default::default());
        let group = |shadow: &wgpu::TextureView| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: self.uniforms.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&gpu.earth) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&gpu.cloud_tex) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&gpu.mask) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(shadow) },
                    wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.nearest) },
                    wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.compare) },
                ],
            })
        };
        let shadow_group = group(&self.dummy_shadow);
        let main_group = group(&shadow_view);

        let colour = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("colour"),
            size: wgpu::Extent3d { width: rw, height: rh, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOUR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let colour_view = colour.create_view(&Default::default());
        let depth_view = depth_texture_sized(&self.device, rw, rh, "depth").create_view(&Default::default());
        let padded = (rw * 4).div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * rh) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut stats = FrameStats { width: rw * ps, height: rh * ps, ..Default::default() };
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &shadow_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.shadow_pipeline);
            pass.set_bind_group(0, Some(&shadow_group), &[]);
            let mut meshes = vec![&gpu.terrain];
            if style.buildings {
                meshes.push(&gpu.buildings);
            }
            for m in meshes {
                draw(&mut pass, m, &mut stats, false);
            }
        }
        {
            let ink = Palette::day().ramps[ramp::INK][0];
            let ink = if style.night { Palette::night().ramps[ramp::INK][0] } else { ink };
            let clear = wgpu::Color {
                r: ink[0] as f64 / 255.0,
                g: ink[1] as f64 / 255.0,
                b: ink[2] as f64 / 255.0,
                a: 1.0,
            };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &colour_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.main_pipeline);
            pass.set_bind_group(0, Some(&main_group), &[]);
            let mut meshes = vec![&gpu.terrain];
            if style.buildings {
                meshes.push(&gpu.buildings);
            }
            if style.clouds {
                meshes.push(&gpu.clouds);
            }
            for m in meshes {
                draw(&mut pass, m, &mut stats, true);
            }
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &colour, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(rh) },
            },
            wgpu::Extent3d { width: rw, height: rh, depth_or_array_layers: 1 },
        );
        let t0 = Instant::now();
        self.queue.submit([enc.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
        self.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        stats.gpu_wait = t0.elapsed();
        let t1 = Instant::now();
        let data = slice.get_mapped_range().expect("mapped");
        let (ow, oh) = (rw * ps, rh * ps);
        let mut rgba = vec![255u8; (ow * oh * 4) as usize];
        for y in 0..oh {
            let row = &data[((y / ps) * padded) as usize..];
            for x in 0..ow {
                let s = ((x / ps) * 4) as usize;
                let d = ((y * ow + x) * 4) as usize;
                rgba[d..d + 3].copy_from_slice(&row[s..s + 3]);
            }
        }
        drop(data);
        readback.unmap();
        stats.readback = t1.elapsed();
        stats.width = ow;
        stats.height = oh;
        Frame { width: ow, height: oh, rgba, stats }
    }
}

fn draw(pass: &mut wgpu::RenderPass<'_>, m: &GpuMesh, stats: &mut FrameStats, count: bool) {
    pass.set_vertex_buffer(0, m.vb.slice(..));
    pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint32);
    pass.draw_indexed(0..m.indices, 0, 0..1);
    stats.draw_calls += 1;
    if count {
        stats.vertices += m.vertices;
        stats.triangles += m.triangles;
    }
}

fn depth_texture(device: &wgpu::Device, size: u32, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

fn depth_texture_sized(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

/// Orthographic light matrix that covers the ground the camera sees, plus its texel size in km.
fn light_matrix(scene: &Scene, view: &crate::camera::View, light: Vec3, size: u32) -> (Mat4, f32) {
    // Ground footprint of the screen corners (clamped to a sane distance for oblique views).
    let mut pts: Vec<Vec3> = Vec::new();
    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let near = view.inv_view_proj * Vec4::new(x, y, 0.0, 1.0);
        let far = view.inv_view_proj * Vec4::new(x, y, 1.0, 1.0);
        let (a, b) = (near.truncate() / near.w, far.truncate() / far.w);
        let dir = (b - a).normalize();
        let t = if dir.z < -0.05 { (-a.z / dir.z).min(view.far * 0.6) } else { view.far * 0.6 };
        pts.push(a + dir * t);
    }
    let zmax = mesh::level_z_km(scene, 40) + 60.0;
    let mut corners = Vec::new();
    for p in &pts {
        corners.push(Vec3::new(p.x, p.y, 0.0));
        corners.push(Vec3::new(p.x, p.y, zmax));
    }
    let centre = corners.iter().fold(Vec3::ZERO, |a, b| a + *b) / corners.len() as f32;
    let lv = Mat4::look_at_rh(centre + light * 2000.0, centre, Vec3::Z);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for c in &corners {
        let p = lv.transform_point3(*c);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    // Square the xy extent so texels are square, and pad for the edge.
    let half = ((hi.x - lo.x).max(hi.y - lo.y) / 2.0) * 1.04 + 5.0;
    let (cx, cy) = ((hi.x + lo.x) / 2.0, (hi.y + lo.y) / 2.0);
    let proj = Mat4::orthographic_rh(cx - half, cx + half, cy - half, cy + half, -hi.z - 30.0, -lo.z + 30.0);
    (proj * lv, 2.0 * half / size as f32)
}
