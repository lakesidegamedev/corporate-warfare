use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use crate::camera::Camera;
use crate::data::Scene;
use crate::mesh::{self, Mesh, VERTEX_BYTES, Vertex};
use crate::palette::{COLOURS, Palette, STEPS, ramp};

const SHADER: &str = include_str!("shaders/scene.wgsl");
/// The offscreen target's format. A window surface uses its own (see `SurfaceState`).
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
    /// The 1.8 cloud setting: off, thin, light (the default) or heavy.
    pub clouds: CloudLevel,
    pub buildings: bool,
    pub roads: bool,
    /// Camera width in km below which the gold centre dashes and the sleepers show.
    pub road_detail_km: f32,
    /// Smallest and largest ground texture texel in km. A texel is always drawn at least
    /// `texel_min_px` screen pixels wide: the texture steps up in powers of two as the camera
    /// pulls back and is gone (flat class colour) once the step would pass the largest texel.
    pub texel_km: (f32, f32),
    pub texel_min_px: f32,
    pub cloud_altitude_km: f32,
    /// Half width in screen pixels of the dithered band between two ground classes.
    pub dither: f32,
    /// Camera width in km below which clouds are gone, and above which they are at full
    /// coverage. Between the two the cloud threshold rises smoothly (the 1.8 rule: clouds
    /// clear as you zoom in, so a close look is never through haze).
    pub cloud_clear_km: (f32, f32),
    /// Palette ramp that map modes (region levels) are painted in.
    pub heat_ramp: usize,
}

/// The 1.8 cloud setting levels. Each is a threshold on the cloud image: a higher one leaves
/// fewer, tighter clouds so the land shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudLevel {
    Off,
    Thin,
    Light,
    Heavy,
}

impl CloudLevel {
    /// Cloud image alpha above which a pixel is cloud, and above which it is the bright core.
    fn thresholds(self) -> (f32, f32) {
        match self {
            CloudLevel::Off => (2.0, 2.0),
            CloudLevel::Thin => (0.9955, 0.9995),
            CloudLevel::Light => (0.9915, 0.998),
            CloudLevel::Heavy => (0.92, 0.97),
        }
    }
}

impl Default for Style {
    fn default() -> Style {
        Style {
            night: false,
            pixel_scale: 1,
            shadow_size: 4096,
            sun_azimuth_deg: 305.0,
            sun_elevation_deg: 34.0,
            clouds: CloudLevel::Light,
            buildings: true,
            roads: true,
            road_detail_km: 450.0,
            texel_km: (0.5, 4.0),
            texel_min_px: 2.0,
            cloud_altitude_km: 18.0,
            dither: 2.5,
            cloud_clear_km: (600.0, 1800.0),
            heat_ramp: ramp::BRICK,
        }
    }
}

impl Style {
    pub fn night() -> Style {
        Style {
            night: true,
            sun_azimuth_deg: 40.0,
            sun_elevation_deg: 38.0,
            ..Style::default()
        }
    }
    fn light_dir(&self) -> Vec3 {
        let (az, el) = (
            self.sun_azimuth_deg.to_radians(),
            self.sun_elevation_deg.to_radians(),
        );
        Vec3::new(az.sin() * el.cos(), az.cos() * el.cos(), el.sin())
    }
}

/// Where a surface renderer presents: a native window the host owns.
#[derive(Clone, Copy, Debug)]
pub enum SurfaceTarget {
    /// A `CAMetalLayer` (Apple).
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    MetalLayer(*mut std::ffi::c_void),
    /// A Windows window handle (`HWND`).
    Win32(isize),
    /// An X11 window id and the `Display*` it belongs to.
    #[cfg(unix)]
    Xlib {
        window: u64,
        display: *mut std::ffi::c_void,
    },
}

