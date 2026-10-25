//! Validated command catalogue.
//!
//! Every family in `docs/rules/INTERFACE.md` is represented. Commands carry an explicit actor,
//! target and units. One central authority and solvency service validates them; screens do not
//! duplicate permission logic. A rejected command mutates nothing. Duplicate command IDs are
//! idempotent at the session layer; the simulation only sees accepted intents.

use cw_core::calendar::Tick;
use cw_core::error::{
    ArithmeticFault, AuthorityReason, DomainError, DomainResult, EntityKind, SolvencyReason,
};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::economy::{deals, equity, governance, insolvency, ownership, policy, unions};
use crate::state::{
    BankModel, Bond, DealKind, Facility, FacilityStatus, Firm, FirmStatus, Grade, HolderId,
    INITIAL_SHARES, Lender, Loan, Policies, PolicyKind, ShareLot, World,
};

/// `actor` is 0 for the investor and a firm handle for a firm actor.
pub const INVESTOR_ACTOR: u64 = 0;

#[derive(Clone, Debug)]
pub struct CommandEnvelope {
    pub command_id_high: u64,
    pub command_id_low: u64,
    pub actor: u64,
    pub expected_revision: u64,
    pub submitted_tick: u32,
    pub intent: CommandIntent,
}

#[derive(Clone, Debug)]
pub struct CommandReceipt {
    pub sequence: u64,
    pub accepted_tick: u32,
    pub revision: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Preview {
    pub revision: u64,
    pub cash_change_cents: i64,
    pub assumptions: Vec<String>,
    pub warnings: Vec<String>,
    pub blocked: Option<DomainError>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum CommandIntent {
    // Career
    FoundFirm {
        name: String,
        region: u8,
        capital_cents: i64,
        recipe: u16,
    },
    AcceptManagementContract,
    SetPrimaryGroup {
        firm: Handle,
    },
    // Operations
    BuildFacility {
        firm: Handle,
        region: u8,
        recipe: u16,
        blocks: u8,
    },
    ExpandFacility {
        facility: Handle,
        blocks: u8,
    },
    RetoolFacility {
        facility: Handle,
        recipe: u16,
    },
    MothballFacility {
        facility: Handle,
    },
    DisposeFacility {
        facility: Handle,
    },
    // Operating policy
    SetPrice {
        firm: Handle,
        product: u16,
        price_cents: i64,
    },
    SetWageOffer {
        firm: Handle,
        tier: u8,
        wage_cents: i64,
    },
    SetProductionTarget {
        firm: Handle,
        target_bps: i32,
    },
    SetMarketingBudget {
        firm: Handle,
        amount_cents: i64,
    },
    SetResearchBudget {
        firm: Handle,
        amount_cents: i64,
    },
    // Supply
    OfferContract {
        buyer: Handle,
        seller: Handle,
        product: u16,
        from_region: u8,
        to_region: u8,
        quantity_lots: i64,
        price_cents: i64,
        weeks: u32,
        penalty_bps: i32,
        escrow_cents: i64,
    },
    AcceptContract {
        contract: Handle,
    },
    CancelContract {
        contract: Handle,
    },
    PlaceSpotOrder {
        firm: Handle,
        product: u16,
        region: u8,
        quantity_lots: i64,
        max_price_cents: i64,
    },
    // Group
    MakeIntercompanyLoan {
        lender: Handle,
        borrower: Handle,
        principal_cents: i64,
        rate_bps: i32,
        weeks: u32,
    },
    SetTransferContract {
        buyer: Handle,
        seller: Handle,
        product: u16,
        quantity_lots: i64,
        price_cents: i64,
        weeks: u32,
    },
    GrantGuarantee {
        guarantor: Handle,
        beneficiary: Handle,
        claim_cents: i64,
        max_cents: i64,
    },
    // Equity
    PlaceShareOrder {
        investor: bool,
        firm: Handle,
        buy: bool,
        qty: i64,
        limit_price_cents: i64,
    },
    IssueShares {
        firm: Handle,
        qty: i64,
        price_cents: i64,
        buyer_outside: u32,
    },
    RepurchaseShares {
        firm: Handle,
        qty: i64,
    },
    RetireTreasury {
        firm: Handle,
        qty: i64,
    },
    PayDividend {
        firm: Handle,
        amount_cents: i64,
    },
    LaunchIPO {
        firm: Handle,
        new_shares: i64,
        price_cents: i64,
    },
    // Governance
    CallMeeting {
        firm: Handle,
    },
    NominateCEO {
        firm: Handle,
        investor: bool,
    },
    CastVote {
        firm: Handle,
        vote_id: u32,
        approve: bool,
    },
    ProposeBoardAction {
        firm: Handle,
        subject: String,
    },
    // Credit
    RequestLoan {
        firm: Handle,
        amount_cents: i64,
    },
    AcceptLoanQuote {
        firm: Handle,
        amount_cents: i64,
        rate_bps: i32,
        weeks: u32,
    },
    RepayLoan {
        loan: Handle,
        amount_cents: i64,
    },
    IssueBond {
        firm: Handle,
        principal_cents: i64,
        coupon_bps: i32,
        weeks: u32,
    },
    PlaceBondOrder {
        bond: Handle,
        investor: bool,
        qty: i64,
    },
    ProposeRestructuring {
        firm: Handle,
    },
    // Deals
    StartDiligence {
        buyer: Handle,
        target: Handle,
        kind: DealKind,
        price_cents: i64,
        max_shares: i64,
    },
    FundTender {
        deal: Handle,
    },
    RespondTender {
        deal: Handle,
        accept: bool,
    },
    WithdrawOffer {
        deal: Handle,
    },
    ProposeMerger {
        deal: Handle,
    },
    SubmitRemedy {
        deal: Handle,
        divest_share_bps: i32,
    },
    // Banking
    SetDepositRate {
        bank: Handle,
        rate_bps: i32,
    },
    DepositFunds {
        bank: Handle,
        firm: Handle,
        amount_cents: i64,
    },
    WithdrawFunds {
        bank: Handle,
        firm: Handle,
        amount_cents: i64,
    },
    ApproveLoan {
        bank: Handle,
        borrower: Handle,
        amount_cents: i64,
    },
    // Insurance
    QuotePolicy {
        insurer: Handle,
        facility: Handle,
        limit_cents: i64,
        deductible_cents: i64,
    },
    BindPolicy {
        insurer: Handle,
        insured: Handle,
        facility: Handle,
        limit_cents: i64,
        deductible_cents: i64,
    },
    CancelPolicy {
        policy: Handle,
    },
    SetUnderwritingLimit {
        insurer: Handle,
        specialist: bool,
    },
    // Labour and policy
    RespondBargaining {
        firm: Handle,
        choice: BargainingChoice,
        offer_cents: i64,
    },
    ProposeSettlement {
        firm: Handle,
        amount_cents: i64,
    },
    FileLobbyRequest {
        firm: Handle,
        region: u8,
        request: PolicyKind,
    },
    RespondEvent {
        event: u64,
        choice: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BargainingChoice {
    Accept,
    Counter,
    Reject,
}

impl CommandIntent {
    pub const fn key(&self) -> &'static str {
        match self {
            CommandIntent::FoundFirm { .. } => "found_firm",
            CommandIntent::AcceptManagementContract => "accept_management_contract",
            CommandIntent::SetPrimaryGroup { .. } => "set_primary_group",
            CommandIntent::BuildFacility { .. } => "build_facility",
            CommandIntent::ExpandFacility { .. } => "expand_facility",
            CommandIntent::RetoolFacility { .. } => "retool_facility",
            CommandIntent::MothballFacility { .. } => "mothball_facility",
            CommandIntent::DisposeFacility { .. } => "dispose_facility",
            CommandIntent::SetPrice { .. } => "set_price",
            CommandIntent::SetWageOffer { .. } => "set_wage_offer",
            CommandIntent::SetProductionTarget { .. } => "set_production_target",
            CommandIntent::SetMarketingBudget { .. } => "set_marketing_budget",
            CommandIntent::SetResearchBudget { .. } => "set_research_budget",
            CommandIntent::OfferContract { .. } => "offer_contract",
            CommandIntent::AcceptContract { .. } => "accept_contract",
            CommandIntent::CancelContract { .. } => "cancel_contract",
            CommandIntent::PlaceSpotOrder { .. } => "place_spot_order",
            CommandIntent::MakeIntercompanyLoan { .. } => "make_intercompany_loan",
            CommandIntent::SetTransferContract { .. } => "set_transfer_contract",
            CommandIntent::GrantGuarantee { .. } => "grant_guarantee",
            CommandIntent::PlaceShareOrder { .. } => "place_share_order",
            CommandIntent::IssueShares { .. } => "issue_shares",
            CommandIntent::RepurchaseShares { .. } => "repurchase_shares",
            CommandIntent::RetireTreasury { .. } => "retire_treasury",
            CommandIntent::PayDividend { .. } => "pay_dividend",
            CommandIntent::LaunchIPO { .. } => "launch_ipo",
            CommandIntent::CallMeeting { .. } => "call_meeting",
            CommandIntent::NominateCEO { .. } => "nominate_ceo",
            CommandIntent::CastVote { .. } => "cast_vote",
            CommandIntent::ProposeBoardAction { .. } => "propose_board_action",
            CommandIntent::RequestLoan { .. } => "request_loan",
            CommandIntent::AcceptLoanQuote { .. } => "accept_loan_quote",
            CommandIntent::RepayLoan { .. } => "repay_loan",
            CommandIntent::IssueBond { .. } => "issue_bond",
            CommandIntent::PlaceBondOrder { .. } => "place_bond_order",
            CommandIntent::ProposeRestructuring { .. } => "propose_restructuring",
            CommandIntent::StartDiligence { .. } => "start_diligence",
            CommandIntent::FundTender { .. } => "fund_tender",
            CommandIntent::RespondTender { .. } => "respond_tender",
            CommandIntent::WithdrawOffer { .. } => "withdraw_offer",
            CommandIntent::ProposeMerger { .. } => "propose_merger",
            CommandIntent::SubmitRemedy { .. } => "submit_remedy",
            CommandIntent::SetDepositRate { .. } => "set_deposit_rate",
            CommandIntent::DepositFunds { .. } => "deposit_funds",
            CommandIntent::WithdrawFunds { .. } => "withdraw_funds",
            CommandIntent::ApproveLoan { .. } => "approve_loan",
            CommandIntent::QuotePolicy { .. } => "quote_policy",
            CommandIntent::BindPolicy { .. } => "bind_policy",
            CommandIntent::CancelPolicy { .. } => "cancel_policy",
            CommandIntent::SetUnderwritingLimit { .. } => "set_underwriting_limit",
            CommandIntent::RespondBargaining { .. } => "respond_bargaining",
            CommandIntent::ProposeSettlement { .. } => "propose_settlement",
            CommandIntent::FileLobbyRequest { .. } => "file_lobby_request",
            CommandIntent::RespondEvent { .. } => "respond_event",
        }
    }
}

fn require_control(world: &World, actor: u64, firm: Handle) -> DomainResult<()> {
    if actor == INVESTOR_ACTOR {
        if governance::can_operate(world, firm, HolderId::Investor) {
            return Ok(());
        }
        return Err(DomainError::NoAuthority {
            actor,
            reason: AuthorityReason::NotManager,
        });
    }
    if actor == firm.0 {
        return Ok(());
    }
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if entity.shares.controller() == Some(HolderId::Firm(Handle(actor))) {
        return Ok(());
    }
    Err(DomainError::NoAuthority {
        actor,
        reason: AuthorityReason::NotOwner,
    })
}

fn firm_ref(world: &World, handle: Handle) -> DomainResult<&Firm> {
    world.firms.get(handle).ok_or(DomainError::InvalidEntity {
        handle: handle.0,
        kind: EntityKind::Firm,
    })
}

/// Command fields arrive from the wire, so an out-of-range region is a typed rejection rather
/// than an index panic inside the settlement code that trusts stored region ids.
fn require_region(world: &World, region: u8) -> DomainResult<()> {
    if (region as usize) < world.regions.len() {
        Ok(())
    } else {
        Err(DomainError::InvalidEntity {
            handle: region as u64,
            kind: EntityKind::Region,
        })
    }
}

/// `Dataset::reference_price` indexes unchecked, which is correct for trusted ids. A command
/// carries an untrusted product id, so it is validated here before any lookup.
fn reference_price(dataset: &Dataset, product: u16) -> DomainResult<i64> {
    dataset
        .products
        .get(product as usize)
        .map(|entry| entry.reference_price_cents)
        .ok_or(DomainError::InvalidEntity {
            handle: product as u64,
            kind: EntityKind::Product,
        })
}

fn spend_firm_cash(world: &mut World, firm: Handle, amount: i64, memo: &str) -> DomainResult<()> {
    let cash = firm_ref(world, firm)?.book.cash().max(0);
    if cash < amount {
        return Err(DomainError::InsufficientCash {
            entity: firm.0,
            required: amount,
            available: cash,
        });
    }
    let entity = world.firms.get_mut(firm).unwrap();
    let _ = entity.book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, amount, memo)
            .debit(Account::RestrictedCash, amount, memo),
    );
    Ok(())
}

fn release_firm_cash(world: &mut World, firm: Handle, amount: i64, memo: &str) {
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new().debit(Account::Cash, amount, memo).credit(
                Account::RestrictedCash,
                amount,
                memo,
            ),
        );
    }
}

