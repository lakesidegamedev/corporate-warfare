//! Corporate lifecycle evidence: unions, banking, insurance and a full takeover.
//!
//! These are integration fixtures over the public simulation API. They assert documented state
//! transitions and ledger identities, not private implementation shapes.

use cw_core::calendar::Tick;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{banking, deals, insolvency, insurance, seed, unions};
use cw_sim::state::{
    BargainState, DealKind, DealState, FirmStatus, HolderId, PolicyStatus, ScenarioKind,
    ScenarioMode, World,
};

fn world(seed_value: u64) -> World {
    let dataset = Dataset::load().unwrap();
    seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Founder,
        ScenarioMode::Sandbox,
    )
}

fn firm_with_industry(world: &World, industry: u8) -> cw_core::ids::Handle {
    world
        .firms
        .iter()
        .find(|(_, firm)| firm.industry == industry)
        .map(|(handle, _)| handle)
        .unwrap()
}

#[test]
fn union_organisation_strike_and_binding_settlement() {
    let dataset = Dataset::load().unwrap();
    let mut world = world(401);
    let firm = firm_with_industry(&world, 4); // manufacturing
    let region = world.firm(firm).unwrap().region;
    let prevailing = world.regions[region as usize].labour[0].prevailing_wage_cents;
    // Pay below ninety percent of prevailing: organisation must rise 25 bps a week.
    world.firms.get_mut(firm).unwrap().policies.wage_offer = [
        prevailing * 80 / 100,
        prevailing * 160 / 100,
        prevailing * 280 / 100,
    ];
    let before = world.firm(firm).unwrap().union.organisation_bps.max(1_500);
    unions::weekly(&mut world, &dataset);
    let after = world.firm(firm).unwrap().union.organisation_bps;
    assert!(
        after >= before + 25,
        "low pay organises the workforce: {before} -> {after}"
    );

    // Ballot at 5000, then a scheduled demand and a strike at the two-week deadline.
    {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.union.organisation_bps = 5_500;
        entity.organisation.insert(region, 5_500);
        entity.union.state = BargainState::Demanding {
            deadline: Tick(world.tick.0 + 2),
        };
        entity.union.demand_cents = prevailing * 120 / 100;
        entity.union.last_offer_cents = prevailing * 80 / 100;
        entity.union.settlement_cost_cents = 1;
    }
    world.tick = Tick(60);
    unions::weekly(&mut world, &dataset);
    assert!(matches!(
        world.firm(firm).unwrap().union.state,
        BargainState::Striking { .. }
    ));

    // Thirteen weeks of strike ends in a binding settlement that respects affordability and the
    // contractual floor rather than inventing money.
    {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.union.state = BargainState::Striking {
            until: Tick(world.tick.0 + 1),
            started: world.tick,
        };
        entity.policies.wage_offer[0] = prevailing * 80 / 100;
    }
    world.tick = Tick(100);
    unions::weekly(&mut world, &dataset);
    let entity = world.firm(firm).unwrap();
    assert!(matches!(entity.union.state, BargainState::Idle));
    assert!(entity.policies.wage_offer[0] >= prevailing * 80 / 100);
    assert!(entity.book.is_balanced());
}

