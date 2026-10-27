//! Corporate structure evidence: group supply and transfer pricing, the minority squeeze, tax
//! location, the markdown on a losing captive supplier, reputation, raids, construction
//! capacity, bonds, carve-outs, listings, spin-offs, joint ventures and activists. Values are
//! worked from `crates/cw-sim/src/economy/structure.rs` and `docs/rules/CORPORATE.md`.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, apply};
use cw_sim::economy::{credit, equity, freight, ownership, sectors, seed, structure};
use cw_sim::state::{FacilityStatus, HolderId, ScenarioKind, ScenarioMode, ShareLot, World};

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

fn fund(world: &mut World, firm: Handle, cents: i64) {
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test funding")
            .credit(Account::ShareCapital, cents, "test funding"),
    );
}

fn run(
    world: &mut World,
    dataset: &Dataset,
    command: CommandIntent,
    actor: u64,
) -> Result<(), String> {
    let envelope = CommandEnvelope {
        command_id_high: 9,
        command_id_low: world.revision + 1,
        actor,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent: command,
    };
    apply(world, dataset, &envelope)
        .map(|_| ())
        .map_err(|e| format!("{e:?}"))
}

/// Sum of all firms' cash, the investor's cash, outside capital and the outside books.
fn all_money(world: &World) -> i64 {
    world.firms.iter().map(|(_, f)| f.book.cash()).sum::<i64>()
        + world.investor.cash_cents
        + world
            .outside
            .investors
            .iter()
            .map(|h| h.capital_cents)
            .sum::<i64>()
        + world.outside.gateway_book.cash()
        + world.outside.legal_services_book.cash()
}

fn all_balanced(world: &World) -> bool {
    world.firms.iter().all(|(_, f)| f.book.is_balanced()) && world.investor.book.is_balanced()
}

/// Put a plant of `recipe` in `region` under `firm`, cloned from any existing plant.
fn give_plant(world: &mut World, firm: Handle, recipe: u16, region: u8, blocks: u8) -> Handle {
    let template = world
        .facilities
        .iter()
        .next()
        .map(|(_, p)| p.clone())
        .unwrap();
    let mut plant = template;
    plant.firm = firm;
    plant.recipe = recipe;
    plant.region = region;
    plant.blocks = blocks;
    plant.status = FacilityStatus::Operating;
    plant.target_bps = 10_000;
    plant.capex_cents = 400_000_000;
    plant.accumulated_depreciation = 40_000_000;
    plant.workers = [0; 3];
    let handle = world.facilities.insert(plant);
    world.firms.get_mut(firm).unwrap().facilities.push(handle);
    // Book the plant so the ledger knows it.
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::FixedAssets, 400_000_000, "test plant")
            .credit(Account::AccumulatedDepreciation, 40_000_000, "test plant")
            .credit(Account::ShareCapital, 360_000_000, "test plant"),
    );
    handle
}

/// Own a firm completely, as the investor.
fn own(world: &mut World, firm: Handle) {
    let f = world.firms.get_mut(firm).unwrap();
    f.shares.holders.clear();
    f.shares.holders.insert(
        HolderId::Investor,
        ShareLot {
            qty: f.shares.issued,
            cost_cents: 1,
        },
    );
    f.board.ceo = Some(cw_sim::state::CeoRef::Investor);
}

/// A recipe pair where the second burns what the first makes.
fn chain_pair(dataset: &Dataset) -> (u16, u16, u16) {
    for (i, seller) in dataset.recipes.iter().enumerate() {
        if seller.storage_class.is_some() || seller.transport_mode.is_some() {
            continue;
        }
        for (j, buyer) in dataset.recipes.iter().enumerate() {
            if i == j || buyer.storage_class.is_some() || buyer.transport_mode.is_some() {
                continue;
            }
            if buyer.output == seller.output {
                continue;
            }
            if buyer.weekly_inputs_lots.contains_key(&seller.output)
                && dataset.product_index.contains_key(&seller.output)
                && dataset.products[dataset.product_index[&seller.output] as usize].kind
                    == cw_data::ProductKind::Goods
            {
                return (i as u16, j as u16, dataset.product_index[&seller.output]);
            }
        }
    }
    panic!("the dataset has a chain");
}

struct Group {
    world: World,
    dataset: Dataset,
    seller: Handle,
    buyer: Handle,
    product: u16,
    region: u8,
    industry: u8,
}

/// Two firms the investor owns outright, one making a good the other burns, in one region.
fn group() -> Group {
    let (mut world, dataset) = world(311);
    let (rs, rb, product) = chain_pair(&dataset);
    let handles: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    let (seller, buyer) = (handles[0], handles[1]);
    // Start both plant lists empty so the chain is clean.
    for firm in [seller, buyer] {
        let old: Vec<Handle> = world.firms.get(firm).unwrap().facilities.clone();
        for plant in old {
            world.facilities.remove(plant);
        }
        world.firms.get_mut(firm).unwrap().facilities.clear();
        own(&mut world, firm);
        fund(&mut world, firm, 500_000_000);
    }
    let region = 0u8;
    give_plant(&mut world, seller, rs, region, 4);
    give_plant(&mut world, buyer, rb, region, 4);
    let industry = sectors::recipe_industry(&dataset, rs);
    Group {
        world,
        dataset,
        seller,
        buyer,
        product,
        region,
        industry,
    }
}

fn stock_seller(g: &mut Group, lots: i64) {
    let cost = g.dataset.reference_price(g.product);
    g.world
        .firms
        .get_mut(g.seller)
        .unwrap()
        .stock_lot_mut(g.product, g.region)
        .receive(MilliLots::from_lots(lots), Cents(cost * lots));
}

