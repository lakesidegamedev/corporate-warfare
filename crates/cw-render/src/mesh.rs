//! Geometry: stepped terrace terrain, voxel buildings and the cloud sheet.

use bytemuck::{Pod, Zeroable};

use crate::data::{Scene, VoxelModel, WATER_LAKE};

/// Vertex kinds, carried in the colour alpha byte.
pub const KIND_TOP: u8 = 0;
pub const KIND_WALL: u8 = 1;
pub const KIND_BUILDING: u8 = 2;
pub const KIND_PLAIN: u8 = 3;
pub const KIND_CLOUD: u8 = 4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// Box colour and the kind in alpha.
    pub colour: [u8; 4],
    /// Position inside the model in voxels (windows and trim); unused for terrain.
    pub local: [f32; 3],
}

pub const VERTEX_BYTES: usize = std::mem::size_of::<Vertex>();

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    fn quad(&mut self, p: [[f32; 3]; 4], normal: [f32; 3], colour: [u8; 4], local: [[f32; 3]; 4]) {
        let base = self.vertices.len() as u32;
        for i in 0..4 {
            self.vertices.push(Vertex {
                pos: p[i],
                normal,
                colour,
                local: local[i],
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Surface height of the Great Lakes in metres (published chart datums, rounded). The
/// elevation grid holds the lake beds, so lake cells take their surface instead.
fn lake_surface_m(lon: f32, lat: f32) -> f32 {
    if lon > -79.9 && lat > 43.0 {
        75.0 // Ontario
    } else if lat > 46.0 && lon < -84.4 {
        183.0 // Superior
    } else if lat < 42.9 && lon > -83.6 && lon < -78.8 {
        174.0 // Erie
    } else {
        176.0 // Michigan, Huron, St Clair
    }
}

/// Terrace level of a heightmap cell: water sits on the lake or sea level, land on its step.
pub fn cell_level(scene: &Scene, col: usize, row: usize) -> i32 {
    let hm = &scene.heightmap;
    let (dx, dy) = hm.cell_degrees();
    let lon = hm.bounds.west + (col as f32 + 0.5) * dx;
    let lat = hm.bounds.north - (row as f32 + 0.5) * dy;
    let water = scene.mask.at(lon, lat)[2];
    let metres = hm.at(col, row) as f32;
    let metres = if water == crate::data::WATER_OCEAN {
        0.0
    } else {
        metres.max(0.0)
    };
    let metres = if water == WATER_LAKE {
        lake_surface_m(lon, lat)
    } else {
        metres
    };
    (metres / scene.terrace_m).floor() as i32
}

pub fn level_z_km(scene: &Scene, level: i32) -> f32 {
    level as f32 * scene.terrace_m * scene.exaggeration / 1000.0
}

pub fn terrain(scene: &Scene) -> Mesh {
    let hm = &scene.heightmap;
    let (dx, dy) = hm.cell_degrees();
    let b = &hm.bounds;
    let mut levels = vec![0i32; hm.w * hm.h];
    for r in 0..hm.h {
        for c in 0..hm.w {
            levels[r * hm.w + c] = cell_level(scene, c, r);
        }
    }
    let lvl = |c: usize, r: usize| levels[r * hm.w + c];
    let mut m = Mesh::default();
    let top = [0, 0, 0, KIND_TOP];
    let wall = [0, 0, 0, KIND_WALL];
    let z0 = [0.0; 3];
    for r in 0..hm.h {
        for c in 0..hm.w {
            let (lon0, lon1) = (b.west + c as f32 * dx, b.west + (c + 1) as f32 * dx);
            let (lat1, lat0) = (b.north - r as f32 * dy, b.north - (r + 1) as f32 * dy);
            let (x0, y0) = scene.projection.to_km(lon0, lat0);
            let (x1, y1) = scene.projection.to_km(lon1, lat1);
            let h = lvl(c, r);
            let z = level_z_km(scene, h);
            m.quad(
                [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
                [0.0, 0.0, 1.0],
                top,
                [z0; 4],
            );
            // Walls on the east edge and the south edge, from the higher cell down to the lower.
            if c + 1 < hm.w {
                let o = lvl(c + 1, r);
                if o != h {
                    let (hi, lo, n) = if h > o {
                        (h, o, [1.0, 0.0, 0.0])
                    } else {
                        (o, h, [-1.0, 0.0, 0.0])
                    };
                    let (za, zb) = (level_z_km(scene, lo), level_z_km(scene, hi));
                    m.quad(
                        [[x1, y0, za], [x1, y1, za], [x1, y1, zb], [x1, y0, zb]],
                        n,
                        wall,
                        [z0; 4],
                    );
                }
            }
            if r + 1 < hm.h {
                let o = lvl(c, r + 1);
                if o != h {
                    let (hi, lo, n) = if h > o {
                        (h, o, [0.0, -1.0, 0.0])
                    } else {
                        (o, h, [0.0, 1.0, 0.0])
                    };
                    let (za, zb) = (level_z_km(scene, lo), level_z_km(scene, hi));
                    m.quad(
                        [[x0, y0, za], [x1, y0, za], [x1, y0, zb], [x0, y0, zb]],
                        n,
                        wall,
                        [z0; 4],
                    );
                }
            }
        }
    }
    m
}

fn box_faces(
    m: &mut Mesh,
    tf: &dyn Fn([f32; 3]) -> [f32; 3],
    rot: &dyn Fn([f32; 3]) -> [f32; 3],
    min: [f32; 3],
    max: [f32; 3],
    rgb: [u8; 3],
    kind: u8,
) {
    let c = [rgb[0], rgb[1], rgb[2], kind];
    let (a, b) = (min, max);
    // (corner order counter clockwise seen from outside, outward normal)
    let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
        (
            [
                [a[0], a[1], b[2]],
                [b[0], a[1], b[2]],
                [b[0], b[1], b[2]],
                [a[0], b[1], b[2]],
            ],
            [0.0, 0.0, 1.0],
        ),
        (
            [
                [a[0], a[1], a[2]],
                [a[0], b[1], a[2]],
                [b[0], b[1], a[2]],
                [b[0], a[1], a[2]],
            ],
            [0.0, 0.0, -1.0],
        ),
        (
            [
                [a[0], a[1], a[2]],
                [b[0], a[1], a[2]],
                [b[0], a[1], b[2]],
                [a[0], a[1], b[2]],
            ],
            [0.0, -1.0, 0.0],
        ),
        (
            [
                [b[0], b[1], a[2]],
                [a[0], b[1], a[2]],
                [a[0], b[1], b[2]],
                [b[0], b[1], b[2]],
            ],
            [0.0, 1.0, 0.0],
        ),
        (
            [
                [b[0], a[1], a[2]],
                [b[0], b[1], a[2]],
                [b[0], b[1], b[2]],
                [b[0], a[1], b[2]],
            ],
            [1.0, 0.0, 0.0],
        ),
        (
            [
                [a[0], b[1], a[2]],
                [a[0], a[1], a[2]],
                [a[0], a[1], b[2]],
                [a[0], b[1], b[2]],
            ],
            [-1.0, 0.0, 0.0],
        ),
    ];
    for (corners, n) in faces {
        m.quad(
            [
                tf(corners[0]),
                tf(corners[1]),
                tf(corners[2]),
                tf(corners[3]),
            ],
            rot(n),
            c,
            corners,
        );
    }
}

/// Voxel buildings standing on the terrain, each on a plinth down to the lowest ground under it.
pub fn buildings(scene: &Scene) -> Mesh {
    let mut m = Mesh::default();
    for p in &scene.buildings {
        let Some(model) = scene.models.get(&p.model) else {
            continue;
        };
        let (cx, cy) = scene.projection.to_km(p.lon, p.lat);
        let (hi, lo) = ground_range(scene, model, cx, cy);
        let (z_top, z_low) = (level_z_km(scene, hi), level_z_km(scene, lo));
        let (s, c) = p.yaw.to_radians().sin_cos();
        let v = scene.voxel_km;
        let rot = move |n: [f32; 3]| [n[0] * c - n[1] * s, n[0] * s + n[1] * c, n[2]];
        let tf = move |q: [f32; 3]| {
            [
                cx + (q[0] * c - q[1] * s) * v,
                cy + (q[0] * s + q[1] * c) * v,
                z_top + q[2] * v,
            ]
        };
        let half = [model.footprint[0] / 2.0, model.footprint[1] / 2.0];
        // Plinth: earth coloured, from the lowest ground under the footprint up to the base.
        let drop = (z_top - z_low) / v;
        if drop > 0.0 {
            box_faces(
                &mut m,
                &tf,
                &rot,
                [-half[0], -half[1], -drop],
                [half[0], half[1], 0.0],
                [0x8C, 0x75, 0x56],
                KIND_PLAIN,
            );
        }
        for b in &model.boxes {
            let tall = b.max[2] - b.min[2] >= 2.0 && b.min[2] >= 1.0;
            box_faces(
                &mut m,
                &tf,
                &rot,
                b.min,
                b.max,
                b.colour,
                if tall { KIND_BUILDING } else { KIND_PLAIN },
            );
        }
    }
    m
}

/// Highest and lowest terrace level under a building's footprint.
fn ground_range(scene: &Scene, model: &VoxelModel, cx: f32, cy: f32) -> (i32, i32) {
    let hm = &scene.heightmap;
    let r = model.footprint[0].max(model.footprint[1]) * scene.voxel_km / 2.0;
    let (mut hi, mut lo) = (i32::MIN, i32::MAX);
    for (ox, oy) in [(-r, -r), (r, -r), (r, r), (-r, r), (0.0, 0.0)] {
        let (lon, lat) = scene.projection.to_lonlat(cx + ox, cy + oy);
        let (c, row) = hm.cell_of(lon, lat);
        let l = cell_level(scene, c, row);
        hi = hi.max(l);
        lo = lo.min(l);
    }
    (hi, lo)
}

/// A flat sheet of cloud over the whole terrain extent.
pub fn cloud_sheet(scene: &Scene, altitude_km: f32) -> Mesh {
    let b = &scene.heightmap.bounds;
    let (x0, y0) = scene.projection.to_km(b.west, b.south);
    let (x1, y1) = scene.projection.to_km(b.east, b.north);
    let mut m = Mesh::default();
    let c = [255, 255, 255, KIND_CLOUD];
    let z = altitude_km;
    m.quad(
        [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
        [0.0, 0.0, 1.0],
        c,
        [[0.0; 3]; 4],
    );
    m
}
