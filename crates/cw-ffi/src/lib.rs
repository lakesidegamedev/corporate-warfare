//! UniFFI boundary.
//!
//! Owned, copyable DTOs cross into Swift. The session handle owns one mutable world behind a
//! mutex; Swift never computes a financial identity or mutates state directly. The full command
//! catalogue is submitted as JSON (the catalogue is generated from the engine's serde types) and
//! answered with a typed rejection record. Frequent phone flows also have typed convenience
//! methods so the UI cannot encode a malformed intent by hand.

use std::sync::{Arc, Mutex};

use cw_session::dto::{
    ContractLine, DealLine, EventLine, FacilitySummary, FinanceSummary, FirmSummary, InsuranceLine,
    LoanLine, Overview, OwnershipSummary, RegionSummary,
};
use cw_session::{Query, Session};

uniffi::setup_scaffolding!();

#[derive(uniffi::Record)]
pub struct EngineInfo {
    pub version: String,
    pub interface_version: u32,
    pub rules_version: u32,
    pub dataset_hash: String,
}

#[derive(uniffi::Error, Debug, Clone)]
pub enum FfiError {
    /// A typed domain rejection. `key` is a stable message key, `detail` is safe for developers.
    Domain { key: String, detail: String },
    /// A save could not be decoded.
    Save { key: String, detail: String },
    /// An internal invariant failed. Never a normal product path.
    Internal { detail: String },
}

impl std::fmt::Display for FfiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfiError::Domain { key, detail } => write!(formatter, "{key}: {detail}"),
            FfiError::Save { key, detail } => write!(formatter, "{key}: {detail}"),
            FfiError::Internal { detail } => write!(formatter, "internal: {detail}"),
        }
    }
}

impl std::error::Error for FfiError {}

impl From<cw_core::error::DomainError> for FfiError {
    fn from(error: cw_core::error::DomainError) -> Self {
        FfiError::Domain {
            key: error.key().to_owned(),
            detail: format!("{error:?}"),
        }
    }
}

#[derive(uniffi::Record)]
pub struct AdvanceDto {
    pub weeks_advanced: u32,
    pub revision: u64,
    pub final_week: u32,
    pub pause: Option<String>,
}

#[derive(uniffi::Record)]
pub struct ReceiptDto {
    pub accepted: bool,
    pub sequence: u64,
    pub revision: u64,
    pub error_key: Option<String>,
    pub error_detail: Option<String>,
}

#[derive(uniffi::Record)]
pub struct FirmDto {
    pub handle: u64,
    pub name: String,
    pub industry: String,
    pub region: String,
    pub cash_cents: i64,
    pub equity_cents: i64,
    pub fair_value_cents: i64,
    pub mark_cents: i64,
    pub grade: String,
    pub status: String,
    pub investor_votes_bps: i32,
    pub controlled: bool,
    pub ceo_is_investor: bool,
    pub facilities: u32,
    pub restricted: bool,
}

#[derive(uniffi::Record)]
pub struct RegionDto {
    pub index: u32,
    pub code: String,
    pub name: String,
    pub population: i64,
    pub employed: i64,
    pub wage_low_cents: i64,
    pub wage_mid_cents: i64,
    pub wage_high_cents: i64,
    pub savings_cents: i64,
    pub demand_index_bps: i32,
    pub land_index_bps: i32,
    pub organisation_bps: i32,
    pub distance_km: i64,
}

#[derive(uniffi::Record)]
pub struct FacilityDto {
    pub handle: u64,
    pub firm: u64,
    pub recipe: String,
    pub blocks: u32,
    pub region: String,
    pub status: String,
    pub capacity_lots: i64,
    pub last_output_lots: i64,
    pub workers_low: i64,
    pub workers_mid: i64,
    pub workers_high: i64,
    pub bottleneck: String,
    pub target_bps: i32,
    pub reopen_cost_cents: i64,
}

#[derive(uniffi::Record)]
pub struct FinanceDto {
    pub firm: u64,
    pub cash_cents: i64,
    pub restricted_cents: i64,
    pub inventory_cents: i64,
    pub assets_cents: i64,
    pub liabilities_cents: i64,
    pub equity_cents: i64,
    pub revenue_trailing_cents: i64,
    pub ebitda_trailing_cents: i64,
    pub net_income_trailing_cents: i64,
    pub bridge_opening_cents: i64,
    pub bridge_receipts_cents: i64,
    pub bridge_operating_cents: i64,
    pub bridge_investment_cents: i64,
    pub bridge_financing_cents: i64,
    pub bridge_tax_cents: i64,
    pub bridge_closing_cents: i64,
    pub recent_weeks: Vec<WeekDto>,
}

#[derive(uniffi::Record)]
pub struct WeekDto {
    pub week: u32,
    pub revenue_cents: i64,
    pub ebit_cents: i64,
    pub net_income_cents: i64,
}

#[derive(uniffi::Record)]
pub struct HolderDto {
    pub holder: String,
    pub quantity: i64,
    pub votes_bps: i32,
    pub cost_cents: i64,
}

#[derive(uniffi::Record)]
pub struct OwnershipDto {
    pub firm: u64,
    pub issued: i64,
    pub treasury: i64,
    pub eligible_votes: i64,
    pub controller: Option<String>,
    pub holders: Vec<HolderDto>,
    pub board_seats: Vec<String>,
    pub next_meeting: u32,
    pub ceo: String,
}

#[derive(uniffi::Record)]
pub struct ContractDto {
    pub handle: u64,
    pub buyer: String,
    pub seller: String,
    pub product: String,
    pub quantity_weekly: i64,
    pub price_cents: i64,
    pub remaining_weeks: u32,
    pub status: String,
    pub from_region: String,
    pub to_region: String,
}

#[derive(uniffi::Record)]
pub struct LoanDto {
    pub handle: u64,
    pub borrower: String,
    pub lender: String,
    pub principal_cents: i64,
    pub rate_bps: i32,
    pub remaining_weeks: u32,
    pub secured: bool,
    pub defaulted: bool,
}

#[derive(uniffi::Record)]
pub struct InsuranceDto {
    pub handle: u64,
    pub insurer: String,
    pub insured: String,
    pub facility: u64,
    pub limit_cents: i64,
    pub deductible_cents: i64,
    pub weekly_premium_cents: i64,
    pub status: String,
    pub claim_reserve_cents: i64,
}

#[derive(uniffi::Record)]
pub struct DealDto {
    pub handle: u64,
    pub buyer: String,
    pub target: String,
    pub buyer_handle: u64,
    pub target_handle: u64,
    pub kind: String,
    pub state: String,
    pub offer_price_cents: i64,
    pub escrow_cents: i64,
    pub committed_shares: i64,
    pub deadline: Option<u32>,
    pub review_deadline: Option<u32>,
}

#[derive(uniffi::Record)]
pub struct ChoiceDto {
    pub id: String,
    pub label: String,
}

#[derive(uniffi::Record)]
pub struct EventDto {
    pub id: u64,
    pub card: String,
    pub title: String,
    pub subject: String,
    pub mandatory: bool,
    pub deadline: u32,
    pub choices: Vec<ChoiceDto>,
}

#[derive(uniffi::Record)]
pub struct MarketDto {
    pub product: String,
    pub product_id: String,
    pub reference_price_cents: i64,
    pub last_price_cents: i64,
    pub regional_stock_lots: i64,
    pub last_week_sales_lots: i64,
    pub gateway_buy_cents: i64,
    pub gateway_sell_cents: i64,
}

#[derive(uniffi::Record)]
pub struct OverviewDto {
    pub tick: u32,
    pub week_label: String,
    pub revision: u64,
    pub investor_cash_cents: i64,
    pub investor_net_worth_cents: i64,
    pub primary_firm: Option<u64>,
    pub primary_name: String,
    pub firm_cash_cents: i64,
    pub forecast_four_week_cents: i64,
    pub last_week_ebit_cents: i64,
    pub alerts: Vec<String>,
    pub pause: Option<String>,
    pub next_action_key: String,
    pub next_action_reason: String,
}

#[derive(uniffi::Record)]
pub struct ScoreDto {
    pub week: u32,
    pub total: i32,
    /// Growth of personal net worth over the start.
    pub return_component: i32,
    pub expansion_component: i32,
    pub leadership_component: i32,
    pub resilience_component: i32,
    /// Reported, not scored.
    pub control_component: i32,
    pub conduct_component: i32,
    pub assisted: bool,
    pub personal_net_worth_cents: i64,
}

#[derive(uniffi::Record)]
pub struct ProductDto {
    pub index: u32,
    pub name: String,
    pub reference_price_cents: i64,
    pub kind: String,
}

#[derive(uniffi::Record)]
pub struct RecipeDto {
    pub index: u32,
    pub id: String,
    pub industry: String,
    pub output: String,
    pub capex_cents: i64,
    pub build_weeks: u32,
    pub weekly_output_lots: i64,
}

/// One node of the investor's controlled group tree, breadth first from a direct holding.
#[derive(uniffi::Record)]
pub struct GroupNodeDto {
    pub handle: u64,
    pub name: String,
    pub region: String,
    pub industry: String,
    pub parent: Option<u64>,
    pub depth: u32,
    /// The parent firm's voting share of this firm, or the investor's when it has no parent.
    pub control_bps: i32,
    pub investor_votes_bps: i32,
    /// The investor's economic look-through fraction of this firm in basis points.
    pub look_through_bps: i32,
    pub cash_cents: i64,
    pub status: String,
    pub directly_controlled: bool,
}

/// A labelled preview. `blocked_key` is a stable domain key, never Rust text in product copy.
#[derive(uniffi::Record)]
pub struct PreviewDto {
    pub revision: u64,
    pub cash_change_cents: i64,
    pub assumptions: Vec<String>,
    pub warnings: Vec<String>,
    pub blocked_key: Option<String>,
    pub blocked_detail: Option<String>,
}

#[derive(uniffi::Record)]
pub struct LabourDto {
    pub firm: u64,
    /// idle, ballot, demanding or striking.
    pub state: String,
    pub demand_deadline: Option<u32>,
    pub strike_until: Option<u32>,
    pub organisation_bps: i32,
    pub ballot_support_bps: i32,
    pub demand_cents: i64,
    pub last_offer_cents: i64,
    pub settlement_cost_cents: i64,
    pub current_wage_cents: i64,
    pub employees: i64,
}

