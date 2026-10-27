//! Market-balance snapshots for anonymous gameplay analytics.
//!
//! Every five weeks the app uploads one snapshot: per good, what the corridor made, sold,
//! imported, exported and holds, at what asking price against reference, and what households
//! spent; the corridor's health; and the player's company results. It carries engine version,
//! dataset hash, seed, scenario and week so runs can be compared. It contains no names, no
//! device or account data; the app adds only a random install id.

use serde::Serialize;

use crate::Session;

/// Weeks between snapshots.
pub const SNAPSHOT_EVERY_WEEKS: u32 = 5;
pub const SNAPSHOT_SCHEMA: u32 = 4;

#[derive(Clone, Debug, Serialize)]
pub struct GoodLine {
    pub id: String,
    pub made_lots: i64,
    pub sold_lots: i64,
    pub imported_lots: i64,
    pub exported_lots: i64,
    pub stock_lots: i64,
    pub median_ask_cents: i64,
    pub reference_cents: i64,
    pub household_spend_cents: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CorridorLine {
    pub firms_operating: u32,
    pub firms_dissolved: u32,
    pub raids: u32,
    pub outside_cash_cents: i64,
}

/// Storage over the last thirteen weeks for the whole corridor, or for the player's company:
/// what is stored against what space there is, and what overflow, holding and spoilage cost.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StorageLine {
    pub used_lots: i64,
    pub capacity_lots: i64,
    pub owned_capacity_lots: i64,
    pub outside_lots: i64,
    pub outside_fees_13w_cents: i64,
    pub holding_13w_cents: i64,
    pub spoiled_lots_13w: i64,
    pub spoiled_13w_cents: i64,
    pub held_lots: i64,
    pub open_buy_orders: u32,
}

/// Freight: capacity, use and price by mode for the week just closed, and how much of it went to
/// outside carriers. `mode_ids` names the four slots of every `*_by_mode` list.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LogisticsLine {
    pub mode_ids: Vec<String>,
    pub carriers: u32,
    pub lanes_with_capacity: u32,
    pub capacity_lots: i64,
    pub capacity_lots_by_mode: Vec<i64>,
    pub lots_by_mode: Vec<i64>,
    pub outside_lots_by_mode: Vec<i64>,
    /// Share of leg lots that overflowed to outside carriers, basis points.
    pub outside_share_bps: i32,
    pub own_group_lots: i64,
    pub contract_lots: i64,
    pub freight_paid_cents: i64,
    pub outside_paid_cents: i64,
    pub shipments: i64,
    /// Average price paid, thousandths of a cent per lot-kilometre.
    pub avg_milli_cents_per_lot_km: i64,
    /// Average spot price of a lane with capacity, basis points of its base rate.
    pub avg_lane_mult_bps: i32,
    pub congested_lanes: u32,
    pub active_contracts: u32,
    pub contract_lots_held: i64,
}

/// The government: which laws are in force, what the reserve holds and how many times the state
/// has stepped in, plus how many systemic incidents of each kind have struck so far.
#[derive(Clone, Debug, Default, Serialize)]
pub struct GovernmentLine {
    pub party: String,
    /// Announced changes that have taken effect so far.
    pub law_changes: u32,
    pub common_carrier: bool,
    pub vertical_review_bps: i64,
    pub quota_states: u32,
    /// Extraction goods under a quota right now, as (region, good) cells.
    pub quota_cells_active: u32,
    pub reserve_on: bool,
    pub reserve_lots: i64,
    pub reserve_value_cents: i64,
    pub reserve_bought_cents: i64,
    pub reserve_sold_cents: i64,
    pub rescue: String,
    pub bailouts: u32,
    pub nationalisations: u32,
    pub privatisations: u32,
    pub state_firms: u32,
    /// Incident kinds, and how many of each have struck since the game began.
    pub incident_kinds: Vec<String>,
    pub incidents: Vec<u32>,
    /// Lanes closed or cut this week.
    pub closures_active: u32,
    /// Pipeline carriers the corridor's firms run.
    pub pipelines: u32,
}

