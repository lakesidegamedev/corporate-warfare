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
/// The schema is the layout of the postcard world payload (`cw_sim::legacy`). Postcard is not
/// self-describing, so any field added to saved state bumps it, with a frozen copy of the older
/// structs in `cw_sim::legacy`. Schema 1 is every release up to 1.8 preview 2. Trailers are
/// outside the payload and keep their own rule: a reader skips what it does not know.
pub const SCHEMA_VERSION: u32 = cw_sim::legacy::LAYOUT;
/// Oldest schema this engine can read. Schema 0 was never released, so there is no migration
/// path: a payload claiming it is rejected instead of being parsed as if it were schema 1.
pub const MIN_SCHEMA_VERSION: u32 = cw_sim::legacy::MIN_LAYOUT;
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// Marks the optional custom scenario trailer after the world payload.
const CUSTOM_MAGIC: &[u8; 4] = b"CUST";
/// Marks the currency trailer of a world with more than one money, after the custom trailer.
/// A world with one money writes nothing, so its save is byte for byte what it always was.
const FX_MAGIC: &[u8; 4] = b"FXST";
/// Marks the freight read-out trailer, last in the file: the Shipping figures of the closed week
/// and the week in progress. Outside the hashed world, so a save without it loads with empty
/// figures and no hash moves.
const FREIGHT_MAGIC: &[u8; 4] = b"FRTS";
/// Marks the history archive trailer, after the freight trailer: long price closes and the
/// weekly and quarterly statements. Outside the hashed world, so a save without it loads with
/// an empty archive and no hash moves.
const ARCHIVE_MAGIC: &[u8; 4] = b"HIST";
/// Marks the systems history trailer, last: the weekly market, region, division, freight and
/// economy figures. Outside the hashed world; a save without it loads with empty history.
const SYSTEMS_MAGIC: &[u8; 4] = b"SYST";
/// Marks the chapters trailer, last: the board mandates on offer and in force, the player's
/// chapter log and the achievements earned (1.9). Unlike HIST and SYST it is future-affecting, so
/// `canonical_hash` folds it in; it stays outside the postcard payload so the save schema does not
/// move. A save without it loads with no chapter state and opens the current chapter fresh.
const CHAPTERS_MAGIC: &[u8; 4] = b"CHAP";
/// Marks the earnings trailer (1.9): analyst estimates, guidance, credibility and the results log.
/// Future-affecting, so `canonical_hash` folds it in; outside the postcard payload, so the save
/// schema does not move. A save without it loads with it empty and the first estimates are
/// written at the next quarter's close. Read in any order with the chapters and letters trailers.
const EARNINGS_MAGIC: &[u8; 4] = b"EARN";
/// Marks the read-letters trailer, after the chapters trailer: which letters are opened. Outside
/// the hashed world; a save without it loads with every letter unread. The reader accepts the
/// chapters and letters trailers in either order.
const MAIL_MAGIC: &[u8; 4] = b"MAIL";

pub fn encode(world: &World, career_id: u64) -> Result<Vec<u8>, DomainError> {
    encode_with(world, career_id, None)
}

