//! Event deck evidence (R29).
//!
//! docs/rules/EVENTS.md: every card has a real trigger, a preview, a deadline and a persisted
//! result; cards reveal or modify the same rules rather than adding a second penalty engine.
//! These fixtures build each trigger state directly, then check that the card opens, that its
//! choices post only what the table says, and that the deck respects authority and pauses.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::economy::{events_engine, seed, unions};
use cw_sim::state::{
    BargainState, ContractStatus, Deal, DealKind, DealState, FacilityStatus, FirmStatus, HolderId,
    ScenarioKind, ScenarioMode, World,
};

fn setup(seed_value: u64) -> (World, Dataset, Handle) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Guided,
        ScenarioMode::Sandbox,
    );
    let player = world.investor.primary_group.unwrap();
    (world, dataset, player)
}

fn rival(world: &World) -> Handle {
    world
        .firms
        .iter()
        .find(|(handle, firm)| !events_engine::player_side(world, *handle) && firm.operative())
        .map(|(handle, _)| handle)
        .unwrap()
}

fn card_for(world: &World, card: &str, subject: Handle) -> Option<u64> {
    world
        .events
        .pending
        .iter()
        .find(|event| !event.resolved && event.card == card && event.subject_firm == Some(subject))
        .map(|event| event.id)
}

fn balanced(world: &World) -> bool {
    world.investor.book.is_balanced() && world.firms.iter().all(|(_, firm)| firm.book.is_balanced())
}

fn fund(world: &mut World, firm: Handle, cents: i64) {
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, cents, "fixture funding")
                .credit(Account::ShareCapital, cents, "fixture funding"),
        )
        .unwrap();
}

#[test]
fn the_deck_has_twenty_four_cards_and_every_choice_resolves_with_balanced_books() {
    let dataset = Dataset::load().unwrap();
    assert_eq!(dataset.events.len(), 24);
    for card in &dataset.events {
        assert!(card.choices.len() >= 2, "{} offers a real choice", card.id);
        for choice in &card.choices {
            let (mut world, dataset, player) = setup(301);
            fund(&mut world, player, 500_000_000);
            let region = world.firm(player).unwrap().region;
            let id = events_engine::open(
                &mut world,
                &card.id,
                Some(player),
                Some(region),
                card.mandatory,
            );
            assert!(id > 0, "{} opens", card.id);
            events_engine::respond(&mut world, &dataset, id, &choice.id)
                .unwrap_or_else(|error| panic!("{}/{}: {error:?}", card.id, choice.id));
            assert!(
                balanced(&world),
                "{}/{} keeps every book balanced",
                card.id,
                choice.id
            );
            assert!(
                events_engine::respond(&mut world, &dataset, id, &choice.id).is_err(),
                "a resolved card cannot be answered twice"
            );
        }
    }
}

#[test]
fn a_supplier_in_distress_opens_the_card_and_termination_is_scoped_to_that_supplier() {
    let (mut world, dataset, _) = setup(302);
    let (supplier, buyer) = world
        .contracts
        .iter()
        .find(|(_, contract)| contract.status == ContractStatus::Active)
        .map(|(_, contract)| (contract.seller, contract.buyer))
        .unwrap();
    world.firms.get_mut(supplier).unwrap().status = FirmStatus::PastDue { since: world.tick };
    events_engine::state_triggers(&mut world, &dataset);
    let id = card_for(&world, "supplier_default", buyer).expect("card opens for the buyer");
    let others: Vec<_> = world
        .contracts
        .iter()
        .filter(|(_, c)| {
            c.buyer == buyer && c.seller != supplier && c.status == ContractStatus::Active
        })
        .map(|(handle, _)| handle)
        .collect();
    events_engine::respond(&mut world, &dataset, id, "terminate").unwrap();
    for (_, contract) in world.contracts.iter() {
        if contract.buyer == buyer && contract.seller == supplier {
            assert_eq!(contract.status, ContractStatus::Cancelled);
        }
    }
    for handle in others {
        assert_eq!(
            world.contracts.get(handle).unwrap().status,
            ContractStatus::Active,
            "healthy suppliers keep their contracts"
        );
    }
}

