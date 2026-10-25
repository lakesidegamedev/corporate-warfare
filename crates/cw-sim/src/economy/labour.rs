//! Regional labour clearing.
//!
//! Three independent skill pools per region. Workers are finite and cannot hold two jobs.
//! Named firms compete for voluntary searchers and the unemployed; background employers take
//! the residual. Offers below 80 percent of the prevailing wage draw no applicants. The
//! prevailing wage moves one sixteenth of the way toward the employment-weighted mean offered
//! wage each week, clamped to two percent outside declared events.

use cw_core::ids::Handle;
use cw_core::money::{Cents, allocate_cents};
use cw_data::Dataset;

use crate::state::{Facility, FacilityStatus, World};

#[derive(Clone, Copy, Debug)]
struct Offer {
    facility: Handle,
    open: i64,
    weight: i64,
}

/// Clear labour for every region and tier, then pay payroll.
pub fn clear(world: &mut World, dataset: &Dataset) {
    for region_index in 0..world.regions.len() {
        for tier in 0..3 {
            clear_tier(world, dataset, region_index, tier);
        }
    }
    pay_payroll(world);
}

fn clear_tier(world: &mut World, dataset: &Dataset, region_index: usize, tier: usize) {
    let region = &world.regions[region_index];
    let prevailing = region.labour[tier].prevailing_wage_cents.max(1);
    let reservation = region.labour[tier].reservation_wage_cents.max(1);
    let population = region.labour[tier].population;
    let cap = population * 9_500 / 10_000;

    // Named employment and the background residual.
    let mut named_employed = 0i64;
    let mut facilities: Vec<(Handle, i64, i64)> = Vec::new();
    for (handle, facility) in world.facilities.iter() {
        if facility.region as usize != region_index {
            continue;
        }
        let firm = match world.firms.get(handle) {
            Some(firm) if firm.operative() => firm,
            _ => continue,
        };
        if facility.status != FacilityStatus::Operating
            && facility.status != FacilityStatus::Building
        {
            // Mothballed facilities keep no workers.
            continue;
        }
        let workers = facility.workers[tier];
        named_employed += workers;
        facilities.push((handle, workers, firm.policies.wage_offer[tier]));
    }
    facilities.sort_by_key(|(handle, _, _)| *handle);

    let background_before = (cap - named_employed).max(0);
    // Voluntary search: two percent of all staffed workers, accumulated deterministically.
    let staffed = named_employed + background_before;
    let carry = region.labour[tier].search_carry + staffed * 20; // 2 percent in milli-workers
    let searchers = carry / 1_000;
    let carry_remainder = carry % 1_000;
    let unemployed = (cap - staffed).max(0);
    let applicants = searchers + unemployed;

    let mut offers: Vec<Offer> = Vec::new();
    for (facility_handle, workers, wage) in &facilities {
        let facility = match world.facilities.get(*facility_handle) {
            Some(facility) => facility,
            None => continue,
        };
        let full = facility_full_workers(facility, dataset, tier);
        let desired = full * facility.target_bps as i64 / 10_000;
        let open = (desired - workers).max(0);
        if open == 0 {
            continue;
        }
        if *wage * 100 < prevailing * 80 {
            continue;
        }
        let advantage = (*wage - reservation).max(0) + reservation / 10;
        let weight = advantage.max(0) * open;
        offers.push(Offer {
            facility: *facility_handle,
            open,
            weight,
        });
    }

    let mut hires = 0i64;
    if applicants > 0 && !offers.is_empty() {
        let total_weight: i64 = offers.iter().map(|offer| offer.weight).sum();
        if total_weight > 0 {
            let weights: Vec<(u64, i64)> = offers
                .iter()
                .map(|offer| (offer.facility.0, offer.weight))
                .collect();
            let split = allocate_cents(Cents(applicants), &weights);
            for (index, offer) in offers.iter().enumerate() {
                let granted = split[index].1.0.min(offer.open).max(0);
                if granted == 0 {
                    continue;
                }
                if let Some(facility) = world.facilities.get_mut(offer.facility) {
                    facility.workers[tier] += granted;
                    hires += granted;
                }
            }
        }
    }
    // Hires move workers out of the background residual, never create them.
    let named_after = named_employed + hires;
    let background_after = (cap - named_after).max(0);

    // Employment-weighted mean offered wage including background employment.
    let mut weighted_sum: i128 = 0;
    let mut weight_total: i128 = 0;
    for (handle, workers, wage) in &facilities {
        let current = world
            .facilities
            .get(*handle)
            .map(|f| f.workers[tier])
            .unwrap_or(*workers);
        weighted_sum += current as i128 * *wage as i128;
        weight_total += current as i128;
    }
    let background_wage = world.regions[region_index].background_wage_cents[tier];
    weighted_sum += background_after as i128 * background_wage as i128;
    weight_total += background_after as i128;
    let target_wage = if weight_total > 0 {
        (weighted_sum / weight_total) as i64
    } else {
        prevailing
    };
    let mut new_prevailing = prevailing + (target_wage - prevailing) / 16;
    let low_clamp = prevailing * 98 / 100;
    let high_clamp = prevailing * 102 / 100;
    new_prevailing = new_prevailing.clamp(low_clamp, high_clamp).max(1);

    let region = &mut world.regions[region_index];
    region.background_employed[tier] = background_after;
    region.labour[tier].employed = named_after + background_after;
    region.labour[tier].prevailing_wage_cents = new_prevailing;
    region.labour[tier].reservation_wage_cents = (new_prevailing * 70 / 100).max(1);
    region.labour[tier].search_carry = carry_remainder;
}

