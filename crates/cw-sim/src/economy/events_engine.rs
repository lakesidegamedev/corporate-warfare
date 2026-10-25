//! Event deck selection and resolution.
//!
//! Cards reveal or modify the same rules. Random opportunity cards are checked at quarter
//! boundaries: at most one eligible card per region, a ten percent chance, a stable sorted
//! eligible list and the event stream. A firm can hold at most two unresolved optional cards;
//! mandatory state-transition notifications bypass that cap. No card repeats on the same
//! subject within 26 weeks. Every choice has a persisted result and never bypasses solvency or
//! authority rules.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;
use cw_core::rng::{Rng, TAG_RESEARCH};
use cw_data::Dataset;

use crate::state::{BargainState, FacilityStatus, PendingEvent, RegimeKind, World};

/// Quarter-boundary checks: macro impulse and at most one opportunity card per region.
pub fn quarter_check(world: &mut World, dataset: &Dataset) {
    if world.tick.0 == 0 || !world.tick.0.is_multiple_of(13) {
        return;
    }
    if world.events.last_quarter_check == world.tick.0 {
        return;
    }
    world.events.last_quarter_check = world.tick.0;

    // One macro impulse, using the dedicated macro stream.
    let impulse = {
        let choices = [-100i32, -50, 0, 0, 0, 50, 100];
        let index = world.macro_state.rng.uniform_below(choices.len() as u64) as usize;
        choices[index]
    };
    world.macro_state.policy_rate_bps =
        (world.macro_state.policy_rate_bps + impulse).clamp(100, 1_500);
    // Nominal reference price index grows at the annual inflation rate with a carried remainder.
    let growth = round_even_div(
        world.macro_state.price_index_bps as i128 * world.macro_state.inflation_bps as i128,
        10_000 * 52,
    ) as i64;
    world.macro_state.price_index_bps += growth.max(1);

    // Clear active regimes at expiry.
    if let Some((kind, until)) = world.macro_state.regime
        && world.tick.0 >= until.0
    {
        world.macro_state.regime = None;
        let _ = kind;
    }

    let mut region_cards: BTreeMap<u8, String> = BTreeMap::new();
    for region in 0..world.regions.len() {
        let mut candidates: Vec<&str> = dataset
            .events
            .iter()
            .filter(|card| !card.mandatory)
            // Macro regimes belong to the macro stream, not to a regional firm card.
            .filter(|card| !card.trigger.starts_with("macro_"))
            .filter(|card| eligible_quiet(world, card.trigger.as_str(), region as u8))
            .map(|card| card.id.as_str())
            .collect();
        candidates.sort_unstable();
        if candidates.is_empty() {
            continue;
        }
        let roll = world.events.rng.uniform_below(10_000);
        if roll >= 1_000 {
            continue;
        }
        let index = world.events.rng.uniform_below(candidates.len() as u64) as usize;
        region_cards.insert(region as u8, candidates[index].to_owned());
    }
    for (region, card) in region_cards {
        let subject = world
            .firms
            .iter()
            .find(|(_, firm)| firm.region == region && firm.operative())
            .map(|(handle, _)| handle);
        open(world, &card, subject, Some(region), false);
    }
}

/// Eligibility checks that do not depend on a specific firm, used for quiet quarter draws.
fn eligible_quiet(world: &World, trigger: &str, region: u8) -> bool {
    match trigger {
        "random_energy_region" => world
            .firms
            .iter()
            .any(|(_, firm)| firm.industry == 2 && firm.region == region && firm.operative()),
        "quarter_crop_event" => world.firms.iter().any(|(_, firm)| firm.industry == 0),
        "deposit_below_26_weeks" => world.firms.iter().any(|(_, firm)| firm.industry == 1),
        "macro_no_active_fuel_shock" => world.macro_state.regime.is_none(),
        "macro_selection_no_regime" => world.macro_state.regime.is_none(),
        "random_active_freight_lane" => true,
        "lane_utilisation_above_90" => true,
        "government_budget_available" => world.government.training_budget_cents > 0,
        "region_disaster_no_active" => world.regions[region as usize].catastrophe_until.is_none(),
        "tech_level_below_frontier" => world
            .firms
            .iter()
            .any(|(_, firm)| firm.tech_level < world.macro_state.tech_frontier),
        _ => false,
    }
}