impl SurfaceTarget {
    fn into_wgpu(self) -> Result<wgpu::SurfaceTargetUnsafe, NoAdapter> {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
        };
        #[cfg(unix)]
        use raw_window_handle::{XlibDisplayHandle, XlibWindowHandle};
        let bad = |what: &str| NoAdapter(format!("no {what} to draw into"));
        Ok(match self {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            SurfaceTarget::MetalLayer(layer) => {
                wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer)
            }
            SurfaceTarget::Win32(hwnd) => {
                let hwnd = std::num::NonZeroIsize::new(hwnd).ok_or_else(|| bad("window"))?;
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(
                        RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                    ),
                    raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
                }
            }
            #[cfg(unix)]
            SurfaceTarget::Xlib { window, display } => {
                let display = std::ptr::NonNull::new(display).ok_or_else(|| bad("display"))?;
                if window == 0 {
                    return Err(bad("window"));
                }
                wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: Some(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
                        Some(display),
                        0,
                    ))),
                    raw_window_handle: RawWindowHandle::Xlib(XlibWindowHandle::new(
                        window as std::ffi::c_ulong,
                    )),
                }
            }
        })
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
    /// Surface frames: time waiting for the window to hand over its next image (the display's
    /// refresh, when frames are queued) and time in the present call.
    pub acquire: Duration,
    pub present: Duration,
    /// A surface frame that was not drawn because the window could not take one (hidden,
    /// minimised, timed out). Always false for offscreen renders.
    pub skipped: bool,
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
    road: [f32; 4],
    tex: [f32; 4],
    ground_b: [f32; 4],
    ground_dim: [f32; 4],
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

/// The road ribbons of one zoom band. The index buffer is rewritten each frame with only the
/// tiles in view, so the road pass stays a single draw call.
struct GpuRoads {
    vb: wgpu::Buffer,
    ib: wgpu::Buffer,
    mesh: mesh::RoadMesh,
}

struct GpuScene {
    /// Address of the scene it was built from; identity only, never dereferenced.
    id: usize,
    terrain: GpuMesh,
    /// The land and sea beyond the terrain's edge; rebuilt when the season moves the snow line.
    apron: GpuMesh,
    buildings: GpuMesh,
    clouds: GpuMesh,
    /// Road ribbons per zoom band, built the first time a camera needs the band.
    roads: [Option<GpuRoads>; 3],
    cloud_altitude: f32,
    season_rev: u64,
    mask_rev: u64,
    buildings_rev: u64,
    earth: wgpu::TextureView,
    cloud_tex: wgpu::TextureView,
    mask: wgpu::TextureView,
    ground: wgpu::TextureView,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    info: AdapterSummary,
    layout: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
    compare: wgpu::Sampler,
    shadow_pipeline: wgpu::RenderPipeline,
    main_pipeline: wgpu::RenderPipeline,
    road_pipeline: wgpu::RenderPipeline,
    dummy_shadow: wgpu::TextureView,
    uniforms: wgpu::Buffer,
    cache: Option<GpuScene>,
    targets: Option<Targets>,
    surface: Option<SurfaceState>,
}

/// A window surface the renderer presents to, with the linear format its pipelines were built for.
struct SurfaceState {
    surface: wgpu::Surface<'static>,
    format: wgpu::TextureFormat,
    alpha: wgpu::CompositeAlphaMode,
    size: Option<(u32, u32)>,
}

/// Render targets kept between frames: a frame allocates nothing while the size stays.
struct Targets {
    width: u32,
    height: u32,
    shadow_size: u32,
    depth_view: wgpu::TextureView,
    shadow_view: wgpu::TextureView,
    /// The offscreen colour target and its readback buffer; absent when drawing to a surface.
    offscreen: Option<Offscreen>,
}

struct Offscreen {
    colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded: u32,
}

fn wgsl_prelude() -> String {
    format!(
        "const STEPS: u32 = {STEPS}u;\nconst PALETTE_LEN: u32 = {COLOURS}u;\n\
         const RAMP_INK: u32 = {}u;\nconst RAMP_WATER: u32 = {}u;\nconst RAMP_FOREST: u32 = {}u;\n\
         const RAMP_FIELD: u32 = {}u;\nconst RAMP_SOIL: u32 = {}u;\nconst RAMP_ROCK: u32 = {}u;\n\
         const RAMP_SNOW: u32 = {}u;\nconst RAMP_URBAN: u32 = {}u;\nconst RAMP_GOLD: u32 = {}u;\n\
         const RAMP_CLOUD: u32 = {}u;\nconst RAMP_BRICK: u32 = {}u;\nconst RAMP_SIGNAL: u32 = {}u;\n",
        ramp::INK,
        ramp::WATER,
        ramp::FOREST,
        ramp::FIELD,
        ramp::SOIL,
        ramp::ROCK,
        ramp::SNOW,
        ramp::URBAN,
        ramp::GOLD,
        ramp::CLOUD,
        ramp::BRICK,
        ramp::SIGNAL
    )
}

