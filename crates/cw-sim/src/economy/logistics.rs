//! Owned logistics: freight modes, lanes with finite weekly capacity, lane contracts and routing.
//!
//! The generic `freight` product is lane capacity. A carrier is a logistics division whose recipe
//! carries a freight mode (truck, rail, pipeline, sea); the lots it makes each week are the lots
//! it can move that week, spread over the lanes it serves. A lane is a pair of regions in one
//! mode. Every shipment is routed over one to three legs and booked leg by leg:
//!
//! 1. Lots under a lane contract the shipper's group holds move first, already paid for.
//! 2. Lots on carriers the shipper's group owns come next, at operating cost.
//! 3. Lots on other firms' carriers are priced by how full the lane is: from the empty-lane rate
//!    up to the outside price as the lane fills, a smooth square curve.
//! 4. Anything beyond capacity goes to the outside carriers at a premium, so goods never stop.
//!    Pipelines have no outside carriers: a shipment either fits or takes another mode.
//!
//! A shipment is charged once. The shipper's cash pays exactly the sum of what carriers and the
//! outside sector receive. Capacity, use and contracts are saved with the world; everything else
//! here (route candidates, rate estimates) is a pure function of saved state and is rebuilt on
//! demand, so a saved game and a continued one never diverge.
//!
//! A standing shipping policy on each division picks among the candidate routes: cheapest,
//! fastest, own carriers first or most reliable, with an optional preferred mode or carrier. The
//! policy governs the freight a division pays for, that is goods arriving in its region.

use std::collections::{BTreeMap, BTreeSet};

use cw_core::error::{ArithmeticFault, DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, allocate_cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::{Dataset, StorageClass};
use serde::{Deserialize, Serialize};

use crate::state::{FacilityStatus, World};

pub const MAX_LEGS: usize = 3;
/// A carrier's weekly capacity is lots over this reference haul. On a shorter lane the same fleet
/// moves more lots, on a longer one fewer: capacity is really lot-kilometres.
pub const REF_KM: i64 = 400;
/// Lane contracts run between a month and a year.
pub const MIN_CONTRACT_WEEKS: u32 = 4;
pub const MAX_CONTRACT_WEEKS: u32 = 52;
/// Most lanes one carrier serves, by mode order (truck, rail, pipeline, sea).
pub const MAX_SERVED: [usize; 4] = [10, 8, 1, 6];
/// A lane at or above this fill is congested; four weeks running is sustained.
pub const CONGESTED_BPS: i32 = 9_000;
pub const SUSTAINED_WEEKS: u16 = 4;
/// A lane price this much above four weeks ago is a carrier rate hike.
pub const HIKE_BPS: i32 = 2_000;
/// One contract holds at most this share of a lane's capacity.
pub const MAX_CONTRACT_SHARE_BPS: i64 = 8_000;
pub const MAX_CONTRACTS: usize = 400;
/// A detour of more than this share of the direct distance is not considered.
const DETOUR_BPS: i64 = 15_000;
/// A week of dwell at every change of mode.
const TRANSFER_WEEKS: u32 = 1;

/// A lane: two regions (low index first) in one mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LaneKey {
    pub a: u8,
    pub b: u8,
    pub mode: u8,
}

impl LaneKey {
    pub fn new(x: u8, y: u8, mode: u8) -> LaneKey {
        LaneKey {
            a: x.min(y),
            b: x.max(y),
            mode,
        }
    }
}

/// How a division picks among routes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShipKind {
    #[default]
    Cheapest,
    Fastest,
    OwnFirst,
    Reliable,
}

impl ShipKind {
    pub fn from_code(code: u8) -> Option<ShipKind> {
        match code {
            0 => Some(ShipKind::Cheapest),
            1 => Some(ShipKind::Fastest),
            2 => Some(ShipKind::OwnFirst),
            3 => Some(ShipKind::Reliable),
            _ => None,
        }
    }

    pub fn code(self) -> u8 {
        match self {
            ShipKind::Cheapest => 0,
            ShipKind::Fastest => 1,
            ShipKind::OwnFirst => 2,
            ShipKind::Reliable => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ShipKind::Cheapest => "Cheapest",
            ShipKind::Fastest => "Fastest",
            ShipKind::OwnFirst => "Own carriers first",
            ShipKind::Reliable => "Most reliable",
        }
    }
}

/// A division's standing shipping policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipPolicy {
    pub kind: ShipKind,
    pub mode: Option<u8>,
    pub carrier: Option<Handle>,
}

/// A firm's carrying division and the lanes its capacity was spread over this week.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Carrier {
    pub firm: Handle,
    pub facility: Handle,
    pub region: u8,
    pub mode: u8,
    /// Lots the carrier can move this week over the reference haul.
    pub cap: i32,
    /// Lot-kilometres it moved this week.
    pub used_lot_km: i64,
}

/// One carrier's share of one lane this week.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alloc {
    pub carrier: u16,
    pub cap: i32,
    pub used: i32,
    /// Lots under contract, which only the holder may use.
    pub held: i32,
}

/// One lane's week: capacity, use and who shipped.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneWeek {
    pub allocs: Vec<Alloc>,
    /// Lots moved under lane contracts.
    pub contract_used: i32,
    /// Lots that overflowed to outside carriers.
    pub outside: i32,
    /// Lots moved by firm this week.
    pub users: Vec<(Handle, i32)>,
}

impl LaneWeek {
    pub fn cap(&self) -> i32 {
        self.allocs.iter().map(|a| a.cap).sum()
    }

    pub fn held(&self) -> i32 {
        self.allocs.iter().map(|a| a.held).sum()
    }

    pub fn used(&self) -> i32 {
        self.allocs.iter().map(|a| a.used).sum()
    }

    /// Committed share of capacity in basis points: used plus held.
    pub fn fill_bps(&self) -> i32 {
        let cap = self.cap();
        if cap <= 0 {
            return 0;
        }
        ((self.used() + self.held()) as i64 * 10_000 / cap as i64).clamp(0, 10_000) as i32
    }

    /// Lots moved through the lane this week by anyone.
    pub fn moved(&self) -> i32 {
        self.used() + self.contract_used + self.outside
    }
}

/// A lane contract: capacity reserved for a holder at a set weekly rate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneContract {
    pub id: u32,
    pub holder: Handle,
    pub key: LaneKey,
    /// Lots asked for, and the lots actually held this week.
    pub lots: i32,
    pub held: i32,
    pub used: i32,
    pub fee_per_lot_cents: i64,
    pub start: u32,
    pub weeks_left: u32,
}

/// Rolling facts about a lane, for alerts, rivals and telemetry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneStat {
    pub fill_bps: i32,
    pub hot_weeks: u16,
    /// Spot price of the lane as basis points of the base rate, at close.
    pub mult_bps: i32,
    /// The last eight closes, oldest first.
    pub mult_hist: Vec<i32>,
    pub cap: i32,
    pub moved: i32,
    pub outside: i32,
    /// Firms that shipped on the lane and the tick they last did.
    pub users: Vec<(Handle, u32)>,
}

/// Freight moved in a week, for telemetry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreightTotals {
    pub lots_by_mode: [i64; 4],
    pub outside_lots_by_mode: [i64; 4],
    pub own_lots: i64,
    pub contract_lots: i64,
    pub cents: i64,
    pub outside_cents: i64,
    pub shipments: i64,
    /// Lot-kilometres moved, so the average rate can be read back.
    pub lot_km: i64,
}

/// A rival or player reserving a lane, kept so affected firms can be told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reservation {
    pub tick: u32,
    pub key: LaneKey,
    pub holder: Handle,
    pub lots: i32,
}

/// Region value of the row that holds a firm's lane contract fees, which belong to no destination.
pub const FEE_ROW: u8 = u8::MAX;

/// One lane's part of a destination's freight week.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneUse {
    pub key: LaneKey,
    pub lots: i64,
    pub cents: i64,
    pub outside_lots: i64,
    pub outside_cents: i64,
}

/// A shipment pattern: lots of one good from one region, kept so a policy can be priced before it
/// is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub from: u8,
    pub product: u16,
    pub lots: i64,
}

/// What a firm paid to bring goods into one region in a week, read back for the Shipping panels.
/// Not part of the hashed world state. The save file carries it in its own trailer so the panels
/// are filled after a load, and a save without the trailer simply starts empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestFreight {
    pub cents: i64,
    pub own_cents: i64,
    pub other_cents: i64,
    pub outside_cents: i64,
    /// Lane contract fees (on the firm's fee row only).
    pub fee_cents: i64,
    pub lots: i64,
    pub own_lots: i64,
    pub other_lots: i64,
    pub outside_lots: i64,
    pub mode_lots: [i64; 4],
    pub lanes: Vec<LaneUse>,
    pub samples: Vec<Sample>,
}

impl DestFreight {
    fn add_lane(&mut self, add: LaneUse) {
        match self.lanes.iter_mut().find(|l| l.key == add.key) {
            Some(l) => {
                l.lots += add.lots;
                l.cents += add.cents;
                l.outside_lots += add.outside_lots;
                l.outside_cents += add.outside_cents;
            }
            None => self.lanes.push(add),
        }
        if self.lanes.len() > 24 {
            self.lanes.sort_by_key(|l| std::cmp::Reverse(l.cents));
            self.lanes.truncate(16);
        }
    }

    fn add_sample(&mut self, add: Sample) {
        match self
            .samples
            .iter_mut()
            .find(|s| s.from == add.from && s.product == add.product)
        {
            Some(s) => s.lots += add.lots,
            None => self.samples.push(add),
        }
        if self.samples.len() > 24 {
            self.samples.sort_by_key(|s| std::cmp::Reverse(s.lots));
            self.samples.truncate(16);
        }
    }
}

/// A disruption to lanes: capacity on matching lanes is cut to `cap_bps` of normal until the
/// week `until`. A closure with `a == b` covers every lane of the mode that touches the region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Closure {
    pub a: u8,
    pub b: u8,
    pub mode: u8,
    pub cap_bps: i32,
    pub until: u32,
}