/// Resolve optional cards addressed to firms the investor does not control. The rival uses the
/// same handler catalogue and the same postings; only the choice is automatic. Player cards stay
/// open for an explicit answer.
pub fn auto_resolve(world: &mut World, dataset: &Dataset) {
    let mut pending: Vec<(u64, String, bool)> = Vec::new();
    let candidates: Vec<(u64, String, Option<Handle>)> = world
        .events
        .pending
        .iter()
        .filter(|event| !event.resolved && !event.mandatory)
        .map(|event| (event.id, event.card.clone(), event.subject_firm))
        .collect();
    for (id, card_id, subject) in candidates {
        let Some(firm) = subject else { continue };
        let controlled = world
            .firms
            .get(firm)
            .map(|firm| firm.shares.qty(crate::state::HolderId::Investor) > 0)
            .unwrap_or(false);
        if controlled {
            continue;
        }
        let Some(card) = dataset.event(&card_id) else {
            continue;
        };
        let index = world
            .events
            .rng
            .uniform_below(card.choices.len().max(1) as u64) as usize;
        if let Some(choice) = card.choices.get(index) {
            pending.push((id, choice.id.clone(), true));
        }
    }
    for (id, choice, _) in pending {
        let _ = respond(world, dataset, id, &choice);
    }
}

/// Optional cards expire at their deadline with an explicit no-action outcome. They otherwise
/// accumulate, block new cards and wait forever for a player who never opens the inbox.
pub fn expire_optional(world: &mut World, dataset: &Dataset) {
    let due: Vec<(u64, String)> = world
        .events
        .pending
        .iter()
        .filter(|event| !event.resolved && !event.mandatory && event.deadline.0 <= world.tick.0)
        .filter_map(|event| {
            let card = dataset.event(&event.card)?;
            let choice = card
                .choices
                .iter()
                .find(|choice| choice.handler == "no_action")
                .or_else(|| card.choices.last())
                .map(|choice| choice.id.clone())?;
            Some((event.id, choice))
        })
        .collect();
    for (id, choice) in due {
        let _ = respond(world, dataset, id, &choice);
    }
}

pub fn open(
    world: &mut World,
    card: &str,
    subject: Option<Handle>,
    region: Option<u8>,
    mandatory: bool,
) -> u64 {
    // Cap unresolved optional cards per firm at two.
    if let Some(firm) = subject {
        let count = world
            .events
            .pending
            .iter()
            .filter(|event| !event.resolved && event.subject_firm == Some(firm) && !event.mandatory)
            .count();
        if count >= 2 && !mandatory {
            return 0;
        }
    }
    // No repeat on the same subject within 26 weeks.
    let key = (card.to_owned(), subject.map(|handle| handle.0).unwrap_or(0));
    if let Some(last) = world.events.cooldowns.get(&key)
        && world.tick.0 < last + 26
    {
        return 0;
    }
    let id = world.events.next_id;
    world.events.next_id += 1;
    world.events.cooldowns.insert(key, world.tick.0);
    world.events.pending.push(PendingEvent {
        id,
        card: card.to_owned(),
        subject_firm: subject,
        region,
        deadline: Tick(world.tick.0 + 4),
        opened: world.tick,
        mandatory,
        resolved: false,
    });
    id
}

/// Resolve one pending event choice.
pub fn respond(
    world: &mut World,
    dataset: &Dataset,
    event_id: u64,
    choice_id: &str,
) -> DomainResult<()> {
    let Some(index) = world
        .events
        .pending
        .iter()
        .position(|event| event.id == event_id)
    else {
        return Err(DomainError::InvalidEntity {
            handle: event_id,
            kind: EntityKind::Event,
        });
    };
    let event = world.events.pending[index].clone();
    if event.resolved {
        return Err(DomainError::WrongPhase {
            expected: "open",
            actual: "resolved",
        });
    }
    let Some(card) = dataset.event(&event.card) else {
        return Err(DomainError::InvalidEntity {
            handle: event_id,
            kind: EntityKind::Event,
        });
    };
    let Some(choice) = card.choices.iter().find(|choice| choice.id == choice_id) else {
        return Err(DomainError::Arithmetic {
            detail: cw_core::error::ArithmeticFault::OutOfRange,
        });
    };
    apply(world, dataset, &event, &choice.handler)?;
    if let Some(stored) = world.events.pending.get_mut(index) {
        stored.resolved = true;
    }
    if world.pending_pause.as_deref() == Some(format!("event:{event_id}").as_str()) {
        world.pending_pause = None;
    }
    Ok(())
}

fn subject_firm(world: &World, event: &PendingEvent) -> Option<Handle> {
    event
        .subject_firm
        .filter(|handle| world.firms.contains(*handle))
}

