//! Ownership graph, control and consolidation.
//!
//! Control is strictly more than half of eligible votes. Treasury shares have no votes. The
//! graph must stay acyclic: no purchase or issuance may create a direct or indirect equity
//! cycle, including a minority cycle. Traversal is iterative so a deep subsidiary tree cannot
//! exhaust the stack. Consolidated views eliminate intercompany balances and internal
//! investments; minority interests stay visible and parent book investment is never added
//! alongside a stake's fair value.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use cw_core::ids::Handle;

use crate::state::{HolderId, World};

/// Firms directly controlled by a holder.
pub fn controlled_by(world: &World, holder: HolderId) -> Vec<Handle> {
    let mut found: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.shares.controller() == Some(holder))
        .map(|(handle, _)| handle)
        .collect();
    found.sort_unstable();
    found
}

/// Every firm in the controlled group rooted at `root`, in stable order.
pub fn controlled_group(world: &World, root: Handle) -> Vec<Handle> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    if world.firms.contains(root) {
        queue.push_back(root);
        seen.insert(root);
    }
    while let Some(current) = queue.pop_front() {
        for handle in controlled_by(world, HolderId::Firm(current)) {
            if seen.insert(handle) {
                queue.push_back(handle);
            }
        }
    }
    seen.into_iter().collect()
}

/// The ultimate controller above a firm, walking immediate firm controllers to the top.
pub fn ultimate_controller(world: &World, firm: Handle) -> Option<HolderId> {
    let mut current = firm;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current) {
            return None;
        }
        let controller = world.firms.get(current)?.shares.controller()?;
        match controller {
            HolderId::Firm(parent) if parent != current => current = parent,
            HolderId::Firm(_) => return None,
            other => return Some(other),
        }
    }
}

/// True when two firms share the same ultimate controller. The investor's controlled firms are
/// one group; two subsidiaries of the same parent are one group; outside holders are not merged
/// unless they are literally the same holder id.
pub fn same_group(world: &World, left: Handle, right: Handle) -> bool {
    if left == right {
        return true;
    }
    match (
        ultimate_controller(world, left),
        ultimate_controller(world, right),
    ) {
        (Some(HolderId::Investor), Some(HolderId::Investor)) => true,
        (Some(HolderId::Outside(a)), Some(HolderId::Outside(b))) => a == b,
        (Some(HolderId::Firm(a)), Some(HolderId::Firm(b))) => a == b,
        _ => false,
    }
}

/// True when buying shares in `target` from `buyer` would close an ownership cycle, including
/// an indirect minority cycle.
pub fn would_create_cycle(world: &World, buyer: Handle, target: Handle) -> bool {
    if buyer == target {
        return true;
    }
    // Walk from the target down through firms in which it holds any shares. Reaching the
    // buyer means the buyer would own part of itself.
    let mut seen = BTreeSet::new();
    let mut stack = vec![target];
    while let Some(current) = stack.pop() {
        if !seen.insert(current) {
            continue;
        }
        if current == buyer {
            return true;
        }
        let Some(firm) = world.firms.get(current) else {
            continue;
        };
        for (holder, _) in firm.shares.holders.iter() {
            if let HolderId::Firm(child) = holder {
                stack.push(*child);
            }
        }
    }
    false
}

/// Product of ownership fractions from the investor down to `firm`, in basis points.
///
/// The graph is acyclic by construction, so a breadth-first walk with a best-known fraction
/// per firm terminates and cannot loop.
pub fn investor_look_through_bps(world: &World, firm: Handle) -> i64 {
    // child owner -> (owned firm, quantity)
    let mut children: BTreeMap<Handle, Vec<(Handle, i64)>> = BTreeMap::new();
    for (handle, entity) in world.firms.iter() {
        let eligible = entity.shares.eligible_votes().max(1);
        for (holder, lot) in entity.shares.holders.iter() {
            if let HolderId::Firm(owner) = holder {
                let bps = (lot.qty as i128 * 10_000 / eligible as i128) as i64;
                children.entry(*owner).or_default().push((handle, bps));
            }
        }
    }
    let mut best: BTreeMap<Handle, i64> = BTreeMap::new();
    let mut queue: VecDeque<Handle> = VecDeque::new();
    for (handle, entity) in world.firms.iter() {
        let bps = entity.shares.vote_bps(HolderId::Investor) as i64;
        if bps > 0 {
            best.insert(handle, bps);
            queue.push_back(handle);
        }
    }
    while let Some(current) = queue.pop_front() {
        let current_bps = *best.get(&current).unwrap_or(&0);
        if let Some(list) = children.get(&current) {
            for (owned, share_bps) in list {
                let candidate = current_bps * share_bps / 10_000;
                let entry = best.entry(*owned).or_default();
                if candidate > *entry {
                    *entry = candidate;
                    queue.push_back(*owned);
                }
            }
        }
    }
    best.get(&firm).copied().unwrap_or(0)
}