impl Closure {
    pub fn covers(&self, key: LaneKey) -> bool {
        if key.mode != self.mode {
            return false;
        }
        if self.a == self.b {
            key.a == self.a || key.b == self.a
        } else {
            key.a == self.a.min(self.b) && key.b == self.a.max(self.b)
        }
    }
}

/// Longest share of a lane's capacity a contract may hold under common carrier law.
pub const COMMON_CARRIER_CONTRACT_BPS: i64 = 4_000;
/// Share of the gap between the empty-lane rate and the outside price that a common carrier may
/// charge at most.
pub const COMMON_CARRIER_RATE_PCT: i64 = 40;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LogisticsState {
    /// Common carrier law was in force when the week's capacity was built.
    #[serde(default)]
    pub common_carrier: bool,
    /// Pipeline spills, port strikes and lane outages in force.
    #[serde(default)]
    pub closures: Vec<Closure>,
    /// The tick the week's capacity was built for.
    pub week: u32,
    pub carriers: Vec<Carrier>,
    pub lanes: BTreeMap<LaneKey, LaneWeek>,
    /// Terminal or port access by mode and region.
    pub access: Vec<Vec<bool>>,
    pub contracts: Vec<LaneContract>,
    pub next_contract: u32,
    pub stats: BTreeMap<LaneKey, LaneStat>,
    /// Smoothed lots asked of each lane and mode, outside overflow included, which steers where
    /// carriers put their capacity.
    pub flows: BTreeMap<LaneKey, i32>,
    pub flow_week: BTreeMap<LaneKey, i32>,
    pub totals: FreightTotals,
    pub last: FreightTotals,
    pub reservations: Vec<Reservation>,
    /// Route candidates and rate estimates: pure functions of the saved state above.
    #[serde(skip)]
    cache: BTreeMap<(u8, u8, u8), Vec<Structure>>,
    #[serde(skip)]
    estimates: Vec<i32>,
    /// Freight paid by firm and destination region: the week in progress and the week just closed.
    #[serde(skip)]
    pub dest_week: BTreeMap<(Handle, u8), DestFreight>,
    #[serde(skip)]
    pub dest_last: BTreeMap<(Handle, u8), DestFreight>,
}

/// One route shape: legs by mode and hub, ranked at empty-lane prices.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Structure {
    legs: Vec<(u8, u8, u8)>,
    min_lots: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Contract(u16),
    Own(u16),
    Public(u16),
    Outside,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub kind: PartKind,
    pub lots: i64,
    pub cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegPlan {
    pub from: u8,
    pub to: u8,
    pub mode: u8,
    pub weeks: u32,
    pub cents: i64,
    pub parts: Vec<Part>,
    pub fill_bps: i32,
}

/// A routed shipment: legs, price and transit time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutePlan {
    pub legs: Vec<LegPlan>,
    pub cents: i64,
    pub weeks: u32,
    pub lots: i64,
}

impl RoutePlan {
    pub fn outside_lots(&self) -> i64 {
        self.legs
            .iter()
            .flat_map(|l| l.parts.iter())
            .filter(|p| p.kind == PartKind::Outside)
            .map(|p| p.lots)
            .sum()
    }

    pub fn modes(&self) -> Vec<u8> {
        self.legs.iter().map(|l| l.mode).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// Prices

/// Basis points of the base rate the lane's owners charge at a given fill: a smooth square curve
/// from the empty-lane rate to the outside price, which a full lane meets.
pub fn spot_mult_bps(ds: &Dataset, fill_bps: i32) -> i64 {
    let l = &ds.logistics;
    let f = fill_bps.clamp(0, 10_000) as i64;
    l.empty_lane_bps + (l.outside_premium_bps - l.empty_lane_bps) * f * f / 100_000_000
}

/// The most a common carrier may charge, in basis points of the base rate.
pub fn regulated_cap_bps(ds: &Dataset) -> i64 {
    let l = &ds.logistics;
    l.empty_lane_bps + (l.outside_premium_bps - l.empty_lane_bps) * COMMON_CARRIER_RATE_PCT / 100
}

/// The share of a lane's capacity contracts may hold.
fn contract_share_bps(st: &LogisticsState) -> i64 {
    if st.common_carrier {
        COMMON_CARRIER_CONTRACT_BPS
    } else {
        MAX_CONTRACT_SHARE_BPS
    }
}

fn leg_km(ds: &Dataset, a: u8, b: u8, mode: u8) -> u32 {
    if ds.logistics.modes[mode as usize].id == "sea" {
        ds.sea_km(a, b).unwrap_or(0)
    } else {
        ds.km(a, b)
    }
}

/// Cents to move `lots` over a leg at a price level, in basis points of the base rate.
pub fn leg_cents(ds: &Dataset, a: u8, b: u8, mode: u8, lots: i64, mult_bps: i64) -> i64 {
    let def = &ds.logistics.modes[mode as usize];
    let km = leg_km(ds, a, b, mode) as i128;
    let road = if def.road_sensitive {
        crate::economy::operations::road_bps(ds, a, b) as i128
    } else {
        10_000
    };
    round_even_div(
        lots as i128 * km * def.rate_milli_cents_per_lot_km as i128 * road * mult_bps as i128,
        1_000 * 10_000 * 10_000,
    ) as i64
}

fn leg_weeks(ds: &Dataset, a: u8, b: u8, mode: u8) -> u32 {
    let def = &ds.logistics.modes[mode as usize];
    (leg_km(ds, a, b, mode).div_ceil(def.speed_km_week)).max(1)
}

/// Basis points of the base rate for a lane's spot price now.
pub fn lane_mult_bps(ds: &Dataset, st: &LogisticsState, key: LaneKey) -> i64 {
    match st.lanes.get(&key) {
        Some(lane) if lane.cap() > 0 => {
            let mult = spot_mult_bps(ds, lane.fill_bps());
            if st.common_carrier {
                mult.min(regulated_cap_bps(ds))
            } else {
                mult
            }
        }
        _ => ds.logistics.outside_premium_bps,
    }
}

/// The spot price of one lot over a lane now, in cents.
pub fn lane_rate_cents_per_lot(ds: &Dataset, st: &LogisticsState, key: LaneKey) -> i64 {
    leg_cents(ds, key.a, key.b, key.mode, 1, lane_mult_bps(ds, st, key)).max(1)
}

// ---------------------------------------------------------------------------------------------
// Access, availability

pub fn has_access(ds: &Dataset, st: &LogisticsState, mode: u8, region: u8) -> bool {
    let def = &ds.logistics.modes[mode as usize];
    if !def.needs_terminal {
        return true;
    }
    st.access
        .get(mode as usize)
        .and_then(|row| row.get(region as usize))
        .copied()
        .unwrap_or_else(|| public_access(ds, &def.id, region))
}

fn public_access(ds: &Dataset, mode_id: &str, region: u8) -> bool {
    match mode_id {
        "rail" => ds
            .public_rail
            .get(region as usize)
            .copied()
            .unwrap_or(false),
        "sea" => ds
            .public_ports
            .get(region as usize)
            .copied()
            .unwrap_or(false),
        _ => false,
    }
}

/// Whether a lane can exist in a mode at all: terminals at both ends, a sea route between ports.
pub fn lane_possible(ds: &Dataset, st: &LogisticsState, mode: u8, a: u8, b: u8) -> bool {
    if a == b {
        return false;
    }
    let def = &ds.logistics.modes[mode as usize];
    if def.id == "sea" && ds.sea_km(a, b).is_none() {
        return false;
    }
    !def.needs_terminal || (has_access(ds, st, mode, a) && has_access(ds, st, mode, b))
}

fn build_access(world: &World, ds: &Dataset) -> Vec<Vec<bool>> {
    let n = ds.regions.len();
    let mut access: Vec<Vec<bool>> = ds
        .logistics
        .modes
        .iter()
        .map(|def| {
            (0..n)
                .map(|r| !def.needs_terminal || public_access(ds, &def.id, r as u8))
                .collect()
        })
        .collect();
    for (_, plant) in world.facilities.iter() {
        if plant.status != FacilityStatus::Operating {
            continue;
        }
        let recipe = &ds.recipes[plant.recipe as usize];
        if !recipe.terminal {
            continue;
        }
        if let Some(mode) = recipe
            .transport_mode
            .as_deref()
            .and_then(|id| ds.logistics.mode_index(id))
        {
            access[mode as usize][plant.region as usize] = true;
        }
    }
    access
}

// ---------------------------------------------------------------------------------------------
// The week

fn plant_capacity(world: &World, ds: &Dataset, plant: &crate::state::Facility) -> i32 {
    let last = plant.last_output.lots();
    if last > 0 {
        return last.min(i32::MAX as i64) as i32;
    }
    // A plant that has not run yet offers what it is set to make, for its first week only.
    if world.tick.0 <= plant.completes.0 + 1 {
        let full = plant.full_weekly_output(ds).lots();
        return (full * plant.target_bps.clamp(0, 10_000) as i64 / 10_000).min(i32::MAX as i64)
            as i32;
    }
    0
}

/// Start of week: close last week's lane statistics, rebuild capacity from the carriers that are
/// running, apply lane contracts and charge their weekly fee. Runs before any shipment settles.
pub fn begin_week(world: &mut World, ds: &Dataset) {
    close_stats(world, ds);
    let access = build_access(world, ds);
    world.logistics.access = access;
    world.logistics.cache.clear();
    world.logistics.estimates.clear();
    world.logistics.week = world.tick.0;
    world.logistics.last = std::mem::take(&mut world.logistics.totals);
    world.logistics.dest_last = std::mem::take(&mut world.logistics.dest_week);
    world.logistics.common_carrier = world.politics.laws.common_carrier;
    let now = world.tick.0;
    world.logistics.closures.retain(|c| c.until > now);

    // Carriers, in plant order.
    let mut carriers: Vec<Carrier> = Vec::new();
    let mut plants: Vec<crate::state::Facility> = Vec::new();
    let mut handles: Vec<Handle> = Vec::new();
    for (handle, plant) in world.facilities.iter() {
        let recipe = &ds.recipes[plant.recipe as usize];
        let Some(mode) = recipe
            .transport_mode
            .as_deref()
            .and_then(|id| ds.logistics.mode_index(id))
        else {
            continue;
        };
        if plant.status != FacilityStatus::Operating
            || !world.firms.get(plant.firm).is_some_and(|f| f.operative())
        {
            continue;
        }
        let cap = plant_capacity(world, ds, plant);
        carriers.push(Carrier {
            firm: plant.firm,
            facility: handle,
            region: plant.region,
            mode,
            cap,
            used_lot_km: 0,
        });
        plants.push(plant.clone());
        handles.push(handle);
    }

    let mut lanes: BTreeMap<LaneKey, LaneWeek> = BTreeMap::new();
    for (index, carrier) in carriers.iter().enumerate() {
        if carrier.cap <= 0 {
            continue;
        }
        let served = served_lanes(world, ds, carrier, &plants[index]);
        let shares = split_capacity(world, ds, carrier, &served);
        for ((x, y), cap) in shares {
            if cap <= 0 {
                continue;
            }
            lanes
                .entry(LaneKey::new(x, y, carrier.mode))
                .or_default()
                .allocs
                .push(Alloc {
                    carrier: index as u16,
                    cap,
                    used: 0,
                    held: 0,
                });
        }
    }
    // Spills, strikes and outages cut the capacity of the lanes they cover.
    for (key, lane) in lanes.iter_mut() {
        let mut keep = 10_000i64;
        for closure in &world.logistics.closures {
            if closure.covers(*key) {
                keep = keep.min(closure.cap_bps.clamp(0, 10_000) as i64);
            }
        }
        if keep < 10_000 {
            for alloc in &mut lane.allocs {
                alloc.cap = (alloc.cap as i64 * keep / 10_000) as i32;
            }
        }
    }
    world.logistics.carriers = carriers;
    world.logistics.lanes = lanes;
    apply_contracts(world, ds);
}

/// The lanes a carrier's capacity is spread over, as region pairs. A carrier serves the lanes
/// with an end within its mode's reach of home, favouring those that carry the most; its owner may
/// instead pin lanes that start at home.
fn served_lanes(
    world: &World,
    ds: &Dataset,
    carrier: &Carrier,
    plant: &crate::state::Facility,
) -> Vec<(u8, u8)> {
    let st = &world.logistics;
    let n = ds.regions.len() as u8;
    let def = &ds.logistics.modes[carrier.mode as usize];
    let home = carrier.region;
    // A rail or sea carrier whose own region has no terminal has nowhere to load.
    if def.needs_terminal && !has_access(ds, st, carrier.mode, home) {
        return Vec::new();
    }
    let pinned: Vec<(u8, u8)> = plant
        .serves
        .iter()
        .copied()
        .filter(|x| *x < n && lane_possible(ds, st, carrier.mode, home, *x))
        .map(|x| (home, x))
        .collect();
    if !pinned.is_empty() {
        return pinned;
    }
    let reach = def.reach_km;
    let near = |r: u8| r == home || ds.km(home, r) <= reach;
    let mut candidates: Vec<(u8, u8)> = Vec::new();
    for x in 0..n {
        for y in (x + 1)..n {
            if (near(x) || near(y)) && lane_possible(ds, st, carrier.mode, x, y) {
                candidates.push((x, y));
            }
        }
    }
    let weight = |l: &(u8, u8)| lane_weight(ds, st, carrier.mode, l.0, l.1);
    let away = |l: &(u8, u8)| ds.km(home, l.0).min(ds.km(home, l.1));
    candidates.sort_by_key(|l| (std::cmp::Reverse(weight(l)), away(l), ds.km(l.0, l.1), *l));
    candidates.truncate(MAX_SERVED.get(carrier.mode as usize).copied().unwrap_or(6));
    candidates
}

fn lane_weight(ds: &Dataset, st: &LogisticsState, mode: u8, a: u8, b: u8) -> i64 {
    let flow = |m: u8| {
        st.flows
            .get(&LaneKey::new(a, b, m))
            .copied()
            .unwrap_or(0)
            .max(0) as i64
    };
    // A mode with no outside carriers (a pipeline) follows the truck traffic it would relieve.
    let relief = if ds.logistics.modes[mode as usize].outside_backstop {
        0
    } else {
        flow(0)
    };
    1 + flow(mode) + relief
}

fn split_capacity(
    world: &World,
    ds: &Dataset,
    carrier: &Carrier,
    served: &[(u8, u8)],
) -> Vec<((u8, u8), i32)> {
    if served.is_empty() {
        return Vec::new();
    }
    let weights: Vec<(u64, i64)> = served
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (
                i as u64,
                lane_weight(ds, &world.logistics, carrier.mode, l.0, l.1),
            )
        })
        .collect();
    allocate_cents(Cents(carrier.cap as i64), &weights)
        .into_iter()
        .map(|(i, share)| {
            let lane = served[i as usize];
            let km = (leg_km(ds, lane.0, lane.1, carrier.mode) as i64).max(1);
            (
                lane,
                (share.0 * REF_KM / km).min(i32::MAX as i64 / 4) as i32,
            )
        })
        .collect()
}

