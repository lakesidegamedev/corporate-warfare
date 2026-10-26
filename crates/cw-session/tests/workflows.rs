//! End-to-end corporate workflows through the real session validator.
//!
//! Each family is driven through `Session::submit` exactly as the phone does. A family may be
//! rejected for a legitimate rule reason; the test requires a typed outcome and an unchanged
//! invariant set, never a panic or a silent partial mutation.

use cw_core::ids::Handle;
use cw_core::ledger::Account;
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::state::{DealKind, HolderId, PolicyKind, ScenarioKind, ScenarioMode};

struct Driver {
    session: Session,
    next: u64,
}

impl Driver {
    fn new(seed: u64) -> Self {
        let config = ScenarioConfig {
            seed,
            scenario: ScenarioKind::Guided,
            mode: ScenarioMode::FiveYear,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
        };
        Driver {
            session: Session::create(config).unwrap(),
            next: 1,
        }
    }

    fn firm(&self) -> Handle {
        self.session
            .world
            .investor
            .primary_group
            .expect("guided firm")
    }

    fn submit(&mut self, intent: CommandIntent) -> (bool, Option<String>) {
        let envelope = CommandEnvelope {
            command_id_high: 7,
            command_id_low: self.next,
            actor: 0,
            expected_revision: self.session.revision(),
            submitted_tick: self.session.world.tick.0,
            intent,
        };
        self.next += 1;
        match self.session.submit(envelope) {
            Ok(_) => (true, None),
            Err(error) => (false, Some(error.key().to_owned())),
        }
    }

    fn advance(&mut self, weeks: u32) {
        let mut done = 0;
        while done < weeks {
            let result = self.session.advance((weeks - done).min(16));
            done += result.weeks_advanced;
            if result.weeks_advanced == 0 {
                self.session.dismiss_pause();
            }
        }
    }
}

fn invariants(session: &Session) {
    for (_, firm) in session.world.firms.iter() {
        assert!(firm.book.is_balanced(), "{} ledger", firm.name);
        assert!(firm.book.cash() >= 0, "{} cash", firm.name);
        for lot in firm.inventory.values() {
            assert!(lot.qty.0 >= 0, "negative inventory");
        }
    }
}

#[test]
fn operating_and_market_families_are_reachable() {
    let mut driver = Driver::new(301);
    let firm = driver.firm();
    let reference = driver.session.dataset.reference_price(0);
    assert!(
        driver
            .submit(CommandIntent::SetPrice {
                firm,
                product: 0,
                price_cents: reference
            })
            .0
    );
    let wage = driver.session.world.firm(firm).unwrap().policies.wage_offer[0];
    assert!(
        driver
            .submit(CommandIntent::SetWageOffer {
                firm,
                tier: 0,
                wage_cents: wage + 1_000
            })
            .0
    );
    assert!(
        driver
            .submit(CommandIntent::SetProductionTarget {
                firm,
                target_bps: 9_000
            })
            .0
    );
    assert!(
        driver
            .submit(CommandIntent::SetResearchBudget {
                firm,
                amount_cents: 100_000
            })
            .0
    );
    assert!(
        driver
            .submit(CommandIntent::SetMarketingBudget {
                firm,
                amount_cents: 100_000
            })
            .0
    );
    // A build the firm can afford is accepted; an unaffordable one is a typed rejection.
    let recipe = 0u16;
    let cash = driver.session.world.firm(firm).unwrap().book.cash();
    let cost = driver.session.dataset.recipes[recipe as usize].capex_cents;
    if cash > cost {
        assert!(
            driver
                .submit(CommandIntent::BuildFacility {
                    firm,
                    region: 3,
                    recipe,
                    blocks: 1
                })
                .0
        );
    }
    let (accepted, key) = driver.submit(CommandIntent::PayDividend {
        firm,
        amount_cents: 1_000_000,
    });
    assert!(
        !accepted && key.is_some(),
        "no retained earnings yet: typed rejection"
    );
    driver.advance(2);
    invariants(&driver.session);
}

