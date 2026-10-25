//! Session orchestration.
//!
//! The session owns one mutable world and serialises all mutation: commands, AI proposals and
//! weekly advances. It exposes immutable read models, a versioned save envelope and a replay log.
//! Swift keeps file I/O and lifecycle; it never computes a financial identity.

pub mod dto;
pub mod save;

use cw_core::error::DomainError;
use cw_core::ids::Handle;
use cw_core::money::round_even_div;
use cw_data::{Dataset, Difficulty};
use cw_sim::commands::{CommandEnvelope, CommandIntent, CommandReceipt, Preview, apply};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};
use serde::{Deserialize, Serialize};

pub use dto::*;

pub const MAX_ADVANCE_WEEKS: u32 = 16;

/// A labelled cash preview for an intent whose settlement price is already explicit.
fn cash_preview(revision: u64, cash_change_cents: i64, assumptions: Vec<String>) -> Preview {
    Preview {
        revision,
        cash_change_cents,
        assumptions,
        warnings: Vec::new(),
        blocked: None,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScenarioConfig {
    pub seed: u64,
    pub scenario: ScenarioKind,
    pub mode: ScenarioMode,
    pub dataset_hash: String,
    pub difficulty: Difficulty,
}

#[derive(Clone, Debug)]
pub struct AdvanceResult {
    pub weeks_advanced: u32,
    pub revision: u64,
    pub pause: Option<String>,
    pub final_week: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayLog {
    pub config: ScenarioConfig,
    pub commands: Vec<RecordedCommand>,
    pub advance_calls: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordedCommand {
    pub actor: u64,
    pub high: u64,
    pub low: u64,
    pub expected_revision: u64,
    pub submitted_tick: u32,
    pub intent_key: String,
    pub intent: String,
}

#[derive(Clone, Debug)]
pub enum Query {
    Overview,
    Firms,
    Firm(u64),
    Facilities(u64),
    Facility(u64),
    Regions(Option<String>),
    Finance(u64),
    Ownership(u64),
    Contracts(u64),
    Loans(u64),
    Insurance(u64),
    Deals,
    Events,
    Market(u8, u16),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Detail {
    /// The caller has no management right or completed diligence for this entity.
    Restricted,
    Overview(Overview),
    Firms(Vec<FirmSummary>),
    Firm(FirmSummary),
    Facilities(Vec<FacilitySummary>),
    Facility(FacilitySummary),
    Regions(Vec<RegionSummary>),
    Finance(FinanceSummary),
    Ownership(OwnershipSummary),
    Contracts(Vec<ContractLine>),
    Loans(Vec<LoanLine>),
    Insurance(Vec<InsuranceLine>),
    Deals(Vec<DealLine>),
    Events(Vec<EventLine>),
    Market(MarketLine),
}

/// Namespace marker for rival proposals. The low half is the persisted command sequence.
pub const AI_COMMAND_HIGH: u64 = 0x00C0_FFEE_0000_0000;

pub struct Session {
    pub dataset: Dataset,
    pub world: World,
    pub career_id: u64,
    pub config: ScenarioConfig,
    log: ReplayLog,
}

impl Session {
    pub fn create(config: ScenarioConfig) -> Result<Session, DomainError> {
        let dataset = Dataset::load().map_err(|_| DomainError::InternalFault { diagnostic: 1 })?;
        if !config.dataset_hash.is_empty() && config.dataset_hash != dataset.hash {
            return Err(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            });
        }
        let world =
            cw_sim::economy::seed::build(&dataset, config.seed, config.scenario, config.mode);
        let career_id = cw_core::rng::scramble(config.seed) & 0x00FF_FFFF_FFFF_FFFF;
        let log = ReplayLog {
            config: config.clone(),
            commands: Vec::new(),
            advance_calls: Vec::new(),
        };
        Ok(Session {
            dataset,
            world,
            career_id,
            config,
            log,
        })
    }

    pub fn dataset_hash(&self) -> &str {
        &self.dataset.hash
    }

    /// Dismiss an advisory pause (score sheet, default grace, strike, board or deal deadline).
    /// Mandatory event pauses are answered through `RespondEvent`, never dismissed here.
    pub fn dismiss_pause(&mut self) {
        let advisory = self
            .world
            .pending_pause
            .as_deref()
            .map(|reason| !reason.starts_with("event:"))
            .unwrap_or(false);
        if advisory {
            self.world.pending_pause = None;
        }
    }

    pub fn revision(&self) -> u64 {
        self.world.revision
    }

    pub fn canonical_hash(&self) -> String {
        cw_sim::state::canonical_hash(&self.world)
    }

    /// Submit one command. A duplicate command ID returns the original receipt.
    pub fn submit(&mut self, envelope: CommandEnvelope) -> Result<CommandReceipt, DomainError> {
        let key = ((envelope.command_id_high as u128) << 64) | envelope.command_id_low as u128;
        if let Some(tick) = self.world.command_nonces.get(&key) {
            if self.world.tick.0 <= tick.0 + 52 {
                return Ok(CommandReceipt {
                    sequence: 0,
                    accepted_tick: tick.0,
                    revision: self.world.revision,
                });
            }
            return Err(DomainError::DuplicateCommand { command: key });
        }
        let intent_text = serde_json::to_string(&envelope.intent).unwrap_or_default();
        let receipt = apply(&mut self.world, &self.dataset, &envelope)?;
        self.world.command_nonces.insert(key, self.world.tick);
        if self.world.command_nonces.len() > 4096 {
            let cutoff = self.world.tick.0.saturating_sub(52);
            self.world.command_nonces.retain(|_, tick| tick.0 >= cutoff);
        }
        self.log.commands.push(RecordedCommand {
            actor: envelope.actor,
            high: envelope.command_id_high,
            low: envelope.command_id_low,
            expected_revision: envelope.expected_revision,
            submitted_tick: envelope.submitted_tick,
            intent_key: envelope.intent.key().to_owned(),
            intent: intent_text,
        });
        Ok(receipt)
    }

    /// Never reserves or mutates. Submit still validates.
    pub fn preview(&self, actor: u64, intent: &CommandIntent) -> Preview {
        let _ = actor;
        let revision = self.world.revision;
        match intent {
            CommandIntent::BuildFacility {
                firm,
                recipe,
                blocks,
                ..
            } => {
                let cash = self
                    .world
                    .firms
                    .get(*firm)
                    .map(|f| f.book.cash())
                    .unwrap_or(0);
                cw_sim::commands::preview_build(&self.dataset, *recipe, *blocks, cash)
            }
            CommandIntent::IssueShares {
                firm,
                qty,
                price_cents,
                ..
            } => cw_sim::commands::preview_issue(&self.world, *firm, *qty, *price_cents),
            CommandIntent::PayDividend {
                firm: _,
                amount_cents,
            } => cash_preview(
                revision,
                -*amount_cents,
                vec!["paid from distributable retained earnings".to_owned()],
            ),
            CommandIntent::SetPrice {
                product,
                price_cents,
                ..
            } => {
                let reference = self
                    .dataset
                    .products
                    .get(*product as usize)
                    .map(|entry| entry.reference_price_cents)
                    .unwrap_or(0);
                cash_preview(
                    revision,
                    0,
                    vec![
                        format!("reference price {reference} cents per lot"),
                        format!("your asking price {price_cents} cents per lot"),
                        "no cash moves until a sale settles".to_owned(),
                    ],
                )
            }
            CommandIntent::SetWageOffer {
                firm,
                tier,
                wage_cents,
            } => {
                let prevailing = self
                    .world
                    .firms
                    .get(*firm)
                    .and_then(|entity| self.world.regions.get(entity.region as usize))
                    .and_then(|region| region.labour.get(*tier as usize))
                    .map(|tier| tier.prevailing_wage_cents)
                    .unwrap_or(0);
                cash_preview(
                    revision,
                    0,
                    vec![
                        format!("prevailing weekly wage {prevailing} cents in this region"),
                        format!("your weekly offer {wage_cents} cents per worker"),
                        "payroll settles with the next advanced week".to_owned(),
                    ],
                )
            }
            CommandIntent::RepurchaseShares { firm, qty } => {
                let fair = self
                    .world
                    .firms
                    .get(*firm)
                    .map(|f| f.shares.fair_cents)
                    .unwrap_or(0);
                let mut assumptions = vec![
                    "estimated at the current fair value per share".to_owned(),
                    "the cleared price is set at settlement".to_owned(),
                    "bought shares move to treasury".to_owned(),
                ];
                if *qty <= 0 {
                    assumptions.push("quantity must be positive".to_owned());
                }
                cash_preview(
                    revision,
                    qty.saturating_mul(fair).saturating_neg(),
                    assumptions,
                )
            }
            CommandIntent::RetireTreasury { qty, .. } => {
                let mut assumptions = vec![
                    "cancellation reduces issued and treasury shares together".to_owned(),
                    "no cash moves".to_owned(),
                ];
                if *qty <= 0 {
                    assumptions.push("quantity must be positive".to_owned());
                }
                cash_preview(revision, 0, assumptions)
            }
            CommandIntent::LaunchIPO {
                new_shares,
                price_cents,
                ..
            } => {
                let proceeds = new_shares.saturating_mul(*price_cents);
                let fee = round_even_div(proceeds as i128 * 200, 10_000) as i64;
                let fee = fee.max(10_000_000).min(proceeds);
                cash_preview(
                    revision,
                    proceeds - fee,
                    vec![
                        format!("gross proceeds {proceeds} cents"),
                        format!("underwriting fee {fee} cents (2 percent, minimum $100,000)"),
                        "at least ten percent of post-issue shares must be sold".to_owned(),
                    ],
                )
            }
            CommandIntent::RequestLoan { amount_cents, .. } => cash_preview(
                revision,
                *amount_cents,
                vec![
                    "funded by an outside lender over 104 weeks".to_owned(),
                    "the rate is the policy rate plus the grade spread".to_owned(),
                ],
            ),
            CommandIntent::IssueBond {
                principal_cents,
                coupon_bps,
                weeks,
                ..
            } => {
                let fee = round_even_div(*principal_cents as i128 * 200, 10_000) as i64;
                cash_preview(
                    revision,
                    principal_cents - fee,
                    vec![
                        format!("bond issue fee {fee} cents (2 percent)"),
                        format!("coupon {coupon_bps} basis points"),
                        format!("matures in {weeks} weeks"),
                    ],
                )
            }
            CommandIntent::DepositFunds { amount_cents, .. } => cash_preview(
                revision,
                -*amount_cents,
                vec!["firm cash leaves; a matching bank deposit claim is created".to_owned()],
            ),
            CommandIntent::WithdrawFunds { amount_cents, .. } => cash_preview(
                revision,
                *amount_cents,
                vec!["paid only from actual bank cash and up to the deposit claim".to_owned()],
            ),
            CommandIntent::ApproveLoan { amount_cents, .. } => cash_preview(
                revision,
                -*amount_cents,
                vec![
                    "bank cash falls by the full principal".to_owned(),
                    "the borrower receives the same principal".to_owned(),
                    "liquidity and capital tests apply".to_owned(),
                ],
            ),
            CommandIntent::StartDiligence {
                price_cents,
                max_shares,
                ..
            } => {
                let escrow = cw_sim::economy::deals::escrow_required(*price_cents, *max_shares);
                let fee =
                    round_even_div(*price_cents as i128 * *max_shares as i128 * 50, 10_000) as i64;
                cash_preview(
                    revision,
                    -(escrow + fee),
                    vec![
                        format!("escrow {escrow} cents includes the 2 percent transaction fee"),
                        format!("diligence fee {fee} cents (0.5 percent of bid value)"),
                        "two weeks of diligence, then a four-week offer".to_owned(),
                    ],
                )
            }
            CommandIntent::RespondTender { deal, accept } => {
                let Some(entity) = self.world.deals.get(*deal) else {
                    return cash_preview(revision, 0, vec!["unknown deal".to_owned()]);
                };
                if !*accept {
                    return cash_preview(
                        revision,
                        0,
                        vec!["declining changes nothing and keeps the shares".to_owned()],
                    );
                }
                let qty = self.world.investor_holds(entity.target);
                cash_preview(
                    revision,
                    qty.saturating_mul(entity.offer_price_cents),
                    vec![
                        format!("your {qty} shares at {} cents", entity.offer_price_cents),
                        "tendered shares transfer to the bidder".to_owned(),
                    ],
                )
            }
            CommandIntent::FileLobbyRequest { firm, .. } => {
                let fee = cw_sim::economy::policy::request_fee(&self.world, *firm);
                cash_preview(
                    revision,
                    -fee,
                    vec![
                        format!("request fee {fee} cents"),
                        "the fee is max($25,000, 0.1 percent of group assets)".to_owned(),
                        "decision in four weeks; success is uncertain".to_owned(),
                    ],
                )
            }
            CommandIntent::ProposeSettlement { amount_cents, .. } => cash_preview(
                revision,
                -*amount_cents,
                vec!["the offer must be at least 95 percent of the current demand".to_owned()],
            ),
            CommandIntent::QuotePolicy { limit_cents, .. } => {
                let premium = cw_sim::economy::insurance::weekly_premium(*limit_cents);
                cash_preview(
                    revision,
                    0,
                    vec![
                        format!("weekly premium {premium} cents"),
                        "annual premium = limit x 200 basis points x 1.25".to_owned(),
                    ],
                )
            }
            CommandIntent::RespondBargaining { choice, .. } => cash_preview(
                revision,
                0,
                vec![match choice {
                    cw_sim::commands::BargainingChoice::Accept => {
                        "accepting adopts the union's wage demand".to_owned()
                    }
                    cw_sim::commands::BargainingChoice::Counter => {
                        "a counter sends your weekly offer; a second unresolved week can strike"
                            .to_owned()
                    }
                    cw_sim::commands::BargainingChoice::Reject => {
                        "rejecting risks a strike and lost output".to_owned()
                    }
                }],
            ),
            CommandIntent::AcceptManagementContract => cash_preview(
                revision,
                10_000_000,
                vec!["one-time signing bonus from the distressed outside investor".to_owned()],
            ),
            CommandIntent::FoundFirm {
                capital_cents,
                recipe,
                ..
            } => {
                let fee = Difficulty::Standard.formation_fee_cents();
                let capex = self
                    .dataset
                    .recipes
                    .get(*recipe as usize)
                    .map(|template| template.capex_cents)
                    .unwrap_or(0);
                cash_preview(
                    revision,
                    -(capital_cents + fee),
                    vec![
                        format!("paid-in capital {capital_cents} cents becomes the firm's cash"),
                        format!("formation fee {fee} cents to outside legal services"),
                        format!(
                            "template capex {capex} cents is escrowed from the paid-in capital"
                        ),
                        "the investor needs at least the minimum capital plus the fee".to_owned(),
                    ],
                )
            }
            CommandIntent::PlaceShareOrder {
                investor,
                firm,
                buy,
                qty,
                limit_price_cents,
            } => {
                let name = self
                    .world
                    .firms
                    .get(*firm)
                    .map(|entity| entity.name.clone())
                    .unwrap_or_default();
                let reserved = if *buy {
                    qty.saturating_mul(*limit_price_cents).saturating_neg()
                } else {
                    qty.saturating_mul(*limit_price_cents)
                };
                cash_preview(
                    revision,
                    reserved,
                    vec![
                        format!("{qty} shares of {name} at a {limit_price_cents} cent limit"),
                        format!(
                            "reserved from {}",
                            if *investor {
                                "your personal account"
                            } else {
                                "the firm's account"
                            }
                        ),
                        "price impact is 10 percent of quantity over weekly float, capped at 25 percent"
                            .to_owned(),
                        "the quote expires at the next boundary and revalidates cash and ownership"
                            .to_owned(),
                    ],
                )
            }
            _ => Preview {
                revision,
                ..Default::default()
            },
        }
    }

    /// Advance the clock, applying rival proposals through the common validator first.
    pub fn advance(&mut self, max_weeks: u32) -> AdvanceResult {
        let weeks = max_weeks.min(MAX_ADVANCE_WEEKS);
        self.log.advance_calls.push(weeks);
        let mut advanced = 0;
        let mut pause = None;
        for _ in 0..weeks {
            if let Some(reason) = self.world.pending_pause.clone() {
                pause = Some(reason);
                break;
            }
            self.run_ai();
            let outcome = cw_sim::tick::advance(&mut self.world, &self.dataset);
            advanced += 1;
            if let Some(reason) = outcome.pause {
                pause = Some(reason);
                break;
            }
        }
        AdvanceResult {
            weeks_advanced: advanced,
            revision: self.world.revision,
            pause,
            final_week: self.world.tick.0,
        }
    }

    fn run_ai(&mut self) {
        let proposals = cw_ai::propose(&self.world, &self.dataset);
        for proposal in proposals {
            // The command identity derives from the persisted sequence counter, never a
            // session-local counter: a resumed save must issue the same ids, and the same
            // revision, as an uninterrupted run. A rejected proposal consumes no sequence,
            // and it is never recorded, so reusing that id is harmless.
            let envelope = CommandEnvelope {
                command_id_high: AI_COMMAND_HIGH,
                command_id_low: self.world.next_sequence,
                actor: proposal.actor,
                expected_revision: self.world.revision,
                submitted_tick: self.world.tick.0,
                intent: proposal.intent,
            };
            let _ = apply(&mut self.world, &self.dataset, &envelope);
        }
    }

    pub fn overview(&self) -> Overview {
        let world = &self.world;
        let net_worth = cw_sim::economy::score::personal_net_worth(world);
        let primary = world
            .investor
            .primary_group
            .filter(|handle| world.firms.contains(*handle));
        let (name, cash, forecast, ebit) = match primary {
            Some(handle) => {
                let firm = world.firms.get(handle).unwrap();
                let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, handle);
                let revenue: i64 = firm
                    .book
                    .recent_weeks()
                    .iter()
                    .map(|week| week.revenue)
                    .sum::<i64>()
                    / firm.book.recent_weeks().len().max(1) as i64;
                let ebit = firm
                    .book
                    .recent_weeks()
                    .last()
                    .map(|week| week.ebit())
                    .unwrap_or(0);
                (
                    firm.name.clone(),
                    firm.book.cash(),
                    firm.book.cash() - outflow * 4 + revenue,
                    ebit,
                )
            }
            None => (String::new(), 0, 0, 0),
        };
        let mut alerts = Vec::new();
        for (handle, firm) in world.firms.iter() {
            if firm.shares.qty(cw_sim::state::HolderId::Investor) == 0 {
                continue;
            }
            if matches!(
                firm.status,
                cw_sim::state::FirmStatus::PastDue { .. }
                    | cw_sim::state::FirmStatus::Restructuring { .. }
            ) {
                alerts.push(Alert {
                    key: "alert.past_due".to_owned(),
                    subject: firm.name.clone(),
                    severity: 3,
                });
            }
            if firm.payout_freeze {
                alerts.push(Alert {
                    key: "alert.payout_frozen".to_owned(),
                    subject: firm.name.clone(),
                    severity: 2,
                });
            }
            let _ = handle;
        }
        for event in dto::events(world, &self.dataset) {
            if event.mandatory {
                alerts.push(Alert {
                    key: "alert.decision".to_owned(),
                    subject: event.title,
                    severity: 2,
                });
            }
        }
        let advice = self.next_action();
        Overview {
            tick: world.tick.0,
            week_label: world.tick.label(),
            revision: world.revision,
            investor_cash_cents: world.investor.cash_cents,
            investor_net_worth_cents: net_worth,
            primary_firm: primary.map(|handle| handle.0),
            primary_name: name,
            firm_cash_cents: cash,
            forecast_four_week_cents: forecast,
            last_week_ebit_cents: ebit,
            alerts,
            pause: world.pending_pause.clone(),
            next_action_key: advice.0,
            next_action_reason: advice.1,
        }
    }

    /// One recommended next action with a reason. Deterministic, based only on visible state:
    /// solvency before growth, staffing before price, stock before expansion.
    fn next_action(&self) -> (String, String) {
        let world = &self.world;
        let controlled: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(_, firm)| firm.shares.qty(cw_sim::state::HolderId::Investor) > 0)
            .map(|(handle, _)| handle)
            .collect();
        if controlled.is_empty() {
            return (
                "advice.find_a_firm".to_owned(),
                "You do not control a company. Found one with personal capital, or take the management contract when eligible.".to_owned(),
            );
        }
        for handle in &controlled {
            let firm = world.firm(*handle).unwrap();
            if matches!(
                firm.status,
                cw_sim::state::FirmStatus::PastDue { .. }
                    | cw_sim::state::FirmStatus::Restructuring { .. }
            ) {
                return (
                    "advice.restore_solvency".to_owned(),
                    format!(
                        "{} cannot meet a due payment. Sell unpledged stock, raise funded equity or negotiate before the four-week grace ends.",
                        firm.name
                    ),
                );
            }
        }
        let primary = world
            .investor
            .primary_group
            .filter(|handle| world.firms.contains(*handle));
        let Some(primary) = primary else {
            return (
                "advice.choose_company".to_owned(),
                "Choose the company you want to operate from the portfolio.".to_owned(),
            );
        };
        let firm = world.firm(primary).unwrap();
        let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, primary);
        if outflow > 0 && firm.book.cash() < outflow * 8 {
            return (
                "advice.build_cash_buffer".to_owned(),
                format!(
                    "{} holds less than eight weeks of mandatory payroll and debt service. Raise a price that still clears, cut marketing, or seek legal credit.",
                    firm.name
                ),
            );
        }
        let understaffed = firm
            .facilities
            .iter()
            .filter_map(|handle| world.facilities.get(*handle))
            .find(|facility| {
                facility.status == cw_sim::state::FacilityStatus::Operating
                    && facility.last_staff_ratio_bps < 8_500
            });
        if let Some(facility) = understaffed {
            return (
                "advice.raise_wage".to_owned(),
                format!(
                    "A facility is staffed at {} percent, so capacity is idle. Raise the wage offer toward the regional prevailing wage.",
                    facility.last_staff_ratio_bps / 100
                ),
            );
        }
        for ((region, product), sold) in &firm.last_week_units_sold {
            let sold = sold.lots();
            let stock = firm.stock(*product, *region).lots();
            if sold > 0 && stock > sold.saturating_mul(4) {
                return (
                    "advice.clear_stock".to_owned(),
                    format!(
                        "You hold {} lots of {} against {} sold last week. Lower the price within the allowed band or sell surplus through a contract.",
                        stock, self.dataset.products[*product as usize].name, sold
                    ),
                );
            }
        }
        (
            "advice.operate".to_owned(),
            "Step one week and compare the result with the forecast. Then inspect the facility bottleneck before expanding."
                .to_owned(),
        )
    }

    /// Private firm data needs a management right, public disclosure, or completed diligence.
    pub fn can_inspect(&self, handle: u64) -> bool {
        let firm_handle = Handle(handle);
        let Some(firm) = self.world.firm(firm_handle) else {
            return false;
        };
        if firm.shares.qty(cw_sim::state::HolderId::Investor) > 0
            || self.world.investor.managed_firms.contains(&firm_handle)
        {
            return true;
        }
        if firm.shares.ipo_done {
            return true;
        }
        // A deal the investor's group funded and has already taken through diligence.
        self.world.deals.iter().any(|(_, deal)| {
            deal.target == firm_handle
                && self.world.tick.0 >= deal.diligence_until.0
                && self
                    .world
                    .firm(deal.buyer)
                    .map(|buyer| buyer.shares.qty(cw_sim::state::HolderId::Investor) > 0)
                    .unwrap_or(false)
        })
    }

    pub fn query(&self, query: Query) -> Detail {
        let world = &self.world;
        match query {
            Query::Overview => Detail::Overview(self.overview()),
            Query::Firms => Detail::Firms(
                world
                    .firms
                    .handles_sorted()
                    .into_iter()
                    .filter_map(|handle| {
                        dto::firm_summary(
                            world,
                            &self.dataset,
                            handle.0,
                            self.can_inspect(handle.0),
                        )
                    })
                    .collect(),
            ),
            Query::Firm(handle) => {
                let summary =
                    dto::firm_summary(world, &self.dataset, handle, self.can_inspect(handle))
                        .unwrap_or_else(|| empty_firm(handle));
                Detail::Firm(summary)
            }
            Query::Facilities(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Facilities(
                    world
                        .firms
                        .get(cw_core::ids::Handle(firm))
                        .map(|entity| {
                            entity
                                .facilities
                                .iter()
                                .filter_map(|handle| {
                                    dto::facility_summary(world, &self.dataset, handle.0)
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                )
            }
            Query::Facility(handle) => {
                let owner = world
                    .facilities
                    .get(cw_core::ids::Handle(handle))
                    .map(|facility| facility.firm.0)
                    .unwrap_or(0);
                if !self.can_inspect(owner) {
                    return Detail::Restricted;
                }
                Detail::Facility(
                    dto::facility_summary(world, &self.dataset, handle)
                        .unwrap_or_else(|| empty_facility(handle)),
                )
            }
            Query::Regions(selected) => {
                let from = selected
                    .and_then(|code| world.region_index(&code))
                    .map(|index| index as usize);
                Detail::Regions(
                    (0..world.regions.len())
                        .map(|index| dto::region_summary(world, &self.dataset, index, from))
                        .collect(),
                )
            }
            Query::Finance(handle) => {
                if !self.can_inspect(handle) {
                    return Detail::Restricted;
                }
                Detail::Finance(
                    dto::finance_summary(world, handle).unwrap_or_else(|| empty_finance(handle)),
                )
            }
            Query::Ownership(handle) => {
                if !self.can_inspect(handle) {
                    return Detail::Restricted;
                }
                Detail::Ownership(
                    dto::ownership_summary(world, handle)
                        .unwrap_or_else(|| empty_ownership(handle)),
                )
            }
            Query::Contracts(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Contracts(dto::contracts(world, &self.dataset, firm))
            }
            Query::Loans(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Loans(dto::loans(world, firm))
            }
            Query::Insurance(firm) => {
                if !self.can_inspect(firm) {
                    return Detail::Restricted;
                }
                Detail::Insurance(dto::insurance(world, firm))
            }
            Query::Deals => Detail::Deals(dto::deals(world)),
            Query::Events => Detail::Events(dto::events(world, &self.dataset)),
            Query::Market(region, product) => {
                Detail::Market(dto::market(world, &self.dataset, region, product))
            }
        }
    }

    pub fn export_save(&self) -> Result<Vec<u8>, DomainError> {
        save::encode(&self.world, self.career_id)
    }

    /// Import into a new session. The current session is untouched on failure.
    pub fn import_save(bytes: &[u8]) -> Result<Session, DomainError> {
        if bytes.len() > save::MAX_BYTES {
            return Err(DomainError::ResourceLimit {
                limit: save::MAX_BYTES as u64,
                requested: bytes.len() as u64,
            });
        }
        let (world, career_id) = save::decode(bytes)?;
        let dataset = Dataset::load().map_err(|_| DomainError::InternalFault { diagnostic: 2 })?;
        if world.dataset_hash != dataset.hash {
            return Err(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            });
        }
        let config = ScenarioConfig {
            seed: world.seed,
            scenario: world.scenario,
            mode: world.mode,
            dataset_hash: world.dataset_hash.clone(),
            difficulty: world.difficulty,
        };
        let log = ReplayLog {
            config: config.clone(),
            commands: Vec::new(),
            advance_calls: Vec::new(),
        };
        Ok(Session {
            dataset,
            world,
            career_id,
            config,
            log,
        })
    }

    pub fn export_replay(&self) -> String {
        serde_json::to_string(&self.log).unwrap_or_default()
    }

    /// Replay a recorded log from a fresh scenario and return the canonical hash.
    pub fn replay(log: &ReplayLog) -> Result<String, DomainError> {
        let mut session = Session::create(log.config.clone())?;
        for command in &log.commands {
            let intent = decode_intent(&command.intent).ok_or(DomainError::InvalidSave {
                detail: cw_core::error::SaveFault::MalformedPayload,
            })?;
            let envelope = CommandEnvelope {
                command_id_high: command.high,
                command_id_low: command.low,
                actor: command.actor,
                expected_revision: command.expected_revision,
                submitted_tick: command.submitted_tick,
                intent,
            };
            session.submit(envelope)?;
        }
        for weeks in &log.advance_calls {
            session.advance(*weeks);
        }
        Ok(session.canonical_hash())
    }
}

fn decode_intent(text: &str) -> Option<CommandIntent> {
    serde_json::from_str(text).ok()
}

fn empty_firm(handle: u64) -> FirmSummary {
    FirmSummary {
        handle,
        name: String::new(),
        industry: String::new(),
        region: String::new(),
        cash_cents: 0,
        equity_cents: 0,
        fair_value_cents: 0,
        mark_cents: 0,
        grade: "D".to_owned(),
        status: "unknown".to_owned(),
        investor_votes_bps: 0,
        controlled: false,
        ceo_is_investor: false,
        facilities: 0,
        restricted: true,
    }
}

fn empty_facility(handle: u64) -> FacilitySummary {
    FacilitySummary {
        handle,
        firm: 0,
        recipe: String::new(),
        blocks: 0,
        region: String::new(),
        status: "unknown".to_owned(),
        capacity: 0,
        last_output: 0,
        workers: [0; 3],
        bottleneck: String::new(),
        deposit_remaining: 0,
        target_bps: 0,
    }
}

fn empty_finance(handle: u64) -> FinanceSummary {
    FinanceSummary {
        firm: handle,
        cash_cents: 0,
        restricted_cents: 0,
        inventory_cents: 0,
        assets_cents: 0,
        liabilities_cents: 0,
        equity_cents: 0,
        revenue_trailing_cents: 0,
        ebitda_trailing_cents: 0,
        net_income_trailing_cents: 0,
        cash_bridge: CashBridge::default(),
        recent_weeks: Vec::new(),
    }
}

fn empty_ownership(handle: u64) -> OwnershipSummary {
    OwnershipSummary {
        firm: handle,
        issued: 0,
        treasury: 0,
        eligible_votes: 0,
        controller: None,
        holders: Vec::new(),
        board_seats: std::array::from_fn(|_| None),
        next_meeting: 0,
        ceo: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(seed: u64) -> ScenarioConfig {
        ScenarioConfig {
            seed,
            scenario: ScenarioKind::Guided,
            mode: ScenarioMode::FiveYear,
            dataset_hash: String::new(),
            difficulty: Difficulty::Standard,
        }
    }

    #[test]
    fn a_new_session_has_a_real_overview() {
        let session = Session::create(config(171)).unwrap();
        let overview = session.overview();
        assert_eq!(overview.tick, 0);
        assert!(overview.investor_cash_cents > 0);
        assert!(!overview.primary_name.is_empty());
    }

    #[test]
    fn advancing_produces_a_new_week_and_a_receipt() {
        let mut session = Session::create(config(172)).unwrap();
        let result = session.advance(4);
        assert_eq!(result.weeks_advanced, 4);
        assert_eq!(session.world.tick.0, 4);
    }

    #[test]
    fn a_save_round_trip_keeps_the_hash() {
        let mut session = Session::create(config(173)).unwrap();
        session.advance(3);
        let bytes = session.export_save().unwrap();
        let restored = Session::import_save(&bytes).unwrap();
        assert_eq!(session.canonical_hash(), restored.canonical_hash());
        assert_eq!(restored.world.tick.0, 3);
    }

    #[test]
    fn a_corrupt_save_does_not_destroy_the_session() {
        let mut session = Session::create(config(174)).unwrap();
        session.advance(2);
        let mut bytes = session.export_save().unwrap();
        let index = bytes.len() - 3;
        bytes[index] ^= 0xFF;
        assert!(Session::import_save(&bytes).is_err());
        assert_eq!(session.world.tick.0, 2);
    }

    #[test]
    fn the_next_action_prioritises_solvency_then_cash_then_operation() {
        let mut session = Session::create(config(211)).unwrap();
        let initial = session.overview();
        assert!(!initial.next_action_key.is_empty());
        assert!(!initial.next_action_reason.is_empty());
        let firm = session.world.investor.primary_group.unwrap();
        // Low cash produces the cash-buffer advice.
        session
            .world
            .firms
            .get_mut(firm)
            .unwrap()
            .book
            .commit(
                session.world.tick,
                cw_core::ledger::Txn::new()
                    .credit(cw_core::ledger::Account::Cash, 90_000_000, "drain")
                    .debit(
                        cw_core::ledger::Account::OperatingExpense,
                        90_000_000,
                        "drain",
                    ),
            )
            .unwrap();
        let low = session.overview();
        assert_eq!(low.next_action_key, "advice.build_cash_buffer");
        // A past-due firm outranks the cash advice.
        cw_sim::economy::insolvency::defer_expense(
            &mut session.world,
            firm,
            1_000_000,
            cw_core::ledger::Account::Wages,
            "unpaid",
        );
        let due = session.overview();
        assert_eq!(due.next_action_key, "advice.restore_solvency");
        assert!(due.next_action_reason.contains("grace"));
    }

    #[test]
    fn private_firm_data_needs_management_public_disclosure_or_diligence() {
        let session = Session::create(config(221)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        assert!(
            session.can_inspect(primary.0),
            "management sees its own firm"
        );
        assert!(matches!(
            session.query(Query::Finance(primary.0)),
            Detail::Finance(_)
        ));
        let private = session
            .world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != primary && !firm.shares.ipo_done)
            .map(|(handle, _)| handle)
            .expect("a private rival");
        assert!(!session.can_inspect(private.0));
        assert!(matches!(
            session.query(Query::Finance(private.0)),
            Detail::Restricted
        ));
        assert!(matches!(
            session.query(Query::Ownership(private.0)),
            Detail::Restricted
        ));
        assert!(matches!(
            session.query(Query::Facilities(private.0)),
            Detail::Restricted
        ));
        if let Detail::Firm(summary) = session.query(Query::Firm(private.0)) {
            assert!(summary.restricted);
            assert_eq!(summary.cash_cents, 0, "private cash is not disclosed");
        } else {
            panic!("firm summary");
        }
        let public = session
            .world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != primary && firm.shares.ipo_done)
            .map(|(handle, _)| handle);
        if let Some(public) = public {
            assert!(
                session.can_inspect(public.0),
                "public statements are disclosed"
            );
        }
    }

    #[test]
    fn completed_diligence_grants_access_to_a_private_target() {
        let mut session = Session::create(config(222)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let target = session
            .world
            .firms
            .iter()
            .find(|(handle, firm)| *handle != primary && !firm.shares.ipo_done)
            .map(|(handle, _)| handle)
            .unwrap();
        assert!(!session.can_inspect(target.0));
        session.world.deals.insert(cw_sim::state::Deal {
            buyer: primary,
            target,
            kind: cw_sim::state::DealKind::SharePurchase,
            state: cw_sim::state::DealState::Diligence,
            escrow_cents: 0,
            offer_price_cents: 1,
            max_shares: 1,
            committed_shares: 0,
            started: cw_core::calendar::Tick(0),
            diligence_until: cw_core::calendar::Tick(0),
            offer_deadline: cw_core::calendar::Tick(4),
            review_deadline: None,
            frozen_fair_cents: 1,
            review_required: false,
            remedy_share_bps: 0,
            hostile: false,
            diligence_paid: true,
        });
        assert!(
            session.can_inspect(target.0),
            "diligence reveals the statements"
        );
    }

    #[test]
    fn the_cross_platform_fixture_hash_is_pinned() {
        // This exact sequence runs in the iOS XCTest bridge. Equal hashes on Linux and iOS are
        // the cross-platform determinism evidence for this scenario.
        //
        // 2026-10-25: the value moved from 8d66f18... when rival command ids stopped using a
        // session-local counter and started using the persisted `world.next_sequence`. It moved
        // again to 3ef82f5... when `production` stopped subtracting weekly output from every
        // facility's `deposit_remaining`, which is only stocked for ore mines; ordinary plants
        // had been drifting to large negative deposits. The economy is unchanged otherwise; the
        // Swift constant must be updated in the same change and re-verified on the Mac runner
        // before R05 claims cross-platform equality again.
        let mut session = Session::create(config(42)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let price = session.dataset.reference_price(0);
        session
            .submit(cw_sim::commands::CommandEnvelope {
                command_id_high: 0x5357_4946_5400_0000,
                command_id_low: 0,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: 0,
                intent: CommandIntent::SetPrice {
                    firm: primary,
                    product: 0,
                    price_cents: price,
                },
            })
            .unwrap();
        session.advance(2);
        let hash = session.canonical_hash();
        assert_eq!(
            hash,
            "2ee957c55392262ac0ee3e276c4f449f628fc95c3ed9b0692d25390391ace286"
        );
    }

    #[test]
    fn batching_does_not_change_the_result() {
        let mut single = Session::create(config(176)).unwrap();
        let mut batched = Session::create(config(176)).unwrap();
        single.advance(1);
        single.advance(1);
        batched.advance(2);
        assert_eq!(single.canonical_hash(), batched.canonical_hash());
    }

    #[test]
    fn a_recorded_log_replays_to_the_same_hash() {
        let mut session = Session::create(config(177)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let price = session.dataset.reference_price(0);
        session
            .submit(cw_sim::commands::CommandEnvelope {
                command_id_high: 177,
                command_id_low: 1,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: 0,
                intent: CommandIntent::SetPrice {
                    firm: primary,
                    product: 0,
                    price_cents: price,
                },
            })
            .unwrap();
        session.advance(3);
        let log_text = session.export_replay();
        let log: ReplayLog = serde_json::from_str(&log_text).unwrap();
        let replayed = Session::replay(&log).unwrap();
        assert_eq!(replayed, session.canonical_hash());
    }

    #[test]
    fn the_five_year_score_pauses_and_the_career_continues() {
        let mut session = Session::create(config(201)).unwrap();
        // The five-year mode pauses exactly at week 260 and records a score. Advance is capped
        // at sixteen weeks per call, so a caller loops until the pause.
        let mut result = session.advance(16);
        while result.final_week < 260 && result.pause.is_none() {
            result = session.advance(16);
        }
        assert_eq!(result.final_week, 260);
        assert_eq!(result.pause.as_deref(), Some("five_year_score"));
        let score = session.world.score.clone().expect("a score is recorded");
        assert_eq!(score.week.0, 260);
        assert!((0..=100).contains(&score.total));
        // Continuing from the score sheet is an ordinary advance, not a new career.
        session.dismiss_pause();
        let continued = session.advance(4);
        assert_eq!(continued.weeks_advanced, 4);
        assert_eq!(session.world.tick.0, 264);
        assert!(session.world.score.is_some(), "the recorded score is kept");
    }

    #[test]
    fn score_net_worth_counts_direct_holdings_once_and_ignores_firm_cash() {
        let session = Session::create(config(202)).unwrap();
        let world = &session.world;
        let mut expected = world.investor.cash_cents - world.investor.personal_loans_cents;
        for (_, firm) in world.firms.iter() {
            expected += firm.shares.qty(cw_sim::state::HolderId::Investor) * firm.shares.fair_cents;
        }
        assert_eq!(
            cw_sim::economy::score::personal_net_worth(world),
            expected.max(0)
        );
        // The guided company's cash is not the investor's cash.
        let primary = world.investor.primary_group.unwrap();
        let firm_cash = world.firm(primary).unwrap().book.cash();
        assert!(firm_cash > 0);
        assert!(
            cw_sim::economy::score::personal_net_worth(world)
                != world.investor.cash_cents + firm_cash
        );
    }

    #[test]
    fn duplicate_command_ids_are_idempotent() {
        let mut session = Session::create(config(175)).unwrap();
        let primary = session.world.investor.primary_group.unwrap();
        let envelope = CommandEnvelope {
            command_id_high: 7,
            command_id_low: 9,
            actor: 0,
            expected_revision: session.revision(),
            submitted_tick: 0,
            intent: CommandIntent::SetProductionTarget {
                firm: primary,
                target_bps: 9_000,
            },
        };
        session.submit(envelope.clone()).unwrap();
        let before = session.revision();
        let receipt = session.submit(envelope).unwrap();
        assert_eq!(receipt.sequence, 0);
        assert_eq!(session.revision(), before);
    }
}
