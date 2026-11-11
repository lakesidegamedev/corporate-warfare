//! The `steward` play style (2.0): a careful owner of an established company, built for helm
//! starts. Where `skilled` grows a one-plant founder one plant at a time, the steward uses the
//! balance sheet of a large company:
//!
//! * keeps what earns and cuts what loses: the newest plant of a division that lost money over
//!   the last half year is mothballed (one per division per half year, feeders with no sales are
//!   judged with the company), and the place is never built again;
//! * sells what it closed: a plant mothballed for half a year is listed through `ListPlant`, and
//!   the best bid put to it is accepted;
//! * invests the company's free cash (cash above 26 weeks of mandatory outflow) in the engine's
//!   best openings at the company's scale: up to eight blocks a plant, up to three projects
//!   under construction at once, each repaying its cost from half the engine's weekly figure
//!   within two years, quickest payback first; it never borrows to build;
//! * bids in break-up auctions and plant listings with the careful script's test, defends raids
//!   with marketing, and keeps control (refuses tender offers, never sells its own shares).

use std::collections::{BTreeMap, BTreeSet};

use cw_core::ids::Handle;
use cw_session::Session;
use cw_sim::commands::CommandIntent;
use cw_sim::state::FacilityStatus;

use super::{PolicyRunner, controlled_firms, finance};

const RESERVE_WEEKS: i64 = 26;
const MAX_PAYBACK_WEEKS: i64 = 104;
/// The share of the engine's opening figure the steward believes.
const TRUST_BPS: i64 = 5_000;
const PROVING_WEEKS: u32 = 13;
const MAX_BUILDING: usize = 3;
const MAX_BLOCKS: i64 = 8;
const LIST_AFTER_WEEKS: u32 = 26;
const CUT_EVERY_WEEKS: u32 = 26;

/// What the steward remembers between cadences.
#[derive(Clone, Debug, Default)]
pub(super) struct StewardState {
    /// (recipe, region) places it closed a plant in: never built again.
    failed_places: BTreeSet<(u16, u8)>,
    /// Last week a plant of each (region, industry) division was cut.
    last_cut: BTreeMap<(u8, u8), u32>,
    pub builds: u32,
    pub cuts: u32,
    pub listed: u32,
    pub accepted: u32,
}

impl PolicyRunner {
    pub(super) fn act_steward(&mut self, session: &mut Session) {
        let controlled = controlled_firms(session);
        for firm in &controlled {
            self.skilled_operate(session, *firm);
        }
        let Some(primary) = session.world.investor.primary_group else {
            return;
        };
        let firm = primary.0;
        self.skilled_defend(session, firm);
        self.steward_prune(session, firm);
        self.steward_sell(session, firm);
        self.steward_invest(session, firm);
        if !crate::policy::NO_PLANT_BIDS.load(std::sync::atomic::Ordering::Relaxed) {
            self.skilled_bid(session, firm);
        }
    }

    /// Mothball the newest plant of each division that lost money over the last half year.
    fn steward_prune(&mut self, session: &mut Session, firm: u64) {
        use cw_sim::economy::sectors;
        let now = session.world.tick.0;
        let mut cuts: Vec<((u8, u8), Handle)> = Vec::new();
        for division in sectors::divisions_of(&session.world, &session.dataset, Handle(firm)) {
            let key = (division.region, division.industry);
            if self
                .steward
                .last_cut
                .get(&key)
                .is_some_and(|w| now < w + CUT_EVERY_WEEKS)
            {
                continue;
            }
            let operating: Vec<(u32, Handle)> = division
                .plants
                .iter()
                .filter_map(|h| {
                    session
                        .world
                        .facilities
                        .get(*h)
                        .filter(|p| p.is_operating())
                        .map(|p| (p.completes.0, *h))
                })
                .collect();
            if operating.is_empty() || operating.iter().any(|(done, _)| now < done + PROVING_WEEKS)
            {
                continue;
            }
            let Some((profit, revenue)) =
                self.skilled_division_profit(session, firm, key, now.saturating_sub(26))
            else {
                continue;
            };
            if revenue <= 0 || profit >= 0 {
                continue;
            }
            if let Some((_, newest)) = operating.iter().max() {
                cuts.push((key, *newest));
            }
        }
        let operating_total = session.world.firm(Handle(firm)).map_or(0, |f| {
            f.facilities
                .iter()
                .filter(|h| {
                    session
                        .world
                        .facilities
                        .get(**h)
                        .is_some_and(|p| p.is_operating())
                })
                .count()
        });
        // Never close the last two plants.
        let allowed = operating_total.saturating_sub(2);
        for (key, plant) in cuts.into_iter().take(allowed) {
            let place = session
                .world
                .facilities
                .get(plant)
                .map(|p| (p.recipe, p.region));
            if self.submit(session, CommandIntent::MothballFacility { facility: plant }) {
                self.steward.cuts += 1;
                self.steward.last_cut.insert(key, now);
                if let Some(place) = place {
                    self.steward.failed_places.insert(place);
                }
            }
        }
    }

