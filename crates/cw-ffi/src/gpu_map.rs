//! The desktop GPU map: a stylized scene of the running game, rendered with wgpu into a
//! caller-owned pixel buffer. Behind the `gpu-map` feature, so the phone and Apple builds never
//! link wgpu.
//!
//! The interface is a small plain C ABI (not UniFFI): the desktop shell calls it through JNA
//! with a direct byte buffer, so a frame is written once into memory the UI can draw from and
//! nothing is copied through the binding layer. Every entry point catches panics and returns a
//! negative code, so a driver fault turns into "fall back to the classic map", never a crash.
//!
//! Threads: `render` runs on the shell's render thread and holds the renderer for a frame;
//! `project` and `pick` (the UI thread, each frame) only read the scene and never wait on a
//! frame in flight; `update` (weekly) waits for the frame that is drawing.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use cw_render::{
    AssetDirs, Camera, CloudLevel, Owner, Placement, PlantSite, Renderer, ScenarioSpec, Scene,
    SceneOptions, SiteStatus, Style, building_boxes, layout_plants, pick_ground, project,
    ramp_by_name, region_tints,
};
use serde::Deserialize;

use crate::MapPlantDto;

/// Everything the shell tells the map about the game, as JSON. Every field is optional so the
/// shell sends only what changed.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Update {
    pub plants: Option<Vec<PlantIn>>,
    pub holdings: Option<Vec<String>>,
    /// Map mode: region code to level 0 (low) to 3 (high), and the palette ramp name.
    pub heat: Option<HashMap<String, u8>>,
    pub heat_ramp: Option<String>,
    pub night: Option<bool>,
    /// 0 off, 1 thin, 2 light, 3 heavy: the classic map's cloud setting.
    pub clouds: Option<u8>,
    /// Week of the year (1 to 52) for the snow, from the game's calendar.
    pub week: Option<u32>,
}

#[derive(Deserialize, Clone)]
pub struct PlantIn {
    pub region: String,
    pub industry: String,
    pub firm: u64,
    pub blocks: u32,
    pub player: bool,
    pub status: String,
}

impl From<&MapPlantDto> for PlantIn {
    fn from(p: &MapPlantDto) -> PlantIn {
        PlantIn {
            region: p.region.clone(),
            industry: p.industry.clone(),
            firm: p.firm,
            blocks: p.blocks,
            player: p.player,
            status: p.status.clone(),
        }
    }
}

/// One building per (region, industry, firm): the plants of a division stand together, sized by
/// how many there are. A division is a construction site while every plant is still building,
/// and mothballed when every plant is.
pub fn sites_of(plants: &[PlantIn]) -> Vec<PlantSite> {
    type Group<'a> = ((String, String, u64), Vec<&'a PlantIn>);
    let mut groups: Vec<Group> = Vec::new();
    for p in plants {
        let key = (p.region.clone(), p.industry.clone(), p.firm);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(p),
            None => groups.push((key, vec![p])),
        }
    }
    groups
        .into_iter()
        .map(|((region, industry, firm), items)| {
            let building = items
                .iter()
                .filter(|p| p.status.starts_with("building"))
                .count();
            let mothballed = items
                .iter()
                .filter(|p| p.status.starts_with("mothballed"))
                .count();
            let status = if building == items.len() {
                SiteStatus::Site
            } else if mothballed == items.len() {
                SiteStatus::Mothballed
            } else {
                SiteStatus::Running
            };
            PlantSite {
                region,
                industry,
                firm,
                plants: items.len() as u32,
                player: items.iter().any(|p| p.player),
                status,
            }
        })
        .collect()
}

/// The scenario's terrain, regions, roads and cities with no game state in it yet.
pub fn build_scene(assets: &Path, scenario: &str) -> Result<Scene, String> {
    let spec =
        ScenarioSpec::find(scenario).ok_or_else(|| format!("unknown scenario {scenario}"))?;
    Scene::for_scenario(
        &AssetDirs {
            resources: assets,
            baked: assets,
        },
        spec,
        &SceneOptions::default(),
    )
}

pub struct AdapterInfo {
    pub name: String,
    pub backend: String,
    pub software: bool,
}

/// Timings of one frame, in milliseconds.
#[derive(Clone, Copy, Default)]
pub struct FrameTiming {
    pub render_ms: f64,
    pub copy_ms: f64,
    pub width: u32,
    pub height: u32,
    pub draw_calls: u32,
}

pub struct GpuMap {
    scene: RwLock<Scene>,
    /// The scenario's own buildings (downtown blocks), kept apart from the game's plants.
    base_buildings: Vec<Placement>,
    renderer: Mutex<Renderer>,
    style: RwLock<Style>,
    info: AdapterInfo,
    holdings: Mutex<Vec<String>>,
    heat: Mutex<HashMap<String, u8>>,
    /// Which division each plant building stands for, in the order `layout_plants` placed them
    /// (the buildings after the scenery).
    sites: Mutex<Vec<Option<PlantSite>>>,
}