/// Reserve held capacity for every live contract and charge the week's fee. A holder that cannot
/// pay loses the contract; an expired contract is dropped.
fn apply_contracts(world: &mut World, ds: &Dataset) {
    let mut contracts = std::mem::take(&mut world.logistics.contracts);
    let mut kept: Vec<LaneContract> = Vec::new();
    for mut contract in contracts.drain(..) {
        if contract.weeks_left == 0 || world.firms.get(contract.holder).is_none() {
            continue;
        }
        let holder = contract.holder;
        // Capacity that belongs to the holder's own group cannot be reserved against itself.
        let free: Vec<(usize, i32)> = world
            .logistics
            .lanes
            .get(&contract.key)
            .map(|lane| {
                lane.allocs
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| {
                        let firm = world.logistics.carriers[a.carrier as usize].firm;
                        !crate::economy::ownership::same_group(world, holder, firm)
                    })
                    .map(|(i, a)| (i, a.cap - a.held))
                    .filter(|(_, free)| *free > 0)
                    .collect()
            })
            .unwrap_or_default();
        let available: i32 = free.iter().map(|(_, f)| *f).sum();
        // Contracts together may hold only so much of a lane (less under common carrier law).
        let room = world
            .logistics
            .lanes
            .get(&contract.key)
            .map(|lane| {
                let limit = lane.cap() as i64 * contract_share_bps(&world.logistics) / 10_000;
                (limit - lane.held() as i64).max(0) as i32
            })
            .unwrap_or(0);
        let held = contract.lots.min(available).min(room);
        let fee = held as i64 * contract.fee_per_lot_cents;
        let cash = world
            .firms
            .get(holder)
            .map(|f| f.book.cash().max(0))
            .unwrap_or(0);
        if held <= 0 || fee > cash {
            continue;
        }
        // Spread the held lots over the lane's other carriers by what each has free.
        let weights: Vec<(u64, i64)> = free.iter().map(|(i, f)| (*i as u64, *f as i64)).collect();
        let split = allocate_cents(Cents(held as i64), &weights);
        let fee_split = allocate_cents(
            Cents(fee),
            &split.iter().map(|(i, c)| (*i, c.0)).collect::<Vec<_>>(),
        );
        if let Some(entity) = world.firms.get_mut(holder) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, fee, "lane contract")
                    .debit(Account::CostOfGoods, fee, "lane contract"),
            );
        }
        for ((index, lots), (_, cents)) in split.iter().zip(fee_split) {
            let carrier = {
                let lane = world.logistics.lanes.get_mut(&contract.key).unwrap();
                let alloc = &mut lane.allocs[*index as usize];
                alloc.held += lots.0 as i32;
                alloc.carrier
            };
            let km = leg_km(ds, contract.key.a, contract.key.b, contract.key.mode) as i64;
            credit_carrier(
                world,
                ds,
                carrier,
                cents.0,
                lots.0 * km,
                crate::economy::fx::currency_of(world, holder),
            );
        }
        world.logistics.totals.cents += fee;
        let row = world
            .logistics
            .dest_week
            .entry((holder, FEE_ROW))
            .or_default();
        row.cents += fee;
        row.fee_cents += fee;
        contract.held = held;
        contract.used = 0;
        contract.weeks_left -= 1;
        kept.push(contract);
    }
    // A contract at zero weeks has had its last paid week and is dropped at the next opening.
    world.logistics.contracts = kept;
}

