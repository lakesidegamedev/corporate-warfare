//! Government laws on markets and logistics, the strategic reserve, state rescue, vertical
//! merger review and systemic events (1.7 preview 4). Oracles are the documented rules and
//! conservation of money, not the code's own arithmetic.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;
use cw_sim::economy::logistics::{self as lg, Closure, LaneKey};
use cw_sim::economy::{
    deals, events_engine, insolvency, politics, rescue, reserve, seed, systemic,
};
use cw_sim::state::{FirmStatus, HolderId, Laws, Party, Rescue, ScenarioKind, ScenarioMode, World};

const TRUCK: u8 = 0;

fn world(seed_value: u64) -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Guided,
        ScenarioMode::Sandbox,
    );
    (world, dataset)
}

/// Every cent in the world, as the liquidation tests count it.
fn money(world: &World) -> i64 {
    let firms: i64 = world
        .firms
        .iter()
        .map(|(_, f)| f.book.balance(Account::Cash) + f.book.balance(Account::RestrictedCash))
        .sum();
    firms
        + world.investor.cash_cents
        + world
            .outside
            .investors
            .iter()
            .map(|h| h.capital_cents)
            .sum::<i64>()
        + world
            .outside
            .lenders
            .iter()
            .map(|h| h.capital_cents)
            .sum::<i64>()
        + world.outside.advocacy_book.balance(Account::Cash)
        + world.outside.legal_services_book.balance(Account::Cash)
        + world.outside.gateway_book.balance(Account::Cash)
        + world.government.book.balance(Account::Cash)
}

fn rival(world: &World) -> Handle {
    world
        .firms
        .iter()
        .find(|(h, f)| {
            f.operative()
                && f.bank.is_none()
                && f.insurer.is_none()
                && f.industry != 6
                && cw_sim::economy::ownership::ultimate_controller(world, *h)
                    != Some(HolderId::Investor)
        })
        .map(|(h, _)| h)
        .unwrap()
}

fn add_plant(world: &mut World, dataset: &Dataset, firm: Handle, region: u8, recipe_id: &str) {
    let mut plant = world
        .facilities
        .iter()
        .map(|(_, f)| f.clone())
        .find(|f| f.firm == firm)
        .expect("the firm has a plant to copy");
    plant.region = region;
    plant.recipe = dataset.recipe_index[recipe_id];
    plant.blocks = 1;
    plant.status = cw_sim::state::FacilityStatus::Operating;
    plant.ship = Default::default();
    plant.serves = Vec::new();
    plant.outage = None;
    plant.last_output =
        MilliLots::from_lots(dataset.recipes[plant.recipe as usize].weekly_output_lots);
    let handle = world.facilities.insert(plant);
    world.firms.get_mut(firm).unwrap().facilities.push(handle);
}

#[test]
fn each_party_brings_its_own_market_laws_and_the_starting_government_is_in_force_at_once() {
    let (mut w, ds) = world(3);
    politics::weekly(&mut w, &ds);
    let laws = w.politics.laws;
    assert_eq!(w.politics.party, Party::Business);
    assert!(!laws.common_carrier && !laws.reserve);
    assert_eq!(laws.rescue, Rescue::Bailout);
    assert!(laws.privatise && laws.quota_bps == 8_000);
    assert_ne!(laws.quota_states, 0, "some state specialises in extraction");
    let labour = politics::programme(Party::Labour, &laws, &ds);
    assert!(labour.common_carrier && labour.reserve && labour.vertical_review_bps > 0);
    assert_eq!(labour.rescue, Rescue::Nationalise);
    assert_eq!(labour.quota_states, 0);
    let business = politics::programme(Party::Business, &labour, &ds);
    assert!(!business.common_carrier && business.quota_bps > 0);
}

