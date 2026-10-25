//! Boards, meetings and management authority.
//!
//! Five seats are apportioned at the thirteen-week meeting by largest-remainder voting share.
//! A holder of more than half the eligible votes may call an extraordinary meeting four weeks
//! ahead. Three seats elect or remove the CEO and approve mergers or large issuance. A tie is
//! the status quo. Owning a minority stake never grants a firm's cash.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::money::Cents;
use cw_core::money::allocate_cents;
use cw_data::Dataset;

use crate::state::{BOARD_SEATS, BoardVote, CeoRef, Firm, HolderId, ShareBook, World};

/// Apportion the five seats by largest-remainder voting share.
pub fn apportion_seats(shares: &ShareBook) -> [Option<HolderId>; BOARD_SEATS] {
    let eligible = shares.eligible_votes();
    if eligible <= 0 {
        return [None; BOARD_SEATS];
    }
    let holders: Vec<(HolderId, i64)> = shares
        .holders
        .iter()
        .filter(|(holder, _)| !holder.is_treasury())
        .map(|(holder, lot)| (*holder, lot.qty))
        .collect();
    let weights: Vec<(u64, i64)> = holders
        .iter()
        .enumerate()
        .map(|(index, (_, qty))| (index as u64, *qty))
        .collect();
    let seats = allocate_cents(Cents(BOARD_SEATS as i64), &weights);
    let mut result = [None; BOARD_SEATS];
    let mut cursor = 0usize;
    for (index, (_, count)) in seats.iter().enumerate() {
        let holder = holders[index].0;
        for _ in 0..count.0 {
            if cursor < BOARD_SEATS {
                result[cursor] = Some(holder);
                cursor += 1;
            }
        }
    }
    result
}

/// Weekly governance: apportion seats at meetings and resolve due votes.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let _ = dataset;
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let due_meeting = world
            .firms
            .get(handle)
            .map(|firm| world.tick.0 >= firm.board.next_meeting.0)
            .unwrap_or(false);
        if due_meeting {
            let seats = {
                let firm = world.firms.get(handle).unwrap();
                apportion_seats(&firm.shares)
            };
            if let Some(firm) = world.firms.get_mut(handle) {
                firm.board.seats = seats;
                firm.board.next_meeting = Tick(firm.board.next_meeting.0 + 13);
            }
        }
        // Resolved votes are kept while they can still authorise an action, then dropped.
        if let Some(firm) = world.firms.get_mut(handle) {
            let now = world.tick.0;
            firm.board
                .pending_votes
                .retain(|vote| !vote.resolved || now < vote.deadline.0 + 26);
        }
        // Resolve votes whose deadline has arrived.
        let pending: Vec<BoardVote> = world
            .firms
            .get(handle)
            .map(|firm| firm.board.pending_votes.clone())
            .unwrap_or_default();
        for vote in pending {
            if vote.resolved || world.tick.0 < vote.deadline.0 {
                continue;
            }
            let passed = resolve_vote(world, handle, &vote);
            if let Some(firm) = world.firms.get_mut(handle)
                && let Some(stored) = firm
                    .board
                    .pending_votes
                    .iter_mut()
                    .find(|candidate| candidate.id == vote.id)
            {
                stored.resolved = true;
                stored.passed = passed;
            }
        }
    }
}

/// A proposal passes with three of five seats. NPC directors support solvency-preserving
/// proposals and reject transfers below fair value or insolvency-causing payouts.
fn resolve_vote(world: &World, firm: Handle, vote: &BoardVote) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    let controller = entity.shares.controller();
    let mut yes = vote.yes as usize;
    let mut no = vote.no as usize;
    for seat in entity.board.seats.iter().flatten() {
        if Some(*seat) == controller {
            yes += 1;
            continue;
        }
        // Minority directors back proposals only when the firm stays solvent.
        let solvent = entity.book.total_equity() >= 0 && entity.book.cash() >= 0;
        if solvent && vote.subject != "conflict_transfer" {
            yes += 1;
        } else {
            no += 1;
        }
        if yes >= 3 || no >= 3 {
            break;
        }
    }
    // Tie is the status quo, which means the proposal does not pass.
    yes > no && yes >= 3
}