/// Roll the closed week into lane statistics and the smoothed flows.
fn close_stats(world: &mut World, ds: &Dataset) {
    let tick = world.tick.0;
    let st = &mut world.logistics;
    let lanes = std::mem::take(&mut st.lanes);
    let mut seen: BTreeSet<LaneKey> = BTreeSet::new();
    for (key, lane) in &lanes {
        seen.insert(*key);
        let cap = lane.cap();
        let fill = lane.fill_bps();
        let mult = if cap > 0 {
            let raw = spot_mult_bps(ds, fill);
            (if st.common_carrier {
                raw.min(regulated_cap_bps(ds))
            } else {
                raw
            }) as i32
        } else {
            ds.logistics.outside_premium_bps as i32
        };
        let stat = st.stats.entry(*key).or_default();
        let congested = cap > 0 && (fill >= CONGESTED_BPS || lane.outside > 0);
        stat.hot_weeks = if congested { stat.hot_weeks + 1 } else { 0 };
        stat.fill_bps = fill;
        stat.mult_bps = mult;
        stat.mult_hist.push(mult);
        if stat.mult_hist.len() > 8 {
            stat.mult_hist.remove(0);
        }
        stat.cap = cap;
        stat.moved = lane.moved();
        stat.outside = lane.outside;
        for (firm, lots) in &lane.users {
            if *lots > 0 {
                match stat.users.iter_mut().find(|(f, _)| f == firm) {
                    Some(entry) => entry.1 = tick,
                    None => stat.users.push((*firm, tick)),
                }
            }
        }
        stat.users
            .retain(|(_, last)| tick.saturating_sub(*last) <= 13);
    }
    // Lanes that lost all capacity and use cool off, then drop out.
    st.stats.retain(|key, stat| {
        if seen.contains(key) {
            return true;
        }
        stat.hot_weeks = 0;
        stat.cap = 0;
        stat.moved = 0;
        stat.outside = 0;
        stat.fill_bps = 0;
        stat.users
            .retain(|(_, last)| tick.saturating_sub(*last) <= 13);
        !stat.users.is_empty()
    });
    // Flows: three quarters of the smoothed value plus this week's lots.
    let week = std::mem::take(&mut st.flow_week);
    let mut keys: BTreeSet<LaneKey> = st.flows.keys().copied().collect();
    keys.extend(week.keys().copied());
    for key in keys {
        let old = st.flows.get(&key).copied().unwrap_or(0);
        let add = week.get(&key).copied().unwrap_or(0);
        let next = old * 3 / 4 + add;
        if next == 0 {
            st.flows.remove(&key);
        } else {
            st.flows.insert(key, next);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Quoting a leg

struct Ctx {
    own_carrier: Vec<bool>,
    own_contract: Vec<bool>,
}

impl Ctx {
    fn new(world: &World, shipper: Handle) -> Ctx {
        let st = &world.logistics;
        Ctx {
            own_carrier: st
                .carriers
                .iter()
                .map(|c| crate::economy::ownership::same_group(world, shipper, c.firm))
                .collect(),
            own_contract: st
                .contracts
                .iter()
                .map(|c| crate::economy::ownership::same_group(world, shipper, c.holder))
                .collect(),
        }
    }
}

/// Price and fill a leg for `lots`. None when the mode cannot carry the good, the shipment is
/// below the mode's smallest lot, the lane cannot exist, or a pipeline has no room.
#[allow(clippy::too_many_arguments)]
fn quote_leg(
    ds: &Dataset,
    st: &LogisticsState,
    ctx: &Ctx,
    product: u16,
    from: u8,
    to: u8,
    mode: u8,
    lots: i64,
) -> Option<LegPlan> {
    let def = &ds.logistics.modes[mode as usize];
    if lots < def.min_lots
        || !ds.mode_carries(mode, product)
        || !lane_possible(ds, st, mode, from, to)
    {
        return None;
    }
    let key = LaneKey::new(from, to, mode);
    let lane = st.lanes.get(&key);
    let mut left = lots;
    let mut parts: Vec<Part> = Vec::new();

    // 1. Contracts held by the shipper's group.
    for (index, contract) in st.contracts.iter().enumerate() {
        if left <= 0 {
            break;
        }
        if contract.key != key || !ctx.own_contract[index] {
            continue;
        }
        let take = left.min((contract.held - contract.used).max(0) as i64);
        if take > 0 {
            parts.push(Part {
                kind: PartKind::Contract(index as u16),
                lots: take,
                cents: 0,
            });
            left -= take;
        }
    }
    let mut max_fill = 0;
    if let Some(lane) = lane {
        max_fill = lane.fill_bps();
        // 2. The shipper's own carriers, at operating cost.
        for alloc in &lane.allocs {
            if left <= 0 {
                break;
            }
            if !ctx.own_carrier[alloc.carrier as usize] {
                continue;
            }
            let take = left.min((alloc.cap - alloc.used - alloc.held).max(0) as i64);
            if take > 0 {
                parts.push(Part {
                    kind: PartKind::Own(alloc.carrier),
                    lots: take,
                    cents: leg_cents(ds, from, to, mode, take, ds.logistics.own_cost_bps),
                });
                left -= take;
            }
        }
        // 3. Other carriers, priced by how full the lane is.
        if left > 0 {
            let others: Vec<(u16, i32)> = lane
                .allocs
                .iter()
                .filter(|a| !ctx.own_carrier[a.carrier as usize])
                .map(|a| (a.carrier, (a.cap - a.used - a.held).max(0)))
                .filter(|(_, free)| *free > 0)
                .collect();
            let free: i64 = others.iter().map(|(_, f)| *f as i64).sum();
            let take = left.min(free);
            if take > 0 {
                let cap = lane.cap().max(1) as i64;
                let committed = (lane.used() + lane.held()) as i64;
                let f0 = committed * 10_000 / cap;
                let f1 = (committed + take) * 10_000 / cap;
                let mut mult = spot_mult_bps(ds, ((f0 + f1) / 2) as i32);
                if st.common_carrier {
                    mult = mult.min(regulated_cap_bps(ds));
                }
                let total = leg_cents(ds, from, to, mode, take, mult);
                let weights: Vec<(u64, i64)> =
                    others.iter().map(|(c, f)| (*c as u64, *f as i64)).collect();
                let lot_split = allocate_cents(Cents(take), &weights);
                let cent_split = allocate_cents(
                    Cents(total),
                    &lot_split.iter().map(|(c, l)| (*c, l.0)).collect::<Vec<_>>(),
                );
                for ((carrier, lots_i), (_, cents_i)) in lot_split.iter().zip(cent_split) {
                    if lots_i.0 > 0 {
                        parts.push(Part {
                            kind: PartKind::Public(*carrier as u16),
                            lots: lots_i.0,
                            cents: cents_i.0,
                        });
                    }
                }
                left -= take;
                max_fill = max_fill.max(f1.clamp(0, 10_000) as i32);
            }
        }
    }
    // 4. Overflow to outside carriers at a premium. Pipelines have none.
    if left > 0 {
        if !def.outside_backstop {
            return None;
        }
        parts.push(Part {
            kind: PartKind::Outside,
            lots: left,
            cents: leg_cents(ds, from, to, mode, left, ds.logistics.outside_premium_bps),
        });
        max_fill = 10_000;
    }
    let cents = parts.iter().map(|p| p.cents).sum();
    Some(LegPlan {
        from,
        to,
        mode,
        weeks: leg_weeks(ds, from, to, mode),
        cents,
        parts,
        fill_bps: max_fill,
    })
}

// ---------------------------------------------------------------------------------------------
// Route shapes

/// The hubs worth routing through for a trip: regions within a modest detour of the direct line.
fn hubs(ds: &Dataset, from: u8, to: u8) -> Vec<u8> {
    let direct = ds.km(from, to) as i64;
    (0..ds.regions.len() as u8)
        .filter(|h| *h != from && *h != to)
        .filter(|h| (ds.km(from, *h) + ds.km(*h, to)) as i64 * 10_000 <= direct * DETOUR_BPS)
        .collect()
}

/// Static price of a leg at the week's start: the owners' empty rate where the lane has capacity,
/// the outside price where it has none.
fn static_leg(ds: &Dataset, st: &LogisticsState, a: u8, b: u8, mode: u8, lots: i64) -> Option<i64> {
    let def = &ds.logistics.modes[mode as usize];
    if !lane_possible(ds, st, mode, a, b) {
        return None;
    }
    let key = LaneKey::new(a, b, mode);
    let has_cap = st.lanes.get(&key).is_some_and(|l| l.cap() > 0);
    if !has_cap && !def.outside_backstop {
        return None;
    }
    let mult = if has_cap {
        ds.logistics.empty_lane_bps
    } else {
        ds.logistics.outside_premium_bps
    };
    Some(leg_cents(ds, a, b, mode, lots, mult))
}

fn structure_cost(
    ds: &Dataset,
    st: &LogisticsState,
    legs: &[(u8, u8, u8)],
    lots: i64,
) -> Option<(i64, u32)> {
    let mut cents = 0i64;
    let mut weeks = 0u32;
    let mut last_mode: Option<u8> = None;
    for (a, b, mode) in legs {
        cents += static_leg(ds, st, *a, *b, *mode, lots)?;
        weeks += leg_weeks(ds, *a, *b, *mode);
        if last_mode.is_some_and(|m| m != *mode) {
            weeks += TRANSFER_WEEKS;
        }
        last_mode = Some(*mode);
    }
    Some((cents, weeks))
}

/// The candidate shapes for a trip and a class of goods: the cheapest few and the fastest few,
/// always with the direct truck haul.
#[allow(clippy::type_complexity)]
fn build_structures(
    ds: &Dataset,
    st: &LogisticsState,
    from: u8,
    to: u8,
    class: StorageClass,
) -> Vec<Structure> {
    const REFERENCE_LOTS: i64 = 100;
    let modes = ds.logistics.modes.len() as u8;
    let carries = |m: u8| ds.logistics.modes[m as usize].classes.contains(&class);
    let mut all: Vec<Vec<(u8, u8, u8)>> = Vec::new();
    for m in (0..modes).filter(|m| carries(*m)) {
        all.push(vec![(from, to, m)]);
    }
    let hub_list = hubs(ds, from, to);
    for h in &hub_list {
        for m1 in (0..modes).filter(|m| carries(*m)) {
            for m2 in (0..modes).filter(|m| carries(*m) && *m != m1) {
                all.push(vec![(from, *h, m1), (*h, to, m2)]);
            }
        }
    }
    // Truck, then a trunk haul, then truck: the usual way to reach a terminal.
    for h1 in &hub_list {
        for h2 in &hub_list {
            if h1 == h2 || ds.km(from, *h1) > ds.km(from, *h2) {
                continue;
            }
            let detour = (ds.km(from, *h1) + ds.km(*h1, *h2) + ds.km(*h2, to)) as i64 * 10_000;
            if detour > ds.km(from, to) as i64 * DETOUR_BPS {
                continue;
            }
            for trunk in (1..modes).filter(|m| carries(*m)) {
                all.push(vec![(from, *h1, 0), (*h1, *h2, trunk), (*h2, to, 0)]);
            }
        }
    }
    let mut scored: Vec<(i64, u32, Vec<(u8, u8, u8)>, i64)> = Vec::new();
    for legs in all {
        let Some((cents, weeks)) = structure_cost(ds, st, &legs, REFERENCE_LOTS) else {
            continue;
        };
        let min_lots = legs
            .iter()
            .map(|(_, _, m)| ds.logistics.modes[*m as usize].min_lots)
            .max()
            .unwrap_or(1);
        scored.push((cents, weeks, legs, min_lots));
    }
    let truck: Vec<(u8, u8, u8)> = vec![(from, to, 0)];
    let mut chosen: Vec<Structure> = Vec::new();
    let push = |legs: &Vec<(u8, u8, u8)>, min_lots: i64, chosen: &mut Vec<Structure>| {
        if !chosen.iter().any(|s| &s.legs == legs) {
            chosen.push(Structure {
                legs: legs.clone(),
                min_lots,
            });
        }
    };
    scored.sort_by(|x, y| (x.0, x.1, &x.2).cmp(&(y.0, y.1, &y.2)));
    for (_, _, legs, min_lots) in scored.iter().take(5) {
        push(legs, *min_lots, &mut chosen);
    }
    scored.sort_by(|x, y| (x.1, x.0, &x.2).cmp(&(y.1, y.0, &y.2)));
    for (_, _, legs, min_lots) in scored.iter().take(3) {
        push(legs, *min_lots, &mut chosen);
    }
    // Small shipments: the cheapest shape that any lot size can use.
    scored.sort_by(|x, y| (x.0, x.1, &x.2).cmp(&(y.0, y.1, &y.2)));
    if let Some((_, _, legs, min_lots)) = scored.iter().find(|s| s.3 <= 1) {
        push(legs, *min_lots, &mut chosen);
    }
    if let Some((_, _, legs, min_lots)) = scored.iter().find(|s| s.2 == truck) {
        push(legs, *min_lots, &mut chosen);
    }
    chosen
}

fn structures(
    world: &mut World,
    ds: &Dataset,
    from: u8,
    to: u8,
    class: StorageClass,
) -> Vec<Structure> {
    let key = (from, to, class.code());
    if let Some(found) = world.logistics.cache.get(&key) {
        return found.clone();
    }
    let built = build_structures(ds, &world.logistics, from, to, class);
    world.logistics.cache.insert(key, built.clone());
    built
}

// ---------------------------------------------------------------------------------------------
// Routing

/// The shipping policy of the division that receives goods: the plant in the region that uses
/// the good as an input, else any plant there, else the default.
pub fn policy_for(
    world: &World,
    ds: &Dataset,
    firm: Handle,
    region: u8,
    product: u16,
) -> ShipPolicy {
    let Some(entity) = world.firms.get(firm) else {
        return ShipPolicy::default();
    };
    let mut fallback: Option<ShipPolicy> = None;
    for handle in &entity.facilities {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        if plant.region != region {
            continue;
        }
        let recipe = &ds.recipes[plant.recipe as usize];
        if recipe
            .weekly_inputs_lots
            .contains_key(&ds.products[product as usize].id)
        {
            return plant.ship;
        }
        fallback.get_or_insert(plant.ship);
    }
    fallback.unwrap_or_default()
}

/// Whether the policy's preferred carrier has room on a leg's lane.
fn carrier_on_lane(st: &LogisticsState, carrier: Handle, leg: &LegPlan) -> bool {
    st.lanes
        .get(&LaneKey::new(leg.from, leg.to, leg.mode))
        .is_some_and(|lane| {
            lane.allocs
                .iter()
                .any(|a| st.carriers[a.carrier as usize].firm == carrier && a.cap > a.used + a.held)
        })
}

/// Pick the best route for a shipment under a policy. None only for a trip within one region.
#[allow(clippy::too_many_arguments)]
pub fn plan_route(
    world: &mut World,
    ds: &Dataset,
    shipper: Handle,
    policy: ShipPolicy,
    product: u16,
    from: u8,
    to: u8,
    qty: MilliLots,
) -> Option<RoutePlan> {
    if from == to {
        return None;
    }
    let lots = qty.lots().max(1);
    let class = ds.products[product as usize].storage_class;
    let shapes = structures(world, ds, from, to, class);
    let ctx = Ctx::new(world, shipper);
    let st = &world.logistics;
    type Rank = (i64, i64, i64, i64, i64);
    let mut best: Option<(Rank, RoutePlan)> = None;
    for shape in shapes {
        if lots < shape.min_lots {
            continue;
        }
        let mut legs: Vec<LegPlan> = Vec::new();
        let mut ok = true;
        for (a, b, mode) in &shape.legs {
            match quote_leg(ds, st, &ctx, product, *a, *b, *mode, lots) {
                Some(leg) => legs.push(leg),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok || legs.is_empty() {
            continue;
        }
        let mut weeks: u32 = legs.iter().map(|l| l.weeks).sum();
        weeks += legs.windows(2).filter(|w| w[0].mode != w[1].mode).count() as u32 * TRANSFER_WEEKS;
        let cents: i64 = legs.iter().map(|l| l.cents).sum();
        let outside: i64 = legs
            .iter()
            .flat_map(|l| l.parts.iter())
            .filter(|p| p.kind == PartKind::Outside)
            .map(|p| p.lots)
            .sum();
        let borrowed: i64 = legs
            .iter()
            .flat_map(|l| l.parts.iter())
            .filter(|p| !matches!(p.kind, PartKind::Own(_) | PartKind::Contract(_)))
            .map(|p| p.lots)
            .sum();
        let max_fill = legs.iter().map(|l| l.fill_bps).max().unwrap_or(0) as i64;
        // A preference that a route does not meet ranks it behind every route that does.
        let mut miss = 0i64;
        if let Some(mode) = policy.mode
            && (legs.iter().any(|l| l.mode != mode && l.mode != 0)
                || !legs.iter().any(|l| l.mode == mode))
        {
            miss += 1;
        }
        if let Some(carrier) = policy.carrier
            && !legs.iter().all(|l| carrier_on_lane(st, carrier, l))
        {
            miss += 1;
        }
        let key = match policy.kind {
            ShipKind::Cheapest => (miss, cents, weeks as i64, 0, 0),
            ShipKind::Fastest => (miss, weeks as i64, cents, 0, 0),
            ShipKind::OwnFirst => (miss, borrowed, cents, weeks as i64, 0),
            ShipKind::Reliable => (miss, outside, max_fill, cents, weeks as i64),
        };
        let plan = RoutePlan {
            legs,
            cents,
            weeks,
            lots,
        };
        if best.as_ref().is_none_or(|(k, _)| key < *k) {
            best = Some((key, plan));
        }
    }
    let mut plan = best.map(|(_, p)| p)?;
    // A shipment never costs less than the minimum charge, unless it is wholly prepaid.
    if plan.cents > 0 && plan.cents < crate::economy::freight::FREIGHT_MINIMUM_CENTS {
        let gap = crate::economy::freight::FREIGHT_MINIMUM_CENTS - plan.cents;
        if let Some(part) = plan
            .legs
            .iter_mut()
            .flat_map(|l| l.parts.iter_mut())
            .rev()
            .find(|p| p.cents > 0)
        {
            part.cents += gap;
            plan.cents += gap;
            if let Some(leg) = plan
                .legs
                .iter_mut()
                .rev()
                .find(|l| l.parts.iter().any(|p| p.cents > 0))
            {
                leg.cents += gap;
            }
        }
    }
    Some(plan)
}

/// What a shipment would cost and how long it takes, without booking it.
pub fn quote(
    world: &mut World,
    ds: &Dataset,
    payer: Handle,
    product: u16,
    from: u8,
    to: u8,
    qty: MilliLots,
) -> Option<RoutePlan> {
    let policy = policy_for(world, ds, payer, to, product);
    plan_route(world, ds, payer, policy, product, from, to, qty)
}

/// The result of booking a shipment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Booked {
    pub cents: i64,
    pub weeks: u32,
}

/// Route a shipment, take capacity on every leg and charge the payer once. Carrier receipts and
/// outside receipts sum to the payer's posting exactly. A payer short of cash pays what it has and
/// every receipt is scaled down together.
#[allow(clippy::too_many_arguments)]
pub fn book(
    world: &mut World,
    ds: &Dataset,
    payer: Handle,
    product: u16,
    from: u8,
    to: u8,
    qty: MilliLots,
    memo: &str,
) -> Booked {
    let Some(plan) = quote(world, ds, payer, product, from, to, qty) else {
        return Booked {
            cents: 0,
            weeks: crate::economy::freight::lead_time_weeks(ds.km(from, to)),
        };
    };
    let cash = world
        .firms
        .get(payer)
        .map(|f| f.book.cash().max(0))
        .unwrap_or(0);
    let paid = plan.cents.min(cash);
    // Every paying part, scaled to what the payer can afford.
    let mut flat: Vec<(usize, usize)> = Vec::new();
    let mut weights: Vec<(u64, i64)> = Vec::new();
    for (li, leg) in plan.legs.iter().enumerate() {
        for (pi, part) in leg.parts.iter().enumerate() {
            if part.cents > 0 {
                flat.push((li, pi));
                weights.push((flat.len() as u64 - 1, part.cents));
            }
        }
    }
    let scaled: Vec<i64> = if paid >= plan.cents {
        flat.iter()
            .map(|(li, pi)| plan.legs[*li].parts[*pi].cents)
            .collect()
    } else if paid > 0 {
        allocate_cents(Cents(paid), &weights)
            .into_iter()
            .map(|(_, c)| c.0)
            .collect()
    } else {
        vec![0; flat.len()]
    };
    if paid > 0
        && let Some(entity) = world.firms.get_mut(payer)
    {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, paid, memo)
                .debit(Account::CostOfGoods, paid, memo),
        );
    }
    let lots = plan.lots;
    let mut outside_cents = 0i64;
    let mut dest = DestFreight {
        lots,
        ..Default::default()
    };
    dest.add_sample(Sample {
        from,
        product,
        lots,
    });
    for (li, leg) in plan.legs.iter().enumerate() {
        let key = LaneKey::new(leg.from, leg.to, leg.mode);
        let km = leg_km(ds, leg.from, leg.to, leg.mode) as i64;
        let totals = &mut world.logistics.totals;
        totals.lots_by_mode[leg.mode as usize] += lots;
        totals.lot_km += lots * km;
        for part in &leg.parts {
            match part.kind {
                PartKind::Own(_) => totals.own_lots += part.lots,
                PartKind::Contract(_) => totals.contract_lots += part.lots,
                PartKind::Outside => totals.outside_lots_by_mode[leg.mode as usize] += part.lots,
                PartKind::Public(_) => {}
            }
        }
        let flow = world.logistics.flow_week.entry(key).or_insert(0);
        *flow = flow.saturating_add(lots as i32);
        for (pi, part) in leg.parts.iter().enumerate() {
            let cents = flat
                .iter()
                .position(|(l, p)| *l == li && *p == pi)
                .map(|i| scaled[i])
                .unwrap_or(0);
            let mut use_ = LaneUse {
                key,
                lots: part.lots,
                cents,
                outside_lots: 0,
                outside_cents: 0,
            };
            dest.cents += cents;
            match part.kind {
                PartKind::Contract(_) | PartKind::Own(_) => {
                    dest.own_cents += cents;
                    dest.own_lots += part.lots;
                }
                PartKind::Public(_) => {
                    dest.other_cents += cents;
                    dest.other_lots += part.lots;
                }
                PartKind::Outside => {
                    dest.outside_cents += cents;
                    dest.outside_lots += part.lots;
                    use_.outside_lots = part.lots;
                    use_.outside_cents = cents;
                }
            }
            dest.add_lane(use_);
            match part.kind {
                PartKind::Contract(index) => {
                    if let Some(c) = world.logistics.contracts.get_mut(index as usize) {
                        c.used += part.lots as i32;
                    }
                    if let Some(lane) = world.logistics.lanes.get_mut(&key) {
                        lane.contract_used += part.lots as i32;
                    }
                }
                PartKind::Own(carrier) | PartKind::Public(carrier) => {
                    if let Some(lane) = world.logistics.lanes.get_mut(&key)
                        && let Some(alloc) = lane.allocs.iter_mut().find(|a| a.carrier == carrier)
                    {
                        alloc.used += part.lots as i32;
                    }
                    if let Some(c) = world.logistics.carriers.get_mut(carrier as usize) {
                        c.used_lot_km += part.lots * km;
                    }
                    credit_carrier(
                        world,
                        ds,
                        carrier,
                        cents,
                        part.lots * km,
                        crate::economy::fx::currency_of(world, payer),
                    );
                }
                PartKind::Outside => {
                    if let Some(lane) = world.logistics.lanes.get_mut(&key) {
                        lane.outside += part.lots as i32;
                    } else {
                        world.logistics.lanes.insert(
                            key,
                            LaneWeek {
                                outside: part.lots as i32,
                                ..Default::default()
                            },
                        );
                    }
                    outside_cents += cents;
                }
            }
        }
        dest.mode_lots[(leg.mode as usize).min(3)] += lots;
        let lane = world.logistics.lanes.entry(key).or_default();
        match lane.users.iter_mut().find(|(f, _)| *f == payer) {
            Some(entry) => entry.1 += lots as i32,
            None => lane.users.push((payer, lots as i32)),
        }
    }
    if outside_cents > 0 {
        let received = world.fx.convert(
            outside_cents,
            crate::economy::fx::currency_of(world, payer),
            0,
        );
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new().debit(Account::Cash, received, memo).credit(
                Account::Revenue,
                received,
                memo,
            ),
        );
    }
    let row = world.logistics.dest_week.entry((payer, to)).or_default();
    row.cents += dest.cents;
    row.own_cents += dest.own_cents;
    row.other_cents += dest.other_cents;
    row.outside_cents += dest.outside_cents;
    row.lots += dest.lots;
    row.own_lots += dest.own_lots;
    row.other_lots += dest.other_lots;
    row.outside_lots += dest.outside_lots;
    for (m, l) in dest.mode_lots.iter().enumerate() {
        row.mode_lots[m] += l;
    }
    for lane in dest.lanes {
        row.add_lane(lane);
    }
    for sample in dest.samples {
        row.add_sample(sample);
    }
    let totals = &mut world.logistics.totals;
    totals.cents += paid;
    totals.outside_cents += outside_cents;
    totals.shipments += 1;
    Booked {
        cents: paid,
        weeks: plan.weeks,
    }
}

/// Pay a carrier for lots it moved: revenue now, and the freight stock it made for the purpose is
/// relieved at cost. A carrier that has gone is replaced by the outside sector.
fn credit_carrier(
    world: &mut World,
    ds: &Dataset,
    carrier: u16,
    cents: i64,
    lot_km: i64,
    payer_currency: u8,
) {
    if cents <= 0 && lot_km <= 0 {
        return;
    }
    let Some(c) = world.logistics.carriers.get(carrier as usize).cloned() else {
        return;
    };
    let freight = ds.product_index["freight"];
    let tick = world.tick;
    let alive = world.firms.get(c.firm).is_some_and(|f| f.operative());
    if !alive {
        if cents > 0 {
            let cents = world.fx.convert(cents, payer_currency, 0);
            let _ = world.outside.gateway_book.commit(
                tick,
                Txn::new().debit(Account::Cash, cents, "freight").credit(
                    Account::Revenue,
                    cents,
                    "freight",
                ),
            );
        }
        return;
    }
    // The payer paid in its money; the carrier is paid in its own.
    let cents = world.fx.convert(
        cents,
        payer_currency,
        crate::economy::fx::currency_of(world, c.firm),
    );
    let Some(firm) = world.firms.get_mut(c.firm) else {
        return;
    };
    // The freight stock a carrier makes is capacity over the reference haul.
    let used = MilliLots(lot_km * 1_000 / REF_KM);
    let take = used.min(firm.stock(freight, c.region));
    let carrying = if take.0 > 0 {
        firm.issue_stock(freight, c.region, take).0
    } else {
        0
    };
    firm.record_sale(c.region, freight, used);
    let _ = firm.book.commit(
        tick,
        Txn::new()
            .debit(Account::Cash, cents, "freight")
            .credit(Account::Revenue, cents, "freight")
            .credit(Account::Inventory, carrying, "freight cost")
            .debit(Account::CostOfGoods, carrying, "freight cost"),
    );
}

// ---------------------------------------------------------------------------------------------
// Estimates for pairing and for rivals

/// Cheapest direct delivery, cents per lot, for a good class between two regions at the week's
/// opening prices. Used by the exchange to rank sellers.
pub fn estimate_cents_per_lot(
    world: &mut World,
    ds: &Dataset,
    from: u8,
    to: u8,
    class: StorageClass,
) -> i64 {
    if from == to {
        return 0;
    }
    let n = ds.regions.len();
    if world.logistics.estimates.len() != 4 * n * n {
        let mut table = vec![0i32; 4 * n * n];
        for class in StorageClass::ALL {
            for a in 0..n as u8 {
                for b in 0..n as u8 {
                    if a == b {
                        continue;
                    }
                    let best = (0..ds.logistics.modes.len() as u8)
                        .filter(|m| ds.logistics.modes[*m as usize].classes.contains(&class))
                        .filter(|m| ds.logistics.modes[*m as usize].min_lots <= 100)
                        .filter_map(|m| static_leg(ds, &world.logistics, a, b, m, 100))
                        .min()
                        .unwrap_or(0);
                    table[class.code() as usize * n * n + a as usize * n + b as usize] =
                        (best / 100) as i32;
                }
            }
        }
        world.logistics.estimates = table;
    }
    let i = class.code() as usize * n * n + from as usize * n + to as usize;
    (world.logistics.estimates[i] as i64).max(1)
}

// ---------------------------------------------------------------------------------------------
// Commands

/// Set the shipping policy of every plant in a division.
pub fn set_policy(
    world: &mut World,
    ds: &Dataset,
    facility: Handle,
    policy: ShipPolicy,
) -> DomainResult<()> {
    let (firm, region, industry) = {
        let plant = world
            .facilities
            .get(facility)
            .ok_or(DomainError::InvalidEntity {
                handle: facility.0,
                kind: EntityKind::Facility,
            })?;
        (
            plant.firm,
            plant.region,
            crate::economy::sectors::recipe_industry(ds, plant.recipe),
        )
    };
    if policy
        .mode
        .is_some_and(|m| m as usize >= ds.logistics.modes.len())
    {
        return Err(DomainError::Arithmetic {
            detail: ArithmeticFault::OutOfRange,
        });
    }
    if let Some(carrier) = policy.carrier
        && world.firms.get(carrier).is_none()
    {
        return Err(DomainError::InvalidEntity {
            handle: carrier.0,
            kind: EntityKind::Firm,
        });
    }
    for handle in crate::economy::sectors::division_plants(world, ds, firm, region, industry) {
        if let Some(plant) = world.facilities.get_mut(handle) {
            plant.ship = policy;
        }
    }
    Ok(())
}

/// Pin the regions a carrier division spreads its capacity over. Empty returns it to automatic.
pub fn set_serves(
    world: &mut World,
    ds: &Dataset,
    facility: Handle,
    regions: Vec<u8>,
) -> DomainResult<()> {
    let plant = world
        .facilities
        .get(facility)
        .ok_or(DomainError::InvalidEntity {
            handle: facility.0,
            kind: EntityKind::Facility,
        })?;
    let recipe = &ds.recipes[plant.recipe as usize];
    if recipe.transport_mode.is_none()
        || regions.len() > 12
        || regions.iter().any(|r| *r as usize >= ds.regions.len())
    {
        return Err(DomainError::Arithmetic {
            detail: ArithmeticFault::OutOfRange,
        });
    }
    let (firm, region, industry) = (
        plant.firm,
        plant.region,
        crate::economy::sectors::recipe_industry(ds, plant.recipe),
    );
    let mut regions = regions;
    regions.sort_unstable();
    regions.dedup();
    regions.retain(|r| *r != region);
    for handle in crate::economy::sectors::division_plants(world, ds, firm, region, industry) {
        if let Some(p) = world.facilities.get_mut(handle) {
            p.serves = regions.clone();
        }
    }
    Ok(())
}

/// The weekly fee for one reserved lot: the lane's spot price once the reservation is counted as
/// used, less the commitment discount. The same figure `reserve_lane` charges.
pub fn contract_fee_cents(world: &World, ds: &Dataset, key: LaneKey, lots: i32) -> i64 {
    let cap = world
        .logistics
        .lanes
        .get(&key)
        .map(|l| l.cap())
        .unwrap_or(0)
        .max(1) as i64;
    let after = world
        .logistics
        .lanes
        .get(&key)
        .map(|lane| {
            (((lane.used() + lane.held()) as i64 + lots as i64) * 10_000 / cap).clamp(0, 10_000)
        })
        .unwrap_or(0);
    let mut spot = spot_mult_bps(ds, after as i32);
    if world.logistics.common_carrier {
        spot = spot.min(regulated_cap_bps(ds));
    }
    let mult = spot * ds.logistics.contract_discount_bps / 10_000;
    leg_cents(ds, key.a, key.b, key.mode, 1, mult).max(1)
}

/// Lots on a lane that a firm's group could reserve now: capacity of other groups' carriers that
/// is neither used nor already held.
pub fn reservable_lots(world: &World, firm: Handle, key: LaneKey) -> i64 {
    let st = &world.logistics;
    st.lanes
        .get(&key)
        .map(|lane| {
            let open: i64 = lane
                .allocs
                .iter()
                .filter(|a| {
                    !crate::economy::ownership::same_group(
                        world,
                        firm,
                        st.carriers[a.carrier as usize].firm,
                    )
                })
                .map(|a| (a.cap - a.held).max(0) as i64)
                .sum();
            let room = lane.cap() as i64 * contract_share_bps(st) / 10_000 - lane.held() as i64;
            open.min(room.max(0))
        })
        .unwrap_or(0)
}

/// Reserve lane capacity for a number of weeks at the lane's spot rate less the commitment
/// discount. The rate is fixed for the term. The first fee is charged when the contract starts,
/// at the next week's opening.
#[allow(clippy::too_many_arguments)]
pub fn reserve_lane(
    world: &mut World,
    ds: &Dataset,
    firm: Handle,
    from: u8,
    to: u8,
    mode: u8,
    lots: i32,
    weeks: u32,
) -> DomainResult<u32> {
    let bad = || DomainError::Arithmetic {
        detail: ArithmeticFault::OutOfRange,
    };
    if from == to
        || mode as usize >= ds.logistics.modes.len()
        || lots < 1
        || !(MIN_CONTRACT_WEEKS..=MAX_CONTRACT_WEEKS).contains(&weeks)
        || world.logistics.contracts.len() >= MAX_CONTRACTS
    {
        return Err(bad());
    }
    let key = LaneKey::new(from, to, mode);
    if !lane_possible(ds, &world.logistics, mode, from, to) {
        return Err(bad());
    }
    let cap = world
        .logistics
        .lanes
        .get(&key)
        .map(|l| l.cap())
        .unwrap_or(0) as i64;
    let open = reservable_lots(world, firm, key);
    if cap <= 0
        || lots as i64 > open
        || lots as i64 * 10_000 > cap * contract_share_bps(&world.logistics)
        || world
            .logistics
            .contracts
            .iter()
            .any(|c| c.holder == firm && c.key == key)
    {
        return Err(DomainError::WrongPhase {
            expected: "a lane with unreserved capacity",
            actual: "no room on the lane",
        });
    }
    let fee = contract_fee_cents(world, ds, key, lots);
    let id = world.logistics.next_contract;
    world.logistics.next_contract += 1;
    let tick = world.tick.0;
    world.logistics.contracts.push(LaneContract {
        id,
        holder: firm,
        key,
        lots,
        held: 0,
        used: 0,
        fee_per_lot_cents: fee,
        start: tick + 1,
        weeks_left: weeks,
    });
    world.logistics.reservations.push(Reservation {
        tick,
        key,
        holder: firm,
        lots,
    });
    if world.logistics.reservations.len() > 32 {
        world.logistics.reservations.remove(0);
    }
    crate::economy::rivalry::on_lane_reserved(world, firm, key);
    Ok(id)
}

// ---------------------------------------------------------------------------------------------
// Reading for apps, rivals, alerts and telemetry

/// One lane for display.
#[derive(Clone, Debug)]
pub struct LaneView {
    pub key: LaneKey,
    pub km: u32,
    pub cap: i32,
    pub used: i32,
    pub held: i32,
    pub outside: i32,
    pub fill_bps: i32,
    /// Spot price of one lot over the whole lane, cents.
    pub rate_cents: i64,
    pub owners: Vec<(Handle, i32)>,
    pub contracts: usize,
}

pub fn lane_views(world: &World, ds: &Dataset) -> Vec<LaneView> {
    let st = &world.logistics;
    let mut out = Vec::new();
    for (key, lane) in &st.lanes {
        let mut owners: BTreeMap<Handle, i32> = BTreeMap::new();
        for alloc in &lane.allocs {
            *owners
                .entry(st.carriers[alloc.carrier as usize].firm)
                .or_default() += alloc.cap;
        }
        out.push(LaneView {
            key: *key,
            km: leg_km(ds, key.a, key.b, key.mode),
            cap: lane.cap(),
            used: lane.used() + lane.contract_used,
            held: lane.held(),
            outside: lane.outside,
            fill_bps: lane.fill_bps(),
            rate_cents: lane_rate_cents_per_lot(ds, st, *key),
            owners: owners.into_iter().collect(),
            contracts: st.contracts.iter().filter(|c| c.key == *key).count(),
        });
    }
    out
}

/// Whether a firm shipped on a lane in the last four weeks.
pub fn depends_on(world: &World, firm: Handle, key: LaneKey) -> bool {
    world.logistics.stats.get(&key).is_some_and(|stat| {
        stat.users
            .iter()
            .any(|(f, last)| *f == firm && world.tick.0.saturating_sub(*last) <= 4)
    })
}

/// A carrier's utilisation last week, in basis points, for its own planning. None for a plant
/// that is not a carrier or has no capacity yet.
pub fn carrier_utilisation_bps(world: &World, facility: Handle) -> Option<i32> {
    world
        .logistics
        .carriers
        .iter()
        .find(|c| c.facility == facility)
        .filter(|c| c.cap > 0)
        .map(|c| (c.used_lot_km * 10_000 / (c.cap as i64 * REF_KM)).clamp(0, 10_000) as i32)
}

/// A lane that is short of capacity: much of its freight went to outside carriers or it has run
/// full for weeks.
#[derive(Clone, Debug)]
pub struct HotLane {
    pub key: LaneKey,
    pub moved: i32,
    pub outside: i32,
    pub fill_bps: i32,
    pub hot_weeks: u16,
    pub cap: i32,
    pub rate_cents: i64,
}

/// Lanes worth adding capacity to, as closed last week, busiest outside overflow first.
pub fn hot_lanes(world: &World, ds: &Dataset) -> Vec<HotLane> {
    let st = &world.logistics;
    let mut out: Vec<HotLane> = st
        .stats
        .iter()
        .filter(|(_, s)| s.moved >= 300 && (s.outside * 100 >= s.moved * 30 || s.hot_weeks >= 2))
        .map(|(key, s)| HotLane {
            key: *key,
            moved: s.moved,
            outside: s.outside,
            fill_bps: s.fill_bps,
            hot_weeks: s.hot_weeks,
            cap: s.cap,
            rate_cents: lane_rate_cents_per_lot(ds, st, *key),
        })
        .collect();
    out.sort_by_key(|h| (std::cmp::Reverse(h.outside), h.key));
    out
}

/// Share of a lane's capacity owned by a firm's group, in basis points.
pub fn group_share_bps(world: &World, firm: Handle, key: LaneKey) -> i64 {
    let st = &world.logistics;
    let Some(lane) = st.lanes.get(&key) else {
        return 0;
    };
    let cap = lane.cap() as i64;
    if cap <= 0 {
        return 0;
    }
    let own: i64 = lane
        .allocs
        .iter()
        .filter(|a| {
            crate::economy::ownership::same_group(world, firm, st.carriers[a.carrier as usize].firm)
        })
        .map(|a| a.cap as i64)
        .sum();
    own * 10_000 / cap
}

/// Lots a firm moved on a lane in the week just closed.
pub fn shipped_last_week(world: &World, firm: Handle, key: LaneKey) -> i32 {
    world
        .logistics
        .lanes
        .get(&key)
        .and_then(|lane| lane.users.iter().find(|(f, _)| *f == firm))
        .map(|(_, lots)| *lots)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Shipping summaries for the apps

/// A lane a firm or division shipped on in the week just closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShippingLane {
    pub key: LaneKey,
    pub lots: i64,
    pub cents: i64,
    pub outside_lots: i64,
    pub fill_bps: i32,
    pub hot_weeks: u16,
}

/// What the engine suggests doing about freight spend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Suggestion {
    /// Build carrier blocks of a mode on a lane the group pays outside carriers on.
    Build {
        key: LaneKey,
        recipe: u16,
        blocks: i64,
        upfront_cents: i64,
        weekly_saving_cents: i64,
        payback_weeks: i64,
    },
    /// Reserve lots on a busy lane the group ships on.
    Reserve {
        key: LaneKey,
        lots: i32,
        weekly_fee_cents: i64,
        weekly_saving_cents: i64,
    },
}