impl GpuMap {
    /// Build the scenario's scene and start the renderer. Fails when the scenario is unknown,
    /// its files are missing, or the machine has no graphics adapter.
    pub fn open(assets: &Path, scenario: &str) -> Result<GpuMap, String> {
        Self::open_with(assets, scenario, || {
            Renderer::new().map_err(|e| e.to_string())
        })
    }

    /// Like [`GpuMap::open`], drawing into a `CAMetalLayer` and presenting each frame (see
    /// [`GpuMap::render_to_surface`]).
    ///
    /// # Safety
    /// `layer` must be a valid `CAMetalLayer` that outlives the map.
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub unsafe fn open_layer(
        assets: &Path,
        scenario: &str,
        layer: *mut std::ffi::c_void,
    ) -> Result<GpuMap, String> {
        Self::open_with(assets, scenario, || {
            unsafe { Renderer::for_metal_layer(layer) }.map_err(|e| e.to_string())
        })
    }

    fn open_with(
        assets: &Path,
        scenario: &str,
        renderer: impl FnOnce() -> Result<Renderer, String>,
    ) -> Result<GpuMap, String> {
        let scene = build_scene(assets, scenario)?;
        let renderer = renderer()?;
        let a = renderer.adapter().clone();
        let base_buildings = scene.buildings.clone();
        Ok(GpuMap {
            scene: RwLock::new(scene),
            base_buildings,
            renderer: Mutex::new(renderer),
            style: RwLock::new(Style::default()),
            info: AdapterInfo {
                software: a.device_type == "Cpu" || a.name.to_lowercase().contains("llvmpipe"),
                name: a.name,
                backend: a.backend,
            },
            holdings: Mutex::new(Vec::new()),
            heat: Mutex::new(HashMap::new()),
            sites: Mutex::new(Vec::new()),
        })
    }

    pub fn info(&self) -> &AdapterInfo {
        &self.info
    }

    pub fn apply(&self, u: &Update) {
        {
            let mut style = self.style.write().unwrap();
            if let Some(n) = u.night {
                style.night = n;
            }
            if let Some(c) = u.clouds {
                style.clouds = match c {
                    0 => CloudLevel::Off,
                    1 => CloudLevel::Thin,
                    2 => CloudLevel::Light,
                    _ => CloudLevel::Heavy,
                };
            }
            if let Some(r) = &u.heat_ramp {
                style.heat_ramp = ramp_by_name(r);
            }
        }
        let mut scene = self.scene.write().unwrap();
        if let Some(week) = u.week {
            scene.set_week(week);
        }
        if let Some(plants) = &u.plants {
            let mut all = self.base_buildings.clone();
            let sites = sites_of(plants);
            let placed = layout_plants(&scene, &sites);
            *self.sites.lock().unwrap() = identify_sites(&scene, &sites, &placed);
            all.extend(placed);
            scene.set_buildings(all);
        }
        let mut tints_changed = false;
        if let Some(h) = &u.holdings {
            *self.holdings.lock().unwrap() = h.clone();
            tints_changed = true;
        }
        if let Some(h) = &u.heat {
            *self.heat.lock().unwrap() = h.clone();
            tints_changed = true;
        }
        if tints_changed {
            let tints = region_tints(
                &scene,
                &self.holdings.lock().unwrap(),
                &self.heat.lock().unwrap(),
            );
            scene.set_region_tints(&tints);
        }
    }

    /// Render into `out` (tightly packed RGBA8, `(width / scale) * (height / scale) * 4`
    /// bytes). `cam` is lon, lat, width in km, tilt, heading, field of view.
    pub fn render(
        &self,
        cam: [f64; 6],
        width: u32,
        height: u32,
        scale: u32,
        out: &mut [u8],
    ) -> FrameTiming {
        let scene = self.scene.read().unwrap();
        let mut style = self.style.read().unwrap().clone();
        style.pixel_scale = scale.max(1);
        let mut renderer = self.renderer.lock().unwrap();
        let t0 = Instant::now();
        let stats = renderer.render_into(&scene, &camera(cam), &style, width, height, out);
        let total = t0.elapsed().as_secs_f64() * 1000.0;
        let copy = stats.readback.as_secs_f64() * 1000.0;
        FrameTiming {
            render_ms: (total - copy).max(0.0),
            copy_ms: copy,
            width: stats.width,
            height: stats.height,
            draw_calls: stats.draw_calls,
        }
    }

    /// Draw a frame of `width` x `height` pixels into the window surface and present it.
    /// `draw_calls` is 0 for a skipped frame (the window could not take one).
    pub fn render_to_surface(&self, cam: [f64; 6], width: u32, height: u32) -> (FrameTiming, bool) {
        let scene = self.scene.read().unwrap();
        let style = self.style.read().unwrap().clone();
        let mut renderer = self.renderer.lock().unwrap();
        let t0 = Instant::now();
        let stats = renderer.render_to_surface(&scene, &camera(cam), &style, width, height);
        let total = t0.elapsed().as_secs_f64() * 1000.0;
        (
            FrameTiming {
                render_ms: total,
                copy_ms: 0.0,
                width: stats.width,
                height: stats.height,
                draw_calls: stats.draw_calls,
            },
            stats.skipped,
        )
    }