/// Validate and apply one intent. Nothing is mutated when this returns an error.
pub fn apply(
    world: &mut World,
    dataset: &Dataset,
    envelope: &CommandEnvelope,
) -> DomainResult<CommandReceipt> {
    if envelope.expected_revision != world.revision {
        return Err(DomainError::StaleRevision {
            expected: envelope.expected_revision,
            actual: world.revision,
        });
    }
    apply_intent(world, dataset, envelope.actor, &envelope.intent)?;
    world.revision += 1;
    let sequence = world.next_sequence;
    world.next_sequence += 1;
    world.accepted_commands.push(crate::state::AcceptedCommand {
        sequence,
        command_id_high: envelope.command_id_high,
        command_id_low: envelope.command_id_low,
        actor: Handle(envelope.actor),
        submitted: Tick(envelope.submitted_tick),
        accepted: world.tick,
        revision: world.revision,
        intent_key: envelope.intent.key().to_owned(),
    });
    if world.accepted_commands.len() > 512 {
        world.accepted_commands.remove(0);
    }
    Ok(CommandReceipt {
        sequence,
        accepted_tick: world.tick.0,
        revision: world.revision,
    })
}

fn apply_intent(
    world: &mut World,
    dataset: &Dataset,
    actor: u64,
    intent: &CommandIntent,
) -> DomainResult<()> {
    match intent {
        CommandIntent::FoundFirm {
            name,
            region,
            capital_cents,
            recipe,
        } => {
            if actor != INVESTOR_ACTOR {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::NotOwner,
                });
            }
            if *capital_cents < 25_000_000 {
                return Err(DomainError::InsufficientCash {
                    entity: INVESTOR_ACTOR,
                    required: 25_000_000,
                    available: *capital_cents,
                });
            }
            let fee = cw_data::Difficulty::Standard.formation_fee_cents();
            if world.investor.cash_cents < capital_cents + fee {
                return Err(DomainError::InsufficientCash {
                    entity: INVESTOR_ACTOR,
                    required: capital_cents + fee,
                    available: world.investor.cash_cents,
                });
            }
            // A command carries an untrusted recipe id, so it is validated before any index.
            if dataset.recipes.get(*recipe as usize).is_none() {
                return Err(DomainError::InvalidEntity {
                    handle: *recipe as u64,
                    kind: EntityKind::Product,
                });
            }
            require_region(world, *region)?;
            world.investor.cash_cents -= capital_cents + fee;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, capital_cents + fee, "formation")
                    .debit(Account::Investments, *capital_cents, "formation")
                    .debit(Account::OperatingExpense, fee, "formation fee"),
            );
            let _ = world.outside.legal_services_book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, fee, "formation fee")
                    .credit(Account::Revenue, fee, "formation fee"),
            );
            let template = &dataset.recipes[*recipe as usize];
            let mut firm = Firm {
                name: name.clone(),
                industry: industry_index(&template.industry),
                region: *region,
                archetype: None,
                book: Default::default(),
                status: FirmStatus::Operating,
                shares: crate::state::ShareBook::new(Default::default()),
                board: crate::state::Board::new(Tick(world.tick.0 + 13)),
                policies: Policies::default(),
                inventory: Default::default(),
                facilities: Vec::new(),
                loans: Vec::new(),
                bonds: Vec::new(),
                insurance: Vec::new(),
                contracts_buy: Vec::new(),
                contracts_sell: Vec::new(),
                grade: Grade::B,
                past_due_weeks: 0,
                payout_freeze: false,
                organisation: Default::default(),
                union: Default::default(),
                tech_level: 1,
                tech_progress_cents: 0,
                brand_bps: Default::default(),
                reputation_bps: 0,
                policy_requests: Vec::new(),
                guarantees: Vec::new(),
                last_strategic_week: 0,
                merged_into: None,
                founding_week: world.tick,
                bank: None,
                insurer: None,
                valuation: Default::default(),
                reserved_cash_cents: 0,
                last_week_units_sold: Default::default(),
                sales_history: Vec::new(),
                quality_bps: Default::default(),
            };
            firm.policies.wage_offer = [
                dataset.baseline_weekly_wage_cents[0]
                    * world.regions[*region as usize].wage_index_bps as i64
                    / 10_000,
                dataset.baseline_weekly_wage_cents[1]
                    * world.regions[*region as usize].wage_index_bps as i64
                    / 10_000,
                dataset.baseline_weekly_wage_cents[2]
                    * world.regions[*region as usize].wage_index_bps as i64
                    / 10_000,
            ];
            let handle = world.firms.insert(firm);
            let entity = world.firms.get_mut(handle).unwrap();
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, *capital_cents, "paid in capital")
                    .credit(Account::ShareCapital, *capital_cents, "paid in capital"),
            );
            entity.shares.issued = INITIAL_SHARES;
            entity.shares.holders.insert(
                HolderId::Investor,
                ShareLot {
                    qty: INITIAL_SHARES,
                    cost_cents: *capital_cents,
                },
            );
            entity.board.ceo = Some(crate::state::CeoRef::Investor);
            world.investor.primary_group = Some(handle);
            // The founder start shows the same build transaction as `BuildFacility`: forty
            // percent construction service is paid from escrow into work in progress and sixty
            // percent equipment stays in escrow until completion. Releasing the whole capex
            // immediately (the pre-P33 behaviour) left work in progress and restricted cash on
            // the books forever, and drove a founded firm's fair value to zero even when it was
            // solvent.
            let cost = template.capex_cents;
            if *capital_cents >= cost {
                spend_firm_cash(world, handle, cost, "construction escrow")?;
                let service = cost * 40 / 100;
                let equipment = cost - service;
                if let Some(entity) = world.firms.get_mut(handle) {
                    let _ = entity.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::WorkInProgress, service, "construction service")
                            .credit(Account::RestrictedCash, service, "construction service"),
                    );
                }
                pay_construction_service(world, handle, *region, service, "construction service");
                let mut facility = start_facility(
                    world, dataset, handle, *region, *recipe, 1, world.tick, cost,
                );
                facility.escrow_cents = equipment;
                let facility_handle = world.facilities.insert(facility);
                world
                    .firms
                    .get_mut(handle)
                    .unwrap()
                    .facilities
                    .push(facility_handle);
            }
            Ok(())
        }
        CommandIntent::AcceptManagementContract => {
            if actor != INVESTOR_ACTOR {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::NotOwner,
                });
            }
            let destitute = world.investor.cash_cents < 2_500_000
                && world.investor_firms().is_empty()
                && world.investor.managed_firms.is_empty();
            if !destitute {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::OutsideLimit,
                });
            }
            if world.investor.recovery_used > 0
                && world.tick.0 < world.investor.last_recovery_week + 260
            {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::OutsideLimit,
                });
            }
            let target = world
                .firms
                .iter()
                .filter(|(_, firm)| firm.operative() && firm.shares.controller().is_none())
                .map(|(handle, _)| handle)
                .next();
            let Some(target) = target else {
                return Err(DomainError::InvalidEntity {
                    handle: 0,
                    kind: EntityKind::Firm,
                });
            };
            world.investor.cash_cents += 10_000_000;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, 10_000_000, "signing bonus")
                    .credit(Account::OtherIncome, 10_000_000, "signing bonus"),
            );
            if let Some(firm) = world.firms.get_mut(target) {
                firm.board.ceo = Some(crate::state::CeoRef::Investor);
            }
            world.investor.managed_firms.push(target);
            world.investor.primary_group = Some(target);
            world.investor.recovery_used += 1;
            world.investor.last_recovery_week = world.tick.0;
            world.investor.assisted_score = true;
            Ok(())
        }
        CommandIntent::SetPrimaryGroup { firm } => {
            if actor != INVESTOR_ACTOR {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::NotOwner,
                });
            }
            firm_ref(world, *firm)?;
            world.investor.primary_group = Some(*firm);
            Ok(())
        }
        CommandIntent::BuildFacility {
            firm,
            region,
            recipe,
            blocks,
        } => {
            require_control(world, actor, *firm)?;
            if *blocks == 0 || *blocks > 8 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let template =
                dataset
                    .recipes
                    .get(*recipe as usize)
                    .ok_or(DomainError::InvalidEntity {
                        handle: *recipe as u64,
                        kind: EntityKind::Product,
                    })?;
            require_region(world, *region)?;
            let cost = template.capex_cents * *blocks as i64;
            spend_firm_cash(world, *firm, cost, "construction escrow")?;
            // Forty percent is construction service bought as the work is commissioned; sixty
            // percent is equipment held in escrow until completion.
            let service = cost * 40 / 100;
            let equipment = cost - service;
            if let Some(entity) = world.firms.get_mut(*firm) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::WorkInProgress, service, "construction service")
                        .credit(Account::RestrictedCash, service, "construction service"),
                );
            }
            pay_construction_service(world, *firm, *region, service, "construction service");
            let mut facility = start_facility(
                world, dataset, *firm, *region, *recipe, *blocks, world.tick, cost,
            );
            facility.escrow_cents = equipment;
            let handle = world.facilities.insert(facility);
            world.firms.get_mut(*firm).unwrap().facilities.push(handle);
            Ok(())
        }
        CommandIntent::ExpandFacility { facility, blocks } => {
            let (owner, recipe_index, facility_blocks) = world
                .facilities
                .get(*facility)
                .map(|entity| (entity.firm, entity.recipe, entity.blocks))
                .ok_or(DomainError::InvalidEntity {
                    handle: facility.0,
                    kind: EntityKind::Facility,
                })?;
            require_control(world, actor, owner)?;
            let recipe = &dataset.recipes[recipe_index as usize];
            let cost = recipe.capex_cents * *blocks as i64;
            let _ = facility_blocks;
            spend_firm_cash(world, owner, cost, "expansion escrow")?;
            if let Some(entity) = world.firms.get_mut(owner) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::WorkInProgress, cost, "expansion")
                        .credit(Account::RestrictedCash, cost, "expansion"),
                );
            }
            if let Some(stored) = world.facilities.get_mut(*facility) {
                stored.blocks += *blocks;
                stored.capex_cents += cost;
                stored.status = FacilityStatus::Building;
                stored.completes = Tick(world.tick.0 + recipe.build_weeks);
            }
            Ok(())
        }
        CommandIntent::RetoolFacility { facility, recipe } => {
            let (owner, facility_blocks) = world
                .facilities
                .get(*facility)
                .map(|entity| (entity.firm, entity.blocks))
                .ok_or(DomainError::InvalidEntity {
                    handle: facility.0,
                    kind: EntityKind::Facility,
                })?;
            require_control(world, actor, owner)?;
            let template =
                dataset
                    .recipes
                    .get(*recipe as usize)
                    .ok_or(DomainError::InvalidEntity {
                        handle: *recipe as u64,
                        kind: EntityKind::Product,
                    })?;
            let cost = template.capex_cents * facility_blocks as i64 / 2;
            spend_firm_cash(world, owner, cost, "retool escrow")?;
            if let Some(stored) = world.facilities.get_mut(*facility) {
                stored.recipe = *recipe;
                stored.status = FacilityStatus::Retooling {
                    until: Tick(world.tick.0 + template.build_weeks / 2),
                };
                stored.capex_cents += cost;
            }
            Ok(())
        }
        CommandIntent::MothballFacility { facility } => {
            let owner = world
                .facilities
                .get(*facility)
                .map(|entity| entity.firm)
                .ok_or(DomainError::InvalidEntity {
                    handle: facility.0,
                    kind: EntityKind::Facility,
                })?;
            require_control(world, actor, owner)?;
            if let Some(stored) = world.facilities.get_mut(*facility) {
                stored.status = FacilityStatus::Mothballed;
                stored.mothballed_since = Some(world.tick);
                stored.workers = [0; 3];
            }
            Ok(())
        }
        CommandIntent::DisposeFacility { facility } => {
            let (firm, book) = world
                .facilities
                .get(*facility)
                .map(|entity| {
                    (
                        entity.firm,
                        (entity.capex_cents - entity.accumulated_depreciation).max(0),
                    )
                })
                .ok_or(DomainError::InvalidEntity {
                    handle: facility.0,
                    kind: EntityKind::Facility,
                })?;
            require_control(world, actor, firm)?;
            let proceeds = book * 40 / 100;
            if let Some(stored) = world.facilities.get_mut(*facility) {
                stored.status = FacilityStatus::Mothballed;
            }
            world.facilities.remove(*facility);
            if let Some(entity) = world.firms.get_mut(firm) {
                entity.facilities.retain(|handle| *handle != *facility);
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, proceeds, "facility sale")
                        .debit(
                            Account::OperatingExpense,
                            book - proceeds,
                            "facility disposal",
                        )
                        .credit(Account::FixedAssets, book, "facility sale"),
                );
            }
            Ok(())
        }
        CommandIntent::SetPrice {
            firm,
            product,
            price_cents,
        } => {
            require_control(world, actor, *firm)?;
            if *price_cents < 1 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let reference = reference_price(dataset, *product)?;
            if *price_cents < reference / 2 || *price_cents > reference * 2 {
                return Err(DomainError::SolvencyRestriction {
                    entity: firm.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            world
                .firms
                .get_mut(*firm)
                .unwrap()
                .policies
                .prices
                .insert(*product, *price_cents);
            Ok(())
        }
        CommandIntent::SetWageOffer {
            firm,
            tier,
            wage_cents,
        } => {
            require_control(world, actor, *firm)?;
            let index = *tier as usize;
            if index > 2 || *wage_cents <= 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let entity = world.firms.get_mut(*firm).unwrap();
            if *wage_cents < entity.policies.wage_offer[index] {
                // Contracts prevent retroactive wage cuts: the cut applies to new offers only.
                entity.policies.wage_offer[index] = *wage_cents;
                return Ok(());
            }
            entity.policies.wage_offer[index] = *wage_cents;
            Ok(())
        }
        CommandIntent::SetProductionTarget { firm, target_bps } => {
            require_control(world, actor, *firm)?;
            if !(0..=10_000).contains(target_bps) {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let entity = world.firms.get_mut(*firm).unwrap();
            entity.policies.production_target_bps = *target_bps;
            let facilities = entity.facilities.clone();
            for handle in facilities {
                if let Some(facility) = world.facilities.get_mut(handle) {
                    facility.target_bps = *target_bps;
                }
            }
            Ok(())
        }
        CommandIntent::SetMarketingBudget { firm, amount_cents } => {
            require_control(world, actor, *firm)?;
            if *amount_cents < 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            // A budget is per-week policy spent by the weekly marketing step, not a one-off
            // charge at submission.
            world
                .firms
                .get_mut(*firm)
                .unwrap()
                .policies
                .marketing_budget_cents = *amount_cents;
            Ok(())
        }
        CommandIntent::SetResearchBudget { firm, amount_cents } => {
            require_control(world, actor, *firm)?;
            if *amount_cents < 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            world
                .firms
                .get_mut(*firm)
                .unwrap()
                .policies
                .research_budget_cents = *amount_cents;
            Ok(())
        }
        CommandIntent::OfferContract {
            buyer,
            seller,
            product,
            from_region,
            to_region,
            quantity_lots,
            price_cents,
            weeks,
            penalty_bps,
            escrow_cents,
        } => {
            require_control(world, actor, *buyer)?;
            if *quantity_lots <= 0 || *price_cents <= 0 || *weeks == 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let reference = reference_price(dataset, *product)?;
            // The stored contract regions are indexed during freight settlement, so they must
            // be valid here or a later week would panic.
            require_region(world, *from_region)?;
            require_region(world, *to_region)?;
            if *price_cents * 100 < reference * 80 || *price_cents * 100 > reference * 120 {
                return Err(DomainError::SolvencyRestriction {
                    entity: buyer.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            firm_ref(world, *seller)?;
            if *escrow_cents > 0 {
                spend_firm_cash(world, *buyer, *escrow_cents, "contract escrow")?;
            }
            let contract = crate::state::Contract {
                buyer: *buyer,
                seller: *seller,
                product: *product,
                from_region: *from_region,
                to_region: *to_region,
                quantity_weekly: MilliLots::from_lots(*quantity_lots),
                price_cents: *price_cents,
                penalty_bps: *penalty_bps,
                escrow_cents: *escrow_cents,
                remaining_weeks: *weeks,
                next_delivery: Tick(world.tick.0 + 1),
                status: crate::state::ContractStatus::Active,
                shortfalls: MilliLots::ZERO,
                auto_renew: false,
            };
            let handle = world.contracts.insert(contract);
            world
                .firms
                .get_mut(*buyer)
                .unwrap()
                .contracts_buy
                .push(handle);
            world
                .firms
                .get_mut(*seller)
                .unwrap()
                .contracts_sell
                .push(handle);
            Ok(())
        }
        CommandIntent::AcceptContract { contract } => {
            let entity = world
                .contracts
                .get(*contract)
                .ok_or(DomainError::InvalidEntity {
                    handle: contract.0,
                    kind: EntityKind::Contract,
                })?;
            require_control(world, actor, entity.seller)?;
            if let Some(stored) = world.contracts.get_mut(*contract) {
                stored.status = crate::state::ContractStatus::Active;
                stored.next_delivery = Tick(world.tick.0 + 1);
            }
            Ok(())
        }
        CommandIntent::CancelContract { contract } => {
            let entity = world
                .contracts
                .get(*contract)
                .ok_or(DomainError::InvalidEntity {
                    handle: contract.0,
                    kind: EntityKind::Contract,
                })?;
            require_control(world, actor, entity.buyer)?;
            let escrow = entity.escrow_cents;
            let buyer = entity.buyer;
            if let Some(stored) = world.contracts.get_mut(*contract) {
                stored.status = crate::state::ContractStatus::Cancelled;
                stored.escrow_cents = 0;
            }
            if escrow > 0 {
                release_firm_cash(world, buyer, escrow, "escrow refund");
            }
            Ok(())
        }
        CommandIntent::PlaceSpotOrder {
            firm,
            product,
            region,
            quantity_lots,
            max_price_cents,
        } => {
            require_control(world, actor, *firm)?;
            if *quantity_lots <= 0 || *max_price_cents <= 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            require_region(world, *region)?;
            let qty = MilliLots::from_lots(*quantity_lots);
            // Cross-region spot buys dispatch a shipment; local buys settle next week.
            let source = world
                .firms
                .iter()
                .find(|(_, entity)| {
                    entity.operative()
                        && entity.region != *region
                        && entity.stock(*product, entity.region).0 > 0
                })
                .map(|(handle, _)| handle);
            if let Some(seller) = source {
                let seller_region = world.firms.get(seller).map(|f| f.region).unwrap_or(0);
                let available = world
                    .firms
                    .get(seller)
                    .map(|f| f.stock(*product, seller_region))
                    .unwrap_or_default();
                let deliver = qty.min(available);
                if deliver.0 > 0 {
                    let goods_value = deliver
                        .value_at(Cents(*max_price_cents))
                        .unwrap_or_default()
                        .0;
                    let freight = crate::economy::freight::freight_cost(
                        dataset,
                        seller_region,
                        *region,
                        deliver,
                    );
                    let km = dataset
                        .lane_km(
                            &dataset.regions[seller_region as usize].code,
                            &dataset.regions[*region as usize].code,
                        )
                        .unwrap_or(500);
                    // Reserve the goods and the freight bill, so the buyer cannot overdraw.
                    spend_firm_cash(world, *firm, goods_value + freight, "spot reserve")?;
                    let mut carry = 0i64;
                    let carrying = world
                        .firms
                        .get_mut(seller)
                        .map(|f| {
                            f.stock_lot_mut(*product, seller_region)
                                .issue(deliver, &mut carry)
                                .0
                        })
                        .unwrap_or(0);
                    // The seller is paid the goods value; freight is the buyer's own cost.
                    if let Some(entity) = world.firms.get_mut(seller) {
                        let _ = entity.book.commit(
                            world.tick,
                            Txn::new()
                                .debit(Account::Cash, goods_value, "spot sale")
                                .credit(Account::Revenue, goods_value, "spot sale")
                                .credit(Account::Inventory, carrying, "spot cogs")
                                .debit(Account::CostOfGoods, carrying, "spot cogs"),
                        );
                    }
                    // The buyer pays the reserved goods value out of restricted cash.
                    if let Some(entity) = world.firms.get_mut(*firm) {
                        let _ = entity.book.commit(
                            world.tick,
                            Txn::new()
                                .credit(Account::RestrictedCash, goods_value, "spot purchase")
                                .debit(Account::Inventory, goods_value, "spot purchase"),
                        );
                    }
                    if freight > 0 {
                        release_firm_cash(world, *firm, freight, "spot freight release");
                        crate::economy::freight::pay_freight(world, *firm, freight, "spot freight");
                    }
                    world.shipments.push(crate::state::Shipment {
                        seller,
                        buyer: *firm,
                        product: *product,
                        quantity: deliver,
                        from_region: seller_region,
                        to_region: *region,
                        arrive: Tick(world.tick.0 + crate::economy::freight::lead_time_weeks(km)),
                        unit_price_cents: *max_price_cents,
                        goods_value_cents: goods_value,
                        freight_cents: freight,
                    });
                    return Ok(());
                }
            }
            Ok(())
        }
        CommandIntent::MakeIntercompanyLoan {
            lender,
            borrower,
            principal_cents,
            rate_bps,
            weeks,
        } => {
            require_control(world, actor, *lender)?;
            require_control(world, actor, *borrower)?;
            if *principal_cents <= 0 || *rate_bps <= 0 || *weeks == 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            if ownership::would_create_cycle(world, *lender, *borrower) {
                return Err(DomainError::OwnershipCycle {
                    buyer: lender.0,
                    target: borrower.0,
                });
            }
            let cash = firm_ref(world, *lender)?.book.cash();
            if cash < *principal_cents {
                return Err(DomainError::InsufficientCash {
                    entity: lender.0,
                    required: *principal_cents,
                    available: cash,
                });
            }
            let _ = world.firms.get_mut(*lender).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, *principal_cents, "intercompany loan")
                    .debit(Account::Receivables, *principal_cents, "intercompany loan"),
            );
            let _ = world.firms.get_mut(*borrower).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, *principal_cents, "intercompany loan")
                    .credit(Account::Loans, *principal_cents, "intercompany loan"),
            );
            let loan = Loan {
                borrower: *borrower,
                lender: Lender::Firm(*lender),
                principal_remaining_cents: *principal_cents,
                rate_bps: *rate_bps,
                weekly_principal_cents: *principal_cents / *weeks as i64,
                remaining_weeks: *weeks,
                collateral: None,
                secured: false,
                created: world.tick,
                carry: 0,
                defaulted: false,
            };
            let handle = world.loans.insert(loan);
            world.firms.get_mut(*borrower).unwrap().loans.push(handle);
            Ok(())
        }
        CommandIntent::SetTransferContract {
            buyer,
            seller,
            product,
            quantity_lots,
            price_cents,
            weeks,
        } => {
            require_control(world, actor, *buyer)?;
            require_control(world, actor, *seller)?;
            let reference = reference_price(dataset, *product)?;
            if *price_cents * 100 < reference * 80 || *price_cents * 100 > reference * 120 {
                return Err(DomainError::SolvencyRestriction {
                    entity: buyer.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            let contract = crate::state::Contract {
                buyer: *buyer,
                seller: *seller,
                product: *product,
                from_region: world.firms.get(*seller).map(|f| f.region).unwrap_or(0),
                to_region: world.firms.get(*buyer).map(|f| f.region).unwrap_or(0),
                quantity_weekly: MilliLots::from_lots(*quantity_lots),
                price_cents: *price_cents,
                penalty_bps: 500,
                escrow_cents: 0,
                remaining_weeks: *weeks,
                next_delivery: Tick(world.tick.0 + 1),
                status: crate::state::ContractStatus::Active,
                shortfalls: MilliLots::ZERO,
                auto_renew: false,
            };
            let handle = world.contracts.insert(contract);
            world
                .firms
                .get_mut(*buyer)
                .unwrap()
                .contracts_buy
                .push(handle);
            world
                .firms
                .get_mut(*seller)
                .unwrap()
                .contracts_sell
                .push(handle);
            Ok(())
        }
        CommandIntent::GrantGuarantee {
            guarantor,
            beneficiary,
            claim_cents,
            max_cents,
        } => {
            require_control(world, actor, *guarantor)?;
            if *max_cents <= 0 || *claim_cents <= 0 || claim_cents > max_cents {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let capacity = firm_ref(world, *guarantor)?.book.net_tangible_assets() / 2;
            if *max_cents > capacity {
                return Err(DomainError::SolvencyRestriction {
                    entity: guarantor.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            world
                .firms
                .get_mut(*guarantor)
                .unwrap()
                .guarantees
                .push(crate::state::Guarantee {
                    beneficiary: *beneficiary,
                    claim_cents: *claim_cents,
                    max_cents: *max_cents,
                });
            Ok(())
        }
        CommandIntent::PlaceShareOrder {
            investor,
            firm,
            buy,
            qty,
            limit_price_cents,
        } => {
            let holder = if *investor {
                HolderId::Investor
            } else {
                HolderId::Firm(Handle(actor))
            };
            if *investor && actor != INVESTOR_ACTOR {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::NotOwner,
                });
            }
            equity::place_order(
                world,
                holder,
                *firm,
                if *buy {
                    crate::state::OrderSide::Buy
                } else {
                    crate::state::OrderSide::Sell
                },
                *qty,
                *limit_price_cents,
            )?;
            Ok(())
        }
        CommandIntent::IssueShares {
            firm,
            qty,
            price_cents,
            buyer_outside,
        } => {
            require_control(world, actor, *firm)?;
            equity::issue_shares(
                world,
                *firm,
                *qty,
                *price_cents,
                HolderId::Outside(*buyer_outside),
            )?;
            Ok(())
        }
        CommandIntent::RepurchaseShares { firm, qty } => {
            require_control(world, actor, *firm)?;
            equity::repurchase(world, *firm, *qty)?;
            Ok(())
        }
        CommandIntent::RetireTreasury { firm, qty } => {
            require_control(world, actor, *firm)?;
            let treasury = firm_ref(world, *firm)?.shares.treasury();
            if *qty <= 0 || *qty > treasury {
                return Err(DomainError::InsufficientStock {
                    product: 0,
                    required: *qty,
                    available: treasury,
                });
            }
            if let Some(entity) = world.firms.get_mut(*firm) {
                let lot = entity.shares.holders.get_mut(&HolderId::Treasury).unwrap();
                lot.qty -= qty;
                if lot.qty == 0 {
                    entity.shares.holders.remove(&HolderId::Treasury);
                }
                entity.shares.issued -= qty;
            }
            Ok(())
        }
        CommandIntent::PayDividend { firm, amount_cents } => {
            require_control(world, actor, *firm)?;
            equity::pay_dividend(world, *firm, *amount_cents)?;
            Ok(())
        }
        CommandIntent::LaunchIPO {
            firm,
            new_shares,
            price_cents,
        } => {
            require_control(world, actor, *firm)?;
            equity::launch_ipo(world, *firm, *new_shares, *price_cents)?;
            Ok(())
        }
        CommandIntent::CallMeeting { firm } => {
            require_control(world, actor, *firm)?;
            let shares = &firm_ref(world, *firm)?.shares;
            let eligible = shares.eligible_votes();
            if shares.qty(HolderId::Investor) * 2 <= eligible && actor == INVESTOR_ACTOR {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::NotOwner,
                });
            }
            world.firms.get_mut(*firm).unwrap().board.next_meeting = Tick(world.tick.0 + 4);
            Ok(())
        }
        CommandIntent::NominateCEO { firm, investor } => {
            require_control(world, actor, *firm)?;
            let seats = world
                .firms
                .get(*firm)
                .map(|f| f.board.seats_held(HolderId::Investor))
                .unwrap_or(0);
            if *investor && seats < 3 {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::BoardApprovalRequired,
                });
            }
            world.firms.get_mut(*firm).unwrap().board.ceo = Some(if *investor {
                crate::state::CeoRef::Investor
            } else {
                crate::state::CeoRef::Manager(0)
            });
            Ok(())
        }
        CommandIntent::CastVote {
            firm,
            vote_id,
            approve,
        } => {
            require_control(world, actor, *firm)?;
            if let Some(entity) = world.firms.get_mut(*firm)
                && let Some(vote) = entity
                    .board
                    .pending_votes
                    .iter_mut()
                    .find(|vote| vote.id == *vote_id)
            {
                if *approve {
                    vote.yes += 1;
                } else {
                    vote.no += 1;
                }
            }
            Ok(())
        }
        CommandIntent::ProposeBoardAction { firm, subject } => {
            require_control(world, actor, *firm)?;
            let deadline = Tick(world.tick.0 + 4);
            let entity = world.firms.get_mut(*firm).unwrap();
            governance::queue_vote(entity, subject, deadline);
            Ok(())
        }
        CommandIntent::RequestLoan { firm, amount_cents } => {
            require_control(world, actor, *firm)?;
            let capacity = crate::economy::credit::secured_capacity(world, *firm)
                + crate::economy::credit::unsecured_capacity(world, *firm);
            if *amount_cents <= 0 || *amount_cents > capacity {
                return Err(DomainError::SolvencyRestriction {
                    entity: firm.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            let grade = firm_ref(world, *firm)?.grade;
            let rate = world.macro_state.policy_rate_bps + grade.spread_bps();
            let lender_index = (0..world.outside.lenders.len())
                .find(|index| world.outside.lenders[*index].capital_cents >= *amount_cents)
                .ok_or(DomainError::InsufficientCash {
                    entity: 0,
                    required: *amount_cents,
                    available: 0,
                })?;
            world.outside.lenders[lender_index].capital_cents -= *amount_cents;
            let _ = world.firms.get_mut(*firm).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, *amount_cents, "loan")
                    .credit(Account::Loans, *amount_cents, "loan"),
            );
            let loan = Loan {
                borrower: *firm,
                lender: Lender::Outside(lender_index as u32),
                principal_remaining_cents: *amount_cents,
                rate_bps: rate,
                weekly_principal_cents: *amount_cents / 104,
                remaining_weeks: 104,
                collateral: None,
                secured: false,
                created: world.tick,
                carry: 0,
                defaulted: false,
            };
            let handle = world.loans.insert(loan);
            world.firms.get_mut(*firm).unwrap().loans.push(handle);
            Ok(())
        }
        CommandIntent::AcceptLoanQuote {
            firm,
            amount_cents,
            rate_bps,
            weeks,
        } => {
            require_control(world, actor, *firm)?;
            if *amount_cents <= 0 || *rate_bps <= 0 || *weeks == 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            let lender_index = (0..world.outside.lenders.len())
                .find(|index| world.outside.lenders[*index].capital_cents >= *amount_cents)
                .ok_or(DomainError::InsufficientCash {
                    entity: 0,
                    required: *amount_cents,
                    available: 0,
                })?;
            world.outside.lenders[lender_index].capital_cents -= *amount_cents;
            let _ = world.firms.get_mut(*firm).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, *amount_cents, "loan")
                    .credit(Account::Loans, *amount_cents, "loan"),
            );
            let loan = Loan {
                borrower: *firm,
                lender: Lender::Outside(lender_index as u32),
                principal_remaining_cents: *amount_cents,
                rate_bps: *rate_bps,
                weekly_principal_cents: *amount_cents / *weeks as i64,
                remaining_weeks: *weeks,
                collateral: None,
                secured: false,
                created: world.tick,
                carry: 0,
                defaulted: false,
            };
            let handle = world.loans.insert(loan);
            world.firms.get_mut(*firm).unwrap().loans.push(handle);
            Ok(())
        }
        CommandIntent::RepayLoan { loan, amount_cents } => {
            let entity = world.loans.get(*loan).ok_or(DomainError::InvalidEntity {
                handle: loan.0,
                kind: EntityKind::Loan,
            })?;
            require_control(world, actor, entity.borrower)?;
            let payment = (*amount_cents).min(entity.principal_remaining_cents);
            let cash = firm_ref(world, entity.borrower)?.book.cash();
            if cash < payment {
                return Err(DomainError::InsufficientCash {
                    entity: entity.borrower.0,
                    required: payment,
                    available: cash,
                });
            }
            let borrower = entity.borrower;
            let lender = entity.lender;
            let _ = world.firms.get_mut(borrower).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, payment, "repayment")
                    .debit(Account::Loans, payment, "repayment"),
            );
            match lender {
                Lender::Outside(index) => {
                    if let Some(holder) = world.outside.lenders.get_mut(index as usize) {
                        holder.capital_cents += payment;
                    }
                }
                Lender::Firm(handle) => {
                    if let Some(firm) = world.firms.get_mut(handle) {
                        let _ = firm.book.commit(
                            world.tick,
                            Txn::new()
                                .debit(Account::Cash, payment, "repayment received")
                                .credit(Account::Receivables, payment, "repayment received"),
                        );
                    }
                }
                Lender::Bank(handle) => {
                    if let Some(firm) = world.firms.get_mut(handle) {
                        let _ = firm.book.commit(
                            world.tick,
                            Txn::new()
                                .debit(Account::Cash, payment, "repayment received")
                                .credit(Account::Receivables, payment, "repayment received"),
                        );
                    }
                }
            }
            if let Some(stored) = world.loans.get_mut(*loan) {
                stored.principal_remaining_cents -= payment;
            }
            Ok(())
        }
        CommandIntent::IssueBond {
            firm,
            principal_cents,
            coupon_bps,
            weeks,
        } => {
            require_control(world, actor, *firm)?;
            let entity = firm_ref(world, *firm)?;
            if *principal_cents < 50_000_000 || !matches!(entity.grade, Grade::A | Grade::B) {
                return Err(DomainError::SolvencyRestriction {
                    entity: firm.0,
                    reason: SolvencyReason::CreditGradeTooLow,
                });
            }
            let tangible = entity.book.net_tangible_assets().max(1);
            let debt_after = entity.book.debt() + principal_cents;
            if debt_after * 100 > tangible * 60 {
                return Err(DomainError::SolvencyRestriction {
                    entity: firm.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            let buyer = (0..world.outside.investors.len())
                .find(|index| world.outside.investors[*index].capital_cents >= *principal_cents)
                .ok_or(DomainError::InsufficientCash {
                    entity: 0,
                    required: *principal_cents,
                    available: 0,
                })?;
            world.outside.investors[buyer].capital_cents -= *principal_cents;
            let fee = round_even_div(*principal_cents as i128 * 200, 10_000) as i64;
            let _ = world.firms.get_mut(*firm).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, *principal_cents - fee, "bond issue")
                    .debit(Account::OperatingExpense, fee, "bond fee")
                    .credit(Account::Bonds, *principal_cents, "bond issue"),
            );
            let mut holders = std::collections::BTreeMap::new();
            holders.insert(HolderId::Outside(buyer as u32), *principal_cents);
            let bond = Bond {
                issuer: *firm,
                principal_cents: *principal_cents,
                coupon_bps: *coupon_bps,
                maturity: Tick(world.tick.0 + *weeks),
                issued: world.tick,
                holders,
                carry: 0,
                next_coupon: Tick(world.tick.0 + 13),
            };
            let handle = world.bonds.insert(bond);
            world.firms.get_mut(*firm).unwrap().bonds.push(handle);
            Ok(())
        }
        CommandIntent::PlaceBondOrder {
            bond,
            investor,
            qty,
        } => {
            let entity = world.bonds.get(*bond).ok_or(DomainError::InvalidEntity {
                handle: bond.0,
                kind: EntityKind::Bond,
            })?;
            let seller = (0..world.outside.investors.len()).find(|index| {
                entity
                    .holders
                    .contains_key(&HolderId::Outside(*index as u32))
            });
            let Some(seller) = seller else {
                return Err(DomainError::InsufficientStock {
                    product: 0,
                    required: *qty,
                    available: 0,
                });
            };
            let seller_id = HolderId::Outside(seller as u32);
            let available = *entity.holders.get(&seller_id).unwrap_or(&0);
            if *qty <= 0 || *qty > available {
                return Err(DomainError::InsufficientStock {
                    product: 0,
                    required: *qty,
                    available,
                });
            }
            let price = *qty;
            if *investor {
                if actor != INVESTOR_ACTOR || world.investor.cash_cents < price {
                    return Err(DomainError::InsufficientCash {
                        entity: 0,
                        required: price,
                        available: world.investor.cash_cents,
                    });
                }
                world.investor.cash_cents -= price;
                let _ = world.investor.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, price, "bond purchase")
                        .debit(Account::Investments, price, "bond purchase"),
                );
            }
            if let Some(bond) = world.bonds.get_mut(*bond) {
                *bond.holders.get_mut(&seller_id).unwrap() -= qty;
                *bond.holders.entry(HolderId::Investor).or_default() += qty;
            }
            Ok(())
        }
        CommandIntent::ProposeRestructuring { firm } => {
            require_control(world, actor, *firm)?;
            if !matches!(firm_ref(world, *firm)?.status, FirmStatus::PastDue { .. }) {
                return Err(DomainError::WrongPhase {
                    expected: "past_due",
                    actual: "operating",
                });
            }
            let debt = firm_ref(world, *firm)?.book.debt();
            insolvency::restructure_for_owner(world, *firm, debt);
            Ok(())
        }
        CommandIntent::StartDiligence {
            buyer,
            target,
            kind,
            price_cents,
            max_shares,
        } => {
            require_control(world, actor, *buyer)?;
            deals::start(world, *buyer, *target, *kind, *price_cents, *max_shares)?;
            Ok(())
        }
        CommandIntent::FundTender { deal } => {
            let entity = world.deals.get(*deal).ok_or(DomainError::InvalidEntity {
                handle: deal.0,
                kind: EntityKind::Deal,
            })?;
            require_control(world, actor, entity.buyer)?;
            if let Some(stored) = world.deals.get_mut(*deal)
                && stored.state == crate::state::DealState::Draft
            {
                stored.state = crate::state::DealState::Funded;
            }
            Ok(())
        }
        CommandIntent::RespondTender { deal, accept } => {
            if actor != INVESTOR_ACTOR {
                return Err(DomainError::NoAuthority {
                    actor,
                    reason: AuthorityReason::NotOwner,
                });
            }
            let entity = world.deals.get(*deal).ok_or(DomainError::InvalidEntity {
                handle: deal.0,
                kind: EntityKind::Deal,
            })?;
            if !matches!(
                entity.state,
                crate::state::DealState::OfferOpen | crate::state::DealState::Diligence
            ) {
                return Err(DomainError::WrongPhase {
                    expected: "offer_open",
                    actual: entity.state.key(),
                });
            }
            if !accept {
                return Ok(());
            }
            let qty = world.investor_holds(entity.target);
            if qty <= 0 {
                return Err(DomainError::InsufficientStock {
                    product: 0,
                    required: 1,
                    available: 0,
                });
            }
            let price = entity.offer_price_cents;
            let cash = qty.saturating_mul(price);
            world.investor.cash_cents += cash;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new().debit(Account::Cash, cash, "tender").credit(
                    Account::Investments,
                    cash,
                    "tender",
                ),
            );
            let buyer = entity.buyer;
            if let Some(target) = world.firms.get_mut(entity.target) {
                target
                    .shares
                    .transfer(HolderId::Investor, HolderId::Firm(buyer), qty, cash);
            }
            if let Some(bidder) = world.firms.get_mut(buyer) {
                let _ = bidder.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::RestrictedCash, cash, "tender")
                        .debit(Account::Investments, cash, "tender"),
                );
            }
            if world.investor.primary_group == Some(entity.target) {
                world.investor.primary_group = None;
            }
            Ok(())
        }
        CommandIntent::WithdrawOffer { deal } => {
            let entity = world.deals.get(*deal).ok_or(DomainError::InvalidEntity {
                handle: deal.0,
                kind: EntityKind::Deal,
            })?;
            require_control(world, actor, entity.buyer)?;
            deals::withdraw(world, *deal)?;
            Ok(())
        }
        CommandIntent::ProposeMerger { deal } => {
            let entity = world.deals.get(*deal).ok_or(DomainError::InvalidEntity {
                handle: deal.0,
                kind: EntityKind::Deal,
            })?;
            require_control(world, actor, entity.buyer)?;
            if entity.state != crate::state::DealState::Settled {
                return Err(DomainError::WrongPhase {
                    expected: "settled",
                    actual: entity.state.key(),
                });
            }
            if !world
                .firms
                .get(entity.target)
                .map(|firm| firm.shares.meets_squeeze_out(HolderId::Firm(entity.buyer)))
                .unwrap_or(false)
            {
                return Err(DomainError::SolvencyRestriction {
                    entity: deal.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            // Legal merger: absorb the target's ledger and contracts, retire its shares.
            let target = entity.target;
            let book_value = firm_ref(world, target)?.book.total_assets();
            if let Some(bidder) = world.firms.get_mut(entity.buyer) {
                let _ = bidder.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::OtherAssets, book_value, "merger")
                        .credit(Account::Investments, book_value, "merger"),
                );
            }
            if let Some(target) = world.firms.get_mut(target) {
                target.status = FirmStatus::Merged { into: entity.buyer };
                target.merged_into = Some(entity.buyer);
                target.shares.holders.clear();
                target.shares.issued = 0;
            }
            Ok(())
        }
        CommandIntent::SubmitRemedy {
            deal,
            divest_share_bps,
        } => {
            deals::submit_remedy(world, *deal, *divest_share_bps)?;
            Ok(())
        }
        CommandIntent::SetDepositRate { bank, rate_bps } => {
            require_control(world, actor, *bank)?;
            if *rate_bps < 0 || *rate_bps > 1_500 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            if let Some(entity) = world.firms.get_mut(*bank) {
                if entity.bank.is_none() {
                    entity.bank = Some(BankModel {
                        deposit_rate_bps: *rate_bps,
                        specialist_concentration: false,
                    });
                } else if let Some(model) = entity.bank.as_mut() {
                    model.deposit_rate_bps = *rate_bps;
                }
            }
            Ok(())
        }
        CommandIntent::DepositFunds {
            bank,
            firm,
            amount_cents,
        } => {
            require_control(world, actor, *firm)?;
            crate::economy::banking::deposit(world, *bank, HolderId::Firm(*firm), *amount_cents)?;
            Ok(())
        }
        CommandIntent::WithdrawFunds {
            bank,
            firm,
            amount_cents,
        } => {
            require_control(world, actor, *firm)?;
            crate::economy::banking::withdraw(world, *bank, HolderId::Firm(*firm), *amount_cents)?;
            Ok(())
        }
        CommandIntent::ApproveLoan {
            bank,
            borrower,
            amount_cents,
        } => {
            require_control(world, actor, *bank)?;
            crate::economy::banking::approve_loan(world, *bank, *borrower, *amount_cents)?;
            Ok(())
        }
        CommandIntent::QuotePolicy {
            insurer,
            facility,
            limit_cents,
            deductible_cents,
        } => {
            let region = world
                .facilities
                .get(*facility)
                .ok_or(DomainError::InvalidEntity {
                    handle: facility.0,
                    kind: EntityKind::Facility,
                })?
                .region;
            crate::economy::insurance::quote(world, *insurer, region, *limit_cents)?;
            if *deductible_cents < 0 {
                return Err(DomainError::Arithmetic {
                    detail: ArithmeticFault::OutOfRange,
                });
            }
            Ok(())
        }
        CommandIntent::BindPolicy {
            insurer,
            insured,
            facility,
            limit_cents,
            deductible_cents,
        } => {
            require_control(world, actor, *insured)?;
            crate::economy::insurance::bind(
                world,
                *insurer,
                *insured,
                *facility,
                *limit_cents,
                *deductible_cents,
            )?;
            Ok(())
        }
        CommandIntent::CancelPolicy { policy } => {
            let entity = world
                .insurance
                .get(*policy)
                .ok_or(DomainError::InvalidEntity {
                    handle: policy.0,
                    kind: EntityKind::Policy,
                })?;
            require_control(world, actor, entity.insured)?;
            if let Some(stored) = world.insurance.get_mut(*policy) {
                stored.status = crate::state::PolicyStatus::Expired;
            }
            Ok(())
        }
        CommandIntent::SetUnderwritingLimit {
            insurer,
            specialist,
        } => {
            require_control(world, actor, *insurer)?;
            if let Some(entity) = world.firms.get_mut(*insurer)
                && let Some(model) = entity.insurer.as_mut()
            {
                model.specialist_concentration = *specialist;
            }
            Ok(())
        }
        CommandIntent::RespondBargaining {
            firm,
            choice,
            offer_cents,
        } => {
            require_control(world, actor, *firm)?;
            match choice {
                BargainingChoice::Accept => {
                    unions::accept(world, *firm);
                }
                BargainingChoice::Counter => {
                    if !unions::counter(world, *firm, *offer_cents) {
                        world.pending_pause = Some("union_counter_rejected".to_owned());
                    }
                }
                BargainingChoice::Reject => unions::reject(world, *firm),
            }
            Ok(())
        }
        CommandIntent::ProposeSettlement { firm, amount_cents } => {
            require_control(world, actor, *firm)?;
            let current = firm_ref(world, *firm)?.union.demand_cents;
            if *amount_cents * 100 < current * 95 {
                return Err(DomainError::SolvencyRestriction {
                    entity: firm.0,
                    reason: SolvencyReason::CoverageTooLow,
                });
            }
            unions::settle_payment(world, *firm, *amount_cents);
            if let Some(entity) = world.firms.get_mut(*firm) {
                entity.union.state = crate::state::BargainState::Idle;
                entity.union.last_demand_week = world.tick.0;
            }
            Ok(())
        }
        CommandIntent::FileLobbyRequest {
            firm,
            region,
            request,
        } => {
            require_control(world, actor, *firm)?;
            policy::file_request(world, *firm, *region, *request)?;
            Ok(())
        }
        CommandIntent::RespondEvent { event, choice } => {
            crate::economy::events_engine::respond(world, dataset, *event, choice)?;
            Ok(())
        }
    }
}

