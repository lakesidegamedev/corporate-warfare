//! Systemic events: shocks that hit the logistics network and the plants and storage behind it,
//! rather than adding or removing cash at random.
//!
//! * Pipeline spill: a pipeline lane is shut for weeks and its operators pay a clean-up bill.
//! * Port strike: every sea lane through a port stops; shipments go to outside carriers at the
//!   premium. Strong union law makes strikes likelier, weak union law rarer.
//! * Refinery fire: a processing plant (one that makes by-products) stops for months and loses a
//!   tenth of the stock it holds in its region.
//! * Lane outage: a truck or rail lane is cut or reduced to a quarter for a few weeks.
//! * Harvest failure: in the harvest quarter a region's farm yields fall for half a year.
//!
//! Incidence is drawn from a stream keyed by the game seed and the week, so it needs no saved
//! generator state and a loaded game draws exactly what a continued one does. Everything that
//! happens is recorded as an [`Incident`] naming the firms it touches, which is what alerts,
//! news and telemetry read.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::rng::Rng;
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::economy::logistics::{Closure, LaneKey};
use crate::state::{FacilityStatus, PoliticalNews, UnionLaw, World};

const TAG_SYSTEMIC: u64 = 0x5359_5354_454d_4943;
const KEPT: usize = 40;

pub const SPILL: u8 = 0;
pub const PORT_STRIKE: u8 = 1;
pub const REFINERY_FIRE: u8 = 2;
pub const LANE_OUTAGE: u8 = 3;
pub const HARVEST_FAILURE: u8 = 4;
pub const KINDS: usize = 5;
/// A dated event from the scenario's timeline. Not one of the five random kinds, so it has no
/// slot in the counts.
pub const TIMELINE: u8 = 5;

/// Weekly chance, in hundredths of a basis point, per exposed pipeline lane, port, plant, lane
/// and farming region (harvest quarter only, in basis points of a year).
const SPILL_CBPS: u64 = 800;
const STRIKE_CBPS: u64 = 600;
const FIRE_CBPS: u64 = 500;
const OUTAGE_CBPS: u64 = 300;
const HARVEST_BPS: i32 = 800;

pub fn kind_key(kind: u8) -> &'static str {
    match kind {
        SPILL => "spill",
        PORT_STRIKE => "port_strike",
        REFINERY_FIRE => "refinery_fire",
        LANE_OUTAGE => "lane_outage",
        TIMELINE => "timeline",
        _ => "harvest_failure",
    }
}

/// One incident: where it struck (`a`, and `b` for a lane), how long, and who it touches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Incident {
    pub week: u32,
    pub kind: u8,
    pub a: u8,
    pub b: u8,
    pub mode: u8,
    pub weeks: u16,
    pub firms: Vec<Handle>,
    pub text: String,
}

/// Part of the save layout: a field change bumps `legacy::LAYOUT` (see `crate::legacy`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct SystemicState {
    pub incidents: Vec<Incident>,
    pub counts: [u32; KINDS],
    /// The scenario timeline's cursor (layout 3). It lives here because incidents and timeline
    /// events share one log; see `economy::timeline`.
    #[serde(default)]
    pub timeline: crate::economy::timeline::TimelineState,
}

fn city(ds: &Dataset, region: u8) -> String {
    ds.regions
        .get(region as usize)
        .map(|r| r.principal_city.split(',').next().unwrap_or("").to_owned())
        .unwrap_or_default()
}

fn lane_text(ds: &Dataset, key: LaneKey) -> String {
    format!(
        "{} to {} by {}",
        city(ds, key.a),
        city(ds, key.b),
        ds.logistics.modes[key.mode as usize].name.to_lowercase()
    )
}

fn lane_firms(world: &World, key: LaneKey) -> Vec<Handle> {
    let st = &world.logistics;
    let mut firms: Vec<Handle> = Vec::new();
    if let Some(lane) = st.lanes.get(&key) {
        for alloc in &lane.allocs {
            firms.push(st.carriers[alloc.carrier as usize].firm);
        }
    }
    if let Some(stat) = st.stats.get(&key) {
        for (firm, last) in &stat.users {
            if world.tick.0.saturating_sub(*last) <= 8 {
                firms.push(*firm);
            }
        }
    }
    firms.sort_unstable();
    firms.dedup();
    firms
}

/// Record a timeline event: an incident and a news line, outside the random kinds' counts.
pub fn record_timeline(world: &mut World, incident: Incident) {
    world.systemic.incidents.push(incident);
    let excess = world.systemic.incidents.len().saturating_sub(KEPT);
    world.systemic.incidents.drain(..excess);
}