    /// List plants mothballed for half a year; accept the best bid put to a listing.
    fn steward_sell(&mut self, session: &mut Session, firm: u64) {
        let now = session.world.tick.0;
        let handle = Handle(firm);
        let listed: BTreeSet<u64> = session
            .world
            .auctions
            .lots
            .iter()
            .map(|l| l.facility.0)
            .collect();
        let answer: Vec<u32> = session
            .world
            .auctions
            .lots
            .iter()
            .filter(|l| l.seller == handle && !l.offered.is_empty())
            .map(|l| l.id)
            .collect();
        for lot in answer {
            if self.submit(session, CommandIntent::AcceptPlantBid { lot }) {
                self.steward.accepted += 1;
            }
        }
        let idle: Vec<Handle> = session
            .world
            .firm(handle)
            .map(|f| {
                f.facilities
                    .iter()
                    .copied()
                    .filter(|h| !listed.contains(&h.0))
                    .filter(|h| {
                        session.world.facilities.get(*h).is_some_and(|p| {
                            p.status == FacilityStatus::Mothballed
                                && p.mothballed_since
                                    .is_some_and(|t| now >= t.0 + LIST_AFTER_WEEKS)
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        for facility in idle {
            if self.submit(session, CommandIntent::ListPlant { facility }) {
                self.steward.listed += 1;
            }
        }
    }

    /// Spend free cash on the best openings at the company's scale, several at a time.
    fn steward_invest(&mut self, session: &mut Session, firm: u64) {
        use cw_sim::economy::sectors;
        let now = session.world.tick.0;
        let handle = Handle(firm);
        if session.world.pending_pause.is_some() {
            return;
        }
        let Some(entity) = session.world.firm(handle) else {
            return;
        };
        let mut building = entity
            .facilities
            .iter()
            .filter(|h| {
                session
                    .world
                    .facilities
                    .get(**h)
                    .is_some_and(|p| p.status == FacilityStatus::Building)
            })
            .count();
        if building >= MAX_BUILDING {
            return;
        }
        let fin = finance(session, firm);
        let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, handle);
        let mut free = fin.cash_cents - outflow.saturating_mul(RESERVE_WEEKS);
        if free <= 0 {
            return;
        }
        let openings = session.capacity_openings(80);
        let mut ranked: Vec<(i64, u16, u8, i64, i64)> = Vec::new();
        for open in &openings {
            let Some(&recipe) = session.dataset.recipe_index.get(&open.recipe_id) else {
                continue;
            };
            if self.steward.failed_places.contains(&(recipe, open.region)) {
                continue;
            }
            let industry = sectors::recipe_industry(&session.dataset, recipe);
            if sectors::division_strategy(
                &session.world,
                &session.dataset,
                handle,
                open.region,
                industry,
            )
            .is_some_and(|strategy| strategy != recipe)
            {
                continue;
            }
            if self
                .skilled_division_profit(
                    session,
                    firm,
                    (open.region, industry),
                    now.saturating_sub(26),
                )
                .is_some_and(|(p, r)| r > 0 && p < 0)
            {
                continue;
            }
            let blocks = (open.room_lots * 4 / 5 / open.block_lots.max(1)).clamp(1, MAX_BLOCKS);
            let cost = sectors::build_cost(
                &session.world,
                &session.dataset,
                handle,
                open.region,
                recipe,
                blocks as _,
            )
            .total_cents();
            let weekly = open.weekly_contribution_cents * blocks * TRUST_BPS / 10_000;
            if weekly <= 0 || cost <= 0 || cost > weekly * MAX_PAYBACK_WEEKS {
                continue;
            }
            ranked.push((weekly * 10_000 / cost, recipe, open.region, blocks, cost));
        }
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut used: BTreeSet<(u8, u8)> = BTreeSet::new();
        for (_, recipe, region, blocks, cost) in ranked {
            if building >= MAX_BUILDING {
                break;
            }
            let industry = sectors::recipe_industry(&session.dataset, recipe);
            if cost > free || !used.insert((region, industry)) {
                continue;
            }
            if self.submit(
                session,
                CommandIntent::BuildFacility {
                    firm: handle,
                    region,
                    recipe,
                    blocks: blocks as _,
                },
            ) {
                self.steward.builds += 1;
                building += 1;
                free -= cost;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ReferencePolicy;

    #[test]
    fn steward_is_a_named_style_that_keeps_control() {
        let p = ReferencePolicy::parse("steward").unwrap();
        assert_eq!(p, ReferencePolicy::Steward);
        assert_eq!(p.key(), "steward");
        // Shares the careful script's answers to cards, so it refuses tender offers.
        assert!(p.is_skilled());
    }
}
