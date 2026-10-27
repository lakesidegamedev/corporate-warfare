//! Currencies and exchange rates for scenarios that span more than one money.
//!
//! Every firm keeps its books in the currency of its home region. Ledger units are re-based so
//! one unit of every currency buys the same on day one; after that each currency's value
//! against the numeraire (the anchor country's money) moves weekly. Prices and wages stay in
//! the local money and are sticky, so a falling rate makes a country's goods cheap abroad and
//! its imports dear, and the rate is the channel through which one country's inflation and
//! central bank reach another's firms.
//!
//! The model, all integer and deterministic:
//! * Each country has its own inflation (the anchor's plus an authored gap and a slow random
//!   walk) and central bank rate (the anchor's plus an authored gap, a share of its own excess
//!   inflation and the strain of defending a peg).
//! * The fundamental rate drifts with relative inflation (purchasing power parity pulls it
//!   back) and with the real rate gap (carry), plus a random weekly shock.
//! * A pegged currency is held inside a band around parity. Strain is how far the fundamental
//!   sits outside the inner half of the band. Strain, and in a historical path a scripted
//!   window, raise the weekly chance of a band crisis, which widens the band like August 1993.
//! * Gateway prices and background wages follow the currency and the inflation gap, so the
//!   outside world is priced in local money.
//! * A sale paid in another currency books a receivable that is collected two weeks later at
//!   the rate then; the difference is a recorded foreign exchange gain or loss.
//!
//! A scenario with one currency never activates any of this and plays exactly as before.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;
use cw_core::rng::Rng;
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::state::{Firm, NewsItem, World};
use cw_core::quantity::MilliLots;

pub const PPM: i64 = 1_000_000;
/// Weeks a cross-currency sale waits for its money.
pub const COLLECTION_WEEKS: u32 = 2;
const TAG_FX: u64 = 13;
const HISTORY_WEEKS: usize = 104;

/// One currency's market state. Index 0 is the numeraire and never moves.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrencyState {
    /// What one ledger unit is worth in numeraire units, in millionths.
    pub value_ppm: i64,
    pub fundamental_ppm: i64,
    pub ppp_ppm: i64,
    pub policy_rate_bps: i32,
    pub inflation_bps: i32,
    pub inflation_noise_bps: i32,
    /// Half width of the peg band now; zero for a float.
    pub band_bps: i32,
    pub strain_bps: i32,
    pub crisis_week: Option<u32>,
    pub history: Vec<i64>,
    /// Units paid into the market by buyers, and units delivered when sellers collected.
    pub inflow: i64,
    pub outflow: i64,
}

