//! Weekly systems history for the deeper displays: each good's market, each region, the
//! player's divisions and freight, and the economy at large. Nothing in the rules reads it.
//!
//! Like the statements archive it lives outside the world's postcard encoding and hash, is
//! saved in its own trailer and is bounded: a save from before it loads with it empty and the
//! charts start from the next week. Only read-only measures of the closed week are kept; the
//! figures are the same ones the boards show, so a chart's last point is the board's number.

use cw_core::ids::Handle;
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::state::{FacilityStatus, FirmStatus, World};

/// Weeks of systems history kept: five years.
pub const SYSTEMS_WEEKS: usize = 260;

/// One good's corridor market in one week. Lots are whole lots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoodWeek {
    pub product: u16,
    pub made: i64,
    pub sold: i64,
    pub imported: i64,
    pub exported: i64,
    /// Every firm's stock of the good, buyers' input cover included.
    pub stock: i64,
    /// Sellers' stock beyond their own plants' input cover and goods held back on purpose.
    pub unsold: i64,
    /// The sales-weighted corridor ask.
    pub ask_cents: i64,
    /// Lots the player's companies sold.
    pub your_sold: i64,
}

/// One region in one week.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionWeek {
    pub region: u8,
    pub population: i64,
    pub employed: i64,
    /// Prevailing wage averaged over the three tiers, weighted by workers present.
    pub wage_cents: i64,
    pub land_index_bps: i32,
    pub demand_index_bps: i32,
    /// Household spending this week, all goods.
    pub spend_cents: i64,
}

/// One of the player's divisions in one week. Milli-lots for physical flows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionWeek {
    pub firm: Handle,
    pub region: u8,
    pub industry: u8,
    pub made_milli: i64,
    pub sold_milli: i64,
    /// What the operating plants make of the primary product at full run.
    pub capacity_milli: i64,
    pub revenue_cents: i64,
    pub costs_cents: i64,
    pub share_bps: i64,
}

/// One of the player's companies' freight in one week.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreightWeek {
    pub firm: Handle,
    pub lots: i64,
    pub own_lots: i64,
    pub other_lots: i64,
    pub outside_lots: i64,
    pub cents: i64,
    /// Lots of lane capacity the company's own carriers offered, and how many were used or held.
    pub carrier_cap: i64,
    pub carrier_used: i64,
}

/// The economy in one week.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EconomyWeek {
    pub policy_rate_bps: i32,
    pub inflation_bps: i32,
    pub price_index_bps: i64,
    pub firms_operating: u32,
    /// Firms dissolved or merged away so far.
    pub firms_gone: u32,
    /// Raids launched this week and how many succeeded.
    pub raids: u32,
    pub raids_won: u32,
    pub household_spend_cents: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemsWeek {
    pub week: u32,
    pub goods: Vec<GoodWeek>,
    pub regions: Vec<RegionWeek>,
    pub divisions: Vec<DivisionWeek>,
    pub freight: Vec<FreightWeek>,
    pub economy: EconomyWeek,
}

/// The weekly systems history, oldest first, at most `SYSTEMS_WEEKS`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemsHistory {
    pub weeks: Vec<SystemsWeek>,
}

impl SystemsHistory {
    pub fn is_empty(&self) -> bool {
        self.weeks.is_empty()
    }
}

/// Lots of every good sold by one firm in the closed week.
fn firm_sold(world: &World, firm: Handle, product: u16) -> i64 {
    world
        .firms
        .get(firm)
        .and_then(|f| f.last_closed_sales())
        .map(|week| {
            week.iter()
                .filter(|((_, p), _)| *p == product)
                .map(|(_, q)| *q)
                .sum()
        })
        .unwrap_or(0)
}

/// The week's market figures for one good, as the commodity board reads them.
pub fn good_week(world: &World, dataset: &Dataset, code: u16) -> GoodWeek {
    let product = &dataset.products[code as usize];
    let made: i64 = world
        .facilities
        .iter()
        .map(|(_, f)| f.last_made(dataset, &product.id).lots())
        .sum();
    let mut sold = 0i64;
    let mut your_sold = 0i64;
    let mut unsold = 0i64;
    for (handle, _) in world.firms.iter() {
        let lots = firm_sold(world, handle, code);
        sold += lots;
        if crate::economy::events_engine::player_side(world, handle) {
            your_sold += lots;
        }
        if lots > 0 || crate::economy::pricing::outputs(world, dataset, handle).contains(&code) {
            unsold += crate::economy::pricing::free_stock(world, dataset, handle, code);
        }
    }
    let (imported, exported) = world
        .gateways
        .iter()
        .filter(|g| g.product == code)
        .fold((0, 0), |(i, e), g| {
            (i + g.last_import_lots, e + g.last_export_lots)
        });
    let stock = world
        .firms
        .iter()
        .flat_map(|(_, f)| f.inventory.iter())
        .filter(|((p, _), _)| *p == code)
        .map(|(_, lot)| lot.qty.lots())
        .sum();
    GoodWeek {
        product: code,
        made,
        sold,
        imported,
        exported,
        stock,
        unsold,
        ask_cents: crate::economy::pricing::corridor_price(world, dataset, code),
        your_sold,
    }
}

/// The goods the history follows: every priced, non-financial good.
pub fn tracked_goods(dataset: &Dataset) -> impl Iterator<Item = u16> + '_ {
    (0..dataset.products.len() as u16).filter(|code| {
        let p = &dataset.products[*code as usize];
        p.reference_price_cents > 0 && !p.kind.is_financial()
    })
}