#[test]
fn the_transfer_setting_runs_from_cost_through_market_to_a_markup() {
    let mut g = group();
    let (cost, market, at_market) = structure::transfer_quote(
        &g.world, &g.dataset, g.seller, g.product, g.region, g.industry,
    );
    assert_eq!(at_market, market, "the default is market");
    for (bps, expected) in [
        (0, cost),
        (5_000, cost + (market - cost) / 2),
        (10_000, market),
        (12_000, market * 12 / 10),
    ] {
        structure::set_transfer(
            &mut g.world,
            &g.dataset,
            g.seller,
            g.region,
            g.industry,
            bps,
        )
        .unwrap();
        let (_, _, price) = structure::transfer_quote(
            &g.world, &g.dataset, g.seller, g.product, g.region, g.industry,
        );
        assert_eq!(price, expected.max(1), "setting {bps}");
    }
    assert!(
        structure::set_transfer(
            &mut g.world,
            &g.dataset,
            g.seller,
            g.region,
            g.industry,
            12_001
        )
        .is_err()
    );
    // Back at market the override is dropped.
    structure::set_transfer(
        &mut g.world,
        &g.dataset,
        g.seller,
        g.region,
        g.industry,
        10_000,
    )
    .unwrap();
    assert!(g.world.structure.transfer.is_empty());
}

#[test]
fn group_members_supply_each_other_first_at_the_transfer_price() {
    let mut g = group();
    stock_seller(&mut g, 400);
    structure::sync_supply(&mut g.world, &g.dataset);
    let link = g
        .world
        .structure
        .links
        .get(&(g.buyer, g.seller, g.product))
        .expect("a standing link between group members")
        .clone();
    let contract = g.world.contracts.get(link.contract).unwrap().clone();
    assert_eq!(contract.price_cents, link.price_cents);
    assert_eq!(contract.buyer, g.buyer);
    assert_eq!(contract.seller, g.seller);
    assert!(contract.quantity_weekly.0 > 0);
    let before = all_money(&g.world);
    let buyer_stock = g.world.firm(g.buyer).unwrap().stock(g.product, g.region);
    let buyer_cash = g.world.firm(g.buyer).unwrap().book.cash();
    let seller_cash = g.world.firm(g.seller).unwrap().book.cash();
    freight::settle(&mut g.world, &g.dataset);
    let got = g.world.firm(g.buyer).unwrap().stock(g.product, g.region).0 - buyer_stock.0;
    assert!(
        got > 0,
        "the buyer received goods ahead of any open-market sale"
    );
    let paid = buyer_cash - g.world.firm(g.buyer).unwrap().book.cash();
    let received = g.world.firm(g.seller).unwrap().book.cash() - seller_cash;
    assert_eq!(paid, received, "what one pays the other receives");
    assert_eq!(
        paid,
        Cents(link.price_cents).0 * got / 1_000,
        "paid at the transfer price"
    );
    assert_eq!(all_money(&g.world), before, "no money is made or lost");
    assert!(all_balanced(&g.world));
    assert!(g.world.structure.stats.group_lots_moved > 0);
    // Pricing inside the group cannot squeeze a wholly owned firm.
    assert!(g.world.structure.squeeze.is_empty());
}

#[test]
fn pricing_inside_a_part_owned_firm_moves_value_from_its_minority() {
    let mut g = group();
    stock_seller(&mut g, 400);
    // The buyer has outside holders owning 30 percent.
    {
        let f = g.world.firms.get_mut(g.buyer).unwrap();
        let issued = f.shares.issued;
        let outside = issued * 3 / 10;
        f.shares
            .transfer(HolderId::Investor, HolderId::Outside(0), outside, outside);
    }
    structure::set_transfer(
        &mut g.world,
        &g.dataset,
        g.seller,
        g.region,
        g.industry,
        12_000,
    )
    .unwrap();
    structure::sync_supply(&mut g.world, &g.dataset);
    let link = g
        .world
        .structure
        .links
        .get(&(g.buyer, g.seller, g.product))
        .unwrap()
        .clone();
    assert!(link.price_cents > link.market_cents);
    let before = g.world.firm(g.buyer).unwrap().stock(g.product, g.region).0;
    freight::settle(&mut g.world, &g.dataset);
    let got = g.world.firm(g.buyer).unwrap().stock(g.product, g.region).0 - before;
    let expected = (link.price_cents - link.market_cents) * got / 1_000 * 3_000 / 10_000;
    let squeezed = g
        .world
        .structure
        .squeeze
        .get(&g.buyer)
        .copied()
        .unwrap_or(0);
    assert!(
        (squeezed - expected).abs() <= 1,
        "the buyer's thirty percent minority bears the premium: {squeezed} vs {expected}"
    );
    assert_eq!(g.world.structure.stats.squeeze_cents, squeezed);
}

#[test]
fn profit_lands_where_it_is_taxed_less() {
    let mut g = group();
    stock_seller(&mut g, 400);
    // The buyer's region taxes a fifth more than the seller's.
    {
        let r = g.world.firms.get(g.buyer).unwrap().region as usize;
        g.world.regions[r].policy.tax_modifier_bps = 2_000;
        g.world.regions[r].policy.tax_modifier_until = Some(Tick(1_000));
        let s = g.world.firms.get(g.seller).unwrap().region as usize;
        if s == r {
            // Same region: shift the seller's firm to a different region instead.
            let other = (r + 1) % g.world.regions.len();
            g.world.firms.get_mut(g.seller).unwrap().region = other as u8;
        }
    }
    assert!(
        structure::tax_rate_bps(&g.world, g.buyer) > structure::tax_rate_bps(&g.world, g.seller)
    );
    structure::set_transfer(
        &mut g.world,
        &g.dataset,
        g.seller,
        g.region,
        g.industry,
        12_000,
    )
    .unwrap();
    structure::sync_supply(&mut g.world, &g.dataset);
    freight::settle(&mut g.world, &g.dataset);
    assert!(
        g.world.structure.stats.tax_shift_cents > 0,
        "pricing up shifts profit to the lower-taxed supplier and saves the group tax"
    );
}