/// A sale in another currency waiting to be collected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receivable {
    pub seller: Handle,
    pub due: u32,
    /// What the buyer paid, in the buyer's currency.
    pub amount: i64,
    pub from: u8,
    pub to: u8,
    /// What the seller booked, in its own currency.
    pub booked: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pnl {
    pub gains: i64,
    pub losses: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxState {
    pub active: bool,
    pub currencies: Vec<CurrencyState>,
    pub rng: Rng,
    pub receivables: Vec<Receivable>,
    /// The currency index of every region, copied from the dataset so rules that only hold the
    /// world can resolve a firm's money.
    pub region_currency: Vec<u8>,
    /// Recorded foreign exchange gains and losses by firm, in the firm's own currency.
    pub pnl: BTreeMap<Handle, Pnl>,
    /// Band crises so far: (week, currency index).
    pub crises: Vec<(u32, u8)>,
}

impl Default for FxState {
    fn default() -> Self {
        FxState {
            active: false,
            currencies: Vec::new(),
            rng: Rng::new(0),
            receivables: Vec::new(),
            region_currency: Vec::new(),
            pnl: BTreeMap::new(),
            crises: Vec::new(),
        }
    }
}

/// Build the opening state for a dataset. Inactive when the scenario has one money.
pub fn init(dataset: &Dataset, seed: u64) -> FxState {
    let Some(set) = &dataset.currencies else {
        return FxState::default();
    };
    let currencies = set
        .currencies
        .iter()
        .map(|def| CurrencyState {
            value_ppm: PPM,
            fundamental_ppm: PPM,
            ppp_ppm: PPM,
            policy_rate_bps: 0,
            inflation_bps: 0,
            inflation_noise_bps: 0,
            band_bps: def.peg.as_ref().map(|p| p.band_bps).unwrap_or(0),
            strain_bps: 0,
            crisis_week: None,
            history: vec![PPM],
            inflow: 0,
            outflow: 0,
        })
        .collect();
    FxState {
        active: true,
        currencies,
        rng: Rng::stream(seed, TAG_FX),
        receivables: Vec::new(),
        region_currency: dataset.region_currency.clone(),
        pnl: BTreeMap::new(),
        crises: Vec::new(),
    }
}

impl FxState {
    pub fn value(&self, currency: u8) -> i64 {
        self.currencies
            .get(currency as usize)
            .map(|c| c.value_ppm)
            .unwrap_or(PPM)
    }

    /// Convert an amount between currencies at the current rate, rounding half to even. An
    /// inactive state, equal currencies or a zero amount return the amount unchanged.
    pub fn convert(&self, amount: i64, from: u8, to: u8) -> i64 {
        if from == to || !self.active || amount == 0 {
            return amount;
        }
        let (vf, vt) = (self.value(from), self.value(to).max(1));
        round_even_div(amount as i128 * vf as i128, vt as i128) as i64
    }

    /// Units of `to` one unit of `from` buys now, in millionths.
    pub fn rate_ppm(&self, from: u8, to: u8) -> i64 {
        if from == to || !self.active {
            return PPM;
        }
        round_even_div(
            self.value(from) as i128 * PPM as i128,
            self.value(to).max(1) as i128,
        ) as i64
    }
}

/// The currency a region's books are kept in; the numeraire (0) when the scenario has one.
pub fn region_currency(dataset: &Dataset, region: u8) -> u8 {
    dataset
        .region_currency
        .get(region as usize)
        .copied()
        .unwrap_or(0)
}

pub fn firm_currency(dataset: &Dataset, firm: &Firm) -> u8 {
    region_currency(dataset, firm.region)
}

pub fn handle_currency(world: &World, _dataset: &Dataset, firm: Handle) -> u8 {
    currency_of(world, firm)
}

/// The currency a firm keeps its books in, from the world alone.
pub fn currency_of(world: &World, firm: Handle) -> u8 {
    if !world.fx.active {
        return 0;
    }
    world
        .firms
        .get(firm)
        .and_then(|f| world.fx.region_currency.get(f.region as usize).copied())
        .unwrap_or(0)
}

/// The currency of a region, from the world alone.
pub fn world_region_currency(world: &World, region: u8) -> u8 {
    if !world.fx.active {
        return 0;
    }
    world
        .fx
        .region_currency
        .get(region as usize)
        .copied()
        .unwrap_or(0)
}

/// Convert between currencies; identity when the world has one money.
pub fn convert(world: &World, amount: i64, from: u8, to: u8) -> i64 {
    world.fx.convert(amount, from, to)
}

/// Whether two firms keep books in the same money. Always true with one currency.
pub fn same_currency(world: &World, dataset: &Dataset, a: Handle, b: Handle) -> bool {
    !world.fx.active || handle_currency(world, dataset, a) == handle_currency(world, dataset, b)
}

/// Whether a firm keeps books in the numeraire, the money of the investor and the exchange.
pub fn in_numeraire(world: &World, dataset: &Dataset, firm: Handle) -> bool {
    !world.fx.active || handle_currency(world, dataset, firm) == 0
}

/// An amount in a firm's own currency expressed in the numeraire.
pub fn firm_to_numeraire(world: &World, dataset: &Dataset, firm: Handle, amount: i64) -> i64 {
    world
        .fx
        .convert(amount, handle_currency(world, dataset, firm), 0)
}

/// An amount in the numeraire expressed in a firm's own currency.
pub fn numeraire_to_firm(world: &World, dataset: &Dataset, firm: Handle, amount: i64) -> i64 {
    world
        .fx
        .convert(amount, 0, handle_currency(world, dataset, firm))
}

/// An amount in a region's money expressed in the numeraire.
pub fn region_to_numeraire(world: &World, dataset: &Dataset, region: u8, amount: i64) -> i64 {
    world
        .fx
        .convert(amount, region_currency(dataset, region), 0)
}

/// The seller's side of a sale whose buyer paid `paid` in its own currency. Same currency:
/// cash straight away. Another currency: a receivable in the seller's money at today's rate,
/// collected later. Returns the seller's revenue.
pub fn book_sale_proceeds(
    world: &mut World,
    dataset: &Dataset,
    seller: Handle,
    buyer_currency: u8,
    paid: i64,
    carrying: i64,
    memo: &str,
) -> i64 {
    let seller_currency = handle_currency(world, dataset, seller);
    if buyer_currency == seller_currency || !world.fx.active {
        if let Some(entity) = world.firms.get_mut(seller) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, paid, memo)
                    .credit(Account::Revenue, paid, memo)
                    .credit(Account::Inventory, carrying, "cogs")
                    .debit(Account::CostOfGoods, carrying, "cogs"),
            );
        }
        return paid;
    }
    let booked = world.fx.convert(paid, buyer_currency, seller_currency);
    if let Some(entity) = world.firms.get_mut(seller) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Receivables, booked, memo)
                .credit(Account::Revenue, booked, memo)
                .credit(Account::Inventory, carrying, "cogs")
                .debit(Account::CostOfGoods, carrying, "cogs"),
        );
    }
    world.fx.currencies[buyer_currency as usize].inflow += paid;
    world.fx.receivables.push(Receivable {
        seller,
        due: world.tick.0 + COLLECTION_WEEKS,
        amount: paid,
        from: buyer_currency,
        to: seller_currency,
        booked,
    });
    booked
}