#[test]
fn common_carrier_caps_the_spot_price_and_the_share_contracts_may_hold() {
    let (mut w, ds) = world(7);
    let owner = rival(&w);
    let holder = w
        .firms
        .iter()
        .find(|(h, f)| *h != owner && f.operative() && f.industry != 6 && f.bank.is_none())
        .map(|(h, _)| h)
        .unwrap();
    let (a, b) = (ds.region_index["OH-CEN"], ds.region_index["OH-NW"]);
    add_plant(&mut w, &ds, owner, a, "carrier");
    let key = LaneKey::new(a, b, TRUCK);
    // A full lane costs the outside premium without the law and the regulated cap with it.
    for law in [false, true] {
        w.politics.laws.common_carrier = law;
        lg::begin_week(&mut w, &ds);
        let cap = w.logistics.lanes[&key].cap();
        {
            let lane = w.logistics.lanes.get_mut(&key).unwrap();
            lane.allocs[0].used = lane.allocs[0].cap;
        }
        let mult = lg::lane_mult_bps(&ds, &w.logistics, key);
        if law {
            assert_eq!(mult, lg::regulated_cap_bps(&ds));
            assert!(mult < ds.logistics.outside_premium_bps);
            assert!(mult > ds.logistics.empty_lane_bps);
            // Contracts hold at most 40 percent of the lane under the law.
            w.logistics.lanes.get_mut(&key).unwrap().allocs[0].used = 0;
            assert!(
                lg::reserve_lane(&mut w, &ds, holder, a, b, TRUCK, cap * 5 / 10, 13).is_err(),
                "half the lane is more than a common carrier may lock up"
            );
            assert!(lg::reserve_lane(&mut w, &ds, holder, a, b, TRUCK, cap * 3 / 10, 13).is_ok());
            assert!(lg::reservable_lots(&w, owner, key) >= 0);
        } else {
            assert_eq!(mult, ds.logistics.outside_premium_bps);
        }
    }
}

#[test]
fn closures_cut_lane_capacity_for_their_weeks_only() {
    let (mut w, ds) = world(11);
    let owner = rival(&w);
    let (a, b) = (ds.region_index["OH-CEN"], ds.region_index["OH-NW"]);
    add_plant(&mut w, &ds, owner, a, "carrier");
    let key = LaneKey::new(a, b, TRUCK);
    w.tick = w.tick.next();
    lg::begin_week(&mut w, &ds);
    let open = w.logistics.lanes[&key].cap();
    assert!(open > 0);
    // A lane outage cuts that lane to a quarter; a region-wide closure takes every lane of the
    // mode that touches the region.
    w.logistics.closures.push(Closure {
        a,
        b,
        mode: TRUCK,
        cap_bps: 2_500,
        until: w.tick.0 + 3,
    });
    lg::begin_week(&mut w, &ds);
    assert_eq!(w.logistics.lanes[&key].cap(), open / 4);
    w.logistics.closures.clear();
    w.logistics.closures.push(Closure {
        a,
        b: a,
        mode: TRUCK,
        cap_bps: 0,
        until: w.tick.0 + 3,
    });
    lg::begin_week(&mut w, &ds);
    assert!(
        w.logistics
            .lanes
            .iter()
            .filter(|(k, _)| k.mode == TRUCK && (k.a == a || k.b == a))
            .all(|(_, l)| l.cap() == 0)
    );
    // Expired, it is gone.
    w.tick = Tick(w.tick.0 + 3);
    lg::begin_week(&mut w, &ds);
    assert!(w.logistics.closures.is_empty());
    assert_eq!(w.logistics.lanes[&key].cap(), open);
}

#[test]
fn quotas_hold_extraction_output_while_the_price_is_soft_and_lift_when_it_recovers() {
    let (mut w, ds) = world(5);
    politics::weekly(&mut w, &ds);
    // The first operating extraction plant in a quota state.
    let (region, product) = w
        .facilities
        .iter()
        .filter(|(_, p)| p.status == cw_sim::state::FacilityStatus::Operating)
        .filter_map(|(_, p)| {
            let r = &ds.recipes[p.recipe as usize];
            let state = politics::state_index(&ds, p.region)?;
            (r.industry == "extraction" && w.politics.laws.quota_states & (1 << state) != 0)
                .then(|| (p.region, ds.product_index[&r.output]))
        })
        .next()
        .expect("an extraction plant in a quota state");
    // A year of history at a high price, then asks well under it.
    let high = ds.reference_price(product) * 2;
    for week in 0..40 {
        w.price_history.push(cw_sim::state::PricePoint {
            week,
            goods: [(product, high)].into_iter().collect(),
            ..Default::default()
        });
    }
    for (_, firm) in w.firms.iter_mut() {
        firm.policies.prices.insert(product, high / 2);
    }
    w.tick = Tick(41);
    politics::weekly(&mut w, &ds);
    assert!(w.politics.quota_active.contains(&(region, product)));
    assert_eq!(w.supply_ratio_bps(region, product), 8_000);
    // Other goods and other regions are untouched.
    let other = (0..ds.products.len() as u16)
        .find(|p| *p != product)
        .unwrap();
    assert_eq!(w.supply_ratio_bps(region, other), 10_000);
    // The price recovers: the quota lifts the same week.
    for (_, firm) in w.firms.iter_mut() {
        firm.policies.prices.insert(product, high);
    }
    w.tick = Tick(42);
    politics::weekly(&mut w, &ds);
    assert!(w.politics.quota_active.is_empty());
    assert_eq!(w.supply_ratio_bps(region, product), 10_000);
    // Repealed, a depressed price does nothing either.
    for (_, firm) in w.firms.iter_mut() {
        firm.policies.prices.insert(product, high / 2);
    }
    w.politics.laws.quota_bps = 0;
    w.tick = Tick(43);
    politics::weekly(&mut w, &ds);
    assert!(w.politics.quota_active.is_empty());
}