#[derive(uniffi::Record)]
pub struct PolicyRequestDto {
    pub kind: String,
    pub region_name: String,
    pub fee_cents: i64,
    pub decision_week: u32,
    pub resolved: bool,
    pub success: bool,
    pub cooldown_until: u32,
}

#[derive(uniffi::Record)]
pub struct RegionCooldownDto {
    pub region: u32,
    pub name: String,
    pub available_week: u32,
    pub active: bool,
}

#[derive(uniffi::Record)]
pub struct PolicyStateDto {
    pub firm: u64,
    pub fee_cents: i64,
    pub reputation_bps: i32,
    pub requests: Vec<PolicyRequestDto>,
    pub cooldowns: Vec<RegionCooldownDto>,
}

#[derive(uniffi::Record)]
pub struct InstitutionDto {
    pub handle: u64,
    pub name: String,
    pub region: String,
    /// bank, insurer or both.
    pub kind: String,
    pub controlled: bool,
    pub deposit_rate_bps: i32,
    pub cash_cents: i64,
    pub equity_cents: i64,
    pub deposits_cents: i64,
    pub loans_cents: i64,
    pub coverage_cents: i64,
    pub specialist: bool,
    /// True when the institution meets its own liquidity requirement.
    pub liquidity_ok: bool,
    /// True when the institution meets its own capital requirement.
    pub capital_ok: bool,
}

#[derive(uniffi::Record)]
pub struct RecoveryDto {
    pub eligible: bool,
    /// recovery.available, recovery.not_destitute, recovery.cooldown or recovery.no_target.
    pub reason_key: String,
    pub cash_cents: i64,
    pub next_eligible_week: u32,
    pub signing_bonus_cents: i64,
    pub managed_firms: u32,
    pub candidate: Option<String>,
    pub formation_fee_cents: i64,
    pub minimum_capital_cents: i64,
}

#[derive(uniffi::Record)]
pub struct BoardVoteDto {
    pub id: u32,
    pub subject: String,
    pub deadline: u32,
    pub yes: u32,
    pub no: u32,
    pub resolved: bool,
    pub passed: bool,
}

#[derive(uniffi::Record)]
pub struct QuoteDto {
    pub weekly_premium_cents: i64,
    pub blocked_key: Option<String>,
    pub blocked_detail: Option<String>,
}

/// One input of an operating strategy, in lots per block-week.
#[derive(uniffi::Record)]
pub struct StrategyInputDto {
    pub product: String,
    pub product_name: String,
    pub lots: i64,
}

/// An operating strategy (recipe) with everything the build and retool screens show.
#[derive(uniffi::Record)]
pub struct StrategyDto {
    pub index: u32,
    pub id: String,
    pub name: String,
    pub industry: String,
    pub output: String,
    pub output_name: String,
    pub weekly_output_lots: i64,
    pub inputs: Vec<StrategyInputDto>,
    pub workers_low: i64,
    pub workers_mid: i64,
    pub workers_high: i64,
    pub capex_cents: i64,
    pub build_weeks: u32,
    pub life_years: u32,
    /// EBITDA margin at reference prices and baseline wages, full capacity, in basis points.
    pub reference_margin_bps: i32,
    pub depletes: bool,
}

/// A good a state imports: demand local plants are not meeting.
#[derive(uniffi::Record)]
pub struct RoomDto {
    pub product: String,
    pub product_name: String,
    pub import_lots: i64,
    pub import_price_cents: i64,
}

/// A playable state with its conditions and room to build.
#[derive(uniffi::Record)]
pub struct RegionBoardDto {
    pub index: u32,
    pub code: String,
    pub name: String,
    pub state_code: String,
    pub state_name: String,
    /// The state's specialized sector types, primary first.
    pub specialization: Vec<String>,
    pub hub_city: String,
    pub land_index_bps: i32,
    pub population_2000: i64,
    pub wage_index_bps: i32,
    pub demand_index_bps: i32,
    pub power_grid_bps: i32,
    pub road_bps: i32,
    pub broadband_bps: i32,
    pub skill_bps: i32,
    pub crime_bps: i32,
    pub cost_of_living_bps: i32,
    pub organisation_bps: i32,
    pub room: Vec<RoomDto>,
    /// Every sector type with operators or unmet imports here.
    pub sectors: Vec<SectorCellDto>,
}

/// One sector type in one region: who operates and how the player's firm stands.
#[derive(uniffi::Record)]
pub struct SectorCellDto {
    pub industry: String,
    pub operators: u32,
    pub leader: String,
    pub leader_share_bps: i32,
    /// The selected firm's blocks and share here; zero when it has no division.
    pub own_blocks: u32,
    pub own_share_bps: i32,
    pub headroom_cents: i64,
}

/// One of a firm's divisions: its plants of one sector type in one region.
#[derive(uniffi::Record)]
pub struct DivisionDto {
    pub region: String,
    pub region_name: String,
    pub industry: String,
    pub strategy: u32,
    pub strategy_name: String,
    pub plants: Vec<u64>,
    pub blocks: u32,
    pub share_bps: i32,
    pub retooling: bool,
}

/// The board's quarterly objective for the player's primary company.
#[derive(uniffi::Record)]
pub struct ObjectiveDto {
    pub title: String,
    pub detail: String,
    pub progress_bps: i32,
    pub due_week: u32,
    pub weeks_left: u32,
}

/// A closed objective.
#[derive(uniffi::Record)]
pub struct ObjectiveResultDto {
    pub title: String,
    pub met: bool,
    pub closed_week: u32,
    pub bonus_cents: i64,
}

/// A player division on the map: one circle per owned sector type in a region.
#[derive(uniffi::Record)]
pub struct MapDivisionDto {
    pub firm: u64,
    pub region: String,
    pub industry: String,
    pub plants: u32,
    pub blocks: u32,
}

/// What a build costs: entry fee, founding discount, dominance and location.
#[derive(uniffi::Record)]
pub struct BuildQuoteDto {
    pub capex_cents: i64,
    pub entry_fee_cents: i64,
    pub total_cents: i64,
    pub opens_division: bool,
    pub dominance_bps: i64,
    pub share_after_bps: i64,
    /// Input need where the plant would sit, in basis points of calibration.
    pub location_input_bps: i32,
    /// The existing division's strategy when the build must use it.
    pub locked_strategy: Option<u32>,
    pub locked_strategy_name: Option<String>,
}

/// A rival division the selected firm can raid.
#[derive(uniffi::Record)]
pub struct RaidOptionDto {
    pub target: u64,
    pub target_name: String,
    pub region: String,
    pub region_index: u32,
    pub industry: String,
    pub industry_index: u32,
    pub rival_blocks: u32,
    pub ready_week: u32,
    pub suggested_budget_cents: i64,
    pub quote: RaidQuoteDto,
}

#[derive(uniffi::Record)]
pub struct RaidQuoteDto {
    pub attack_cents: i64,
    pub defence_cents: i64,
    pub chance_bps: i32,
    pub blocks_at_stake: u32,
    pub price_cents: i64,
}

#[derive(uniffi::Record)]
pub struct RaidRecordDto {
    pub week: u32,
    pub attacker: String,
    pub defender: String,
    pub region: String,
    pub industry: String,
    pub success: bool,
    pub blocks: u32,
    pub player_attacker: bool,
    pub player_defender: bool,
}

/// Every plant in the world for the map. Rival plants show only public facts.
#[derive(uniffi::Record)]
pub struct MapPlantDto {
    pub handle: u64,
    pub firm: u64,
    pub firm_name: String,
    /// The investor ultimately controls the owner.
    pub player: bool,
    pub region: String,
    pub industry: String,
    pub strategy: String,
    pub blocks: u32,
    pub status: String,
    pub utilisation_bps: i32,
}

/// A firm's sector focus and logistics position.
#[derive(uniffi::Record)]
pub struct FirmOpsDto {
    pub firm: u64,
    pub primary: String,
    pub secondary: Option<String>,
    pub focus_transition_weeks: u32,
    pub focus_cooldown_weeks: u32,
    pub logistics_strength: i32,
    pub logistics_budget_cents: i64,
    pub plants: u32,
    /// Divisions run; sprawl applies past the threshold.
    pub divisions: u32,
    pub sprawl_threshold: u32,
}

#[derive(uniffi::Record)]
pub struct BuyerDto {
    pub index: u32,
    pub name: String,
    pub capital_cents: i64,
}

#[derive(uniffi::Object)]
pub struct SessionHandle {
    inner: Mutex<Session>,
}

fn overview_dto(overview: Overview) -> OverviewDto {
    OverviewDto {
        tick: overview.tick,
        week_label: overview.week_label,
        revision: overview.revision,
        investor_cash_cents: overview.investor_cash_cents,
        investor_net_worth_cents: overview.investor_net_worth_cents,
        primary_firm: overview.primary_firm,
        primary_name: overview.primary_name,
        firm_cash_cents: overview.firm_cash_cents,
        forecast_four_week_cents: overview.forecast_four_week_cents,
        last_week_ebit_cents: overview.last_week_ebit_cents,
        alerts: overview.alerts.into_iter().map(|alert| alert.key).collect(),
        pause: overview.pause,
        next_action_key: overview.next_action_key,
        next_action_reason: overview.next_action_reason,
    }
}

fn firm_dto(summary: FirmSummary) -> FirmDto {
    FirmDto {
        handle: summary.handle,
        name: summary.name,
        industry: summary.industry,
        region: summary.region,
        cash_cents: summary.cash_cents,
        equity_cents: summary.equity_cents,
        fair_value_cents: summary.fair_value_cents,
        mark_cents: summary.mark_cents,
        grade: summary.grade,
        status: summary.status,
        investor_votes_bps: summary.investor_votes_bps,
        controlled: summary.controlled,
        ceo_is_investor: summary.ceo_is_investor,
        facilities: summary.facilities,
        restricted: summary.restricted,
    }
}

fn region_dto(index: u32, summary: RegionSummary) -> RegionDto {
    RegionDto {
        index,
        code: summary.code,
        name: summary.name,
        population: summary.population,
        employed: summary.employed,
        wage_low_cents: summary.prevailing_wage_cents[0],
        wage_mid_cents: summary.prevailing_wage_cents[1],
        wage_high_cents: summary.prevailing_wage_cents[2],
        savings_cents: summary.households_savings_cents,
        demand_index_bps: summary.demand_index_bps,
        land_index_bps: summary.land_index_bps,
        organisation_bps: summary.organisation_bps,
        distance_km: summary.distance_km_from_selected,
    }
}