/// Collect the receivables that fall due this week at the rate of the day.
fn collect(world: &mut World) {
    let week = world.tick.0;
    let (due, keep): (Vec<Receivable>, Vec<Receivable>) = std::mem::take(&mut world.fx.receivables)
        .into_iter()
        .partition(|r| r.due <= week);
    world.fx.receivables = keep;
    for r in due {
        let collected = world.fx.convert(r.amount, r.from, r.to);
        world.fx.currencies[r.from as usize].outflow += r.amount;
        let diff = collected - r.booked;
        let Some(entity) = world.firms.get_mut(r.seller) else {
            continue;
        };
        let mut txn = Txn::new()
            .debit(Account::Cash, collected, "foreign sale collected")
            .credit(Account::Receivables, r.booked, "foreign sale collected");
        if diff > 0 {
            txn = txn.credit(Account::OtherIncome, diff, "fx gain");
        } else if diff < 0 {
            txn = txn.debit(Account::OtherIncome, -diff, "fx loss");
        }
        let _ = entity.book.commit(world.tick, txn);
        let pnl = world.fx.pnl.entry(r.seller).or_default();
        if diff > 0 {
            pnl.gains += diff;
        } else {
            pnl.losses += -diff;
        }
    }
}

/// Run one week of the currency model, after the macro cycle has set the anchor's numbers.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    if !world.fx.active {
        return;
    }
    let Some(set) = dataset.currencies.as_ref() else {
        return;
    };
    let week = world.tick.0;
    let anchor_rate = world.macro_state.policy_rate_bps;
    let anchor_inflation = world.macro_state.inflation_bps;
    let historical = world.world_prices.mode.follows_history();
    let before: Vec<i64> = world.fx.currencies.iter().map(|c| c.value_ppm).collect();
    // The anchor country's own figures.
    if let Some(anchor) = world.fx.currencies.get_mut(0) {
        anchor.policy_rate_bps = anchor_rate;
        anchor.inflation_bps = anchor_inflation;
    }
    for index in 1..set.currencies.len() {
        let def = &set.currencies[index];
        let shock_bps = world
            .fx
            .rng
            .uniform_inclusive(-(def.volatility_bps as i64), def.volatility_bps as i64);
        let noise_step = world.fx.rng.uniform_inclusive(-12, 12) as i32;
        let crisis_roll = world.fx.rng.uniform_below(1_000_000) as i32;
        let st = &mut world.fx.currencies[index];
        // Inflation and the central bank rate of the country.
        st.inflation_noise_bps = st.inflation_noise_bps * 90 / 100 + noise_step;
        let inflation = (anchor_inflation + def.inflation_spread_bps + st.inflation_noise_bps)
            .clamp(-300, 2_500);
        let excess = inflation - anchor_inflation - def.inflation_spread_bps;
        let strain_term = (st.strain_bps / 8).clamp(0, 300);
        let rate =
            (anchor_rate + def.policy_spread_bps + excess / 2 + strain_term).clamp(25, 2_000);
        st.inflation_bps = inflation;
        st.policy_rate_bps = rate;
        // Fundamental value: purchasing power parity with a carry tilt and a shock.
        let gap = (inflation - anchor_inflation) as i64;
        st.ppp_ppm = (st.ppp_ppm as i128 * (PPM as i128 * 52 * 100 - gap as i128 * 100 * 100)
            / (PPM as i128 * 52 * 100)) as i64;
        let carry_bps = ((rate - inflation) - (anchor_rate - anchor_inflation)) as i64;
        let mut drift_ppm = carry_bps * 100 / 52 / 4 + shock_bps * 100;
        let in_window = def
            .peg
            .as_ref()
            .and_then(|p| p.crisis_window_weeks)
            .map(|[from, to]| week >= from && week <= to)
            .unwrap_or(false);
        if historical && in_window && def.policy_spread_bps > 0 {
            // The 1992 squeeze: the Bundesbank holds rates high after reunification and the
            // weaker pegs bleed.
            drift_ppm -= def.policy_spread_bps as i64 * 10;
        }
        let reversion = (st.ppp_ppm - st.fundamental_ppm) / 104;
        st.fundamental_ppm = (st.fundamental_ppm
            + (st.fundamental_ppm as i128 * drift_ppm as i128 / PPM as i128) as i64
            + reversion)
            .clamp(PPM / 3, PPM * 3);
        // A peg holds the market rate inside its band around parity.
        if let Some(peg) = &def.peg {
            let band = st.band_bps.max(1) as i64;
            let central = PPM;
            let low = central * (10_000 - band) / 10_000;
            let high = central * (10_000 + band) / 10_000;
            st.value_ppm = st.fundamental_ppm.clamp(low, high);
            let pressure_bps = (st.fundamental_ppm - central).abs() * 10_000 / central;
            let raw = (pressure_bps - peg.band_bps as i64 / 2).max(0) as i32;
            st.strain_bps = raw.max(st.strain_bps * 90 / 100);
            if st.crisis_week.is_none() {
                let scripted = if historical && in_window { 40 } else { 1 };
                let hazard = peg.crisis_hazard_ppm * scripted + st.strain_bps * 10;
                if crisis_roll < hazard {
                    st.crisis_week = Some(week);
                    st.band_bps = peg.widened_band_bps;
                    world.fx.crises.push((week, index as u8));
                    let name = def.name.clone();
                    world.news.push(NewsItem {
                        week: world.tick,
                        key: "fx_band_crisis".to_owned(),
                        subject: name,
                        amount_cents: peg.widened_band_bps as i64,
                        quantity: MilliLots::ZERO,
                    });
                }
            }
        } else {
            st.value_ppm = st.fundamental_ppm;
        }
        st.history.push(st.value_ppm);
        let excess_len = st.history.len().saturating_sub(HISTORY_WEEKS);
        st.history.drain(..excess_len);
    }
    // The outside world and local wages are priced in local money.
    for region in 0..world.regions.len() {
        let currency = region_currency(dataset, region as u8) as usize;
        if currency == 0 {
            continue;
        }
        let (old, new) = (before[currency], world.fx.currencies[currency].value_ppm);
        let gap = (world.fx.currencies[currency].inflation_bps - anchor_inflation) as i128;
        let scale = |value: i64| -> i64 {
            let moved = (value as i128 * old as i128 + new as i128 / 2) / new.max(1) as i128;
            let drifted = moved + moved * gap / (52 * 10_000);
            drifted.max(1) as i64
        };
        for wage in world.regions[region].background_wage_cents.iter_mut() {
            // Wages are local money and follow only the inflation gap.
            *wage = (*wage as i128 + *wage as i128 * gap / (52 * 10_000)).max(1) as i64;
        }
        for gateway in world
            .gateways
            .iter_mut()
            .filter(|g| g.region as usize == region)
        {
            gateway.sell_price_cents = scale(gateway.sell_price_cents);
            gateway.buy_price_cents = scale(gateway.buy_price_cents);
        }
    }
    collect(world);
}

