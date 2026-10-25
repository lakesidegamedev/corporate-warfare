//! Corporate Warfare simulation: authoritative state, validated commands and weekly
//! economic transitions. No I/O, no AI and no UI.
//!
//! Settlement order is fixed by `docs/rules/MARKETS.md` and implemented in `tick`. Every
//! monetary change posts through the double-entry ledger; every physical quantity is
//! milli-lots; every random draw comes from a persisted named stream.

pub mod commands;
pub mod economy;
pub mod fixtures;
pub mod state;
pub mod tick;

pub use commands::{CommandEnvelope, CommandIntent, CommandReceipt, Preview};
pub use state::World;
