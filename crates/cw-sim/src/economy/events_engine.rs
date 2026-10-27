//! Event deck selection and resolution.
//!
//! Cards reveal or modify the same rules. Random opportunity cards are checked at quarter
//! boundaries: at most one eligible card per region, a ten percent chance, a stable sorted
//! eligible list and the event stream. A firm can hold at most two unresolved optional cards;
//! mandatory state-transition notifications bypass that cap. No card repeats on the same
//! subject within 26 weeks. Every choice has a persisted result and never bypasses solvency or
//! authority rules.

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
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

    // Rates, inflation and the cycle run weekly in `macro_cycle`; the quarter only draws shocks.
    // Clear active regimes at expiry.
    if let Some((kind, until)) = world.macro_state.regime
        && world.tick.0 >= until.0
    {
        world.macro_state.regime = None;
        let _ = kind;
    }

    macro_draw(world, dataset);

    // At most one opportunity card per region: a ten percent chance, then a stable sorted list
    // of cards with at least one eligible firm in the region, then one eligible subject.
    for region in 0..world.regions.len() as u8 {
        let mut candidates: Vec<(String, bool)> = dataset
            .events
            .iter()
            .filter(|card| QUIET_TRIGGERS.contains(&card.trigger.as_str()))
            .filter(|card| !eligible_subjects(world, dataset, &card.trigger, region).is_empty())
            .map(|card| (card.id.clone(), card.mandatory))
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
        let (card_id, mandatory) = candidates[index].clone();
        let Some(card) = dataset.event(&card_id) else {
            continue;
        };
        let subjects = eligible_subjects(world, dataset, &card.trigger, region);
        let pick = world.events.rng.uniform_below(subjects.len() as u64) as usize;
        let subject = subjects[pick];
        let id = open(world, &card_id, Some(subject), Some(region), mandatory);
        // A catastrophe is a physical loss that happens once when the card opens; the choice
        // only decides what to do with the damaged plant.
        if id > 0 && card.trigger == "region_disaster_no_active" {
            crate::economy::insurance::catastrophe(world, region);
        }
    }
}

/// Triggers drawn at random at the quarter boundary. Every other card opens only when its
/// actual state transition happens (see `state_triggers`); macro cards come from the macro
/// stream.
const QUIET_TRIGGERS: [&str; 9] = [
    "random_energy_region",
    "quarter_crop_event",
    "deposit_below_26_weeks",
    "random_active_freight_lane",
    "lane_utilisation_above_90",
    "government_budget_available",
    "region_disaster_no_active",
    "tech_level_below_frontier",
    "region_crisis_or_bank_shock",
];

/// The player investor ultimately controls this firm.
pub fn player_side(world: &World, firm: Handle) -> bool {
    crate::economy::ownership::ultimate_controller(world, firm)
        == Some(crate::state::HolderId::Investor)
}

/// A fuel shock at most once a quarter, from the macro stream, only when no regime is active.
/// Booms and recessions come from the business cycle in `macro_cycle`, which opens their cards.
fn macro_draw(world: &mut World, dataset: &Dataset) {
    if world.macro_state.regime.is_some() {
        return;
    }
    if world.macro_state.rng.uniform_below(10_000) >= 330 {
        return;
    }
    let card = "fuel_spike";
    // Fuel supply falls a fifth for thirteen weeks, which lifts the executable fuel price
    // through the normal market rather than by decree, and adds to inflation.
    world.macro_state.regime = Some((RegimeKind::FuelShock, Tick(world.tick.0 + 13)));
    world.macro_state.inflation_bps += 40;
    let fuel = dataset.product_index["fuel"];
    for region in &mut world.regions {
        region.supply_modifiers.push((
            "fuel_shock".to_owned(),
            Some(fuel),
            -2_000,
            Tick(world.tick.0 + 13),
        ));
    }
    let subject = world
        .investor
        .primary_group
        .filter(|handle| world.firms.contains(*handle));
    open(world, card, subject, None, false);
}

fn uses_input(dataset: &Dataset, world: &World, firm: Handle, input: &str) -> bool {
    world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .any(|facility| {
                    facility.status == FacilityStatus::Operating
                        && dataset.recipes[facility.recipe as usize]
                            .weekly_inputs_lots
                            .contains_key(input)
                })
        })
        .unwrap_or(false)
}

fn has_facility_in(world: &World, firm: Handle, region: u8) -> bool {
    world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .any(|facility| {
                    facility.region == region && facility.status != FacilityStatus::Building
                })
        })
        .unwrap_or(false)
}