/// A firm's or division's freight for the week just closed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShippingSummary {
    pub cents: i64,
    pub own_cents: i64,
    pub other_cents: i64,
    pub outside_cents: i64,
    pub fee_cents: i64,
    pub lots: i64,
    pub own_lots: i64,
    pub other_lots: i64,
    pub outside_lots: i64,
    pub mode_lots: [i64; 4],
    /// What outside carriers charged beyond what the same lots cost on own carriers.
    pub outside_extra_cents: i64,
    /// The busiest three lanes by spend.
    pub lanes: Vec<ShippingLane>,
    pub suggestion: Option<Suggestion>,
}

fn merged_rows(
    world: &World,
    firm: Handle,
    region: Option<u8>,
) -> impl Iterator<Item = &DestFreight> {
    world
        .logistics
        .dest_last
        .range((firm, 0)..=(firm, u8::MAX))
        .filter(move |((_, r), _)| match region {
            Some(want) => *r == want,
            None => true,
        })
        .map(|(_, row)| row)
}

/// Lots the investor's group moved on a lane in the closed week.
fn group_lane_lots(world: &World, firm: Handle, key: LaneKey) -> i32 {
    world
        .logistics
        .lanes
        .get(&key)
        .map(|lane| {
            lane.users
                .iter()
                .filter(|(f, _)| crate::economy::ownership::same_group(world, firm, *f))
                .map(|(_, lots)| *lots)
                .sum()
        })
        .unwrap_or(0)
}

