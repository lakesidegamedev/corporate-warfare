//! Voluntary wind-down: a solvent company with nothing left to run sells what remains, pays
//! every creditor in full and returns the rest to its shareholders; one that cannot pay, still
//! runs a plant or is the career's primary company is refused. Same command for player and AI.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::economy::insolvency::wind_down_blocker;
use cw_sim::state::{FacilityStatus, FirmStatus, ScenarioKind, ScenarioMode};

fn session() -> Session {
    let mut session = Session::create(ScenarioConfig {
        seed: 3,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    })
    .unwrap();
    for _ in 0..3 {
        if session.advance(1).pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    session
}

fn wind_down(session: &mut Session, actor: u64, firm: Handle, id: u64) -> bool {
    session
        .submit(CommandEnvelope {
            command_id_high: 0x00D1_55017E,
            command_id_low: id,
            actor,
            expected_revision: session.world.revision,
            submitted_tick: session.world.tick.0,
            intent: CommandIntent::WindDown { firm },
        })
        .is_ok()
}

fn mothball_all(session: &mut Session, firm: Handle) {
    let tick = session.world.tick;
    let plants = session.world.firm(firm).unwrap().facilities.clone();
    for plant in plants {
        let plant = session.world.facilities.get_mut(plant).unwrap();
        plant.status = FacilityStatus::Mothballed;
        plant.mothballed_since = Some(tick);
    }
}

/// Rival companies with a working plant that would clear every other test once idle.
fn candidates(session: &Session) -> Vec<Handle> {
    let world = &session.world;
    world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            firm.operative()
                && !cw_ai::player_controlled(world, *handle)
                && wind_down_blocker(world, *handle) == Some("has a working plant")
        })
        .map(|(handle, _)| handle)
        .collect()
}

#[test]
fn an_idle_solvent_company_pays_creditors_then_returns_the_rest_to_shareholders() {
    let mut session = session();
    let mut tried = 0;
    for firm in candidates(&session) {
        // A working plant is refused, and nothing changes.
        assert!(!wind_down(&mut session, firm.0, firm, 1 + firm.0));
        mothball_all(&mut session, firm);
        if wind_down_blocker(&session.world, firm).is_some() {
            continue;
        }
        tried += 1;
        let world = &session.world;
        let book = world.firm(firm).unwrap().book.clone();
        let carrying: i64 = [
            Account::FixedAssets,
            Account::AccumulatedDepreciation,
            Account::Inventory,
            Account::Receivables,
            Account::OtherAssets,
            Account::WorkInProgress,
        ]
        .iter()
        .map(|a| book.balance(*a))
        .sum();
        let loans: i64 = world
            .firm(firm)
            .unwrap()
            .loans
            .iter()
            .filter_map(|l| world.loans.get(*l))
            .map(|l| l.principal_remaining_cents)
            .sum();
        let owed = loans
            - book.balance(Account::Deposits)
            - book.balance(Account::OverduePayables)
            - book.balance(Account::TaxesPayable);
        // Independent oracle: cash plus 40 percent of what is sold, less every claim in full.
        let estate =
            book.cash() + book.balance(Account::RestrictedCash) + carrying.max(0) * 40 / 100;
        let residual = estate - owed;
        let holders = world.firm(firm).unwrap().shares.holders.len() as i64;
        let tax_before = world.government.book.cash();
        assert!(residual >= 0);

        assert!(wind_down(&mut session, firm.0, firm, 1_000 + firm.0));
        let world = &session.world;
        let entity = world.firm(firm).unwrap();
        assert_eq!(entity.status, FirmStatus::Dissolved);
        assert!(entity.facilities.is_empty());
        assert!(world.facilities.iter().all(|(_, p)| p.firm != firm));
        // Every loan repaid in full; the tax owed reached the state.
        assert!(
            entity
                .loans
                .iter()
                .filter_map(|l| world.loans.get(*l))
                .all(|l| l.principal_remaining_cents == 0)
        );
        assert_eq!(entity.book.balance(Account::OverduePayables), 0);
        assert_eq!(entity.book.balance(Account::TaxesPayable), 0);
        assert!(world.government.book.cash() - tax_before >= -book.balance(Account::TaxesPayable));
        // Outside buyers paid exactly the forced-sale value; nothing was minted.
        let proceeds: i64 = entity
            .book
            .journal()
            .iter()
            .filter(|p| p.memo == "liquidation proceeds")
            .map(|p| p.delta)
            .sum();
        assert_eq!(proceeds, carrying.max(0) * 40 / 100);
        let distributed: i64 = entity
            .book
            .journal()
            .iter()
            .filter(|p| p.memo == "liquidation distribution" && p.account == Account::Cash)
            .map(|p| -p.delta)
            .sum();
        // Pro-rata rounding leaves at most a cent per holder.
        assert!(
            residual - distributed >= 0 && residual - distributed < holders.max(1),
            "residual {residual}, distributed {distributed}"
        );
        assert!(entity.book.is_balanced());
        if tried == 3 {
            break;
        }
    }
    assert!(tried > 0, "no idle candidate found");
}