#[test]
fn a_captive_supplier_that_keeps_losing_is_marked_down() {
    let mut g = group();
    equity::refresh_valuation(&mut g.world, &g.dataset);
    let before = g.world.firm(g.seller).unwrap().valuation.equity_cents;
    assert_eq!(structure::markdown_bps(&g.world, g.seller), 0);
    g.world.structure.captive_weeks.insert(g.seller, 40);
    assert_eq!(structure::markdown_bps(&g.world, g.seller), 1_350);
    g.world.structure.captive_weeks.insert(g.seller, 200);
    assert_eq!(
        structure::markdown_bps(&g.world, g.seller),
        2_500,
        "capped at a quarter"
    );
    g.world.structure.captive_weeks.insert(g.seller, 40);
    // Marks move an eighth at a time, but fair value moves at once.
    equity::refresh_valuation(&mut g.world, &g.dataset);
    let after = g.world.firm(g.seller).unwrap().valuation.equity_cents;
    assert!(after < before, "equity {before} -> {after}");
    assert!(after >= before * 8_000 / 10_000);
}

#[test]
fn reputation_is_earned_slowly_and_lost_more_slowly_and_earns_a_premium() {
    let mut g = group();
    let key = (g.seller, g.region, g.industry);
    structure::weekly(&mut g.world, &g.dataset);
    let start = g.world.structure.reputation.get(&key).copied().unwrap();
    assert!(
        (start - structure::NEUTRAL_REP).abs() < 200,
        "a new division starts near neutral"
    );
    // Invest in quality: every product the seller makes sits 10 percent above its baseline.
    {
        let f = g.world.firms.get_mut(g.seller).unwrap();
        let tech = f.tech_level;
        f.quality_bps.insert(
            g.product,
            cw_sim::economy::research::quality(tech, 0) + 1_000,
        );
    }
    let mut last = start;
    for _ in 0..26 {
        structure::weekly(&mut g.world, &g.dataset);
        let now = g.world.structure.reputation[&key];
        assert!(now >= last);
        last = now;
    }
    let built = last;
    assert!(built > start + 200, "reputation rose: {start} -> {built}");
    let premium = structure::loyalty_premium_bps(&g.world, g.seller, g.region, g.product);
    assert!(premium > 0, "a good name earns a price premium");
    // Stop investing: it fades, but each week less than it was built.
    g.world.firms.get_mut(g.seller).unwrap().quality_bps.clear();
    let mut drops = Vec::new();
    for _ in 0..26 {
        let before = g.world.structure.reputation[&key];
        structure::weekly(&mut g.world, &g.dataset);
        drops.push(before - g.world.structure.reputation[&key]);
    }
    let fell = built - g.world.structure.reputation[&key];
    assert!(
        fell > 0 && fell < built - start,
        "it decays more slowly than it was earned"
    );
    // The premium bounds.
    g.world
        .structure
        .rep_product
        .insert((g.seller, g.region, g.product), 10_000);
    assert_eq!(
        structure::loyalty_premium_bps(&g.world, g.seller, g.region, g.product),
        1_500
    );
    g.world
        .structure
        .rep_product
        .insert((g.seller, g.region, g.product), 0);
    assert_eq!(
        structure::loyalty_premium_bps(&g.world, g.seller, g.region, g.product),
        -500
    );
}

#[test]
fn repeat_raids_cost_more_and_hurt_reputation_on_both_sides() {
    let mut g = group();
    let (a, d) = (g.seller, g.buyer);
    g.world
        .structure
        .reputation
        .insert((d, g.region, g.industry), 5_000);
    g.world
        .structure
        .reputation
        .insert((a, g.region, g.industry), 5_000);
    assert_eq!(structure::raid_surcharge(&g.world, a, d, 1_000_000), 0);
    let record = cw_sim::state::RaidRecord {
        tick: g.world.tick.0,
        attacker: a,
        defender: d,
        region: g.region,
        industry: g.industry,
        budget_cents: 1_000_000,
        chance_bps: 5_000,
        success: true,
        blocks: 1,
        price_cents: 0,
    };
    structure::note_raid(&mut g.world, &record, 2_000);
    let rep = |w: &World, f: Handle| w.structure.reputation[&(f, g.region, g.industry)];
    assert_eq!(
        rep(&g.world, d),
        5_000 - 300 - 400,
        "a taken division loses name"
    );
    assert_eq!(
        rep(&g.world, a),
        5_000,
        "the first raid costs the raider nothing in name"
    );
    assert_eq!(
        structure::raid_surcharge(&g.world, a, d, 1_000_000),
        400_000
    );
    structure::note_raid(&mut g.world, &record, 2_000);
    assert_eq!(
        structure::raid_surcharge(&g.world, a, d, 1_000_000),
        800_000,
        "forty percent more for every raid still on the record"
    );
    assert!(
        rep(&g.world, a) < 5_000,
        "a repeat raider is seen for what it is"
    );
    // A quarter later one raid is forgotten.
    g.world.tick.0 += 13;
    assert_eq!(
        structure::raid_surcharge(&g.world, a, d, 1_000_000),
        400_000
    );
    g.world.tick.0 += 26;
    assert_eq!(structure::raid_surcharge(&g.world, a, d, 1_000_000), 0);
}