impl Renderer {
    /// Find an adapter and build the pipelines. Fails cleanly when the machine has no
    /// graphics adapter at all (CI), so callers and tests can skip.
    pub fn new() -> Result<Renderer, NoAdapter> {
        Self::create(None)
    }

    /// A renderer that draws straight into a `CAMetalLayer` (iOS, Mac Catalyst, macOS) and
    /// presents each frame, with no readback. Use [`Renderer::render_to_surface`].
    ///
    /// # Safety
    /// `layer` must be a valid `CAMetalLayer` that outlives the renderer.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub unsafe fn for_metal_layer(layer: *mut std::ffi::c_void) -> Result<Renderer, NoAdapter> {
        Self::create(Some(SurfaceTarget::MetalLayer(layer)))
    }

    /// A renderer that draws straight into a native desktop window (a Windows `HWND` or an X11
    /// window) and presents each frame at the display's rate, with no readback. Use
    /// [`Renderer::render_to_surface`].
    ///
    /// # Safety
    /// The window (and the X display) must stay valid until the renderer is dropped.
    pub unsafe fn for_window(target: SurfaceTarget) -> Result<Renderer, NoAdapter> {
        Self::create(Some(target))
    }

    fn create(target: Option<SurfaceTarget>) -> Result<Renderer, NoAdapter> {
        let instance = wgpu::Instance::default();
        let surface: Option<wgpu::Surface<'static>> = match target {
            None => None,
            Some(target) => Some(
                unsafe { instance.create_surface_unsafe(target.into_wgpu()?) }
                    .map_err(|e| NoAdapter(e.to_string()))?,
            ),
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: surface.as_ref(),
            apply_limit_buckets: false,
        }))
        .map_err(|e| NoAdapter(e.to_string()))?;
        // The pipelines write the shader's colours as they are, so the surface needs a linear
        // (non sRGB) format; Bgra8Unorm is the one every Metal layer has.
        let surface = match surface {
            None => None,
            Some(surface) => {
                let caps = surface.get_capabilities(&adapter);
                let format = [
                    wgpu::TextureFormat::Bgra8Unorm,
                    wgpu::TextureFormat::Rgba8Unorm,
                ]
                .into_iter()
                .find(|f| caps.formats.contains(f))
                .ok_or_else(|| NoAdapter("the display has no linear colour format".into()))?;
                Some(SurfaceState {
                    surface,
                    format,
                    alpha: caps
                        .alpha_modes
                        .first()
                        .copied()
                        .unwrap_or(wgpu::CompositeAlphaMode::Auto),
                    size: None,
                })
            }
        };
        let colour_format = surface.as_ref().map_or(COLOUR_FORMAT, |s| s.format);
        let ai = adapter.get_info();
        let info = AdapterSummary {
            name: ai.name.clone(),
            backend: format!("{:?}", ai.backend),
            device_type: format!("{:?}", ai.device_type),
            driver: format!("{} {}", ai.driver, ai.driver_info)
                .trim()
                .to_string(),
        };
        let request = |limits: wgpu::Limits| {
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("cw-render"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                ..Default::default()
            }))
        };
        // The standard limits first; an adapter that offers a little less (the iOS simulator's
        // Metal has fewer shader varyings than the default asks for) gets what it has, which the
        // scene fits in: the shaders pass six varyings and bind nine resources.
        let (device, queue) =
            match request(wgpu::Limits::default().using_resolution(adapter.limits())) {
                Ok(d) => d,
                Err(_) => request(adapter.limits()).map_err(|e| NoAdapter(e.to_string()))?,
            };

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
                tex_entry(
                    1,
                    tex2d(wgpu::TextureSampleType::Float { filterable: true }),
                ),
                tex_entry(
                    2,
                    tex2d(wgpu::TextureSampleType::Float { filterable: true }),
                ),
                tex_entry(
                    3,
                    tex2d(wgpu::TextureSampleType::Float { filterable: false }),
                ),
                tex_entry(4, tex2d(wgpu::TextureSampleType::Depth)),
                tex_entry(
                    5,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
                tex_entry(
                    7,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                ),
                tex_entry(
                    8,
                    tex2d(wgpu::TextureSampleType::Float { filterable: false }),
                ),
                tex_entry(
                    6,
                    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                ),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Float32x3, 4 => Float32x2];
        let vbl = wgpu::VertexBufferLayout {
            array_stride: VERTEX_BYTES as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        };
        let vbl_road = vbl.clone();
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
            depth_stencil: Some(depth(wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            })),
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
                buffers: &[Some(vbl.clone())],
            },
            primitive,
            depth_stencil: Some(depth(wgpu::DepthBiasState::default())),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: colour_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let road_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("roads"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_road"),
                compilation_options: Default::default(),
                buffers: &[Some(vbl_road)],
            },
            primitive,
            depth_stencil: Some(depth(wgpu::DepthBiasState::default())),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_road"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: colour_format,
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
        let linear = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let compare = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let dummy_shadow =
            depth_texture(&device, 1, "dummy shadow").create_view(&Default::default());
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
            linear,
            compare,
            shadow_pipeline,
            main_pipeline,
            road_pipeline,
            dummy_shadow,
            uniforms,
            cache: None,
            targets: None,
            surface,
        })
    }

    pub fn adapter(&self) -> &AdapterSummary {
        &self.info
    }

    fn upload_mesh(&self, m: &Mesh, label: &str) -> GpuMesh {
        upload(&self.device, m, label)
    }

    fn rgba_texture(&self, w: u32, h: u32, data: &[u8], label: &str) -> wgpu::TextureView {
        self.texture(w, h, data, label, wgpu::TextureFormat::Rgba8Unorm)
    }

    fn texture(
        &self,
        w: u32,
        h: u32,
        data: &[u8],
        label: &str,
        format: wgpu::TextureFormat,
    ) -> wgpu::TextureView {
        texture_of(&self.device, &self.queue, w, h, data, label, format)
    }
}

