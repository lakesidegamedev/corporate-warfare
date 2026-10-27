//! State rescue of failing strategic firms, and privatisation.
//!
//! A firm is strategic when it carries freight, stores or makes energy, or is a bank or insurer.
//! When such a firm has run out its payment grace and the law says so, the state steps in before
//! the court does:
//!
//! * Bailout (the Commerce Party): a state loan clears the arrears and the firm stays private.
//! * Nationalisation (the Workers' Party): when the court would liquidate the firm, the state
//!   buys every private share at the higher of fair value and the market mark, puts in the cash
//!   to clear the arrears in exchange for new shares, and the firm becomes a state firm. A firm
//!   the investor controls is never taken over, and a sound firm that is merely out of cash gets
//!   the bailout loan instead.
//!
//! The state acts only when its cash covers the whole rescue, and a firm is rescued at most once
//! in three years. A state firm behaves simply: it prices at cost plus five percent, never above the
//! market, and the rival AI leaves its strategy alone. Under a privatising government, a state
//! firm that has run a year and earned for a quarter is sold back to outside investors at fair
//! value, as far as their cash goes.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;

use crate::state::{
    FirmStatus, HolderId, Lender, Loan, PoliticalNews, Rescue, STATE_LENDER, World,
};

/// Most of the government's cash one rescue may take, as a percentage.
pub const MAX_CASH_PCT: i64 = 30;
/// A firm the state has rescued is not rescued again for three years: a second failure goes
/// to the court, so the state does not feed a firm that cannot recover.
pub const RESCUE_COOLDOWN_WEEKS: u32 = 156;
/// Weeks a state firm is held before it can be sold.
pub const HOLD_WEEKS: u32 = 52;

pub fn is_state_owned(world: &World, firm: Handle) -> bool {
    world
        .firms
        .get(firm)
        .is_some_and(|f| f.state_owned_since.is_some())
}

/// Carries freight, stores or makes energy, or is a bank or insurer.
pub fn is_strategic(world: &World, ds: &Dataset, firm: Handle) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    if entity.bank.is_some() || entity.insurer.is_some() {
        return true;
    }
    entity.facilities.iter().any(|handle| {
        world.facilities.get(*handle).is_some_and(|plant| {
            let recipe = &ds.recipes[plant.recipe as usize];
            recipe.transport_mode.is_some()
                || recipe.storage_class.is_some()
                || recipe.industry == "energy"
        })
    })
}

/// Real systemic size and a future: the firm is a large employer or carries a real share of a
/// lane, and its last quarter's operations covered their costs, so a rescue buys a firm that can
/// stand on its own. Small or structurally losing firms go to the court.
fn systemic_and_viable(world: &World, ds: &Dataset, firm: Handle) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    let weeks = entity.book.recent_weeks();
    if weeks.len() < 13 || weeks.iter().map(|w| w.ebit()).sum::<i64>() < 0 {
        return false;
    }
    let staff = entity.total_employees(&world.facilities);
    let mut sizes: Vec<i64> = world
        .firms
        .iter()
        .filter(|(_, f)| f.operative())
        .map(|(_, f)| f.total_employees(&world.facilities))
        .collect();
    sizes.sort_unstable();
    let top_quarter = sizes.get(sizes.len() * 3 / 4).copied().unwrap_or(0);
    let lane_share = world
        .logistics
        .lanes
        .keys()
        .any(|key| crate::economy::logistics::group_share_bps(world, firm, *key) >= 3_000);
    let _ = ds;
    staff >= top_quarter.max(1) || lane_share
}

fn loan_arrears(world: &World, firm: Handle) -> Vec<(Handle, i64)> {
    world
        .firms
        .get(firm)
        .map(|f| {
            f.loans
                .iter()
                .filter_map(|h| world.loans.get(*h).map(|l| (*h, l.arrears_cents)))
                .filter(|(_, a)| *a > 0)
                .collect()
        })
        .unwrap_or_default()
}

fn overdue(world: &World, firm: Handle) -> i64 {
    world
        .firms
        .get(firm)
        .map(|f| (-f.book.balance(Account::OverduePayables)).max(0))
        .unwrap_or(0)
}

fn news(world: &mut World, text: String) {
    world.politics.news.push(PoliticalNews {
        person: None,
        week: world.tick.0,
        text,
    });
    let excess = world.politics.news.len().saturating_sub(40);
    world.politics.news.drain(..excess);
}