fn apply(
    world: &mut World,
    dataset: &Dataset,
    event: &PendingEvent,
    handler: &str,
) -> DomainResult<()> {
    let firm = subject_firm(world, event);
    match handler {
        "terminate_supplier_contract" => {
            if let Some(buyer) = firm {
                let contracts: Vec<Handle> = world
                    .contracts
                    .iter()
                    .filter(|(_, contract)| contract.buyer == buyer)
                    .map(|(handle, _)| handle)
                    .collect();
                for handle in contracts {
                    if let Some(contract) = world.contracts.get_mut(handle) {
                        let refund = contract.escrow_cents;
                        contract.status = crate::state::ContractStatus::Cancelled;
                        contract.escrow_cents = 0;
                        if refund > 0
                            && let Some(entity) = world.firms.get_mut(buyer)
                        {
                            let _ = entity.book.commit(
                                world.tick,
                                Txn::new()
                                    .debit(Account::Cash, refund, "escrow refund")
                                    .credit(Account::RestrictedCash, refund, "escrow refund"),
                            );
                        }
                    }
                }
            }
        }
        "keep_supplier_claim" => {}
        "buy_backup_power" => {
            if let Some(firm) = firm {
                let cost = facility_power_cost(world, dataset, firm) * 2;
                pay_operating(world, firm, cost, "backup power");
                set_facility_target(world, firm, 7_500);
            }
        }
        "accept_power_cut" => {
            if let Some(firm) = firm {
                set_facility_target(world, firm, 5_000);
            }
        }
        "buy_fuel_stock" => {
            if let Some(firm) = firm {
                let product = dataset.product_index["fuel"];
                let price =
                    dataset.reference_price(product) + dataset.reference_price(product) * 25 / 100;
                let qty = cw_core::quantity::MilliLots::from_lots(50);
                let cost = qty
                    .value_at(cw_core::money::Cents(price))
                    .unwrap_or_default()
                    .0;
                let cash = world
                    .firms
                    .get(firm)
                    .map(|f| f.book.cash())
                    .unwrap_or(0)
                    .max(0);
                let paid = cost.min(cash);
                if paid > 0 {
                    let region = world.firms.get(firm).map(|f| f.region).unwrap_or(0);
                    let _ = world.firms.get_mut(firm).unwrap().book.commit(
                        world.tick,
                        Txn::new()
                            .credit(Account::Cash, paid, "fuel purchase")
                            .debit(Account::Inventory, paid, "fuel purchase"),
                    );
                    world
                        .firms
                        .get_mut(firm)
                        .unwrap()
                        .stock_lot_mut(product, region)
                        .receive(qty, cw_core::money::Cents(paid));
                }
            }
        }
        "no_action" => {}
        "reroute_lane" => {
            for shipment in &mut world.shipments {
                shipment.arrive = Tick(shipment.arrive.0 + 1);
                shipment.freight_cents += shipment.freight_cents * 20 / 100;
            }
        }
        "pause_lane" => {
            world.shipments.clear();
        }
        "reserve_carrier" => {
            if let Some(firm) = firm {
                let fee = freight_reserve_fee(world, dataset, firm);
                pay_operating(world, firm, fee, "carrier reservation");
            }
        }
        "spot_delay" => {
            for shipment in &mut world.shipments {
                shipment.arrive = Tick(shipment.arrive.0 + 1);
            }
        }
        "mitigate_crop" | "absorb_crop" => {
            let reduction = if handler == "mitigate_crop" {
                -1_000
            } else {
                -2_500
            };
            if let Some(region) = event.region {
                world.regions[region as usize].supply_modifiers.push((
                    handler.to_owned(),
                    Some(dataset.product_index["food_raw"]),
                    reduction,
                    Tick(world.tick.0 + 13),
                ));
            }
            if handler == "mitigate_crop"
                && let Some(firm) = firm
            {
                let blocks: i64 = world
                    .firms
                    .get(firm)
                    .map(|entity| {
                        entity
                            .facilities
                            .iter()
                            .filter_map(|h| world.facilities.get(*h))
                            .map(|f| f.blocks as i64)
                            .sum()
                    })
                    .unwrap_or(0);
                pay_operating(world, firm, blocks.max(1) * 1_000_000, "crop mitigation");
            }
        }
        "retool_deposit" => {
            if let Some(firm) = firm {
                let handle = world
                    .firms
                    .get(firm)
                    .and_then(|entity| entity.facilities.first().copied());
                if let Some(facility) = handle {
                    let capex = world
                        .facilities
                        .get(facility)
                        .map(|f| f.capex_cents)
                        .unwrap_or(0);
                    let cost = capex * 50 / 100;
                    pay_operating(world, firm, cost, "deposit retool");
                    if let Some(stored) = world.facilities.get_mut(facility) {
                        stored.deposit_remaining =
                            cw_core::quantity::MilliLots::from_lots(52 * 20 * 100);
                    }
                }
            }
        }
        "plan_depletion" => {}
        "full_recall" | "voluntary_refund" => {
            let rate = if handler == "full_recall" { 10 } else { 25 };
            let reputation = if handler == "full_recall" { 250 } else { 50 };
            if let Some(firm) = firm {
                let sold: i64 = world
                    .firms
                    .get(firm)
                    .map(|entity| {
                        entity
                            .sales_history
                            .iter()
                            .flat_map(|week| week.values().copied())
                            .sum()
                    })
                    .unwrap_or(0);
                let price = dataset.reference_price(0);
                let cost = sold * price * rate / 100;
                pay_operating(world, firm, cost, "quality recall");
                if let Some(entity) = world.firms.get_mut(firm) {
                    entity.reputation_bps -= reputation;
                }
            }
        }
        "raise_offer" => {
            if let Some(firm) = firm
                && let Some(entity) = world.firms.get_mut(firm)
            {
                for wage in &mut entity.policies.wage_offer {
                    *wage = *wage * 105 / 100;
                }
            }
        }
        "fund_training" => {
            if let Some(firm) = firm {
                pay_operating(world, firm, 10_000_000, "training");
            }
        }
        "allow_ballot" => {
            if let Some(firm) = firm
                && let Some(entity) = world.firms.get_mut(firm)
            {
                entity.union.state = BargainState::Ballot;
                entity.union.ballot_support_bps = 5_500;
            }
        }
        "improve_conditions" => {
            if let Some(firm) = firm {
                let quarterly_payroll = world
                    .firms
                    .get(firm)
                    .map(|entity| entity.book.lifetime().wages / 52 * 13)
                    .unwrap_or(0);
                let cost = (quarterly_payroll / 100).max(0);
                pay_operating(world, firm, cost, "conditions");
                if let Some(entity) = world.firms.get_mut(firm) {
                    entity.union.ballot_support_bps =
                        (entity.union.ballot_support_bps - 250).max(0);
                }
            }
        }
        "bargain_accept" => {
            if let Some(firm) = firm {
                crate::economy::unions::accept(world, firm);
            }
        }
        "bargain_counter" => {
            if let Some(firm) = firm {
                let current = world
                    .firms
                    .get(firm)
                    .map(|f| f.policies.wage_offer[0])
                    .unwrap_or(0);
                crate::economy::unions::counter(world, firm, current + current / 50);
            }
        }
        "bargain_reject" => {
            if let Some(firm) = firm {
                crate::economy::unions::reject(world, firm);
            }
        }
        "strike_settle" => {
            if let Some(firm) = firm {
                crate::economy::unions::accept(world, firm);
            }
        }
        "strike_continue" => {}
        "accept_training_grant" => {
            if let Some(firm) = firm {
                let cost = 10_000_000 * 80 / 100;
                pay_operating(world, firm, cost, "training match");
                let grant = (10_000_000 * 20 / 100).min(world.government.training_budget_cents);
                if grant > 0 {
                    world.government.training_budget_cents -= grant;
                    let _ = world.government.book.commit(
                        world.tick,
                        Txn::new()
                            .credit(Account::Cash, grant, "training grant")
                            .debit(Account::OperatingExpense, grant, "training grant"),
                    );
                    if let Some(entity) = world.firms.get_mut(firm) {
                        let _ = entity.book.commit(
                            world.tick,
                            Txn::new()
                                .debit(Account::Cash, grant, "training grant")
                                .credit(Account::OtherIncome, grant, "training grant"),
                        );
                    }
                }
            }
        }
        "expedite_permit" => {
            if let Some(firm) = firm {
                let capex = world
                    .firms
                    .get(firm)
                    .map(|entity| {
                        entity
                            .facilities
                            .iter()
                            .filter_map(|h| world.facilities.get(*h))
                            .map(|f| f.capex_cents)
                            .sum::<i64>()
                    })
                    .unwrap_or(0);
                pay_operating(world, firm, capex * 2 / 100, "permit expedite");
            }
        }
        "accept_permit_delay" => {}
        "disclose_lobbying" | "cancel_lobbying" => {
            if let Some(firm) = firm {
                crate::economy::policy::backlash(world, firm, handler == "cancel_lobbying");
            }
        }
        "submit_remedy" => {
            let deal = world
                .deals
                .iter()
                .find(|(_, deal)| !deal.state.terminal())
                .map(|(handle, _)| handle);
            if let Some(deal) = deal {
                let _ = crate::economy::deals::submit_remedy(world, deal, 500);
            }
        }
        "withdraw_bid" => {
            let deal = world
                .deals
                .iter()
                .find(|(_, deal)| !deal.state.terminal())
                .map(|(handle, _)| handle);
            if let Some(deal) = deal {
                let _ = crate::economy::deals::withdraw(world, deal);
            }
        }
        "schedule_defence" => {
            if let Some(firm) = firm
                && let Some(entity) = world.firms.get_mut(firm)
            {
                entity.board.next_meeting = Tick(world.tick.0 + 4);
            }
        }
        "tender_shares" => {
            if let Some(firm) = firm {
                let _ = firm;
            }
        }
        "refuse_tender" => {}
        "cure_covenant" | "renegotiate_covenant" => {}
        "bank_run" => {
            let bank = world
                .firms
                .iter()
                .find(|(_, firm)| firm.bank.is_some() && firm.operative())
                .map(|(handle, _)| handle);
            if let Some(bank) = bank {
                crate::economy::banking::run(world, bank);
            }
        }
        "raise_liquidity" => {}
        "catastrophe_rebuild" | "catastrophe_mothball" => {
            if let Some(region) = event.region {
                crate::economy::insurance::catastrophe(world, region);
                if handler == "catastrophe_mothball"
                    && let Some(firm) = firm
                {
                    let facilities: Vec<Handle> = world
                        .firms
                        .get(firm)
                        .map(|entity| entity.facilities.clone())
                        .unwrap_or_default();
                    for handle in facilities {
                        if let Some(facility) = world.facilities.get_mut(handle) {
                            facility.status = FacilityStatus::Mothballed;
                            facility.mothballed_since = Some(world.tick);
                        }
                    }
                }
            }
        }
        "fund_tech_opportunity" => {
            if let Some(firm) = firm {
                let level = world.firms.get(firm).map(|f| f.tech_level).unwrap_or(0);
                let cost = crate::economy::research::threshold(level);
                pay_operating(world, firm, cost, "technology opportunity");
                let mut rng = Rng::entity_stream(world.seed, TAG_RESEARCH, firm);
                if rng.chance_bps(9_000)
                    && let Some(entity) = world.firms.get_mut(firm)
                {
                    entity.tech_level =
                        (entity.tech_level + 1).min(crate::economy::research::MAX_TECH_LEVEL);
                }
            }
        }
        "accept_recession" => {
            world.macro_state.regime = Some((RegimeKind::Recession, Tick(world.tick.0 + 26)));
            apply_demand_regime(world, -1_000);
        }
        "accept_boom" => {
            world.macro_state.regime = Some((RegimeKind::Boom, Tick(world.tick.0 + 26)));
            apply_demand_regime(world, 1_000);
        }
        "energy_outage" => {}
        other => {
            let _ = other;
        }
    }
    Ok(())
}

