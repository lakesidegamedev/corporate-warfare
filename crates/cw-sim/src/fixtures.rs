//! Non-release world fixtures for performance and stress measurement.
//!
//! These constructors deliberately bypass the command layer to multiply an already valid world.
//! They are never reachable from a release session, and every clone keeps a balanced ledger. Real
//! transactions then run on the scaled world through the ordinary weekly tick.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_data::Dataset;

use crate::state::{
    Facility, FacilityStatus, HolderId, OutsideHolder, ShareLot, ShareBook, World,
};

/// Scale a valid world by cloning firms, facilities and supply contracts.
///
/// `firm_copies` includes the original, so 32 copies of a 31-firm corridor gives 992 firms.
/// `facilities_per_firm` clones each existing facility that many times for every copied firm.
/// Clones are investor-controlled so the measurement includes group and valuation traversal.
pub fn scale(base: &World, firm_copies: u32, facilities_per_firm: u32) -> World {
    let copies = firm_copies.max(1);
    let per_firm = facilities_per_firm.max(1);
    let mut world = base.clone();
    if copies == 1 && per_firm == 1 {
        return world;
    }
    let originals = base.firms.handles_sorted();
    let mut maps: Vec<BTreeMap<Handle, Handle>> = Vec::new();
    maps.push(originals.iter().map(|handle| (*handle, *handle)).collect());

    for copy in 1..copies {
        let mut map: BTreeMap<Handle, Handle> = BTreeMap::new();
        for old in &originals {
            let Some(source) = base.firms.get(*old) else {
                continue;
            };
            let mut firm = source.clone();
            firm.name = format!("{} {}", firm.name, copy);
            firm.facilities.clear();
            firm.loans.clear();
            firm.bonds.clear();
            firm.insurance.clear();
            firm.contracts_buy.clear();
            firm.contracts_sell.clear();
            firm.merged_into = None;
            firm.reserved_cash_cents = 0;
            let issued = firm.shares.issued.max(1);
            let mut holders = BTreeMap::new();
            holders.insert(
                HolderId::Investor,
                ShareLot { qty: issued * 60 / 100, cost_cents: 0 },
            );
            holders.insert(
                HolderId::Outside((copy as u32) % 8),
                ShareLot { qty: issued - issued * 60 / 100, cost_cents: 0 },
            );
            firm.shares = ShareBook::new(holders);
            firm.board.seats = [None; 5];
            let handle = world.firms.insert(firm);
            map.insert(*old, handle);
        }
        for old in &originals {
            let Some(source) = base.firms.get(*old) else {
                continue;
            };
            let new_handle = map[old];
            let facilities: Vec<Handle> = source.facilities.clone();
            for facility_handle in facilities {
                let Some(template) = base.facilities.get(facility_handle) else {
                    continue;
                };
                for _ in 0..per_firm {
                    let mut facility = template.clone();
                    facility.firm = new_handle;
                    facility.status = FacilityStatus::Operating;
                    facility.escrow_cents = 0;
                    facility.accumulated_depreciation = 0;
                    let handle = world.facilities.insert(facility);
                    world.firms.get_mut(new_handle).unwrap().facilities.push(handle);
                }
            }
        }
        // Establish the same supply relationships inside the copied economy.
        for (_, contract) in base.contracts.iter() {
            let (Some(buyer), Some(seller)) = (map.get(&contract.buyer), map.get(&contract.seller)) else {
                continue;
            };
            let mut cloned = contract.clone();
            cloned.buyer = *buyer;
            cloned.seller = *seller;
            cloned.status = crate::state::ContractStatus::Active;
            cloned.escrow_cents = 0;
            cloned.shortfalls = cw_core::quantity::MilliLots::ZERO;
            let handle = world.contracts.insert(cloned);
            world.firms.get_mut(*buyer).unwrap().contracts_buy.push(handle);
            world.firms.get_mut(*seller).unwrap().contracts_sell.push(handle);
        }
        maps.push(map);
    }

    // The original firms can also carry extra facilities for the grown and stress envelopes.
    for old in &originals {
        let Some(source) = base.firms.get(*old) else {
            continue;
        };
        let facilities: Vec<Handle> = source.facilities.clone();
        for facility_handle in facilities {
            let Some(template) = base.facilities.get(facility_handle) else {
                continue;
            };
            for _ in 1..per_firm {
                let mut facility = template.clone();
                facility.firm = *old;
                facility.status = FacilityStatus::Operating;
                let handle = world.facilities.insert(facility);
                world.firms.get_mut(*old).unwrap().facilities.push(handle);
            }
        }
    }

    // Ensure every outside investor still has finite capital to trade with.
    for (index, holder) in world.outside.investors.iter_mut().enumerate() {
        if holder.capital_cents <= 0 {
            holder.capital_cents = 100_000_000 * (index as i64 + 1);
        }
    }
    world
}

/// A compact summary used by the benchmark and its tests.
pub fn counts(world: &World) -> (usize, usize, usize) {
    (world.firms.len(), world.facilities.len(), world.contracts.len())
}

/// A named envelope from TESTING.md, scaled from the pinned corridor.
pub fn envelope(base: &World, name: &str) -> Option<World> {
    match name {
        "representative" => Some(scale(base, 1, 10)),
        "grown" => Some(scale(base, 7, 10)),
        "stress" => Some(scale(base, 32, 10)),
        _ => None,
    }
}

/// The outside investor list is public state; this keeps the fixture constructor honest about it.
pub fn outside_capital(world: &World) -> i64 {
    world.outside.investors.iter().map(|holder: &OutsideHolder| holder.capital_cents).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn scaling_multiplies_firms_and_keeps_ledgers_balanced() {
        let dataset = Dataset::load().unwrap();
        let base = seed::build(&dataset, 501, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let scaled = scale(&base, 3, 2);
        let (firms, facilities, contracts) = counts(&scaled);
        assert_eq!(firms, base.firms.len() * 3);
        assert!(facilities >= base.facilities.len() * 3);
        assert!(contracts >= base.contracts.len() * 2);
        for (_, firm) in scaled.firms.iter() {
            assert!(firm.book.is_balanced(), "{}", firm.name);
        }
    }

    #[test]
    fn stress_envelope_is_near_the_declared_targets() {
        let dataset = Dataset::load().unwrap();
        let base = seed::build(&dataset, 502, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let stress = envelope(&base, "stress").unwrap();
        let (firms, facilities, _) = counts(&stress);
        assert!((900..=1_100).contains(&firms), "firms {firms}");
        assert!((8_000..=11_000).contains(&facilities), "facilities {facilities}");
        assert!(outside_capital(&stress) > 0);
    }
}
