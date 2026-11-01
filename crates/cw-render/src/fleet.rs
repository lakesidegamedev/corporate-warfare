//! Models of the vehicles, the harbours and the selection outline: trucks with a cab and a trailer
//! that follows the cargo, trains of a locomotive and wagons of the matching type, ships by kind,
//! quays with stacked containers and gantry cranes, and the one pixel outline round a hovered or
//! selected plant.
//!
//! Plain boxes in palette colours, built on the CPU for one clock value like the rest of the
//! animated layer. The same inputs always give the same mesh.

use crate::anim::{colour, hash, period, put_turned};
use crate::data::Placement;
use crate::mesh::Mesh;
use crate::palette::{Rgb, ramp};

/// What a vehicle carries, which sets the shape of its trailer or wagons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cargo {
    /// Fuel and chemicals: a round tank.
    Tanker,
    /// Steel and timber: an open deck with the load strapped on.
    Flatbed,
    /// Goods: a closed box.
    Box,
    /// Grain and ore: a hopper with a heap on top.
    Hopper,
}

/// Ships by what they carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipKind {
    Bulk,
    Tanker,
    Container,
    /// A river barge pushed by a tug.
    Barge,
}

/// The cargo of a freight lane from the name of its goods.
pub fn cargo_of_goods(goods: &str) -> Cargo {
    let g = goods.to_lowercase();
    let tokens: Vec<&str> = g.split(|c: char| !c.is_alphabetic()).collect();
    let has = |words: &[&str]| {
        words
            .iter()
            .any(|w| tokens.iter().any(|t| t.starts_with(w)))
    };
    if has(&[
        "oil", "fuel", "gas", "petrol", "diesel", "chemical", "plastic", "fertil", "acid",
    ]) {
        Cargo::Tanker
    } else if has(&[
        "steel",
        "timber",
        "lumber",
        "wood",
        "metal",
        "machine",
        "vehicle",
        "car",
        "pipe",
        "construction",
    ]) {
        Cargo::Flatbed
    } else if has(&[
        "grain", "wheat", "corn", "ore", "coal", "cement", "sand", "mineral", "crop", "bulk",
        "agri",
    ]) {
        Cargo::Hopper
    } else {
        Cargo::Box
    }
}

/// The cargo a plant's trucks carry, from its sector.
pub fn cargo_of_industry(industry: &str) -> Cargo {
    match industry {
        "energy" | "chemical_industries" => Cargo::Tanker,
        "manufacturing" | "construction" | "defense" => Cargo::Flatbed,
        "extraction" | "agriculture" => Cargo::Hopper,
        _ => Cargo::Box,
    }
}

/// The ship that carries `cargo`.
pub fn ship_of(cargo: Cargo, river: bool) -> ShipKind {
    if river {
        return ShipKind::Barge;
    }
    match cargo {
        Cargo::Tanker => ShipKind::Tanker,
        Cargo::Hopper | Cargo::Flatbed => ShipKind::Bulk,
        Cargo::Box => ShipKind::Container,
    }
}

/// Container colours: one pick per stack, from the palette.
fn box_colour(k: u32) -> Rgb {
    match k % 6 {
        0 => colour(ramp::BRICK, 2),
        1 => colour(ramp::WATER, 3),
        2 => colour(ramp::FIELD, 2),
        3 => colour(ramp::WATER, 2),
        4 => colour(ramp::CLOUD, 2),
        _ => colour(ramp::SOIL, 2),
    }
}