#[derive(Clone, Debug, Default)]
pub struct Consolidated {
    pub firms: Vec<Handle>,
    pub assets_cents: i64,
    pub liabilities_cents: i64,
    pub cash_cents: i64,
    pub revenue_lifetime_cents: i64,
    pub ebitda_trailing_cents: i64,
    pub minority_interest_cents: i64,
    pub intercompany_eliminated_cents: i64,
}

/// Consolidated group totals with intercompany loans and internal investments removed.
pub fn consolidate(world: &World, root: Handle) -> Consolidated {
    let group = controlled_group(world, root);
    let members: BTreeSet<Handle> = group.iter().copied().collect();
    let mut result = Consolidated {
        firms: group.clone(),
        ..Default::default()
    };
    let mut internal_investment_cost = 0i64;
    let mut unrealised_profit_cents = 0i64;
    for handle in &group {
        let Some(firm) = world.firms.get(*handle) else {
            continue;
        };
        result.assets_cents += firm.book.total_assets();
        result.liabilities_cents += firm.book.total_liabilities();
        result.cash_cents += firm.book.cash();
        result.revenue_lifetime_cents += firm.book.lifetime().revenue;
        result.ebitda_trailing_cents += firm
            .book
            .recent_weeks()
            .iter()
            .map(|week| week.ebitda())
            .sum::<i64>();
        // Internal investments are eliminated at their book cost. The cost is the parent's
        // share lot in the member's book, so an external investment is not removed.
        for (holder, lot) in firm.shares.holders.iter() {
            if let HolderId::Firm(parent) = holder
                && members.contains(parent)
            {
                internal_investment_cost += lot.cost_cents;
                result.intercompany_eliminated_cents += lot.cost_cents;
            }
        }
    }
    for handle in &group {
        if let Some(firm) = world.firms.get(*handle) {
            unrealised_profit_cents += firm
                .intercompany_inventory
                .values()
                .filter(|layer| layer.qty.0 > 0)
                .map(|layer| layer.margin_cents.max(0))
                .sum::<i64>();
        }
    }
    // Intercompany loans between two group members are eliminated on both sides.
    for (_, loan) in world.loans.iter() {
        let lender = match loan.lender {
            crate::state::Lender::Firm(handle) => Some(handle),
            _ => None,
        };
        if let Some(lender) = lender
            && members.contains(&lender)
            && members.contains(&loan.borrower)
        {
            let principal = loan.principal_remaining_cents.max(0);
            result.assets_cents -= principal;
            result.liabilities_cents -= principal;
            result.intercompany_eliminated_cents += principal;
        }
    }
    // The eliminated internal investment cost is an asset on the parent's book and must not
    // be counted alongside the subsidiary's own assets. Intercompany loans were already
    // removed from both sides above.
    result.assets_cents = result
        .assets_cents
        .saturating_sub(internal_investment_cost + unrealised_profit_cents);
    result.intercompany_eliminated_cents += unrealised_profit_cents;
    // Minority interest: the share of group equity held outside the controlling chain.
    for handle in &group {
        if let Some(firm) = world.firms.get(*handle) {
            let outside: i64 = firm
                .shares
                .holders
                .iter()
                .filter(|(holder, _)| {
                    !matches!(
                        holder,
                        HolderId::Investor | HolderId::Firm(_) | HolderId::Treasury
                    )
                })
                .map(|(_, lot)| lot.qty)
                .sum();
            let eligible = firm.shares.eligible_votes().max(1);
            result.minority_interest_cents += firm.shares.fair_cents * outside / eligible;
        }
    }
    result
}

/// Look-through market value of the investor's stake in a firm.
pub fn stake_value(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let bps = investor_look_through_bps(world, firm);
    entity.shares.fair_cents * entity.shares.eligible_votes() * bps / 10_000
}

/// Group value for the score: controlled productive-group value, control adjusted.
pub fn group_value(world: &World) -> i64 {
    let mut seen: BTreeMap<Handle, i64> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if firm.shares.qty(HolderId::Investor) == 0 {
            continue;
        }
        for member in controlled_group(world, handle) {
            let value = stake_value(world, member);
            let entry = seen.entry(member).or_default();
            *entry = (*entry).max(value);
        }
    }
    seen.values().sum()
}