/// Encode with the custom scenario the career started from, so a reload applies its rivals and
/// regional levels again. The trailer follows the payload; older readers stop at the payload.
pub fn encode_with(
    world: &World,
    career_id: u64,
    custom: Option<&cw_data::CustomScenario>,
) -> Result<Vec<u8>, DomainError> {
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
    if let Some(custom) = custom {
        let json = serde_json::to_vec(custom).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        out.extend_from_slice(CUSTOM_MAGIC);
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &json));
        out.extend_from_slice(&json);
    }
    if world.fx.active {
        let bytes = postcard::to_allocvec(&world.fx).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        out.extend_from_slice(FX_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    let freight = (&world.logistics.dest_last, &world.logistics.dest_week);
    if !freight.0.is_empty() || !freight.1.is_empty() {
        let bytes = postcard::to_allocvec(&freight).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        out.extend_from_slice(FREIGHT_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    if !world.archive.is_empty() {
        let bytes =
            postcard::to_allocvec(&world.archive).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
        if out.len() + bytes.len() > MAX_BYTES {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Oversize,
            });
        }
        out.extend_from_slice(ARCHIVE_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    if !world.systems.is_empty() {
        let bytes =
            postcard::to_allocvec(&world.systems).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
        if out.len() + bytes.len() > MAX_BYTES {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Oversize,
            });
        }
        out.extend_from_slice(SYSTEMS_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    if !world.chapters.is_empty() {
        let bytes =
            postcard::to_allocvec(&world.chapters).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
        if out.len() + bytes.len() > MAX_BYTES {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Oversize,
            });
        }
        out.extend_from_slice(CHAPTERS_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    if !world.earnings.is_empty() {
        let bytes =
            postcard::to_allocvec(&world.earnings).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
        if out.len() + bytes.len() > MAX_BYTES {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Oversize,
            });
        }
        out.extend_from_slice(EARNINGS_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    if !world.mail.is_empty() {
        let bytes = postcard::to_allocvec(&world.mail).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        if out.len() + bytes.len() > MAX_BYTES {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Oversize,
            });
        }
        out.extend_from_slice(MAIL_MAGIC);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&digest(DOMAIN_SAVE, &bytes));
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

pub fn decode(bytes: &[u8]) -> Result<(World, u64), DomainError> {
    decode_with(bytes).map(|(world, career, _)| (world, career))
}

pub fn decode_with(
    bytes: &[u8],
) -> Result<(World, u64, Option<cw_data::CustomScenario>), DomainError> {
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
    if !(MIN_SCHEMA_VERSION..=SCHEMA_VERSION).contains(&schema) {
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
    let mut world: World = cw_sim::legacy::read_layout(schema, || postcard::from_bytes(payload))
        .map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
    // Not saved: freight rates follow the price index, refreshed each week; until the first week
    // after a load, quotes must already read the saved economy's level, not the base rate.
    world.logistics.price_level_bps = world.macro_state.price_index_bps.max(1);
    let mut rest = &bytes[cursor + declared..];
    let mut custom_end = 0usize;
    let custom = if rest.len() >= 40 && &rest[..4] == CUSTOM_MAGIC {
        let length = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
        let mut sum = [0u8; 32];
        sum.copy_from_slice(&rest[8..40]);
        let json = rest.get(40..40 + length).ok_or(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        })?;
        if digest(DOMAIN_SAVE, json) != sum {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Checksum,
            });
        }
        custom_end = 40 + length;
        Some(
            serde_json::from_slice(json).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?,
        )
    } else {
        None
    };
    rest = &rest[custom_end..];
    if rest.len() >= 40 && &rest[..4] == FX_MAGIC {
        let length = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
        let mut sum = [0u8; 32];
        sum.copy_from_slice(&rest[8..40]);
        let body = rest.get(40..40 + length).ok_or(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        })?;
        if digest(DOMAIN_SAVE, body) != sum {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Checksum,
            });
        }
        world.fx = postcard::from_bytes(body).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        rest = &rest[40 + length..];
    }
    if rest.len() >= 4 && &rest[..4] == FREIGHT_MAGIC {
        if rest.len() < 40 {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Truncated,
            });
        }
        let length = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
        let mut sum = [0u8; 32];
        sum.copy_from_slice(&rest[8..40]);
        let body = rest.get(40..40 + length).ok_or(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        })?;
        if digest(DOMAIN_SAVE, body) != sum {
            return Err(DomainError::InvalidSave {
                detail: SaveFault::Checksum,
            });
        }
        (world.logistics.dest_last, world.logistics.dest_week) = postcard::from_bytes(body)
            .map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
        rest = &rest[40 + length..];
    }
    if rest.len() >= 4 && &rest[..4] == ARCHIVE_MAGIC {
        let body = trailer_body(rest)?;
        world.archive = postcard::from_bytes(body).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        rest = &rest[40 + body.len()..];
    }
    if rest.len() >= 4 && &rest[..4] == SYSTEMS_MAGIC {
        let body = trailer_body(rest)?;
        world.systems = postcard::from_bytes(body).map_err(|_| DomainError::InvalidSave {
            detail: SaveFault::MalformedPayload,
        })?;
        rest = &rest[40 + body.len()..];
    }
    // Chapters and letters trailers, in either order (1.9 previews wrote them in both orders).
    while rest.len() >= 4 {
        let magic = &rest[..4];
        if magic == CHAPTERS_MAGIC {
            let body = trailer_body(rest)?;
            world.chapters = postcard::from_bytes(body).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
            rest = &rest[40 + body.len()..];
        } else if magic == EARNINGS_MAGIC {
            let body = trailer_body(rest)?;
            world.earnings = postcard::from_bytes(body).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
            rest = &rest[40 + body.len()..];
        } else if magic == MAIL_MAGIC {
            let body = trailer_body(rest)?;
            world.mail = postcard::from_bytes(body).map_err(|_| DomainError::InvalidSave {
                detail: SaveFault::MalformedPayload,
            })?;
            rest = &rest[40 + body.len()..];
        } else {
            break;
        }
    }
    Ok((world, career_id, custom))
}