fn facility_dto(summary: FacilitySummary) -> FacilityDto {
    FacilityDto {
        handle: summary.handle,
        firm: summary.firm,
        recipe: summary.recipe,
        blocks: summary.blocks as u32,
        region: summary.region,
        status: summary.status,
        capacity_lots: summary.capacity,
        last_output_lots: summary.last_output,
        workers_low: summary.workers[0],
        workers_mid: summary.workers[1],
        workers_high: summary.workers[2],
        bottleneck: summary.bottleneck,
        target_bps: summary.target_bps,
        reopen_cost_cents: summary.reopen_cost_cents,
    }
}

fn finance_dto(summary: FinanceSummary) -> FinanceDto {
    FinanceDto {
        firm: summary.firm,
        cash_cents: summary.cash_cents,
        restricted_cents: summary.restricted_cents,
        inventory_cents: summary.inventory_cents,
        assets_cents: summary.assets_cents,
        liabilities_cents: summary.liabilities_cents,
        equity_cents: summary.equity_cents,
        revenue_trailing_cents: summary.revenue_trailing_cents,
        ebitda_trailing_cents: summary.ebitda_trailing_cents,
        net_income_trailing_cents: summary.net_income_trailing_cents,
        bridge_opening_cents: summary.cash_bridge.opening_cents,
        bridge_receipts_cents: summary.cash_bridge.receipts_cents,
        bridge_operating_cents: summary.cash_bridge.operating_payments_cents,
        bridge_investment_cents: summary.cash_bridge.investment_cents,
        bridge_financing_cents: summary.cash_bridge.financing_cents,
        bridge_tax_cents: summary.cash_bridge.tax_cents,
        bridge_closing_cents: summary.cash_bridge.closing_cents,
        recent_weeks: summary
            .recent_weeks
            .into_iter()
            .map(|week| WeekDto {
                week: week.week,
                revenue_cents: week.revenue_cents,
                ebit_cents: week.ebit_cents,
                net_income_cents: week.net_income_cents,
            })
            .collect(),
    }
}

fn ownership_dto(summary: OwnershipSummary) -> OwnershipDto {
    OwnershipDto {
        firm: summary.firm,
        issued: summary.issued,
        treasury: summary.treasury,
        eligible_votes: summary.eligible_votes,
        controller: summary.controller,
        holders: summary
            .holders
            .into_iter()
            .map(|holder| HolderDto {
                holder: holder.holder,
                quantity: holder.quantity,
                votes_bps: holder.votes_bps,
                cost_cents: holder.cost_cents,
            })
            .collect(),
        board_seats: summary
            .board_seats
            .iter()
            .map(|seat| seat.clone().unwrap_or_else(|| "Vacant".to_owned()))
            .collect(),
        next_meeting: summary.next_meeting,
        ceo: summary.ceo,
    }
}

fn contract_dto(line: ContractLine) -> ContractDto {
    ContractDto {
        handle: line.handle,
        buyer: line.buyer,
        seller: line.seller,
        product: line.product,
        quantity_weekly: line.quantity_weekly,
        price_cents: line.price_cents,
        remaining_weeks: line.remaining_weeks,
        status: line.status,
        from_region: line.from_region,
        to_region: line.to_region,
    }
}

fn loan_dto(line: LoanLine) -> LoanDto {
    LoanDto {
        handle: line.handle,
        borrower: line.borrower,
        lender: line.lender,
        principal_cents: line.principal_cents,
        rate_bps: line.rate_bps,
        remaining_weeks: line.remaining_weeks,
        secured: line.secured,
        defaulted: line.defaulted,
    }
}

fn insurance_dto(line: InsuranceLine) -> InsuranceDto {
    InsuranceDto {
        handle: line.handle,
        insurer: line.insurer,
        insured: line.insured,
        facility: line.facility,
        limit_cents: line.limit_cents,
        deductible_cents: line.deductible_cents,
        weekly_premium_cents: line.weekly_premium_cents,
        status: line.status,
        claim_reserve_cents: line.claim_reserve_cents,
    }
}

fn deal_dto(line: DealLine) -> DealDto {
    DealDto {
        handle: line.handle,
        buyer: line.buyer,
        target: line.target,
        buyer_handle: line.buyer_handle,
        target_handle: line.target_handle,
        kind: line.kind,
        state: line.state,
        offer_price_cents: line.offer_price_cents,
        escrow_cents: line.escrow_cents,
        committed_shares: line.committed_shares,
        deadline: line.deadline,
        review_deadline: line.review_deadline,
    }
}

fn event_dto(line: EventLine) -> EventDto {
    EventDto {
        id: line.id,
        card: line.card,
        title: line.title,
        subject: line.subject,
        mandatory: line.mandatory,
        deadline: line.deadline,
        choices: line
            .choices
            .into_iter()
            .map(|choice| ChoiceDto {
                id: choice.id,
                label: choice.label,
            })
            .collect(),
    }
}

#[uniffi::export]
pub fn engine_info() -> EngineInfo {
    EngineInfo {
        version: cw_core::ENGINE_VERSION.to_owned(),
        interface_version: cw_core::INTERFACE_VERSION,
        rules_version: cw_core::RULES_VERSION,
        dataset_hash: cw_data::Dataset::load()
            .map(|dataset| dataset.hash)
            .unwrap_or_default(),
    }
}