fn in_transit(world: &World, firm: Handle) -> bool {
    world
        .shipments
        .iter()
        .any(|shipment| shipment.buyer == firm || shipment.seller == firm)
}

/// Operating firms in the region that the card's trigger actually applies to, in stable order.
fn eligible_subjects(world: &World, dataset: &Dataset, trigger: &str, region: u8) -> Vec<Handle> {
    let mut found: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative() && firm.region == region)
        .map(|(handle, _)| handle)
        .filter(|handle| {
            let Some(firm) = world.firms.get(*handle) else {
                return false;
            };
            match trigger {
                "random_energy_region" => uses_input(dataset, world, *handle, "electricity"),
                "quarter_crop_event" => {
                    firm.industry == 0 && has_facility_in(world, *handle, region)
                }
                "deposit_below_26_weeks" => firm
                    .facilities
                    .iter()
                    .filter_map(|f| world.facilities.get(*f))
                    .any(|facility| {
                        let recipe = &dataset.recipes[facility.recipe as usize];
                        let weekly = recipe.weekly_output_lots * facility.blocks as i64;
                        recipe.depletes()
                            && facility.status == FacilityStatus::Operating
                            && facility.deposit_remaining.lots() < weekly * 26
                    }),
                "random_active_freight_lane" | "lane_utilisation_above_90" => {
                    in_transit(world, *handle)
                }
                "government_budget_available" => {
                    world.government.training_budget_cents > 0
                        && has_facility_in(world, *handle, region)
                }
                "region_disaster_no_active" => {
                    world.regions[region as usize].catastrophe_until.is_none()
                        && has_facility_in(world, *handle, region)
                }
                "tech_level_below_frontier" => firm.tech_level < world.macro_state.tech_frontier,
                "region_crisis_or_bank_shock" => firm.bank.is_some(),
                _ => false,
            }
        })
        .collect();
    found.sort_unstable();
    found
}