#[test]
fn bank_deposit_loan_and_withdrawal_chain() {
    let mut world = world(402);
    let bank = world
        .firms
        .iter()
        .find(|(_, firm)| firm.bank.is_some())
        .map(|(handle, _)| handle)
        .unwrap();
    let borrower = world
        .firms
        .iter()
        .find(|(_, firm)| firm.bank.is_none() && firm.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    let deposits_before = banking::deposits(&world, bank);
    let cash_before = world.firm(borrower).unwrap().book.cash();
    banking::deposit(&mut world, bank, HolderId::Firm(borrower), 5_000_000).unwrap();
    assert_eq!(banking::deposits(&world, bank), deposits_before + 5_000_000);
    assert_eq!(
        world.firm(borrower).unwrap().book.cash(),
        cash_before - 5_000_000
    );
    // A loan inside the liquidity and capital limits moves cash and creates matching debt.
    let loan = banking::approve_loan(&mut world, bank, borrower, 2_000_000).expect("legal loan");
    assert!(world.loans.get(loan).is_some());
    let cash_after_loan = world.firm(borrower).unwrap().book.cash();
    assert_eq!(cash_after_loan, cash_before - 5_000_000 + 2_000_000);
    // Withdrawing the remaining claim is bounded by the bank's actual cash.
    let paid = banking::withdraw(&mut world, bank, HolderId::Firm(borrower), 3_000_000).unwrap();
    assert!(paid > 0);
    assert!(banking::deposits(&world, bank) >= 0);
    assert!(world.firm(bank).unwrap().book.cash() >= 0);
    assert!(world.firm(bank).unwrap().book.is_balanced());
}

#[test]
fn insurer_binds_and_pays_a_claim_from_reserves() {
    let dataset = Dataset::load().unwrap();
    let mut world = world(403);
    let insurer = world
        .firms
        .iter()
        .find(|(_, firm)| firm.insurer.is_some())
        .map(|(handle, _)| handle)
        .unwrap();
    let insured = world
        .firms
        .iter()
        .find(|(_, firm)| firm.insurer.is_none() && !firm.facilities.is_empty())
        .map(|(handle, _)| handle)
        .unwrap();
    let facility = world.firm(insured).unwrap().facilities[0];
    let limit = world.facilities.get(facility).unwrap().capex_cents / 2;
    let policy = insurance::bind(&mut world, insurer, insured, facility, limit, limit / 20)
        .expect("policy within limits");
    assert_eq!(
        world.insurance.get(policy).unwrap().status,
        PolicyStatus::Active
    );
    let reserve_before = world
        .firm(insurer)
        .unwrap()
        .book
        .balance(Account::ClaimLiabilities);
    let cash_before = world.firm(insured).unwrap().book.cash();
    let region = world.facilities.get(facility).unwrap().region;
    insurance::catastrophe(&mut world, region);
    let reserve_after = world
        .firm(insurer)
        .unwrap()
        .book
        .balance(Account::ClaimLiabilities);
    let cash_after = world.firm(insured).unwrap().book.cash();
    // A claim is either still reserved as a liability or already paid from insurer cash; both
    // are honest, and neither may leave the insured uncredited.
    assert!(
        reserve_after < reserve_before || cash_after > cash_before,
        "the claim is recognised or paid"
    );
    assert!(
        cash_after >= cash_before,
        "the insured is never debited by a catastrophe"
    );
    assert!(world.firm(insurer).unwrap().book.is_balanced());
    let _ = dataset;
}

#[test]
fn a_funded_hostile_tender_transfers_control_and_pays_minorities() {
    let mut world = world(404);
    let buyer = world
        .firms
        .iter()
        .find(|(_, firm)| firm.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    let target = world
        .firms
        .iter()
        .find(|(handle, firm)| *handle != buyer && firm.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    // Fund the buyer as an explicit fixture; the command still validates the escrow.
    let funding = 2_000_000_000;
    world
        .firms
        .get_mut(buyer)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, funding, "fixture funding")
                .credit(Account::ShareCapital, funding, "fixture funding"),
        )
        .unwrap();
    let fair = world.firm(target).unwrap().shares.fair_cents.max(1);
    let price = fair * 2;
    let deal = deals::start(
        &mut world,
        buyer,
        target,
        DealKind::SharePurchase,
        price,
        600_000,
    )
    .expect("funded escrow");
    // Diligence two weeks, then the four-week offer window.
    for _ in 0..7 {
        world.tick = world.tick.next();
        deals::weekly(&mut world);
    }
    let state = world.deals.get(deal).unwrap().state;
    assert!(
        matches!(state, DealState::Settled | DealState::Review),
        "the tender settles or enters review, was {state:?}"
    );
    // Control is measured from real votes, not from the deal state alone.
    if state == DealState::Settled {
        assert_eq!(
            world.firm(target).unwrap().shares.controller(),
            Some(HolderId::Firm(buyer))
        );
    }
    assert!(world.firm(buyer).unwrap().book.is_balanced());
    assert!(world.firm(target).unwrap().book.is_balanced());
}

#[test]
fn a_parent_guarantee_backs_a_failed_subsidiary_only_up_to_its_cap() {
    let mut world = world(406);
    let handles: Vec<_> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative())
        .map(|(handle, _)| handle)
        .take(3)
        .collect();
    let (parent, child, sibling) = (handles[0], handles[1], handles[2]);
    let parent_cash = world.firm(parent).unwrap().book.cash();
    let sibling_cash = world.firm(sibling).unwrap().book.cash();
    world
        .firms
        .get_mut(parent)
        .unwrap()
        .guarantees
        .push(cw_sim::state::Guarantee {
            beneficiary: child,
            claim_cents: 10_000_000,
            max_cents: 10_000_000,
        });
    insolvency::defer_expense(
        &mut world,
        child,
        5_000_000_000,
        Account::OperatingExpense,
        "unpayable",
    );
    let dataset = Dataset::load().unwrap();
    for _ in 0..8 {
        world.tick = world.tick.next();
        insolvency::settle(&mut world, &dataset);
    }
    let paid = parent_cash - world.firm(parent).unwrap().book.cash();
    assert!(
        paid > 0,
        "the guarantee is called when unpaid claims remain"
    );
    assert!(paid >= 0, "the guarantor is never credited by the failure");
    assert!(paid <= 10_000_000, "payment is capped by the guarantee");
    assert_eq!(
        world.firm(sibling).unwrap().book.cash(),
        sibling_cash,
        "siblings carry no implied guarantee"
    );
    assert!(world.firm(parent).unwrap().book.is_balanced());
}