/// A truck at `at` facing `yaw`: wheels, chassis, cab and the trailer of `cargo`. `u` is the unit
/// of the model (about a third of the truck's length); `seed` varies its colours.
pub fn truck(m: &mut Mesh, at: [f32; 3], yaw: f32, u: f32, cargo: Cargo, seed: u32) {
    let b = |m: &mut Mesh, min: [f32; 3], max: [f32; 3], rgb: Rgb| {
        put_turned(
            m,
            at,
            yaw,
            [min[0] * u, min[1] * u, min[2] * u],
            [max[0] * u, max[1] * u, max[2] * u],
            rgb,
        )
    };
    let ink = colour(ramp::INK, 0);
    let cab = match hash(seed, 1) {
        h if h < 0.34 => colour(ramp::BRICK, 2),
        h if h < 0.67 => colour(ramp::WATER, 3),
        _ => colour(ramp::FOREST, 3),
    };
    // Chassis and the three axles.
    b(m, [-1.7, -0.6, 0.3], [1.6, 0.6, 0.5], colour(ramp::INK, 2));
    for (x0, x1) in [(-1.5, -0.95), (-0.8, -0.25), (0.8, 1.35)] {
        b(m, [x0, -0.82, 0.0], [x1, 0.82, 0.42], ink);
    }
    // Cab with a hood and a windscreen.
    b(m, [0.5, -0.72, 0.5], [1.25, 0.72, 1.55], cab);
    b(m, [1.25, -0.7, 0.5], [1.62, 0.7, 1.0], cab);
    b(
        m,
        [1.24, -0.52, 1.08],
        [1.3, 0.52, 1.45],
        colour(ramp::WATER, 1),
    );
    match cargo {
        Cargo::Box => {
            b(
                m,
                [-1.7, -0.78, 0.5],
                [0.42, 0.78, 1.95],
                colour(ramp::CLOUD, 2),
            );
            b(m, [-1.7, -0.8, 1.1], [0.42, 0.8, 1.3], cab);
        }
        Cargo::Tanker => {
            let tank = colour(ramp::ROCK, 3);
            b(m, [-1.68, -0.52, 0.5], [0.4, 0.52, 0.85], tank);
            b(m, [-1.7, -0.76, 0.85], [0.42, 0.76, 1.55], tank);
            b(m, [-1.68, -0.52, 1.55], [0.4, 0.52, 1.9], tank);
            b(
                m,
                [-1.7, -0.78, 1.1],
                [0.42, 0.78, 1.3],
                colour(ramp::BRICK, 2),
            );
            b(m, [-0.9, -0.2, 1.9], [-0.6, 0.2, 2.05], ink);
        }
        Cargo::Flatbed => {
            b(
                m,
                [-1.7, -0.8, 0.5],
                [0.42, 0.8, 0.65],
                colour(ramp::INK, 3),
            );
            if hash(seed, 2) < 0.5 {
                // Steel: plates in two stacks.
                b(
                    m,
                    [-1.6, -0.6, 0.65],
                    [-0.7, 0.6, 1.25],
                    colour(ramp::ROCK, 1),
                );
                b(
                    m,
                    [-0.6, -0.6, 0.65],
                    [0.3, 0.6, 1.05],
                    colour(ramp::ROCK, 2),
                );
            } else {
                // Timber: three layers of logs.
                b(
                    m,
                    [-1.65, -0.7, 0.65],
                    [0.35, 0.7, 0.95],
                    colour(ramp::SOIL, 1),
                );
                b(
                    m,
                    [-1.65, -0.5, 0.95],
                    [0.35, 0.5, 1.2],
                    colour(ramp::SOIL, 2),
                );
                b(
                    m,
                    [-1.65, -0.3, 1.2],
                    [0.35, 0.3, 1.4],
                    colour(ramp::SOIL, 1),
                );
            }
        }
        Cargo::Hopper => {
            let ore = hash(seed, 3) < 0.5;
            b(m, [-1.6, -0.5, 0.5], [0.3, 0.5, 0.9], colour(ramp::ROCK, 2));
            b(
                m,
                [-1.7, -0.78, 0.9],
                [0.42, 0.78, 1.7],
                colour(ramp::ROCK, 2),
            );
            let heap = if ore {
                colour(ramp::ROCK, 0)
            } else {
                colour(ramp::FIELD, 3)
            };
            b(m, [-1.55, -0.6, 1.7], [0.27, 0.6, 1.95], heap);
        }
    }
}