fn record(world: &mut World, incident: Incident) {
    world.systemic.counts[incident.kind as usize] += 1;
    world.politics.news.push(PoliticalNews {
        person: None,
        week: world.tick.0,
        text: incident.text.clone(),
    });
    let excess = world.politics.news.len().saturating_sub(40);
    world.politics.news.drain(..excess);
    world.systemic.incidents.push(incident);
    let excess = world.systemic.incidents.len().saturating_sub(KEPT);
    world.systemic.incidents.drain(..excess);
}

fn closed(world: &World, key: LaneKey) -> bool {
    world.logistics.closures.iter().any(|c| c.covers(key))
}

/// Draw this week's incidents and put their effects in force from next week's opening.
pub fn weekly(world: &mut World, ds: &Dataset) {
    let week = world.tick.0;
    if week == 0 {
        return;
    }
    let mut rng = Rng::entity_stream(world.seed, TAG_SYSTEMIC, Handle(week as u64));
    let draw = |rng: &mut Rng, cbps: u64| rng.uniform_below(1_000_000) < cbps;

    // Pipeline spills and lane outages, lane by lane in key order.
    let pipeline = ds.logistics.mode_index("pipeline");
    let sea = ds.logistics.mode_index("sea");
    let lanes: Vec<LaneKey> = world
        .logistics
        .lanes
        .iter()
        .filter(|(_, lane)| lane.cap() > 0)
        .map(|(key, _)| *key)
        .collect();
    for key in lanes {
        if closed(world, key) {
            continue;
        }
        if Some(key.mode) == pipeline {
            if draw(&mut rng, SPILL_CBPS) {
                let weeks = rng.uniform_inclusive(3, 8) as u32;
                spill(world, ds, key, weeks);
            }
        } else if Some(key.mode) != sea && draw(&mut rng, OUTAGE_CBPS) {
            let weeks = rng.uniform_inclusive(1, 4) as u32;
            let cap_bps = if rng.chance_bps(5_000) { 0 } else { 2_500 };
            outage(world, ds, key, weeks, cap_bps);
        }
    }

    // Port strikes: ports are the regions with sea access.
    if let Some(sea) = sea {
        let union_law = world.politics.laws.unions;
        let chance = match union_law {
            UnionLaw::Strong => STRIKE_CBPS * 2,
            UnionLaw::Standard => STRIKE_CBPS,
            UnionLaw::Weak => STRIKE_CBPS / 2,
        };
        let ports: Vec<u8> = (0..ds.regions.len() as u8)
            .filter(|r| {
                world
                    .logistics
                    .access
                    .get(sea as usize)
                    .and_then(|row| row.get(*r as usize))
                    .copied()
                    .unwrap_or(false)
            })
            .collect();
        for port in ports {
            let key = LaneKey::new(port, port, sea);
            if closed(world, key) || !draw(&mut rng, chance) {
                continue;
            }
            let weeks = rng.uniform_inclusive(2, 5) as u32;
            strike(world, ds, port, sea, weeks);
        }
    }

    // Refinery fires: processing plants that make by-products and are running.
    let plants: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, plant)| {
            let recipe = &ds.recipes[plant.recipe as usize];
            plant.status == FacilityStatus::Operating
                && !recipe.by_products.is_empty()
                && recipe.industry != "extraction"
                && plant.outage.is_none_or(|(_, until)| until.0 <= week)
        })
        .map(|(handle, _)| handle)
        .collect();
    for plant in plants {
        if draw(&mut rng, FIRE_CBPS) {
            let weeks = rng.uniform_inclusive(8, 16) as u32;
            fire(world, ds, plant, weeks);
        }
    }

    // Harvest failure in the harvest quarter's closing week.
    if world.tick.quarter() == 3 && week.is_multiple_of(13) {
        for region in 0..world.regions.len() as u8 {
            let farms = farm_firms(world, ds, region);
            if farms.is_empty() || !rng.chance_bps(HARVEST_BPS) {
                continue;
            }
            let loss = rng.uniform_inclusive(2_500, 4_000) as i32;
            harvest(world, ds, region, loss, farms);
        }
    }
}

