//! Save envelope, checksum and recovery.
//!
//! Envelope: magic, schema, rules version, data identity, engine build, career ID, tick,
//! revision, declared payload length, BLAKE3 checksum and the encoded world. Length bounds are
//! validated before allocation and the checksum before parsing. A corrupt, truncated, oversized
//! or future payload is rejected with an actionable typed error and imports build a new world
//! first, so a failed import never damages the live session.

use cw_core::RULES_VERSION;
use cw_core::error::{DomainError, SaveFault};
use cw_core::hash::{DOMAIN_SAVE, digest};
use cw_sim::state::World;

pub const MAGIC: &[u8; 4] = b"CWSV";
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

pub fn encode(world: &World, career_id: u64) -> Result<Vec<u8>, DomainError> {
    let payload = postcard::to_allocvec(world).map_err(|_| DomainError::InvalidSave {
        detail: SaveFault::MalformedPayload,
    })?;
    if payload.len() > MAX_BYTES {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Oversize,
        });
    }
    let checksum = digest(DOMAIN_SAVE, &payload);
    let mut out = Vec::with_capacity(payload.len() + 160);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    out.extend_from_slice(&RULES_VERSION.to_le_bytes());
    write_string(&mut out, &world.dataset_hash);
    write_string(&mut out, cw_core::ENGINE_VERSION);
    out.extend_from_slice(&career_id.to_le_bytes());
    out.extend_from_slice(&world.tick.0.to_le_bytes());
    out.extend_from_slice(&world.revision.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&checksum);
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<(World, u64), DomainError> {
    if bytes.len() > MAX_BYTES {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Oversize,
        });
    }
    let mut cursor = 0usize;
    if bytes.len() < 4 || &bytes[..4] != MAGIC {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::BadMagic,
        });
    }
    cursor += 4;
    let schema = read_u32(bytes, &mut cursor)?;
    if schema > SCHEMA_VERSION {
        return Err(DomainError::UnsupportedVersion {
            found: schema,
            supported: SCHEMA_VERSION,
        });
    }
    let _rules = read_u32(bytes, &mut cursor)?;
    let _data = read_string(bytes, &mut cursor)?;
    let _engine = read_string(bytes, &mut cursor)?;
    let career_id = read_u64(bytes, &mut cursor)?;
    let _tick = read_u32(bytes, &mut cursor)?;
    let _revision = read_u64(bytes, &mut cursor)?;
    let declared = read_u64(bytes, &mut cursor)? as usize;
    if declared > MAX_BYTES {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Oversize,
        });
    }
    if bytes.len() < cursor + 32 {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let mut checksum = [0u8; 32];
    checksum.copy_from_slice(&bytes[cursor..cursor + 32]);
    cursor += 32;
    if bytes.len() < cursor + declared {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let payload = &bytes[cursor..cursor + declared];
    if digest(DOMAIN_SAVE, payload) != checksum {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Checksum,
        });
    }
    let world: World = postcard::from_bytes(payload).map_err(|_| DomainError::InvalidSave {
        detail: SaveFault::MalformedPayload,
    })?;
    Ok((world, career_id))
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, DomainError> {
    if bytes.len() < *cursor + 4 {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let value = u32::from_le_bytes(bytes[*cursor..*cursor + 4].try_into().unwrap());
    *cursor += 4;
    Ok(value)
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, DomainError> {
    if bytes.len() < *cursor + 8 {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let value = u64::from_le_bytes(bytes[*cursor..*cursor + 8].try_into().unwrap());
    *cursor += 8;
    Ok(value)
}

fn read_string(bytes: &[u8], cursor: &mut usize) -> Result<String, DomainError> {
    if bytes.len() < *cursor + 2 {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let length = u16::from_le_bytes(bytes[*cursor..*cursor + 2].try_into().unwrap()) as usize;
    *cursor += 2;
    if bytes.len() < *cursor + length {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let value = String::from_utf8(bytes[*cursor..*cursor + length].to_vec()).map_err(|_| {
        DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        }
    })?;
    *cursor += length;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cw_data::Dataset;
    use cw_sim::economy::seed;
    use cw_sim::state::{ScenarioKind, ScenarioMode};

    fn world() -> World {
        let dataset = Dataset::load().unwrap();
        seed::build(&dataset, 161, ScenarioKind::Guided, ScenarioMode::Sandbox)
    }

    #[test]
    fn round_trip_preserves_the_canonical_hash() {
        let world = world();
        let bytes = encode(&world, 9).unwrap();
        let (restored, career) = decode(&bytes).unwrap();
        assert_eq!(career, 9);
        assert_eq!(
            cw_sim::state::canonical_hash(&world),
            cw_sim::state::canonical_hash(&restored)
        );
    }

    #[test]
    fn a_bit_flip_is_rejected_before_parsing() {
        let world = world();
        let mut bytes = encode(&world, 1).unwrap();
        let last = bytes.len() - 5;
        bytes[last] ^= 0x40;
        assert!(matches!(
            decode(&bytes),
            Err(DomainError::InvalidSave { .. })
        ));
    }

    #[test]
    fn a_future_schema_is_rejected() {
        let world = world();
        let mut bytes = encode(&world, 1).unwrap();
        bytes[4..8].copy_from_slice(&(SCHEMA_VERSION + 1).to_le_bytes());
        assert!(matches!(
            decode(&bytes),
            Err(DomainError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn truncation_and_oversize_are_rejected() {
        let world = world();
        let bytes = encode(&world, 1).unwrap();
        assert!(matches!(
            decode(&bytes[..bytes.len() - 10]),
            Err(DomainError::InvalidSave { .. })
        ));
        let oversized = vec![0u8; MAX_BYTES + 1];
        assert!(matches!(
            decode(&oversized),
            Err(DomainError::InvalidSave {
                detail: SaveFault::Oversize
            })
        ));
    }
}