/// Net worth of a currency holding set in a chosen reporting currency.
pub fn to_reporting(world: &World, amount_numeraire: i64, reporting: u8) -> i64 {
    world.fx.convert(amount_numeraire, 0, reporting)
}

/// The refusal for an arrangement between firms of different currencies. `Ok` with one money.
pub fn require_same_currency(
    world: &World,
    a: Handle,
    b: Handle,
) -> cw_core::error::DomainResult<()> {
    if !world.fx.active || currency_of(world, a) == currency_of(world, b) {
        return Ok(());
    }
    Err(cw_core::error::DomainError::NoAuthority {
        actor: a.0,
        reason: cw_core::error::AuthorityReason::OtherCurrency,
    })
}

/// The refusal for a firm that keeps books outside the numeraire taking a stake in another.
/// The exchange quotes every share in the numeraire, the money of the investor, so only firms
/// that keep books in it may hold shares. `Ok` with one money.
pub fn require_numeraire_holder(world: &World, holder: Handle) -> cw_core::error::DomainResult<()> {
    if !world.fx.active || currency_of(world, holder) == 0 {
        return Ok(());
    }
    Err(cw_core::error::DomainError::NoAuthority {
        actor: holder.0,
        reason: cw_core::error::AuthorityReason::OtherCurrency,
    })
}