#[test]
fn a_quality_recall_charges_one_four_week_cohort_at_the_firms_price() {
    let (mut world, dataset, player) = setup(303);
    fund(&mut world, player, 500_000_000);
    let region = world.firm(player).unwrap().region;
    let product = 3u16;
    {
        let firm = world.firms.get_mut(player).unwrap();
        firm.quality_bps.insert(product, 7_000);
        firm.policies.prices.insert(product, 2_000);
        let mut week = std::collections::BTreeMap::new();
        week.insert((region, product), 50i64);
        // Eight weeks of history: only the last four count.
        firm.sales_history = vec![week; 8];
    }
    events_engine::state_triggers(&mut world, &dataset);
    let id = card_for(&world, "quality_recall", player).expect("recall opens");
    let cash = world.firm(player).unwrap().book.cash();
    let reputation = world.firm(player).unwrap().reputation_bps;
    events_engine::respond(&mut world, &dataset, id, "recall").unwrap();
    // 4 weeks * 50 units * $20.00 * 10 percent = $400.00.
    assert_eq!(cash - world.firm(player).unwrap().book.cash(), 40_000);
    assert_eq!(world.firm(player).unwrap().reputation_bps, reputation - 250);
}

#[test]
fn talent_poaching_needs_real_churn_not_the_firms_own_layoffs() {
    let (mut world, dataset, player) = setup(304);
    let employed = world
        .firm(player)
        .unwrap()
        .total_employees(&world.facilities);
    assert!(employed > 0);
    world.firms.get_mut(player).unwrap().staff_history = vec![employed * 2; 4];
    // The firm itself laid off everyone it lost: no poaching.
    world.firms.get_mut(player).unwrap().layoffs = vec![(world.tick.0, employed)];
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "talent_poaching", player).is_none());
    // Without the layoffs, the same loss is churn.
    let (mut world, dataset, player) = setup(304);
    world.firms.get_mut(player).unwrap().staff_history = vec![employed * 2; 4];
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "talent_poaching", player).is_some());
}

#[test]
fn organisation_crossing_half_opens_the_union_drive() {
    let (mut world, dataset, player) = setup(305);
    let region = world.firm(player).unwrap().region;
    let prevailing = world.regions[region as usize].labour[0].prevailing_wage_cents;
    {
        let firm = world.firms.get_mut(player).unwrap();
        firm.organisation.insert(region, 4_990);
        // Below 90 percent of prevailing, so organisation grows 25 bps this week.
        firm.policies.wage_offer[0] = prevailing * 80 / 100;
    }
    unions::weekly(&mut world, &dataset);
    assert!(card_for(&world, "union_drive", player).is_some());
}

#[test]
fn player_bargaining_and_strikes_are_mandatory_and_pause_but_rival_ones_do_not() {
    let (mut world, dataset, player) = setup(306);
    let other = rival(&world);
    for firm in [player, other] {
        world.firms.get_mut(firm).unwrap().union.state = BargainState::Demanding {
            deadline: Tick(world.tick.0 + 2),
        };
    }
    events_engine::state_triggers(&mut world, &dataset);
    let id = card_for(&world, "wage_negotiation", player).expect("player card");
    assert!(
        world
            .events
            .pending
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .mandatory
    );
    assert!(
        card_for(&world, "wage_negotiation", other).is_none(),
        "the rival answers through its own command"
    );
    world.firms.get_mut(player).unwrap().union.state = BargainState::Striking {
        until: Tick(world.tick.0 + 4),
        started: world.tick,
    };
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "strike_escalation", player).is_some());
}