/// Freight paid by a firm for goods arriving in one region (a division), or in all of them with
/// the firm's lane contract fees (the CEO Office). Read back from the shipments of the week just
/// closed.
pub fn shipping_summary(
    world: &World,
    ds: &Dataset,
    firm: Handle,
    region: Option<u8>,
) -> ShippingSummary {
    let mut out = ShippingSummary::default();
    let mut lanes: Vec<LaneUse> = Vec::new();
    for row in merged_rows(world, firm, region) {
        out.cents += row.cents;
        out.own_cents += row.own_cents;
        out.other_cents += row.other_cents;
        out.outside_cents += row.outside_cents;
        out.fee_cents += row.fee_cents;
        out.lots += row.lots;
        out.own_lots += row.own_lots;
        out.other_lots += row.other_lots;
        out.outside_lots += row.outside_lots;
        for (m, l) in row.mode_lots.iter().enumerate() {
            out.mode_lots[m] += l;
        }
        for lane in &row.lanes {
            match lanes.iter_mut().find(|l| l.key == lane.key) {
                Some(l) => {
                    l.lots += lane.lots;
                    l.cents += lane.cents;
                    l.outside_lots += lane.outside_lots;
                    l.outside_cents += lane.outside_cents;
                }
                None => lanes.push(*lane),
            }
        }
    }
    let l = &ds.logistics;
    out.outside_extra_cents =
        out.outside_cents - out.outside_cents * l.own_cost_bps / l.outside_premium_bps.max(1);
    lanes.sort_by_key(|u| (std::cmp::Reverse(u.cents), u.key));
    let stat_of = |key: LaneKey| world.logistics.stats.get(&key);
    out.lanes = lanes
        .iter()
        .take(3)
        .map(|u| ShippingLane {
            key: u.key,
            lots: u.lots,
            cents: u.cents,
            outside_lots: u.outside_lots,
            fill_bps: stat_of(u.key).map(|s| s.fill_bps).unwrap_or(0),
            hot_weeks: stat_of(u.key).map(|s| s.hot_weeks).unwrap_or(0),
        })
        .collect();
    out.suggestion = suggest(world, ds, firm, &lanes, out.cents);
    out
}