/// The ground class raster as a texture: class in red, crop row variant in green.
fn ground_texture(device: &wgpu::Device, queue: &wgpu::Queue, scene: &Scene) -> wgpu::TextureView {
    let data: Vec<u8> = scene
        .ground
        .classes
        .iter()
        .zip(&scene.ground.variant)
        .flat_map(|(c, v)| [*c, *v])
        .collect();
    texture_of(
        device,
        queue,
        scene.ground.w as u32,
        scene.ground.h as u32,
        &data,
        "ground",
        wgpu::TextureFormat::Rg8Unorm,
    )
}

fn texture_of(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    w: u32,
    h: u32,
    data: &[u8],
    label: &str,
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    {
        let tex = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            data,
        );
        tex.create_view(&Default::default())
    }
}

/// Receives the padded readback bytes, the row stride and the frame size; returns the size written.
type Finish<'a> = dyn FnMut(&[u8], u32, u32, u32) -> (u32, u32) + 'a;

impl Renderer {
    fn prepare(&mut self, scene: &Scene, style: &Style) {
        let id = scene as *const Scene as usize;
        if let Some(c) = &mut self.cache
            && c.id == id
            && (c.cloud_altitude - style.cloud_altitude_km).abs() < 1e-6
        {
            if c.buildings_rev != scene.buildings_rev {
                c.buildings_rev = scene.buildings_rev;
                let m = mesh::buildings(scene);
                c.buildings = upload(&self.device, &m, "buildings");
            }
            if c.season_rev != scene.season_rev {
                c.season_rev = scene.season_rev;
                c.apron = upload(&self.device, &mesh::apron(scene), "apron");
                c.ground = ground_texture(&self.device, &self.queue, scene);
            }
            if c.mask_rev != scene.mask_rev {
                c.mask_rev = scene.mask_rev;
                c.mask = texture_of(
                    &self.device,
                    &self.queue,
                    scene.mask.w as u32,
                    scene.mask.h as u32,
                    &scene.mask.rgba,
                    "mask",
                    wgpu::TextureFormat::Rgba8Unorm,
                );
            }
            return;
        }
        let gpu = GpuScene {
            id,
            terrain: self.upload_mesh(&mesh::terrain(scene), "terrain"),
            apron: self.upload_mesh(&mesh::apron(scene), "apron"),
            buildings: self.upload_mesh(&mesh::buildings(scene), "buildings"),
            season_rev: scene.season_rev,
            mask_rev: scene.mask_rev,
            buildings_rev: scene.buildings_rev,
            clouds: self.upload_mesh(&mesh::cloud_sheet(scene, style.cloud_altitude_km), "clouds"),
            roads: [None, None, None],
            cloud_altitude: style.cloud_altitude_km,
            earth: self.rgba_texture(scene.earth.w, scene.earth.h, &scene.earth.rgba, "earth"),
            cloud_tex: self.rgba_texture(
                scene.clouds.w,
                scene.clouds.h,
                &scene.clouds.rgba,
                "clouds",
            ),
            mask: self.rgba_texture(
                scene.mask.w as u32,
                scene.mask.h as u32,
                &scene.mask.rgba,
                "mask",
            ),
            ground: ground_texture(&self.device, &self.queue, scene),
        };
        self.cache = Some(gpu);
    }