    /// Screen position of each (lon, lat) pair, `None` behind the camera.
    pub fn project(
        &self,
        cam: [f64; 6],
        width: f32,
        height: f32,
        pts: &[(f32, f32)],
    ) -> Vec<Option<(f32, f32)>> {
        let scene = self.scene.read().unwrap();
        let c = camera(cam);
        pts.iter()
            .map(|&(lon, lat)| project(&scene, &c, width, height, lon, lat, 0.0))
            .collect()
    }

    /// The region code under a screen point, from the geometry the frame was drawn from.
    pub fn pick(&self, cam: [f64; 6], width: f32, height: f32, px: f32, py: f32) -> Option<String> {
        let scene = self.scene.read().unwrap();
        let (lon, lat) = pick_ground(&scene, &camera(cam), width, height, px, py)?;
        scene.region_at(lon, lat).map(|r| r.code.clone())
    }

    /// The placed buildings as JSON for the shell: position, owner, sector and body colour, in
    /// the order `building_rects` answers for. Fixed between updates.
    pub fn buildings_json(&self) -> String {
        let scene = self.scene.read().unwrap();
        let sites = self.sites.lock().unwrap();
        let base = self.base_buildings.len();
        let items: Vec<serde_json::Value> = building_boxes(&scene)
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let site = i.checked_sub(base).and_then(|k| sites.get(k)).and_then(|s| s.as_ref());
                serde_json::json!({
                    "region": site.map(|s| s.region.as_str()),
                    "industry": site.map(|s| s.industry.as_str()),
                    "firm": site.map(|s| s.firm),
                    "plants": site.map(|s| s.plants),
                    "lon": b.lon,
                    "lat": b.lat,
                    "owner": match b.owner {
                        Owner::Player => "player",
                        Owner::Rival => "rival",
                        Owner::Scenery => "scenery",
                    },
                    "kind": b.model.split('/').next().unwrap_or(""),
                    "colour": format!("{:02X}{:02X}{:02X}", b.colour[0], b.colour[1], b.colour[2]),
                })
            })
            .collect();
        serde_json::Value::Array(items).to_string()
    }

    /// The screen rectangle (left, top, right, bottom) each building fills for this camera,
    /// ground footprint and roof together, NaN when any corner is behind the camera. Computed
    /// from the same matrices as the frame, so the interface can keep names off buildings and
    /// decide whether one is big enough to show.
    pub fn building_rects(&self, cam: [f64; 6], width: f32, height: f32) -> Vec<[f32; 4]> {
        let scene = self.scene.read().unwrap();
        let c = camera(cam);
        let kx = scene.projection.km_per_deg_lon();
        building_boxes(&scene)
            .iter()
            .map(|b| {
                let (dlon, dlat) = (b.half_km / kx, b.half_km / cw_render::KM_PER_DEG_LAT);
                let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    for lift in [0.0, b.height_km] {
                        let Some((x, y)) = project(
                            &scene,
                            &c,
                            width,
                            height,
                            b.lon + sx * dlon,
                            b.lat + sy * dlat,
                            lift,
                        ) else {
                            return [f32::NAN; 4];
                        };
                        r = [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)];
                    }
                }
                r
            })
            .collect()
    }

    /// Positions of the placed buildings, for tests and the shell's labels.
    pub fn building_positions(&self) -> Vec<(String, f32, f32)> {
        self.scene
            .read()
            .unwrap()
            .buildings
            .iter()
            .map(|b| (b.model.clone(), b.lon, b.lat))
            .collect()
    }

    pub fn region_tint_of(&self, code: &str) -> Option<u8> {
        let scene = self.scene.read().unwrap();
        let i = scene.regions.iter().position(|r| r.code == code)?;
        scene
            .mask
            .rgba
            .chunks_exact(4)
            .find(|p| p[0] as usize == i + 1)
            .map(|p| p[3])
    }
}

/// The division behind each placed plant building. `layout_plants` places a region's sites in a
/// fixed order (the player's first, the bigger first, then by firm and industry); the buildings
/// come out region by region in that order, so the k-th building of a region is that region's
/// k-th site. A building whose owner disagrees with its site (the order changed upstream) is
/// left unidentified rather than named wrongly.
fn identify_sites(scene: &Scene, sites: &[PlantSite], placed: &[Placement]) -> Vec<Option<PlantSite>> {
    let mut order: HashMap<&str, Vec<&PlantSite>> = HashMap::new();
    for s in sites {
        order.entry(s.region.as_str()).or_default().push(s);
    }
    for list in order.values_mut() {
        list.sort_by(|a, b| {
            b.player
                .cmp(&a.player)
                .then(b.plants.cmp(&a.plants))
                .then(a.firm.cmp(&b.firm))
                .then(a.industry.cmp(&b.industry))
        });
    }
    let mut used: HashMap<&str, usize> = HashMap::new();
    placed
        .iter()
        .map(|p| {
            // The home region is the site region whose centre is nearest the building.
            let home = order.keys().copied().min_by(|a, b| {
                let d = |code: &str| {
                    scene.regions.iter().find(|r| r.code == code).map_or(f32::MAX, |r| {
                        (r.lon - p.lon).powi(2) + (r.lat - p.lat).powi(2)
                    })
                };
                d(a).total_cmp(&d(b))
            })?;
            let k = used.entry(home).or_insert(0);
            let site = order[home].get(*k)?;
            *k += 1;
            let same_owner = (p.owner == Owner::Player) == site.player;
            same_owner.then(|| (*site).clone())
        })
        .collect()
}