#[test]
fn a_rival_mandatory_card_never_stops_the_players_clock() {
    let (mut world, dataset, _) = setup(307);
    let other = rival(&world);
    let region = world.firm(other).unwrap().region;
    events_engine::open(&mut world, "bank_surge", Some(other), Some(region), true);
    let outcome = cw_sim::tick::advance(&mut world, &dataset);
    assert!(
        !outcome.pause.as_deref().unwrap_or("").starts_with("event:"),
        "paused on {:?}",
        outcome.pause
    );
    assert!(
        world
            .events
            .pending
            .iter()
            .filter(|event| event.subject_firm == Some(other))
            .all(|event| event.resolved),
        "the rival answered its own card"
    );
}

#[test]
fn a_hostile_stake_is_reported_when_it_crosses_a_quarter() {
    let (mut world, dataset, player) = setup(308);
    // The guided start has a 40 percent co-investor; begin from a wholly owned company.
    {
        let firm = world.firms.get_mut(player).unwrap();
        let issued = firm.shares.issued;
        let mut holders = std::collections::BTreeMap::new();
        holders.insert(
            HolderId::Investor,
            cw_sim::state::ShareLot {
                qty: issued,
                cost_cents: 0,
            },
        );
        firm.shares = cw_sim::state::ShareBook::new(holders);
        firm.outside_peak_bps = 0;
    }
    // An outside holder at 20 percent is below the line.
    {
        let firm = world.firms.get_mut(player).unwrap();
        let issued = firm.shares.issued;
        firm.shares.add(HolderId::Outside(0), issued / 4, 0);
        firm.shares.issued += issued / 4;
    }
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "hostile_stake", player).is_none());
    {
        let firm = world.firms.get_mut(player).unwrap();
        let issued = firm.shares.issued;
        firm.shares.add(HolderId::Outside(0), issued / 5, 0);
        firm.shares.issued += issued / 5;
    }
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "hostile_stake", player).is_some());
    // Staying above the line does not reopen it.
    world
        .events
        .pending
        .iter_mut()
        .for_each(|event| event.resolved = true);
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "hostile_stake", player).is_none());
}

#[test]
fn a_covenant_breach_opens_and_the_cure_repays_from_spare_cash() {
    let (mut world, dataset, player) = setup(309);
    let tangible = world
        .firm(player)
        .unwrap()
        .book
        .net_tangible_assets()
        .max(1);
    let principal = tangible;
    let loan = world.loans.insert(cw_sim::state::Loan {
        borrower: player,
        lender: cw_sim::state::Lender::Outside(0),
        principal_remaining_cents: principal,
        rate_bps: 800,
        weekly_principal_cents: principal / 52,
        remaining_weeks: 52,
        collateral: None,
        secured: false,
        created: world.tick,
        carry: 0,
        defaulted: false,
        arrears_cents: 0,
    });
    world
        .firms
        .get_mut(player)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, principal, "fixture loan")
                .credit(Account::Loans, principal, "fixture loan"),
        )
        .unwrap();
    world.firms.get_mut(player).unwrap().loans.push(loan);
    events_engine::state_triggers(&mut world, &dataset);
    let id = card_for(&world, "covenant_warning", player).expect("breach card");
    let lender_before = world.outside.lenders[0].capital_cents;
    events_engine::respond(&mut world, &dataset, id, "raise").unwrap();
    let repaid = principal - world.loans.get(loan).unwrap().principal_remaining_cents;
    assert!(repaid > 0);
    assert_eq!(
        world.outside.lenders[0].capital_cents,
        lender_before + repaid
    );
    assert!(balanced(&world));
}

fn deal(world: &mut World, buyer: Handle, target: Handle, state: DealState) -> Handle {
    world.deals.insert(Deal {
        buyer,
        target,
        kind: DealKind::SharePurchase,
        state,
        escrow_cents: 0,
        offer_price_cents: 1_000,
        max_shares: 1,
        committed_shares: 0,
        started: world.tick,
        diligence_until: world.tick,
        offer_deadline: Tick(world.tick.0 + 4),
        review_deadline: Some(Tick(world.tick.0 + 4)),
        frozen_fair_cents: 1_000,
        review_required: true,
        remedy_share_bps: 0,
        hostile: true,
        diligence_paid: true,
        investor_tendered: false,
    })
}

