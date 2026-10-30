//! Read-only views for the deeper systems displays: a good's market depth, a region's dossier,
//! a division's operations, a company's freight, the rivals board and the economy's history.
//!
//! Series come from the engine's weekly systems history (`cw_sim::economy::systems`), so a save
//! from before it starts its charts from the next week. Derived numbers (shares, cover, unit
//! costs) are computed here with checked integer arithmetic so every client shows the same.

use cw_core::ids::Handle;
use cw_sim::economy::systems::{self, SystemsWeek};
use cw_sim::state::{INDUSTRY_NAMES, World};

use crate::SessionHandle;

/// One named weekly series for a chart: `values[i]` is the figure in game week `weeks[i]`.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct SeriesDto {
    pub key: String,
    pub weeks: Vec<u32>,
    pub values: Vec<i64>,
}

/// One company's part of a market: lots and their share of the total.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ShareRowDto {
    pub firm: u64,
    pub name: String,
    pub lots: i64,
    pub share_bps: i64,
    pub yours: bool,
}

/// One reason a price is moving, in plain words. `push` is +1 when it lifts the price, -1 when
/// it lowers it and 0 when it explains without pushing.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DriverDto {
    pub key: String,
    pub text: String,
    pub push: i32,
}

/// A good's corridor market this week and over time.
#[derive(uniffi::Record)]
pub struct GoodDepthDto {
    pub product: u32,
    pub name: String,
    pub service: bool,
    pub reference_cents: i64,
    pub ask_cents: i64,
    pub made_lots: i64,
    pub sold_lots: i64,
    pub imported_lots: i64,
    pub exported_lots: i64,
    pub stock_lots: i64,
    pub unsold_lots: i64,
    /// Weeks of sales the stock covers, in hundredths; absent when nothing sold.
    pub cover_x100: Option<i64>,
    pub your_share_bps: i64,
    pub drivers: Vec<DriverDto>,
    pub sellers: Vec<ShareRowDto>,
    /// Weekly use by plants that consume it, and households' purchases.
    pub buyers: Vec<ShareRowDto>,
    /// made, sold, imported, exported, stock, cover_x100, ask, your_share_bps.
    pub series: Vec<SeriesDto>,
}

/// One good households buy in a region.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DemandDto {
    pub product: u32,
    pub name: String,
    pub spend_cents: i64,
    pub share_bps: i64,
}

/// One company's division in a region, for the competition table.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CompetitorDto {
    pub firm: u64,
    pub name: String,
    pub industry: String,
    pub blocks: u32,
    pub share_bps: i64,
    pub yours: bool,
    /// Estimated weekly operating profit; only for your own divisions, else zero.
    pub profit_cents: i64,
    pub revenue_cents: i64,
}

/// One labour tier in a region.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct LabourPoolDto {
    pub population: i64,
    pub employed: i64,
    pub idle: i64,
    pub wage_cents: i64,
}

/// A region's dossier.
#[derive(uniffi::Record)]
pub struct RegionDossierDto {
    pub index: u32,
    pub name: String,
    pub population: i64,
    pub employed: i64,
    pub wage_cents: i64,
    pub land_index_bps: i32,
    pub demand_index_bps: i32,
    pub spend_cents: i64,
    pub tiers: Vec<LabourPoolDto>,
    pub demand: Vec<DemandDto>,
    pub competitors: Vec<CompetitorDto>,
    /// Workers who moved in minus those who moved out, over the last 52 weeks and the last week.
    pub net_migration_year: i64,
    pub net_migration_week: i64,
    /// population, employed, wage, land_index, spend, migration (net movers a week), then
    /// `share:<industry>` for each of your divisions here.
    pub series: Vec<SeriesDto>,
}

/// One part of a division's cost per lot of its main product.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct UnitCostDto {
    pub key: String,
    pub cents: i64,
}

/// A division's operations: how hard it runs and what each lot costs against its price.
#[derive(uniffi::Record)]
pub struct DivisionOpsDto {
    pub made_milli: i64,
    pub sold_milli: i64,
    pub capacity_milli: i64,
    pub utilisation_bps: i64,
    /// Cost per lot of the main product by part; the parts sum to `unit_cost_cents`.
    pub unit_costs: Vec<UnitCostDto>,
    pub unit_cost_cents: i64,
    pub price_cents: i64,
    pub margin_per_lot_cents: i64,
    /// made, sold, capacity (milli-lots), utilisation_bps, unit_cost, revenue, costs, share_bps.
    pub series: Vec<SeriesDto>,
}

/// A company's freight this week and over time.
#[derive(uniffi::Record)]
pub struct LogisticsStatsDto {
    pub lots: i64,
    pub own_lots: i64,
    pub other_lots: i64,
    pub outside_lots: i64,
    pub cents: i64,
    pub cents_per_lot: i64,
    pub carrier_cap: i64,
    pub carrier_used: i64,
    pub carrier_idle: i64,
    pub carrier_fill_bps: i64,
    /// lots, own, other, outside, cents, cents_per_lot, carrier_cap, carrier_used, fill_bps.
    pub series: Vec<SeriesDto>,
}

/// One company on the rivals board.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RivalRowDto {
    pub firm: u64,
    pub name: String,
    pub industry: String,
    pub yours: bool,
    /// Revenue and net income over the last closed weeks (up to thirteen). A bank's or
    /// insurer's revenue includes its interest income alongside premiums and fees.
    pub revenue_cents: i64,
    pub profit_cents: i64,
    pub cash_cents: i64,
    pub debt_cents: i64,
    pub share_price_cents: i64,
    pub market_cap_cents: i64,
    pub divisions: u32,
    /// Net income over revenue across the same weeks; absent when there was no revenue.
    pub net_margin_bps: Option<i64>,
}