fn crude(ds: &Dataset) -> u16 {
    ds.product_index["crude_oil"]
}

#[test]
fn the_reserve_buys_in_a_bust_sells_in_a_spike_and_moves_only_real_money() {
    let (mut w, ds) = world(9);
    let seller = rival(&w);
    let product = crude(&ds);
    assert!(reserve::strategic_goods(&ds).contains(&product));
    let region = w.firms.get(seller).unwrap().region;
    let mean = ds.reference_price(product);
    for week in 0..40 {
        w.price_history.push(cw_sim::state::PricePoint {
            week,
            goods: [(product, mean)].into_iter().collect(),
            ..Default::default()
        });
    }
    w.tick = Tick(41);
    // Law off: nothing happens even in a bust.
    for (_, firm) in w.firms.iter_mut() {
        firm.policies.prices.insert(product, mean / 2);
    }
    w.firms
        .get_mut(seller)
        .unwrap()
        .stock_lot_mut(product, region)
        .receive(
            MilliLots::from_lots(500),
            cw_core::money::Cents(500 * mean / 2),
        );
    assert_eq!(reserve::phase(&w, &ds, product), reserve::Phase::Idle);
    w.politics.laws.reserve = true;
    assert_eq!(reserve::phase(&w, &ds, product), reserve::Phase::Buying);

    let before = money(&w);
    let gov_before = w.government.book.cash();
    let seller_cash = w.firms.get(seller).unwrap().book.cash();
    reserve::weekly(&mut w, &ds);
    let held = w.politics.reserve.lots_of(product);
    assert!(held > 0, "the reserve bought");
    assert!(held <= reserve::BUY_LOTS_PER_WEEK);
    let paid = w.politics.reserve.bought_cents;
    assert_eq!(gov_before - w.government.book.cash(), paid);
    assert_eq!(w.firms.get(seller).unwrap().book.cash() - seller_cash, paid);
    assert_eq!(money(&w), before, "money only changed hands");
    assert_eq!(
        w.government.book.balance(Account::Inventory),
        w.politics.reserve.value_cents(),
        "the reserve's stock is the government's inventory"
    );

    // A modest recovery, well under 120 percent of the year average but over 110 percent of
    // what the reserve paid, is enough to sell.
    for (_, firm) in w.firms.iter_mut() {
        firm.policies.prices.insert(product, mean * 9 / 10);
    }
    assert_eq!(reserve::phase(&w, &ds, product), reserve::Phase::Selling);
    // Nothing held, nothing to sell.
    let saved = std::mem::take(&mut w.politics.reserve);
    assert_eq!(reserve::phase(&w, &ds, product), reserve::Phase::Idle);
    w.politics.reserve = saved;
    // Price spike: the reserve offers its stock just under the going price and a sale pays the
    // government what the buyer pays.
    for (_, firm) in w.firms.iter_mut() {
        firm.policies.prices.insert(product, mean * 2);
    }
    assert_eq!(reserve::phase(&w, &ds, product), reserve::Phase::Selling);
    let (ask, milli) = reserve::release_offer(&w, &ds, product, region).expect("an offer");
    assert!(ask < mean * 2 && milli > 0);
    let gov = w.government.book.cash();
    let qty = MilliLots(milli.min(10_000));
    let cents = qty.value_at(cw_core::money::Cents(ask)).unwrap().0;
    reserve::record_sale(&mut w, product, region, qty, cents);
    assert_eq!(w.government.book.cash() - gov, cents);
    assert_eq!(w.politics.reserve.sold_milli, qty.0);
    assert_eq!(
        w.government.book.balance(Account::Inventory),
        w.politics.reserve.value_cents()
    );
}