/// Pay a construction bill to named contractors in the region, or to the finite outside
/// construction sector when none can supply.
fn pay_construction_service(world: &mut World, payer: Handle, region: u8, amount: i64, memo: &str) {
    if amount <= 0 {
        return;
    }
    let contractors: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            *handle != payer && firm.industry == 5 && firm.region == region && firm.operative()
        })
        .map(|(handle, _)| handle)
        .collect();
    if contractors.is_empty() {
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, amount, memo)
                .credit(Account::Revenue, amount, memo),
        );
        return;
    }
    let share = amount / contractors.len() as i64;
    let mut remaining = amount;
    for contractor in contractors {
        let paid = share.min(remaining);
        if paid > 0 {
            if let Some(firm) = world.firms.get_mut(contractor) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new().debit(Account::Cash, paid, memo).credit(
                        Account::Revenue,
                        paid,
                        memo,
                    ),
                );
            }
            remaining -= paid;
        }
    }
}

fn industry_index(industry: &str) -> u8 {
    crate::state::INDUSTRY_NAMES
        .iter()
        .position(|name| *name == industry)
        .unwrap_or(0) as u8
}

#[allow(clippy::too_many_arguments)]
fn start_facility(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    recipe: u16,
    blocks: u8,
    now: Tick,
    cost: i64,
) -> Facility {
    let template = &dataset.recipes[recipe as usize];
    let mut weeks = template.build_weeks;
    if world.regions[region as usize]
        .policy
        .permit_bonus_until
        .map(|until| until.0 > now.0)
        .unwrap_or(false)
    {
        weeks = (weeks * 75 / 100).max(1);
    }
    Facility {
        firm,
        region,
        recipe,
        blocks,
        status: FacilityStatus::Building,
        started: now,
        completes: Tick(now.0 + weeks),
        escrow_cents: cost,
        capex_cents: cost,
        accumulated_depreciation: 0,
        workers: [0; 3],
        target_bps: 10_000,
        quality_bps: 10_000,
        deposit_remaining: if template.output == "ore" {
            MilliLots::from_lots(template.weekly_output_lots * blocks as i64 * 52 * 20)
        } else {
            MilliLots::ZERO
        },
        last_output: MilliLots::ZERO,
        last_inputs_short: false,
        last_staff_ratio_bps: 10_000,
        mothballed_since: None,
    }
}