/// True when the firm's controller holds at least three of the five seats. Board authority is
/// required for large issuance and for mergers.
pub fn controller_has_board(world: &World, firm: Handle) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    let Some(controller) = entity.shares.controller() else {
        return false;
    };
    entity.board.seats_held(controller) >= 3
}

/// Large actions (big capex, debt above the leverage limit, large issuance, merger) need board
/// authority. A widely held firm has no controller to seat a majority, so v0.1 lets its
/// management act; a controlled firm needs three seats held by its controller.
pub fn large_action_authorised(world: &World, firm: Handle) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    // A board vote the directors passed in the last thirteen weeks also authorises it: the
    // rules let a holder without a seat majority win the other directors' support.
    let supported = entity.board.pending_votes.iter().any(|vote| {
        vote.resolved
            && vote.passed
            && vote.subject != "conflict_transfer"
            && world.tick.0 < vote.deadline.0 + 13
    });
    supported
        || match entity.shares.controller() {
            Some(controller) => entity.board.seats_held(controller) >= 3,
            None => true,
        }
}

/// Whether an actor may issue operating commands for a firm.
pub fn can_operate(world: &World, firm: Handle, actor: HolderId) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return false;
    };
    if entity.shares.controller() == Some(actor) {
        return true;
    }
    match (&entity.board.ceo, actor) {
        (Some(CeoRef::Investor), HolderId::Investor) => true,
        (Some(CeoRef::Manager(_)), HolderId::Investor) => false,
        _ => false,
    }
}

/// Board approval needed for debt above 60 percent of tangible assets or a single capex above
/// 25 percent of total assets.
pub fn approval_required(world: &World, firm: Handle, debt_after: i64, capex: i64) -> bool {
    let Some(entity) = world.firms.get(firm) else {
        return true;
    };
    let tangible = entity.book.net_tangible_assets().max(1);
    let assets = entity.book.total_assets().max(1);
    debt_after * 100 > tangible * 60 || capex * 100 > assets * 25
}

pub fn queue_vote(firm: &mut Firm, subject: &str, deadline: Tick) -> u32 {
    let id = firm
        .board
        .pending_votes
        .iter()
        .map(|vote| vote.id)
        .max()
        .unwrap_or(0)
        + 1;
    firm.board.pending_votes.push(BoardVote {
        id,
        subject: subject.to_owned(),
        deadline,
        yes: 0,
        no: 0,
        resolved: false,
        passed: false,
    });
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn book(parts: &[(HolderId, i64)]) -> ShareBook {
        let mut holders = BTreeMap::new();
        for (holder, qty) in parts {
            holders.insert(
                *holder,
                crate::state::ShareLot {
                    qty: *qty,
                    cost_cents: 0,
                },
            );
        }
        ShareBook::new(holders)
    }

    #[test]
    fn five_seats_follow_voting_share() {
        let shares = book(&[(HolderId::Investor, 600), (HolderId::Outside(1), 400)]);
        let seats = apportion_seats(&shares);
        let investor = seats
            .iter()
            .filter(|seat| **seat == Some(HolderId::Investor))
            .count();
        let outsider = seats
            .iter()
            .filter(|seat| **seat == Some(HolderId::Outside(1)))
            .count();
        assert_eq!(investor + outsider, 5);
        assert_eq!(investor, 3);
        assert_eq!(outsider, 2);
    }

    #[test]
    fn ties_are_the_status_quo() {
        let shares = book(&[(HolderId::Investor, 500), (HolderId::Outside(1), 500)]);
        let seats = apportion_seats(&shares);
        // 2.5 seats each: largest remainder gives one side three seats, never four.
        let counts = [
            seats
                .iter()
                .filter(|s| **s == Some(HolderId::Investor))
                .count(),
            seats
                .iter()
                .filter(|s| **s == Some(HolderId::Outside(1)))
                .count(),
        ];
        assert_eq!(counts.iter().sum::<usize>(), 5);
        assert!(counts.contains(&2));
    }
}