/// A wagon of `cargo` with its centre at `at`, `u` the unit (a wagon is two units long).
fn wagon(m: &mut Mesh, at: [f32; 3], yaw: f32, u: f32, cargo: Cargo, seed: u32) {
    let b = |m: &mut Mesh, min: [f32; 3], max: [f32; 3], rgb: Rgb| {
        put_turned(
            m,
            at,
            yaw,
            [min[0] * u, min[1] * u, min[2] * u],
            [max[0] * u, max[1] * u, max[2] * u],
            rgb,
        )
    };
    // Two bogies under every wagon.
    for x in [-0.7f32, 0.4] {
        b(
            m,
            [x, -0.62, 0.0],
            [x + 0.3, 0.62, 0.3],
            colour(ramp::INK, 0),
        );
    }
    match cargo {
        Cargo::Box => {
            let c = if hash(seed, 4) < 0.5 {
                colour(ramp::BRICK, 1)
            } else {
                colour(ramp::SOIL, 1)
            };
            b(m, [-1.0, -0.66, 0.28], [1.0, 0.66, 1.45], c);
            b(
                m,
                [-1.02, -0.2, 0.5],
                [1.02, 0.2, 1.2],
                colour(ramp::INK, 3),
            );
        }
        Cargo::Tanker => {
            let tank = colour(ramp::ROCK, 3);
            b(m, [-0.95, -0.42, 0.28], [0.95, 0.42, 0.6], tank);
            b(m, [-1.0, -0.64, 0.6], [1.0, 0.64, 1.15], tank);
            b(m, [-0.95, -0.42, 1.15], [0.95, 0.42, 1.4], tank);
            b(
                m,
                [-1.02, -0.66, 0.75],
                [1.02, 0.66, 0.9],
                colour(ramp::BRICK, 2),
            );
        }
        Cargo::Flatbed => {
            b(
                m,
                [-1.0, -0.66, 0.28],
                [1.0, 0.66, 0.42],
                colour(ramp::INK, 3),
            );
            if hash(seed, 5) < 0.5 {
                b(
                    m,
                    [-0.9, -0.5, 0.42],
                    [0.9, 0.5, 0.9],
                    colour(ramp::ROCK, 1),
                );
            } else {
                b(
                    m,
                    [-0.9, -0.55, 0.42],
                    [0.9, 0.55, 0.75],
                    colour(ramp::SOIL, 1),
                );
                b(
                    m,
                    [-0.9, -0.35, 0.75],
                    [0.9, 0.35, 0.98],
                    colour(ramp::SOIL, 2),
                );
            }
        }
        Cargo::Hopper => {
            b(
                m,
                [-0.9, -0.45, 0.28],
                [0.9, 0.45, 0.7],
                colour(ramp::ROCK, 2),
            );
            b(
                m,
                [-1.0, -0.66, 0.7],
                [1.0, 0.66, 1.3],
                colour(ramp::ROCK, 2),
            );
            let heap = if hash(seed, 6) < 0.5 {
                colour(ramp::ROCK, 0)
            } else {
                colour(ramp::FIELD, 3)
            };
            b(m, [-0.9, -0.52, 1.3], [0.9, 0.52, 1.5], heap);
        }
    }
}

/// The locomotive, centred at `at`; two units long like a wagon.
fn locomotive(m: &mut Mesh, at: [f32; 3], yaw: f32, u: f32, seed: u32) {
    let b = |m: &mut Mesh, min: [f32; 3], max: [f32; 3], rgb: Rgb| {
        put_turned(
            m,
            at,
            yaw,
            [min[0] * u, min[1] * u, min[2] * u],
            [max[0] * u, max[1] * u, max[2] * u],
            rgb,
        )
    };
    let body = if hash(seed, 7) < 0.5 {
        colour(ramp::BRICK, 2)
    } else {
        colour(ramp::WATER, 2)
    };
    for x in [-0.8f32, 0.5] {
        b(
            m,
            [x, -0.62, 0.0],
            [x + 0.3, 0.62, 0.3],
            colour(ramp::INK, 0),
        );
    }
    b(m, [-1.05, -0.68, 0.28], [1.05, 0.68, 1.1], body);
    // The cab at the back, the hood in front, a stripe and a headlamp.
    b(m, [-1.05, -0.68, 1.1], [-0.15, 0.68, 1.65], body);
    b(
        m,
        [-0.5, -0.5, 1.1],
        [-0.2, 0.5, 1.5],
        colour(ramp::WATER, 1),
    );
    b(m, [0.7, -0.4, 1.1], [1.0, 0.4, 1.3], colour(ramp::INK, 2));
    b(
        m,
        [-1.07, -0.7, 0.55],
        [1.07, 0.7, 0.72],
        colour(ramp::SIGNAL, 1),
    );
}

/// A train with the locomotive at `at` (the front) and `wagons` wagons behind, each placed with
/// `place` (position and yaw at a distance behind the front, in kilometres).
pub fn train(
    m: &mut Mesh,
    place: &dyn Fn(f32) -> ([f32; 3], f32),
    u: f32,
    cargo: Cargo,
    wagons: u32,
    seed: u32,
) {
    let (p, y) = place(0.0);
    locomotive(m, p, y, u, seed);
    for w in 0..wagons {
        let (p, y) = place((w + 1) as f32 * 2.3 * u);
        wagon(m, p, y, u, cargo, seed + w * 3);
    }
}