/// Preview a build without reserving anything.
pub fn preview_build(dataset: &Dataset, recipe: u16, blocks: u8, cash: i64) -> Preview {
    let Ok(template) = dataset.recipes.get(recipe as usize).ok_or(()) else {
        return Preview {
            blocked: Some(DomainError::InvalidEntity {
                handle: recipe as u64,
                kind: EntityKind::Product,
            }),
            ..Default::default()
        };
    };
    let cost = template.capex_cents * blocks as i64;
    Preview {
        revision: 0,
        cash_change_cents: -cost,
        assumptions: vec![
            format!("build {} weeks", template.build_weeks),
            format!("asset basis {} cents", cost),
        ],
        warnings: if cash < cost {
            vec!["shortfall".to_owned()]
        } else {
            Vec::new()
        },
        blocked: if cash < cost {
            Some(DomainError::InsufficientCash {
                entity: 0,
                required: cost,
                available: cash,
            })
        } else {
            None
        },
    }
}

/// Dilution preview used by the issuance sheet.
pub fn preview_issue(world: &World, firm: Handle, qty: i64, price_cents: i64) -> Preview {
    let Some(entity) = world.firms.get(firm) else {
        return Preview {
            blocked: Some(DomainError::InvalidEntity {
                handle: firm.0,
                kind: EntityKind::Firm,
            }),
            ..Default::default()
        };
    };
    let eligible = entity.shares.eligible_votes().max(1);
    let after = eligible + qty;
    let before_bps = entity.shares.qty(HolderId::Investor) * 10_000 / eligible;
    let after_bps = entity.shares.qty(HolderId::Investor) * 10_000 / after.max(1);
    Preview {
        revision: world.revision,
        cash_change_cents: qty.saturating_mul(price_cents),
        assumptions: vec![
            format!("votes before {} bps", before_bps),
            format!("votes after {} bps", after_bps),
            format!("price {} cents per share", price_cents),
        ],
        warnings: if price_cents * 100 < entity.shares.fair_cents * 90 {
            vec!["price below the ninety percent floor".to_owned()]
        } else {
            Vec::new()
        },
        blocked: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    fn envelope(world: &World, intent: CommandIntent) -> CommandEnvelope {
        CommandEnvelope {
            command_id_high: 1,
            command_id_low: 1,
            actor: INVESTOR_ACTOR,
            expected_revision: world.revision,
            submitted_tick: world.tick.0,
            intent,
        }
    }

    #[test]
    fn a_founded_firm_capitalises_its_plant_once() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 142, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let recipe = dataset.recipe_index["farm"];
        let capex = dataset.recipes[recipe as usize].capex_cents;
        let build_weeks = dataset.recipes[recipe as usize].build_weeks;
        let region = world.region_index("035").unwrap();
        let capital = capex + 100_000_000;
        let command = envelope(
            &world,
            CommandIntent::FoundFirm {
                name: "Founder Test".to_owned(),
                region,
                capital_cents: capital,
                recipe,
            },
        );
        apply(&mut world, &dataset, &command).expect("founding accepted");
        let firm = world.investor.primary_group.unwrap();
        for _ in 0..build_weeks {
            crate::tick::advance(&mut world, &dataset);
        }
        let entity = world.firms.get(firm).unwrap();
        assert!(entity.book.is_balanced());
        assert!(entity.book.cash() >= 0);
        assert_eq!(
            entity.book.restricted_cash(),
            0,
            "the equipment escrow is released once, not held forever"
        );
        assert_eq!(
            entity.book.balance(Account::WorkInProgress),
            0,
            "completed work leaves construction in progress"
        );
        assert!(entity.book.balance(Account::FixedAssets) >= capex);
        assert!(
            entity.shares.fair_cents > 0,
            "a solvent founded firm must not be valued at zero"
        );
    }

    #[test]
    fn stale_revision_is_rejected_without_mutation() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 141, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let mut command = envelope(
            &world,
            CommandIntent::SetPrimaryGroup {
                firm: world.investor.primary_group.unwrap(),
            },
        );
        command.expected_revision = world.revision + 5;
        let revision = world.revision;
        let result = apply(&mut world, &dataset, &command);
        assert!(matches!(result, Err(DomainError::StaleRevision { .. })));
        assert_eq!(world.revision, revision);
    }

    #[test]
    fn unauthorised_actor_cannot_change_prices() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 142, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        let other = world
            .firms
            .iter()
            .find(|(handle, _)| *handle != primary)
            .map(|(handle, _)| handle)
            .unwrap();
        let command = envelope(
            &world,
            CommandIntent::SetPrice {
                firm: other,
                product: 0,
                price_cents: 1_000,
            },
        );
        let result = apply(&mut world, &dataset, &command);
        assert!(matches!(result, Err(DomainError::NoAuthority { .. })));
        assert!(world.firm(other).unwrap().policies.prices.is_empty());
    }

    #[test]
    fn a_failed_issuance_leaves_shares_and_cash_unchanged() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 144, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        crate::economy::equity::refresh_valuation(&mut world, &dataset);
        let fair = world.firm(primary).unwrap().shares.fair_cents;
        let issued = world.firm(primary).unwrap().shares.issued;
        let cash = world.firm(primary).unwrap().book.cash();
        // Outside investor 7 has less capital than this issuance; the command must not
        // partially credit shares or cash.
        if let Some(holder) = world.outside.investors.get_mut(7) {
            holder.capital_cents = 1;
        }
        let command = envelope(
            &world,
            CommandIntent::IssueShares {
                firm: primary,
                qty: 1_000_000,
                price_cents: fair * 95 / 100,
                buyer_outside: 7,
            },
        );
        let result = apply(&mut world, &dataset, &command);
        assert!(result.is_err());
        assert_eq!(world.firm(primary).unwrap().shares.issued, issued);
        assert_eq!(world.firm(primary).unwrap().book.cash(), cash);
    }

    #[test]
    fn a_legal_price_change_succeeds() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 143, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        let command = envelope(
            &world,
            CommandIntent::SetPrice {
                firm: primary,
                product: 0,
                price_cents: dataset.reference_price(0),
            },
        );
        let receipt = apply(&mut world, &dataset, &command).unwrap();
        assert!(receipt.sequence >= 1);
        assert_eq!(
            world.firm(primary).unwrap().policies.prices.get(&0),
            Some(&dataset.reference_price(0))
        );
    }
}