/// Make a firm a failing strategic firm: a carrier, with overdue payables past the grace.
fn fail(world: &mut World, ds: &Dataset, firm: Handle, owed: i64) {
    let region = world.firms.get(firm).unwrap().region;
    add_plant(world, ds, firm, region, "carrier");
    // Systemic and viable: a big employer with a profitable last quarter.
    let last = *world.firms.get(firm).unwrap().facilities.last().unwrap();
    world.facilities.get_mut(last).unwrap().workers = [5_000, 5_000, 5_000];
    for week in 0..13 {
        let book = &mut world.firms.get_mut(firm).unwrap().book;
        let t = Tick(week);
        book.commit(
            t,
            Txn::new()
                .debit(Account::Cash, 5_000_000, "test sale")
                .credit(Account::Revenue, 5_000_000, "test sale"),
        )
        .unwrap();
        let _ = book.close_week(t);
    }
    insolvency::defer_expense(world, firm, owed, Account::OperatingExpense, "test arrears");
    let f = world.firms.get_mut(firm).unwrap();
    f.past_due_weeks = cw_sim::state::PAST_DUE_GRACE_WEEKS + 1;
}

#[test]
fn a_bailout_clears_the_arrears_with_a_state_loan_and_conserves_money() {
    let (mut w, ds) = world(21);
    w.tick = Tick(10);
    let firm = rival(&w);
    w.politics.laws.rescue = Rescue::Bailout;
    let owed = 40_000_000;
    // The government can afford it.
    let _ = w.government.book.commit(
        w.tick,
        Txn::new()
            .debit(Account::Cash, 1_000_000_000, "test")
            .credit(Account::Bonds, 1_000_000_000, "test"),
    );
    fail(&mut w, &ds, firm, owed);
    let before = money(&w);
    let gov = w.government.book.cash();
    let events = insolvency::settle(&mut w, &ds);
    assert!(events.contains(&(firm, "rescued")), "{events:?}");
    let f = w.firms.get(firm).unwrap();
    assert_eq!(f.status, FirmStatus::Operating);
    assert_eq!(-f.book.balance(Account::OverduePayables), 0);
    assert!(
        f.loans.iter().any(|l| matches!(
            w.loans.get(*l).map(|l| l.lender),
            Some(cw_sim::state::Lender::Outside(cw_sim::state::STATE_LENDER))
        )),
        "a state loan"
    );
    assert!(w.government.book.cash() < gov, "the state lent real cash");
    assert_eq!(money(&w), before);
    assert_eq!(w.politics.bailouts, 1);
    // Not again within a year.
    assert!(f.rescued_week > 0);
}

#[test]
fn nationalisation_pays_private_holders_and_leaves_the_state_in_control() {
    let (mut w, ds) = world(22);
    let firm = rival(&w);
    w.politics.laws.rescue = Rescue::Nationalise;
    let _ = w.government.book.commit(
        w.tick,
        Txn::new()
            .debit(Account::Cash, 5_000_000_000, "test")
            .credit(Account::Bonds, 5_000_000_000, "test"),
    );
    fail(&mut w, &ds, firm, 40_000_000);
    let issued_before = w.firms.get(firm).unwrap().shares.issued;
    let before = money(&w);
    let events = insolvency::settle(&mut w, &ds);
    assert!(events.contains(&(firm, "rescued")), "{events:?}");
    let f = w.firms.get(firm).unwrap();
    assert!(f.state_owned_since.is_some());
    assert_eq!(f.status, FirmStatus::Operating);
    assert_eq!(f.shares.controller(), Some(HolderId::State));
    // Every private holding moved to the state; only new shares were issued, for the cash put in.
    assert_eq!(
        f.shares
            .holders
            .keys()
            .filter(|h| **h != HolderId::State && !h.is_treasury())
            .count(),
        0
    );
    assert!(f.shares.issued > issued_before);
    assert_eq!(
        f.shares.holders.values().map(|l| l.qty).sum::<i64>(),
        f.shares.issued
    );
    assert_eq!(
        money(&w),
        before,
        "compensation and recapitalisation are real transfers"
    );
    assert_eq!(w.politics.nationalisations, 1);
}