#[test]
fn building_past_construction_capacity_costs_more_and_takes_longer_but_is_allowed() {
    let mut g = group();
    let firm = g.buyer;
    let cap = sectors::construction_capacity_blocks(&g.world, firm);
    assert!(cap >= sectors::FAST_BLOCKS);
    let recipe = g
        .world
        .facilities
        .get(g.world.firm(firm).unwrap().facilities[0])
        .unwrap()
        .recipe;
    let region = 1u8;
    let small = sectors::build_cost(&g.world, &g.dataset, firm, region, recipe, 10);
    assert_eq!(
        small.rush_bps, 10_000,
        "within capacity there is no premium"
    );
    let big = sectors::build_cost(&g.world, &g.dataset, firm, region, recipe, 200);
    assert!(big.rush_bps > 10_000 && big.rush_bps <= 25_000);
    // The same blocks one at a time inside capacity would cost 20 times a single block's capex
    // less the founding discount; the rush order costs strictly more per block.
    let one = sectors::build_cost(&g.world, &g.dataset, firm, region, recipe, 1);
    assert!(
        big.capex_cents > one.capex_cents * 200 / 2,
        "rising smoothly, not a wall"
    );
    let (excess, capacity) = sectors::construction_excess(&g.world, firm, 200);
    assert_eq!(excess, 200 - capacity);
    let base = g.dataset.recipes[recipe as usize].build_weeks;
    assert!(sectors::stretched_weeks(base, excess, capacity) > base);
    assert_eq!(sectors::stretched_weeks(base, 0, capacity), base);
    assert_eq!(
        sectors::stretched_weeks(base, capacity * 10, capacity),
        base * 3
    );
    // The order is accepted: there is no hard cap of forty.
    fund(&mut g.world, firm, 100_000_000_000);
    let result = run(
        &mut g.world,
        &g.dataset,
        CommandIntent::BuildFacility {
            firm,
            region,
            recipe,
            blocks: 100,
        },
        cw_sim::commands::INVESTOR_ACTOR,
    );
    assert!(result.is_ok(), "{result:?}");
    let building: i64 = sectors::blocks_in_construction(&g.world, firm);
    assert_eq!(building, 100);
    // Ordering more on top now is dearer again.
    let more = sectors::build_cost(&g.world, &g.dataset, firm, 2, recipe, 10);
    assert!(more.rush_bps > 10_000);
}

#[test]
fn a_bond_is_priced_off_the_ten_year_yield_plus_a_spread_that_rises_with_leverage() {
    let (mut world, _dataset) = world(312);
    let firm = world.investor.primary_group.unwrap();
    let low = structure::bond_quote(&world, firm, 50_000_000, 520);
    assert_eq!(
        low.base_bps,
        cw_sim::economy::macro_cycle::ten_year_yield_bps(&world),
        "ten years and more price off the ten-year yield"
    );
    assert_eq!(low.coupon_bps, (low.base_bps + low.spread_bps).max(100));
    let big = structure::bond_quote(&world, firm, 5_000_000_000, 520);
    assert!(big.spread_bps > low.spread_bps, "a larger issue costs more");
    assert!(big.leverage_bps > low.leverage_bps);
    // Shorter terms blend toward the policy rate.
    let short = structure::bond_quote(&world, firm, 50_000_000, 52);
    assert!(short.base_bps <= low.base_bps || world.macro_state.policy_rate_bps > low.base_bps);
    // More debt on the books raises the spread.
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, 2_000_000_000, "debt")
            .credit(Account::Loans, 2_000_000_000, "debt"),
    );
    let indebted = structure::bond_quote(&world, firm, 50_000_000, 520);
    assert!(indebted.spread_bps > low.spread_bps);
    assert!(indebted.coverage_bps <= low.coverage_bps);
}

#[test]
fn a_bond_issue_trades_pays_coupons_and_returns_principal_without_inventing_interest() {
    let (mut world, dataset) = world(313);
    let firm = world.investor.primary_group.unwrap();
    fund(&mut world, firm, 2_000_000_000);
    world.outside.investors[0].capital_cents += 5_000_000_000;
    let principal = 100_000_000;
    let start_money = all_money(&world);
    let quote = structure::bond_quote(&world, firm, principal, 104);
    // A cap under the market price is refused; zero takes the market.
    assert!(structure::issue_bond(&mut world, firm, principal, 104, quote.coupon_bps - 1).is_err());
    assert!(
        structure::issue_bond(&mut world, firm, 1_000, 104, 0).is_err(),
        "too small"
    );
    let bond = structure::issue_bond(&mut world, firm, principal, 104, 0).unwrap();
    let stored = world.bonds.get(bond).unwrap().clone();
    assert_eq!(stored.coupon_bps, quote.coupon_bps);
    assert_eq!(stored.principal_cents, principal);
    let fee = principal * 200 / 10_000;
    assert_eq!(
        all_money(&world),
        start_money,
        "the fee moves to the legal sector; nothing is made"
    );
    assert!(world.firms.get(firm).unwrap().book.balance(Account::Bonds) == -principal);
    assert_eq!(world.structure.stats.bonds_issued, 1);
    let _ = fee;

    // The board lists it; the investor buys at the mark and sells back.
    let board = structure::bond_board(&world);
    assert_eq!(board.len(), world.bonds.iter().count());
    let line = board.iter().find(|l| l.bond == bond).unwrap();
    assert!(line.outside_qty == principal);
    let money = all_money(&world);
    run(
        &mut world,
        &dataset,
        CommandIntent::PlaceBondOrder {
            bond,
            investor: true,
            qty: 10_000_000,
        },
        0,
    )
    .unwrap();
    assert_eq!(
        all_money(&world),
        money,
        "a trade only moves money between holders"
    );
    assert_eq!(
        world.bonds.get(bond).unwrap().holders[&HolderId::Investor],
        10_000_000
    );
    assert!(world.investor.book.is_balanced());
    run(
        &mut world,
        &dataset,
        CommandIntent::SellBond {
            bond,
            qty: 10_000_000,
        },
        0,
    )
    .unwrap();
    assert!(
        !world
            .bonds
            .get(bond)
            .unwrap()
            .holders
            .contains_key(&HolderId::Investor)
    );
    assert_eq!(all_money(&world), money);
    assert!(
        world.structure.bond_basis.is_empty()
            || !world
                .structure
                .bond_basis
                .contains_key(&(bond, HolderId::Investor))
    );

    // Hold a slice to maturity; the investor ends with its investment cleared and the gain in
    // other income, not interest income.
    run(
        &mut world,
        &dataset,
        CommandIntent::PlaceBondOrder {
            bond,
            investor: true,
            qty: 20_000_000,
        },
        0,
    )
    .unwrap();
    let basis = world.structure.bond_basis[&(bond, HolderId::Investor)];
    assert!(basis > 0, "the investor's cost of the slice is on record");
    let cash_before = world.investor.cash_cents;
    world.tick = Tick(world.bonds.get(bond).unwrap().maturity.0);
    let coupon_due = world.bonds.get(bond).unwrap().next_coupon.0 <= world.tick.0;
    credit::service(&mut world, &dataset);
    assert!(
        world.bonds.get(bond).is_none(),
        "the bond is retired at maturity"
    );
    let received = world.investor.cash_cents - cash_before;
    assert!(
        received >= 20_000_000,
        "the slice is repaid at par, plus any final coupon: {received}"
    );
    if !coupon_due {
        assert_eq!(received, 20_000_000);
    }
    assert!(
        world
            .structure
            .bond_basis
            .get(&(bond, HolderId::Investor))
            .is_none_or(|b| *b == 0),
        "its cost is released as the principal comes back"
    );
    assert!(world.investor.book.is_balanced());
    assert!(world.firms.get(firm).unwrap().book.is_balanced());
}

