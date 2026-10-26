//! Pinned deterministic random streams.
//!
//! Algorithm: SplitMix64 exactly as specified in `docs/rules/INTERFACE.md`. Every subsystem
//! owns a stream seeded from `master_seed ^ tag`, and per-entity draws derive their own
//! stream from that subsystem seed and the stable entity handle, so adding an unrelated
//! entity cannot reorder another entity's draws. Integer uniforms use rejection sampling;
//! no platform float and no modulo bias.

use serde::{Deserialize, Serialize};

use crate::ids::Handle;

/// Fixed subsystem tags. These numbers are part of the rules version.
pub const TAG_MACRO: u64 = 1;
pub const TAG_EVENTS: u64 = 2;
pub const TAG_AI: u64 = 3;
pub const TAG_GENERATION: u64 = 4;
pub const TAG_INSURANCE: u64 = 5;
pub const TAG_RESEARCH: u64 = 6;
pub const TAG_LABOUR: u64 = 7;
pub const TAG_EXCHANGE: u64 = 8;
pub const TAG_DEALS: u64 = 9;
pub const TAG_POLICY: u64 = 10;
pub const TAG_RAID: u64 = 11;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub const fn new(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// Derive a named subsystem stream from a master seed.
    pub fn stream(master_seed: u64, tag: u64) -> Self {
        Rng::new(scramble(
            master_seed ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15),
        ))
    }

    /// Derive a per-entity stream so unrelated entities cannot perturb this entity's draws.
    pub fn entity_stream(master_seed: u64, tag: u64, handle: Handle) -> Self {
        Rng::new(scramble(
            master_seed ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ handle.0,
        ))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `0..bound` with rejection sampling. `bound == 0` returns 0.
    pub fn uniform_below(&mut self, bound: u64) -> u64 {
        if bound <= 1 {
            return 0;
        }
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next_u64();
            if value >= threshold {
                return value % bound;
            }
        }
    }

    /// Uniform including both endpoints.
    pub fn uniform_inclusive(&mut self, low: i64, high: i64) -> i64 {
        if high <= low {
            return low;
        }
        let span = (high - low) as u64 + 1;
        low + self.uniform_below(span) as i64
    }

    /// True with probability `bps / 10000`.
    pub fn chance_bps(&mut self, bps: i32) -> bool {
        let clamped = bps.clamp(0, 10_000) as u64;
        self.uniform_below(10_000) < clamped
    }

    /// Weighted index selection. Zero weights are skipped; an all-zero weight set returns 0.
    pub fn weighted_index(&mut self, weights: &[i64]) -> usize {
        let total: i64 = weights.iter().copied().filter(|w| *w > 0).sum();
        if total <= 0 {
            return 0;
        }
        let mut roll = self.uniform_below(total as u64) as i64;
        for (index, weight) in weights.iter().enumerate() {
            if *weight <= 0 {
                continue;
            }
            if roll < *weight {
                return index;
            }
            roll -= *weight;
        }
        weights.len().saturating_sub(1)
    }

    pub const fn snapshot(&self) -> u64 {
        self.state
    }

    pub fn restore(state: u64) -> Self {
        Rng { state }
    }
}

/// One SplitMix64 output for a bare seed, used to mix stream inputs.
pub fn scramble(seed: u64) -> u64 {
    Rng::new(seed).next_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_is_reproducible_and_tag_isolated() {
        let mut a = Rng::stream(42, TAG_MACRO);
        let mut b = Rng::stream(42, TAG_MACRO);
        let mut c = Rng::stream(42, TAG_EVENTS);
        let left: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
        let right: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
        assert_eq!(left, right);
        assert_ne!(left[0], c.next_u64());
    }

    #[test]
    fn uniform_below_stays_in_range() {
        let mut rng = Rng::new(7);
        for bound in 1..50u64 {
            for _ in 0..20 {
                assert!(rng.uniform_below(bound) < bound);
            }
        }
    }

    #[test]
    fn entity_streams_are_independent_of_creation_order() {
        let first = Rng::entity_stream(9, TAG_AI, Handle::new(3, 1)).next_u64();
        let second = Rng::entity_stream(9, TAG_AI, Handle::new(3, 1)).next_u64();
        assert_eq!(first, second);
        assert_ne!(
            first,
            Rng::entity_stream(9, TAG_AI, Handle::new(4, 1)).next_u64()
        );
    }

    #[test]
    fn weighted_index_respects_zero_weights() {
        let mut rng = Rng::new(11);
        for _ in 0..100 {
            assert_eq!(rng.weighted_index(&[0, 5, 0]), 1);
        }
    }
}