#[test]
fn a_firm_the_investor_controls_gets_a_loan_never_a_takeover() {
    let (mut w, ds) = world(23);
    let mine = w
        .investor
        .primary_group
        .expect("a guided career has a company");
    w.politics.laws.rescue = Rescue::Nationalise;
    let _ = w.government.book.commit(
        w.tick,
        Txn::new()
            .debit(Account::Cash, 5_000_000_000, "test")
            .credit(Account::Bonds, 5_000_000_000, "test"),
    );
    fail(&mut w, &ds, mine, 40_000_000);
    let events = insolvency::settle(&mut w, &ds);
    assert!(events.contains(&(mine, "rescued")), "{events:?}");
    let f = w.firms.get(mine).unwrap();
    assert!(f.state_owned_since.is_none());
    assert_ne!(f.shares.controller(), Some(HolderId::State));
    assert_eq!(w.politics.bailouts, 1);
}

#[test]
fn without_the_cash_or_the_law_the_court_proceeds() {
    let (mut w, ds) = world(24);
    let firm = rival(&w);
    fail(&mut w, &ds, firm, 40_000_000);
    assert!(!rescue::try_rescue(&mut w, &ds, firm), "no rescue law");
    w.politics.laws.rescue = Rescue::Bailout;
    // The treasury cannot spend more than 30 percent of its cash on one rescue.
    let gov = w.government.book.cash();
    insolvency::defer_expense(&mut w, firm, gov, Account::OperatingExpense, "huge");
    assert!(
        !rescue::try_rescue(&mut w, &ds, firm),
        "too dear for the treasury"
    );
}

#[test]
fn a_sound_state_firm_is_sold_back_under_a_privatising_government() {
    let (mut w, ds) = world(25);
    let firm = rival(&w);
    w.politics.laws.rescue = Rescue::Nationalise;
    let _ = w.government.book.commit(
        w.tick,
        Txn::new()
            .debit(Account::Cash, 5_000_000_000, "test")
            .credit(Account::Bonds, 5_000_000_000, "test"),
    );
    fail(&mut w, &ds, firm, 40_000_000);
    insolvency::settle(&mut w, &ds);
    assert!(rescue::is_state_owned(&w, firm));
    // A profitable year held.
    w.tick = Tick(w.tick.0 + 60);
    for week in 0..13 {
        let book = &mut w.firms.get_mut(firm).unwrap().book;
        let t = Tick(w.tick.0 + week);
        book.commit(
            t,
            Txn::new()
                .debit(Account::Cash, 5_000_000, "test sale")
                .credit(Account::Revenue, 5_000_000, "test sale"),
        )
        .unwrap();
        let _ = book.close_week(t);
    }
    for investor in w.outside.investors.iter_mut() {
        investor.capital_cents = 1_000_000_000_000;
    }
    let before = money(&w);
    // Not while the government keeps firms.
    w.politics.laws.privatise = false;
    rescue::weekly(&mut w, &ds);
    assert!(rescue::is_state_owned(&w, firm));
    w.politics.laws.privatise = true;
    let gov = w.government.book.cash();
    rescue::weekly(&mut w, &ds);
    assert!(!rescue::is_state_owned(&w, firm), "sold back");
    assert!(w.government.book.cash() > gov);
    assert_eq!(money(&w), before);
    assert_eq!(w.politics.privatisations, 1);
}

#[test]
fn vertical_deals_are_reviewed_only_when_the_law_asks_and_the_supplier_dominates() {
    let (mut w, ds) = world(31);
    let pair: Vec<Handle> = w
        .firms
        .iter()
        .filter(|(h, f)| {
            f.operative()
                && cw_sim::economy::ownership::ultimate_controller(&w, *h)
                    != Some(HolderId::Investor)
        })
        .map(|(h, _)| h)
        .take(2)
        .collect();
    let (maker, user) = (pair[0], pair[1]);
    let product = ds.product_index["steel"];
    let region = w.firms.get(maker).unwrap().region;
    // The maker is the only seller of a good the user's plants consume.
    for (h, f) in w.firms.iter_mut() {
        f.sales_history.clear();
        if h == maker {
            f.sales_history
                .push([((region, product), 100)].into_iter().collect());
        }
    }
    w.firms.get_mut(user).unwrap().inputs_used = vec![product];
    w.politics.laws.vertical_review_bps = 0;
    assert_eq!(
        deals::vertical_share_bps(&w, user, maker),
        0,
        "not reviewed"
    );
    w.politics.laws.vertical_review_bps = 3_500;
    assert_eq!(deals::vertical_share_bps(&w, user, maker), 10_000);
    // The link works from either side.
    assert_eq!(deals::vertical_share_bps(&w, maker, user), 10_000);
    // A user that does not consume the good is not a vertical buyer.
    w.firms.get_mut(user).unwrap().inputs_used = vec![];
    assert_eq!(deals::vertical_share_bps(&w, user, maker), 0);
}