pub fn invalidate(world: &mut World, root: Handle) {
    world.group_cache.remove(&root);
    let roots: Vec<Handle> = world.group_cache.keys().copied().collect();
    for key in roots {
        if world
            .group_cache
            .get(&key)
            .map(|group| group.contains(&root))
            .unwrap_or(false)
        {
            world.group_cache.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode, ShareBook, ShareLot};
    use cw_core::money::Cents;
    use cw_core::quantity::MilliLots;
    use cw_data::Dataset;

    fn synthetic_firm(world: &mut World, parent: Option<Handle>) -> Handle {
        let template = world
            .firms
            .iter()
            .next()
            .map(|(_, firm)| firm.clone())
            .expect("a template firm");
        let mut firm = template;
        firm.name = format!("Synthetic {}", world.firms.len());
        firm.region = 0;
        firm.status = crate::state::FirmStatus::Operating;
        firm.facilities.clear();
        firm.inventory.clear();
        firm.loans.clear();
        firm.bonds.clear();
        firm.insurance.clear();
        firm.contracts_buy.clear();
        firm.contracts_sell.clear();
        firm.guarantees.clear();
        let holder = parent.map(HolderId::Firm).unwrap_or(HolderId::Investor);
        firm.shares = ShareBook::new(std::collections::BTreeMap::from([
            (
                holder,
                ShareLot {
                    qty: 600_000,
                    cost_cents: 0,
                },
            ),
            (
                HolderId::Outside(99),
                ShareLot {
                    qty: 400_000,
                    cost_cents: 0,
                },
            ),
        ]));
        world.firms.insert(firm)
    }

    #[test]
    fn exactly_half_is_not_control_but_above_half_is() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 51, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        let group = controlled_group(&world, primary);
        assert!(group.contains(&primary));
        // Reduce the investor to exactly half: no control.
        if let Some(firm) = world.firms.get_mut(primary) {
            let held = firm.shares.qty(HolderId::Investor);
            let issued = firm.shares.issued;
            let mut holders = BTreeMap::new();
            holders.insert(
                HolderId::Investor,
                ShareLot {
                    qty: issued / 2,
                    cost_cents: 0,
                },
            );
            holders.insert(
                HolderId::Outside(1),
                ShareLot {
                    qty: issued - issued / 2,
                    cost_cents: 0,
                },
            );
            firm.shares = ShareBook::new(holders);
            let _ = held;
        }
        assert_eq!(world.firm(primary).unwrap().shares.controller(), None);
        assert!(controlled_group(&world, primary).contains(&primary));
    }

    #[test]
    fn indirect_cycles_are_rejected() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 52, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
        let (first, second) = (handles[0], handles[1]);
        // Second holds 30 percent of first. First buying second would close a cycle.
        // The second firm holds shares in the first: first owns second -> cycle.
        if let Some(firm) = world.firms.get_mut(first) {
            firm.shares.add(HolderId::Firm(second), 300_000, 0);
            firm.shares.issued += 300_000;
        }
        // And the second firm is held by the first, closing the loop if the first buys it.
        if let Some(firm) = world.firms.get_mut(second) {
            firm.shares.add(HolderId::Firm(first), 600_000, 0);
            firm.shares.issued += 600_000;
        }
        assert!(would_create_cycle(&world, first, second));
        assert!(!would_create_cycle(&world, second, handles[2]));
    }

    #[test]
    fn firms_under_the_same_controller_are_one_group() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 58, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let root = world.investor.primary_group.unwrap();
        let child = synthetic_firm(&mut world, Some(root));
        let grandchild = synthetic_firm(&mut world, Some(child));
        assert!(same_group(&world, root, child));
        assert!(same_group(&world, root, grandchild));
        let outside = world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != root && firm.shares.qty(HolderId::Investor) == 0)
            .map(|(handle, _)| handle)
            .unwrap();
        assert!(!same_group(&world, root, outside));
    }

    #[test]
    fn unrealised_intercompany_margin_leaves_consolidated_assets_only_when_sold() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 57, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let root = world.investor.primary_group.unwrap();
        let child = synthetic_firm(&mut world, Some(root));
        let qty = MilliLots(10_000);
        let margin = 5_000_000;
        {
            let buyer = world.firms.get_mut(child).unwrap();
            buyer.stock_lot_mut(0, 0).receive(qty, Cents(margin * 2));
            buyer.record_intercompany_receipt(root, 0, 0, qty, margin);
        }
        let gross: i64 = controlled_group(&world, root)
            .iter()
            .map(|handle| world.firm(*handle).unwrap().book.total_assets())
            .sum();
        let consolidated = consolidate(&world, root);
        assert_eq!(consolidated.assets_cents, gross - margin);
        assert!(consolidated.intercompany_eliminated_cents >= margin);
        let _ = world.firms.get_mut(child).unwrap().issue_stock(0, 0, qty);
        let consolidated = consolidate(&world, root);
        assert_eq!(
            consolidated.assets_cents,
            controlled_group(&world, root)
                .iter()
                .map(|handle| world.firm(*handle).unwrap().book.total_assets())
                .sum::<i64>(),
            "once the goods leave the group, no unrealised margin remains"
        );
    }

    #[test]
    fn internal_investment_cost_is_eliminated_from_consolidated_assets() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 55, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let root = world.investor.primary_group.unwrap();
        let child = synthetic_firm(&mut world, Some(root));
        let cost = 5_000_000;
        if let Some(firm) = world.firms.get_mut(child)
            && let Some(lot) = firm.shares.holders.get_mut(&HolderId::Firm(root))
        {
            lot.cost_cents = cost;
        }
        world
            .firms
            .get_mut(root)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(
                        cw_core::ledger::Account::Investments,
                        cost,
                        "internal investment",
                    )
                    .credit(cw_core::ledger::Account::Cash, cost, "internal investment"),
            )
            .unwrap();
        let group = controlled_group(&world, root);
        let gross: i64 = group
            .iter()
            .map(|handle| world.firm(*handle).unwrap().book.total_assets())
            .sum();
        let consolidated = consolidate(&world, root);
        assert_eq!(consolidated.intercompany_eliminated_cents, cost);
        assert_eq!(consolidated.assets_cents, gross - cost, "assets count once");
    }

    #[test]
    fn intercompany_loans_are_eliminated_from_both_sides() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 56, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let root = world.investor.primary_group.unwrap();
        let child = synthetic_firm(&mut world, Some(root));
        let principal = 3_000_000;
        world
            .firms
            .get_mut(root)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(
                        cw_core::ledger::Account::Receivables,
                        principal,
                        "intercompany loan",
                    )
                    .credit(
                        cw_core::ledger::Account::Cash,
                        principal,
                        "intercompany loan",
                    ),
            )
            .unwrap();
        world
            .firms
            .get_mut(child)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(
                        cw_core::ledger::Account::Cash,
                        principal,
                        "intercompany loan",
                    )
                    .credit(
                        cw_core::ledger::Account::Loans,
                        principal,
                        "intercompany loan",
                    ),
            )
            .unwrap();
        world.loans.insert(crate::state::Loan {
            borrower: child,
            lender: crate::state::Lender::Firm(root),
            principal_remaining_cents: principal,
            rate_bps: 0,
            weekly_principal_cents: 0,
            remaining_weeks: 52,
            collateral: None,
            secured: false,
            created: world.tick,
            carry: 0,
            defaulted: false,
        });
        let group = controlled_group(&world, root);
        let gross_assets: i64 = group
            .iter()
            .map(|handle| world.firm(*handle).unwrap().book.total_assets())
            .sum();
        let gross_liabilities: i64 = group
            .iter()
            .map(|handle| world.firm(*handle).unwrap().book.total_liabilities())
            .sum();
        let consolidated = consolidate(&world, root);
        assert_eq!(consolidated.assets_cents, gross_assets - principal);
        assert_eq!(
            consolidated.liabilities_cents,
            gross_liabilities - principal
        );
        assert!(consolidated.intercompany_eliminated_cents >= principal);
    }

    #[test]
    fn a_hundred_level_group_is_walked_iteratively() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 53, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let root = world.investor.primary_group.unwrap();
        let mut previous = root;
        for _ in 0..100 {
            previous = synthetic_firm(&mut world, Some(previous));
        }
        let group = controlled_group(&world, root);
        assert_eq!(group.len(), 101, "root plus one hundred levels");
        assert!(group.contains(&root));
        assert!(group.contains(&previous));
        let consolidated = consolidate(&world, root);
        assert_eq!(consolidated.firms.len(), group.len());
    }

    #[test]
    fn a_hundred_child_group_counts_each_member_once() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 54, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let root = world.investor.primary_group.unwrap();
        let mut children = Vec::new();
        for _ in 0..100 {
            children.push(synthetic_firm(&mut world, Some(root)));
        }
        let group = controlled_group(&world, root);
        assert_eq!(group.len(), 101);
        for child in children {
            assert!(group.contains(&child));
        }
        let consolidated = consolidate(&world, root);
        assert_eq!(consolidated.firms.len(), group.len());
    }
}