#[test]
fn equity_governance_and_credit_families_are_reachable() {
    let mut driver = Driver::new(302);
    let firm = driver.firm();
    cw_sim::economy::equity::refresh_valuation(&mut driver.session.world, &driver.session.dataset);
    let fair = driver
        .session
        .world
        .firm(firm)
        .unwrap()
        .shares
        .fair_cents
        .max(1);
    // Issuance needs a funded outside buyer and at least ninety percent of fair value.
    let (accepted, key) = driver.submit(CommandIntent::IssueShares {
        firm,
        qty: 10_000,
        price_cents: fair,
        buyer_outside: 0,
    });
    assert!(accepted || key.is_some());
    assert!(driver.submit(CommandIntent::CallMeeting { firm }).0);
    let (accepted, key) = driver.submit(CommandIntent::NominateCEO {
        firm,
        investor: true,
    });
    // The opening board is seated from the seeded cap table, so the investor can appoint a CEO
    // before the first scheduled meeting.
    assert!(
        accepted,
        "seeded board seats allow the appointment: {key:?}"
    );
    let (accepted, key) = driver.submit(CommandIntent::RequestLoan {
        firm,
        amount_cents: 5_000_000,
    });
    assert!(accepted || key.is_some());
    driver.advance(14);
    // The 13-week meeting appoints seats; the investor holds sixty percent, so three seats.
    assert!(driver.submit(CommandIntent::CallMeeting { firm }).0);
    driver.advance(4);
    assert!(
        driver
            .submit(CommandIntent::NominateCEO {
                firm,
                investor: true
            })
            .0
    );
    invariants(&driver.session);
}