fn camera(c: [f64; 6]) -> Camera {
    Camera {
        lon: c[0] as f32,
        lat: c[1] as f32,
        width_km: c[2] as f32,
        tilt_deg: c[3] as f32,
        heading_deg: c[4] as f32,
        fov_deg: c[5] as f32,
    }
}

// The C interface ------------------------------------------------------------------------

static NEXT: AtomicU64 = AtomicU64::new(1);
static MAPS: Mutex<Vec<(u64, Arc<GpuMap>)>> = Mutex::new(Vec::new());
static LAST_ERROR: Mutex<String> = Mutex::new(String::new());

fn set_error(e: String) {
    *LAST_ERROR.lock().unwrap() = e;
}

fn get(id: u64) -> Option<Arc<GpuMap>> {
    MAPS.lock()
        .unwrap()
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, m)| m.clone())
}

/// Run `f`, turning a panic into the error text and `fallback`.
fn guarded<T>(fallback: T, f: impl FnOnce() -> Result<T, String>) -> T {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            set_error(e);
            fallback
        }
        Err(p) => {
            let msg = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "the graphics driver failed".to_owned());
            set_error(msg);
            fallback
        }
    }
}

unsafe fn text<'a>(p: *const c_char) -> Result<&'a str, String> {
    if p.is_null() {
        return Err("null string".into());
    }
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|e| e.to_string())
}

unsafe fn put_text(s: &str, buf: *mut c_char, len: usize) -> i32 {
    let Ok(c) = CString::new(s.replace('\0', " ")) else {
        return -1;
    };
    let bytes = c.as_bytes_with_nul();
    if buf.is_null() || bytes.len() > len {
        return -(bytes.len() as i32);
    }
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr().cast(), buf, bytes.len()) };
    bytes.len() as i32 - 1
}

/// Open the map for a scenario. `assets` is a folder with the world map, earth and cloud images,
/// voxel models and baked terrain. Returns a handle, or 0 (see `cw_gpu_last_error`).
///
/// # Safety
/// `assets` and `scenario` must be NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_open(assets: *const c_char, scenario: *const c_char) -> u64 {
    guarded(0, || {
        let (assets, scenario) = unsafe { (text(assets)?, text(scenario)?) };
        let map = GpuMap::open(Path::new(assets), scenario)?;
        // The desktop picture is copied out and scaled by the app: texture steps are drawn at
        // least one screen pixel wide (the renderer's default is two), so ground detail stays crisp.
        map.style.write().unwrap().texel_min_px = DESKTOP_TEXEL_MIN_PX;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        MAPS.lock().unwrap().push((id, Arc::new(map)));
        Ok(id)
    })
}

/// Smallest ground texture step on the desktop, in screen pixels.
const DESKTOP_TEXEL_MIN_PX: f32 = 1.0;

/// Open the map drawing into a `CAMetalLayer` (iOS, Mac Catalyst): frames go to the screen with
/// `cw_gpu_render_layer`, no pixels are copied. Returns a handle, or 0 (see `cw_gpu_last_error`).
/// Close the map (`cw_gpu_close`) before the layer is released.
///
/// # Safety
/// `assets` and `scenario` must be NUL-terminated strings and `layer` a valid `CAMetalLayer`.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_open_layer(
    assets: *const c_char,
    scenario: *const c_char,
    layer: *mut std::ffi::c_void,
) -> u64 {
    guarded(0, || {
        if layer.is_null() {
            return Err("no layer".into());
        }
        let (assets, scenario) = unsafe { (text(assets)?, text(scenario)?) };
        let map = unsafe { GpuMap::open_layer(Path::new(assets), scenario, layer) }?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        MAPS.lock().unwrap().push((id, Arc::new(map)));
        Ok(id)
    })
}

/// Draw one frame of `width` x `height` pixels into the layer the map was opened on and present
/// it. Returns 0 when drawn, 1 when the display could not take a frame (try again later), a
/// negative number on failure. `cam` points at six doubles as for `cw_gpu_render`; `timing` (or
/// null) receives six doubles: render ms, 0, width, height, draw calls, 0.
///
/// # Safety
/// `cam` must be readable for 6 doubles and `timing` writable for 6 doubles.
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_render_layer(
    id: u64,
    cam: *const f64,
    width: u32,
    height: u32,
    timing: *mut f64,
) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        if cam.is_null() || width == 0 || height == 0 {
            return Err("bad frame request".into());
        }
        let cam: [f64; 6] = unsafe { std::slice::from_raw_parts(cam, 6) }
            .try_into()
            .map_err(|_| "camera")?;
        let (t, skipped) = m.render_to_surface(cam, width, height);
        if !timing.is_null() {
            let o = unsafe { std::slice::from_raw_parts_mut(timing, 6) };
            o.copy_from_slice(&[
                t.render_ms,
                0.0,
                t.width as f64,
                t.height as f64,
                t.draw_calls as f64,
                0.0,
            ]);
        }
        Ok(skipped as i32)
    })
}