/// Make one systemic event happen now, at the first place it can, for a timeline. Returns whether
/// anything was struck. Durations are the middle of each kind's usual range, and the event is
/// recorded like a random one. The news line is the timeline event's own, so this adds none.
pub fn force(world: &mut World, ds: &Dataset, event: &str) -> bool {
    let pipeline = ds.logistics.mode_index("pipeline");
    let sea = ds.logistics.mode_index("sea");
    match event {
        "spill" | "lane_outage" => {
            let key = world
                .logistics
                .lanes
                .iter()
                .filter(|(key, lane)| {
                    lane.cap() > 0
                        && !closed(world, **key)
                        && if event == "spill" {
                            Some(key.mode) == pipeline
                        } else {
                            Some(key.mode) != pipeline && Some(key.mode) != sea
                        }
                })
                .map(|(key, _)| *key)
                .next();
            let Some(key) = key else { return false };
            if event == "spill" {
                spill(world, ds, key, 5);
            } else {
                outage(world, ds, key, 2, 0);
            }
            true
        }
        "port_strike" => {
            let Some(sea) = sea else { return false };
            let port = (0..ds.regions.len() as u8).find(|r| {
                world
                    .logistics
                    .access
                    .get(sea as usize)
                    .and_then(|row| row.get(*r as usize))
                    .copied()
                    .unwrap_or(false)
                    && !closed(world, LaneKey::new(*r, *r, sea))
            });
            let Some(port) = port else { return false };
            strike(world, ds, port, sea, 3);
            true
        }
        "refinery_fire" => {
            let week = world.tick.0;
            let plant = world
                .facilities
                .iter()
                .find(|(_, plant)| {
                    let recipe = &ds.recipes[plant.recipe as usize];
                    plant.status == FacilityStatus::Operating
                        && !recipe.by_products.is_empty()
                        && recipe.industry != "extraction"
                        && plant.outage.is_none_or(|(_, until)| until.0 <= week)
                })
                .map(|(handle, _)| handle);
            let Some(plant) = plant else { return false };
            fire(world, ds, plant, 12);
            true
        }
        "harvest_failure" => {
            let found = (0..world.regions.len() as u8)
                .map(|region| (region, farm_firms(world, ds, region)))
                .find(|(_, farms)| !farms.is_empty());
            let Some((region, farms)) = found else {
                return false;
            };
            harvest(world, ds, region, 3_000, farms);
            true
        }
        _ => false,
    }
}

fn spill(world: &mut World, ds: &Dataset, key: LaneKey, weeks: u32) {
    let firms = lane_firms(world, key);
    world.logistics.closures.push(Closure {
        a: key.a,
        b: key.b,
        mode: key.mode,
        cap_bps: 0,
        until: world.tick.0 + 1 + weeks,
    });
    // Operators pay to clean up: two weeks of the lane's revenue at the empty-lane rate.
    let carriers: Vec<(Handle, i32)> = world
        .logistics
        .lanes
        .get(&key)
        .map(|lane| {
            lane.allocs
                .iter()
                .map(|a| (world.logistics.carriers[a.carrier as usize].firm, a.cap))
                .collect()
        })
        .unwrap_or_default();
    for (firm, cap) in carriers {
        let bill = crate::economy::logistics::leg_cents(
            ds,
            key.a,
            key.b,
            key.mode,
            cap as i64 * 2,
            world.logistics.at_price_level(ds.logistics.empty_lane_bps),
        );
        crate::economy::events_engine::pay_service(world, firm, bill, "pipeline spill clean-up");
    }
    let text = format!(
        "Pipeline spill on {}: shut for {} weeks.",
        lane_text(ds, key),
        weeks
    );
    record(
        world,
        Incident {
            week: world.tick.0,
            kind: SPILL,
            a: key.a,
            b: key.b,
            mode: key.mode,
            weeks: weeks as u16,
            firms,
            text,
        },
    );
}

fn outage(world: &mut World, ds: &Dataset, key: LaneKey, weeks: u32, cap_bps: i32) {
    let firms = lane_firms(world, key);
    world.logistics.closures.push(Closure {
        a: key.a,
        b: key.b,
        mode: key.mode,
        cap_bps,
        until: world.tick.0 + 1 + weeks,
    });
    let text = format!(
        "{} {} for {} weeks.",
        if cap_bps == 0 {
            "Lane closed:"
        } else {
            "Lane cut to a quarter:"
        },
        lane_text(ds, key),
        weeks
    );
    record(
        world,
        Incident {
            week: world.tick.0,
            kind: LANE_OUTAGE,
            a: key.a,
            b: key.b,
            mode: key.mode,
            weeks: weeks as u16,
            firms,
            text,
        },
    );
}