#[uniffi::export]
pub fn products() -> Vec<ProductDto> {
    match cw_data::Dataset::load() {
        Ok(dataset) => dataset
            .products
            .iter()
            .enumerate()
            .map(|(index, product)| ProductDto {
                index: index as u32,
                name: product.name.clone(),
                reference_price_cents: product.reference_price_cents,
                kind: format!("{:?}", product.kind).to_lowercase(),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Facility templates with their capex and build time, for build and founder forms.
#[uniffi::export]
pub fn recipes() -> Vec<RecipeDto> {
    match cw_data::Dataset::load() {
        Ok(dataset) => dataset
            .recipes
            .iter()
            .enumerate()
            .map(|(index, recipe)| RecipeDto {
                index: index as u32,
                id: recipe.id.clone(),
                industry: recipe.industry.clone(),
                output: recipe.output.clone(),
                capex_cents: recipe.capex_cents,
                build_weeks: recipe.build_weeks,
                weekly_output_lots: recipe.weekly_output_lots,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The full strategy catalogue with inputs and reference margins.
/// Player-facing title and detail for an objective.
fn objective_copy(
    dataset: &cw_data::Dataset,
    kind: &cw_sim::economy::objectives::ObjectiveKind,
) -> (String, String) {
    use cw_sim::economy::objectives::ObjectiveKind;
    let money = |cents: i64| {
        let dollars = cents / 100;
        if dollars.abs() >= 1_000 {
            format!("${}K", dollars / 1_000)
        } else {
            format!("${dollars}")
        }
    };
    match kind {
        ObjectiveKind::NewRegion { .. } => (
            "Enter a new region".to_owned(),
            "Open a division in a region where the company does not operate yet.".to_owned(),
        ),
        ObjectiveKind::NewState { .. } => (
            "Expand into another state".to_owned(),
            "Open a division in a state where the company does not operate yet.".to_owned(),
        ),
        ObjectiveKind::WeeklyEbit { target_cents } => (
            format!("Lift operating profit to {} a week", money(*target_cents)),
            "Average weekly operating profit over the last four weeks.".to_owned(),
        ),
        ObjectiveKind::MarketShare {
            region,
            industry,
            target_bps,
        } => (
            format!(
                "Take {}% of {} in {}",
                target_bps / 100,
                cw_sim::state::INDUSTRY_NAMES[*industry as usize].replace('_', " "),
                dataset.regions[*region as usize].name
            ),
            "Capacity share of the market, counting unmet imports.".to_owned(),
        ),
    }
}

fn raid_quote_dto(quote: cw_sim::economy::sectors::RaidQuote) -> RaidQuoteDto {
    RaidQuoteDto {
        attack_cents: quote.attack_cents,
        defence_cents: quote.defence_cents,
        chance_bps: quote.chance_bps,
        blocks_at_stake: quote.blocks_at_stake as u32,
        price_cents: quote.price_cents,
    }
}

#[uniffi::export]
pub fn strategies() -> Vec<StrategyDto> {
    let Ok(dataset) = cw_data::Dataset::load() else {
        return Vec::new();
    };
    let price = |id: &str| {
        dataset
            .product_index
            .get(id)
            .map(|code| dataset.reference_price(*code))
            .unwrap_or(0)
    };
    let name = |id: &str| {
        dataset
            .product_index
            .get(id)
            .map(|code| dataset.products[*code as usize].name.clone())
            .unwrap_or_else(|| id.to_owned())
    };
    dataset
        .recipes
        .iter()
        .enumerate()
        .map(|(index, recipe)| {
            let revenue = recipe.weekly_output_lots * price(&recipe.output);
            let inputs: i64 = recipe
                .weekly_inputs_lots
                .iter()
                .map(|(id, qty)| qty * price(id))
                .sum();
            let cost = inputs
                + recipe.baseline_weekly_payroll(dataset.baseline_weekly_wage_cents)
                + recipe.weekly_maintenance(dataset.maintenance_annual_bps)
                + dataset.fixture_land_rent_cents_weekly;
            StrategyDto {
                index: index as u32,
                id: recipe.id.clone(),
                name: if recipe.name.is_empty() {
                    recipe.id.clone()
                } else {
                    recipe.name.clone()
                },
                industry: recipe.industry.clone(),
                output: recipe.output.clone(),
                output_name: name(&recipe.output),
                weekly_output_lots: recipe.weekly_output_lots,
                inputs: recipe
                    .weekly_inputs_lots
                    .iter()
                    .map(|(id, qty)| StrategyInputDto {
                        product: id.clone(),
                        product_name: name(id),
                        lots: *qty,
                    })
                    .collect(),
                workers_low: recipe.workers_low_mid_high[0],
                workers_mid: recipe.workers_low_mid_high[1],
                workers_high: recipe.workers_low_mid_high[2],
                capex_cents: recipe.capex_cents,
                build_weeks: recipe.build_weeks,
                life_years: recipe.life_years,
                reference_margin_bps: if revenue > 0 {
                    ((revenue - cost) as i128 * 10_000 / revenue as i128) as i32
                } else {
                    0
                },
                depletes: recipe.depletes(),
            }
        })
        .collect()
}

fn preview_dto(preview: cw_sim::commands::Preview) -> PreviewDto {
    PreviewDto {
        revision: preview.revision,
        cash_change_cents: preview.cash_change_cents,
        assumptions: preview.assumptions,
        warnings: preview.warnings,
        blocked_key: preview.blocked.as_ref().map(|error| error.key().to_owned()),
        blocked_detail: preview.blocked.as_ref().map(|error| format!("{error:?}")),
    }
}

fn policy_kind_key(kind: cw_sim::state::PolicyKind) -> &'static str {
    match kind {
        cw_sim::state::PolicyKind::PermitAcceleration => "permit_acceleration",
        cw_sim::state::PolicyKind::TrainingGrant => "training_grant",
        cw_sim::state::PolicyKind::TaxAdjustment => "tax_adjustment",
    }
}

#[uniffi::export]
pub fn import_save(bytes: Vec<u8>) -> Result<Arc<SessionHandle>, FfiError> {
    let session = Session::import_save(&bytes).map_err(|error| FfiError::Save {
        key: error.key().to_owned(),
        detail: format!("{error:?}"),
    })?;
    Ok(Arc::new(SessionHandle {
        inner: Mutex::new(session),
    }))
}

#[uniffi::export]
impl SessionHandle {
    #[uniffi::constructor]
    pub fn new(seed: u64, guided: bool, five_year: bool) -> Result<Arc<SessionHandle>, FfiError> {
        let config = cw_session::ScenarioConfig {
            seed,
            scenario: if guided {
                cw_sim::state::ScenarioKind::Guided
            } else {
                cw_sim::state::ScenarioKind::Founder
            },
            mode: if five_year {
                cw_sim::state::ScenarioMode::FiveYear
            } else {
                cw_sim::state::ScenarioMode::Sandbox
            },
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
        };
        let session = Session::create(config)?;
        Ok(Arc::new(SessionHandle {
            inner: Mutex::new(session),
        }))
    }

    pub fn overview(&self) -> OverviewDto {
        overview_dto(self.lock().overview())
    }

    pub fn advance(&self, weeks: u32) -> AdvanceDto {
        let result = self.lock().advance(weeks);
        AdvanceDto {
            weeks_advanced: result.weeks_advanced,
            revision: result.revision,
            final_week: result.final_week,
            pause: result.pause,
        }
    }

    /// Dismiss the week-260 score pause and keep playing the same career.
    pub fn dismiss_pause(&self) {
        self.lock().dismiss_pause();
    }

    pub fn revision(&self) -> u64 {
        self.lock().revision()
    }

    /// The recorded five-year score, if the career has reached week 260.
    pub fn score(&self) -> Option<ScoreDto> {
        let session = self.lock();
        session.world.score.as_ref().map(|score| ScoreDto {
            week: score.week.0,
            total: score.total,
            return_component: score.return_component,
            expansion_component: score.expansion_component,
            leadership_component: score.leadership_component,
            resilience_component: score.resilience_component,
            control_component: score.control_component,
            conduct_component: score.conduct_component,
            assisted: score.assisted,
            personal_net_worth_cents: score.personal_net_worth_cents,
        })
    }

    pub fn canonical_hash(&self) -> String {
        self.lock().canonical_hash()
    }

    pub fn export_save(&self) -> Result<Vec<u8>, FfiError> {
        self.lock().export_save().map_err(FfiError::from)
    }

    pub fn firms(&self) -> Vec<FirmDto> {
        match self.lock().query(Query::Firms) {
            cw_session::Detail::Firms(rows) => rows.into_iter().map(firm_dto).collect(),
            _ => Vec::new(),
        }
    }

    pub fn firm(&self, handle: u64) -> Option<FirmDto> {
        match self.lock().query(Query::Firm(handle)) {
            cw_session::Detail::Firm(row) if !row.name.is_empty() => Some(firm_dto(row)),
            _ => None,
        }
    }

    pub fn facilities(&self, firm: u64) -> Vec<FacilityDto> {
        match self.lock().query(Query::Facilities(firm)) {
            cw_session::Detail::Facilities(rows) => rows.into_iter().map(facility_dto).collect(),
            _ => Vec::new(),
        }
    }

    pub fn regions(&self, selected_code: Option<String>) -> Vec<RegionDto> {
        match self.lock().query(Query::Regions(selected_code)) {
            cw_session::Detail::Regions(rows) => rows
                .into_iter()
                .enumerate()
                .map(|(index, row)| region_dto(index as u32, row))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The eight playable states with conditions and room to build.
    /// Every region's board: conditions, room to build and the sector cells, with the
    /// selected firm's position in each.
    pub fn region_boards(&self, firm: Option<u64>) -> Vec<RegionBoardDto> {
        use cw_sim::economy::sectors;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let own = firm.map(cw_core::ids::Handle);
        dataset
            .regions
            .iter()
            .enumerate()
            .map(|(index, region)| {
                let mut room: Vec<RoomDto> = world
                    .gateways
                    .iter()
                    .filter(|g| g.region as usize == index && g.smoothed_import_lots > 0)
                    .map(|g| RoomDto {
                        product: dataset.products[g.product as usize].id.clone(),
                        product_name: dataset.products[g.product as usize].name.clone(),
                        import_lots: g.smoothed_import_lots,
                        import_price_cents: g.sell_price_cents,
                    })
                    .collect();
                room.sort_by(|a, b| {
                    (b.import_lots * b.import_price_cents)
                        .cmp(&(a.import_lots * a.import_price_cents))
                });
                let mut cells = Vec::new();
                for (industry_index, industry) in cw_sim::state::INDUSTRY_NAMES.iter().enumerate() {
                    let industry_index = industry_index as u8;
                    let (firms, headroom) =
                        sectors::cell(world, dataset, index as u8, industry_index);
                    if firms.is_empty() && headroom <= 0 {
                        continue;
                    }
                    let total: i64 = firms.iter().map(|(_, v)| *v).sum::<i64>() + headroom;
                    let leader = firms
                        .iter()
                        .max_by_key(|(h, v)| (*v, std::cmp::Reverse(*h)));
                    let own_value = own
                        .and_then(|o| firms.iter().find(|(h, _)| *h == o))
                        .map(|(_, v)| *v)
                        .unwrap_or(0);
                    let own_blocks = own
                        .map(|o| {
                            sectors::division_plants(world, dataset, o, index as u8, industry_index)
                                .iter()
                                .filter_map(|h| world.facilities.get(*h))
                                .map(|p| p.blocks as u32)
                                .sum()
                        })
                        .unwrap_or(0);
                    let bps = |value: i64| {
                        if total > 0 {
                            (value as i128 * 10_000 / total as i128) as i32
                        } else {
                            0
                        }
                    };
                    cells.push(SectorCellDto {
                        industry: (*industry).to_owned(),
                        operators: firms.len() as u32,
                        leader: leader
                            .and_then(|(h, _)| world.firms.get(*h))
                            .map(|f| f.name.clone())
                            .unwrap_or_default(),
                        leader_share_bps: leader.map(|(_, v)| bps(*v)).unwrap_or(0),
                        own_blocks,
                        own_share_bps: bps(own_value),
                        headroom_cents: headroom,
                    });
                }
                let state = dataset.state_of(index as u8);
                let c = region.conditions;
                RegionBoardDto {
                    index: index as u32,
                    code: region.code.clone(),
                    name: region.name.clone(),
                    state_code: region.state().to_owned(),
                    state_name: state.map(|s| s.name.clone()).unwrap_or_default(),
                    specialization: state.map(|s| s.specialization.clone()).unwrap_or_default(),
                    hub_city: region.principal_city.clone(),
                    land_index_bps: region.land_index_bps,
                    population_2000: region.population_2000,
                    wage_index_bps: region.wage_index_bps,
                    demand_index_bps: region.demand_index_bps,
                    power_grid_bps: c.power_grid_bps,
                    road_bps: c.road_bps,
                    broadband_bps: c.broadband_bps,
                    skill_bps: c.skill_bps,
                    crime_bps: c.crime_bps,
                    cost_of_living_bps: c.cost_of_living_bps,
                    organisation_bps: world
                        .regions
                        .get(index)
                        .map(|r| r.organisation_bps)
                        .unwrap_or(region.opening_organisation_bps),
                    room,
                    sectors: cells,
                }
            })
            .collect()
    }

    /// A firm's divisions, by region then sector type.
    pub fn divisions(&self, firm: u64) -> Vec<DivisionDto> {
        use cw_sim::economy::sectors;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = cw_core::ids::Handle(firm);
        sectors::divisions_of(world, dataset, handle)
            .into_iter()
            .map(|d| {
                let recipe = &dataset.recipes[d.strategy as usize];
                DivisionDto {
                    region: dataset.regions[d.region as usize].code.clone(),
                    region_name: dataset.regions[d.region as usize].name.clone(),
                    industry: cw_sim::state::INDUSTRY_NAMES[d.industry as usize].to_owned(),
                    strategy: d.strategy as u32,
                    strategy_name: if recipe.name.is_empty() {
                        recipe.id.clone()
                    } else {
                        recipe.name.clone()
                    },
                    plants: d.plants.iter().map(|h| h.0).collect(),
                    blocks: d.blocks,
                    share_bps: sectors::share_bps(world, dataset, handle, d.region, d.industry, 0)
                        as i32,
                    retooling: d.plants.iter().any(|h| {
                        world.facilities.get(*h).is_some_and(|p| {
                            matches!(p.status, cw_sim::state::FacilityStatus::Retooling { .. })
                        })
                    }),
                }
            })
            .collect()
    }

    /// The board's current objective, if one is open.
    pub fn objective(&self) -> Option<ObjectiveDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let objective = world.objective.as_ref()?;
        let (title, detail) = objective_copy(dataset, &objective.kind);
        Some(ObjectiveDto {
            title,
            detail,
            progress_bps: cw_sim::economy::objectives::progress_bps(world, dataset, objective)
                as i32,
            due_week: objective.due,
            weeks_left: objective.due.saturating_sub(world.tick.0),
        })
    }

    /// Closed objectives, newest first.
    pub fn objective_history(&self) -> Vec<ObjectiveResultDto> {
        let session = self.lock();
        session
            .world
            .objective_history
            .iter()
            .rev()
            .map(|result| ObjectiveResultDto {
                title: objective_copy(&session.dataset, &result.objective.kind).0,
                met: result.met,
                closed_week: result.closed,
                bonus_cents: result.bonus_cents,
            })
            .collect()
    }

    /// The player's divisions for the map: one circle per owned sector type in a region.
    pub fn map_divisions(&self) -> Vec<MapDivisionDto> {
        use cw_sim::economy::sectors;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let mut out = Vec::new();
        for (handle, firm) in world.firms.iter() {
            if !firm.operative() || !cw_sim::economy::events_engine::player_side(world, handle) {
                continue;
            }
            for d in sectors::divisions_of(world, dataset, handle) {
                out.push(MapDivisionDto {
                    firm: handle.0,
                    region: dataset.regions[d.region as usize].code.clone(),
                    industry: cw_sim::state::INDUSTRY_NAMES[d.industry as usize].to_owned(),
                    plants: d.plants.len() as u32,
                    blocks: d.blocks,
                });
            }
        }
        out
    }

    /// What building `blocks` of `recipe` in `region` would cost the firm.
    pub fn build_quote(&self, firm: u64, region: u32, recipe: u32, blocks: u32) -> BuildQuoteDto {
        use cw_sim::economy::sectors;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = cw_core::ids::Handle(firm);
        let (region, recipe) = (region as u8, recipe as u16);
        let quote = sectors::build_cost(
            world,
            dataset,
            handle,
            region,
            recipe,
            blocks.clamp(1, cw_sim::economy::sectors::MAX_BLOCKS_PER_ORDER as u32) as u8,
        );
        let industry = sectors::recipe_industry(dataset, recipe);
        let locked = sectors::division_strategy(world, dataset, handle, region, industry);
        let home = world.firms.get(handle).map(|f| f.region).unwrap_or(region);
        BuildQuoteDto {
            capex_cents: quote.capex_cents,
            entry_fee_cents: quote.entry_fee_cents,
            total_cents: quote.total_cents(),
            opens_division: quote.opens_division,
            dominance_bps: quote.dominance_bps,
            share_after_bps: quote.share_after_bps,
            location_input_bps: sectors::location_factor_bps(
                dataset,
                home,
                region,
                &dataset.recipes[recipe as usize].industry,
            ),
            locked_strategy: locked.map(|r| r as u32),
            locked_strategy_name: locked.map(|r| {
                let template = &dataset.recipes[r as usize];
                if template.name.is_empty() {
                    template.id.clone()
                } else {
                    template.name.clone()
                }
            }),
        }
    }

    /// Rival divisions the firm can raid: cells where both operate and the rival has at least
    /// two blocks, with a two-to-one campaign suggested.
    pub fn raid_targets(&self, firm: u64) -> Vec<RaidOptionDto> {
        use cw_sim::economy::sectors;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = cw_core::ids::Handle(firm);
        let Some(entity) = world.firms.get(handle) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for d in sectors::divisions_of(world, dataset, handle) {
            let (firms, _) = sectors::cell(world, dataset, d.region, d.industry);
            for (rival, _) in firms {
                if rival == handle || cw_sim::economy::ownership::same_group(world, rival, handle) {
                    continue;
                }
                let rival_blocks: u32 =
                    sectors::division_plants(world, dataset, rival, d.region, d.industry)
                        .iter()
                        .filter_map(|h| world.facilities.get(*h))
                        .map(|p| p.blocks as u32)
                        .sum();
                if rival_blocks < 2 {
                    continue;
                }
                let probe =
                    sectors::raid_quote(world, dataset, handle, rival, d.region, d.industry, 0);
                let budget = probe.defence_cents * 2;
                let quote = sectors::raid_quote(
                    world, dataset, handle, rival, d.region, d.industry, budget,
                );
                out.push(RaidOptionDto {
                    target: rival.0,
                    target_name: world
                        .firms
                        .get(rival)
                        .map(|f| f.name.clone())
                        .unwrap_or_default(),
                    region: dataset.regions[d.region as usize].code.clone(),
                    region_index: d.region as u32,
                    industry: cw_sim::state::INDUSTRY_NAMES[d.industry as usize].to_owned(),
                    industry_index: d.industry as u32,
                    rival_blocks,
                    ready_week: entity
                        .raid_ready
                        .get(&(d.region, d.industry))
                        .copied()
                        .unwrap_or(0),
                    suggested_budget_cents: budget,
                    quote: raid_quote_dto(quote),
                });
            }
        }
        out
    }

    /// The odds and stake of a raid at a chosen budget.
    pub fn raid_quote(
        &self,
        firm: u64,
        target: u64,
        region: u32,
        industry: u32,
        budget_cents: i64,
    ) -> RaidQuoteDto {
        let session = self.lock();
        raid_quote_dto(cw_sim::economy::sectors::raid_quote(
            &session.world,
            &session.dataset,
            cw_core::ids::Handle(firm),
            cw_core::ids::Handle(target),
            region as u8,
            industry as u8,
            budget_cents,
        ))
    }

    /// The market-balance snapshot as JSON, for anonymous gameplay analytics.
    pub fn market_snapshot_json(&self) -> String {
        serde_json::to_string(&self.lock().market_snapshot()).unwrap_or_default()
    }

    /// The most recent market raids anywhere, newest first.
    pub fn recent_raids(&self) -> Vec<RaidRecordDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let name = |h: cw_core::ids::Handle| {
            world
                .firms
                .get(h)
                .map(|f| f.name.clone())
                .unwrap_or_default()
        };
        world
            .raids
            .iter()
            .rev()
            .map(|r| RaidRecordDto {
                week: r.tick,
                attacker: name(r.attacker),
                defender: name(r.defender),
                region: dataset.regions[r.region as usize].code.clone(),
                industry: cw_sim::state::INDUSTRY_NAMES[r.industry as usize].to_owned(),
                success: r.success,
                blocks: r.blocks as u32,
                player_attacker: cw_sim::economy::events_engine::player_side(world, r.attacker),
                player_defender: cw_sim::economy::events_engine::player_side(world, r.defender),
            })
            .collect()
    }

    /// Every plant in the world, for the map.
    pub fn map_plants(&self) -> Vec<MapPlantDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        world
            .facilities
            .iter()
            .filter_map(|(handle, plant)| {
                let firm = world.firms.get(plant.firm)?;
                if !firm.operative() {
                    return None;
                }
                let recipe = &dataset.recipes[plant.recipe as usize];
                let capacity = recipe.weekly_output_lots * plant.blocks as i64;
                Some(MapPlantDto {
                    handle: handle.0,
                    firm: plant.firm.0,
                    firm_name: firm.name.clone(),
                    player: cw_sim::economy::events_engine::player_side(world, plant.firm),
                    region: dataset.regions[plant.region as usize].code.clone(),
                    industry: recipe.industry.clone(),
                    strategy: if recipe.name.is_empty() {
                        recipe.id.clone()
                    } else {
                        recipe.name.clone()
                    },
                    blocks: plant.blocks as u32,
                    status: format!("{:?}", plant.status)
                        .split(' ')
                        .next()
                        .unwrap_or("")
                        .to_lowercase(),
                    utilisation_bps: if capacity > 0 {
                        (plant.last_output.lots() * 10_000 / capacity).clamp(0, 10_000) as i32
                    } else {
                        0
                    },
                })
            })
            .collect()
    }

    /// A firm's sector focus and logistics position.
    pub fn firm_ops(&self, firm: u64) -> Option<FirmOpsDto> {
        let session = self.lock();
        let world = &session.world;
        let entity = world.firms.get(cw_core::ids::Handle(firm))?;
        let names = cw_sim::state::INDUSTRY_NAMES;
        let (threshold, _) = cw_sim::economy::operations::sprawl_terms(
            entity.logistics_strength,
            entity.secondary_industry.is_some(),
        );
        Some(FirmOpsDto {
            firm,
            primary: names
                .get(entity.industry as usize)
                .copied()
                .unwrap_or("")
                .to_owned(),
            secondary: entity
                .secondary_industry
                .and_then(|s| names.get(s as usize).copied())
                .map(str::to_owned),
            focus_transition_weeks: entity.focus_transition_until.saturating_sub(world.tick.0),
            focus_cooldown_weeks: entity.focus_cooldown_until.saturating_sub(world.tick.0),
            logistics_strength: entity.logistics_strength,
            logistics_budget_cents: entity.policies.logistics_budget_cents,
            plants: entity.facilities.len() as u32,
            divisions: cw_sim::economy::sectors::divisions_of(
                world,
                &session.dataset,
                cw_core::ids::Handle(firm),
            )
            .len() as u32,
            sprawl_threshold: threshold as u32,
        })
    }

    pub fn finance(&self, firm: u64) -> Option<FinanceDto> {
        match self.lock().query(Query::Finance(firm)) {
            cw_session::Detail::Finance(row) => Some(finance_dto(row)),
            _ => None,
        }
    }

    pub fn ownership(&self, firm: u64) -> Option<OwnershipDto> {
        match self.lock().query(Query::Ownership(firm)) {
            cw_session::Detail::Ownership(row) => Some(ownership_dto(row)),
            _ => None,
        }
    }

    pub fn contracts(&self, firm: u64) -> Vec<ContractDto> {
        match self.lock().query(Query::Contracts(firm)) {
            cw_session::Detail::Contracts(rows) => rows.into_iter().map(contract_dto).collect(),
            _ => Vec::new(),
        }
    }

    pub fn loans(&self, firm: u64) -> Vec<LoanDto> {
        match self.lock().query(Query::Loans(firm)) {
            cw_session::Detail::Loans(rows) => rows.into_iter().map(loan_dto).collect(),
            _ => Vec::new(),
        }
    }

    pub fn insurance(&self, firm: u64) -> Vec<InsuranceDto> {
        match self.lock().query(Query::Insurance(firm)) {
            cw_session::Detail::Insurance(rows) => rows.into_iter().map(insurance_dto).collect(),
            _ => Vec::new(),
        }
    }

    pub fn market(&self, region: u32, product: u32) -> Option<MarketDto> {
        match self
            .lock()
            .query(Query::Market(region as u8, product as u16))
        {
            cw_session::Detail::Market(row) => Some(MarketDto {
                product: row.product,
                product_id: row.product_id,
                reference_price_cents: row.reference_price_cents,
                last_price_cents: row.last_price_cents,
                regional_stock_lots: row.regional_stock_lots,
                last_week_sales_lots: row.last_week_sales_lots,
                gateway_buy_cents: row.gateway_buy_cents,
                gateway_sell_cents: row.gateway_sell_cents,
            }),
            _ => None,
        }
    }

    pub fn deals(&self) -> Vec<DealDto> {
        match self.lock().query(Query::Deals) {
            cw_session::Detail::Deals(rows) => rows.into_iter().map(deal_dto).collect(),
            _ => Vec::new(),
        }
    }

    pub fn events(&self) -> Vec<EventDto> {
        match self.lock().query(Query::Events) {
            cw_session::Detail::Events(rows) => rows.into_iter().map(event_dto).collect(),
            _ => Vec::new(),
        }
    }

    /// Submit any catalogue command as JSON for the investor actor.
    pub fn submit_json(&self, json: String) -> ReceiptDto {
        self.submit_json_as(cw_sim::commands::INVESTOR_ACTOR, json)
    }

    /// Submit any catalogue command as JSON for an explicit legal actor.
    ///
    /// The investor is `0`; a firm handle lets a parent firm operate a controlled subsidiary
    /// through the same authority service the simulation already uses.
    pub fn submit_json_as(&self, actor: u64, json: String) -> ReceiptDto {
        let intent: cw_sim::commands::CommandIntent = match serde_json::from_str(&json) {
            Ok(intent) => intent,
            Err(error) => {
                return ReceiptDto {
                    accepted: false,
                    sequence: 0,
                    revision: 0,
                    error_key: Some("error.malformed_command".to_owned()),
                    error_detail: Some(error.to_string()),
                };
            }
        };
        self.submit_intent(actor, intent)
    }

    /// Set the selected firm's asking price for one product.
    pub fn set_price(&self, firm: u64, product: u32, price_cents: i64) -> ReceiptDto {
        self.submit_intent(
            cw_sim::commands::INVESTOR_ACTOR,
            cw_sim::commands::CommandIntent::SetPrice {
                firm: cw_core::ids::Handle(firm),
                product: product as u16,
                price_cents,
            },
        )
    }

    /// Set the weekly wage offer for one skill tier.
    pub fn set_wage_offer(&self, firm: u64, tier: u32, wage_cents: i64) -> ReceiptDto {
        self.submit_intent(
            cw_sim::commands::INVESTOR_ACTOR,
            cw_sim::commands::CommandIntent::SetWageOffer {
                firm: cw_core::ids::Handle(firm),
                tier: tier as u8,
                wage_cents,
            },
        )
    }

    /// Set the production target across the firm's operating facilities.
    pub fn set_production_target(&self, firm: u64, target_bps: i32) -> ReceiptDto {
        self.submit_intent(
            cw_sim::commands::INVESTOR_ACTOR,
            cw_sim::commands::CommandIntent::SetProductionTarget {
                firm: cw_core::ids::Handle(firm),
                target_bps,
            },
        )
    }

    /// Build a facility from a template index in the published product list.
    pub fn build_facility(&self, firm: u64, region: u32, recipe: u32, blocks: u32) -> ReceiptDto {
        self.submit_intent(
            cw_sim::commands::INVESTOR_ACTOR,
            cw_sim::commands::CommandIntent::BuildFacility {
                firm: cw_core::ids::Handle(firm),
                region: region as u8,
                recipe: recipe as u16,
                blocks: blocks as u8,
            },
        )
    }

    /// Pay a dividend from the firm's distributable retained earnings.
    pub fn pay_dividend(&self, firm: u64, amount_cents: i64) -> ReceiptDto {
        self.submit_intent(
            cw_sim::commands::INVESTOR_ACTOR,
            cw_sim::commands::CommandIntent::PayDividend {
                firm: cw_core::ids::Handle(firm),
                amount_cents,
            },
        )
    }

    /// The investor's controlled group as a breadth-first tree with control percentages.
    pub fn group_tree(&self) -> Vec<GroupNodeDto> {
        use std::collections::{BTreeSet, VecDeque};

        use cw_sim::economy::ownership;
        use cw_sim::state::HolderId;

        let session = self.lock();
        let world = &session.world;
        let mut seen: BTreeSet<cw_core::ids::Handle> = BTreeSet::new();
        let mut queue: VecDeque<(cw_core::ids::Handle, Option<u64>, u32)> = VecDeque::new();
        for root in ownership::controlled_by(world, HolderId::Investor) {
            if seen.insert(root) {
                queue.push_back((root, None, 0));
            }
        }
        let mut nodes = Vec::new();
        while let Some((handle, parent, depth)) = queue.pop_front() {
            let Some(firm) = world.firms.get(handle) else {
                continue;
            };
            let control_bps = match parent {
                Some(parent_handle) => world
                    .firms
                    .get(handle)
                    .map(|entity| {
                        let eligible = entity.shares.eligible_votes().max(1);
                        (entity
                            .shares
                            .qty(HolderId::Firm(cw_core::ids::Handle(parent_handle)))
                            as i128
                            * 10_000
                            / eligible as i128) as i32
                    })
                    .unwrap_or(0),
                None => firm.shares.vote_bps(HolderId::Investor),
            };
            nodes.push(GroupNodeDto {
                handle: handle.0,
                name: firm.name.clone(),
                region: world
                    .regions
                    .get(firm.region as usize)
                    .map(|region| region.name.clone())
                    .unwrap_or_default(),
                industry: cw_sim::state::INDUSTRY_NAMES
                    .get(firm.industry as usize)
                    .copied()
                    .unwrap_or("Firm")
                    .to_owned(),
                parent,
                depth,
                control_bps,
                investor_votes_bps: firm.shares.vote_bps(HolderId::Investor),
                look_through_bps: ownership::investor_look_through_bps(world, handle) as i32,
                cash_cents: firm.book.cash(),
                status: if firm.operative() {
                    "operating".to_owned()
                } else {
                    "distressed".to_owned()
                },
                directly_controlled: firm.shares.controller() == Some(HolderId::Investor),
            });
            for child in ownership::controlled_by(world, HolderId::Firm(handle)) {
                if seen.insert(child) {
                    queue.push_back((child, Some(handle.0), depth + 1));
                }
            }
        }
        nodes
    }

    /// Preview any catalogue intent from its JSON shape without reserving anything.
    pub fn preview_json(&self, json: String) -> PreviewDto {
        let intent: cw_sim::commands::CommandIntent = match serde_json::from_str(&json) {
            Ok(intent) => intent,
            Err(error) => {
                return PreviewDto {
                    revision: 0,
                    cash_change_cents: 0,
                    assumptions: Vec::new(),
                    warnings: Vec::new(),
                    blocked_key: Some("error.malformed_command".to_owned()),
                    blocked_detail: Some(error.to_string()),
                };
            }
        };
        let session = self.lock();
        preview_dto(session.preview(cw_sim::commands::INVESTOR_ACTOR, &intent))
    }

    /// Typed dilution preview used by the issuance sheet.
    pub fn preview_issue(&self, firm: u64, qty: i64, price_cents: i64) -> PreviewDto {
        let session = self.lock();
        preview_dto(session.preview(
            cw_sim::commands::INVESTOR_ACTOR,
            &cw_sim::commands::CommandIntent::IssueShares {
                firm: cw_core::ids::Handle(firm),
                qty,
                price_cents,
                buyer_outside: 0,
            },
        ))
    }

    /// Typed build preview used by the facility and founder sheets.
    pub fn preview_build(&self, firm: u64, region: u32, recipe: u32, blocks: u32) -> PreviewDto {
        let session = self.lock();
        preview_dto(session.preview(
            cw_sim::commands::INVESTOR_ACTOR,
            &cw_sim::commands::CommandIntent::BuildFacility {
                firm: cw_core::ids::Handle(firm),
                region: region as u8,
                recipe: recipe as u16,
                blocks: blocks as u8,
            },
        ))
    }

    /// Current worker bargaining and strike state for one firm.
    pub fn labour(&self, firm: u64) -> LabourDto {
        use cw_sim::state::BargainState;

        let session = self.lock();
        let Some(entity) = session.world.firms.get(cw_core::ids::Handle(firm)) else {
            return LabourDto {
                firm,
                state: "idle".to_owned(),
                demand_deadline: None,
                strike_until: None,
                organisation_bps: 0,
                ballot_support_bps: 0,
                demand_cents: 0,
                last_offer_cents: 0,
                settlement_cost_cents: 0,
                current_wage_cents: 0,
                employees: 0,
            };
        };
        let (state, demand_deadline, strike_until) = match entity.union.state {
            BargainState::Idle => ("idle", None, None),
            BargainState::Ballot => ("ballot", None, None),
            BargainState::Demanding { deadline } => ("demanding", Some(deadline.0), None),
            BargainState::Striking { until, .. } => ("striking", None, Some(until.0)),
        };
        LabourDto {
            firm,
            state: state.to_owned(),
            demand_deadline,
            strike_until,
            organisation_bps: entity.union.organisation_bps,
            ballot_support_bps: entity.union.ballot_support_bps,
            demand_cents: entity.union.demand_cents,
            last_offer_cents: entity.union.last_offer_cents,
            settlement_cost_cents: entity.union.settlement_cost_cents,
            current_wage_cents: entity.policies.wage_offer[0],
            employees: entity.total_employees(&session.world.facilities),
        }
    }

    /// Lobbying fee, history and per-region cooldown for one firm.
    pub fn policy_state(&self, firm: u64) -> PolicyStateDto {
        let session = self.lock();
        let world = &session.world;
        let Some(entity) = world.firms.get(cw_core::ids::Handle(firm)) else {
            return PolicyStateDto {
                firm,
                fee_cents: 0,
                reputation_bps: 0,
                requests: Vec::new(),
                cooldowns: Vec::new(),
            };
        };
        let requests: Vec<PolicyRequestDto> = entity
            .policy_requests
            .iter()
            .map(|request| PolicyRequestDto {
                kind: policy_kind_key(request.kind).to_owned(),
                region_name: world
                    .regions
                    .get(request.region as usize)
                    .map(|region| region.name.clone())
                    .unwrap_or_default(),
                fee_cents: request.fee_cents,
                decision_week: request.decision.0,
                resolved: request.resolved,
                success: request.success,
                cooldown_until: request.cooldown_until.0,
            })
            .collect();
        let cooldowns: Vec<RegionCooldownDto> = world
            .regions
            .iter()
            .enumerate()
            .map(|(index, region)| {
                let mut available_week = world.tick.0;
                let mut active = false;
                for request in &entity.policy_requests {
                    if request.region != index as u8 {
                        continue;
                    }
                    if !request.resolved {
                        active = true;
                    }
                    if request.cooldown_until.0 > available_week {
                        available_week = request.cooldown_until.0;
                    }
                }
                RegionCooldownDto {
                    region: index as u32,
                    name: region.name.clone(),
                    available_week,
                    active,
                }
            })
            .collect();
        let fee_cents = cw_sim::economy::policy::request_fee(world, cw_core::ids::Handle(firm));
        PolicyStateDto {
            firm,
            fee_cents,
            reputation_bps: entity.reputation_bps,
            requests,
            cooldowns,
        }
    }

    /// Every bank and insurer in the world with its own liquidity and capital test result.
    pub fn institutions(&self) -> Vec<InstitutionDto> {
        let session = self.lock();
        let world = &session.world;
        let mut rows = Vec::new();
        for (handle, firm) in world.firms.iter() {
            if firm.bank.is_none() && firm.insurer.is_none() {
                continue;
            }
            let deposits = cw_sim::economy::banking::deposits(world, handle);
            let loans = cw_sim::economy::banking::loan_assets(world, handle);
            let coverage: i64 = world
                .insurance
                .iter()
                .filter(|(_, policy)| {
                    policy.insurer == handle && policy.status == cw_sim::state::PolicyStatus::Active
                })
                .map(|(_, policy)| policy.limit_cents)
                .sum();
            let cash = firm.book.cash();
            let equity = firm.book.total_equity();
            let specialist = firm
                .insurer
                .as_ref()
                .map(|model| model.specialist_concentration)
                .or_else(|| firm.bank.as_ref().map(|_| false))
                .unwrap_or(false);
            let kind = match (firm.bank.is_some(), firm.insurer.is_some()) {
                (true, true) => "bank_insurer",
                (true, false) => "bank",
                (false, true) => "insurer",
                (false, false) => "institution",
            };
            let (liquidity_ok, capital_ok) = if firm.insurer.is_some() {
                let overhead =
                    2 * cw_sim::economy::insolvency::mandatory_weekly_outflow(world, handle);
                let capital_need = coverage * 10 / 100;
                let capital_need = if specialist {
                    capital_need * 2
                } else {
                    capital_need
                };
                (
                    cash >= overhead + coverage * 5 / 100,
                    equity >= capital_need,
                )
            } else {
                (
                    cash >= deposits * 2_000 / 10_000,
                    equity * 10_000 >= loans * 1_200,
                )
            };
            rows.push(InstitutionDto {
                handle: handle.0,
                name: firm.name.clone(),
                region: world
                    .regions
                    .get(firm.region as usize)
                    .map(|region| region.name.clone())
                    .unwrap_or_default(),
                kind: kind.to_owned(),
                controlled: firm.shares.controller() == Some(cw_sim::state::HolderId::Investor),
                deposit_rate_bps: firm
                    .bank
                    .as_ref()
                    .map(|model| model.deposit_rate_bps)
                    .unwrap_or(0),
                cash_cents: cash,
                equity_cents: equity,
                deposits_cents: deposits,
                loans_cents: loans,
                coverage_cents: coverage,
                specialist,
                liquidity_ok,
                capital_ok,
            });
        }
        rows
    }

    /// Whether the recovery management contract is available, and why it is not.
    pub fn recovery(&self) -> RecoveryDto {
        let session = self.lock();
        let world = &session.world;
        let destitute = world.investor.cash_cents < 2_500_000
            && world.investor_firms().is_empty()
            && world.investor.managed_firms.is_empty();
        let next_eligible_week = if world.investor.recovery_used > 0 {
            world.investor.last_recovery_week + 260
        } else {
            0
        };
        let candidate = world
            .firms
            .iter()
            .filter(|(_, firm)| firm.operative() && firm.shares.controller().is_none())
            .map(|(_, firm)| firm.name.clone())
            .next();
        let reason_key = if !destitute {
            "recovery.not_destitute"
        } else if next_eligible_week > world.tick.0 {
            "recovery.cooldown"
        } else if candidate.is_none() {
            "recovery.no_target"
        } else {
            "recovery.available"
        };
        RecoveryDto {
            eligible: reason_key == "recovery.available",
            reason_key: reason_key.to_owned(),
            cash_cents: world.investor.cash_cents,
            next_eligible_week,
            signing_bonus_cents: 10_000_000,
            managed_firms: world.investor.managed_firms.len() as u32,
            candidate,
            formation_fee_cents: cw_data::Difficulty::Standard.formation_fee_cents(),
            minimum_capital_cents: 25_000_000,
        }
    }

    /// Outside investors that can fund a primary issuance.
    pub fn funding_buyers(&self) -> Vec<BuyerDto> {
        let session = self.lock();
        session
            .world
            .outside
            .investors
            .iter()
            .enumerate()
            .map(|(index, investor)| BuyerDto {
                index: index as u32,
                name: investor.name.clone(),
                capital_cents: investor.capital_cents,
            })
            .collect()
    }

    /// Pending board votes so the phone can cast a real vote id.
    pub fn board_votes(&self, firm: u64) -> Vec<BoardVoteDto> {
        let session = self.lock();
        session
            .world
            .firms
            .get(cw_core::ids::Handle(firm))
            .map(|entity| {
                entity
                    .board
                    .pending_votes
                    .iter()
                    .map(|vote| BoardVoteDto {
                        id: vote.id,
                        subject: vote.subject.clone(),
                        deadline: vote.deadline.0,
                        yes: vote.yes as u32,
                        no: vote.no as u32,
                        resolved: vote.resolved,
                        passed: vote.passed,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Quote a property policy without binding it.
    pub fn insurance_quote(&self, insurer: u64, facility: u64, limit_cents: i64) -> QuoteDto {
        let session = self.lock();
        let world = &session.world;
        let Some(region) = world
            .facilities
            .get(cw_core::ids::Handle(facility))
            .map(|entity| entity.region)
        else {
            return QuoteDto {
                weekly_premium_cents: 0,
                blocked_key: Some("error.invalid_entity".to_owned()),
                blocked_detail: Some("facility".to_owned()),
            };
        };
        match cw_sim::economy::insurance::quote(
            world,
            cw_core::ids::Handle(insurer),
            region,
            limit_cents,
        ) {
            Ok(premium) => QuoteDto {
                weekly_premium_cents: premium,
                blocked_key: None,
                blocked_detail: None,
            },
            Err(error) => QuoteDto {
                weekly_premium_cents: 0,
                blocked_key: Some(error.key().to_owned()),
                blocked_detail: Some(format!("{error:?}")),
            },
        }
    }
}

impl SessionHandle {
    fn submit_intent(&self, actor: u64, intent: cw_sim::commands::CommandIntent) -> ReceiptDto {
        let mut session = self.lock();
        let revision = session.revision();
        let envelope = cw_sim::commands::CommandEnvelope {
            command_id_high: 0x5357_4946_5400_0000,
            command_id_low: revision,
            actor,
            expected_revision: revision,
            submitted_tick: session.world.tick.0,
            intent,
        };
        match session.submit(envelope) {
            Ok(receipt) => ReceiptDto {
                accepted: true,
                sequence: receipt.sequence,
                revision: receipt.revision,
                error_key: None,
                error_detail: None,
            },
            Err(error) => ReceiptDto {
                accepted: false,
                sequence: 0,
                revision,
                error_key: Some(error.key().to_owned()),
                error_detail: Some(format!("{error:?}")),
            },
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Session> {
        match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_session_round_trips_through_the_boundary() {
        let session = SessionHandle::new(181, true, true).unwrap();
        let overview = session.overview();
        assert!(overview.investor_cash_cents > 0);
        let advance = session.advance(2);
        assert_eq!(advance.weeks_advanced, 2);
        let firms = session.firms();
        assert!(firms.len() >= 31);
        let bytes = session.export_save().unwrap();
        let imported = import_save(bytes).unwrap();
        assert_eq!(session.canonical_hash(), imported.canonical_hash());
    }

    #[test]
    fn the_score_is_exposed_after_week_260() {
        let session = SessionHandle::new(203, true, true).unwrap();
        assert!(session.score().is_none(), "no score before the horizon");
        let mut week = 0;
        while week < 260 {
            let advance = session.advance(16);
            week = advance.final_week;
            if advance.pause.as_deref() == Some("five_year_score") {
                break;
            }
            if advance.pause.is_some() {
                // Answer waiting cards as the app's inbox would, then clear the pause.
                for event in session.events().into_iter().filter(|event| event.mandatory) {
                    if let Some(choice) = event.choices.first() {
                        let json = format!(
                            r#"{{"RespondEvent":{{"event":{},"choice":"{}"}}}}"#,
                            event.id, choice.id
                        );
                        session.submit_json(json);
                    }
                }
                session.dismiss_pause();
            }
        }
        let score = session.score().expect("score at week 260");
        assert_eq!(score.week, 260);
        assert!((0..=100).contains(&score.total));
        let weighted = (score.return_component as f64 * 0.45
            + score.expansion_component as f64 * 0.25
            + score.leadership_component as f64 * 0.1
            + score.resilience_component as f64 * 0.1
            + score.conduct_component as f64 * 0.1)
            .round() as i32;
        assert!(
            (weighted - score.total).abs() <= 2,
            "total follows the published weights"
        );
    }

    #[test]
    fn a_rejected_command_returns_a_typed_key() {
        let session = SessionHandle::new(182, false, false).unwrap();
        let receipt = session.set_price(0xDEAD_BEEF, 0, 1_000);
        assert!(!receipt.accepted);
        assert!(receipt.error_key.is_some());
    }

    #[test]
    fn typed_price_change_is_accepted_for_the_investor_firm() {
        let session = SessionHandle::new(183, true, false).unwrap();
        let primary = session.overview().primary_firm.unwrap();
        let dataset = cw_data::Dataset::load().unwrap();
        let price = dataset.reference_price(0);
        let receipt = session.set_price(primary, 0, price);
        assert!(receipt.accepted, "{:?}", receipt.error_detail);
    }

    #[test]
    fn the_group_tree_starts_at_a_directly_controlled_firm() {
        let session = SessionHandle::new(184, true, false).unwrap();
        let tree = session.group_tree();
        assert!(!tree.is_empty());
        assert!(
            tree.iter()
                .any(|node| node.depth == 0 && node.directly_controlled)
        );
        assert!(tree.iter().all(|node| node.depth < 16));
    }

    #[test]
    fn the_issuance_preview_reports_dilution_without_mutating() {
        let session = SessionHandle::new(185, true, false).unwrap();
        let primary = session.overview().primary_firm.unwrap();
        let before = session.revision();
        let preview = session.preview_issue(primary, 1_000_000, 100);
        assert_eq!(preview.blocked_key, None);
        assert!(preview.cash_change_cents > 0);
        assert!(
            preview
                .assumptions
                .iter()
                .any(|line| line.contains("votes"))
        );
        assert_eq!(session.revision(), before);
    }

    #[test]
    fn preview_json_parses_the_phone_shapes() {
        let session = SessionHandle::new(186, true, false).unwrap();
        let primary = session.overview().primary_firm.unwrap();
        let json = format!(
            "{{\"IssueShares\":{{\"firm\":{primary},\"qty\":1000,\"price_cents\":250,\"buyer_outside\":0}}}}"
        );
        let preview = session.preview_json(json);
        assert_eq!(preview.blocked_key, None);
        assert_eq!(preview.cash_change_cents, 250_000);
        let malformed = session.preview_json("{\"Nope\":{}}".to_owned());
        assert_eq!(
            malformed.blocked_key.as_deref(),
            Some("error.malformed_command")
        );
    }

    #[test]
    fn labour_policy_and_recovery_read_models_are_populated() {
        let session = SessionHandle::new(187, true, false).unwrap();
        let primary = session.overview().primary_firm.unwrap();
        let labour = session.labour(primary);
        assert_eq!(labour.firm, primary);
        assert!(!labour.state.is_empty());
        let policy = session.policy_state(primary);
        assert!(policy.fee_cents >= 2_500_000);
        assert_eq!(policy.cooldowns.len(), session.regions(None).len());
        let recovery = session.recovery();
        assert!(!recovery.reason_key.is_empty());
        assert!(!recipes().is_empty());
        assert!(!session.institutions().is_empty());
        assert!(!session.funding_buyers().is_empty());
    }

    #[test]
    fn every_phone_command_shape_parses_as_an_intent() {
        let session = SessionHandle::new(188, true, false).unwrap();
        let primary = session.overview().primary_firm.unwrap();
        let facility = session
            .facilities(primary)
            .first()
            .map(|row| row.handle)
            .unwrap_or(1);
        let insurer = session
            .institutions()
            .iter()
            .find(|row| row.kind.contains("insurer"))
            .map(|row| row.handle)
            .unwrap_or(1);
        let bank = session
            .institutions()
            .iter()
            .find(|row| row.kind.contains("bank"))
            .map(|row| row.handle)
            .unwrap_or(1);
        let bodies = vec![
            format!(r#"{{"SetPrimaryGroup":{{"firm":{primary}}}}}"#),
            format!(r#"{{"CallMeeting":{{"firm":{primary}}}}}"#),
            format!(r#"{{"NominateCEO":{{"firm":{primary},"investor":true}}}}"#),
            format!(r#"{{"CastVote":{{"firm":{primary},"vote_id":1,"approve":true}}}}"#),
            format!(
                r#"{{"ProposeBoardAction":{{"firm":{primary},"subject":"merger_authority"}}}}"#
            ),
            format!(
                r#"{{"StartDiligence":{{"buyer":{primary},"target":{primary},"kind":"SharePurchase","price_cents":100,"max_shares":1000}}}}"#
            ),
            r#"{"FundTender":{"deal":1}}"#.to_owned(),
            r#"{"RespondTender":{"deal":1,"accept":true}}"#.to_owned(),
            r#"{"WithdrawOffer":{"deal":1}}"#.to_owned(),
            r#"{"ProposeMerger":{"deal":1}}"#.to_owned(),
            r#"{"SubmitRemedy":{"deal":1,"divest_share_bps":500}}"#.to_owned(),
            format!(
                r#"{{"PlaceShareOrder":{{"investor":true,"firm":{primary},"buy":true,"qty":10,"limit_price_cents":100}}}}"#
            ),
            format!(
                r#"{{"IssueShares":{{"firm":{primary},"qty":1000,"price_cents":100,"buyer_outside":0}}}}"#
            ),
            format!(r#"{{"RepurchaseShares":{{"firm":{primary},"qty":10}}}}"#),
            format!(r#"{{"RetireTreasury":{{"firm":{primary},"qty":10}}}}"#),
            format!(r#"{{"PayDividend":{{"firm":{primary},"amount_cents":100}}}}"#),
            format!(r#"{{"LaunchIPO":{{"firm":{primary},"new_shares":1000,"price_cents":100}}}}"#),
            format!(r#"{{"RequestLoan":{{"firm":{primary},"amount_cents":100}}}}"#),
            format!(
                r#"{{"IssueBond":{{"firm":{primary},"principal_cents":100,"coupon_bps":700,"weeks":104}}}}"#
            ),
            format!(r#"{{"DepositFunds":{{"bank":{bank},"firm":{primary},"amount_cents":100}}}}"#),
            format!(r#"{{"WithdrawFunds":{{"bank":{bank},"firm":{primary},"amount_cents":100}}}}"#),
            format!(
                r#"{{"ApproveLoan":{{"bank":{bank},"borrower":{primary},"amount_cents":100}}}}"#
            ),
            format!(
                r#"{{"QuotePolicy":{{"insurer":{insurer},"facility":{facility},"limit_cents":100,"deductible_cents":10}}}}"#
            ),
            format!(
                r#"{{"BindPolicy":{{"insurer":{insurer},"insured":{primary},"facility":{facility},"limit_cents":100,"deductible_cents":10}}}}"#
            ),
            r#"{"CancelPolicy":{"policy":1}}"#.to_owned(),
            format!(r#"{{"SetUnderwritingLimit":{{"insurer":{insurer},"specialist":true}}}}"#),
            format!(
                r#"{{"RespondBargaining":{{"firm":{primary},"choice":"Counter","offer_cents":100}}}}"#
            ),
            format!(r#"{{"ProposeSettlement":{{"firm":{primary},"amount_cents":100}}}}"#),
            format!(
                r#"{{"FileLobbyRequest":{{"firm":{primary},"region":0,"request":"TrainingGrant"}}}}"#
            ),
            r#""AcceptManagementContract""#.to_owned(),
            format!(
                r#"{{"FoundFirm":{{"name":"Test","region":0,"capital_cents":25000000,"recipe":0}}}}"#
            ),
            format!(r#"{{"SetPrice":{{"firm":{primary},"product":0,"price_cents":100}}}}"#),
            format!(r#"{{"SetWageOffer":{{"firm":{primary},"tier":0,"wage_cents":100}}}}"#),
            format!(r#"{{"SetProductionTarget":{{"firm":{primary},"target_bps":9000}}}}"#),
            format!(r#"{{"BuildFacility":{{"firm":{primary},"region":0,"recipe":0,"blocks":1}}}}"#),
            r#"{"RespondEvent":{"event":1,"choice":"x"}}"#.to_owned(),
        ];
        for body in bodies {
            let preview = session.preview_json(body.clone());
            assert_ne!(
                preview.blocked_key.as_deref(),
                Some("error.malformed_command"),
                "{body}"
            );
        }
    }

    #[test]
    fn the_map_and_strategy_surfaces_cover_the_world() {
        let session = SessionHandle::new(211, true, false).unwrap();
        let primary = session.overview().primary_firm;
        let boards = session.region_boards(primary);
        assert_eq!(boards.len(), 31);
        assert!(boards.iter().all(|b| b.population_2000 > 200_000
            && b.code.starts_with(&b.state_code)
            && b.specialization.len() == 2
            && !b.state_name.is_empty()));
        // The player's farm shows as its own agriculture cell on its region's board.
        let firm = primary.expect("a guided career has a company");
        let divisions = session.divisions(firm);
        assert_eq!(divisions.len(), 1);
        let home = boards
            .iter()
            .find(|b| b.code == divisions[0].region)
            .unwrap();
        let cell = home
            .sectors
            .iter()
            .find(|c| c.industry == divisions[0].industry)
            .expect("the player's sector cell");
        assert_eq!(cell.own_blocks, divisions[0].blocks);
        assert!(cell.own_share_bps > 0);
        let map = session.map_divisions();
        assert_eq!(map.len(), 1, "one owned sector circle");
        assert_eq!(map[0].plants, divisions[0].plants.len() as u32);
        // A second plant of the same strategy joins the division: no entry fee, strategy locked.
        let quote = session.build_quote(firm, home.index, divisions[0].strategy, 1);
        assert!(!quote.opens_division);
        assert_eq!(quote.entry_fee_cents, 0);
        assert_eq!(quote.locked_strategy, Some(divisions[0].strategy));
        // The same strategy elsewhere opens a division with the entry fee.
        let away = boards.iter().find(|b| b.code != home.code).unwrap();
        let quote = session.build_quote(firm, away.index, divisions[0].strategy, 1);
        assert!(quote.opens_division);
        assert_eq!(quote.entry_fee_cents, 10_000_000);
        assert_eq!(quote.total_cents, quote.capex_cents + quote.entry_fee_cents);
        assert!(session.recent_raids().is_empty());
        let plants = session.map_plants();
        // Thirty seeded rivals, of which the bank and the insurer run no plants, plus the
        // player's own plant.
        assert_eq!(
            plants.iter().filter(|p| !p.player).count(),
            28,
            "every rival plant is on the map"
        );
        assert!(
            plants.iter().any(|p| p.player),
            "the player's plant is marked"
        );
        let catalogue = strategies();
        assert_eq!(catalogue.len(), 53);
        assert!(
            catalogue
                .iter()
                .all(|s| !s.name.is_empty() && s.weekly_output_lots > 0)
        );
        let ops = session.firm_ops(firm).expect("ops for the player's firm");
        assert_eq!(ops.sprawl_threshold, 15);
    }
}
