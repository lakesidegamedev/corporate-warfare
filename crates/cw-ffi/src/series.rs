//! Bulk history for the grids (2.0): one call per grid instead of one per row, so a timeframe
//! of up to five years can drive every change, sparkline and chart column.
//!
//! Nothing here is new engine state. The weekly goods and division figures come from the
//! systems history (`World.systems`, five years), the price and share closes from the history
//! archive (`World.archive.prices`, ten years) and the debt from the archive's weekly and
//! quarterly statements. All three are presentation history outside the hashed world, so a
//! save from before them starts its series at the next week played.

use cw_core::ids::Handle;
use cw_sim::state::INDUSTRY_NAMES;

use crate::SessionHandle;

/// Weeks the grids ask for at most: five years.
const GRID_WEEKS: usize = 260;

/// One good's weekly figures, oldest first: `made[i]` and the others are lots in game week
/// `weeks[i]`.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct GoodSeriesDto {
    pub product: u32,
    pub weeks: Vec<u32>,
    pub made: Vec<i64>,
    pub sold: Vec<i64>,
    pub imported: Vec<i64>,
    pub exported: Vec<i64>,
    pub stock: Vec<i64>,
    /// Lots the player's companies sold of it.
    pub your_sold: Vec<i64>,
}

/// One weekly or periodic series keyed by an id (a product code or a firm handle).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct KeyedSeriesDto {
    pub id: u64,
    pub weeks: Vec<u32>,
    pub values: Vec<i64>,
}

/// One division's weekly figures, oldest first: its share of the corridor market in basis
/// points, revenue and costs in cents, made and sold in thousandths of a lot.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct DivisionSeriesDto {
    pub region: String,
    pub industry: String,
    pub weeks: Vec<u32>,
    pub share_bps: Vec<i64>,
    pub revenue_cents: Vec<i64>,
    pub costs_cents: Vec<i64>,
    pub made_milli: Vec<i64>,
    pub sold_milli: Vec<i64>,
}

/// Where a new plant would pay (2.0), from the engine's capacity openings. See
/// `cw_session::dto::CapacityOpening`.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CapacityOpeningDto {
    pub recipe_id: String,
    pub industry: String,
    pub region: u32,
    pub region_code: String,
    pub product_id: String,
    pub traded: bool,
    pub room_lots: i64,
    pub block_lots: i64,
    pub price_cents: i64,
    pub price_vs_reference_bps: i64,
    pub unmet_lots: i64,
    pub wanted_lots: i64,
    pub weekly_contribution_cents: i64,
    pub build_cost_cents: i64,
    /// Weeks of contribution to pay back the build, or none when it never pays back.
    pub payback_weeks: Option<i64>,
}

fn tail<T>(items: &[T]) -> &[T] {
    &items[items.len().saturating_sub(GRID_WEEKS)..]
}

#[uniffi::export]
impl SessionHandle {
    /// Every good's weekly made, sold, imported, exported and stock, up to five years.
    pub fn goods_series(&self) -> Vec<GoodSeriesDto> {
        let session = self.lock();
        let weeks = tail(&session.world.systems.weeks);
        let mut out: std::collections::BTreeMap<u16, GoodSeriesDto> = Default::default();
        for w in weeks {
            for g in &w.goods {
                let row = out.entry(g.product).or_insert_with(|| GoodSeriesDto {
                    product: g.product as u32,
                    weeks: Vec::new(),
                    made: Vec::new(),
                    sold: Vec::new(),
                    imported: Vec::new(),
                    exported: Vec::new(),
                    stock: Vec::new(),
                    your_sold: Vec::new(),
                });
                row.weeks.push(w.week);
                row.made.push(g.made);
                row.sold.push(g.sold);
                row.imported.push(g.imported);
                row.exported.push(g.exported);
                row.stock.push(g.stock);
                row.your_sold.push(g.your_sold);
            }
        }
        out.into_values().collect()
    }

    /// Every good's weekly closing price, up to five years, oldest first.
    pub fn goods_price_series(&self) -> Vec<KeyedSeriesDto> {
        let session = self.lock();
        let mut out: std::collections::BTreeMap<u16, KeyedSeriesDto> = Default::default();
        for p in tail(&session.world.archive.prices) {
            for (code, cents) in &p.goods {
                let row = out.entry(*code).or_insert_with(|| KeyedSeriesDto {
                    id: *code as u64,
                    weeks: Vec::new(),
                    values: Vec::new(),
                });
                row.weeks.push(p.week);
                row.values.push(*cents);
            }
        }
        out.into_values().collect()
    }

    /// Every listed company's weekly closing share price, up to five years, oldest first.
    pub fn share_price_series(&self) -> Vec<KeyedSeriesDto> {
        let session = self.lock();
        let mut out: std::collections::BTreeMap<Handle, KeyedSeriesDto> = Default::default();
        for p in tail(&session.world.archive.prices) {
            for (firm, cents) in &p.shares {
                let row = out.entry(*firm).or_insert_with(|| KeyedSeriesDto {
                    id: firm.0,
                    weeks: Vec::new(),
                    values: Vec::new(),
                });
                row.weeks.push(p.week);
                row.values.push(*cents);
            }
        }
        out.into_values().collect()
    }

