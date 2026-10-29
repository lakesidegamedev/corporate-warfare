//! cw-render: the 1.9 stylized GPU map renderer, offscreen only.
//!
//! This is the style and performance spike for gate G0. It depends on no UI crate, is not
//! linked into the FFI or the apps, reads no engine state and cannot change a save or a hash.
//! Headless renders use whatever adapter wgpu finds; on a machine without a GPU that is the
//! Mesa software Vulkan driver.

mod camera;
mod data;
mod ground;
mod mesh;
mod palette;
mod renderer;

pub use camera::Camera;
pub use data::{
    Bounds, City, GREAT_LAKES_STATES, GeoImage, Heightmap, LINE_INTERSTATE, LINE_MAIN, LINE_RAIL,
    Line, Mask, PLAYER_REGIONS, Placement, Projection, Scene, VoxelBox, VoxelModel,
};
pub use palette::{COLOURS as PALETTE_COLOURS, Palette};
pub use renderer::{AdapterSummary, CloudLevel, Frame, FrameStats, NoAdapter, Renderer, Style};

pub use mesh::{RoadPoint, ground_z, road_chains};