/// A ship at `at` facing `yaw`, `u` a tenth of its length.
pub fn ship(m: &mut Mesh, at: [f32; 3], yaw: f32, u: f32, kind: ShipKind, seed: u32) {
    let b = |m: &mut Mesh, min: [f32; 3], max: [f32; 3], rgb: Rgb| {
        put_turned(
            m,
            at,
            yaw,
            [min[0] * u, min[1] * u, min[2] * u],
            [max[0] * u, max[1] * u, max[2] * u],
            rgb,
        )
    };
    let hull = colour(ramp::INK, 2);
    let white = colour(ramp::CLOUD, 3);
    let red = colour(ramp::BRICK, 1);
    if kind == ShipKind::Barge {
        // A long low barge with a heap, pushed by a tug at the stern.
        b(m, [-1.1, -0.62, 0.0], [1.7, 0.62, 0.38], hull);
        b(m, [-1.12, -0.64, 0.0], [1.72, 0.64, 0.14], red);
        let heap = if hash(seed, 8) < 0.5 {
            colour(ramp::ROCK, 0)
        } else {
            colour(ramp::SOIL, 1)
        };
        b(
            m,
            [-0.9, -0.48, 0.38],
            [1.5, 0.48, 0.62],
            colour(ramp::ROCK, 2),
        );
        b(m, [-0.8, -0.4, 0.62], [1.4, 0.4, 0.82], heap);
        b(m, [-1.9, -0.4, 0.0], [-1.1, 0.4, 0.35], hull);
        b(m, [-1.8, -0.34, 0.35], [-1.2, 0.34, 0.8], white);
        b(
            m,
            [-1.7, -0.26, 0.8],
            [-1.4, 0.26, 1.0],
            colour(ramp::WATER, 1),
        );
        return;
    }
    // The hull: a red waterline, the dark side, a stepped bow.
    b(m, [-1.8, -0.8, 0.0], [1.2, 0.8, 0.7], hull);
    b(m, [1.2, -0.55, 0.0], [1.75, 0.55, 0.7], hull);
    b(m, [1.75, -0.28, 0.0], [2.0, 0.28, 0.7], hull);
    b(m, [-1.82, -0.82, 0.0], [1.22, 0.82, 0.2], red);
    b(m, [1.2, -0.57, 0.0], [1.77, 0.57, 0.2], red);
    // The accommodation at the stern, the bridge on top and a funnel.
    b(m, [-1.7, -0.62, 0.7], [-0.95, 0.62, 1.55], white);
    b(m, [-1.55, -0.5, 1.55], [-1.1, 0.5, 1.9], white);
    b(
        m,
        [-1.54, -0.52, 1.7],
        [-1.4, 0.52, 1.82],
        colour(ramp::WATER, 1),
    );
    b(
        m,
        [-1.35, -0.16, 1.55],
        [-1.15, 0.16, 2.1],
        colour(ramp::BRICK, 2),
    );
    match kind {
        ShipKind::Bulk => {
            // Five hatches along the deck and a pair of derricks.
            for k in 0..5 {
                let x = -0.8 + k as f32 * 0.38;
                b(
                    m,
                    [x, -0.5, 0.7],
                    [x + 0.28, 0.5, 0.92],
                    colour(ramp::ROCK, 2),
                );
            }
            b(
                m,
                [-0.4, -0.06, 0.92],
                [-0.34, 0.06, 1.5],
                colour(ramp::ROCK, 3),
            );
            b(
                m,
                [0.74, -0.06, 0.92],
                [0.8, 0.06, 1.5],
                colour(ramp::ROCK, 3),
            );
        }
        ShipKind::Tanker => {
            // A red deck, a pipe run down the middle and the domes of the tanks.
            b(m, [-0.95, -0.76, 0.7], [1.2, 0.76, 0.78], red);
            b(
                m,
                [-0.9, -0.05, 0.78],
                [1.7, 0.05, 0.92],
                colour(ramp::ROCK, 3),
            );
            for k in 0..5 {
                let x = -0.7 + k as f32 * 0.42;
                b(
                    m,
                    [x, -0.3, 0.78],
                    [x + 0.2, 0.3, 0.92],
                    colour(ramp::ROCK, 2),
                );
            }
        }
        ShipKind::Container => {
            // Stacks of containers in three rows across, a few layers high.
            for bay in 0..5 {
                let x = -0.85 + bay as f32 * 0.4;
                for row in 0..3 {
                    let y = -0.56 + row as f32 * 0.38;
                    let layers = 1 + (hash(seed + bay * 7 + row, 9) * 3.0) as u32;
                    for l in 0..layers {
                        let z = 0.7 + l as f32 * 0.2;
                        b(
                            m,
                            [x, y, z],
                            [x + 0.36, y + 0.34, z + 0.19],
                            box_colour(seed + bay * 5 + row * 3 + l),
                        );
                    }
                }
            }
        }
        ShipKind::Barge => {}
    }
}