/// One good both companies sold last week.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct HeadToHeadDto {
    pub product: u32,
    pub name: String,
    pub your_lots: i64,
    pub their_lots: i64,
    pub your_share_bps: i64,
    pub their_share_bps: i64,
}

/// `part / whole` in basis points, zero when the whole is not positive.
pub fn share_bps(part: i64, whole: i64) -> i64 {
    if whole <= 0 {
        return 0;
    }
    (part as i128 * 10_000 / whole as i128) as i64
}

/// Weeks of sales that a stock covers, in hundredths; none when nothing sold.
pub fn cover_x100(stock: i64, weekly_sales: i64) -> Option<i64> {
    (weekly_sales > 0).then(|| (stock as i128 * 100 / weekly_sales as i128) as i64)
}

/// Split weekly costs into a cost per lot, given the lots made in milli-lots. Each part is
/// floored and the leftover cents go to the parts with the largest remainders, so the parts
/// always add up to the total cost per lot exactly.
pub fn unit_costs(parts: &[(String, i64)], made_milli: i64) -> (Vec<UnitCostDto>, i64) {
    if made_milli <= 0 {
        return (
            parts
                .iter()
                .map(|(k, _)| UnitCostDto {
                    key: k.clone(),
                    cents: 0,
                })
                .collect(),
            0,
        );
    }
    let total_cents: i128 = parts.iter().map(|(_, c)| *c as i128).sum();
    let total = (total_cents * 1_000 / made_milli as i128) as i64;
    let mut rows: Vec<(usize, i64, i128)> = parts
        .iter()
        .enumerate()
        .map(|(i, (_, c))| {
            let scaled = *c as i128 * 1_000;
            let floor = scaled.div_euclid(made_milli as i128);
            (i, floor as i64, scaled.rem_euclid(made_milli as i128))
        })
        .collect();
    let mut left = total - rows.iter().map(|r| r.1).sum::<i64>();
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|a, b| rows[*b].2.cmp(&rows[*a].2).then(a.cmp(b)));
    let mut k = 0;
    while left > 0 && !order.is_empty() {
        rows[order[k % order.len()]].1 += 1;
        left -= 1;
        k += 1;
    }
    while left < 0 && !order.is_empty() {
        rows[order[order.len() - 1 - (k % order.len())]].1 -= 1;
        left += 1;
        k += 1;
    }
    (
        rows.iter()
            .map(|(i, cents, _)| UnitCostDto {
                key: parts[*i].0.clone(),
                cents: *cents,
            })
            .collect(),
        total,
    )
}

/// Shares of a whole, top `keep` by lots plus every row flagged as yours, largest first.
pub fn share_rows(mut rows: Vec<ShareRowDto>, keep: usize) -> Vec<ShareRowDto> {
    let total: i64 = rows.iter().map(|r| r.lots.max(0)).sum();
    rows.retain(|r| r.lots > 0);
    rows.sort_by(|a, b| b.lots.cmp(&a.lots).then(a.firm.cmp(&b.firm)));
    for r in &mut rows {
        r.share_bps = share_bps(r.lots, total);
    }
    let mut out: Vec<ShareRowDto> = Vec::new();
    for (i, r) in rows.into_iter().enumerate() {
        if i < keep || r.yours {
            out.push(r);
        }
    }
    out
}

/// Facts the price drivers read, all from the closed week.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriverFacts {
    pub service: bool,
    pub made: i64,
    pub sold: i64,
    pub imported: i64,
    pub unsold: i64,
    /// Buyers' stock held as input cover and their weekly use, in lots.
    pub buyer_stock: i64,
    pub buyer_use: i64,
    /// Weeks of input cover buyers aim for and weeks of sales sellers may hold.
    pub buyer_target_weeks: i64,
    pub glut_weeks: i64,
    /// Full-run capacity, for services.
    pub capacity: i64,
    pub ask: i64,
    pub reference: i64,
}

/// The week's price drivers in plain words, strongest first.
pub fn drivers(f: &DriverFacts) -> Vec<DriverDto> {
    let mut out = Vec::new();
    let d = |key: &str, text: String, push: i32| DriverDto {
        key: key.to_owned(),
        text,
        push,
    };
    if f.service {
        if f.capacity > 0 {
            let used = share_bps(f.sold, f.capacity);
            if used >= 9_500 {
                out.push(d(
                    "sold_out",
                    format!(
                        "Sold out: {}% of capacity used, so sellers ask more.",
                        used / 100
                    ),
                    1,
                ));
            } else if used < 8_500 {
                out.push(d(
                    "spare_capacity",
                    format!(
                        "Spare capacity: only {}% of capacity used, so sellers cut prices to fill it.",
                        used / 100
                    ),
                    -1,
                ));
            }
        }
    } else {
        if f.buyer_use > 0 {
            let held = cover_x100(f.buyer_stock, f.buyer_use).unwrap_or(0);
            if held < f.buyer_target_weeks * 100 {
                out.push(d(
                    "buyers_short",
                    format!(
                        "Buyers are short: they hold {}.{:02} weeks of what they use against {} wanted.",
                        held / 100,
                        held % 100,
                        f.buyer_target_weeks
                    ),
                    1,
                ));
            }
        }
        if let Some(weeks) = cover_x100(f.unsold, f.sold.max(1))
            && f.unsold > 0
            && weeks > f.glut_weeks * 100
        {
            out.push(d(
                "unsold_stock",
                format!(
                    "Unsold stock: sellers hold {} weeks of sales on the shelf, so prices fall.",
                    weeks / 100
                ),
                -1,
            ));
        }
        if f.imported > 0 && f.imported as i128 * 10_000 >= f.made.max(0) as i128 * 12_500 {
            let text = if f.made > 0 {
                format!(
                    "Import dependent: imports are {}% of home output, so the price sits near the import price.",
                    share_bps(f.imported, f.made) / 100
                )
            } else {
                "Import dependent: nobody makes it here, so the import price sets it.".to_owned()
            };
            out.push(d("imports", text, 1));
        }
    }
    if out.is_empty() {
        out.push(d(
            "balanced",
            "Balanced: buyers are covered and shelves are clear, so the price holds.".to_owned(),
            0,
        ));
    }
    if f.reference > 0 {
        let gap = share_bps(f.ask - f.reference, f.reference);
        let text = match gap {
            g if g > 0 => format!("The ask is {}.{}% over reference.", g / 100, (g % 100) / 10),
            g if g < 0 => format!(
                "The ask is {}.{}% under reference.",
                -g / 100,
                (-g % 100) / 10
            ),
            _ => "The ask is at reference.".to_owned(),
        };
        out.push(d("vs_reference", text, 0));
    }
    out
}