pub fn region_week(world: &World, index: u8) -> RegionWeek {
    let r = &world.regions[index as usize];
    let population: i64 = r.labour.iter().map(|t| t.population).sum();
    let employed: i64 = r.labour.iter().map(|t| t.employed).sum();
    let weighted: i128 = r
        .labour
        .iter()
        .map(|t| t.prevailing_wage_cents as i128 * t.population.max(0) as i128)
        .sum();
    RegionWeek {
        region: index,
        population,
        employed,
        wage_cents: if population > 0 {
            (weighted / population as i128) as i64
        } else {
            0
        },
        land_index_bps: r.land_index_bps,
        demand_index_bps: r.demand_index_bps,
        spend_cents: r.households.spend_cents.values().sum(),
    }
}

/// Full-run weekly output of a division's primary product, in milli-lots, as the division
/// report measures utilisation. A good harvest can make more than this.
pub fn division_capacity_milli(
    world: &World,
    dataset: &Dataset,
    division: &crate::economy::sectors::Division,
) -> i64 {
    division
        .plants
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .filter(|p| p.status == FacilityStatus::Operating)
        .map(|p| p.full_weekly_output(dataset).0)
        .sum()
}

fn division_weeks(world: &World, dataset: &Dataset, firm: Handle) -> Vec<DivisionWeek> {
    crate::economy::sectors::divisions_of(world, dataset, firm)
        .into_iter()
        .map(|d| {
            let report =
                crate::economy::division_report::report(world, dataset, firm, d.region, d.industry);
            let (made, sold, revenue, profit) = report
                .as_ref()
                .map(|r| {
                    let primary = r.outputs.iter().find(|o| o.product == r.primary_product);
                    (
                        primary.map(|o| o.made_milli).unwrap_or(0),
                        primary.map(|o| o.sold_milli).unwrap_or(0),
                        r.revenue_cents,
                        r.operating_profit_cents,
                    )
                })
                .unwrap_or((0, 0, 0, 0));
            DivisionWeek {
                firm,
                region: d.region,
                industry: d.industry,
                made_milli: made,
                sold_milli: sold,
                capacity_milli: division_capacity_milli(world, dataset, &d),
                revenue_cents: revenue,
                costs_cents: revenue.saturating_sub(profit),
                share_bps: crate::economy::sectors::share_bps(
                    world, dataset, firm, d.region, d.industry, 0,
                ),
            }
        })
        .collect()
}

/// One firm's freight this week: shipments by who carried them, and its own carriers' lane
/// capacity against what was used or held on it.
pub fn freight_week(world: &World, firm: Handle) -> FreightWeek {
    let st = &world.logistics;
    let mut out = FreightWeek {
        firm,
        ..Default::default()
    };
    for ((owner, _), d) in st.dest_week.iter() {
        if *owner != firm {
            continue;
        }
        out.lots += d.lots;
        out.own_lots += d.own_lots;
        out.other_lots += d.other_lots;
        out.outside_lots += d.outside_lots;
        out.cents += d.cents;
    }
    for lane in st.lanes.values() {
        for a in &lane.allocs {
            if st
                .carriers
                .get(a.carrier as usize)
                .is_some_and(|c| c.firm == firm)
            {
                out.carrier_cap += a.cap as i64;
                out.carrier_used += (a.used + a.held).min(a.cap) as i64;
            }
        }
    }
    out
}

pub fn economy_week(world: &World) -> EconomyWeek {
    let week = world.tick.0;
    let (mut operating, mut gone) = (0u32, 0u32);
    for (_, f) in world.firms.iter() {
        match f.status {
            FirmStatus::Dissolved | FirmStatus::Merged { .. } => gone += 1,
            s if s.operative() => operating += 1,
            _ => {}
        }
    }
    let raids = world.raids.iter().filter(|r| r.tick == week);
    let (raids, won) = raids.fold((0, 0), |(n, w), r| (n + 1, w + r.success as u32));
    EconomyWeek {
        policy_rate_bps: world.macro_state.policy_rate_bps,
        inflation_bps: world.macro_state.inflation_bps,
        price_index_bps: world.macro_state.price_index_bps,
        firms_operating: operating,
        firms_gone: gone,
        raids,
        raids_won: won,
        household_spend_cents: world
            .regions
            .iter()
            .map(|r| r.households.spend_cents.values().sum::<i64>())
            .sum(),
    }
}

/// Record this week. Runs at the end of the tick, after the statements archive.
pub fn record(world: &mut World, dataset: &Dataset) {
    let week = world.tick.0;
    if world.systems.weeks.last().is_some_and(|w| w.week >= week) {
        return;
    }
    let goods = tracked_goods(dataset)
        .map(|code| good_week(world, dataset, code))
        .filter(|g| g.made | g.sold | g.imported | g.exported | g.stock != 0)
        .collect();
    let regions = (0..world.regions.len() as u8)
        .map(|i| region_week(world, i))
        .collect();
    let players: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(h, f)| {
            f.status.operative() && crate::economy::events_engine::player_side(world, *h)
        })
        .map(|(h, _)| h)
        .collect();
    let divisions = players
        .iter()
        .flat_map(|h| division_weeks(world, dataset, *h))
        .collect();
    let freight = players.iter().map(|h| freight_week(world, *h)).collect();
    let entry = SystemsWeek {
        week,
        goods,
        regions,
        divisions,
        freight,
        economy: economy_week(world),
    };
    let weeks = &mut world.systems.weeks;
    weeks.push(entry);
    let excess = weeks.len().saturating_sub(SYSTEMS_WEEKS);
    weeks.drain(..excess);
}