/// A quay with its yard and cranes. `at` is the middle of the quay edge, `yaw` points from the
/// land out over the water; the quay runs along the edge, `len` km long.
pub struct Harbour {
    pub at: [f32; 3],
    pub yaw: f32,
    pub len: f32,
    pub cranes: u32,
    pub seed: u32,
}

/// Harbour parts are never thinner than `min` km, so a crane leg stays a few pixels wide.
fn thick(world: f32, min: f32) -> f32 {
    world.max(min)
}

impl Harbour {
    /// Where a ship lies alongside: the middle of the berth, and the yaw along the quay.
    pub fn berth(&self) -> ([f32; 3], f32) {
        let (s, c) = self.yaw.sin_cos();
        (
            [self.at[0] + c * 0.07, self.at[1] + s * 0.07, self.at[2]],
            self.yaw + std::f32::consts::FRAC_PI_2,
        )
    }
}

/// Draw a harbour: the paved quay, rows of stacked containers behind it and gantry cranes along
/// its edge whose trolleys run out to the ship and back with the clock. `px_km` is the size of a
/// screen pixel; containers are drawn in blocks that double in size with each zoom step so a
/// block never shrinks below a few pixels.
pub fn harbour(m: &mut Mesh, h: &Harbour, px_km: f32, t: f32, animated: bool) {
    let back = 0.34f32;
    let len = h.len;
    let place = |m: &mut Mesh, min: [f32; 3], max: [f32; 3], rgb: Rgb| {
        put_turned(m, h.at, h.yaw, min, max, rgb)
    };
    // The quay slab and a darker kerb along the water.
    place(
        m,
        [-back, -len / 2.0, 0.0],
        [0.0, len / 2.0, 0.006],
        colour(ramp::ROCK, 2),
    );
    place(
        m,
        [-0.012, -len / 2.0, 0.006],
        [0.0, len / 2.0, 0.012],
        colour(ramp::ROCK, 0),
    );
    // The yard: blocks of containers, stepped in size by zoom.
    let mut step = 1.0f32;
    while 0.012 * step < px_km * 2.5 && step < 16.0 {
        step *= 2.0;
    }
    let (bx, by) = (0.013 * step, 0.034 * step);
    let rows = ((back - 0.12) / (bx * 1.5)).floor().max(1.0) as i32;
    let cols = ((len - 0.04) / (by * 1.2)).floor().max(1.0) as i32;
    let lane = (cols / 2).max(1);
    for r in 0..rows {
        for c in 0..cols {
            if c % lane == lane - 1 && cols > 3 {
                continue; // a gap for the trucks
            }
            let hh = hash(h.seed + (r * 31 + c) as u32 * 7, 13);
            if hh < 0.12 {
                continue;
            }
            let x = -0.1 - r as f32 * bx * 1.5 - bx;
            let y = -len / 2.0 + 0.02 + c as f32 * by * 1.2;
            let layers = 1 + (hh * 4.0) as u32 % 3;
            let top = 0.006 + 0.0087 * step.min(2.0) * layers as f32;
            place(
                m,
                [x, y, 0.006],
                [x + bx, y + by, top],
                box_colour(h.seed + (r * 5 + c) as u32 * 3),
            );
        }
    }
    // The gantry cranes: two portal legs astride the quay edge, a boom reaching out over the
    // ship and back over the yard, and a trolley with a hanging box that runs along it.
    let leg = thick(0.004, px_km * 1.6);
    let tall = thick(0.05, px_km * 14.0).min(0.18);
    let gauge = 0.045f32.max(leg * 5.0);
    let steel = colour(ramp::ROCK, 3);
    let dark = colour(ramp::ROCK, 1);
    for k in 0..h.cranes {
        let y = -len / 2.0 + len * (k as f32 + 0.5) / h.cranes as f32;
        for (x, yy) in [(-0.05, -1.0f32), (-0.05, 1.0), (0.0, -1.0), (0.0, 1.0)] {
            place(
                m,
                [
                    x - leg * 0.5 - gauge * 0.0,
                    y + yy * gauge * 0.5 - leg * 0.5,
                    0.006,
                ],
                [x + leg * 0.5, y + yy * gauge * 0.5 + leg * 0.5, tall],
                dark,
            );
        }
        // The boom: from the yard side out over the water.
        let reach = 0.13f32.max(leg * 12.0);
        let rear = 0.11f32.max(leg * 10.0);
        place(
            m,
            [-0.05 - rear, y - gauge * 0.5, tall],
            [reach, y + gauge * 0.5, tall + leg * 1.2],
            steel,
        );
        place(
            m,
            [-0.05 - rear * 0.2, y - leg, tall + leg * 1.2],
            [-0.05 + leg * 2.0, y + leg, tall + leg * 4.0],
            dark,
        );
        // The trolley runs out to the ship and back in eight whole steps.
        let phase = if animated {
            (t / period(24.0) + hash(h.seed, 20 + k)).fract()
        } else {
            hash(h.seed, 20 + k) * 0.5
        };
        let tri = if phase < 0.5 {
            phase * 2.0
        } else {
            2.0 - phase * 2.0
        };
        let stop = (tri * 8.0).floor() / 8.0;
        let tx = -0.08 + (reach - 0.03 + 0.08) * stop;
        place(
            m,
            [tx - leg * 1.4, y - leg * 1.4, tall - leg * 1.5],
            [tx + leg * 1.4, y + leg * 1.4, tall],
            colour(ramp::GOLD, 1),
        );
        // The load hangs a fixed way down and is a box when the trolley is out over the ship.
        let drop = tall * (0.5 - 0.35 * (stop - 0.5).abs());
        place(
            m,
            [tx - leg * 0.3, y - leg * 0.3, tall - drop],
            [tx + leg * 0.3, y + leg * 0.3, tall - leg * 1.5],
            colour(ramp::INK, 2),
        );
        if stop > 0.4 {
            let bw = thick(0.012, px_km * 2.5);
            place(
                m,
                [tx - bw, y - bw, tall - drop - bw],
                [tx + bw, y + bw, tall - drop],
                box_colour(h.seed + k),
            );
        }
    }
}