fn series<T>(
    weeks: &[SystemsWeek],
    key: &str,
    pick: impl Fn(&SystemsWeek) -> Option<T>,
) -> SeriesDto
where
    T: Into<i64>,
{
    let mut out = SeriesDto {
        key: key.to_owned(),
        weeks: Vec::new(),
        values: Vec::new(),
    };
    for w in weeks {
        if let Some(v) = pick(w) {
            out.weeks.push(w.week);
            out.values.push(v.into());
        }
    }
    out
}

/// Lots of each product a firm sold in the closed week.
fn sold_by_product(world: &World, h: Handle) -> std::collections::BTreeMap<u16, i64> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(week) = world.firms.get(h).and_then(|f| f.last_closed_sales()) {
        for ((_, p), q) in week {
            *out.entry(*p).or_insert(0) += *q;
        }
    }
    out
}

#[uniffi::export]
impl SessionHandle {
    /// A good's market depth: this week's flows, price drivers, sellers and buyers, and its
    /// weekly history.
    pub fn good_depth(&self, product: u32) -> Option<GoodDepthDto> {
        use cw_sim::economy::events_engine::player_side;
        use cw_sim::economy::pricing;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let code = u16::try_from(product).ok()?;
        let good = dataset.products.get(code as usize)?;
        let now = systems::good_week(world, dataset, code);
        let id = &good.id;
        let service = good.kind.is_service();
        let (mut sellers, mut buyers) = (Vec::new(), Vec::new());
        let (mut buyer_use, mut capacity, mut target_weeks, mut buyer_firms) =
            (0i64, 0i64, 0i64, 0i64);
        for (h, f) in world.firms.iter() {
            if !f.status.operative() {
                continue;
            }
            let yours = player_side(world, h);
            let lots = sold_by_product(world, h).get(&code).copied().unwrap_or(0);
            sellers.push(ShareRowDto {
                firm: h.0,
                name: f.name.clone(),
                lots,
                share_bps: 0,
                yours,
            });
            let (mut uses, mut cap) = (0i64, 0i64);
            for plant in f.facilities.iter().filter_map(|p| world.facilities.get(*p)) {
                if plant.status != cw_sim::state::FacilityStatus::Operating {
                    continue;
                }
                let recipe = &dataset.recipes[plant.recipe as usize];
                uses +=
                    recipe.weekly_inputs_lots.get(id).copied().unwrap_or(0) * plant.blocks as i64;
                cap += recipe.lots_of(id) * plant.blocks as i64;
            }
            capacity += cap;
            if uses > 0 {
                buyer_use += uses;
                target_weeks += pricing::cover_weeks(f, dataset, code);
                buyer_firms += 1;
                buyers.push(ShareRowDto {
                    firm: h.0,
                    name: f.name.clone(),
                    lots: uses,
                    share_bps: 0,
                    yours,
                });
            }
        }
        let ask = now.ask_cents.max(1);
        let households: i64 = world
            .regions
            .iter()
            .map(|r| r.households.spend_cents.get(&code).copied().unwrap_or(0))
            .sum::<i64>()
            / ask;
        if households > 0 {
            buyers.push(ShareRowDto {
                firm: 0,
                name: "Households".to_owned(),
                lots: households,
                share_bps: 0,
                yours: false,
            });
        }
        let facts = DriverFacts {
            service,
            made: now.made,
            sold: now.sold,
            imported: now.imported,
            unsold: now.unsold,
            buyer_stock: (now.stock - now.unsold).max(0),
            buyer_use,
            buyer_target_weeks: if buyer_firms > 0 {
                target_weeks / buyer_firms
            } else {
                0
            },
            glut_weeks: pricing::glut_weeks(dataset, code),
            capacity,
            ask: now.ask_cents,
            reference: good.reference_price_cents,
        };
        let weeks = &world.systems.weeks;
        let pick = |w: &SystemsWeek| w.goods.iter().find(|g| g.product == code).cloned();
        let s = |key: &str, f: &dyn Fn(&systems::GoodWeek) -> i64| {
            series(weeks, key, |w| pick(w).map(|g| f(&g)))
        };
        Some(GoodDepthDto {
            product,
            name: good.name.clone(),
            service,
            reference_cents: good.reference_price_cents,
            ask_cents: now.ask_cents,
            made_lots: now.made,
            sold_lots: now.sold,
            imported_lots: now.imported,
            exported_lots: now.exported,
            stock_lots: now.stock,
            unsold_lots: now.unsold,
            cover_x100: cover_x100(now.stock, now.sold),
            your_share_bps: share_bps(now.your_sold, now.sold),
            drivers: drivers(&facts),
            sellers: share_rows(sellers, 6),
            buyers: share_rows(buyers, 6),
            series: vec![
                s("made", &|g| g.made),
                s("sold", &|g| g.sold),
                s("imported", &|g| g.imported),
                s("exported", &|g| g.exported),
                s("stock", &|g| g.stock),
                series(weeks, "cover_x100", |w| {
                    pick(w).and_then(|g| cover_x100(g.stock, g.sold))
                }),
                s("ask", &|g| g.ask_cents),
                s("your_share_bps", &|g| share_bps(g.your_sold, g.sold)),
            ],
        })
    }