fn facility_full_workers(facility: &Facility, dataset: &Dataset, tier: usize) -> i64 {
    dataset.recipes[facility.recipe as usize].workers_low_mid_high[tier] * facility.blocks as i64
}

/// Pay this week's wages from each firm's own cash.
pub fn pay_payroll(world: &mut World) {
    let mut payroll: Vec<(Handle, i64, String)> = Vec::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.operative() {
            continue;
        }
        let mut total = 0i64;
        let mut detail = String::new();
        for facility_handle in &firm.facilities {
            if let Some(facility) = world.facilities.get(*facility_handle) {
                for tier in 0..3 {
                    let wages = facility.workers[tier] * firm.policies.wage_offer[tier];
                    total += wages;
                }
            }
        }
        if total > 0 {
            detail.push_str("payroll");
            payroll.push((handle, total, detail));
        }
    }
    for (handle, total, memo) in payroll {
        let cash = world
            .firms
            .get(handle)
            .map(|firm| firm.book.cash())
            .unwrap_or(0);
        let paid = total.min(cash.max(0));
        if paid > 0 {
            let _ = world.firms.get_mut(handle).unwrap().book.commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .credit(cw_core::ledger::Account::Cash, paid, &memo)
                    .debit(cw_core::ledger::Account::Wages, paid, &memo),
            );
        }
        if paid < total {
            crate::economy::insolvency::defer_expense(
                world,
                handle,
                total - paid,
                cw_core::ledger::Account::Wages,
                "unpaid wages",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn labour_is_finite_and_never_double_counted() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 3, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for _ in 0..6 {
            clear(&mut world, &dataset);
        }
        for region in &world.regions {
            for tier in 0..3 {
                let cap = region.labour[tier].population * 9_500 / 10_000;
                assert!(
                    region.labour[tier].employed <= cap,
                    "employed {} exceeds cap {}",
                    region.labour[tier].employed,
                    cap
                );
            }
        }
    }

    #[test]
    fn a_firm_that_offers_nothing_gets_no_workers() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 5, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for (_, firm) in world.firms.iter_mut() {
            firm.policies.wage_offer = [1, 1, 1];
        }
        for _ in 0..4 {
            clear(&mut world, &dataset);
        }
        for (_, facility) in world.facilities.iter() {
            assert_eq!(facility.workers, [0, 0, 0]);
        }
    }
}