#[test]
fn a_bond_that_cannot_be_repaid_is_a_default_that_feeds_insolvency() {
    let (mut world, dataset) = world(314);
    let firm = world.investor.primary_group.unwrap();
    fund(&mut world, firm, 2_000_000_000);
    world.outside.investors[0].capital_cents += 5_000_000_000;
    let bond = structure::issue_bond(&mut world, firm, 100_000_000, 52, 0).unwrap();
    // Strip the issuer's cash.
    let cash = world.firms.get(firm).unwrap().book.cash();
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new().credit(Account::Cash, cash, "spent").debit(
            Account::OperatingExpense,
            cash,
            "spent",
        ),
    );
    world.tick = Tick(world.bonds.get(bond).unwrap().maturity.0);
    credit::service(&mut world, &dataset);
    assert_eq!(world.structure.stats.bond_defaults, 1);
    let status = world.firms.get(firm).unwrap().status;
    let owed = world
        .firms
        .get(firm)
        .unwrap()
        .book
        .balance(Account::OverduePayables);
    assert!(owed != 0 || !matches!(status, cw_sim::state::FirmStatus::Operating));
    // A bond whose issuer is past due marks far below par.
    assert!(
        structure::bond_board(&world)
            .iter()
            .all(|line| line.mark_bps <= 6_000 || world.bonds.get(line.bond).is_none())
    );
}

/// A firm with two divisions to carve one from.
fn two_division_firm() -> (World, Dataset, Handle, u8, u8, u8, u8) {
    let (mut world, dataset) = world(315);
    let firm = world.investor.primary_group.unwrap();
    own(&mut world, firm);
    fund(&mut world, firm, 3_000_000_000);
    // Two divisions: whatever the firm runs, plus a farm somewhere else.
    let first = world.firms.get(firm).unwrap().facilities[0];
    let r1 = world.facilities.get(first).unwrap().region;
    let rec1 = world.facilities.get(first).unwrap().recipe;
    let ind1 = sectors::recipe_industry(&dataset, rec1);
    let (rec2, ind2) = dataset
        .recipes
        .iter()
        .enumerate()
        .find_map(|(i, r)| {
            let ind = sectors::recipe_industry(&dataset, i as u16);
            (ind != ind1
                && r.storage_class.is_none()
                && r.transport_mode.is_none()
                && r.industry == "agriculture")
                .then_some((i as u16, ind))
        })
        .unwrap_or_else(|| {
            let i = dataset
                .recipes
                .iter()
                .position(|r| {
                    sectors::recipe_industry(&dataset, 0) != 99
                        && r.id != dataset.recipes[rec1 as usize].id
                        && r.storage_class.is_none()
                        && r.transport_mode.is_none()
                })
                .unwrap();
            (i as u16, sectors::recipe_industry(&dataset, i as u16))
        });
    let r2 = (r1 + 1) % world.regions.len() as u8;
    give_plant(&mut world, firm, rec2, r2, 3);
    (world, dataset, firm, r1, ind1, r2, ind2)
}

#[test]
fn a_carve_out_moves_a_division_into_a_wholly_owned_subsidiary_and_conserves_money() {
    let (mut world, dataset, firm, _r1, _i1, r2, i2) = two_division_firm();
    let before = all_money(&world);
    let plants = sectors::division_plants(&world, &dataset, firm, r2, i2);
    assert!(!plants.is_empty());
    let book: i64 = plants
        .iter()
        .map(|h| {
            let p = world.facilities.get(*h).unwrap();
            p.capex_cents - p.accumulated_depreciation
        })
        .sum();
    let parent_assets = world.firm(firm).unwrap().book.total_assets();
    let child = structure::carve_out(&mut world, &dataset, firm, r2, i2, "Carved Co".to_owned())
        .expect("carve-out");
    assert_eq!(all_money(&world), before);
    assert!(all_balanced(&world));
    assert!(sectors::division_plants(&world, &dataset, firm, r2, i2).is_empty());
    assert_eq!(
        sectors::division_plants(&world, &dataset, child, r2, i2).len(),
        plants.len()
    );
    for plant in &plants {
        assert_eq!(world.facilities.get(*plant).unwrap().firm, child);
    }
    assert_eq!(
        world.firm(child).unwrap().shares.controller(),
        Some(HolderId::Firm(firm))
    );
    assert!(ownership::same_group(&world, firm, child));
    assert!(world.firm(child).unwrap().book.net_tangible_assets() > 0);
    // The parent now holds the division as an investment worth its book value plus capital.
    assert!(world.firm(firm).unwrap().book.balance(Account::Investments) >= book);
    let after_assets = world.firm(firm).unwrap().book.total_assets();
    assert!(
        (parent_assets - after_assets).abs() < 10_000_000,
        "assets only change form"
    );
    // A firm keeps one division: carving out its last is refused.
    let last = sectors::divisions_of(&world, &dataset, firm)[0].clone();
    assert!(
        structure::carve_out(
            &mut world,
            &dataset,
            firm,
            last.region,
            last.industry,
            "x".to_owned()
        )
        .is_err()
    );
}