    /// A region's dossier: its people, wages, land, what households buy, who competes there and
    /// how your divisions do, with weekly history.
    pub fn region_dossier(&self, region: u32, firm: u64) -> Option<RegionDossierDto> {
        use cw_sim::economy::events_engine::player_side;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let index = u8::try_from(region).ok()?;
        let state = world.regions.get(index as usize)?;
        let now = systems::region_week(world, index);
        let spend: Vec<(u16, i64)> = state
            .households
            .spend_cents
            .iter()
            .filter(|(_, c)| **c > 0)
            .map(|(p, c)| (*p, *c))
            .collect();
        let total_spend: i64 = spend.iter().map(|(_, c)| *c).sum();
        let mut demand: Vec<DemandDto> = spend
            .iter()
            .map(|(p, c)| DemandDto {
                product: *p as u32,
                name: dataset
                    .products
                    .get(*p as usize)
                    .map(|g| g.name.clone())
                    .unwrap_or_default(),
                spend_cents: *c,
                share_bps: share_bps(*c, total_spend),
            })
            .collect();
        demand.sort_by(|a, b| {
            b.spend_cents
                .cmp(&a.spend_cents)
                .then(a.product.cmp(&b.product))
        });
        demand.truncate(8);
        let you = Handle(firm);
        let mut competitors = Vec::new();
        for (h, f) in world.firms.iter() {
            if !f.status.operative() {
                continue;
            }
            let yours = h == you || player_side(world, h);
            for d in cw_sim::economy::sectors::divisions_of(world, dataset, h)
                .into_iter()
                .filter(|d| d.region == index)
            {
                let (revenue, profit) = if yours {
                    cw_sim::economy::division_report::report(world, dataset, h, index, d.industry)
                        .map(|r| (r.revenue_cents, r.operating_profit_cents))
                        .unwrap_or((0, 0))
                } else {
                    (0, 0)
                };
                competitors.push(CompetitorDto {
                    firm: h.0,
                    name: f.name.clone(),
                    industry: INDUSTRY_NAMES[d.industry as usize].to_owned(),
                    blocks: d.blocks,
                    share_bps: cw_sim::economy::sectors::share_bps(
                        world, dataset, h, index, d.industry, 0,
                    ),
                    yours,
                    profit_cents: profit,
                    revenue_cents: revenue,
                });
            }
        }
        competitors.sort_by(|a, b| {
            a.industry
                .cmp(&b.industry)
                .then(b.share_bps.cmp(&a.share_bps))
                .then(a.firm.cmp(&b.firm))
        });
        let weeks = &world.systems.weeks;
        let at = |w: &SystemsWeek| w.regions.iter().find(|r| r.region == index).cloned();
        let mut series_list = vec![
            series(weeks, "population", |w| at(w).map(|r| r.population)),
            series(weeks, "employed", |w| at(w).map(|r| r.employed)),
            series(weeks, "wage", |w| at(w).map(|r| r.wage_cents)),
            series(weeks, "land_index", |w| at(w).map(|r| r.land_index_bps)),
            series(weeks, "spend", |w| at(w).map(|r| r.spend_cents)),
        ];
        let moved = world
            .plans
            .migration
            .net
            .get(index as usize)
            .cloned()
            .unwrap_or_default();
        series_list.push(SeriesDto {
            key: "migration".to_owned(),
            weeks: (0..moved.len() as u32)
                .map(|i| (world.tick.0 + 1 + i).saturating_sub(moved.len() as u32))
                .collect(),
            values: moved.iter().map(|n| *n as i64).collect(),
        });
        let mut industries: Vec<u8> = weeks
            .iter()
            .flat_map(|w| w.divisions.iter())
            .filter(|d| d.region == index)
            .map(|d| d.industry)
            .collect();
        industries.sort_unstable();
        industries.dedup();
        for industry in industries {
            series_list.push(series(
                weeks,
                &format!("share:{}", INDUSTRY_NAMES[industry as usize]),
                |w| {
                    let rows: Vec<_> = w
                        .divisions
                        .iter()
                        .filter(|d| d.region == index && d.industry == industry)
                        .collect();
                    (!rows.is_empty()).then(|| rows.iter().map(|d| d.share_bps).sum::<i64>())
                },
            ));
        }
        Some(RegionDossierDto {
            index: region,
            name: state.name.clone(),
            population: now.population,
            employed: now.employed,
            wage_cents: now.wage_cents,
            land_index_bps: now.land_index_bps,
            demand_index_bps: now.demand_index_bps,
            spend_cents: now.spend_cents,
            tiers: state
                .labour
                .iter()
                .map(|t| LabourPoolDto {
                    population: t.population,
                    employed: t.employed,
                    idle: (t.population - t.employed).max(0),
                    wage_cents: t.prevailing_wage_cents,
                })
                .collect(),
            demand,
            competitors,
            net_migration_year: world.plans.migration.net_over(index as usize, 52),
            net_migration_week: world.plans.migration.net_over(index as usize, 1),
            series: series_list,
        })
    }