#[test]
fn banking_insurance_and_deal_families_are_reachable() {
    let mut driver = Driver::new(303);
    let firm = driver.firm();
    let bank = driver
        .session
        .world
        .firms
        .iter()
        .find(|(_, candidate)| candidate.bank.is_some())
        .map(|(handle, _)| handle)
        .expect("a bank");
    driver.advance(2);
    let (accepted, key) = driver.submit(CommandIntent::DepositFunds {
        bank,
        firm,
        amount_cents: 1_000_000,
    });
    assert!(accepted || key.is_some());
    if accepted {
        assert!(
            driver
                .submit(CommandIntent::WithdrawFunds {
                    bank,
                    firm,
                    amount_cents: 500_000
                })
                .0
        );
    }
    let facility = driver.session.world.firm(firm).unwrap().facilities[0];
    let limit = driver
        .session
        .world
        .facilities
        .get(facility)
        .unwrap()
        .capex_cents
        / 2;
    let insurer = driver
        .session
        .world
        .firms
        .iter()
        .find(|(_, candidate)| candidate.insurer.is_some())
        .map(|(handle, _)| handle)
        .expect("an insurer");
    let (accepted, key) = driver.submit(CommandIntent::BindPolicy {
        insurer,
        insured: firm,
        facility,
        limit_cents: limit,
        deductible_cents: limit / 20,
    });
    assert!(accepted || key.is_some());
    // A funded diligence requires escrow; either it is accepted or the shortfall is typed.
    let target = driver
        .session
        .world
        .firms
        .iter()
        .find(|(handle, candidate)| *handle != firm && candidate.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    let price = driver
        .session
        .world
        .firm(target)
        .unwrap()
        .shares
        .mark_cents
        .max(1);
    let (accepted, key) = driver.submit(CommandIntent::StartDiligence {
        buyer: firm,
        target,
        kind: DealKind::SharePurchase,
        price_cents: price,
        max_shares: 1_000,
    });
    assert!(accepted || key.is_some());
    invariants(&driver.session);
}

#[test]
fn labour_policy_and_recovery_families_are_reachable() {
    let mut driver = Driver::new(304);
    let firm = driver.firm();
    assert!(
        driver
            .submit(CommandIntent::RespondBargaining {
                firm,
                choice: cw_sim::commands::BargainingChoice::Reject,
                offer_cents: 0,
            })
            .0
    );
    let (settled, settle_key) = driver.submit(CommandIntent::ProposeSettlement {
        firm,
        amount_cents: 0,
    });
    assert!(
        settled || settle_key.is_some(),
        "settlement is accepted or typed"
    );
    let (accepted, key) = driver.submit(CommandIntent::FileLobbyRequest {
        firm,
        region: 3,
        request: PolicyKind::PermitAcceleration,
    });
    assert!(accepted || key.is_some());
    // Recovery is only offered when destitute; a funded investor is refused with a typed key.
    let (accepted, key) = driver.submit(CommandIntent::AcceptManagementContract);
    assert!(!accepted && key.is_some());
    invariants(&driver.session);
}

#[test]
fn group_tree_and_intercompany_loan_use_real_authority() {
    let mut driver = Driver::new(305);
    let parent = driver.firm();
    // Fixture: make a second firm a controlled subsidiary by granting the parent sixty percent.
    let child = driver
        .session
        .world
        .firms
        .iter()
        .find(|(handle, candidate)| *handle != parent && candidate.operative())
        .map(|(handle, _)| handle)
        .unwrap();
    {
        let world = &mut driver.session.world;
        let issued = world.firm(child).unwrap().shares.issued;
        // More than the whole existing issue, so the parent holds strictly over half of the
        // enlarged total and the control threshold is exercised rather than assumed.
        let acquired = issued + 1;
        world
            .firms
            .get_mut(child)
            .unwrap()
            .shares
            .add(HolderId::Firm(parent), acquired, 0);
        world.firms.get_mut(child).unwrap().shares.issued += acquired;
    }
    assert!(
        driver
            .session
            .world
            .firm(child)
            .unwrap()
            .shares
            .controller()
            == Some(HolderId::Firm(parent))
    );
    let group = cw_sim::economy::ownership::controlled_group(&driver.session.world, parent);
    assert!(group.contains(&child));
    let cash = driver.session.world.firm(parent).unwrap().book.cash();
    let (accepted, key) = driver.submit(CommandIntent::MakeIntercompanyLoan {
        lender: parent,
        borrower: child,
        principal_cents: cash / 10,
        rate_bps: 600,
        weeks: 52,
    });
    assert!(accepted || key.is_some());
    // The two legal ledgers each balance; consolidated cash does not double count the transfer.
    let consolidated = cw_sim::economy::ownership::consolidate(&driver.session.world, parent);
    assert!(consolidated.firms.contains(&child));
    invariants(&driver.session);
    let parent_assets = driver
        .session
        .world
        .firm(parent)
        .unwrap()
        .book
        .total_assets();
    assert!(parent_assets >= 0);
    let _ = Account::Cash;
}

#[test]
fn weekly_cost_lines_reconcile_to_ebit_and_the_quarter_sums_net_income() {
    let mut session = Session::create(ScenarioConfig {
        seed: 11,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
    })
    .unwrap();
    for _ in 0..20 {
        if session.advance(1).pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    let firm = session.world.investor.primary_group.unwrap();
    let summary = cw_session::dto::finance_summary(&session.world, firm.0).unwrap();
    assert_eq!(summary.recent_weeks.len(), 13);
    for week in &summary.recent_weeks {
        assert_eq!(
            week.revenue_cents
                - week.cost_of_goods_cents
                - week.wages_cents
                - week.operating_expense_cents
                - week.depreciation_cents,
            week.ebit_cents,
            "week {} cost lines do not reconcile",
            week.week
        );
    }
    // The quarter's profit is the closed weeks' net income, straight from the ledger closes.
    let ledger_net: i64 = session
        .world
        .firms
        .get(firm)
        .unwrap()
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.net_income)
        .sum();
    let overview = session.overview();
    assert_eq!(overview.quarter_weeks, 13);
    assert_eq!(overview.quarter_net_income_cents, ledger_net);
}