#[test]
fn review_and_tender_cards_follow_actual_deals_and_the_tender_choice_is_recorded() {
    let (mut world, dataset, player) = setup(310);
    let other = rival(&world);
    deal(&mut world, player, other, DealState::Review);
    events_engine::state_triggers(&mut world, &dataset);
    assert!(card_for(&world, "antitrust_review", player).is_some());

    world
        .firms
        .get_mut(other)
        .unwrap()
        .shares
        .add(HolderId::Investor, 100, 0);
    world.firms.get_mut(other).unwrap().shares.issued += 100;
    let third = world
        .firms
        .iter()
        .find(|(handle, firm)| {
            *handle != other && !events_engine::player_side(&world, *handle) && firm.operative()
        })
        .map(|(handle, _)| handle)
        .unwrap();
    let bid = deal(&mut world, third, other, DealState::OfferOpen);
    events_engine::state_triggers(&mut world, &dataset);
    let id = world
        .events
        .pending
        .iter()
        .find(|event| event.card == "tender_offer" && event.deal == Some(bid))
        .map(|event| event.id)
        .expect("the player holds target shares");
    // Addressed to the investor: rivals never answer it for the player.
    events_engine::auto_resolve(&mut world, &dataset);
    assert!(
        !world
            .events
            .pending
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .resolved
    );
    events_engine::respond(&mut world, &dataset, id, "tender").unwrap();
    assert!(world.deals.get(bid).unwrap().investor_tendered);
    assert_eq!(
        world.investor_holds(other),
        100,
        "shares move only at settlement"
    );
}

#[test]
fn a_permit_delay_card_targets_the_new_project() {
    let (mut world, dataset, player) = setup(311);
    let facility = world.firm(player).unwrap().facilities[0];
    let completes = Tick(world.tick.0 + 10);
    {
        let stored = world.facilities.get_mut(facility).unwrap();
        stored.status = FacilityStatus::Building;
        stored.started = world.tick;
        stored.completes = completes;
    }
    events_engine::state_triggers(&mut world, &dataset);
    let id = card_for(&world, "permit_delay", player).expect("permit card");
    events_engine::respond(&mut world, &dataset, id, "wait").unwrap();
    assert_eq!(
        world.facilities.get(facility).unwrap().completes,
        Tick(completes.0 + 4)
    );
}

#[test]
fn an_outage_caps_capacity_temporarily_and_leaves_the_target_alone() {
    let (mut world, dataset, player) = setup(312);
    let region = world.firm(player).unwrap().region;
    let facility = world.firm(player).unwrap().facilities[0];
    let target = world.facilities.get(facility).unwrap().target_bps;
    let id = events_engine::open(
        &mut world,
        "energy_outage",
        Some(player),
        Some(region),
        false,
    );
    events_engine::respond(&mut world, &dataset, id, "accept").unwrap();
    let stored = world.facilities.get(facility).unwrap();
    assert_eq!(
        stored.target_bps, target,
        "the player's own target is untouched"
    );
    assert_eq!(stored.outage, Some((5_000, Tick(world.tick.0 + 2))));
}

#[test]
fn freight_cards_never_delete_goods_or_touch_other_firms_shipments() {
    let (mut world, dataset, player) = setup(313);
    for _ in 0..3 {
        cw_sim::tick::advance(&mut world, &dataset);
        world.pending_pause = None;
    }
    let before: Vec<(Handle, Handle, Tick)> = world
        .shipments
        .iter()
        .map(|s| (s.buyer, s.seller, s.arrive))
        .collect();
    let region = world.firm(player).unwrap().region;
    let id = events_engine::open(
        &mut world,
        "bridge_closure",
        Some(player),
        Some(region),
        false,
    );
    events_engine::respond(&mut world, &dataset, id, "pause").unwrap();
    assert_eq!(
        world.shipments.len(),
        before.len(),
        "goods in transit stay in transit"
    );
    let id = events_engine::open(
        &mut world,
        "freight_congestion",
        Some(player),
        Some(region),
        false,
    );
    events_engine::respond(&mut world, &dataset, id, "spot").unwrap();
    for (shipment, (buyer, seller, arrive)) in world.shipments.iter().zip(before) {
        let ours = buyer == player || seller == player;
        let expected = if ours { Tick(arrive.0 + 1) } else { arrive };
        assert_eq!(shipment.arrive, expected);
    }
}