    /// A division's operations: utilisation, cost per lot by part against its price, and weekly
    /// history. Freight is the division's shipping bill for the closed week.
    pub fn division_ops(
        &self,
        firm: u64,
        region: String,
        industry: String,
    ) -> Option<DivisionOpsDto> {
        let first_plant = {
            let session = self.lock();
            let world = &session.world;
            let dataset = &session.dataset;
            let region_index = dataset.regions.iter().position(|r| r.code == region)? as u8;
            let industry_index = cw_sim::economy::operations::industry_index(&industry)?;
            cw_sim::economy::sectors::divisions_of(world, dataset, Handle(firm))
                .into_iter()
                .find(|d| d.region == region_index && d.industry == industry_index)?
                .plants
                .first()
                .copied()?
        };
        let freight = self.shipping_summary(firm, first_plant.0).weekly_cents;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = Handle(firm);
        let region_index = dataset.regions.iter().position(|r| r.code == region)? as u8;
        let industry_index = cw_sim::economy::operations::industry_index(&industry)?;
        let division = cw_sim::economy::sectors::divisions_of(world, dataset, handle)
            .into_iter()
            .find(|d| d.region == region_index && d.industry == industry_index)?;
        let r = cw_sim::economy::division_report::report(
            world,
            dataset,
            handle,
            region_index,
            industry_index,
        )?;
        let primary = r.outputs.iter().find(|o| o.product == r.primary_product);
        let made = primary.map(|o| o.made_milli).unwrap_or(0);
        let sold = primary.map(|o| o.sold_milli).unwrap_or(0);
        let capacity = systems::division_capacity_milli(world, dataset, &division);
        let mut parts: Vec<(String, i64)> = r
            .costs
            .iter()
            .map(|c| (c.key.to_owned(), c.cents))
            .collect();
        parts.push(("freight".to_owned(), freight));
        let (unit, total) = unit_costs(&parts, made);
        let price = primary.map(|o| o.price_cents).unwrap_or(0);
        let weeks = &world.systems.weeks;
        let at = |w: &SystemsWeek| {
            w.divisions
                .iter()
                .find(|d| {
                    d.firm == handle && d.region == region_index && d.industry == industry_index
                })
                .cloned()
        };
        Some(DivisionOpsDto {
            made_milli: made,
            sold_milli: sold,
            capacity_milli: capacity,
            utilisation_bps: share_bps(made, capacity),
            unit_costs: unit,
            unit_cost_cents: total,
            price_cents: price,
            margin_per_lot_cents: if made > 0 { price - total } else { 0 },
            series: vec![
                series(weeks, "made", |w| at(w).map(|d| d.made_milli)),
                series(weeks, "sold", |w| at(w).map(|d| d.sold_milli)),
                series(weeks, "capacity", |w| at(w).map(|d| d.capacity_milli)),
                series(weeks, "utilisation_bps", |w| {
                    at(w).map(|d| share_bps(d.made_milli, d.capacity_milli))
                }),
                series(weeks, "unit_cost", |w| {
                    at(w)
                        .filter(|d| d.made_milli > 0)
                        .map(|d| (d.costs_cents as i128 * 1_000 / d.made_milli as i128) as i64)
                }),
                series(weeks, "revenue", |w| at(w).map(|d| d.revenue_cents)),
                series(weeks, "costs", |w| at(w).map(|d| d.costs_cents)),
                series(weeks, "share_bps", |w| at(w).map(|d| d.share_bps)),
            ],
        })
    }

    /// A company's freight: who carried it, what it cost, its own carriers' capacity used and
    /// idle, this week and over time. Your companies only.
    pub fn logistics_stats(&self, firm: u64) -> Option<LogisticsStatsDto> {
        let session = self.lock();
        let world = &session.world;
        let handle = Handle(firm);
        if !cw_sim::economy::events_engine::player_side(world, handle) {
            return None;
        }
        let now = systems::freight_week(world, handle);
        let weeks = &world.systems.weeks;
        let at = |w: &SystemsWeek| w.freight.iter().find(|f| f.firm == handle).cloned();
        let per_lot = |f: &systems::FreightWeek| if f.lots > 0 { f.cents / f.lots } else { 0 };
        Some(LogisticsStatsDto {
            lots: now.lots,
            own_lots: now.own_lots,
            other_lots: now.other_lots,
            outside_lots: now.outside_lots,
            cents: now.cents,
            cents_per_lot: per_lot(&now),
            carrier_cap: now.carrier_cap,
            carrier_used: now.carrier_used,
            carrier_idle: (now.carrier_cap - now.carrier_used).max(0),
            carrier_fill_bps: share_bps(now.carrier_used, now.carrier_cap),
            series: vec![
                series(weeks, "lots", |w| at(w).map(|f| f.lots)),
                series(weeks, "own", |w| at(w).map(|f| f.own_lots)),
                series(weeks, "other", |w| at(w).map(|f| f.other_lots)),
                series(weeks, "outside", |w| at(w).map(|f| f.outside_lots)),
                series(weeks, "cents", |w| at(w).map(|f| f.cents)),
                series(weeks, "cents_per_lot", |w| at(w).map(|f| per_lot(&f))),
                series(weeks, "carrier_cap", |w| at(w).map(|f| f.carrier_cap)),
                series(weeks, "carrier_used", |w| at(w).map(|f| f.carrier_used)),
                series(weeks, "fill_bps", |w| {
                    at(w).map(|f| share_bps(f.carrier_used, f.carrier_cap))
                }),
            ],
        })
    }