/// The last error text, or -(needed size) when the buffer is too small.
///
/// # Safety
/// `buf` must be writable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_last_error(buf: *mut c_char, len: usize) -> i32 {
    let e = LAST_ERROR.lock().unwrap().clone();
    unsafe { put_text(&e, buf, len) }
}

/// Adapter as `name|backend|software` (software is 1 or 0).
///
/// # Safety
/// `buf` must be writable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_info(id: u64, buf: *mut c_char, len: usize) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        let i = m.info();
        Ok(unsafe {
            put_text(
                &format!(
                    "{}|{}|{}",
                    i.name.replace('|', "/"),
                    i.backend,
                    i.software as u8
                ),
                buf,
                len,
            )
        })
    })
}

/// Apply a JSON [`Update`]. Returns 0, or a negative code.
///
/// # Safety
/// `json` must be a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_update(id: u64, json: *const c_char) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        let u: Update = serde_json::from_str(unsafe { text(json)? }).map_err(|e| e.to_string())?;
        m.apply(&u);
        Ok(0)
    })
}

/// Render a frame into `out` (`out_len` bytes). `cam` points at six doubles (lon, lat, width
/// km, tilt, heading, fov); `timing` at six doubles that receive render ms, copy ms, width,
/// height, draw calls and a spare. The image is `width / scale` by `height / scale`.
///
/// # Safety
/// `out` must be writable for `out_len` bytes, `cam` readable for 6 doubles and `timing`
/// writable for 6 doubles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_render(
    id: u64,
    cam: *const f64,
    width: u32,
    height: u32,
    scale: u32,
    out: *mut u8,
    out_len: usize,
    timing: *mut f64,
) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        let scale = scale.max(1);
        let need = ((width / scale).max(1) * (height / scale).max(1) * 4) as usize;
        if out.is_null() || cam.is_null() || out_len < need {
            return Err("render buffer too small".into());
        }
        let cam: [f64; 6] = unsafe { std::slice::from_raw_parts(cam, 6) }
            .try_into()
            .map_err(|_| "camera")?;
        let out = unsafe { std::slice::from_raw_parts_mut(out, out_len) };
        let t = m.render(cam, width, height, scale, out);
        if !timing.is_null() {
            let o = unsafe { std::slice::from_raw_parts_mut(timing, 6) };
            o.copy_from_slice(&[
                t.render_ms,
                t.copy_ms,
                t.width as f64,
                t.height as f64,
                t.draw_calls as f64,
                0.0,
            ]);
        }
        Ok(0)
    })
}

/// Project `n` (lon, lat) pairs to screen pixels: `out` receives `n` pairs, NaN when a point is
/// behind the camera.
///
/// # Safety
/// `pts` must be readable for `2 * n` doubles, `cam` for 6, and `out` writable for `2 * n` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_project(
    id: u64,
    cam: *const f64,
    width: f32,
    height: f32,
    pts: *const f64,
    n: usize,
    out: *mut f32,
) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        let cam: [f64; 6] = unsafe { std::slice::from_raw_parts(cam, 6) }
            .try_into()
            .map_err(|_| "camera")?;
        let pts: Vec<(f32, f32)> = unsafe { std::slice::from_raw_parts(pts, n * 2) }
            .chunks_exact(2)
            .map(|p| (p[0] as f32, p[1] as f32))
            .collect();
        let out = unsafe { std::slice::from_raw_parts_mut(out, n * 2) };
        for (i, p) in m.project(cam, width, height, &pts).into_iter().enumerate() {
            let (x, y) = p.unwrap_or((f32::NAN, f32::NAN));
            out[i * 2] = x;
            out[i * 2 + 1] = y;
        }
        Ok(0)
    })
}

/// The region code under a screen point, written to `buf`; 0 when no region is there, the
/// text length otherwise, a negative number on failure.
///
/// # Safety
/// `cam` must be readable for 6 doubles and `buf` writable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_pick(
    id: u64,
    cam: *const f64,
    width: f32,
    height: f32,
    px: f32,
    py: f32,
    buf: *mut c_char,
    len: usize,
) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        let cam: [f64; 6] = unsafe { std::slice::from_raw_parts(cam, 6) }
            .try_into()
            .map_err(|_| "camera")?;
        Ok(match m.pick(cam, width, height, px, py) {
            Some(code) => unsafe { put_text(&code, buf, len) },
            None => 0,
        })
    })
}

/// The placed buildings as JSON (see `GpuMap::buildings_json`), written to `buf`; the text
/// length, or a negative number (minus the size needed) when the buffer is too small.
///
/// # Safety
/// `buf` must be writable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_buildings(id: u64, buf: *mut c_char, len: usize) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        Ok(unsafe { put_text(&m.buildings_json(), buf, len) })
    })
}

