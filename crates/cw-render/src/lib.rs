//! cw-render: the 1.9 stylized GPU map renderer, offscreen only.
//!
//! This is the style and performance spike for gate G0. It depends on no UI crate, is not
//! linked into the FFI or the apps, reads no engine state and cannot change a save or a hash.
//! Headless renders use whatever adapter wgpu finds; on a machine without a GPU that is the
//! Mesa software Vulkan driver.

mod camera;
mod data;
mod far;
mod game;
mod ground;
mod mesh;
mod palette;
mod pick;
mod renderer;

pub use camera::Camera;
pub use data::{
    AssetDirs, Bounds, City, GREAT_LAKES_STATES, GeoImage, Heightmap, KM_PER_DEG_LAT,
    LINE_INTERSTATE, LINE_MAIN, LINE_RAIL, Line, Mask, Owner, PLANT_MODELS, PLAYER_REGIONS,
    Placement, Projection, RegionInfo, SCENARIOS, ScenarioSpec, Scene, SceneOptions, TINT_HEAT,
    TINT_PLAY, TINT_PLAYER, VoxelBox, VoxelModel,
};
pub use game::{PlantSite, SiteStatus, layout_plants, plant_size_km, ramp_by_name, region_tints};
pub use palette::{COLOURS as PALETTE_COLOURS, Palette};
pub use pick::{pick_ground, project};
pub use renderer::{AdapterSummary, CloudLevel, Frame, FrameStats, NoAdapter, Renderer, Style};

pub use ground::snow_cover;
pub use mesh::{
    BuildingBox, ROAD_BANDS, RoadPoint, apron_extent_km, apron_land_boxes, apron_seam_steps,
    building_boxes, ground_z, road_band, road_chains, road_chains_for_band,
};
