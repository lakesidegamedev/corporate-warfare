//! Screen to map and map to screen for a camera, from the same matrices and terrain the
//! renderer draws with. The interface uses these for labels (projection) and taps (pick), so a
//! tap lands on the region the player sees.

use glam::{Vec3, Vec4};

use crate::camera::Camera;
use crate::data::Scene;
use crate::mesh::ground_z;

/// Where a map position lands on a `width` x `height` screen, `lift_km` above the ground. None
/// when it is behind the camera.
pub fn project(
    scene: &Scene,
    camera: &Camera,
    width: f32,
    height: f32,
    lon: f32,
    lat: f32,
    lift_km: f32,
) -> Option<(f32, f32)> {
    let (tx, ty) = scene.projection.to_km(camera.lon, camera.lat);
    let view = camera.view((tx, ty), 0.0, width / height);
    let (x, y) = scene.projection.to_km(lon, lat);
    let z = ground_z(scene, x, y) + lift_km;
    let clip = view.view_proj * Vec4::new(x, y, z, 1.0);
    if clip.w <= 1e-6 {
        return None;
    }
    let (nx, ny) = (clip.x / clip.w, clip.y / clip.w);
    Some(((nx * 0.5 + 0.5) * width, (0.5 - ny * 0.5) * height))
}

/// The map position (lon, lat) under a screen point: the ray is walked down onto the terrain.
pub fn pick_ground(
    scene: &Scene,
    camera: &Camera,
    width: f32,
    height: f32,
    px: f32,
    py: f32,
) -> Option<(f32, f32)> {
    let (tx, ty) = scene.projection.to_km(camera.lon, camera.lat);
    let view = camera.view((tx, ty), 0.0, width / height);
    let (nx, ny) = (px / width * 2.0 - 1.0, 1.0 - py / height * 2.0);
    let near = view.inv_view_proj.project_point3(Vec3::new(nx, ny, 0.0));
    let far = view.inv_view_proj.project_point3(Vec3::new(nx, ny, 1.0));
    let dir = far - near;
    let above = |t: f32| {
        let p = near + dir * t;
        p.z - ground_z(scene, p.x, p.y)
    };
    // March from the eye until the ray is under the terrain, then bisect that step.
    const STEPS: usize = 240;
    let mut prev = 0.0f32;
    if above(prev) < 0.0 {
        return None;
    }
    let mut hit = None;
    for i in 1..=STEPS {
        let t = i as f32 / STEPS as f32;
        if above(t) <= 0.0 {
            hit = Some((prev, t));
            break;
        }
        prev = t;
    }
    let (mut lo, mut hi) = hit?;
    for _ in 0..24 {
        let mid = (lo + hi) / 2.0;
        if above(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let p = near + dir * hi;
    Some(scene.projection.to_lonlat(p.x, p.y))
}