/// Corporate structure: groups, transfer pricing, reputation, bonds, ventures and activists.
#[derive(Clone, Debug, Default, Serialize)]
pub struct StructureLine {
    pub groups: u32,
    pub group_links: u32,
    pub group_lots_moved: i64,
    /// Links priced away from market, and the largest gap in basis points of market.
    pub links_off_market: u32,
    pub squeeze_cents_total: i64,
    pub tax_shift_cents_total: i64,
    pub captive_suppliers_marked_down: u32,
    pub avg_reputation_bps: i32,
    pub divisions_with_premium: u32,
    pub bonds_outstanding: u32,
    pub bond_principal_cents: i64,
    pub avg_coupon_bps: i32,
    pub bonds_issued_total: u32,
    pub bond_defaults_total: u32,
    pub ventures: u32,
    pub ventures_formed_total: u32,
    pub ipos_total: u32,
    pub spin_offs_total: u32,
    pub carve_outs_total: u32,
    pub activist_campaigns_total: u32,
    pub activist_wins_total: u32,
    pub activist_open: u32,
    pub repeat_raids_total: u32,
    pub blocks_under_construction: i64,
    pub firms_over_construction_capacity: u32,
}

/// The character cast at work: rival chief executives replaced after long losses, and how many
/// rivals have a named chief executive. Counts only, never names.
#[derive(Clone, Debug, Default, Serialize)]
pub struct CastLine {
    pub ceo_turnovers: u32,
    pub named_ceos: u32,
}