fn apply_demand_regime(world: &mut World, delta_bps: i32) {
    for region in &mut world.regions {
        region.demand_modifiers.push((
            "macro_regime".to_owned(),
            None,
            delta_bps,
            Tick(world.tick.0 + 26),
        ));
    }
}

fn pay_operating(world: &mut World, firm: Handle, amount: i64, memo: &str) {
    if amount <= 0 {
        return;
    }
    crate::economy::insolvency::pay_or_defer(world, firm, amount, Account::OperatingExpense, memo);
}

fn set_facility_target(world: &mut World, firm: Handle, target_bps: i32) {
    let facilities: Vec<Handle> = world
        .firms
        .get(firm)
        .map(|entity| entity.facilities.clone())
        .unwrap_or_default();
    for handle in facilities {
        if let Some(facility) = world.facilities.get_mut(handle)
            && facility.status == FacilityStatus::Operating
        {
            facility.target_bps = target_bps;
        }
    }
}

fn facility_power_cost(world: &World, dataset: &Dataset, firm: Handle) -> i64 {
    let price = dataset.reference_price(dataset.product_index["electricity"]);
    let blocks: i64 = world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .map(|facility| {
                    let recipe = &dataset.recipes[facility.recipe as usize];
                    recipe
                        .weekly_inputs_lots
                        .get("electricity")
                        .copied()
                        .unwrap_or(0)
                        * facility.blocks as i64
                })
                .sum::<i64>()
        })
        .unwrap_or(0);
    blocks * price
}