/// Try to rescue a firm whose grace has run out. True when the state took it on and the court
/// should leave it alone.
pub fn try_rescue(world: &mut World, ds: &Dataset, firm: Handle) -> bool {
    let law = world.politics.laws.rescue;
    // Only the lead country's state keeps a rescue fund; its books are in the numeraire.
    if law == Rescue::None
        || !crate::economy::fx::in_numeraire(world, ds, firm)
        || !is_strategic(world, ds, firm)
    {
        return false;
    }
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    if entity.rescued_week > 0 && world.tick.0 < entity.rescued_week + RESCUE_COOLDOWN_WEEKS {
        return false;
    }
    let need = overdue(world, firm)
        + loan_arrears(world, firm)
            .iter()
            .map(|(_, a)| a)
            .sum::<i64>();
    if need <= 0 {
        return false;
    }
    if !systemic_and_viable(world, ds, firm) {
        return false;
    }
    let gov = world.government.book.cash().max(0);
    let player = crate::economy::events_engine::player_side(world, firm);
    // The state only takes a firm over when it is truly gone, that is when the court would
    // liquidate it: its assets would not cover half its debt. A sound firm that is merely out of
    // cash gets the rescue loan instead, under either government.
    let unsound = {
        let liquid = crate::economy::valuation::liquidation_value(&entity.book);
        liquid * 100 < entity.book.debt() * 50
    };
    let take_over = law == Rescue::Nationalise && !player && unsound;
    let compensation = if take_over {
        compensation_cents(world, firm)
    } else {
        0
    };
    if (need + compensation) * 100 > gov * MAX_CASH_PCT {
        return false;
    }
    if take_over {
        nationalise(world, firm, need);
    } else {
        bailout(world, firm, need);
    }
    true
}

/// Cash the firm needs to clear its arrears, put in by the state.
fn clear_arrears(world: &mut World, firm: Handle) {
    let owed = overdue(world, firm);
    if owed > 0 {
        let paid = owed.min(world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0));
        if paid > 0 {
            if let Some(entity) = world.firms.get_mut(firm) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, paid, "arrears cleared")
                        .debit(Account::OverduePayables, paid, "arrears cleared"),
                );
            }
            let _ = world.outside.gateway_book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, paid, "arrears received")
                    .credit(Account::Revenue, paid, "arrears received"),
            );
        }
    }
    for (loan, arrears) in loan_arrears(world, firm) {
        let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
        let pay = arrears.min(cash);
        if pay > 0 {
            crate::economy::credit::repay(world, loan, pay);
        }
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.status = FirmStatus::Operating;
        entity.payout_freeze = false;
        entity.past_due_weeks = 0;
        entity.rescued_week = world.tick.0;
    }
}

fn bailout(world: &mut World, firm: Handle, need: i64) {
    let _ = world.government.book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, need, "state rescue loan")
            .debit(Account::Receivables, need, "state rescue loan"),
    );
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, need, "state rescue loan")
                .credit(Account::Loans, need, "state rescue loan"),
        );
    }
    let loan = Loan {
        borrower: firm,
        lender: Lender::Outside(STATE_LENDER),
        principal_remaining_cents: need,
        rate_bps: world.macro_state.policy_rate_bps + 300,
        weekly_principal_cents: (need / 104).max(1),
        remaining_weeks: 104,
        collateral: None,
        secured: false,
        created: world.tick,
        carry: 0,
        defaulted: false,
        arrears_cents: 0,
    };
    let handle = world.loans.insert(loan);
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.loans.push(handle);
    }
    clear_arrears(world, firm);
    world.politics.bailouts += 1;
    let name = world
        .firms
        .get(firm)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    news(world, format!("State rescue loan keeps {name} running."));
}

/// What the state pays the private holders: the higher of fair value and the mark per share.
fn compensation_cents(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let price = entity
        .shares
        .fair_cents
        .max(entity.shares.mark_cents)
        .max(0);
    entity
        .shares
        .holders
        .iter()
        .filter(|(h, _)| !h.is_treasury() && **h != HolderId::State)
        .map(|(_, lot)| lot.qty.saturating_mul(price))
        .sum()
}

