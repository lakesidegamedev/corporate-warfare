//! Disclosed stakes and the "likely next move" line for the apps (1.9). The engine keeps the
//! plans (`cw_sim::economy::plans`); this shows only what a filing or the public record shows:
//! who holds 5 percent or more of a company, the weekly history since the filing, and one line
//! on what a rival is likely to do next, from those signals and never from hidden intent.

use cw_core::ids::Handle;
use cw_sim::state::World;

/// One point of a stake over time.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct StakePointDto {
    pub week: u32,
    pub bps: i32,
}

/// A holding of 5 percent or more in a company, as filed.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct StakeDto {
    pub holder: u64,
    pub holder_name: String,
    pub bps: i32,
    /// The week the holding crossed the filing line, when it did so while this game was played.
    pub since: Option<u32>,
    /// A rival building the holding toward a bid, as far as the record shows; false for an
    /// ordinary holding such as a parent's.
    pub campaign: bool,
    pub history: Vec<StakePointDto>,
}

pub fn stakes(world: &World, target: Handle) -> Vec<StakeDto> {
    cw_sim::economy::plans::disclosed_stakes(world, target)
        .into_iter()
        .map(|s| StakeDto {
            holder: s.holder.0,
            holder_name: world
                .firms
                .get(s.holder)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| "a former rival".to_owned()),
            bps: s.bps,
            since: s.since,
            campaign: s.campaign,
            history: s
                .history
                .into_iter()
                .map(|(week, bps)| StakePointDto { week, bps })
                .collect(),
        })
        .collect()
}

pub fn likely_next(world: &World, rival: Handle) -> Option<String> {
    cw_sim::economy::plans::likely_next_move(world, rival)
}