/// Weekly state-transition cards. Each opens only when its documented state actually holds;
/// the 26-week subject cooldown in `open` keeps a persistent state from reopening the card.
/// Cards whose answer is a rival's own command (bargaining, review, tenders, covenants) open
/// only for the player's companies, because the rival AI answers through the command path.
pub fn state_triggers(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    // Staffing history for churn.
    for handle in &handles {
        let employed = world
            .firms
            .get(*handle)
            .map(|firm| firm.total_employees(&world.facilities))
            .unwrap_or(0);
        if let Some(firm) = world.firms.get_mut(*handle) {
            firm.staff_history.push(employed);
            if firm.staff_history.len() > 5 {
                firm.staff_history.remove(0);
            }
        }
    }
    let mut opens: Vec<(&'static str, Handle, Option<u8>, bool)> = Vec::new();
    for handle in &handles {
        let Some(firm) = world.firms.get(*handle) else {
            continue;
        };
        if !firm.operative() {
            continue;
        }
        let player = player_side(world, *handle);
        let region = Some(firm.region);
        // Supplier default: an actual supplier on an active contract is in distress.
        let supplier_down = world.contracts.iter().any(|(_, contract)| {
            contract.buyer == *handle
                && contract.status == crate::state::ContractStatus::Active
                && world
                    .firms
                    .get(contract.seller)
                    .map(|seller| {
                        matches!(
                            seller.status,
                            crate::state::FirmStatus::PastDue { .. }
                                | crate::state::FirmStatus::Restructuring { .. }
                        )
                    })
                    .unwrap_or(false)
        });
        if supplier_down {
            opens.push(("supplier_default", *handle, region, false));
        }
        // Quality recall: a product below 8,000 quality bps with sales in the last four weeks.
        if recall_products(world, *handle)
            .iter()
            .any(|(_, units)| *units > 0)
        {
            opens.push(("quality_recall", *handle, region, false));
        }
        // Talent poaching: more than a tenth of the workforce left in four weeks, not counting
        // the firm's own layoffs.
        if let (Some(then), Some(now)) = (firm.staff_history.first(), firm.staff_history.last())
            && firm.staff_history.len() == 5
            && *then > 0
        {
            let laid_off: i64 = firm.layoffs.iter().map(|(_, count)| *count).sum();
            let lost = then - now - laid_off;
            if lost * 10 > *then {
                opens.push(("talent_poaching", *handle, region, false));
            }
        }
        // Permit delay: a project started this week where no permit bonus is active.
        let fresh_build = firm
            .facilities
            .iter()
            .filter_map(|f| world.facilities.get(*f))
            .find(|facility| {
                // Commands run before the week advances, so a project ordered last week
                // started one tick before this check.
                facility.status == FacilityStatus::Building
                    && facility.started.0 + 1 >= world.tick.0
                    && world.regions[facility.region as usize]
                        .policy
                        .permit_bonus_until
                        .map(|until| until.0 <= world.tick.0)
                        .unwrap_or(true)
            })
            .map(|facility| facility.region);
        if let Some(build_region) = fresh_build {
            opens.push(("permit_delay", *handle, Some(build_region), false));
        }
        // Lobby backlash: an open campaign while the group is laying off a tenth of its staff.
        if firm.policy_requests.iter().any(|request| !request.resolved)
            && crate::economy::policy::mass_layoffs(world, *handle)
        {
            opens.push(("lobbying_backlash", *handle, region, false));
        }
        if player {
            match firm.union.state {
                BargainState::Demanding { .. } => {
                    opens.push(("wage_negotiation", *handle, region, true))
                }
                BargainState::Striking { .. } => {
                    opens.push(("strike_escalation", *handle, region, true))
                }
                _ => {}
            }
            // Hostile stake: an outside holder crosses a quarter of the votes this week. A
            // long-standing co-investor above the line is not news.
            let peak = outside_peak_bps(world, *handle);
            if peak >= 2_500 && firm.outside_peak_bps < 2_500 {
                opens.push(("hostile_stake", *handle, region, false));
            }
            // Covenant warning: debt above sixty percent of tangible assets.
            let tangible = firm.book.net_tangible_assets();
            if !firm.loans.is_empty() && firm.book.debt() * 100 > tangible.max(0) * 60 {
                opens.push(("covenant_warning", *handle, region, true));
            }
        }
    }
    let mut tenders: Vec<Handle> = Vec::new();
    for (handle, deal) in world.deals.iter() {
        if deal.state == crate::state::DealState::Review && player_side(world, deal.buyer) {
            opens.push(("antitrust_review", deal.buyer, None, true));
        }
        // The player's own bids do not ask the player to tender to themselves.
        if deal.state == crate::state::DealState::OfferOpen
            && !deal.investor_tendered
            && !player_side(world, deal.buyer)
            && world.investor_holds(deal.target) > 0
        {
            tenders.push(handle);
        }
    }
    for deal in tenders {
        open_tender(world, deal);
    }
    for handle in &handles {
        let peak = outside_peak_bps(world, *handle);
        if let Some(firm) = world.firms.get_mut(*handle) {
            firm.outside_peak_bps = peak;
        }
    }
    for (card, subject, region, mandatory) in opens {
        open(world, card, Some(subject), region, mandatory);
    }
    let _ = dataset;
}

/// Largest vote share held by a holder outside the player's group.
pub(crate) fn outside_peak_bps(world: &World, firm: Handle) -> i32 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    entity
        .shares
        .holders
        .keys()
        .filter(|holder| match holder {
            crate::state::HolderId::Firm(other) => !player_side(world, *other),
            crate::state::HolderId::Outside(_) => true,
            _ => false,
        })
        .map(|holder| entity.shares.vote_bps(*holder))
        .max()
        .unwrap_or(0)
}

/// Products below 8,000 quality bps with their units sold over the last four weeks.
fn recall_products(world: &World, firm: Handle) -> Vec<(u16, i64)> {
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };
    let recent = entity.sales_history.len().saturating_sub(4);
    entity
        .quality_bps
        .iter()
        .filter(|(_, quality)| **quality < 8_000)
        .map(|(product, _)| {
            let units: i64 = entity.sales_history[recent..]
                .iter()
                .flat_map(|week| week.iter())
                .filter(|((_, sold), _)| sold == product)
                .map(|(_, units)| *units)
                .sum();
            (*product, units)
        })
        .collect()
}

