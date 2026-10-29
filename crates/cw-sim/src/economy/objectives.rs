//! The 1.2 quarterly board objectives, retired in 1.9.
//!
//! Chapters and board mandates (`chapters.rs`) replace them: one goal system, one chapter at a
//! time, offered by the board of every company rather than set for the player alone. The types
//! below remain only because `World.objective` and `World.objective_history` are part of the
//! postcard save layout and cannot be removed without a schema bump. They are never written any
//! more; `chapters::weekly` clears a goal that an older save still carries.

use cw_core::ids::Handle;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectiveKind {
    NewRegion {
        baseline: u32,
    },
    WeeklyEbit {
        target_cents: i64,
    },
    MarketShare {
        region: u8,
        industry: u8,
        target_bps: i64,
    },
    NewState {
        baseline: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    pub firm: Handle,
    pub set: u32,
    pub due: u32,
    pub kind: ObjectiveKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveResult {
    pub objective: Objective,
    pub met: bool,
    pub closed: u32,
    pub bonus_cents: i64,
}