/// Rival personalities at work: moves made by temperament and by class, grudges and alliances
/// formed, and how many rivals hold a grudge now. Counts only, never names.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RivalryLine {
    pub grudges_formed: u32,
    pub alliances_formed: u32,
    pub grudges_active: u32,
    pub alliances_active: u32,
    /// Moves aimed at a group the mover held a grudge against.
    pub grudge_moves: u32,
    pub moves_by_temperament: std::collections::BTreeMap<String, u32>,
    pub moves_by_class: std::collections::BTreeMap<String, u32>,
    /// Rivals by the temperament that runs them right now.
    pub rivals_by_temperament: std::collections::BTreeMap<String, u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlayerLine {
    pub cash_cents: i64,
    pub revenue_13w_cents: i64,
    pub net_income_13w_cents: i64,
    pub divisions: u32,
    pub plants: u32,
    pub storage: StorageLine,
    pub carrier_lots: i64,
    pub group_lots_shipped: i64,
    pub contract_lots_held: i64,
    pub bonds_issued: u32,
    pub reputation_bps: i32,
    pub group_links: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct MarketSnapshot {
    pub schema: u32,
    pub engine_version: String,
    pub dataset_hash: String,
    pub seed: u64,
    pub scenario: String,
    pub week: u32,
    pub goods: Vec<GoodLine>,
    pub corridor: CorridorLine,
    pub storage: StorageLine,
    pub logistics: LogisticsLine,
    pub government: GovernmentLine,
    pub structure: StructureLine,
    pub cast: CastLine,
    pub rivalry: RivalryLine,
    pub player: Option<PlayerLine>,
}

fn government_line(world: &cw_sim::state::World, dataset: &cw_data::Dataset) -> GovernmentLine {
    use cw_sim::economy::systemic;
    let p = &world.politics;
    let pipeline = dataset.logistics.mode_index("pipeline");
    GovernmentLine {
        party: p.party.key().to_owned(),
        law_changes: p.law_changes,
        common_carrier: p.laws.common_carrier,
        vertical_review_bps: p.laws.vertical_review_bps,
        quota_states: if p.laws.quota_bps > 0 {
            p.laws.quota_states.count_ones()
        } else {
            0
        },
        quota_cells_active: p.quota_active.len() as u32,
        reserve_on: p.laws.reserve,
        reserve_lots: p.reserve.total_lots(),
        reserve_value_cents: p.reserve.value_cents(),
        reserve_bought_cents: p.reserve.bought_cents,
        reserve_sold_cents: p.reserve.sold_cents,
        rescue: p.laws.rescue.key().to_owned(),
        bailouts: p.bailouts,
        nationalisations: p.nationalisations,
        privatisations: p.privatisations,
        state_firms: world
            .firms
            .iter()
            .filter(|(_, f)| f.state_owned_since.is_some() && f.operative())
            .count() as u32,
        incident_kinds: (0..systemic::KINDS as u8)
            .map(|k| systemic::kind_key(k).to_owned())
            .collect(),
        incidents: world.systemic.counts.to_vec(),
        closures_active: world.logistics.closures.len() as u32,
        pipelines: world
            .logistics
            .carriers
            .iter()
            .filter(|c| Some(c.mode) == pipeline)
            .count() as u32,
    }
}

fn storage_line(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    firms: impl Iterator<Item = cw_core::ids::Handle>,
) -> StorageLine {
    let mut line = StorageLine::default();
    for handle in firms {
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        for u in cw_sim::economy::storage::uses(world, dataset, firm) {
            line.used_lots += u.used_milli / 1_000;
            line.capacity_lots += u.capacity_milli() / 1_000;
            line.owned_capacity_lots += u.owned_milli / 1_000;
            line.outside_lots += u.outside_milli() / 1_000;
        }
        let quarter = firm.storage.recent(13);
        line.outside_fees_13w_cents += quarter.fee_cents;
        line.holding_13w_cents += quarter.holding_cents;
        line.spoiled_lots_13w += quarter.spoiled_milli / 1_000;
        line.spoiled_13w_cents += quarter.spoiled_cents;
        line.held_lots += firm.storage.held.values().map(|q| q.lots()).sum::<i64>();
        line.open_buy_orders += firm.storage.orders.len() as u32;
    }
    line
}

fn logistics_line(world: &cw_sim::state::World, dataset: &cw_data::Dataset) -> LogisticsLine {
    let st = &world.logistics;
    let last = &st.last;
    let modes = dataset.logistics.modes.len();
    let mut capacity_by_mode = vec![0i64; modes];
    let mut with_cap = 0u32;
    let mut mult_sum = 0i64;
    for (key, lane) in &st.lanes {
        let cap = lane.cap() as i64;
        if cap > 0 {
            with_cap += 1;
            capacity_by_mode[key.mode as usize] += cap;
            mult_sum += cw_sim::economy::logistics::spot_mult_bps(dataset, lane.fill_bps());
        }
    }
    let moved: i64 = last.lots_by_mode.iter().sum();
    let outside: i64 = last.outside_lots_by_mode.iter().sum();
    LogisticsLine {
        mode_ids: dataset
            .logistics
            .modes
            .iter()
            .map(|m| m.id.clone())
            .collect(),
        carriers: st.carriers.len() as u32,
        lanes_with_capacity: with_cap,
        capacity_lots: capacity_by_mode.iter().sum(),
        capacity_lots_by_mode: capacity_by_mode,
        lots_by_mode: last.lots_by_mode.to_vec(),
        outside_lots_by_mode: last.outside_lots_by_mode.to_vec(),
        outside_share_bps: (outside * 10_000 / moved.max(1)) as i32,
        own_group_lots: last.own_lots,
        contract_lots: last.contract_lots,
        freight_paid_cents: last.cents,
        outside_paid_cents: last.outside_cents,
        shipments: last.shipments,
        avg_milli_cents_per_lot_km: last.cents * 1_000 / last.lot_km.max(1),
        avg_lane_mult_bps: (mult_sum / (with_cap as i64).max(1)) as i32,
        congested_lanes: st.stats.values().filter(|s| s.hot_weeks >= 4).count() as u32,
        active_contracts: st.contracts.iter().filter(|c| c.weeks_left > 0).count() as u32,
        contract_lots_held: st.contracts.iter().map(|c| c.held as i64).sum(),
    }
}

fn structure_line(world: &cw_sim::state::World) -> StructureLine {
    use cw_sim::economy::structure as st;
    let stats = &world.structure.stats;
    let mut roots = std::collections::BTreeSet::new();
    for (handle, f) in world.firms.iter() {
        if f.operative()
            && let Some(root) = cw_sim::economy::ownership::ultimate_controller(world, handle)
        {
            roots.insert(root);
        }
    }
    let reps: Vec<i32> = world.structure.reputation.values().copied().collect();
    let bonds: Vec<&cw_sim::state::Bond> = world.bonds.iter().map(|(_, b)| b).collect();
    let over: u32 = world
        .firms
        .iter()
        .filter(|(h, f)| {
            f.operative() && cw_sim::economy::sectors::construction_excess(world, *h, 0).0 > 0
        })
        .count() as u32;
    StructureLine {
        groups: roots.len() as u32,
        group_links: world.structure.links.len() as u32,
        group_lots_moved: stats.group_lots_moved,
        links_off_market: world
            .structure
            .links
            .values()
            .filter(|l| l.price_cents != l.market_cents)
            .count() as u32,
        squeeze_cents_total: stats.squeeze_cents,
        tax_shift_cents_total: stats.tax_shift_cents,
        captive_suppliers_marked_down: world
            .structure
            .captive_weeks
            .keys()
            .filter(|f| st::markdown_bps(world, **f) > 0)
            .count() as u32,
        avg_reputation_bps: (reps.iter().map(|r| *r as i64).sum::<i64>()
            / (reps.len().max(1) as i64)) as i32,
        divisions_with_premium: reps.iter().filter(|r| **r > st::NEUTRAL_REP).count() as u32,
        bonds_outstanding: bonds.len() as u32,
        bond_principal_cents: bonds.iter().map(|b| b.principal_cents).sum(),
        avg_coupon_bps: (bonds.iter().map(|b| b.coupon_bps as i64).sum::<i64>()
            / (bonds.len().max(1) as i64)) as i32,
        bonds_issued_total: stats.bonds_issued,
        bond_defaults_total: stats.bond_defaults,
        ventures: world.structure.jvs.len() as u32,
        ventures_formed_total: stats.jvs_formed,
        ipos_total: stats.ipos,
        spin_offs_total: stats.spin_offs,
        carve_outs_total: stats.carve_outs,
        activist_campaigns_total: stats.activist_events,
        activist_wins_total: stats.activist_wins,
        activist_open: world
            .structure
            .campaigns
            .iter()
            .filter(|c| c.outcome == st::Outcome::Open)
            .count() as u32,
        repeat_raids_total: stats.raids_repeat,
        blocks_under_construction: world
            .facilities
            .iter()
            .filter(|(_, p)| p.status == cw_sim::state::FacilityStatus::Building)
            .map(|(_, p)| p.blocks as i64)
            .sum(),
        firms_over_construction_capacity: over,
    }
}

/// True on the weeks a snapshot is due.
pub fn due(week: u32) -> bool {
    week > 0 && week.is_multiple_of(SNAPSHOT_EVERY_WEEKS)
}

impl Session {
    /// The current market-balance snapshot.
    pub fn market_snapshot(&self) -> MarketSnapshot {
        let world = &self.world;
        let dataset = &self.dataset;
        let mut goods = Vec::new();
        for (code, product) in dataset.products.iter().enumerate() {
            let code = code as u16;
            if product.reference_price_cents <= 0 {
                continue;
            }
            let made: i64 = world
                .facilities
                .iter()
                .map(|(_, f)| f.last_made(dataset, &product.id).lots())
                .sum();
            let sold: i64 = world
                .firms
                .iter()
                .filter_map(|(_, f)| f.last_closed_sales())
                .flat_map(|week| week.iter())
                .filter(|((_, p), _)| *p == code)
                .map(|(_, q)| *q)
                .sum();
            let (imported, exported) = world
                .gateways
                .iter()
                .filter(|g| g.product == code)
                .fold((0, 0), |(i, e), g| {
                    (i + g.last_import_lots, e + g.last_export_lots)
                });
            let stock: i64 = world
                .firms
                .iter()
                .flat_map(|(_, f)| f.inventory.iter())
                .filter(|((p, _), _)| *p == code)
                .map(|(_, lot)| lot.qty.lots())
                .sum();
            let mut asks: Vec<i64> = world
                .firms
                .iter()
                .filter(|(_, f)| f.operative())
                .filter_map(|(_, f)| f.policies.prices.get(&code).copied())
                .collect();
            asks.sort_unstable();
            let reference = dataset.reference_price(code);
            let spend: i64 = world
                .regions
                .iter()
                .map(|r| r.households.spend_cents.get(&code).copied().unwrap_or(0))
                .sum();
            goods.push(GoodLine {
                id: product.id.clone(),
                made_lots: made,
                sold_lots: sold,
                imported_lots: imported,
                exported_lots: exported,
                stock_lots: stock,
                median_ask_cents: asks.get(asks.len() / 2).copied().unwrap_or(reference),
                reference_cents: reference,
                household_spend_cents: spend,
            });
        }
        let operating = world.firms.iter().filter(|(_, f)| f.operative()).count() as u32;
        let corridor = CorridorLine {
            firms_operating: operating,
            firms_dissolved: world.firms.iter().count() as u32 - operating,
            raids: world.raids.len() as u32,
            outside_cash_cents: world.outside.gateway_book.cash(),
        };
        let player = world.investor.primary_group.and_then(|handle| {
            let firm = world.firms.get(handle)?;
            let weeks = firm.book.recent_weeks();
            Some(PlayerLine {
                cash_cents: firm.book.cash(),
                revenue_13w_cents: weeks.iter().map(|w| w.revenue).sum(),
                net_income_13w_cents: weeks.iter().map(|w| w.net_income).sum(),
                divisions: cw_sim::economy::sectors::divisions_of(world, dataset, handle).len()
                    as u32,
                plants: firm
                    .facilities
                    .iter()
                    .filter_map(|h| world.facilities.get(*h))
                    .map(|p| p.blocks as u32)
                    .sum(),
                storage: storage_line(world, dataset, [handle].into_iter()),
                carrier_lots: world
                    .logistics
                    .carriers
                    .iter()
                    .filter(|c| cw_sim::economy::ownership::same_group(world, handle, c.firm))
                    .map(|c| c.cap as i64)
                    .sum(),
                group_lots_shipped: world
                    .logistics
                    .lanes
                    .values()
                    .flat_map(|l| l.users.iter())
                    .filter(|(f, _)| cw_sim::economy::ownership::same_group(world, handle, *f))
                    .map(|(_, lots)| *lots as i64)
                    .sum(),
                contract_lots_held: world
                    .logistics
                    .contracts
                    .iter()
                    .filter(|c| cw_sim::economy::ownership::same_group(world, handle, c.holder))
                    .map(|c| c.held as i64)
                    .sum(),
                bonds_issued: firm.bonds.len() as u32,
                reputation_bps: {
                    let divisions: Vec<i32> =
                        cw_sim::economy::sectors::divisions_of(world, dataset, handle)
                            .iter()
                            .map(|d| {
                                cw_sim::economy::structure::reputation(
                                    world, handle, d.region, d.industry,
                                )
                            })
                            .collect();
                    divisions.iter().sum::<i32>() / (divisions.len().max(1) as i32)
                },
                group_links: world
                    .structure
                    .links
                    .keys()
                    .filter(|(b, s, _)| {
                        cw_sim::economy::ownership::same_group(world, handle, *b)
                            || cw_sim::economy::ownership::same_group(world, handle, *s)
                    })
                    .count() as u32,
            })
        });
        let storage = storage_line(
            world,
            dataset,
            world
                .firms
                .iter()
                .filter(|(_, f)| f.operative())
                .map(|(h, _)| h),
        );
        MarketSnapshot {
            schema: SNAPSHOT_SCHEMA,
            engine_version: cw_core::ENGINE_VERSION.to_owned(),
            dataset_hash: dataset.hash.clone(),
            seed: world.seed,
            scenario: format!("{:?}", world.scenario).to_lowercase(),
            week: world.tick.0,
            goods,
            corridor,
            storage,
            logistics: logistics_line(world, dataset),
            government: government_line(world, dataset),
            structure: structure_line(world),
            cast: CastLine {
                ceo_turnovers: world.cast.turnovers,
                named_ceos: world.firms.iter().filter(|(_, f)| f.ceo.is_some()).count() as u32,
            },
            rivalry: rivalry_line(world, dataset),
            player,
        }
    }
}

fn rivalry_line(world: &cw_sim::state::World, dataset: &cw_data::Dataset) -> RivalryLine {
    use cw_sim::economy::rivalry::ACTIVE;
    let stats = &world.rivalry.stats;
    let mut rivals_by_temperament = std::collections::BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if firm.archetype.is_some() && firm.operative() {
            *rivals_by_temperament
                .entry(cw_ai::style(world, dataset, handle).key().to_owned())
                .or_insert(0) += 1;
        }
    }
    RivalryLine {
        grudges_formed: stats.grudges_formed,
        alliances_formed: stats.alliances_formed,
        grudges_active: world
            .rivalry
            .grudges
            .values()
            .filter(|m| m.score >= ACTIVE)
            .count() as u32,
        alliances_active: world
            .rivalry
            .alliances
            .values()
            .filter(|m| m.score >= ACTIVE)
            .count() as u32,
        grudge_moves: stats.grudge_moves,
        moves_by_temperament: stats.moves_by_temperament.clone(),
        moves_by_class: stats.moves_by_class.clone(),
        rivals_by_temperament,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_names_no_one_and_covers_every_priced_good() {
        let mut session = Session::create(crate::ScenarioConfig {
            seed: 9,
            scenario: cw_sim::state::ScenarioKind::Guided,
            mode: cw_sim::state::ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
        })
        .unwrap();
        session.advance(5);
        assert!(due(5) && !due(4) && !due(0));
        let snapshot = session.market_snapshot();
        assert_eq!(snapshot.week, 5);
        let priced = session
            .dataset
            .products
            .iter()
            .filter(|p| p.reference_price_cents > 0)
            .count();
        assert_eq!(snapshot.goods.len(), priced);
        let json = serde_json::to_string(&snapshot).unwrap();
        // No company or person names: only good ids, numbers and run identity.
        for (_, firm) in session.world.firms.iter() {
            assert!(!json.contains(&firm.name), "{} leaked", firm.name);
        }
        assert!(
            json.len() < 16_000,
            "small enough to upload: {} bytes",
            json.len()
        );
    }
}
