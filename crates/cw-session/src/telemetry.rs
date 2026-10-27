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
pub const SNAPSHOT_SCHEMA: u32 = 1;

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

#[derive(Clone, Debug, Serialize)]
pub struct PlayerLine {
    pub cash_cents: i64,
    pub revenue_13w_cents: i64,
    pub net_income_13w_cents: i64,
    pub divisions: u32,
    pub plants: u32,
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
    pub player: Option<PlayerLine>,
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
                .filter(|(_, f)| dataset.recipes[f.recipe as usize].output == product.id)
                .map(|(_, f)| f.last_output.lots())
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
            })
        });
        MarketSnapshot {
            schema: SNAPSHOT_SCHEMA,
            engine_version: cw_core::ENGINE_VERSION.to_owned(),
            dataset_hash: dataset.hash.clone(),
            seed: world.seed,
            scenario: format!("{:?}", world.scenario).to_lowercase(),
            week: world.tick.0,
            goods,
            corridor,
            player,
        }
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