    fn ensure_roads(&mut self, scene: &Scene, band: usize) {
        let built = self.cache.as_ref().is_some_and(|c| c.roads[band].is_some());
        if built {
            return;
        }
        let mesh = mesh::roads(scene, band);
        let vertices = if mesh.mesh.vertices.is_empty() {
            vec![Vertex::zeroed()]
        } else {
            mesh.mesh.vertices.clone()
        };
        let vb = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("roads"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let ib = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("roads"),
            size: (mesh.mesh.indices.len().max(1) * 4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if let Some(c) = &mut self.cache {
            c.roads[band] = Some(GpuRoads { vb, ib, mesh });
        }
    }

    /// Drop everything cached for the current scene (a new scenario was loaded).
    pub fn forget_scene(&mut self) {
        self.cache = None;
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
        let ps = style.pixel_scale.max(1);
        let mut rgba = Vec::new();
        let mut t_copy = Duration::ZERO;
        let mut stats = self.render_with(
            scene,
            camera,
            style,
            width,
            height,
            Some(&mut |data, rw, rh, padded| {
                let t1 = Instant::now();
                let (ow, oh) = (rw * ps, rh * ps);
                rgba = vec![255u8; (ow * oh * 4) as usize];
                for y in 0..oh {
                    let row = &data[((y / ps) * padded) as usize..];
                    for x in 0..ow {
                        let s = ((x / ps) * 4) as usize;
                        let d = ((y * ow + x) * 4) as usize;
                        rgba[d..d + 3].copy_from_slice(&row[s..s + 3]);
                    }
                }
                t_copy = t1.elapsed();
                (ow, oh)
            }),
        );
        stats.readback = t_copy;
        Frame {
            width: stats.width,
            height: stats.height,
            rgba,
            stats,
        }
    }

    /// Render at `width` x `height` divided by `style.pixel_scale` and write tightly packed RGBA8
    /// (alpha 255) into `out`, which must hold `(width / n) * (height / n) * 4` bytes. The
    /// caller scales the small image up if it wants the full size. Nothing is allocated per
    /// frame once the size is settled.
    pub fn render_into(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
        out: &mut [u8],
    ) -> FrameStats {
        let mut t_copy = Duration::ZERO;
        let mut stats = self.render_with(
            scene,
            camera,
            style,
            width,
            height,
            Some(&mut |data, rw, rh, padded| {
                let t1 = Instant::now();
                let row_bytes = (rw * 4) as usize;
                assert!(
                    out.len() >= row_bytes * rh as usize,
                    "output buffer too small"
                );
                for y in 0..rh as usize {
                    let src = &data[y * padded as usize..y * padded as usize + row_bytes];
                    out[y * row_bytes..(y + 1) * row_bytes].copy_from_slice(src);
                }
                // The target is cleared opaque and never blended, so alpha is already 255.
                t_copy = t1.elapsed();
                (rw, rh)
            }),
        );
        stats.readback = t_copy;
        stats.width = (width / style.pixel_scale.max(1)).max(1);
        stats.height = (height / style.pixel_scale.max(1)).max(1);
        stats
    }

    /// Draw the scene into the window surface and present it, at exactly `width` x `height`
    /// pixels (`style.pixel_scale` is ignored). Nothing is read back. `skipped` in the result
    /// says the window could not take a frame. Only for a renderer built with
    /// [`Renderer::for_metal_layer`].
    pub fn render_to_surface(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
    ) -> FrameStats {
        self.render_with(scene, camera, style, width, height, None)
    }

    fn render_with(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        style: &Style,
        width: u32,
        height: u32,
        finish: Option<&mut Finish>,
    ) -> FrameStats {
        // No `finish` means the frame goes to the window surface instead of a readback.
        let to_surface = finish.is_none();
        assert_eq!(
            to_surface,
            self.surface.is_some(),
            "a surface renderer draws only to its surface, an offscreen one only offscreen"
        );
        self.prepare(scene, style);
        let road_band = mesh::road_band(camera.width_km);
        if style.roads {
            self.ensure_roads(scene, road_band);
        }
        let ps = if to_surface {
            1
        } else {
            style.pixel_scale.max(1)
        };
        let (rw, rh) = ((width / ps).max(1), (height / ps).max(1));
        let mut acquire_time = Duration::ZERO;
        let surface_frame = if to_surface {
            let ta = Instant::now();
            let got = self.acquire(rw, rh);
            acquire_time = ta.elapsed();
            match got {
                Some(f) => Some(f),
                None => {
                    return FrameStats {
                        width: rw,
                        height: rh,
                        skipped: true,
                        ..Default::default()
                    };
                }
            }
        } else {
            None
        };
        let aspect = rw as f32 / rh as f32;

        let (tx, ty) = scene.projection.to_km(camera.lon, camera.lat);
        let view = camera.view(
            (tx, ty),
            mesh::camera_target_z(scene, tx, ty, camera.width_km),
            aspect,
        );
        let light = style.light_dir();
        let (light_vp, texel_km) = light_matrix(scene, &view, light, style.shadow_size);
        let palette = if style.night {
            Palette::night()
        } else {
            Palette::day()
        };
        let km_lon = scene.projection.km_per_deg_lon();
        let em = scene.earth.bounds;
        let mb = scene.mask.bounds;
        // The cloud shadow lands displaced from the cloud, away from the light.
        let fall = style.cloud_altitude_km / light.z.max(0.2);
        let (sx, sy) = (-light.x * fall, -light.y * fall);
        // Clouds clear as the camera closes in: the threshold rises toward "no cloud".
        let (near, far) = style.cloud_clear_km;
        let t = ((camera.width_km - near) / (far - near)).clamp(0.0, 1.0);
        let cover = t * t * (3.0 - 2.0 * t);
        let (thr0, core0) = style.clouds.thresholds();
        let cloud_thr = thr0 + (1.0 - cover) * (1.01 - thr0).max(0.0);
        let cloud_core = core0 + (1.0 - cover) * (1.02 - core0).max(0.0);
        let clouds_on = style.clouds != CloudLevel::Off;
        // Screen pixel size on the ground at the target, for the texture steps, and at unit
        // distance for the road widths (the vertex shader scales it by the vertex distance).
        let px_per_dist = 2.0 * (camera.fov_deg.to_radians() / 2.0).tan() / rh as f32;
        let target_dist = (view.eye - view.target).length();
        let px_km = px_per_dist * target_dist;
        let (tmin, tmax) = style.texel_km;
        let mut texel = tmin;
        while texel < style.texel_min_px * px_km && texel < tmax * 4.0 {
            texel *= 2.0;
        }
        let tex_on = if texel <= tmax { 1.0 } else { 0.0 };
        // Road widths in render pixels, one set per zoom band (0 hides the class). Interstates
        // are always there, main roads and railways join as the camera closes in, and every
        // width is a whole number of pixels, odd so the line has a centre pixel.
        let (inter_px, main_px, rail_px) = match camera.width_km {
            w if w > 1500.0 => (1.0, 0.0, 0.0),
            w if w > 900.0 => (3.0, 0.0, 0.0),
            w if w > 450.0 => (3.0, 0.0, 1.0),
            _ => (5.0, 1.0, 3.0),
        };
        let detail = if camera.width_km <= style.road_detail_km {
            1.0
        } else {
            0.0
        };
        let gb = scene.ground.bounds;
        let globals = Globals {
            view_proj: view.view_proj.to_cols_array_2d(),
            light_vp: light_vp.to_cols_array_2d(),
            sun: [
                light.x,
                light.y,
                light.z,
                if style.night { 1.0 } else { 0.0 },
            ],
            proj: [
                scene.projection.lon0,
                scene.projection.lat0,
                km_lon,
                crate::data::KM_PER_DEG_LAT,
            ],
            earth: [em.west, em.south, em.east, em.north],
            mask_b: [mb.west, mb.south, mb.east, mb.north],
            mask_dim: [
                scene.mask.w as f32,
                scene.mask.h as f32,
                scene.mask.px_per_degree(),
                style.dither,
            ],
            cloud: [
                cloud_thr,
                cloud_core,
                style.cloud_altitude_km,
                if clouds_on { 1.0 } else { 0.0 },
            ],
            cloud2: [
                sx / km_lon,
                sy / crate::data::KM_PER_DEG_LAT,
                texel_km,
                style.heat_ramp as f32,
            ],
            road: [inter_px, main_px, rail_px, detail],
            tex: [texel, tex_on, rw as f32, rh as f32],
            ground_b: [gb.west, gb.south, gb.east, gb.north],
            ground_dim: [
                scene.ground.w as f32,
                scene.ground.h as f32,
                scene.ground.ppd,
                0.0,
            ],
            palette: palette.flat(),
            day: Palette::day().flat(),
        };
        self.queue
            .write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&globals));

        let gpu = self.cache.as_ref().expect("prepared");
        // Road tiles in view: their indices are gathered into the band's index buffer.
        let mut visible_road_indices = 0u32;
        if style.roads
            && let Some(r) = &gpu.roads[road_band]
        {
            let mut gathered: Vec<u32> = Vec::new();
            for t in &r.mesh.tiles {
                if tile_in_view(&view.view_proj, t) {
                    let a = t.first as usize;
                    gathered.extend_from_slice(&r.mesh.mesh.indices[a..a + t.count as usize]);
                }
            }
            visible_road_indices = gathered.len() as u32;
            if !gathered.is_empty() {
                self.queue
                    .write_buffer(&r.ib, 0, bytemuck::cast_slice(&gathered));
            }
        }
        let stale = self.targets.as_ref().is_none_or(|t| {
            t.width != rw
                || t.height != rh
                || t.shadow_size != style.shadow_size
                || t.offscreen.is_some() == to_surface
        });
        if stale {
            let offscreen = if to_surface {
                None
            } else {
                let colour = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("colour"),
                    size: wgpu::Extent3d {
                        width: rw,
                        height: rh,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: COLOUR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                let padded = (rw * 4).div_ceil(256) * 256;
                Some(Offscreen {
                    colour_view: colour.create_view(&Default::default()),
                    colour,
                    readback: self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("readback"),
                        size: (padded * rh) as u64,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    padded,
                })
            };
            self.targets = Some(Targets {
                width: rw,
                height: rh,
                shadow_size: style.shadow_size,
                depth_view: depth_texture_sized(&self.device, rw, rh, "depth")
                    .create_view(&Default::default()),
                shadow_view: depth_texture(&self.device, style.shadow_size, "shadow")
                    .create_view(&Default::default()),
                offscreen,
            });
        }
        let targets = self.targets.as_ref().expect("targets");
        let shadow_view = &targets.shadow_view;
        let group = |shadow: &wgpu::TextureView| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scene"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&gpu.earth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&gpu.cloud_tex),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&gpu.mask),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(shadow),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: wgpu::BindingResource::Sampler(&self.nearest),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: wgpu::BindingResource::Sampler(&self.linear),
                    },
                    wgpu::BindGroupEntry {
                        binding: 8,
                        resource: wgpu::BindingResource::TextureView(&gpu.ground),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: wgpu::BindingResource::Sampler(&self.compare),
                    },
                ],
            })
        };
        let shadow_group = group(&self.dummy_shadow);
        let main_group = group(shadow_view);

        let surface_view = surface_frame
            .as_ref()
            .map(|f| f.texture.create_view(&Default::default()));
        let colour_view = match (&surface_view, &targets.offscreen) {
            (Some(v), _) => v,
            (None, Some(o)) => &o.colour_view,
            (None, None) => unreachable!("targets match the sink"),
        };
        let depth_view = &targets.depth_view;

        let mut stats = FrameStats {
            width: rw * ps,
            height: rh * ps,
            ..Default::default()
        };
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
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
            // Beyond the apron (the far horizon of a tilted view) is open sea, never a void: the
            // clear colour is the apron's own water step, so the world fades into the sea.
            let sea = if style.night {
                Palette::night().ramps[ramp::WATER][2]
            } else {
                Palette::day().ramps[ramp::WATER][2]
            };
            let clear = wgpu::Color {
                r: sea[0] as f64 / 255.0,
                g: sea[1] as f64 / 255.0,
                b: sea[2] as f64 / 255.0,
                a: 1.0,
            };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: colour_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.main_pipeline);
            pass.set_bind_group(0, Some(&main_group), &[]);
            let mut meshes = vec![&gpu.terrain, &gpu.apron];
            if style.buildings {
                meshes.push(&gpu.buildings);
            }
            for m in meshes {
                draw(&mut pass, m, &mut stats, true);
            }
            if style.roads
                && let Some(r) = &gpu.roads[road_band]
            {
                pass.set_pipeline(&self.road_pipeline);
                pass.set_vertex_buffer(0, r.vb.slice(..));
                pass.set_index_buffer(r.ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..visible_road_indices, 0, 0..1);
                stats.draw_calls += 1;
                stats.vertices += visible_road_indices as usize / 6 * 4;
                stats.triangles += visible_road_indices as usize / 3;
                pass.set_pipeline(&self.main_pipeline);
            }
            if clouds_on {
                draw(&mut pass, &gpu.clouds, &mut stats, true);
            }
        }
        let t0 = Instant::now();
        let Some(finish) = finish else {
            self.queue.submit([enc.finish()]);
            if let Some(frame) = surface_frame {
                let tp = Instant::now();
                self.queue.present(frame);
                stats.present = tp.elapsed();
                stats.acquire = acquire_time;
            }
            stats.gpu_wait = t0.elapsed();
            return stats;
        };
        let off = targets.offscreen.as_ref().expect("offscreen targets");
        let (colour, readback, padded) = (&off.colour, &off.readback, off.padded);
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: colour,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(rh),
                },
            },
            wgpu::Extent3d {
                width: rw,
                height: rh,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
        stats.gpu_wait = t0.elapsed();
        let data = slice.get_mapped_range().expect("mapped");
        let t1 = Instant::now();
        let (ow, oh) = finish(&data, rw, rh, padded);
        let finish_time = t1.elapsed();
        drop(data);
        readback.unmap();
        stats.readback = finish_time;
        stats.width = ow;
        stats.height = oh;
        stats
    }

    /// Configure the surface for this size when it changed, then take the next frame to draw
    /// into. `None` when the window cannot take a frame now (hidden, timed out, lost): the
    /// caller skips the frame and tries again later.
    fn acquire(&mut self, w: u32, h: u32) -> Option<wgpu::SurfaceTexture> {
        let s = self.surface.as_mut()?;
        let configure = |s: &SurfaceState| {
            s.surface.configure(
                &self.device,
                &wgpu::SurfaceConfiguration {
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    format: s.format,
                    color_space: wgpu::SurfaceColorSpace::Auto,
                    width: w,
                    height: h,
                    desired_maximum_frame_latency: 2,
                    present_mode: wgpu::PresentMode::Fifo,
                    alpha_mode: s.alpha,
                    view_formats: vec![],
                },
            );
        };
        if s.size != Some((w, h)) {
            configure(s);
            s.size = Some((w, h));
        }
        for _ in 0..2 {
            match s.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t) => return Some(t),
                wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                    // Drawn this once; the next frame reconfigures.
                    s.size = None;
                    return Some(t);
                }
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    configure(s);
                }
                _ => return None,
            }
        }
        None
    }
}