/// Most weeks of payback worth suggesting a build for.
const MAX_PAYBACK_WEEKS: i64 = 260;
/// A build that pays back within this is preferred over reserving.
const GOOD_PAYBACK_WEEKS: i64 = 104;

fn suggest(
    world: &World,
    ds: &Dataset,
    firm: Handle,
    lanes: &[LaneUse],
    total_cents: i64,
) -> Option<Suggestion> {
    let st = &world.logistics;
    let l = &ds.logistics;
    let mut build: Option<Suggestion> = None;
    // The lane where outside carriers took the most money.
    if let Some(lane) = lanes
        .iter()
        .filter(|u| u.outside_lots > 0)
        .max_by_key(|u| (u.outside_cents, std::cmp::Reverse(u.key)))
        .filter(|u| u.outside_cents >= 50_000 && u.outside_cents * 10 >= total_cents)
    {
        let key = lane.key;
        let def = &l.modes[key.mode as usize];
        let km = leg_km(ds, key.a, key.b, key.mode).max(1) as i64;
        let recipe = ds
            .recipes
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                !r.terminal
                    && r.weekly_output_lots > 0
                    && r.transport_mode.as_deref() == Some(def.id.as_str())
            })
            .min_by_key(|(i, r)| (r.capex_cents / r.weekly_output_lots.max(1), *i))
            .map(|(i, _)| i);
        if let Some(recipe) = recipe
            && def.outside_backstop
            && has_access(ds, st, key.mode, key.a)
            && has_access(ds, st, key.mode, key.b)
        {
            let per_block = (ds.recipes[recipe].weekly_output_lots * REF_KM / km).max(1);
            let blocks = (lane.outside_lots + per_block - 1) / per_block;
            let upfront = blocks * ds.recipes[recipe].capex_cents;
            let saving = lane.outside_cents
                - lane.outside_cents * l.own_cost_bps / l.outside_premium_bps.max(1);
            if saving > 0 && upfront > 0 {
                let payback = (upfront + saving - 1) / saving;
                if payback <= MAX_PAYBACK_WEEKS {
                    build = Some(Suggestion::Build {
                        key,
                        recipe: recipe as u16,
                        blocks,
                        upfront_cents: upfront,
                        weekly_saving_cents: saving,
                        payback_weeks: payback,
                    });
                }
            }
        }
    }
    if let Some(Suggestion::Build { payback_weeks, .. }) = &build
        && *payback_weeks <= GOOD_PAYBACK_WEEKS
    {
        return build;
    }
    // Otherwise a busy lane the group leans on and does not own.
    let reserve = lanes
        .iter()
        .filter(|u| {
            st.stats
                .get(&u.key)
                .is_some_and(|s| s.hot_weeks >= 2 || s.fill_bps >= 8_000)
        })
        .filter_map(|u| {
            let lots = (group_lane_lots(world, firm, u.key) as i64)
                .min(reservable_lots(world, firm, u.key));
            if lots < 10 {
                return None;
            }
            let q = reserve_quote(world, ds, firm, u.key, lots as i32, 13)?;
            (q.weekly_net_cents > 0).then_some((q.weekly_net_cents, u.key, lots as i32, q))
        })
        .max_by_key(|(net, key, _, _)| (*net, std::cmp::Reverse(*key)));
    if let Some((_, key, lots, q)) = reserve {
        return Some(Suggestion::Reserve {
            key,
            lots,
            weekly_fee_cents: q.weekly_fee_cents,
            weekly_saving_cents: q.weekly_net_cents,
        });
    }
    build
}