/// Resolve cards addressed to firms the investor does not ultimately control, optional and
/// mandatory alike. The rival uses the same handler catalogue and the same postings; only the
/// choice is automatic. Player cards stay open for an explicit answer. A card whose subject has
/// left the world (merged or dissolved) resolves with its no-action choice.
pub fn auto_resolve(world: &mut World, dataset: &Dataset) {
    let mut pending: Vec<(u64, String, bool)> = Vec::new();
    let candidates: Vec<(u64, String, Option<Handle>)> = world
        .events
        .pending
        .iter()
        .filter(|event| !event.resolved)
        .map(|event| (event.id, event.card.clone(), event.subject_firm))
        .collect();
    for (id, card_id, subject) in candidates {
        let Some(firm) = subject else { continue };
        let Some(card) = dataset.event(&card_id) else {
            continue;
        };
        let gone = world.firm(firm).map(|f| !f.operative()).unwrap_or(true);
        if gone {
            if let Some(choice) = card
                .choices
                .iter()
                .find(|choice| choice.handler == "no_action")
                .or_else(|| card.choices.last())
            {
                pending.push((id, choice.id.clone(), true));
            }
            continue;
        }
        if player_side(world, firm) {
            continue;
        }
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
        deal: None,
    });
    id
}

/// A tender card for the investor about one specific deal, with a cooldown per deal.
pub fn open_tender(world: &mut World, deal: Handle) -> u64 {
    let key = ("tender_offer".to_owned(), deal.0 | (1 << 63));
    if world.events.cooldowns.contains_key(&key) {
        return 0;
    }
    let id = world.events.next_id;
    world.events.next_id += 1;
    world.events.cooldowns.insert(key, world.tick.0);
    world.events.pending.push(PendingEvent {
        id,
        card: "tender_offer".to_owned(),
        subject_firm: None,
        region: None,
        deadline: Tick(world.tick.0 + 4),
        opened: world.tick,
        mandatory: true,
        resolved: false,
        deal: Some(deal),
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
                // Only contracts with the supplier that is actually in distress.
                let contracts: Vec<Handle> = world
                    .contracts
                    .iter()
                    .filter(|(_, contract)| {
                        contract.buyer == buyer
                            && contract.status == crate::state::ContractStatus::Active
                            && world
                                .firms
                                .get(contract.seller)
                                .map(|seller| {
                                    matches!(
                                        seller.status,
                                        crate::state::FirmStatus::PastDue { .. }
                                            | crate::state::FirmStatus::Restructuring { .. }
                                    )
                                })
                                .unwrap_or(false)
                    })
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
                set_outage(world, firm, event.region, 7_500, 2);
            }
        }
        "accept_power_cut" => {
            if let Some(firm) = firm {
                set_outage(world, firm, event.region, 5_000, 2);
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
                    let _ = world.outside.gateway_book.commit(
                        world.tick,
                        Txn::new().debit(Account::Cash, paid, "fuel sale").credit(
                            Account::Revenue,
                            paid,
                            "fuel sale",
                        ),
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
            // Only the subject's own shipments: one week later, and the carrier's reroute quote
            // of a fifth of their freight is paid now.
            if let Some(firm) = firm {
                let mut surcharge = 0i64;
                for shipment in &mut world.shipments {
                    if shipment.buyer == firm || shipment.seller == firm {
                        shipment.arrive = Tick(shipment.arrive.0 + 1);
                        surcharge += shipment.freight_cents * 20 / 100;
                    }
                }
                pay_operating(world, firm, surcharge, "freight reroute");
            }
        }
        // Pausing new lane orders never touches goods already in transit.
        "pause_lane" => {}
        "reserve_carrier" => {
            if let Some(firm) = firm {
                let fee = freight_reserve_fee(world, dataset, firm);
                pay_operating(world, firm, fee, "carrier reservation");
            }
        }
        "spot_delay" => {
            if let Some(firm) = firm {
                for shipment in &mut world.shipments {
                    if shipment.buyer == firm || shipment.seller == firm {
                        shipment.arrive = Tick(shipment.arrive.0 + 1);
                    }
                }
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
                    // A new sourced deposit after a thirteen-week retool, not a free refill.
                    if let Some(stored) = world.facilities.get_mut(facility) {
                        stored.deposit_remaining =
                            cw_core::quantity::MilliLots::from_lots(52 * 20 * 100);
                        stored.status = FacilityStatus::Retooling {
                            until: Tick(world.tick.0 + 13),
                        };
                    }
                }
            }
        }
        "plan_depletion" => {}
        "full_recall" | "voluntary_refund" => {
            // One four-week sales cohort of the products actually below 8,000 quality bps, at
            // the firm's own price: ten percent for a recall, twenty-five for a voluntary refund.
            let rate = if handler == "full_recall" { 10 } else { 25 };
            let reputation = if handler == "full_recall" { 250 } else { 50 };
            if let Some(firm) = firm {
                let cost: i64 = recall_products(world, firm)
                    .into_iter()
                    .map(|(product, units)| {
                        let price = world
                            .firm(firm)
                            .and_then(|entity| entity.policies.prices.get(&product).copied())
                            .unwrap_or_else(|| dataset.reference_price(product));
                        units * price * rate / 100
                    })
                    .sum();
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
            // The firm commits its eighty percent match as a training budget for 26 weeks, and
            // the region's grant window lets labour draw the twenty percent subsidy from the
            // finite government budget as retraining actually happens. No cash is minted.
            if let Some(firm) = firm {
                if let Some(region) = event.region.and_then(|r| world.regions.get_mut(r as usize)) {
                    region.policy.training_grant_until = Some(Tick(world.tick.0 + 26));
                }
                if let Some(entity) = world.firms.get_mut(firm) {
                    let weekly_match = 10_000_000 * 80 / 100 / 26;
                    entity.policies.training_budget_cents =
                        entity.policies.training_budget_cents.max(weekly_match);
                }
            }
        }
        "expedite_permit" | "accept_permit_delay" => {
            // The project that triggered the card: expedite for two percent of its capex to an
            // explicit outside engineering service, or accept a four-week delay.
            if let Some(firm) = firm {
                let project = world.firm(firm).and_then(|entity| {
                    entity.facilities.iter().copied().find(|handle| {
                        world
                            .facilities
                            .get(*handle)
                            .map(|facility| {
                                facility.status == FacilityStatus::Building
                                    && Some(facility.region) == event.region
                            })
                            .unwrap_or(false)
                    })
                });
                if let Some(project) = project {
                    if handler == "expedite_permit" {
                        let capex = world
                            .facilities
                            .get(project)
                            .map(|f| f.capex_cents)
                            .unwrap_or(0);
                        pay_service(world, firm, capex * 2 / 100, "permit expedite");
                    } else if let Some(facility) = world.facilities.get_mut(project) {
                        facility.completes = Tick(facility.completes.0 + 4);
                    }
                }
            }
        }
        "disclose_lobbying" => {
            if let Some(firm) = firm {
                crate::economy::policy::backlash(world, firm, false);
            }
        }
        "cancel_lobbying" => {
            if let Some(firm) = firm {
                crate::economy::policy::cancel_open(world, firm);
                crate::economy::policy::backlash(world, firm, true);
            }
        }
        "submit_remedy" => {
            // Only the recipient's own open deal; an event never acts on another group's bid.
            let deal = world
                .deals
                .iter()
                .find(|(_, deal)| !deal.state.terminal() && Some(deal.buyer) == firm)
                .map(|(handle, _)| handle);
            if let Some(deal) = deal {
                let _ = crate::economy::deals::submit_remedy(world, deal, 500);
            }
        }
        "withdraw_bid" => {
            // Only the recipient's own open deal; an event never acts on another group's bid.
            let deal = world
                .deals
                .iter()
                .find(|(_, deal)| !deal.state.terminal() && Some(deal.buyer) == firm)
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
        "tender_shares" | "refuse_tender" => {
            // The card's own deal records the player's choice; shares move only at settlement,
            // from the buyer's escrow. A bid that has already closed needs no answer.
            if let Some(deal) = event.deal
                && world
                    .deals
                    .get(deal)
                    .map(|stored| stored.state == crate::state::DealState::OfferOpen)
                    .unwrap_or(false)
            {
                crate::economy::deals::respond_tender(world, deal, handler == "tender_shares")?;
            }
        }
        "cure_covenant" => {
            // Repay principal from cash above a four-week mandatory-outflow reserve.
            if let Some(firm) = firm {
                let reserve = crate::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4;
                let mut spare = world
                    .firm(firm)
                    .map(|entity| entity.book.cash() - reserve)
                    .unwrap_or(0);
                let loans: Vec<Handle> = world
                    .firm(firm)
                    .map(|e| e.loans.clone())
                    .unwrap_or_default();
                for loan in loans {
                    if spare <= 0 {
                        break;
                    }
                    let owed = world
                        .loans
                        .get(loan)
                        .map(|l| l.principal_remaining_cents)
                        .unwrap_or(0);
                    let payment = owed.min(spare);
                    crate::economy::credit::repay(world, loan, payment);
                    spare -= payment;
                }
            }
        }
        "renegotiate_covenant" => {
            // Lenders reprice the breach: one percentage point on every outstanding loan.
            if let Some(firm) = firm {
                let loans: Vec<Handle> = world
                    .firm(firm)
                    .map(|e| e.loans.clone())
                    .unwrap_or_default();
                for loan in loans {
                    if let Some(stored) = world.loans.get_mut(loan) {
                        stored.rate_bps += 100;
                    }
                }
            }
        }
        "bank_run" => {
            if let Some(bank) = firm.filter(|handle| {
                world
                    .firm(*handle)
                    .map(|f| f.bank.is_some())
                    .unwrap_or(false)
            }) {
                crate::economy::banking::run(world, bank);
            }
        }
        "raise_liquidity" => {}
        // The loss happened when the card opened; insured claims settle through the insurer.
        "catastrophe_rebuild" => {}
        "catastrophe_mothball" => {
            if let (Some(region), Some(firm)) = (event.region, firm) {
                let facilities: Vec<Handle> = world
                    .firms
                    .get(firm)
                    .map(|entity| entity.facilities.clone())
                    .unwrap_or_default();
                for handle in facilities {
                    if let Some(facility) = world.facilities.get_mut(handle)
                        && facility.region == region
                        && facility.status == FacilityStatus::Operating
                    {
                        crate::economy::operations::mothball(facility, world.tick);
                    }
                }
            }
        }
        "fund_tech_opportunity" => {
            if let Some(firm) = firm {
                let level = world.firms.get(firm).map(|f| f.tech_level).unwrap_or(0);
                let cost = crate::economy::research::threshold(level);
                pay_operating(world, firm, cost, "technology opportunity");
                let mut rng = Rng::entity_stream(world.seed ^ event.id, TAG_RESEARCH, firm);
                if rng.chance_bps(9_000)
                    && let Some(entity) = world.firms.get_mut(firm)
                {
                    entity.tech_level =
                        (entity.tech_level + 1).min(crate::economy::research::MAX_TECH_LEVEL);
                }
            }
        }
        // The macro stream already applied the regime when the card opened; the player's
        // answer is their own policy, not a vote on the economy.
        "accept_recession" | "accept_boom" => {}
        "energy_outage" => {}
        other => {
            let _ = other;
        }
    }
    Ok(())
}

fn pay_operating(world: &mut World, firm: Handle, amount: i64, memo: &str) {
    if amount <= 0 {
        return;
    }
    crate::economy::insolvency::pay_or_defer(world, firm, amount, Account::OperatingExpense, memo);
}

/// Cap the firm's operating capacity in the region for a number of weeks. The player's own
/// production target is untouched and applies again when the outage ends.
fn set_outage(world: &mut World, firm: Handle, region: Option<u8>, cap_bps: i32, weeks: u32) {
    let facilities: Vec<Handle> = world
        .firms
        .get(firm)
        .map(|entity| entity.facilities.clone())
        .unwrap_or_default();
    let until = Tick(world.tick.0 + weeks);
    for handle in facilities {
        if let Some(facility) = world.facilities.get_mut(handle)
            && facility.status == FacilityStatus::Operating
            && region.map(|r| r == facility.region).unwrap_or(true)
        {
            facility.outage = Some((cap_bps, until));
        }
    }
}

/// Pay an explicit outside professional service; it is income to the outside services sector.
fn pay_service(world: &mut World, firm: Handle, amount: i64, memo: &str) {
    let cash = world.firm(firm).map(|f| f.book.cash().max(0)).unwrap_or(0);
    pay_operating(world, firm, amount, memo);
    let paid = amount.min(cash).max(0);
    if paid > 0 {
        let _ = world.outside.legal_services_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, paid, memo)
                .credit(Account::Revenue, paid, memo),
        );
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
        .lane_km("OH-CEN", &dataset.regions[region as usize].code)
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
        let mut regimes = 0;
        for quarter in 1..=40u32 {
            world.tick = Tick(quarter * 13);
            let before = world.macro_state.regime;
            quarter_check(&mut world, &dataset);
            if before.is_none() && world.macro_state.regime.is_some() {
                regimes += 1;
            }
        }
        let macro_cards: Vec<_> = world
            .events
            .pending
            .iter()
            .filter(|event| {
                ["recession", "demand_boom", "fuel_spike"].contains(&event.card.as_str())
            })
            .collect();
        assert!(regimes > 0, "the macro stream produces regimes");
        assert_eq!(macro_cards.len(), regimes, "one notice per regime");
        assert!(
            macro_cards.iter().all(|event| event.region.is_none()),
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