    /// Every operating company with shares or yours, for the sortable rivals board.
    pub fn rivals_board(&self) -> Vec<RivalRowDto> {
        use cw_sim::economy::events_engine::player_side;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let mut out = Vec::new();
        for (h, f) in world.firms.iter() {
            let yours = player_side(world, h);
            if !f.status.operative() || (f.shares.issued <= 0 && !yours) {
                continue;
            }
            let recent = f.book.recent_weeks();
            let tail = &recent[recent.len().saturating_sub(13)..];
            let divisions = cw_sim::economy::sectors::divisions_of(world, dataset, h);
            let financial = crate::statements::is_financial(f.industry);
            let revenue: i64 = tail
                .iter()
                .map(|w| w.revenue + if financial { w.interest_income } else { 0 })
                .sum();
            let profit: i64 = tail.iter().map(|w| w.net_income).sum();
            out.push(RivalRowDto {
                firm: h.0,
                name: f.name.clone(),
                industry: INDUSTRY_NAMES[f.industry as usize].to_owned(),
                yours,
                revenue_cents: revenue,
                profit_cents: profit,
                cash_cents: f.book.cash() + f.book.restricted_cash(),
                debt_cents: f.book.debt(),
                share_price_cents: f.shares.mark_cents,
                market_cap_cents: f.shares.mark_cents.saturating_mul(f.shares.issued),
                divisions: divisions.len() as u32,
                net_margin_bps: (revenue > 0).then(|| share_bps(profit, revenue)),
            });
        }
        out.sort_by(|a, b| {
            b.revenue_cents
                .cmp(&a.revenue_cents)
                .then(a.firm.cmp(&b.firm))
        });
        out
    }