fn nationalise(world: &mut World, firm: Handle, need: i64) {
    let price = world
        .firms
        .get(firm)
        .map(|f| f.shares.fair_cents.max(f.shares.mark_cents).max(0))
        .unwrap_or(0);
    let holders: Vec<(HolderId, i64, i64)> = world
        .firms
        .get(firm)
        .map(|f| {
            f.shares
                .holders
                .iter()
                .filter(|(h, _)| !h.is_treasury() && **h != HolderId::State)
                .map(|(h, lot)| (*h, lot.qty, lot.cost_cents))
                .collect()
        })
        .unwrap_or_default();
    for (holder, qty, basis) in holders {
        let pay = qty.saturating_mul(price);
        if pay > 0 {
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, pay, "nationalisation compensation")
                    .debit(Account::Investments, pay, "nationalisation compensation"),
            );
        }
        // The holder's stake is written off at cost basis against what it receives.
        crate::economy::insolvency::pay_equity(world, holder, pay, basis);
        if let Some(entity) = world.firms.get_mut(firm) {
            entity.shares.transfer(holder, HolderId::State, qty, pay);
        }
    }
    // The state recapitalises the firm with the cash to clear its arrears, for new shares.
    let _ = world.government.book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, need, "state recapitalisation")
            .debit(Account::Investments, need, "state recapitalisation"),
    );
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, need, "state recapitalisation")
                .credit(Account::ShareCapital, need, "state recapitalisation"),
        );
        let new_price = price.max(1);
        let qty = (need / new_price).max(1);
        entity.shares.issued += qty;
        entity.shares.add(HolderId::State, qty, need);
        entity.state_owned_since = Some(world.tick.0);
    }
    clear_arrears(world, firm);
    world.politics.nationalisations += 1;
    let name = world
        .firms
        .get(firm)
        .map(|f| f.name.clone())
        .unwrap_or_default();
    news(world, format!("The state takes over {name}."));
}

/// Sell sound state firms back to outside investors when the government privatises.
pub fn weekly(world: &mut World, ds: &Dataset) {
    let _ = ds;
    if !world.politics.laws.privatise {
        return;
    }
    let now = world.tick.0;
    let candidates: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, f)| {
            f.status == FirmStatus::Operating
                && f.state_owned_since.is_some_and(|w| now >= w + HOLD_WEEKS)
                && f.book.recent_weeks().len() >= 13
                && f.book
                    .recent_weeks()
                    .iter()
                    .map(|w| w.net_income)
                    .sum::<i64>()
                    > 0
        })
        .map(|(h, _)| h)
        .collect();
    for firm in candidates {
        let (price, held) = match world.firms.get(firm) {
            Some(f) => (
                f.shares.fair_cents.max(f.shares.mark_cents).max(1),
                f.shares.qty(HolderId::State),
            ),
            None => continue,
        };
        let mut left = held;
        let investors = world.outside.investors.len();
        for index in 0..investors {
            if left <= 0 {
                break;
            }
            let capital = world.outside.investors[index].capital_cents;
            let qty = left.min(capital / price);
            if qty <= 0 {
                continue;
            }
            let cost = qty * price;
            world.outside.investors[index].capital_cents -= cost;
            let basis = world
                .firms
                .get(firm)
                .and_then(|f| f.shares.holders.get(&HolderId::State))
                .map(|lot| lot.cost_cents * qty / lot.qty.max(1))
                .unwrap_or(0);
            let mut txn = Txn::new()
                .debit(Account::Cash, cost, "privatisation")
                .credit(Account::Investments, basis, "privatisation");
            if cost > basis {
                txn = txn.credit(Account::OtherIncome, cost - basis, "privatisation gain");
            } else if basis > cost {
                txn = txn.debit(Account::OtherIncome, basis - cost, "privatisation loss");
            }
            let _ = world.government.book.commit(world.tick, txn);
            if let Some(f) = world.firms.get_mut(firm) {
                f.shares
                    .transfer(HolderId::State, HolderId::Outside(index as u32), qty, cost);
            }
            left -= qty;
        }
        let still = world
            .firms
            .get(firm)
            .map(|f| f.shares.controller() == Some(HolderId::State))
            .unwrap_or(false);
        if left < held {
            if !still {
                if let Some(f) = world.firms.get_mut(firm) {
                    f.state_owned_since = None;
                }
                world.politics.privatisations += 1;
                let name = world
                    .firms
                    .get(firm)
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                news(world, format!("{name} is sold back to private owners."));
            }
            // One sale a week keeps the market's cash from being drained at once.
            break;
        }
    }
}
