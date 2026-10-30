//! Canonical state hashing.
//!
//! Fields are written in an explicitly enumerated order with fixed-width little-endian
//! primitives and length-prefixed strings. The hash covers authoritative state only:
//! profiling counters, UI caches and report caches are excluded by construction because
//! they are never passed to this writer.

use blake3::Hasher;

/// Domain separation so a save checksum and a state hash can never collide.
pub const DOMAIN_STATE: &[u8] = b"CWSTATE1";
pub const DOMAIN_SAVE: &[u8] = b"CWSAVE01";

#[derive(Clone, Debug)]
pub struct CanonicalHasher {
    hasher: Hasher,
    /// Small writes gather here and reach BLAKE3 in one call; the digest is the same as feeding
    /// each field on its own, because the hash is a stream.
    pending: [u8; PENDING],
    used: usize,
}

const PENDING: usize = 512;

impl CanonicalHasher {
    pub fn new(domain: &'static [u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(domain);
        CanonicalHasher {
            hasher,
            pending: [0; PENDING],
            used: 0,
        }
    }

    #[inline]
    fn put(&mut self, value: &[u8]) {
        if value.len() > PENDING - self.used {
            self.flush();
            if value.len() >= PENDING {
                self.hasher.update(value);
                return;
            }
        }
        self.pending[self.used..self.used + value.len()].copy_from_slice(value);
        self.used += value.len();
    }

    fn flush(&mut self) {
        if self.used > 0 {
            self.hasher.update(&self.pending[..self.used]);
            self.used = 0;
        }
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.put(&[value]);
        self
    }

    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.u8(u8::from(value))
    }

    pub fn u16(&mut self, value: u16) -> &mut Self {
        self.put(&value.to_le_bytes());
        self
    }

    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.put(&value.to_le_bytes());
        self
    }

    pub fn u64(&mut self, value: u64) -> &mut Self {
        self.put(&value.to_le_bytes());
        self
    }

    pub fn i64(&mut self, value: i64) -> &mut Self {
        self.put(&value.to_le_bytes());
        self
    }

    pub fn i32(&mut self, value: i32) -> &mut Self {
        self.put(&value.to_le_bytes());
        self
    }

    pub fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.u64(value.len() as u64);
        self.put(value);
        self
    }

    pub fn string(&mut self, value: &str) -> &mut Self {
        self.bytes(value.as_bytes())
    }

    pub fn finish(&self) -> [u8; 32] {
        let mut hasher = self.hasher.clone();
        hasher.update(&self.pending[..self.used]);
        *hasher.finalize().as_bytes()
    }

    pub fn finish_hex(&self) -> String {
        self.finish()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

/// BLAKE3 over an arbitrary byte slice, used for save payload checksums.
pub fn digest(domain: &'static [u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_order_sensitive() {
        let mut a = CanonicalHasher::new(DOMAIN_STATE);
        a.u32(1).u32(2);
        let mut b = CanonicalHasher::new(DOMAIN_STATE);
        b.u32(2).u32(1);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn batched_writes_hash_like_one_stream() {
        // The reference feeds BLAKE3 each field on its own; sizes cross the batch boundary.
        for long in [0usize, 1, 7, 100, 503, 504, 511, 512, 513, 1500, 4096] {
            let text = "x".repeat(long);
            let mut reference = Hasher::new();
            reference.update(DOMAIN_STATE);
            for round in 0..40u32 {
                reference.update(&round.to_le_bytes());
                reference.update(&(round as u64 * 31).to_le_bytes());
                reference.update(&(text.len() as u64).to_le_bytes());
                reference.update(text.as_bytes());
                reference.update(&[1u8]);
            }
            let mut batched = CanonicalHasher::new(DOMAIN_STATE);
            for round in 0..40u32 {
                batched
                    .u32(round)
                    .u64(round as u64 * 31)
                    .string(&text)
                    .bool(true);
            }
            assert_eq!(batched.finish(), *reference.finalize().as_bytes(), "{long}");
            // Finishing twice, and after more writes, stays consistent.
            assert_eq!(batched.finish(), batched.finish());
        }
    }

    #[test]
    fn hash_is_length_prefixed_for_strings() {
        let mut a = CanonicalHasher::new(DOMAIN_STATE);
        a.string("ab").string("c");
        let mut b = CanonicalHasher::new(DOMAIN_STATE);
        b.string("a").string("bc");
        assert_ne!(a.finish(), b.finish());
    }
}