#[test]
fn a_recession_notice_does_not_let_the_player_veto_or_double_the_regime() {
    let (mut world, dataset, player) = setup(314);
    world.macro_state.regime = Some((
        cw_sim::state::RegimeKind::Recession,
        Tick(world.tick.0 + 26),
    ));
    let modifiers: usize = world.regions.iter().map(|r| r.demand_modifiers.len()).sum();
    let id = events_engine::open(&mut world, "recession", Some(player), None, false);
    events_engine::respond(&mut world, &dataset, id, "hold").unwrap();
    let after: usize = world.regions.iter().map(|r| r.demand_modifiers.len()).sum();
    assert_eq!(after, modifiers);
    assert!(
        world.macro_state.regime.is_some(),
        "holding does not cancel the regime"
    );
}

#[test]
fn a_card_whose_company_is_gone_resolves_with_no_action() {
    let (mut world, dataset, _) = setup(315);
    let other = rival(&world);
    let id = events_engine::open(&mut world, "crop_loss", Some(other), Some(0), false);
    world.firms.get_mut(other).unwrap().status = FirmStatus::Dissolved;
    events_engine::auto_resolve(&mut world, &dataset);
    assert!(
        world
            .events
            .pending
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .resolved
    );
}

#[test]
fn a_pending_card_survives_save_and_reload() {
    let (mut world, dataset, player) = setup(316);
    let region = world.firm(player).unwrap().region;
    let id = events_engine::open(&mut world, "crop_loss", Some(player), Some(region), false);
    let bytes = postcard::to_allocvec(&world).unwrap();
    let mut reloaded: World = postcard::from_bytes(&bytes).unwrap();
    assert!(
        !reloaded
            .events
            .pending
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .resolved
    );
    events_engine::respond(&mut reloaded, &dataset, id, "mitigate").unwrap();
    assert!(
        reloaded
            .events
            .pending
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .resolved
    );
}

#[test]
fn only_the_company_runner_may_answer_its_card() {
    let (mut world, dataset, player) = setup(317);
    let other = rival(&world);
    let region = world.firm(player).unwrap().region;
    let id = events_engine::open(&mut world, "crop_loss", Some(player), Some(region), false);
    let envelope = |actor: u64, world: &World| CommandEnvelope {
        command_id_high: 9,
        command_id_low: actor,
        actor,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent: CommandIntent::RespondEvent {
            event: id,
            choice: "absorb".to_owned(),
        },
    };
    let attempt = envelope(other.0, &world);
    let rejected = cw_sim::commands::apply(&mut world, &dataset, &attempt);
    assert!(rejected.is_err(), "a rival cannot answer the player's card");
    let answer = envelope(0, &world);
    cw_sim::commands::apply(&mut world, &dataset, &answer).expect("the player can");
}

#[test]
fn both_starts_run_a_year_with_events_and_balanced_books() {
    for scenario in [ScenarioKind::Founder, ScenarioKind::Guided] {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 318, scenario, ScenarioMode::Sandbox);
        for _ in 0..52 {
            cw_sim::tick::advance(&mut world, &dataset);
            let open: Vec<(u64, String)> = world
                .events
                .pending
                .iter()
                .filter(|event| !event.resolved)
                .map(|event| (event.id, event.card.clone()))
                .collect();
            for (id, card) in open {
                let choice = dataset.event(&card).unwrap().choices[0].id.clone();
                let _ = events_engine::respond(&mut world, &dataset, id, &choice);
            }
            world.pending_pause = None;
        }
        assert!(!world.events.pending.is_empty(), "{scenario:?} draws cards");
        assert!(balanced(&world), "{scenario:?} books balance");
    }
}