/// The checked body of a trailer: magic, length, BLAKE3 sum, body.
fn trailer_body(rest: &[u8]) -> Result<&[u8], DomainError> {
    if rest.len() < 40 {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Truncated,
        });
    }
    let length = u32::from_le_bytes(rest[4..8].try_into().unwrap()) as usize;
    let mut sum = [0u8; 32];
    sum.copy_from_slice(&rest[8..40]);
    let body = rest.get(40..40 + length).ok_or(DomainError::InvalidSave {
        detail: SaveFault::Truncated,
    })?;
    if digest(DOMAIN_SAVE, body) != sum {
        return Err(DomainError::InvalidSave {
            detail: SaveFault::Checksum,
        });
    }
    Ok(body)
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
    fn chapters_and_letters_trailers_load_together_in_either_order() {
        let dataset = Dataset::load().unwrap();
        let mut world = world();
        for _ in 0..10 {
            world.pending_pause = None;
            cw_sim::tick::advance(&mut world, &dataset);
        }
        assert!(
            !world.chapters.is_empty(),
            "a running career has chapter state"
        );
        world.mail.mark("letter-a", 5, 10);
        let hash = cw_sim::state::canonical_hash(&world);
        let bytes = encode(&world, 9).unwrap();
        let (restored, _) = decode(&bytes).unwrap();
        assert_eq!(restored.chapters, world.chapters);
        assert!(restored.mail.is_read("letter-a"));
        assert_eq!(cw_sim::state::canonical_hash(&restored), hash);
        // Swap the two trailers: CHAP then MAIL becomes MAIL then CHAP.
        let chap = bytes.windows(4).rposition(|w| w == CHAPTERS_MAGIC).unwrap();
        let mail = bytes.windows(4).rposition(|w| w == MAIL_MAGIC).unwrap();
        assert!(chap < mail);
        let mut swapped = bytes[..chap].to_vec();
        swapped.extend_from_slice(&bytes[mail..]);
        swapped.extend_from_slice(&bytes[chap..mail]);
        let (again, _) = decode(&swapped).unwrap();
        assert_eq!(again.chapters, world.chapters);
        assert!(again.mail.is_read("letter-a"));
        assert_eq!(cw_sim::state::canonical_hash(&again), hash);
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
    fn the_freight_read_out_survives_a_save_without_moving_the_hash() {
        let mut world = world();
        let before = cw_sim::state::canonical_hash(&world);
        let key = (cw_core::ids::Handle::new(3, 1), 2u8);
        let row = cw_sim::economy::logistics::DestFreight {
            cents: 12_345,
            lots: 7,
            ..Default::default()
        };
        world.logistics.dest_last.insert(key, row.clone());
        world.logistics.dest_week.insert(key, row.clone());
        assert_eq!(cw_sim::state::canonical_hash(&world), before);
        let bytes = encode(&world, 9).unwrap();
        let (restored, _) = decode(&bytes).unwrap();
        assert_eq!(restored.logistics.dest_last.get(&key), Some(&row));
        assert_eq!(restored.logistics.dest_week.get(&key), Some(&row));
        assert_eq!(cw_sim::state::canonical_hash(&restored), before);
        // A save from before the trailer existed still loads, with empty figures.
        for cut in 1..=8 {
            assert!(decode(&bytes[..bytes.len() - cut]).is_err(), "cut {cut}");
        }
        let plain = encode(&self::world(), 9).unwrap();
        assert!(decode(&plain).unwrap().0.logistics.dest_last.is_empty());
    }

    #[test]
    fn the_history_archive_survives_a_save_without_moving_the_hash() {
        let dataset = Dataset::load().unwrap();
        let mut world = world();
        for _ in 0..20 {
            world.pending_pause = None;
            cw_sim::tick::advance(&mut world, &dataset);
        }
        assert!(!world.archive.is_empty());
        let hash = cw_sim::state::canonical_hash(&world);
        let mut bare = world.clone();
        bare.archive = Default::default();
        assert_eq!(
            cw_sim::state::canonical_hash(&bare),
            hash,
            "the archive is not hashed"
        );
        let bytes = encode(&world, 4).unwrap();
        let (restored, _) = decode(&bytes).unwrap();
        assert_eq!(restored.archive, world.archive);
        assert_eq!(cw_sim::state::canonical_hash(&restored), hash);
        for cut in 1..=8 {
            assert!(decode(&bytes[..bytes.len() - cut]).is_err(), "cut {cut}");
        }
        // A save written before the archive existed loads with it empty.
        let old = encode(&bare, 4).unwrap();
        assert!(decode(&old).unwrap().0.archive.is_empty());
    }

    #[test]
    fn read_letters_survive_a_save_without_moving_the_hash() {
        let dataset = Dataset::load().unwrap();
        let mut world = world();
        for _ in 0..6 {
            world.pending_pause = None;
            cw_sim::tick::advance(&mut world, &dataset);
        }
        let hash = cw_sim::state::canonical_hash(&world);
        world.mail.mark("raid:3:1:2", 3, 6);
        world.mail.mark("card:9", 5, 6);
        assert_eq!(
            cw_sim::state::canonical_hash(&world),
            hash,
            "read flags are not hashed"
        );
        let bytes = encode(&world, 4).unwrap();
        let (restored, _) = decode(&bytes).unwrap();
        assert_eq!(restored.mail, world.mail);
        assert_eq!(restored.systems, world.systems);
        assert_eq!(cw_sim::state::canonical_hash(&restored), hash);
        for cut in 1..=8 {
            assert!(decode(&bytes[..bytes.len() - cut]).is_err(), "cut {cut}");
        }
        // A save from before letters loads with every letter unread.
        world.mail = Default::default();
        let old = encode(&world, 4).unwrap();
        assert!(decode(&old).unwrap().0.mail.is_empty());
    }

    #[test]
    fn the_systems_history_survives_a_save_without_moving_the_hash() {
        let dataset = Dataset::load().unwrap();
        let mut world = world();
        for _ in 0..6 {
            world.pending_pause = None;
            cw_sim::tick::advance(&mut world, &dataset);
        }
        assert_eq!(world.systems.weeks.len(), 6);
        let hash = cw_sim::state::canonical_hash(&world);
        let mut bare = world.clone();
        bare.systems = Default::default();
        assert_eq!(cw_sim::state::canonical_hash(&bare), hash, "not hashed");
        let bytes = encode(&world, 4).unwrap();
        let (restored, _) = decode(&bytes).unwrap();
        assert_eq!(restored.systems, world.systems);
        assert_eq!(restored.archive, world.archive);
        assert_eq!(cw_sim::state::canonical_hash(&restored), hash);
        for cut in 1..=8 {
            assert!(decode(&bytes[..bytes.len() - cut]).is_err(), "cut {cut}");
        }
        // Without the archive the systems trailer still follows straight after the freight one.
        let mut no_archive = world.clone();
        no_archive.archive = Default::default();
        let (restored, _) = decode(&encode(&no_archive, 4).unwrap()).unwrap();
        assert_eq!(restored.systems, world.systems);
        // A save from before the systems history loads with it empty.
        let old = encode(&bare, 4).unwrap();
        assert!(decode(&old).unwrap().0.systems.is_empty());
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