#[test]
fn a_company_that_cannot_pay_in_full_or_the_primary_company_is_refused() {
    let mut session = session();
    let firm = candidates(&session)
        .into_iter()
        .find(|firm| {
            let mut probe = session.world.clone();
            let tick = probe.tick;
            for plant in probe.firm(*firm).unwrap().facilities.clone() {
                let plant = probe.facilities.get_mut(plant).unwrap();
                plant.status = FacilityStatus::Mothballed;
                plant.mothballed_since = Some(tick);
            }
            wind_down_blocker(&probe, *firm).is_none()
        })
        .expect("an idle solvent candidate");
    mothball_all(&mut session, firm);
    // A past-due bill one cent beyond the estate makes it insolvent: refused, firm untouched.
    let book = &session.world.firm(firm).unwrap().book;
    let carrying: i64 = [
        Account::FixedAssets,
        Account::AccumulatedDepreciation,
        Account::Inventory,
        Account::Receivables,
        Account::OtherAssets,
        Account::WorkInProgress,
    ]
    .iter()
    .map(|a| book.balance(*a))
    .sum();
    let estate = book.cash() + book.balance(Account::RestrictedCash) + carrying.max(0) * 40 / 100;
    let headroom = estate
        - session
            .world
            .firm(firm)
            .unwrap()
            .loans
            .iter()
            .filter_map(|l| session.world.loans.get(*l))
            .map(|l| l.principal_remaining_cents)
            .sum::<i64>()
        + book.balance(Account::Deposits)
        + book.balance(Account::OverduePayables)
        + book.balance(Account::TaxesPayable)
        + book.balance(Account::Bonds);
    let tick = session.world.tick;
    let entity = session.world.firms.get_mut(firm).unwrap();
    let _ = entity.book.commit(
        tick,
        Txn::new()
            .debit(Account::OperatingExpense, headroom + 1, "test bill")
            .credit(Account::OverduePayables, headroom + 1, "test bill"),
    );
    assert_eq!(
        wind_down_blocker(&session.world, firm),
        Some("cannot pay its creditors in full")
    );
    assert!(!wind_down(&mut session, firm.0, firm, 7));
    assert!(session.world.firm(firm).unwrap().operative());

    // The player's primary company is refused whatever its state.
    let primary = session.world.investor.primary_group.unwrap();
    mothball_all(&mut session, primary);
    assert_eq!(
        wind_down_blocker(&session.world, primary),
        Some("primary company")
    );
    assert!(!wind_down(&mut session, INVESTOR_ACTOR, primary, 8));
}

#[test]
fn the_wind_down_command_is_appended_so_older_logs_read_the_same() {
    let text = serde_json::to_string(&CommandIntent::WindDown { firm: Handle(5) }).unwrap();
    assert_eq!(text, r#"{"WindDown":{"firm":5}}"#);
    let older: CommandIntent =
        serde_json::from_str(r#"{"RespondActivist":{"campaign":2,"stance":1}}"#).unwrap();
    assert_eq!(older.key(), "respond_activist");
}