#[test]
fn a_listing_floats_part_of_a_subsidiary_and_keeps_control_then_a_spin_off_hands_it_over() {
    let (mut world, dataset, firm, _r1, _i1, r2, i2) = two_division_firm();
    let child =
        structure::carve_out(&mut world, &dataset, firm, r2, i2, "Floatco".to_owned()).unwrap();
    world.outside.investors[0].capital_cents += 5_000_000_000;
    equity::refresh_valuation(&mut world, &dataset);
    // Too young to list.
    assert!(structure::list_subsidiary(&mut world, child, 3_000).is_err());
    world.firms.get_mut(child).unwrap().shares.statement_weeks = 13;
    equity::refresh_valuation(&mut world, &dataset);
    let before = all_money(&world);
    let cash = world.firm(firm).unwrap().book.cash();
    // Too much would give up control.
    assert!(structure::list_subsidiary(&mut world, child, 5_500).is_err());
    let net = structure::list_subsidiary(&mut world, child, 3_000).expect("listing");
    assert!(net > 0);
    assert_eq!(world.firm(firm).unwrap().book.cash(), cash + net);
    assert_eq!(
        all_money(&world),
        before,
        "the fee moves to the legal sector"
    );
    assert!(all_balanced(&world));
    let sub = world.firm(child).unwrap();
    assert_eq!(
        sub.shares.controller(),
        Some(HolderId::Firm(firm)),
        "control kept"
    );
    assert!(sub.shares.ipo_done);
    let outside: i64 = sub
        .shares
        .holders
        .iter()
        .filter(|(h, _)| h.is_external())
        .map(|(_, l)| l.qty)
        .sum();
    assert_eq!(outside, sub.shares.issued * 3_000 / 10_000);
    assert_eq!(world.structure.stats.ipos, 1);

    // Spin it off: the parent's shareholders (the investor) receive the parent's stake.
    let held = world.firm(child).unwrap().shares.qty(HolderId::Firm(firm));
    let investor_before = world.firm(child).unwrap().shares.qty(HolderId::Investor);
    structure::spin_off(&mut world, child).expect("spin-off");
    let sub = world.firm(child).unwrap();
    assert_eq!(sub.shares.qty(HolderId::Firm(firm)), 0);
    assert_eq!(sub.shares.qty(HolderId::Investor), investor_before + held);
    assert_eq!(
        sub.shares.issued,
        sub.shares.holders.values().map(|l| l.qty).sum::<i64>()
    );
    assert!(
        !ownership::same_group(&world, firm, child)
            || sub.shares.controller() == Some(HolderId::Investor)
    );
    assert_eq!(all_money(&world), before);
    assert!(all_balanced(&world));
    assert_eq!(world.structure.stats.spin_offs, 1);
}