    /// Goods both companies sold in the closed week, with each one's share of the corridor.
    pub fn head_to_head(&self, you: u64, rival: u64) -> Vec<HeadToHeadDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let mine = sold_by_product(world, Handle(you));
        let theirs = sold_by_product(world, Handle(rival));
        let mut total = std::collections::BTreeMap::<u16, i64>::new();
        for (h, _) in world.firms.iter() {
            for (p, q) in sold_by_product(world, h) {
                *total.entry(p).or_insert(0) += q;
            }
        }
        let mut products: Vec<u16> = mine.keys().chain(theirs.keys()).copied().collect();
        products.sort_unstable();
        products.dedup();
        products
            .into_iter()
            .map(|p| {
                let a = mine.get(&p).copied().unwrap_or(0);
                let b = theirs.get(&p).copied().unwrap_or(0);
                let t = total.get(&p).copied().unwrap_or(0);
                HeadToHeadDto {
                    product: p as u32,
                    name: dataset
                        .products
                        .get(p as usize)
                        .map(|g| g.name.clone())
                        .unwrap_or_default(),
                    your_lots: a,
                    their_lots: b,
                    your_share_bps: share_bps(a, t),
                    their_share_bps: share_bps(b, t),
                }
            })
            .collect()
    }

    /// The economy week by week: rates, prices, firms operating and gone, raids and household
    /// spending.
    pub fn economy_series(&self) -> Vec<SeriesDto> {
        let session = self.lock();
        let weeks = &session.world.systems.weeks;
        vec![
            series(weeks, "policy_rate_bps", |w| {
                Some(w.economy.policy_rate_bps)
            }),
            series(weeks, "inflation_bps", |w| Some(w.economy.inflation_bps)),
            series(weeks, "price_index_bps", |w| {
                Some(w.economy.price_index_bps)
            }),
            series(weeks, "firms_operating", |w| {
                Some(w.economy.firms_operating)
            }),
            series(weeks, "firms_gone", |w| Some(w.economy.firms_gone)),
            series(weeks, "raids", |w| Some(w.economy.raids)),
            series(weeks, "raids_won", |w| Some(w.economy.raids_won)),
            series(weeks, "household_spend", |w| {
                Some(w.economy.household_spend_cents)
            }),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(firm: u64, lots: i64, yours: bool) -> ShareRowDto {
        ShareRowDto {
            firm,
            name: String::new(),
            lots,
            share_bps: 0,
            yours,
        }
    }

    #[test]
    fn cover_is_stock_over_weekly_sales() {
        assert_eq!(cover_x100(30, 12), Some(250));
        assert_eq!(cover_x100(0, 5), Some(0));
        assert_eq!(cover_x100(9, 0), None);
    }

    #[test]
    fn shares_are_of_the_whole_and_keep_yours_past_the_cut() {
        let rows = vec![
            row(1, 50, false),
            row(2, 30, false),
            row(3, 15, false),
            row(4, 5, true),
        ];
        let all = share_rows(rows.clone(), 10);
        // Hand arithmetic: 50, 30, 15 and 5 of 100.
        assert_eq!(
            all.iter().map(|r| r.share_bps).collect::<Vec<_>>(),
            vec![5_000, 3_000, 1_500, 500]
        );
        assert_eq!(all.iter().map(|r| r.share_bps).sum::<i64>(), 10_000);
        let top = share_rows(rows, 2);
        assert_eq!(
            top.iter().map(|r| r.firm).collect::<Vec<_>>(),
            vec![1, 2, 4]
        );
        // Shares stay shares of the whole market, not of the rows kept.
        assert_eq!(top[2].share_bps, 500);
    }

    #[test]
    fn unit_cost_parts_add_up_to_the_total_per_lot() {
        let parts = vec![
            ("materials".to_owned(), 10_000),
            ("wages".to_owned(), 7_001),
            ("freight".to_owned(), 2_999),
        ];
        // 20,000 cents over 3 lots made: 6,666 per lot (floored).
        let (rows, total) = unit_costs(&parts, 3_000);
        assert_eq!(total, 6_666);
        assert_eq!(rows.iter().map(|r| r.cents).sum::<i64>(), total);
        // Each part within a cent of its own share: 3,333.3, 2,333.7 and 999.7.
        let exact = [3_333, 2_333, 999];
        for (r, e) in rows.iter().zip(exact) {
            assert!((r.cents - e).abs() <= 1, "{} {}", r.key, r.cents);
        }
        let (zero, total) = unit_costs(&parts, 0);
        assert_eq!(total, 0);
        assert!(zero.iter().all(|r| r.cents == 0));
        // Odd sizes and many parts still reconcile.
        for made in [1, 7, 999, 1_001, 12_345] {
            let parts: Vec<(String, i64)> =
                (1..=6).map(|i| (format!("p{i}"), i * 1_237 + 11)).collect();
            let (rows, total) = unit_costs(&parts, made);
            let sum: i64 = parts.iter().map(|p| p.1).sum();
            assert_eq!(total, (sum as i128 * 1_000 / made as i128) as i64);
            assert_eq!(
                rows.iter().map(|r| r.cents).sum::<i64>(),
                total,
                "made {made}"
            );
        }
    }

    fn keys(f: &DriverFacts) -> Vec<String> {
        drivers(f).into_iter().map(|d| d.key).collect()
    }

    #[test]
    fn drivers_name_the_signals_that_are_present() {
        let base = DriverFacts {
            made: 100,
            sold: 100,
            buyer_stock: 400,
            buyer_use: 100,
            buyer_target_weeks: 4,
            glut_weeks: 4,
            ask: 10_000,
            reference: 10_000,
            ..Default::default()
        };
        assert_eq!(keys(&base), vec!["balanced", "vs_reference"]);
        // Buyers hold 1.5 weeks against 4 wanted.
        let short = DriverFacts {
            buyer_stock: 150,
            ..base
        };
        let d = drivers(&short);
        assert_eq!(d[0].key, "buyers_short");
        assert!(d[0].text.contains("1.50 weeks"), "{}", d[0].text);
        assert_eq!(d[0].push, 1);
        // Sellers sit on 6 weeks of sales against 4 allowed.
        let glut = DriverFacts {
            unsold: 600,
            ..base
        };
        assert_eq!(keys(&glut)[0], "unsold_stock");
        // Imports at 125 percent of home output count, just under does not.
        let imports = DriverFacts {
            imported: 125,
            ..base
        };
        assert_eq!(keys(&imports)[0], "imports");
        let under = DriverFacts {
            imported: 124,
            ..base
        };
        assert_eq!(keys(&under)[0], "balanced");
        // A service at 70 percent of capacity has spare room; at 96 percent it is sold out.
        let spare = DriverFacts {
            service: true,
            sold: 70,
            capacity: 100,
            ..base
        };
        assert_eq!(keys(&spare)[0], "spare_capacity");
        let full = DriverFacts {
            service: true,
            sold: 96,
            capacity: 100,
            ..base
        };
        assert_eq!(keys(&full)[0], "sold_out");
        let over = DriverFacts {
            ask: 11_250,
            ..base
        };
        assert_eq!(
            drivers(&over).last().unwrap().text,
            "The ask is 12.5% over reference."
        );
    }

    #[test]
    fn a_played_session_reads_back_consistent_systems_figures() {
        let session = SessionHandle::new(181, true, true).unwrap();
        session.advance(10);
        let board = session.commodity_board();
        let mut checked = 0;
        for line in &board {
            let Some(depth) = session.good_depth(line.product) else {
                continue;
            };
            // The depth view reads the same week as the board.
            assert_eq!(depth.made_lots, line.made_lots, "{}", line.name);
            assert_eq!(depth.sold_lots, line.sold_lots, "{}", line.name);
            assert_eq!(depth.stock_lots, line.stock_lots, "{}", line.name);
            assert_eq!(depth.imported_lots, line.imported_lots, "{}", line.name);
            // Cover is stock over weekly sales, from the board's own numbers.
            if line.sold_lots > 0 {
                assert_eq!(
                    depth.cover_x100,
                    Some(line.stock_lots * 100 / line.sold_lots)
                );
            }
            // Seller shares are of all sales: those listed never add past the whole, and with
            // every seller listed they add to it within rounding.
            let listed: i64 = depth.sellers.iter().map(|r| r.lots).sum();
            let shares: i64 = depth.sellers.iter().map(|r| r.share_bps).sum();
            assert!(listed <= line.sold_lots);
            assert!(shares <= 10_000);
            if listed == line.sold_lots && listed > 0 {
                assert!(
                    shares > 10_000 - depth.sellers.len() as i64,
                    "{}",
                    line.name
                );
            }
            // The last point of the history is this week's figure.
            let sold = depth.series.iter().find(|s| s.key == "sold").unwrap();
            if let Some(last) = sold.values.last() {
                assert_eq!(*last, line.sold_lots);
                checked += 1;
            }
            assert!(!depth.drivers.is_empty());
        }
        assert!(checked > 5, "only {checked} goods had history");
        let you = session
            .overview()
            .primary_firm
            .expect("a guided career has a company");
        for d in session.divisions(you) {
            let ops = session
                .division_ops(you, d.region.clone(), d.industry.clone())
                .unwrap();
            assert_eq!(
                ops.unit_costs.iter().map(|u| u.cents).sum::<i64>(),
                ops.unit_cost_cents
            );
            assert_eq!(
                ops.margin_per_lot_cents,
                if ops.made_milli > 0 {
                    ops.price_cents - ops.unit_cost_cents
                } else {
                    0
                }
            );
            if ops.capacity_milli > 0 {
                assert_eq!(
                    ops.utilisation_bps,
                    ops.made_milli * 10_000 / ops.capacity_milli
                );
            }
        }
        let stats = session.logistics_stats(you).unwrap();
        assert_eq!(
            stats.carrier_idle + stats.carrier_used,
            stats.carrier_cap.max(stats.carrier_used)
        );
        assert!(
            stats.own_lots + stats.other_lots + stats.outside_lots
                <= stats
                    .lots
                    .max(stats.own_lots + stats.other_lots + stats.outside_lots)
        );
        let rivals = session.rivals_board();
        assert!(rivals.iter().any(|r| r.yours));
        let stocks = session.stock_board();
        for r in &rivals {
            if let Some(s) = stocks.iter().find(|s| s.firm == r.firm) {
                assert_eq!(r.market_cap_cents, r.share_price_cents * s.shares_issued);
            }
        }
        let economy = session.economy_series();
        let firms = economy.iter().find(|s| s.key == "firms_operating").unwrap();
        assert_eq!(firms.values.len(), 10);
        let region = session.region_dossier(0, you).unwrap();
        assert_eq!(
            region.population,
            region.tiers.iter().map(|t| t.population).sum::<i64>()
        );
        let demand: i64 = region.demand.iter().map(|d| d.share_bps).sum();
        assert!(demand <= 10_000);
    }

    /// A bank's and an insurer's living is interest, premiums and fees: the board and the
    /// statements count it as revenue, so neither reads as earning nothing, and net income is
    /// the same figure the books close with.
    #[test]
    fn financial_firms_show_interest_and_premiums_as_revenue() {
        let session = SessionHandle::new(181, true, true).unwrap();
        session.advance(2);
        // Give the bank a loan to earn interest on and the insurer a policy to collect on.
        let you = session.overview().primary_firm.unwrap();
        {
            let mut guard = session.lock();
            let world = &mut guard.world;
            let of = |name: &str| {
                world
                    .firms
                    .iter()
                    .find(|(_, f)| f.industry == 9 && f.name.contains(name))
                    .map(|(h, _)| h)
                    .unwrap()
            };
            let (bank, insurer) = (of("Bank"), of("Assurance"));
            let plant = world.firms.get(Handle(you)).unwrap().facilities[0];
            cw_sim::economy::banking::approve_loan(world, bank, Handle(you), 5_000_000).unwrap();
            cw_sim::economy::insurance::bind(
                world,
                insurer,
                Handle(you),
                plant,
                10_000_000,
                100_000,
            )
            .unwrap();
        }
        session.advance(12);
        let board = session.rivals_board();
        let banks: Vec<&RivalRowDto> = board.iter().filter(|r| r.industry == "financial").collect();
        assert!(!banks.is_empty(), "no bank or insurer on the board");
        let handle = session.lock();
        for bank in &banks {
            let firm = handle.world.firms.get(Handle(bank.firm)).unwrap();
            let recent = firm.book.recent_weeks();
            let tail = &recent[recent.len().saturating_sub(13)..];
            // Income the books credited: sales, premiums and fees, and interest earned.
            let earned: i64 = tail.iter().map(|w| w.revenue + w.interest_income).sum();
            assert_eq!(bank.revenue_cents, earned, "{}", bank.name);
            assert_eq!(
                bank.profit_cents,
                tail.iter().map(|w| w.net_income).sum::<i64>()
            );
            if bank.revenue_cents > 0 {
                assert_eq!(
                    bank.net_margin_bps,
                    Some(bank.profit_cents * 10_000 / bank.revenue_cents)
                );
            }
        }
        assert!(
            banks.iter().all(|b| b.revenue_cents > 0),
            "a bank or insurer with business shows no revenue"
        );
        drop(handle);
        let bank = banks.iter().max_by_key(|b| b.revenue_cents).unwrap();
        let weeks = session.statements(bank.firm, "week".into());
        assert!(!weeks.is_empty());
        for w in &weeks {
            // Interest income sits in revenue, and the income statement still adds up.
            assert_eq!(w.interest_income_cents, 0);
            assert_eq!(
                w.revenue_cents
                    - w.cost_of_goods_cents
                    - w.wages_cents
                    - w.operating_expense_cents
                    - w.depreciation_cents
                    + w.other_cents,
                w.ebit_cents
            );
            assert_eq!(
                w.ebit_cents - w.interest_expense_cents - w.tax_cents,
                w.net_income_cents,
                "{}",
                w.label
            );
        }
        assert!(weeks.iter().any(|w| w.revenue_cents > 0));
        // A company that is not a bank keeps interest income below the line.
        let mine = session
            .rivals_board()
            .into_iter()
            .find(|r| r.firm == you)
            .unwrap();
        assert_ne!(mine.industry, "financial");
    }
}