#[test]
fn congestion_cards_come_from_lane_fill_data() {
    let (mut w, ds) = world(41);
    let firm = rival(&w);
    let key = LaneKey::new(ds.region_index["OH-CEN"], ds.region_index["OH-NW"], TRUCK);
    assert!(events_engine::congested_lane(&w, firm).is_none());
    let stat = lg::LaneStat {
        fill_bps: 9_500,
        hot_weeks: 4,
        users: vec![(firm, w.tick.0)],
        ..Default::default()
    };
    w.logistics.stats.insert(key, stat);
    assert_eq!(events_engine::congested_lane(&w, firm), Some(key));
    // A firm that does not ship there, a lane under 90 percent and a lane hot for under four
    // weeks do not count.
    let other = w.firms.iter().map(|(h, _)| h).find(|h| *h != firm).unwrap();
    assert!(events_engine::congested_lane(&w, other).is_none());
    w.logistics.stats.get_mut(&key).unwrap().fill_bps = 8_900;
    assert!(events_engine::congested_lane(&w, firm).is_none());
    w.logistics.stats.get_mut(&key).unwrap().fill_bps = 9_500;
    w.logistics.stats.get_mut(&key).unwrap().hot_weeks = 3;
    assert!(events_engine::congested_lane(&w, firm).is_none());
    // And the weekly trigger opens the card for the shipper once.
    w.logistics.stats.get_mut(&key).unwrap().hot_weeks = 4;
    events_engine::state_triggers(&mut w, &ds);
    assert!(
        w.events
            .pending
            .iter()
            .any(|e| e.card == "freight_congestion" && e.subject_firm == Some(firm))
    );
    let count = |w: &World| {
        w.events
            .pending
            .iter()
            .filter(|e| e.card == "freight_congestion" && e.subject_firm == Some(firm))
            .count()
    };
    events_engine::state_triggers(&mut w, &ds);
    assert_eq!(count(&w), 1, "the 26-week cooldown keeps it from reopening");
    // The random quarterly draw no longer carries this card.
    assert!(
        !w.events
            .pending
            .iter()
            .any(|e| e.card == "freight_congestion" && e.subject_firm != Some(firm))
    );
}

#[test]
fn systemic_events_strike_deterministically_and_through_a_save() {
    let (mut a, ds) = world(51);
    let mut b = a.clone();
    // Run a world for three years, snapshot it through serde mid-way and continue both.
    let mut copy: Option<World> = None;
    for week in 1..=156 {
        cw_sim::tick::advance(&mut a, &ds);
        if week == 70 {
            let bytes = postcard::to_allocvec(&a).unwrap();
            copy = Some(postcard::from_bytes(&bytes).unwrap());
        }
    }
    let mut c = copy.unwrap();
    for _ in 71..=156 {
        cw_sim::tick::advance(&mut c, &ds);
    }
    assert_eq!(
        postcard::to_allocvec(&a).unwrap(),
        postcard::to_allocvec(&c).unwrap(),
        "a saved and reloaded world diverges from the continued one"
    );
    for _ in 1..=156 {
        cw_sim::tick::advance(&mut b, &ds);
    }
    assert_eq!(a.systemic, b.systemic, "same seed, same incidents");
    // Over a few seeds and years every kind that can strike here does strike somewhere.
    let mut totals = [0u32; systemic::KINDS];
    for seed_value in 60..63 {
        let (mut w, ds) = world(seed_value);
        for _ in 0..156 {
            cw_sim::tick::advance(&mut w, &ds);
        }
        for (i, n) in w.systemic.counts.iter().enumerate() {
            totals[i] += n;
        }
    }
    assert!(
        totals.iter().sum::<u32>() > 0,
        "incidents happen over 3 worlds and 3 years: {totals:?}"
    );
}

#[test]
fn a_laws_default_is_neutral_and_old_saves_load() {
    let laws = Laws::default();
    assert!(!laws.common_carrier && !laws.reserve && laws.rescue == Rescue::None);
    assert_eq!(laws.quota_bps, 0);
    // The new fields default when absent from an older save.
    let json = r#"{"corporate_tax_bps":2500,"wage_floor_cents":34000,"merger_review_bps":3500,"unions":"Standard"}"#;
    let old: Laws = serde_json::from_str(json).unwrap();
    assert_eq!(old, laws);
}