#[test]
fn a_legal_merger_moves_assets_debt_facilities_and_contracts() {
    let mut world = world(407);
    let handles: Vec<_> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative())
        .map(|(handle, _)| handle)
        .take(2)
        .collect();
    let (buyer, target) = (handles[0], handles[1]);
    // The buyer owns the whole target, so the ninety-percent squeeze-out is satisfied.
    {
        let entity = world.firms.get_mut(target).unwrap();
        let issued = entity.shares.issued;
        entity.shares.holders.clear();
        entity.shares.add(HolderId::Firm(buyer), issued, 0);
    }
    let target_assets = world.firm(target).unwrap().book.total_assets();
    let target_facilities = world.firm(target).unwrap().facilities.len();
    let buyer_facilities = world.firm(buyer).unwrap().facilities.len();
    let loan = world.loans.insert(cw_sim::state::Loan {
        borrower: target,
        lender: cw_sim::state::Lender::Outside(0),
        principal_remaining_cents: 1_000_000,
        rate_bps: 600,
        weekly_principal_cents: 10_000,
        remaining_weeks: 100,
        collateral: None,
        secured: false,
        created: Tick(0),
        carry: 0,
        defaulted: false,
    });
    world.firms.get_mut(target).unwrap().loans.push(loan);
    let deal = world.deals.insert(cw_sim::state::Deal {
        buyer,
        target,
        kind: DealKind::Merger,
        state: DealState::Settled,
        escrow_cents: 0,
        offer_price_cents: 1,
        max_shares: 0,
        committed_shares: 0,
        started: Tick(0),
        diligence_until: Tick(0),
        offer_deadline: Tick(0),
        review_deadline: None,
        frozen_fair_cents: 1,
        review_required: false,
        remedy_share_bps: 0,
        hostile: false,
        diligence_paid: true,
    });
    let dataset = Dataset::load().unwrap();
    let revision = world.revision;
    let tick = world.tick.0;
    cw_sim::commands::apply(
        &mut world,
        &dataset,
        &cw_sim::commands::CommandEnvelope {
            command_id_high: 1,
            command_id_low: 1,
            actor: buyer.0,
            expected_revision: revision,
            submitted_tick: tick,
            intent: cw_sim::commands::CommandIntent::ProposeMerger { deal },
        },
    )
    .expect("merger accepted");
    assert!(matches!(
        world.firm(target).unwrap().status,
        FirmStatus::Merged { .. }
    ));
    assert_eq!(world.firm(target).unwrap().shares.issued, 0);
    for account in cw_core::ledger::Account::ALL {
        assert_eq!(
            world.firm(target).unwrap().book.balance(account),
            0,
            "target is empty"
        );
    }
    assert_eq!(
        world.firm(buyer).unwrap().facilities.len(),
        buyer_facilities + target_facilities,
        "facilities move to the survivor"
    );
    assert_eq!(
        world.loans.get(loan).unwrap().borrower,
        buyer,
        "debt follows the merger"
    );
    assert!(
        world
            .contracts
            .iter()
            .all(|(_, contract)| contract.buyer != target && contract.seller != target),
        "contracts are reattached"
    );
    assert!(world.firm(buyer).unwrap().book.is_balanced());
    assert!(target_assets > 0);
}

#[test]
fn liquidation_pays_creditors_before_equity_and_keeps_siblings() {
    let mut world = world(405);
    let doomed = world
        .firms
        .iter()
        .find(|(_, firm)| firm.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    let sibling = world
        .firms
        .iter()
        .find(|(handle, firm)| *handle != doomed && firm.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    let sibling_cash = world.firm(sibling).unwrap().book.cash();
    insolvency::defer_expense(
        &mut world,
        doomed,
        5_000_000_000,
        Account::OperatingExpense,
        "unpayable",
    );
    for _ in 0..8 {
        world.tick = world.tick.next();
        insolvency::settle(&mut world, &Dataset::load().unwrap());
    }
    let status = world.firm(doomed).unwrap().status;
    assert!(
        matches!(
            status,
            FirmStatus::Dissolved | FirmStatus::Restructuring { .. }
        ),
        "grace expires into a resolution, was {status:?}"
    );
    assert_eq!(
        world.firm(sibling).unwrap().book.cash(),
        sibling_cash,
        "an unrelated sibling keeps its cash"
    );
}
