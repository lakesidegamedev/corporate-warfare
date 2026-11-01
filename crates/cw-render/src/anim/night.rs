//! Light after dark: the lamps of trucks, trains and ships, and the aviation beacons. A lamp is a
//! small cube drawn flat in one step of the signal ramp (`KIND_LIGHT`), so it is a palette colour
//! at every light and never shaded, outlined, blended or glowing. It is as small as one screen
//! pixel and grows with the vehicle when the camera is close.
//!
//! This is a child of the animated layer so it can read the vehicle types; the vehicle code only
//! calls [`vehicle_lights`] and [`beacon`].

use super::{Kind, Route};
use crate::mesh::{KIND_LIGHT, Mesh, box_faces};

/// Steps of the signal ramp: green, warm white, red.
pub(super) const GREEN: u8 = 0;
pub(super) const WARM: u8 = 1;
pub(super) const RED: u8 = 2;

/// A lamp `size` on a side centred at `c`.
pub(super) fn lamp(m: &mut Mesh, c: [f32; 3], size: f32, step: u8) {
    let h = size / 2.0;
    box_faces(
        m,
        &|q| q,
        &|n| n,
        [c[0] - h, c[1] - h, c[2] - h],
        [c[0] + h, c[1] + h, c[2] + h],
        [step, 0, 0],
        KIND_LIGHT,
    );
}

/// A blinking aviation beacon sitting on `base` (the top of a tower or stack).
pub(super) fn beacon(m: &mut Mesh, base: [f32; 3], size: f32) {
    lamp(m, [base[0], base[1], base[2] + size / 2.0], size, RED);
}

/// A lamp at `local` in a frame turned by `yaw` about `at` (x ahead, y to the left).
fn lamp_at(m: &mut Mesh, at: [f32; 3], yaw: f32, local: [f32; 3], size: f32, step: u8) {
    let (s, c) = yaw.sin_cos();
    lamp(
        m,
        [
            at[0] + local[0] * c - local[1] * s,
            at[1] + local[0] * s + local[1] * c,
            at[2] + local[2],
        ],
        size,
        step,
    );
}

/// Where a vehicle is and which way it faces.
#[derive(Clone, Copy)]
pub(super) struct Pose {
    pub at: [f32; 3],
    pub yaw: f32,
}

/// A pair of lamps `spread` either side of the centre line, or one on it when they would touch.
fn pair(m: &mut Mesh, pose: Pose, x: f32, z: f32, spread: f32, size: f32, step: u8) {
    if spread >= size * 1.5 {
        for side in [-1.0f32, 1.0] {
            lamp_at(m, pose.at, pose.yaw, [x, side * spread, z], size, step);
        }
    } else {
        lamp_at(m, pose.at, pose.yaw, [x, 0.0, z], size, step);
    }
}

/// The lamps of one vehicle at `s` kilometres along its route facing `dir`; `pose` is its
/// front car's pose and `u` its unit (see `vehicle`). Trucks and trains show
/// white at the front and red at the back, ships a masthead light, a stern light and red and green
/// sidelights.
pub(super) fn vehicle_lights(
    m: &mut Mesh,
    r: &Route,
    s: f32,
    dir: f32,
    pose: Pose,
    u: f32,
    px_km: f32,
) {
    let size = (0.3 * u).max(1.2 * px_km);
    match r.kind {
        Kind::Truck => {
            pair(m, pose, 1.6 * u + size / 2.0, 0.45 * u, 0.5 * u, size, WARM);
            pair(m, pose, -1.7 * u - size / 2.0, 0.45 * u, 0.5 * u, size, RED);
        }
        Kind::Train => {
            pair(m, pose, u + size / 2.0, 0.7 * u, 0.4 * u, size, WARM);
            // The last wagon, placed as fleet::train places it (2.3 u apart behind the locomotive).
            let back = r.wagons.max(1) as f32 * 2.3 * u * -dir;
            let (p, h) = r.at((s + back).clamp(0.0, r.length()));
            let tail_yaw = if dir > 0.0 {
                h
            } else {
                h + std::f32::consts::PI
            };
            pair(
                m,
                Pose {
                    at: p,
                    yaw: tail_yaw,
                },
                -u - size / 2.0,
                0.6 * u,
                0.4 * u,
                size,
                RED,
            );
        }
        Kind::Ship => {
            lamp_at(
                m,
                pose.at,
                pose.yaw,
                [-1.3 * u, 0.0, 1.9 * u + size / 2.0],
                size,
                WARM,
            );
            lamp_at(
                m,
                pose.at,
                pose.yaw,
                [-1.8 * u - size / 2.0, 0.0, 0.4 * u],
                size,
                WARM,
            );
            lamp_at(
                m,
                pose.at,
                pose.yaw,
                [1.5 * u, 0.55 * u + size / 2.0, 0.45 * u],
                size,
                RED,
            );
            lamp_at(
                m,
                pose.at,
                pose.yaw,
                [1.5 * u, -0.55 * u - size / 2.0, 0.45 * u],
                size,
                GREEN,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lamp_is_a_flat_signal_cube() {
        let mut m = Mesh::default();
        lamp(&mut m, [1.0, 2.0, 3.0], 0.2, RED);
        assert_eq!(m.vertices.len(), 24);
        assert!(
            m.vertices
                .iter()
                .all(|v| v.colour == [RED, 0, 0, KIND_LIGHT])
        );
    }

    #[test]
    fn a_vehicle_shows_a_white_front_and_a_red_back() {
        let r = Route::new(Kind::Truck, vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], 1, 1).unwrap();
        let mut m = Mesh::default();
        vehicle_lights(
            &mut m,
            &r,
            5.0,
            1.0,
            Pose {
                at: [5.0, 0.0, 0.0],
                yaw: 0.0,
            },
            0.035,
            0.001,
        );
        let front: Vec<_> = m.vertices.iter().filter(|v| v.colour[0] == WARM).collect();
        let back: Vec<_> = m.vertices.iter().filter(|v| v.colour[0] == RED).collect();
        assert!(!front.is_empty() && !back.is_empty());
        // Facing +x: the white lamps are ahead of the red ones.
        let mean =
            |v: &[&crate::mesh::Vertex]| v.iter().map(|q| q.pos[0]).sum::<f32>() / v.len() as f32;
        assert!(mean(&front) > 5.0 && mean(&back) < 5.0);
    }
}
