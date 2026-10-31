// CI demonstration only: this pull request is closed without merging.
//! Corporate Warfare engine core.
//!
//! Stable identities, checked fixed-point units, the double-entry ledger, deterministic
//! random streams and the canonical state hash. No economy, I/O or UI lives here.

pub mod calendar;
pub mod error;
pub mod hash;
pub mod ids;
pub mod ledger;
pub mod money;
pub mod quantity;
pub mod rng;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Public session-interface schema version. Bumped only with a migration.
pub const INTERFACE_VERSION: u32 = 1;
/// Rules revision. Part of every save and replay identity.
pub const RULES_VERSION: u32 = 1;