/// The bright outline of a hovered or selected plant: a box of thin bars round its footprint at
/// the ground and at the roof with a post at each corner. A hovered plant gets the bar one pixel
/// wide in the player's gold or a rival's white. A selected one pulses in whole steps between
/// two palette colours and one and two pixel bars.
#[allow(clippy::too_many_arguments)]
pub fn outline(
    m: &mut Mesh,
    p: &Placement,
    frame: [f32; 4],
    half: [f32; 2],
    height: f32,
    px_km: f32,
    t: f32,
    selected: bool,
    animated: bool,
) {
    let [cx, cy, z, _] = frame;
    let player = p.owner == crate::data::Owner::Player;
    let (bright, dim) = if player {
        (colour(ramp::GOLD, 3), colour(ramp::GOLD, 2))
    } else {
        (colour(ramp::CLOUD, 3), colour(ramp::CLOUD, 1))
    };
    let (rgb, bar) = if selected {
        let step = if animated {
            ((t / period(1.2)).fract() * 4.0).floor() as u32
        } else {
            0
        };
        (
            if step % 2 == 0 { bright } else { dim },
            px_km * if step == 1 || step == 2 { 2.0 } else { 1.0 },
        )
    } else {
        (bright, px_km)
    };
    let at = [cx, cy, z];
    let yaw = p.yaw.to_radians();
    let (hx, hy) = (half[0] + bar, half[1] + bar);
    let top = height + bar;
    let mut bars = |min: [f32; 3], max: [f32; 3]| put_turned(m, at, yaw, min, max, rgb);
    for zz in [0.0, top - bar] {
        bars([-hx, -hy, zz], [hx, -hy + bar, zz + bar]);
        bars([-hx, hy - bar, zz], [hx, hy, zz + bar]);
        bars([-hx, -hy, zz], [-hx + bar, hy, zz + bar]);
        bars([hx - bar, -hy, zz], [hx, hy, zz + bar]);
    }
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let (x, y) = (sx * (hx - bar / 2.0), sy * (hy - bar / 2.0));
        bars(
            [x - bar / 2.0, y - bar / 2.0, 0.0],
            [x + bar / 2.0, y + bar / 2.0, top],
        );
    }
}