/// Screen rectangles of the buildings for a camera: four floats (left, top, right, bottom) per
/// building, in the order of `cw_gpu_buildings`, NaN when behind the camera. Returns the number
/// of buildings; at most `max` are written.
///
/// # Safety
/// `cam` must be readable for 6 doubles and `out` writable for `4 * max` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cw_gpu_building_rects(
    id: u64,
    cam: *const f64,
    width: f32,
    height: f32,
    out: *mut f32,
    max: usize,
) -> i32 {
    guarded(-1, || {
        let m = get(id).ok_or("no such map")?;
        let cam: [f64; 6] = unsafe { std::slice::from_raw_parts(cam, 6) }
            .try_into()
            .map_err(|_| "camera")?;
        let rects = m.building_rects(cam, width, height);
        if !out.is_null() {
            let o = unsafe { std::slice::from_raw_parts_mut(out, max * 4) };
            for (i, r) in rects.iter().take(max).enumerate() {
                o[i * 4..i * 4 + 4].copy_from_slice(r);
            }
        }
        Ok(rects.len() as i32)
    })
}

/// Free a map.
#[unsafe(no_mangle)]
pub extern "C" fn cw_gpu_close(id: u64) {
    MAPS.lock().unwrap().retain(|(i, _)| *i != id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewGameDto, SessionHandle};
    use std::ffi::CString;

    fn plant(region: &str, industry: &str, firm: u64, status: &str, player: bool) -> PlantIn {
        PlantIn {
            region: region.into(),
            industry: industry.into(),
            firm,
            blocks: 1,
            player,
            status: status.into(),
        }
    }

    #[test]
    fn plants_of_one_division_stand_as_one_building() {
        let sites = sites_of(&[
            plant("TX-1", "oil", 1, "running", true),
            plant("TX-1", "oil", 1, "building", true),
            plant("TX-1", "oil", 2, "building", false),
            plant("TX-2", "oil", 2, "mothballed", false),
        ]);
        assert_eq!(sites.len(), 3);
        let mine = sites.iter().find(|s| s.firm == 1).unwrap();
        assert_eq!(
            (mine.plants, mine.status, mine.player),
            (2, SiteStatus::Running, true)
        );
        let rival = sites
            .iter()
            .find(|s| s.firm == 2 && s.region == "TX-1")
            .unwrap();
        assert_eq!((rival.status, rival.player), (SiteStatus::Site, false));
        assert_eq!(
            sites.iter().find(|s| s.region == "TX-2").unwrap().status,
            SiteStatus::Mothballed
        );
    }

    /// The scenario folder, assembled the way the desktop build does: the shared images and models
    /// from the Apple resources, the baked terrain from the renderer's assets.
    fn assets() -> std::path::PathBuf {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("cw-gpu-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for src in [
            root.join("apple/Resources"),
            root.join("crates/cw-render/assets"),
        ] {
            for e in std::fs::read_dir(src).unwrap().flatten() {
                let p = e.path();
                if p.is_file() {
                    std::fs::copy(&p, dir.join(p.file_name().unwrap())).unwrap();
                }
            }
        }
        dir
    }

    fn career(scenario: &str) -> std::sync::Arc<SessionHandle> {
        SessionHandle::start(NewGameDto {
            seed: 7,
            scenario_id: scenario.into(),
            guided: true,
            five_year: false,
            difficulty: "standard".into(),
            economy: "historic".into(),
            starting_capital_cents: None,
            price_path: String::new(),
            timeline: String::new(),
            chief_name: String::new(),
            chief_portrait: String::new(),
            chief_temperament: String::new(),
        })
        .unwrap()
    }

    #[test]
    fn real_plants_and_holdings_land_where_the_engine_says() {
        for id in ["texas-1981", "great-lakes-2000"] {
            let scene = build_scene(&assets(), id).unwrap();
            let session = career(id);
            let plants: Vec<PlantIn> = session.map_plants().iter().map(PlantIn::from).collect();
            assert!(!plants.is_empty(), "{id}: the career has no plants");
            let sites = sites_of(&plants);
            let placed = layout_plants(&scene, &sites);
            let in_scenario: Vec<&PlantSite> = sites
                .iter()
                .filter(|s| scene.regions.iter().any(|r| r.code == s.region))
                .collect();
            assert!(
                !in_scenario.is_empty(),
                "{id}: no plant is in a scenario region"
            );
            // Every placed building stands inside the region its site belongs to, near its hub.
            let kx = scene.projection.km_per_deg_lon();
            for b in &placed {
                let home = scene.region_at(b.lon, b.lat);
                let hub_near = scene.regions.iter().any(|r| {
                    let dx = (b.lon - r.lon) * kx;
                    let dy = (b.lat - r.lat) * cw_render::KM_PER_DEG_LAT;
                    (dx * dx + dy * dy).sqrt() < 25.0
                });
                assert!(
                    hub_near || home.is_some(),
                    "{id}: a building floats away from every region"
                );
            }
            // The player's own plants are among them, and none is dropped for a region it is in.
            for s in in_scenario.iter().filter(|s| s.player) {
                let hub = scene.regions.iter().find(|r| r.code == s.region).unwrap();
                assert!(
                    placed.iter().any(|b| {
                        let dx = (b.lon - hub.lon) * kx;
                        let dy = (b.lat - hub.lat) * cw_render::KM_PER_DEG_LAT;
                        (dx * dx + dy * dy).sqrt() < 25.0
                    }),
                    "{id}: the player's plant in {} has no building at its hub",
                    s.region
                );
            }
            // Holdings tint exactly the regions the divisions are in.
            let held: Vec<String> = session
                .map_divisions()
                .iter()
                .map(|d| d.region.clone())
                .collect();
            let tints = region_tints(&scene, &held, &HashMap::new());
            for r in &scene.regions {
                let want = if held.contains(&r.code) {
                    cw_render::TINT_PLAYER
                } else {
                    cw_render::TINT_PLAY
                };
                assert_eq!(tints[&r.code], want, "{id}: tint of {}", r.code);
            }
        }
    }

    #[test]
    fn every_scenario_builds_with_its_own_terrain_and_regions() {
        let dir = assets();
        for spec in cw_render::SCENARIOS.iter() {
            let scene = build_scene(&dir, spec.id).unwrap_or_else(|e| panic!("{}: {e}", spec.id));
            assert!(
                scene.regions.len() >= 3,
                "{}: {} regions",
                spec.id,
                scene.regions.len()
            );
            assert!(!scene.cities.is_empty(), "{}: no cities", spec.id);
            assert!(!scene.lines.is_empty(), "{}: no roads or rails", spec.id);
        }
    }

    #[test]
    fn a_missing_or_unknown_scenario_opens_nothing_and_says_why() {
        let dir = CString::new(
            std::env::temp_dir()
                .join("cw-gpu-no-such-folder")
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let bad = CString::new("atlantis-1").unwrap();
        let ok = CString::new("texas-1981").unwrap();
        for scenario in [&bad, &ok] {
            let id = unsafe { cw_gpu_open(dir.as_ptr(), scenario.as_ptr()) };
            assert_eq!(id, 0);
            let mut buf = vec![0 as c_char; 512];
            let n = unsafe { cw_gpu_last_error(buf.as_mut_ptr(), buf.len()) };
            assert!(n > 0, "the failure has no text");
        }
        // Calls on a map that is not open fail cleanly instead of crashing.
        let mut cam = [0.0f64; 6];
        let mut out = [0u8; 16];
        let code = unsafe {
            cw_gpu_render(
                99,
                cam.as_mut_ptr(),
                2,
                2,
                1,
                out.as_mut_ptr(),
                16,
                std::ptr::null_mut(),
            )
        };
        assert!(code < 0);
    }

    #[test]
    fn a_map_renders_a_frame_when_an_adapter_exists() {
        let dir = assets();
        let map = match GpuMap::open(&dir, "texas-1981") {
            Ok(m) => m,
            Err(e) => {
                eprintln!("skipping, no usable adapter: {e}");
                return;
            }
        };
        let session = career("texas-1981");
        let plants: Vec<PlantIn> = session.map_plants().iter().map(PlantIn::from).collect();
        map.apply(&Update {
            plants: Some(plants),
            holdings: Some(vec!["TX-1".into()]),
            ..Update::default()
        });
        let (w, h) = (320u32, 200u32);
        let mut out = vec![0u8; (w * h * 4) as usize];
        let cam = [-99.5, 31.0, 1800.0, 20.0, 0.0, 28.0];
        let t = map.render(cam, w, h, 1, &mut out);
        assert_eq!((t.width, t.height), (w, h));
        assert!(
            out.chunks_exact(4).any(|p| p[0] != p[1] || p[1] != p[2]),
            "the frame is flat grey"
        );
        // A tap on the centre of the view lands in a Texas region.
        let hit = map.pick(cam, w as f32, h as f32, w as f32 / 2.0, h as f32 / 2.0);
        let mut grid = 0;
        for gy in 0..10 {
            for gx in 0..16 {
                if map
                    .pick(
                        cam,
                        w as f32,
                        h as f32,
                        gx as f32 * 20.0 + 10.0,
                        gy as f32 * 20.0 + 10.0,
                    )
                    .is_some()
                {
                    grid += 1;
                }
            }
        }
        eprintln!("grid picks: {grid} of 160");
        assert!(grid > 0, "no point of the view picks a region");
        eprintln!(
            "centre pick: {hit:?}, render {:.1} ms copy {:.1} ms",
            t.render_ms, t.copy_ms
        );
    }

    #[test]
    fn the_game_camera_over_great_lakes_draws_land() {
        let dir = assets();
        let Ok(map) = GpuMap::open(&dir, "great-lakes-2000") else {
            eprintln!("skipping, no usable adapter");
            return;
        };
        let (w, h) = (320u32, 200u32);
        let mut out = vec![0u8; (w * h * 4) as usize];
        let cam = [-81.6846, 42.34405, 2330.0, 0.0, 0.0, 28.0];
        map.render(cam, w, h, 1, &mut out);
        let at = |x: usize, y: usize| out[(y * w as usize + x) * 4..][..3].to_vec();
        let land = out
            .chunks_exact(4)
            .filter(|p| p[..3] != [0x1e, 0x47, 0x72])
            .count();
        eprintln!(
            "centre {:?} corner {:?} non-clear {land}",
            at(160, 100),
            at(5, 5)
        );
        assert!(
            land > (w * h) as usize / 2,
            "most of the view is the clear colour"
        );
        let hits = (0..10)
            .flat_map(|gy| (0..16).map(move |gx| (gx, gy)))
            .filter(|(gx, gy)| {
                map.pick(
                    cam,
                    w as f32,
                    h as f32,
                    *gx as f32 * 20.0 + 10.0,
                    *gy as f32 * 20.0 + 10.0,
                )
                .is_some()
            })
            .count();
        eprintln!("great lakes grid picks {hits}");
        assert!(hits > 0);
    }

    /// Writes the review views to the folder in CW_GPU_DUMP (a P6 picture each); does nothing
    /// otherwise. Run it by hand: `CW_GPU_DUMP=dir cargo test -p cw-ffi --features gpu-map dump_polish_views -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_polish_views() {
        let Ok(out) = std::env::var("CW_GPU_DUMP") else {
            return;
        };
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        let dir = assets();
        let (w, h) = (1600u32, 1000u32);
        let only = std::env::var("CW_GPU_ONLY").unwrap_or_default();
        for (id, hub, views) in [
            (
                "great-lakes-2000",
                "OH-CEN",
                [
                    ("scenario", [-82.0, 41.9, 2250.0, 24.0, 0.0, 28.0]),
                    ("close", [-82.9, 40.0, 420.0, 40.0, 0.0, 28.0]),
                    ("turned", [-82.3, 41.1, 900.0, 52.0, 38.0, 28.0]),
                    ("edge", [-83.0, 44.0, 1800.0, 64.0, 0.0, 28.0]),
                ],
            ),
            (
                "texas-1981",
                "TX-HOU",
                [
                    ("scenario", [-97.5, 31.0, 2300.0, 24.0, 0.0, 28.0]),
                    ("close", [-95.4, 29.8, 380.0, 40.0, 0.0, 28.0]),
                    ("turned", [-100.0, 31.5, 1100.0, 52.0, 38.0, 28.0]),
                    ("edge", [-98.0, 31.0, 1800.0, 64.0, 200.0, 28.0]),
                ],
            ),
            (
                "ruhr-rhine-1991",
                "",
                [
                    ("scenario", [8.0, 51.0, 1500.0, 24.0, 0.0, 28.0]),
                    ("close", [7.2, 51.4, 300.0, 40.0, 0.0, 28.0]),
                    ("turned", [8.0, 50.5, 800.0, 52.0, 38.0, 28.0]),
                    ("edge", [8.0, 49.0, 1500.0, 64.0, 0.0, 28.0]),
                ],
            ),
            (
                "silicon-valley-1995",
                "",
                [
                    ("scenario", [-119.5, 37.0, 1500.0, 24.0, 0.0, 28.0]),
                    ("close", [-121.9, 37.4, 300.0, 40.0, 0.0, 28.0]),
                    ("turned", [-120.0, 37.5, 800.0, 52.0, 38.0, 28.0]),
                    ("edge", [-120.5, 36.0, 1500.0, 64.0, 270.0, 28.0]),
                ],
            ),
        ] {
            if !only.is_empty() && !id.contains(&only) {
                continue;
            }
            let _ = hub;
            let Ok(map) = GpuMap::open(&dir, id) else {
                eprintln!("no adapter");
                return;
            };
            let session = career(id);
            let plants: Vec<PlantIn> = session.map_plants().iter().map(PlantIn::from).collect();
            let held: Vec<String> = session
                .map_divisions()
                .iter()
                .map(|d| d.region.clone())
                .collect();
            for (season, week) in [("day", 28u32), ("winter", 3)] {
                map.apply(&Update {
                    plants: Some(plants.clone()),
                    holdings: Some(held.clone()),
                    week: Some(week),
                    ..Update::default()
                });
                let mut views = views.to_vec();
                if let Some(b) = map
                    .scene
                    .read()
                    .unwrap()
                    .buildings
                    .iter()
                    .find(|b| b.owner == Owner::Player)
                {
                    views.push((
                        "building",
                        [b.lon as f64, b.lat as f64, 40.0, 45.0, 20.0, 28.0],
                    ));
                }
                for (name, cam) in views {
                    let mut buf = vec![0u8; (w * h * 4) as usize];
                    map.render(cam, w, h, 1, &mut buf);
                    let rects = map.building_rects(cam, w as f32, h as f32);
                    let sizes: Vec<f32> = rects
                        .iter()
                        .filter(|r| r[0].is_finite())
                        .map(|r| (r[2] - r[0]).max(r[3] - r[1]))
                        .collect();
                    let big = sizes.iter().filter(|s| **s >= 6.0).count();
                    eprintln!(
                        "{id} {season} {name}: {} buildings, {big} at 6 px or more",
                        sizes.len()
                    );
                    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                    for p in buf.chunks_exact(4) {
                        ppm.extend_from_slice(&p[..3]);
                    }
                    std::fs::write(out.join(format!("{id}-{name}-{season}.ppm")), ppm).unwrap();
                }
            }
        }
    }
}