    /// Each company's closing debt by period, oldest first: `week` gives the last year week by
    /// week, `quarter` the whole career quarter by quarter (the week of `weeks[i]` is the
    /// period's first). Companies on the board and the ones the player may inspect only, the
    /// same rule as `statements`.
    pub fn debt_series(&self, period: String) -> Vec<KeyedSeriesDto> {
        let session = self.lock();
        let world = &session.world;
        let book = if period == "week" {
            &world.archive.weeks
        } else {
            &world.archive.quarters
        };
        book.iter()
            .filter(|(h, _)| {
                world
                    .firms
                    .get(**h)
                    .is_some_and(|f| f.on_board() || session.can_inspect(h.0))
            })
            .map(|(h, records)| KeyedSeriesDto {
                id: h.0,
                weeks: records.iter().map(|r| r.first_week).collect(),
                values: records.iter().map(|r| r.debt).collect(),
            })
            .collect()
    }

    /// A company's divisions week by week, up to five years: share of the corridor market,
    /// revenue and costs (so margin), made and sold. The player's companies and inspected
    /// companies only.
    pub fn division_series(&self, firm: u64) -> Vec<DivisionSeriesDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = Handle(firm);
        let mine = cw_sim::economy::events_engine::player_side(world, handle);
        if !mine && !session.can_inspect(firm) {
            return Vec::new();
        }
        let mut out: std::collections::BTreeMap<(u8, u8), DivisionSeriesDto> = Default::default();
        for w in tail(&world.systems.weeks) {
            for d in w.divisions.iter().filter(|d| d.firm == handle) {
                let row = out
                    .entry((d.region, d.industry))
                    .or_insert_with(|| DivisionSeriesDto {
                        region: dataset
                            .regions
                            .get(d.region as usize)
                            .map(|r| r.code.clone())
                            .unwrap_or_default(),
                        industry: INDUSTRY_NAMES
                            .get(d.industry as usize)
                            .map(|s| (*s).to_owned())
                            .unwrap_or_default(),
                        weeks: Vec::new(),
                        share_bps: Vec::new(),
                        revenue_cents: Vec::new(),
                        costs_cents: Vec::new(),
                        made_milli: Vec::new(),
                        sold_milli: Vec::new(),
                    });
                row.weeks.push(w.week);
                row.share_bps.push(d.share_bps);
                row.revenue_cents.push(d.revenue_cents);
                row.costs_cents.push(d.costs_cents);
                row.made_milli.push(d.made_milli);
                row.sold_milli.push(d.sold_milli);
            }
        }
        out.into_values().collect()
    }

    /// Where a new plant would pay, best first, at most `limit` (2.0). The engine's capacity
    /// openings measured from the player's home region.
    pub fn capacity_openings(&self, limit: u32) -> Vec<CapacityOpeningDto> {
        let session = self.lock();
        session
            .capacity_openings(limit as usize)
            .into_iter()
            .map(|o| CapacityOpeningDto {
                recipe_id: o.recipe_id,
                industry: o.industry,
                region: o.region as u32,
                region_code: o.region_code,
                product_id: o.product_id,
                traded: o.traded,
                room_lots: o.room_lots,
                block_lots: o.block_lots,
                price_cents: o.price_cents,
                price_vs_reference_bps: o.price_vs_reference_bps,
                unmet_lots: o.unmet_lots,
                wanted_lots: o.wanted_lots,
                weekly_contribution_cents: o.weekly_contribution_cents,
                build_cost_cents: o.build_cost_cents,
                payback_weeks: o.payback_weeks,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_last_five_years() {
        let v: Vec<u32> = (0..300).collect();
        assert_eq!(tail(&v).len(), GRID_WEEKS);
        assert_eq!(tail(&v)[0], 40);
        let short: Vec<u32> = (0..10).collect();
        assert_eq!(tail(&short).len(), 10);
    }

    #[test]
    fn the_grid_series_cross_the_boundary() {
        let career = SessionHandle::start(crate::NewGameDto {
            seed: 9,
            scenario_id: "texas-1981".to_owned(),
            guided: true,
            five_year: false,
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            price_path: String::new(),
            timeline: String::new(),
            chief_name: String::new(),
            chief_portrait: String::new(),
            chief_temperament: String::new(),
        })
        .unwrap();
        let mut left = 20;
        while career.overview().tick < 12 && left > 0 {
            career.advance(4);
            career.dismiss_pause();
            left -= 1;
        }
        let played = career.overview().tick as usize;
        let goods = career.goods_series();
        assert!(!goods.is_empty());
        for g in &goods {
            assert!(g.weeks.len() <= played);
            for v in [
                &g.made,
                &g.sold,
                &g.imported,
                &g.exported,
                &g.stock,
                &g.your_sold,
            ] {
                assert_eq!(v.len(), g.weeks.len());
            }
            assert!(g.weeks.windows(2).all(|w| w[0] < w[1]));
        }
        let prices = career.goods_price_series();
        assert!(!prices.is_empty());
        assert!(prices.iter().all(|p| p.weeks.len() == p.values.len()));
        // The same closes the one-good call gives.
        let first = &prices[0];
        let one = career.good_price_history(first.id as u32);
        assert_eq!(
            one.iter().map(|p| p.cents).collect::<Vec<_>>(),
            first.values
        );
        let player = career.overview().primary_firm.expect("a primary company");
        let divisions = career.division_series(player);
        assert!(
            divisions
                .iter()
                .all(|d| d.share_bps.len() == d.weeks.len()
                    && d.revenue_cents.len() == d.weeks.len())
        );
        let debt = career.debt_series("week".to_owned());
        assert!(debt.iter().any(|d| d.id == player));
        let openings = career.capacity_openings(5);
        assert!(openings.len() <= 5);
    }
}