/// A priced lane reservation before it is made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveQuote {
    pub fee_per_lot_cents: i64,
    pub spot_per_lot_cents: i64,
    pub weekly_fee_cents: i64,
    pub total_cents: i64,
    /// Lots the group is expected to use, from what it moved on the lane last week.
    pub expected_use_lots: i32,
    /// Spot price saved on lots used, less the fee on lots left idle, per week.
    pub weekly_net_cents: i64,
    /// Most the lane will let the group reserve now.
    pub max_lots: i64,
}

/// What reserving lots on a lane for a number of weeks would cost and save. None for a lane with
/// no capacity.
pub fn reserve_quote(
    world: &World,
    ds: &Dataset,
    firm: Handle,
    key: LaneKey,
    lots: i32,
    weeks: u32,
) -> Option<ReserveQuote> {
    let lane = world.logistics.lanes.get(&key)?;
    let cap = lane.cap() as i64;
    if cap <= 0 || lots < 1 {
        return None;
    }
    let fee = contract_fee_cents(world, ds, key, lots);
    let spot = lane_rate_cents_per_lot(ds, &world.logistics, key);
    let used = group_lane_lots(world, firm, key).min(lots).max(0);
    let weekly_fee = fee * lots as i64;
    Some(ReserveQuote {
        fee_per_lot_cents: fee,
        spot_per_lot_cents: spot,
        weekly_fee_cents: weekly_fee,
        total_cents: weekly_fee * weeks as i64,
        expected_use_lots: used,
        weekly_net_cents: used as i64 * (spot - fee) - (lots - used) as i64 * fee,
        max_lots: reservable_lots(world, firm, key)
            .min(cap * contract_share_bps(&world.logistics) / 10_000),
    })
}

/// What the shipments of the closed week would have cost under a policy, at today's lane prices:
/// total cents and lots that would go to outside carriers.
pub fn estimate_policy(
    world: &mut World,
    ds: &Dataset,
    firm: Handle,
    region: u8,
    policy: ShipPolicy,
) -> Option<(i64, i64)> {
    let samples = world
        .logistics
        .dest_last
        .get(&(firm, region))
        .map(|row| row.samples.clone())
        .unwrap_or_default();
    if samples.is_empty() {
        return None;
    }
    let mut cents = 0;
    let mut outside = 0;
    for s in samples {
        if let Some(plan) = plan_route(
            world,
            ds,
            firm,
            policy,
            s.product,
            s.from,
            region,
            MilliLots::from_lots(s.lots),
        ) {
            cents += plan.cents;
            outside += plan.outside_lots();
        }
    }
    Some((cents, outside))
}
