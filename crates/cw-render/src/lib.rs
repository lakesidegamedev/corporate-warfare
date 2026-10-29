//! cw-render: the 1.9 stylized GPU map renderer, offscreen only.
//!
//! This is the style and performance spike for gate G0. It depends on no UI crate, is not
//! linked into the FFI or the apps, reads no engine state and cannot change a save or a hash.
//! Headless renders use whatever adapter wgpu finds; on a machine without a GPU that is the
//! Mesa software Vulkan driver.

mod camera;
mod data;
mod mesh;
mod palette;
mod renderer;

pub use camera::Camera;
pub use data::{
    Bounds, GeoImage, Heightmap, Mask, Placement, Projection, Scene, VoxelBox, VoxelModel,
    GREAT_LAKES_STATES, PLAYER_REGIONS,
};
pub use palette::{Palette, COLOURS as PALETTE_COLOURS};
pub use renderer::{AdapterSummary, Frame, FrameStats, NoAdapter, Renderer, Style};
