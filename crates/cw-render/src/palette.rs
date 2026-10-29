//! The one fixed game palette: twelve ramps of four steps, 48 colours. Every colour the
//! renderer outputs is one of these; ground, water, light and cloud all snap to it.

/// Index of a ramp in [`Palette`]. The shader gets the same numbers as WGSL constants.
pub mod ramp {
    pub const INK: usize = 0;
    pub const WATER: usize = 1;
    pub const FOREST: usize = 2;
    pub const FIELD: usize = 3;
    pub const SOIL: usize = 4;
    pub const ROCK: usize = 5;
    pub const SNOW: usize = 6;
    pub const URBAN: usize = 7;
    pub const GOLD: usize = 8;
    pub const CLOUD: usize = 9;
    pub const BRICK: usize = 10;
    pub const SIGNAL: usize = 11;
    pub const COUNT: usize = 12;
}

pub const STEPS: usize = 4;
pub const COLOURS: usize = ramp::COUNT * STEPS;

pub type Rgb = [u8; 3];

/// Day ramps, darkest step first. The UI tokens of docs/rules/VISUAL.md sit inside them:
/// ink, panel, raised panel and stroke are the INK ramp; water #163555 is WATER step 1; gold
/// #E2B04A is GOLD step 3; text and secondary are CLOUD steps 3 and 1; the signal colours
/// are positive, warning, negative and copper.
const DAY: [[Rgb; STEPS]; ramp::COUNT] = [
    [[0x0B, 0x16, 0x22], [0x13, 0x21, 0x2F], [0x1A, 0x2B, 0x3C], [0x22, 0x36, 0x4A]],
    [[0x0F, 0x2B, 0x4A], [0x16, 0x35, 0x55], [0x1E, 0x47, 0x72], [0x2D, 0x63, 0x96]],
    [[0x1B, 0x3A, 0x22], [0x28, 0x50, 0x2B], [0x3A, 0x6B, 0x35], [0x54, 0x8A, 0x45]],
    [[0x4B, 0x5A, 0x26], [0x66, 0x7A, 0x33], [0x8A, 0x9A, 0x45], [0xB0, 0xB6, 0x5A]],
    [[0x4A, 0x3A, 0x2A], [0x6B, 0x54, 0x40], [0x8C, 0x75, 0x56], [0xAF, 0x9A, 0x74]],
    [[0x3B, 0x3F, 0x46], [0x56, 0x5B, 0x63], [0x7A, 0x7F, 0x86], [0xA3, 0xA8, 0xAE]],
    [[0x6F, 0x86, 0xA8], [0x9D, 0xB2, 0xCC], [0xC9, 0xD8, 0xEA], [0xF2, 0xF7, 0xFC]],
    [[0x2B, 0x3B, 0x4A], [0x47, 0x53, 0x64], [0x6A, 0x78, 0x89], [0x93, 0xA2, 0xB4]],
    [[0x5E, 0x47, 0x18], [0x8C, 0x6A, 0x25], [0xBC, 0x92, 0x38], [0xE2, 0xB0, 0x4A]],
    [[0x6B, 0x7C, 0x93], [0x93, 0xA4, 0xB8], [0xC9, 0xD4, 0xE2], [0xE8, 0xEE, 0xF5]],
    [[0x5C, 0x20, 0x1B], [0x7F, 0x2B, 0x23], [0xB2, 0x3A, 0x2E], [0xD9, 0x59, 0x4A]],
    [[0x3F, 0xBF, 0x8A], [0xF2, 0xB7, 0x48], [0xE5, 0x60, 0x5E], [0xC9, 0x80, 0x4A]],
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub ramps: [[Rgb; STEPS]; ramp::COUNT],
}

impl Palette {
    pub fn day() -> Palette {
        Palette { ramps: DAY }
    }

    /// The night palette is a palette swap, not an overlay: every ramp is remapped to a
    /// cooler, darker one. Gold keeps most of its warmth so the player's regions still read,
    /// and the signal ramp is untouched so lit windows keep their glow.
    pub fn night() -> Palette {
        let mut ramps = DAY;
        for (i, r) in ramps.iter_mut().enumerate() {
            if i == ramp::SIGNAL {
                continue;
            }
            let (gain, lift): ([f32; 3], [f32; 3]) = match i {
                ramp::GOLD => ([0.62, 0.55, 0.42], [4.0, 4.0, 10.0]),
                ramp::INK => ([0.55, 0.62, 0.78], [2.0, 4.0, 10.0]),
                ramp::CLOUD => ([0.26, 0.33, 0.46], [3.0, 6.0, 16.0]),
                _ => ([0.34, 0.44, 0.72], [3.0, 7.0, 22.0]),
            };
            for c in r.iter_mut() {
                for k in 0..3 {
                    c[k] = (c[k] as f32 * gain[k] + lift[k]).clamp(0.0, 255.0) as u8;
                }
            }
        }
        Palette { ramps }
    }

    /// Flattened for the shader: ramp-major, darkest step first.
    pub fn flat(&self) -> [[f32; 4]; COLOURS] {
        let mut out = [[0.0; 4]; COLOURS];
        for (i, r) in self.ramps.iter().enumerate() {
            for (j, c) in r.iter().enumerate() {
                out[i * STEPS + j] = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
            }
        }
        out
    }

    pub fn colours(&self) -> Vec<Rgb> {
        self.ramps.iter().flatten().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forty_eight_distinct_colours_in_both_palettes() {
        for p in [Palette::day(), Palette::night()] {
            let mut c = p.colours();
            assert_eq!(c.len(), 48);
            c.sort();
            c.dedup();
            assert_eq!(c.len(), 48);
        }
    }

    #[test]
    fn ramps_get_lighter() {
        for (i, r) in Palette::day().ramps.into_iter().enumerate() {
            if i == ramp::SIGNAL {
                continue;
            }
            let lum = |c: Rgb| c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32;
            for w in r.windows(2) {
                assert!(lum(w[0]) <= lum(w[1]));
            }
        }
    }
}