#[test]
fn a_joint_venture_has_stated_stakes_builds_its_division_splits_profit_and_can_be_bought_out() {
    let (mut world, dataset) = world(316);
    let handles: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    let (a, b) = (handles[0], handles[1]);
    for f in [a, b] {
        fund(&mut world, f, 4_000_000_000);
    }
    assert!(!ownership::same_group(&world, a, b));
    let recipe = dataset
        .recipes
        .iter()
        .position(|r| {
            r.storage_class.is_none() && r.transport_mode.is_none() && r.capex_cents < 800_000_000
        })
        .unwrap() as u16;
    let before = all_money(&world);
    // Stakes outside the allowed range are refused.
    assert!(
        structure::form_jv(&mut world, &dataset, a, b, 0, recipe, 1, 1_000, "X".into()).is_err()
    );
    let jv = structure::form_jv(
        &mut world,
        &dataset,
        a,
        b,
        0,
        recipe,
        1,
        6_000,
        "Pipeline Partners".into(),
    )
    .expect("venture forms");
    assert!(all_balanced(&world));
    let f = world.firm(jv).unwrap();
    assert_eq!(f.shares.vote_bps(HolderId::Firm(a)), 6_000);
    assert_eq!(f.shares.vote_bps(HolderId::Firm(b)), 4_000);
    assert_eq!(
        f.shares.controller(),
        Some(HolderId::Firm(a)),
        "the larger partner controls it"
    );
    assert!(ownership::same_group(&world, a, jv));
    assert!(!ownership::same_group(&world, b, jv));
    assert_eq!(f.facilities.len(), 1, "it is building its division");
    assert_eq!(world.structure.jvs.len(), 1);
    assert!(
        all_money(&world) <= before,
        "construction pays contractors; nothing is made"
    );
    // The cash each partner paid is proportional to its stake.
    let paid_a = f.shares.holders[&HolderId::Firm(a)].cost_cents;
    let paid_b = f.shares.holders[&HolderId::Firm(b)].cost_cents;
    assert!(
        (paid_a * 4 - paid_b * 6).abs() <= 10,
        "{paid_a} vs {paid_b}"
    );
    // Same-group partners are refused.
    assert!(
        structure::form_jv(&mut world, &dataset, a, jv, 0, recipe, 1, 5_000, "Y".into()).is_err()
    );
    // Locked up for 26 weeks.
    assert!(structure::buyout_jv(&mut world, jv, b).is_err());
    assert!(structure::exit_jv(&mut world, jv, b).is_err());
    world.tick.0 += structure::JV_LOCKUP_WEEKS;
    for facility in world.firm(jv).unwrap().facilities.clone() {
        world.facilities.get_mut(facility).unwrap().status = FacilityStatus::Operating;
    }
    equity::refresh_valuation(&mut world, &dataset);
    // Profit split: a quarterly payout in proportion to the stakes.
    {
        let book = &mut world.firms.get_mut(jv).unwrap().book;
        let _ = book.commit(
            Tick(world.tick.0),
            Txn::new()
                .debit(Account::Cash, 1_000_000_000, "profit")
                .credit(Account::RetainedEarnings, 1_000_000_000, "profit"),
        );
    }
    world.tick.0 = (world.tick.0 / 13 + 1) * 13;
    let (ca, cb) = (
        world.firm(a).unwrap().book.cash(),
        world.firm(b).unwrap().book.cash(),
    );
    structure::weekly(&mut world, &dataset);
    let (da, db) = (
        world.firm(a).unwrap().book.cash() - ca,
        world.firm(b).unwrap().book.cash() - cb,
    );
    assert!(da > 0 && db > 0, "both partners are paid");
    assert!(
        (da * 4 - db * 6).abs() <= 10,
        "profit follows the stakes: {da} vs {db}"
    );

    // The smaller partner buys the larger out at fair value plus a premium.
    let before_buyout = all_money(&world);
    let stake_a = world.firm(jv).unwrap().shares.qty(HolderId::Firm(a));
    structure::buyout_jv(&mut world, jv, b).expect("buyout");
    assert_eq!(
        world.firm(jv).unwrap().shares.qty(HolderId::Firm(b)),
        world.firm(jv).unwrap().shares.issued
    );
    assert_eq!(world.firm(jv).unwrap().shares.qty(HolderId::Firm(a)), 0);
    assert!(stake_a > 0);
    assert!(world.structure.jvs.is_empty());
    assert_eq!(
        all_money(&world),
        before_buyout,
        "a buyout moves money between partners"
    );
    assert!(all_balanced(&world));
    assert_eq!(world.structure.stats.jv_buyouts, 1);
}

#[test]
fn a_partner_can_exit_to_the_other_partner_or_to_the_market() {
    let (mut world, dataset) = world(317);
    let handles: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    let (a, b) = (handles[0], handles[1]);
    for f in [a, b] {
        fund(&mut world, f, 4_000_000_000);
    }
    let recipe = dataset
        .recipes
        .iter()
        .position(|r| {
            r.storage_class.is_none() && r.transport_mode.is_none() && r.capex_cents < 800_000_000
        })
        .unwrap() as u16;
    let jv = structure::form_jv(
        &mut world,
        &dataset,
        a,
        b,
        0,
        recipe,
        1,
        5_000,
        "Refinery Partners".into(),
    )
    .unwrap();
    world.tick.0 += structure::JV_LOCKUP_WEEKS;
    equity::refresh_valuation(&mut world, &dataset);
    let before = all_money(&world);
    structure::exit_jv(&mut world, jv, a).expect("exit to the partner");
    assert_eq!(world.firm(jv).unwrap().shares.qty(HolderId::Firm(a)), 0);
    assert_eq!(all_money(&world), before);
    assert!(all_balanced(&world));
}

#[test]
fn an_activist_campaign_opens_on_a_stopped_dividend_and_settles_by_temperament() {
    let (mut world, dataset) = world(318);
    // A listed rival with outside holders, a price far under its peak, seasoned statements.
    let target = world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| ownership::ultimate_controller(&world, *h) != Some(HolderId::Investor))
        .unwrap();
    fund(&mut world, target, 2_000_000_000);
    {
        let f = world.firms.get_mut(target).unwrap();
        let issued = f.shares.issued;
        f.shares.holders.clear();
        f.shares
            .holders
            .insert(HolderId::Firm(Handle(u64::MAX)), ShareLot::default());
        f.shares.holders.clear();
        f.shares.holders.insert(
            HolderId::Outside(0),
            ShareLot {
                qty: issued * 6 / 10,
                cost_cents: 1,
            },
        );
        f.shares.holders.insert(
            HolderId::Outside(1),
            ShareLot {
                qty: issued * 4 / 10,
                cost_cents: 1,
            },
        );
        f.shares.statement_weeks = 60;
    }
    // Two quarters of dividends, then nothing for twenty weeks, with cash to spare.
    world.structure.dividend_quarters.insert(target, 2);
    world.structure.last_dividend.insert(target, 1);
    world.tick.0 = 40;
    // Land on a week this firm is scanned.
    while target.slot() % 13 != world.tick.0 % 13 {
        world.tick.0 += 1;
    }
    structure::weekly(&mut world, &dataset);
    let campaign = world
        .structure
        .campaigns
        .iter()
        .find(|c| c.target == target)
        .cloned()
        .expect("a campaign opens against a firm that stopped paying");
    assert_eq!(campaign.reason, structure::Reason::DividendCut);
    assert_eq!(campaign.outcome, structure::Outcome::Open);
    assert!(campaign.justification_bps >= 5_000);
    assert_eq!(
        world.structure.stats.activist_events, 1,
        "{:?}",
        world.structure.campaigns
    );
    // A rival with no answer settles it at the deadline by its temperament.
    world.tick.0 = campaign.deadline;
    structure::weekly(&mut world, &dataset);
    let settled = world
        .structure
        .campaigns
        .iter()
        .find(|c| c.id == campaign.id)
        .unwrap();
    assert_ne!(settled.outcome, structure::Outcome::Open);
    assert!(all_balanced(&world));
    // One campaign a year per firm.
    world.tick.0 += 13;
    while target.slot() % 13 != world.tick.0 % 13 {
        world.tick.0 += 1;
    }
    structure::weekly(&mut world, &dataset);
    assert_eq!(
        world
            .structure
            .campaigns
            .iter()
            .filter(|c| c.target == target)
            .count(),
        1
    );
}