fn freight_reserve_fee(world: &World, dataset: &Dataset, firm: Handle) -> i64 {
    let region = world.firms.get(firm).map(|f| f.region).unwrap_or(0);
    let km = dataset
        .lane_km("035", &dataset.regions[region as usize].code)
        .unwrap_or(300);
    (km as i64 * 100).max(100_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn the_deck_has_distinct_cards_and_every_choice_has_a_known_handler() {
        let dataset = Dataset::load().unwrap();
        assert!(dataset.events.len() >= 24, "at least twenty-four cards");
        let mut world = seed::build(&dataset, 111, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        let mut card_ids = std::collections::BTreeSet::new();
        for card in dataset.events.clone() {
            assert!(
                card_ids.insert(card.id.clone()),
                "duplicate card {}",
                card.id
            );
            assert!(!card.trigger.is_empty(), "{} has no trigger", card.id);
            assert!(!card.choices.is_empty(), "{} has no choices", card.id);
            let mut choice_ids = std::collections::BTreeSet::new();
            for choice in &card.choices {
                assert!(
                    choice_ids.insert(choice.id.clone()),
                    "{} has duplicate choice {}",
                    card.id,
                    choice.id
                );
                assert!(
                    !choice.handler.is_empty(),
                    "{} has an empty handler",
                    card.id
                );
                // Exercise each choice in isolation so the two-card cap and cooldown do not
                // hide a handler.
                world.events.pending.clear();
                world.events.cooldowns.clear();
                let event_id = open(&mut world, &card.id, Some(primary), Some(3), false);
                assert!(event_id > 0, "{} did not open", card.id);
                let result = respond(&mut world, &dataset, event_id, &choice.id);
                assert!(result.is_ok(), "{} {}: {:?}", card.id, choice.id, result);
                assert!(world.firm(primary).unwrap().book.is_balanced());
            }
        }
    }

    #[test]
    fn rival_cards_resolve_automatically_but_player_cards_do_not() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            196,
            crate::state::ScenarioKind::Guided,
            crate::state::ScenarioMode::Sandbox,
        );
        let player_firm = world.investor.primary_group.unwrap();
        let rival = world
            .firms
            .iter()
            .find(|(handle, _)| *handle != player_firm)
            .map(|(handle, _)| handle)
            .unwrap();
        let rival_event = open(&mut world, "crop_loss", Some(rival), None, false);
        let player_event = open(&mut world, "quality_recall", Some(player_firm), None, false);
        auto_resolve(&mut world, &dataset);
        let rival_state = world
            .events
            .pending
            .iter()
            .find(|e| e.id == rival_event)
            .unwrap();
        let player_state = world
            .events
            .pending
            .iter()
            .find(|e| e.id == player_event)
            .unwrap();
        assert!(rival_state.resolved, "rival cards answer themselves");
        assert!(!player_state.resolved, "player cards wait for the player");
    }

    #[test]
    fn macro_regimes_are_not_firm_addressed_region_cards() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            198,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        for quarter in 1..=40u32 {
            world.tick = Tick(quarter * 13);
            quarter_check(&mut world, &dataset);
        }
        assert!(
            world
                .events
                .pending
                .iter()
                .all(|event| event.card != "recession" && event.card != "demand_boom"),
            "macro cards are drawn from the macro stream, never from regional firm draws"
        );
    }

    #[test]
    fn optional_cards_expire_at_their_deadline() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            197,
            crate::state::ScenarioKind::Guided,
            crate::state::ScenarioMode::Sandbox,
        );
        let firm = world.investor.primary_group.unwrap();
        let id = open(&mut world, "crop_loss", Some(firm), None, false);
        assert!(
            !world
                .events
                .pending
                .iter()
                .find(|e| e.id == id)
                .unwrap()
                .resolved
        );
        world.tick = Tick(8);
        expire_optional(&mut world, &dataset);
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
    fn a_card_does_not_repeat_within_26_weeks() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 112, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        assert!(open(&mut world, "crop_loss", Some(primary), Some(3), false) > 0);
        assert_eq!(
            open(&mut world, "crop_loss", Some(primary), Some(3), false),
            0
        );
    }
}