fn strike(world: &mut World, ds: &Dataset, port: u8, sea: u8, weeks: u32) {
    let mut firms: Vec<Handle> = Vec::new();
    for (key, stat) in &world.logistics.stats {
        if key.mode == sea && (key.a == port || key.b == port) {
            for (firm, last) in &stat.users {
                if world.tick.0.saturating_sub(*last) <= 8 {
                    firms.push(*firm);
                }
            }
        }
    }
    for (key, lane) in &world.logistics.lanes {
        if key.mode == sea && (key.a == port || key.b == port) {
            for alloc in &lane.allocs {
                firms.push(world.logistics.carriers[alloc.carrier as usize].firm);
            }
        }
    }
    firms.sort_unstable();
    firms.dedup();
    world.logistics.closures.push(Closure {
        a: port,
        b: port,
        mode: sea,
        cap_bps: 0,
        until: world.tick.0 + 1 + weeks,
    });
    let text = format!(
        "Port strike at {}: ships stop for {} weeks.",
        city(ds, port),
        weeks
    );
    record(
        world,
        Incident {
            week: world.tick.0,
            kind: PORT_STRIKE,
            a: port,
            b: port,
            mode: sea,
            weeks: weeks as u16,
            firms,
            text,
        },
    );
}

fn fire(world: &mut World, ds: &Dataset, plant: Handle, weeks: u32) {
    let (firm, region, recipe) = match world.facilities.get(plant) {
        Some(p) => (p.firm, p.region, p.recipe),
        None => return,
    };
    let until = Tick(world.tick.0 + 1 + weeks);
    if let Some(p) = world.facilities.get_mut(plant) {
        p.outage = Some((0, until));
    }
    // A tenth of what the firm holds of the plant's products in its region burns.
    let products: Vec<u16> = ds.recipes[recipe as usize]
        .outputs()
        .filter_map(|(id, _)| ds.product_index.get(id).copied())
        .collect();
    let mut lost = 0i64;
    if let Some(entity) = world.firms.get_mut(firm) {
        for product in products {
            let held = entity.stock(product, region);
            let burn = cw_core::quantity::MilliLots(held.0 / 10);
            if burn.0 > 0 {
                lost += entity.issue_stock(product, region, burn).0;
            }
        }
        if lost > 0 {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Inventory, lost, "fire loss")
                    .debit(Account::OperatingExpense, lost, "fire loss"),
            );
        }
    }
    let name = world
        .firms
        .get(firm)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    let text = format!(
        "Fire at {}'s plant in {}: shut for {} weeks.",
        name,
        city(ds, region),
        weeks
    );
    record(
        world,
        Incident {
            week: world.tick.0,
            kind: REFINERY_FIRE,
            a: region,
            b: region,
            mode: 0,
            weeks: weeks as u16,
            firms: vec![firm],
            text,
        },
    );
}

fn farm_firms(world: &World, ds: &Dataset, region: u8) -> Vec<Handle> {
    let mut firms: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, p)| {
            p.region == region
                && p.status == FacilityStatus::Operating
                && ds.recipes[p.recipe as usize].industry == "agriculture"
        })
        .map(|(_, p)| p.firm)
        .collect();
    firms.sort_unstable();
    firms.dedup();
    firms
}

fn harvest(world: &mut World, ds: &Dataset, region: u8, loss_bps: i32, firms: Vec<Handle>) {
    let weeks = 26u32;
    let until = Tick(world.tick.0 + weeks);
    let crops: Vec<u16> = {
        let mut v: Vec<u16> = ds
            .recipes
            .iter()
            .filter(|r| r.industry == "agriculture")
            .filter_map(|r| ds.product_index.get(&r.output).copied())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    for crop in crops {
        world.regions[region as usize].supply_modifiers.push((
            "harvest_failure".to_owned(),
            Some(crop),
            -loss_bps,
            until,
        ));
    }
    let text = format!(
        "Harvest failure in {}: farm yields down {}% for {} weeks.",
        city(ds, region),
        loss_bps / 100,
        weeks
    );
    record(
        world,
        Incident {
            week: world.tick.0,
            kind: HARVEST_FAILURE,
            a: region,
            b: region,
            mode: 0,
            weeks: weeks as u16,
            firms,
            text,
        },
    );
}

/// Incidents in the last `weeks` weeks that touch the given firm.
pub fn touching(world: &World, firm: Handle, weeks: u32) -> impl Iterator<Item = &Incident> {
    let now = world.tick.0;
    world
        .systemic
        .incidents
        .iter()
        .filter(move |i| now.saturating_sub(i.week) <= weeks && i.firms.contains(&firm))
}