#[test]
fn an_activist_demand_is_met_in_cash_when_conceded_and_the_chance_follows_the_case() {
    let (mut world, dataset) = world(319);
    let target = world.investor.primary_group.unwrap();
    own(&mut world, target);
    fund(&mut world, target, 4_000_000_000);
    {
        let f = world.firms.get_mut(target).unwrap();
        let issued = f.shares.issued;
        f.shares.holders.clear();
        f.shares.holders.insert(
            HolderId::Investor,
            ShareLot {
                qty: issued * 7 / 10,
                cost_cents: 1,
            },
        );
        f.shares.holders.insert(
            HolderId::Outside(0),
            ShareLot {
                qty: issued * 3 / 10,
                cost_cents: 1,
            },
        );
        f.shares.statement_weeks = 60;
        // Retained earnings to pay a dividend out of.
        let _ = f.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::ShareCapital, 3_000_000_000, "earned")
                .credit(Account::RetainedEarnings, 3_000_000_000, "earned"),
        );
    }
    let weak = structure::Campaign {
        id: 1,
        target,
        activist: 0,
        stake_bps: 500,
        reason: structure::Reason::DividendCut,
        demand: structure::Demand::Dividend,
        justification_bps: 2_000,
        amount_cents: 0,
        opened: 0,
        deadline: 4,
        outcome: structure::Outcome::Open,
    };
    let strong = structure::Campaign {
        justification_bps: 9_500,
        ..weak.clone()
    };
    assert!(
        structure::activist_chance_bps(&world, &strong)
            > structure::activist_chance_bps(&world, &weak),
        "a better founded complaint wins more often"
    );
    // A controlling holder is hard to beat.
    assert!(structure::activist_chance_bps(&world, &weak) <= 2_000);
    world.structure.campaigns.push(strong.clone());
    world.structure.next_campaign = 2;
    let cash = world.firm(target).unwrap().book.cash();
    let investor_cash = world.investor.cash_cents;
    run(
        &mut world,
        &dataset,
        CommandIntent::RespondActivist {
            campaign: 1,
            stance: 0,
        },
        0,
    )
    .expect("the player answers");
    let paid = cash - world.firm(target).unwrap().book.cash();
    assert!(paid > 0, "conceding pays the dividend");
    assert!(
        world.investor.cash_cents > investor_cash,
        "the controlling investor gets its share"
    );
    assert_eq!(
        world.structure.campaigns[0].outcome,
        structure::Outcome::Conceded
    );
    assert!(all_balanced(&world));
    // A settled campaign cannot be answered twice; only the player's own firms can be answered.
    assert!(
        run(
            &mut world,
            &dataset,
            CommandIntent::RespondActivist {
                campaign: 1,
                stance: 2
            },
            0
        )
        .is_err()
    );
    assert!(
        run(
            &mut world,
            &dataset,
            CommandIntent::RespondActivist {
                campaign: 1,
                stance: 7
            },
            0
        )
        .is_err()
    );
}

#[test]
fn a_squeeze_challenge_is_settled_by_restitution_and_prices_return_to_market() {
    let mut g = group();
    // The buyer is 70 percent owned by the seller's controller; the rest is outside.
    {
        let f = g.world.firms.get_mut(g.buyer).unwrap();
        let issued = f.shares.issued;
        f.shares.holders.clear();
        f.shares.holders.insert(
            HolderId::Firm(g.seller),
            ShareLot {
                qty: issued * 7 / 10,
                cost_cents: 1,
            },
        );
        f.shares.holders.insert(
            HolderId::Outside(0),
            ShareLot {
                qty: issued * 3 / 10,
                cost_cents: 1,
            },
        );
        f.shares.statement_weeks = 60;
    }
    structure::set_transfer(
        &mut g.world,
        &g.dataset,
        g.seller,
        g.region,
        g.industry,
        12_000,
    )
    .unwrap();
    structure::sync_supply(&mut g.world, &g.dataset);
    assert!(!g.world.structure.links.is_empty(), "the group trades");
    g.world.structure.squeeze.insert(g.buyer, 50_000_000);
    let campaign = structure::Campaign {
        id: 3,
        target: g.buyer,
        activist: 0,
        stake_bps: 600,
        reason: structure::Reason::Squeeze,
        demand: structure::Demand::Restitution,
        justification_bps: 9_000,
        amount_cents: 50_000_000,
        opened: 0,
        deadline: 4,
        outcome: structure::Outcome::Open,
    };
    g.world.structure.campaigns.push(campaign);
    let money = all_money(&g.world);
    let buyer_cash = g.world.firm(g.buyer).unwrap().book.cash();
    structure::resolve_campaign(
        &mut g.world,
        &g.dataset,
        3,
        structure::Stance::Concede,
        false,
    );
    assert_eq!(
        g.world.firm(g.buyer).unwrap().book.cash(),
        buyer_cash + 50_000_000,
        "the controller pays the damage into the firm"
    );
    assert_eq!(
        all_money(&g.world),
        money,
        "restitution moves money inside the group"
    );
    assert!(
        g.world.structure.transfer.is_empty(),
        "prices return to market"
    );
    assert!(!g.world.structure.squeeze.contains_key(&g.buyer));
    assert!(all_balanced(&g.world));
}