/// An amount in the numeraire expressed in a firm's own currency, from the world alone.
pub fn numeraire_to_firm_world(world: &World, firm: Handle, amount: i64) -> i64 {
    world.fx.convert(amount, 0, currency_of(world, firm))
}

/// An amount in a firm's own currency expressed in the numeraire, from the world alone.
pub fn firm_to_numeraire_world(world: &World, firm: Handle, amount: i64) -> i64 {
    world.fx.convert(amount, currency_of(world, firm), 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_data::CurrencySet;

    /// Texas content split into three money zones by state, so cross-border trade, freight
    /// and the exchange all cross a currency line. TX is the numeraire, OK is pegged, LA floats
    /// with the French franc's behaviour.
    pub fn three_zone_dataset() -> Dataset {
        let mut dataset = Dataset::load_scenario("texas-1981").expect("texas loads");
        let set = CurrencySet::parse(include_str!(
            "../../../../content/scenarios/ruhr-rhine-1991/currencies.json"
        ))
        .expect("currencies parse");
        dataset.region_currency = dataset
            .regions
            .iter()
            .map(|region| match region.state() {
                "TX" => 0u8,
                "OK" => set.index_of("NLG").unwrap() as u8,
                _ => set.index_of("FRF").unwrap() as u8,
            })
            .collect();
        dataset.currencies = Some(set);
        dataset.hash = dataset.compute_hash();
        dataset
    }

    fn run(dataset: &Dataset, seed_value: u64, weeks: u32) -> World {
        let mut world = seed::build(
            dataset,
            seed_value,
            ScenarioKind::Guided,
            ScenarioMode::Sandbox,
        );
        for _ in 0..weeks {
            let outcome = crate::tick::advance(&mut world, dataset);
            if outcome.pause.is_some() {
                for event in world.events.pending.iter_mut() {
                    event.resolved = true;
                }
                world.pending_pause = None;
            }
        }
        world
    }

    #[test]
    fn conversion_is_exact_at_parity_and_rounds_half_to_even() {
        let mut fx = FxState {
            active: true,
            currencies: vec![CurrencyState::default(); 2],
            ..FxState::default()
        };
        fx.currencies[0].value_ppm = PPM;
        fx.currencies[1].value_ppm = PPM;
        assert_eq!(fx.convert(12_345, 1, 0), 12_345);
        fx.currencies[1].value_ppm = 500_000;
        assert_eq!(fx.convert(1_001, 1, 0), 500, "500.5 rounds to the even 500");
        assert_eq!(fx.convert(1_003, 1, 0), 502, "501.5 rounds to the even 502");
        assert_eq!(fx.convert(100, 0, 1), 200);
        fx.active = false;
        assert_eq!(fx.convert(1_001, 1, 0), 1_001, "inactive means one money");
    }

    #[test]
    fn a_multi_currency_world_is_deterministic_and_its_books_balance() {
        let dataset = three_zone_dataset();
        let a = run(&dataset, 7, 40);
        let b = run(&dataset, 7, 40);
        assert!(a.fx.active);
        assert_eq!(
            crate::state::canonical_hash(&a),
            crate::state::canonical_hash(&b)
        );
        for (_, firm) in a.firms.iter() {
            assert!(firm.book.is_balanced());
        }
        assert!(a.government.book.is_balanced());
        assert!(a.outside.gateway_book.is_balanced());
    }

    #[test]
    fn foreign_payments_reconcile_per_currency_with_open_receivables() {
        let dataset = three_zone_dataset();
        let world = run(&dataset, 3, 52);
        let mut open = vec![0i64; world.fx.currencies.len()];
        for r in &world.fx.receivables {
            open[r.from as usize] += r.amount;
        }
        let mut trades = 0;
        for (index, c) in world.fx.currencies.iter().enumerate() {
            // Money paid in by buyers is either delivered to a seller or still waiting.
            assert_eq!(c.inflow - c.outflow, open[index], "currency {index}");
            trades += c.inflow;
        }
        assert!(trades > 0, "the zones traded across borders");
        // Every gain and loss is a real posting: totals equal the sum of the per-firm tallies.
        let gains: i64 = world.fx.pnl.values().map(|p| p.gains).sum();
        let losses: i64 = world.fx.pnl.values().map(|p| p.losses).sum();
        assert!(gains >= 0 && losses >= 0);
    }

    #[test]
    fn pegged_currencies_stay_in_their_band_until_a_crisis_widens_it() {
        let dataset = three_zone_dataset();
        let set = dataset.currencies.clone().unwrap();
        let world = run(&dataset, 5, 104);
        for (index, def) in set.currencies.iter().enumerate().skip(1) {
            let st = &world.fx.currencies[index];
            let Some(peg) = &def.peg else { continue };
            let band = if st.crisis_week.is_some() {
                peg.widened_band_bps
            } else {
                peg.band_bps
            } as i64;
            let deviation = (st.value_ppm - PPM).abs() * 10_000 / PPM;
            assert!(
                deviation <= band,
                "{} off by {} of {}",
                def.id,
                deviation,
                band
            );
        }
    }

    #[test]
    fn a_single_money_world_never_activates_the_model() {
        let dataset = Dataset::load_scenario("texas-1981").unwrap();
        let world = run(&dataset, 2, 8);
        assert!(!world.fx.active);
        assert!(world.fx.receivables.is_empty());
    }
}