/// Empty meshes still get a one-vertex buffer so buffers are never zero sized.
fn upload(device: &wgpu::Device, m: &Mesh, label: &str) -> GpuMesh {
    let vertices = if m.vertices.is_empty() {
        vec![Vertex::zeroed()]
    } else {
        m.vertices.clone()
    };
    let indices = if m.indices.is_empty() {
        vec![0u32]
    } else {
        m.indices.clone()
    };
    GpuMesh {
        vb: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }),
        ib: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        }),
        indices: m.indices.len() as u32,
        vertices: m.vertices.len(),
        triangles: m.triangles(),
    }
}

/// False when every corner of the tile's box lies outside the same clip plane.
fn tile_in_view(view_proj: &glam::Mat4, t: &mesh::RoadTile) -> bool {
    let mut outside = [true; 4];
    for x in [t.min[0], t.max[0]] {
        for y in [t.min[1], t.max[1]] {
            for z in [0.0f32, 6.0] {
                let c = *view_proj * glam::Vec4::new(x, y, z, 1.0);
                if c.w <= 1e-3 {
                    return true;
                }
                outside[0] &= c.x < -c.w;
                outside[1] &= c.x > c.w;
                outside[2] &= c.y < -c.w;
                outside[3] &= c.y > c.w;
            }
        }
    }
    !outside.iter().any(|o| *o)
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
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
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
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
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
        let t = if dir.z < -0.05 {
            (-a.z / dir.z).min(view.far * 0.6)
        } else {
            view.far * 0.6
        };
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
    let proj = Mat4::orthographic_rh(
        cx - half,
        cx + half,
        cy - half,
        cy + half,
        -hi.z - 30.0,
        -lo.z + 30.0,
    );
    (proj * lv, 2.0 * half / size as f32)
}
