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

mod cast;
mod catalogue;
mod chapters;
mod company;
mod custom;
mod letters;
mod paper;
mod statements;
mod systems;
pub use cast::{
    DossierDto, MoveDto, PersonDto, PostDto, RivalMoveFeedDto, RivalSummaryDto, RivalTagDto,
};
pub use catalogue::{ActionDto, FieldDto, OptionDto, action_catalogue};
pub use paper::{
    ClassifiedDto, GoodMoveDto, MarketsDto, PaperDto, PaperLinkDto, ShareMoveDto, StoryDto,
};
pub use statements::{DivisionPeriodDto, PricePointDto, StatementDto};

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
    /// Currency id of cash and equity; empty when the scenario has one money. Fair value and
    /// the share mark are quoted in the numeraire.
    pub currency: String,
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

/// One good on the corridor commodity board.
#[derive(uniffi::Record)]
pub struct CommodityDto {
    pub product: u32,
    pub product_id: String,
    pub name: String,
    pub sector: String,
    pub price_cents: i64,
    pub reference_cents: i64,
    pub change_week_bps: i64,
    pub change_quarter_bps: i64,
    pub history_cents: Vec<i64>,
    pub made_lots: i64,
    pub sold_lots: i64,
    pub imported_lots: i64,
    pub exported_lots: i64,
    pub stock_lots: i64,
    pub import_cents: i64,
    pub export_cents: i64,
    pub yours: bool,
}

/// One company on the corridor stock board.
#[derive(uniffi::Record)]
pub struct StockDto {
    pub firm: u64,
    pub name: String,
    pub industry: String,
    pub region: String,
    pub price_cents: i64,
    pub fair_cents: i64,
    pub change_week_bps: i64,
    pub change_quarter_bps: i64,
    pub history_cents: Vec<i64>,
    pub shares_issued: i64,
    pub market_cap_cents: i64,
    pub earnings_cents: i64,
    pub earnings_weeks: u32,
    pub your_shares: i64,
    pub float_bps: i64,
    pub controlled_by_you: bool,
}

/// One good the firm makes, for the pricing panel.
#[derive(uniffi::Record)]
pub struct ProductLineDto {
    pub product: u32,
    pub product_id: String,
    pub name: String,
    pub price_cents: i64,
    pub reference_cents: i64,
    pub market_cents: i64,
    pub unit_cost_cents: i64,
    pub made_last_week_lots: i64,
    pub sold_last_week_lots: i64,
    pub stock_lots: i64,
    pub mode: String,
    pub posture_bps: i32,
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
    pub cost_of_goods_cents: i64,
    pub wages_cents: i64,
    pub operating_expense_cents: i64,
    pub depreciation_cents: i64,
    pub net_interest_cents: i64,
    pub tax_cents: i64,
    pub ebit_cents: i64,
    pub net_income_cents: i64,
}

/// Four weeks of a firm's results with closing balances and share price.
#[derive(uniffi::Record)]
pub struct PeriodDto {
    pub first_week: u32,
    pub weeks: u32,
    pub revenue_cents: i64,
    pub cost_of_goods_cents: i64,
    pub wages_cents: i64,
    pub operating_expense_cents: i64,
    pub depreciation_cents: i64,
    pub net_interest_cents: i64,
    pub tax_cents: i64,
    pub other_cents: i64,
    pub net_income_cents: i64,
    pub cash_cents: i64,
    pub assets_cents: i64,
    pub liabilities_cents: i64,
    pub equity_cents: i64,
    pub share_price_cents: i64,
    pub shares_issued: i64,
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
    pub start_year: i32,
    pub revision: u64,
    /// Investor cash and net worth, in the reporting currency.
    pub investor_cash_cents: i64,
    pub investor_net_worth_cents: i64,
    /// Currency id the investor figures are in; empty when the scenario has one money.
    pub reporting_currency: String,
    /// Currency id of the primary company's figures; empty when the scenario has one money.
    pub primary_currency: String,
    pub primary_firm: Option<u64>,
    pub primary_name: String,
    pub firm_cash_cents: i64,
    pub forecast_four_week_cents: i64,
    pub last_week_ebit_cents: i64,
    pub quarter_net_income_cents: i64,
    pub quarter_weeks: u32,
    pub alerts: Vec<String>,
    pub pause: Option<String>,
    /// Why the clock is stopped and the one action that resumes it.
    pub pause_info: Option<PauseInfoDto>,
    pub next_action_key: String,
    pub next_action_reason: String,
}

/// A pause in plain words. `action` is `answer` (open the card `event_id`) or `continue`
/// (call `dismiss_pause`); `kind` is `decision`, `deadline`, `warning` or `milestone`.
#[derive(uniffi::Record)]
pub struct PauseInfoDto {
    pub reason: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub action: String,
    pub action_label: String,
    pub event_id: Option<u64>,
    pub firm: Option<u64>,
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
    /// The store a good needs: dry, tank, silo or cold. Empty for services and contracts.
    pub storage_class: String,
    /// Weekly holding cost beyond a plant's yard, basis points of the reference price.
    pub holding_bps_weekly: u32,
    /// Share of stock that spoils each week, basis points.
    pub spoilage_bps_weekly: u32,
}

/// A firm's use of one class of storage in one region.
#[derive(uniffi::Record)]
pub struct StorageCellDto {
    pub region: String,
    pub region_name: String,
    pub class_key: String,
    pub class_label: String,
    pub used_lots: i64,
    /// Working yard of the firm's plants plus owned storage facilities.
    pub capacity_lots: i64,
    pub owned_lots: i64,
    /// Lots in the region's outside storage, which pays a weekly fee per lot.
    pub outside_lots: i64,
    pub outside_fee_cents_weekly: i64,
}

/// Goods a firm holds back from sale, and any open buy order for them.
#[derive(uniffi::Record)]
pub struct HeldStockDto {
    pub product: u32,
    pub product_name: String,
    pub region: String,
    pub region_name: String,
    pub held_lots: i64,
    pub order_lots_left: i64,
    pub order_limit_bps: i32,
}

/// Storage results of a firm: costs over the last quarter and the last week's totals.
#[derive(uniffi::Record)]
pub struct StorageSummaryDto {
    pub used_lots: i64,
    pub capacity_lots: i64,
    pub outside_lots: i64,
    pub fee_cents_13w: i64,
    pub holding_cents_13w: i64,
    pub spoiled_cents_13w: i64,
    pub spoiled_lots_13w: i64,
    pub rent_cents_13w: i64,
}

/// One freight mode: its price, speed, smallest shipment and what it can carry.
#[derive(uniffi::Record)]
pub struct FreightModeDto {
    pub index: u32,
    pub id: String,
    pub name: String,
    pub rate_milli_cents_per_lot_km: i64,
    pub speed_km_week: u32,
    pub min_lots: i64,
    pub needs_terminal: bool,
    pub needs_infrastructure: bool,
    /// Storage class keys the mode may carry: dry, tank, silo, cold.
    pub classes: Vec<String>,
}

/// A firm's capacity share on a lane.
#[derive(uniffi::Record)]
pub struct LaneOwnerDto {
    pub firm: u64,
    pub name: String,
    pub lots: i32,
    pub mine: bool,
}

/// One lane this week: two regions in one mode, with its capacity, use and price.
#[derive(uniffi::Record)]
pub struct LaneDto {
    pub a: String,
    pub b: String,
    pub a_name: String,
    pub b_name: String,
    pub mode: u32,
    pub mode_id: String,
    pub mode_name: String,
    pub km: u32,
    pub capacity_lots: i32,
    pub used_lots: i32,
    /// Lots under lane contracts.
    pub held_lots: i32,
    /// Lots that overflowed to outside carriers.
    pub outside_lots: i32,
    pub fill_bps: i32,
    /// Spot price of one lot over the whole lane, and the outside carriers' price.
    pub rate_cents_per_lot: i64,
    pub outside_rate_cents_per_lot: i64,
    /// Weeks running the lane has been full or overflowing.
    pub hot_weeks: u32,
    pub owners: Vec<LaneOwnerDto>,
    pub contracts: u32,
    pub my_capacity_lots: i32,
    pub my_shipped_lots: i32,
    pub my_contract_lots: i32,
    /// Lots the player's group could reserve now.
    pub reservable_lots: i64,
}

/// A lane contract: reserved capacity at a fixed weekly rate.
#[derive(uniffi::Record)]
pub struct LaneContractDto {
    pub id: u32,
    pub holder: u64,
    pub holder_name: String,
    pub mine: bool,
    pub a: String,
    pub b: String,
    pub a_name: String,
    pub b_name: String,
    pub mode: u32,
    pub mode_name: String,
    pub lots: i32,
    pub held_lots: i32,
    pub used_lots: i32,
    pub fee_cents_per_lot: i64,
    pub weeks_left: u32,
}

/// The corridor's freight in one glance, for the week just closed.
#[derive(uniffi::Record)]
pub struct FreightSummaryDto {
    pub carriers: u32,
    pub lanes_with_capacity: u32,
    pub capacity_lots: i64,
    pub moved_lots: i64,
    pub outside_share_bps: i32,
    pub own_group_lots: i64,
    pub contract_lots: i64,
    pub paid_cents: i64,
    pub congested_lanes: u32,
}

/// One lane a division or firm ships on, for its Shipping section.
#[derive(uniffi::Record)]
pub struct ShippingLaneDto {
    pub a: String,
    pub b: String,
    pub a_name: String,
    pub b_name: String,
    pub mode: u32,
    pub mode_id: String,
    pub mode_name: String,
    pub lots: i64,
    pub cents: i64,
    /// Lots on this lane that went to outside carriers at a premium.
    pub outside_lots: i64,
    pub fill_bps: i32,
    pub congested: bool,
}

/// One shipping policy choice with its plain-English line and what it would have cost the
/// division's last week of shipments at today's prices.
#[derive(uniffi::Record)]
pub struct PolicyOptionDto {
    /// The policy kind (0 to 3), or the mode index with -1 meaning any.
    pub id: i32,
    pub label: String,
    pub blurb: String,
    pub selected: bool,
    pub has_estimate: bool,
    pub weekly_cents: i64,
    /// Change against the current choice in basis points; negative is cheaper.
    pub change_bps: i32,
    pub outside_lots: i64,
}

/// What the engine suggests doing about freight spend, with a quoted payback.
#[derive(uniffi::Record)]
pub struct ShippingSuggestionDto {
    /// build or reserve.
    pub kind: String,
    pub headline: String,
    pub a: String,
    pub b: String,
    pub mode: u32,
    /// The strategy (recipe) index to build, so the build dialog can open prefilled. Zero for a
    /// reservation.
    pub recipe: u32,
    pub lots: i64,
    pub upfront_cents: i64,
    pub weekly_saving_cents: i64,
    /// Zero when the suggestion has no payback (a reservation has no upfront cost).
    pub payback_weeks: i64,
}

/// Freight for the week just closed: for one division (goods arriving in its region) or for the
/// whole company with its lane contract fees.
#[derive(uniffi::Record)]
pub struct ShippingSummaryDto {
    pub has_data: bool,
    pub weekly_cents: i64,
    pub own_cents: i64,
    pub other_cents: i64,
    pub outside_cents: i64,
    pub fee_cents: i64,
    /// What outside carriers charged beyond what the same lots cost on own carriers.
    pub outside_extra_cents: i64,
    pub lots: i64,
    pub outside_lots: i64,
    pub cents_per_lot: i64,
    pub own_share_bps: i32,
    pub other_share_bps: i32,
    pub outside_share_bps: i32,
    pub policy_text: String,
    pub lanes: Vec<ShippingLaneDto>,
    /// Empty for the whole company: each division picks its own.
    pub kinds: Vec<PolicyOptionDto>,
    pub modes: Vec<PolicyOptionDto>,
    pub suggestion: Option<ShippingSuggestionDto>,
}

/// A lane reservation priced before it is made.
#[derive(uniffi::Record)]
pub struct ReserveQuoteDto {
    pub ok: bool,
    pub fee_cents_per_lot: i64,
    pub spot_cents_per_lot: i64,
    pub weekly_fee_cents: i64,
    pub total_cents: i64,
    pub expected_use_lots: i32,
    pub weekly_net_cents: i64,
    pub max_lots: i64,
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
    /// Set on a storage facility; its `weekly_output_lots` is the lots of space one plant holds.
    pub storage_class: String,
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
    /// Further products the plant makes in a fixed yield beside its main output, per plant.
    pub by_products: Vec<StrategyInputDto>,
    /// The by-products as one plain sentence tail, such as "plus 36 lots of Gasoline"; empty when
    /// the plant makes one product.
    pub by_products_text: String,
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
    /// Set on a storage facility: dry, tank, silo or cold. Empty for a producing plant.
    pub storage_class: String,
    /// Set on a carrier: truck, rail, pipeline or sea. Its output is lots of lane capacity.
    pub transport_mode: String,
    /// A rail terminal or port, which opens its region to the mode for every firm.
    pub terminal: bool,
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
    /// The selected firm's storage here: capacity, use and outside storage by class.
    pub storage: Vec<StorageCellDto>,
}

/// One sector type in one region: who operates and how the player's firm stands.
#[derive(uniffi::Record)]
pub struct SectorCellDto {
    pub industry: String,
    pub operators: u32,
    pub leader: String,
    /// The leader's firm handle, zero when there is none, for its dossier.
    pub leader_firm: u64,
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
    /// Weeks of inputs the division keeps (zero is the firm's default cover) and whether it
    /// builds seasonal inputs ahead in the harvest quarter. Read from its first plant.
    pub stock_weeks: i32,
    /// The firm's default cover in weeks, what a zero stock target means (perishable inputs
    /// keep at most two).
    pub stock_weeks_default: i32,
    pub seasonal_build: bool,
    /// The firm's storage in the division's region.
    pub storage: Vec<StorageCellDto>,
    /// The division's shipping policy: 0 cheapest, 1 fastest, 2 own carriers first, 3 most
    /// reliable; the preferred mode index or -1; the preferred carrier or 0.
    pub ship_kind: u32,
    pub ship_mode: i32,
    pub ship_carrier: u64,
    /// The freight mode the division carries when it is a carrier, else empty, and the regions
    /// its capacity is pinned to (empty is automatic).
    pub carrier_mode: String,
    pub serves: Vec<String>,
}

/// One cost line of a division, weekly.
#[derive(uniffi::Record)]
pub struct DivisionCostDto {
    /// materials, wages, maintenance, land or depreciation.
    pub key: String,
    pub cents: i64,
}

/// One input of a division: what it needs, where it comes from and how long the stock lasts.
#[derive(uniffi::Record)]
pub struct DivisionInputDto {
    pub product: u32,
    pub product_name: String,
    pub need_milli: i64,
    pub used_milli: i64,
    pub source: String,
    pub price_cents: i64,
    pub stock_milli: i64,
    pub cover_weeks_x100: i64,
    pub store_class: String,
}

/// One product a division makes, a by-product included.
#[derive(uniffi::Record)]
pub struct DivisionOutputDto {
    pub product: u32,
    pub product_name: String,
    pub by_product: bool,
    pub made_milli: i64,
    pub sold_milli: i64,
    pub stored_milli: i64,
    pub price_cents: i64,
    pub market_cents: i64,
    /// Sister companies buying it under a group supply link; empty means the open market.
    pub buyers: Vec<String>,
    pub store_class: String,
}

/// One plant of a division.
#[derive(uniffi::Record)]
pub struct DivisionPlantDto {
    pub handle: u64,
    pub blocks: u32,
    pub status: String,
    pub utilisation_bps: i32,
    pub age_weeks: u32,
    pub staff_bps: i32,
    pub inputs_short: bool,
    pub capex_cents: i64,
    pub book_cents: i64,
}

/// One labour tier of a division, lowest skill first.
#[derive(uniffi::Record)]
pub struct DivisionTierDto {
    pub employed: i64,
    pub needed: i64,
    pub wage_cents: i64,
    pub regional_wage_cents: i64,
    pub payroll_cents: i64,
}

/// Everything the division detail view shows, computed by the engine. Results are estimated
/// from the plants' last week and the firm's closed sales at today's asking prices.
#[derive(uniffi::Record)]
pub struct DivisionDetailDto {
    pub firm: u64,
    pub region: String,
    pub region_index: u32,
    pub industry: String,
    pub industry_index: u32,
    pub strategy_name: String,
    pub meaning: String,
    pub revenue_cents: i64,
    pub costs: Vec<DivisionCostDto>,
    pub operating_profit_cents: i64,
    pub margin_bps: i32,
    pub revenue_trend: Vec<i64>,
    pub capex_cents: i64,
    pub book_cents: i64,
    /// auto, manual, market or cost_plus, with its basis points, for the main product.
    pub price_mode: String,
    pub price_bps: i32,
    pub primary_product: u32,
    pub transfer_bps: i32,
    pub has_group_buyers: bool,
    pub stock_weeks: i32,
    pub stock_weeks_effective: i32,
    pub inputs: Vec<DivisionInputDto>,
    pub outputs: Vec<DivisionOutputDto>,
    pub plants: Vec<DivisionPlantDto>,
    pub tiers: Vec<DivisionTierDto>,
    /// none, balloting, demanding or striking.
    pub union_state: String,
    pub union_organisation_bps: i32,
    pub shortage: bool,
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

/// A standing supply link inside a group: one supplier, one buyer, one product.
#[derive(uniffi::Record)]
pub struct TransferLinkDto {
    pub buyer: u64,
    pub buyer_name: String,
    pub seller: u64,
    pub seller_name: String,
    pub product: String,
    pub lots_weekly: i64,
    pub cost_cents: i64,
    pub market_cents: i64,
    pub price_cents: i64,
    /// The supplier division's setting: 0 at cost, 10,000 at market, up to 12,000.
    pub setting_bps: i32,
    pub region_index: u32,
    pub industry_index: u32,
    /// Share of the supplier held outside the controlling group, basis points.
    pub supplier_minority_bps: i32,
    pub buyer_minority_bps: i32,
    /// How far the market marks the supplier down for persistent losses, basis points.
    pub markdown_bps: i32,
    /// Value moved off the supplier's minority holders over the past year, cents.
    pub squeeze_cents: i64,
    pub mine: bool,
}

/// A division's reputation and what it earns.
#[derive(uniffi::Record)]
pub struct ReputationDto {
    pub firm: u64,
    pub firm_name: String,
    pub region: String,
    pub industry: String,
    pub reputation_bps: i32,
    /// Price tolerance the reputation earns in the market, basis points of perceived quality.
    pub premium_bps: i32,
}

#[derive(uniffi::Record)]
pub struct BondLineDto {
    pub bond: u64,
    pub issuer: u64,
    pub issuer_name: String,
    pub principal_cents: i64,
    pub coupon_bps: i32,
    pub weeks_left: u32,
    pub mark_bps: i64,
    pub yield_bps: i64,
    pub spread_now_bps: i32,
    pub available: i64,
    pub held: i64,
    pub mine: bool,
}

#[derive(uniffi::Record)]
pub struct BondQuoteDto {
    pub base_bps: i32,
    pub spread_bps: i32,
    pub coupon_bps: i32,
    pub leverage_bps: i32,
    pub coverage_bps: i32,
    pub rating: String,
    pub max_principal_cents: i64,
}

#[derive(uniffi::Record)]
pub struct VentureDto {
    pub venture: u64,
    pub name: String,
    pub a: u64,
    pub a_name: String,
    pub b: u64,
    pub b_name: String,
    pub stake_a_bps: i32,
    pub stake_b_bps: i32,
    pub formed: u32,
    pub open_to_exit: bool,
    pub equity_cents: i64,
    pub mine: bool,
}

#[derive(uniffi::Record)]
pub struct CampaignDto {
    pub id: u32,
    pub firm: u64,
    pub firm_name: String,
    pub reason: String,
    pub demand: String,
    pub outcome: String,
    pub justification_bps: i32,
    pub stake_bps: i32,
    /// The activist's chance of winning a fight, basis points.
    pub chance_bps: i32,
    pub amount_cents: i64,
    pub deadline: u32,
    pub mine: bool,
}

#[derive(uniffi::Record)]
pub struct StructureStatsDto {
    pub bonds_issued: u32,
    pub ventures_formed: u32,
    pub ipos: u32,
    pub spin_offs: u32,
    pub carve_outs: u32,
    pub activist_campaigns: u32,
    pub activist_wins: u32,
    pub squeeze_cents: i64,
    pub tax_shift_cents: i64,
    pub repeat_raids: u32,
}

#[derive(uniffi::Record)]
pub struct StructureViewDto {
    pub links: Vec<TransferLinkDto>,
    pub reputations: Vec<ReputationDto>,
    pub bonds: Vec<BondLineDto>,
    pub ventures: Vec<VentureDto>,
    pub campaigns: Vec<CampaignDto>,
    pub stats: StructureStatsDto,
    /// The central bank's ten-year yield bonds are priced off, basis points.
    pub ten_year_bps: i32,
}

/// One option for a Command Center picker that the engine alone can list.
#[derive(uniffi::Record)]
pub struct PickDto {
    pub value: String,
    pub label: String,
}

/// A rival division the selected firm can raid.
#[derive(Clone, uniffi::Record)]
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

#[derive(Clone, uniffi::Record)]
pub struct RaidQuoteDto {
    pub attack_cents: i64,
    pub defence_cents: i64,
    pub chance_bps: i32,
    pub blocks_at_stake: u32,
    pub price_cents: i64,
    /// Share of the rival's division at stake, basis points. A bigger stake is harder to win.
    pub stake_bps: i32,
    /// Extra legal and retaliation cost of raiding the same rival again soon.
    pub surcharge_cents: i64,
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
    pub attacker_ceo: String,
    pub attacker_temperament: String,
    pub defender_ceo: String,
    pub defender_temperament: String,
}

/// Whether the selected firm can raid a rival in one market, and if not, why and how to fix it.
#[derive(uniffi::Record)]
pub struct RaidCheckDto {
    pub raidable: bool,
    /// ok, own_group, other_currency, no_division, rival_absent or unknown.
    pub kind: String,
    /// One plain sentence when not raidable.
    pub reason: String,
    /// True when building a plant of that sector in that region would unlock the raid.
    pub needs_build: bool,
    pub option: Option<RaidOptionDto>,
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
    pub divisions: u32,
    /// Sprawl overhead rises smoothly with divisions; this is the count at which it reaches one
    /// percent of weekly revenue, given strength, owned logistics and any secondary type.
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

/// A firm's storage by region and class, optionally for one region. Cells with nothing stored
/// and no space are left out.
fn storage_cells(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    firm: &cw_sim::state::Firm,
    region: Option<u8>,
) -> Vec<StorageCellDto> {
    cw_sim::economy::storage::uses(world, dataset, firm)
        .into_iter()
        .filter(|u| region.is_none_or(|r| r == u.region))
        .filter(|u| u.used_milli > 0 || u.owned_milli > 0)
        .map(|u| StorageCellDto {
            region: dataset.regions[u.region as usize].code.clone(),
            region_name: dataset.regions[u.region as usize].name.clone(),
            class_key: u.class.key().to_owned(),
            class_label: u.class.label().to_owned(),
            used_lots: u.used_milli / 1_000,
            capacity_lots: u.capacity_milli() / 1_000,
            owned_lots: u.owned_milli / 1_000,
            outside_lots: u.outside_milli() / 1_000,
            outside_fee_cents_weekly: u.outside_milli() / 1_000 * u.class.outside_fee_cents(),
        })
        .collect()
}

/// Player-facing text for an overview alert key; unknown keys pass through unchanged.
fn alert_text(alert: cw_session::Alert) -> String {
    if !alert.detail.is_empty() {
        return alert.detail;
    }
    match alert.key.as_str() {
        "alert.storage_full" => format!("{}: storage is nearly full.", alert.subject),
        "alert.storage_fees" => format!(
            "{}: outside storage fees are eating into margin. Build storage or sell down stock.",
            alert.subject
        ),
        "alert.spoilage" => format!(
            "{}: spoilage is losing real money. Sell perishables sooner or build cold storage.",
            alert.subject
        ),
        "alert.lane_congested" => format!(
            "{}: the lane has been full for weeks. Add capacity or reserve it before rivals do.",
            alert.subject
        ),
        "alert.carrier_hike" => format!(
            "{}: carriers raised their rate by a fifth in four weeks. Another mode or your own carrier may pay.",
            alert.subject
        ),
        "alert.activist" => format!(
            "{}: an activist investor has made demands. Open Structure to concede, negotiate or fight; if you do nothing the board answers in four weeks.",
            alert.subject
        ),
        "alert.squeeze" => format!(
            "{}: pricing inside the group is taking value from minority shareholders and may draw a challenge. Move its transfer price toward market.",
            alert.subject
        ),
        "alert.captive_markdown" => format!(
            "{}: the market is marking this supplier down after months of losses. Raise its transfer price or fix its costs.",
            alert.subject
        ),
        "alert.bond_due" => format!(
            "{}: a bond falls due within a quarter and cash does not cover it yet.",
            alert.subject
        ),
        "alert.lane_reserved" => format!(
            "{}: a rival reserved capacity on a lane you ship on.",
            alert.subject
        ),
        _ => alert.key,
    }
}

fn pause_info_dto(info: cw_session::dto::PauseInfo) -> PauseInfoDto {
    PauseInfoDto {
        reason: info.reason,
        kind: info.kind,
        title: info.title,
        detail: info.detail,
        action: info.action,
        action_label: info.action_label,
        event_id: info.event_id,
        firm: info.firm,
    }
}

fn overview_dto(overview: Overview) -> OverviewDto {
    OverviewDto {
        tick: overview.tick,
        week_label: overview.week_label,
        start_year: overview.start_year,
        revision: overview.revision,
        investor_cash_cents: overview.investor_cash_cents,
        investor_net_worth_cents: overview.investor_net_worth_cents,
        reporting_currency: overview.reporting_currency,
        primary_currency: overview.primary_currency,
        primary_firm: overview.primary_firm,
        primary_name: overview.primary_name,
        firm_cash_cents: overview.firm_cash_cents,
        forecast_four_week_cents: overview.forecast_four_week_cents,
        last_week_ebit_cents: overview.last_week_ebit_cents,
        quarter_net_income_cents: overview.quarter_net_income_cents,
        quarter_weeks: overview.quarter_weeks,
        alerts: overview.alerts.into_iter().map(alert_text).collect(),
        pause: overview.pause,
        pause_info: overview.pause_info.map(pause_info_dto),
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
        currency: summary.currency,
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
                cost_of_goods_cents: week.cost_of_goods_cents,
                wages_cents: week.wages_cents,
                operating_expense_cents: week.operating_expense_cents,
                depreciation_cents: week.depreciation_cents,
                net_interest_cents: week.net_interest_cents,
                tax_cents: week.tax_cents,
                ebit_cents: week.ebit_cents,
                net_income_cents: week.net_income_cents,
            })
            .collect(),
    }
}

fn stock_dto(line: cw_session::dto::StockLine) -> StockDto {
    StockDto {
        firm: line.firm,
        name: line.name,
        industry: line.industry,
        region: line.region,
        price_cents: line.price_cents,
        fair_cents: line.fair_cents,
        change_week_bps: line.change_week_bps,
        change_quarter_bps: line.change_quarter_bps,
        history_cents: line.history_cents,
        shares_issued: line.shares_issued,
        market_cap_cents: line.market_cap_cents,
        earnings_cents: line.earnings_cents,
        earnings_weeks: line.earnings_weeks,
        your_shares: line.your_shares,
        float_bps: line.float_bps,
        controlled_by_you: line.controlled_by_you,
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

#[derive(uniffi::Record)]
pub struct ScenarioDto {
    pub id: String,
    pub name: String,
    pub country: String,
    pub start_year: u32,
    pub summary: String,
    pub center_lon_milli: i32,
    pub center_lat_milli: i32,
    pub region_count: u32,
    /// False for scenarios shown as coming soon.
    pub available: bool,
    pub region: String,
    pub image: String,
    pub currency: String,
    pub tagline: String,
    /// The scenario's default world price path: "historical", "historical_variance" or "random".
    pub default_price_path: String,
}

/// The scenarios a new career can start in, in display order.
#[uniffi::export]
pub fn scenarios() -> Vec<ScenarioDto> {
    cw_data::scenarios()
        .into_iter()
        .map(|scenario| {
            // Each scenario has its own regions; one with no content of its own shows none.
            let regions = cw_data::Dataset::load_scenario(&scenario.id)
                .map(|dataset| dataset.regions.len() as u32)
                .unwrap_or_default();
            (scenario, regions)
        })
        .map(|(scenario, regions)| ScenarioDto {
            default_price_path: scenario.price_path().key().to_owned(),
            id: scenario.id,
            name: scenario.name,
            country: scenario.country,
            start_year: scenario.start_year,
            summary: scenario.summary,
            center_lon_milli: scenario.center_lon_milli,
            center_lat_milli: scenario.center_lat_milli,
            region_count: if scenario.available { regions } else { 0 },
            available: scenario.available,
            region: scenario.region,
            image: scenario.image,
            currency: scenario.currency,
            tagline: scenario.tagline,
        })
        .collect()
}

/// The products, priced for the default scenario. A career should ask its own session instead
/// (`SessionHandle::products`) or name its scenario (`products_for`): reference prices differ by
/// scenario.
#[uniffi::export]
pub fn products() -> Vec<ProductDto> {
    products_for(cw_data::DEFAULT_SCENARIO.to_owned())
}

/// The products with one scenario's reference prices.
#[uniffi::export]
pub fn products_for(scenario_id: String) -> Vec<ProductDto> {
    match cw_data::Dataset::load_scenario(&scenario_id) {
        Ok(dataset) => product_dtos(&dataset),
        Err(_) => Vec::new(),
    }
}

fn product_dtos(dataset: &cw_data::Dataset) -> Vec<ProductDto> {
    {
        dataset
            .products
            .iter()
            .enumerate()
            .map(|(index, product)| ProductDto {
                index: index as u32,
                name: product.name.clone(),
                reference_price_cents: product.reference_price_cents,
                kind: format!("{:?}", product.kind).to_lowercase(),
                storage_class: if product.kind.is_service() || product.kind.is_financial() {
                    String::new()
                } else {
                    product.storage_class.key().to_owned()
                },
                holding_bps_weekly: product.holding_bps_weekly,
                spoilage_bps_weekly: product.spoilage_bps_weekly,
            })
            .collect()
    }
}

/// Facility templates with their capex and build time, for build and founder forms.
#[uniffi::export]
pub fn recipes() -> Vec<RecipeDto> {
    recipes_for(cw_data::DEFAULT_SCENARIO.to_owned())
}

/// Facility templates of one scenario.
#[uniffi::export]
pub fn recipes_for(scenario_id: String) -> Vec<RecipeDto> {
    match cw_data::Dataset::load_scenario(&scenario_id) {
        Ok(dataset) => recipe_dtos(&dataset),
        Err(_) => Vec::new(),
    }
}

fn recipe_dtos(dataset: &cw_data::Dataset) -> Vec<RecipeDto> {
    {
        dataset
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
                storage_class: recipe
                    .storage_class
                    .map(|c| c.key().to_owned())
                    .unwrap_or_default(),
            })
            .collect()
    }
}

/// The full strategy catalogue with inputs and reference margins.
fn pct_text(bps: i64) -> String {
    format!("{}.{:02}%", bps / 100, (bps % 100).abs())
}

fn raid_quote_dto(quote: cw_sim::economy::sectors::RaidQuote) -> RaidQuoteDto {
    RaidQuoteDto {
        attack_cents: quote.attack_cents,
        defence_cents: quote.defence_cents,
        chance_bps: quote.chance_bps,
        blocks_at_stake: quote.blocks_at_stake as u32,
        price_cents: quote.price_cents,
        stake_bps: quote.stake_bps,
        surcharge_cents: quote.surcharge_cents,
    }
}

/// The raid `me` could launch on `rival`'s division in one market, quoted at a two-to-one
/// campaign. None when the rival has no plants there or `me` is the rival's own group.
pub(crate) fn raid_option(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    me: cw_core::ids::Handle,
    rival: cw_core::ids::Handle,
    region: u8,
    industry: u8,
) -> Option<RaidOptionDto> {
    use cw_sim::economy::sectors;
    if rival == me
        || cw_sim::economy::ownership::same_group(world, rival, me)
        || cw_sim::economy::fx::require_same_currency(world, me, rival).is_err()
        || sectors::division_plants(world, dataset, me, region, industry).is_empty()
    {
        return None;
    }
    let rival_blocks: u32 = sectors::division_plants(world, dataset, rival, region, industry)
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .map(|p| p.blocks as u32)
        .sum();
    if rival_blocks < 1 {
        return None;
    }
    let probe = sectors::raid_quote(world, dataset, me, rival, region, industry, 0);
    let budget = probe.defence_cents * 2;
    let quote = sectors::raid_quote(world, dataset, me, rival, region, industry, budget);
    Some(RaidOptionDto {
        target: rival.0,
        target_name: world
            .firms
            .get(rival)
            .map(|f| f.name.clone())
            .unwrap_or_default(),
        region: dataset.regions[region as usize].code.clone(),
        region_index: region as u32,
        industry: cw_sim::state::INDUSTRY_NAMES[industry as usize].to_owned(),
        industry_index: industry as u32,
        rival_blocks,
        // No cooldown any more: repeat raids on a rival cost more instead.
        ready_week: 0,
        suggested_budget_cents: budget,
        quote: raid_quote_dto(quote),
    })
}

#[uniffi::export]
pub fn freight_modes() -> Vec<FreightModeDto> {
    match cw_data::Dataset::load() {
        Ok(dataset) => dataset
            .logistics
            .modes
            .iter()
            .enumerate()
            .map(|(index, mode)| FreightModeDto {
                index: index as u32,
                id: mode.id.clone(),
                name: mode.name.clone(),
                rate_milli_cents_per_lot_km: mode.rate_milli_cents_per_lot_km,
                speed_km_week: mode.speed_km_week,
                min_lots: mode.min_lots,
                needs_terminal: mode.needs_terminal,
                needs_infrastructure: mode.needs_infrastructure,
                classes: mode.classes.iter().map(|c| c.key().to_owned()).collect(),
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[uniffi::export]
pub fn strategies() -> Vec<StrategyDto> {
    strategies_for(cw_data::DEFAULT_SCENARIO.to_owned())
}

/// The strategy catalogue with one scenario's prices and wages.
#[uniffi::export]
pub fn strategies_for(scenario_id: String) -> Vec<StrategyDto> {
    match cw_data::Dataset::load_scenario(&scenario_id) {
        Ok(dataset) => strategy_dtos(&dataset),
        Err(_) => Vec::new(),
    }
}

fn strategy_dtos(dataset: &cw_data::Dataset) -> Vec<StrategyDto> {
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
            let revenue = recipe.output_value(price);
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
                output_name: match recipe.storage_class {
                    Some(class) => format!("{} space", class.label().to_lowercase()),
                    None => name(&recipe.output),
                },
                weekly_output_lots: recipe.weekly_output_lots,
                by_products: recipe
                    .by_products
                    .iter()
                    .map(|by| StrategyInputDto {
                        product: by.product.clone(),
                        product_name: name(&by.product),
                        lots: by.weekly_lots,
                    })
                    .collect(),
                by_products_text: if recipe.by_products.is_empty() {
                    String::new()
                } else {
                    format!(
                        "plus {}",
                        recipe
                            .by_products
                            .iter()
                            .map(|by| format!("{} lots of {}", by.weekly_lots, name(&by.product)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
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
                storage_class: recipe
                    .storage_class
                    .map(|c| c.key().to_owned())
                    .unwrap_or_default(),
                transport_mode: recipe.transport_mode.clone().unwrap_or_default(),
                terminal: recipe.terminal,
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

/// One week of the economy for the charts.
#[derive(uniffi::Record)]
pub struct MacroPointDto {
    pub week: u32,
    pub gdp_index: i64,
    pub output_gap_bps: i32,
    pub inflation_bps: i32,
    pub policy_rate_bps: i32,
}

#[derive(uniffi::Record)]
pub struct RateDecisionDto {
    pub week: u32,
    pub change_bps: i32,
    pub rate_bps: i32,
    pub reason: String,
}

#[derive(uniffi::Record)]
pub struct NewsDto {
    pub week: u32,
    pub text: String,
    /// The cast member the item is about, for the portrait beside it.
    pub person: Option<String>,
    /// The company the item names, for a tap through to its page.
    pub firm: Option<u64>,
    /// That company's name, for its logo.
    pub firm_name: Option<String>,
}

/// The laws in force or announced.
#[derive(uniffi::Record)]
pub struct LawsDto {
    pub corporate_tax_bps: i32,
    pub wage_floor_hourly_cents: i64,
    pub merger_review_bps: i64,
    pub unions: String,
    /// Owned pipelines, rail and ships sell spare capacity at a regulated rate.
    pub common_carrier: bool,
    /// Share at which buying a supplier or customer goes to review; zero when it does not.
    pub vertical_review_bps: i64,
    /// The government buys in busts and sells in spikes.
    pub reserve: bool,
    /// none, bailout or nationalise.
    pub rescue: String,
    pub privatise: bool,
    /// Output cap in quota states, as a share of capacity; zero when there are no quotas.
    pub quota_bps: i32,
    /// Names of the states with quotas.
    pub quota_states: Vec<String>,
}

/// One good in the strategic reserve.
#[derive(uniffi::Record)]
pub struct ReserveLineDto {
    pub product: String,
    pub label: String,
    pub lots: i64,
    /// buying, selling or holding.
    pub phase: String,
}

/// A production quota biting this week.
#[derive(uniffi::Record)]
pub struct QuotaDto {
    pub region_name: String,
    pub product: String,
    pub label: String,
    pub cap_bps: i32,
}

/// A pipeline spill, port strike, refinery fire, lane outage or harvest failure.
#[derive(uniffi::Record)]
pub struct IncidentDto {
    pub week: u32,
    /// spill, port_strike, refinery_fire, lane_outage, harvest_failure or timeline.
    pub kind: String,
    pub text: String,
    /// Weeks left until it is over.
    pub weeks_left: u32,
    /// One of your companies is touched.
    pub touches_you: bool,
}

/// A dated event from the scenario's history the player has been told is coming.
#[derive(uniffi::Record)]
pub struct TimelineUpcomingDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub year: i32,
    /// 1 to 52.
    pub week_of_year: u32,
    /// The game week it happens in.
    pub week: u32,
    pub weeks_away: u32,
}

/// A dated event from the scenario's history that has happened.
#[derive(uniffi::Record)]
pub struct TimelinePastDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub year: i32,
    pub week_of_year: u32,
    pub week: u32,
}

/// The scenario timeline as the player may see it. Events not yet announced are never listed.
#[derive(uniffi::Record)]
pub struct TimelineDto {
    /// "off", "historical" or "shuffled".
    pub mode: String,
    pub upcoming: Vec<TimelineUpcomingDto>,
    pub past: Vec<TimelinePastDto>,
}

/// The economy, the central bank and the government, for the Markets economy card.
#[derive(uniffi::Record)]
pub struct EconomyDto {
    /// expansion, boom, slowdown, recession or recovery.
    pub phase: String,
    pub growth_bps: i32,
    pub output_gap_bps: i32,
    pub inflation_bps: i32,
    pub expected_inflation_bps: i32,
    pub target_inflation_bps: i32,
    pub policy_rate_bps: i32,
    pub prime_rate_bps: i32,
    pub ten_year_yield_bps: i32,
    pub price_index_bps: i64,
    /// How stretched borrowing is, 0 to 1,500: above about 450 a bust grows likely.
    pub credit_bps: i32,
    pub next_meeting_week: u32,
    pub chair_name: String,
    /// dovish, leaning dovish, centrist, leaning hawkish or hawkish.
    pub chair_stance: String,
    pub chair_term_ends: u32,
    /// The chair's portrait id, empty when the chair is not a cast member.
    pub chair_person: String,
    /// The central bank chair, the country's leader, the opposition leader and the governors.
    pub posts: Vec<PostDto>,
    pub decisions: Vec<RateDecisionDto>,
    pub history: Vec<MacroPointDto>,
    /// The governing party's name and key (business or labour).
    pub party_name: String,
    pub party: String,
    pub approval_bps: i32,
    pub next_election_week: u32,
    pub laws: LawsDto,
    pub pending_laws: Option<LawsDto>,
    pub pending_effective_week: u32,
    pub news: Vec<NewsDto>,
    pub reserve: Vec<ReserveLineDto>,
    pub reserve_value_cents: i64,
    pub quotas: Vec<QuotaDto>,
    pub incidents: Vec<IncidentDto>,
    /// State-owned firms, by name.
    pub state_firms: Vec<String>,
    /// How world commodity prices move: off, historical, historical_variance or random.
    pub price_path: String,
    /// The outside world's price levels the gateways follow, one per product family.
    pub world_prices: Vec<WorldPriceDto>,
    /// Exchange rates and central banks; None when the scenario has one money.
    pub fx: Option<FxDto>,
}

/// One currency against the reporting currency, with its central bank.
#[derive(uniffi::Record)]
pub struct CurrencyDto {
    pub id: String,
    pub name: String,
    pub symbol: String,
    pub country: String,
    pub central_bank: String,
    /// The bank's chair and their stance (dovish to hawkish); empty without a cast.
    pub chair_name: String,
    pub chair_stance: String,
    /// What one unit is worth in the reporting currency, in millionths.
    pub value_ppm: i64,
    /// Change over the last 13 weeks, in basis points.
    pub trend_bps: i32,
    pub policy_rate_bps: i32,
    pub inflation_bps: i32,
    pub pegged: bool,
    /// Half width of the peg band now, in basis points; zero for a float.
    pub band_bps: i32,
    /// How hard the market is pushing on the peg, in basis points.
    pub strain_bps: i32,
    /// The week a band crisis widened the peg, if one has.
    pub crisis_week: Option<u32>,
    /// Weekly value in the reporting currency, millionths, oldest first, up to 52 weeks.
    pub history_ppm: Vec<i64>,
}

/// The foreign exchange card: rates, the reporting currency and recorded gains and losses.
#[derive(uniffi::Record)]
pub struct FxDto {
    /// The reporting currency id. `currencies[0]` is the numeraire.
    pub reporting: String,
    pub reporting_symbol: String,
    pub currencies: Vec<CurrencyDto>,
    /// Foreign exchange results booked by your companies, in the reporting currency.
    pub gains_cents: i64,
    pub losses_cents: i64,
    /// Foreign sales your companies are still waiting to collect, in the reporting currency.
    pub open_foreign_sales_cents: i64,
}

fn fx_dto(view: cw_session::dto::FxView) -> FxDto {
    FxDto {
        reporting: view.reporting,
        reporting_symbol: view.reporting_symbol,
        currencies: view
            .currencies
            .into_iter()
            .map(|c| CurrencyDto {
                id: c.id,
                name: c.name,
                symbol: c.symbol,
                country: c.country,
                central_bank: c.central_bank,
                chair_name: c.chair_name,
                chair_stance: c.chair_stance,
                value_ppm: c.value_ppm,
                trend_bps: c.trend_bps,
                policy_rate_bps: c.policy_rate_bps,
                inflation_bps: c.inflation_bps,
                pegged: c.pegged,
                band_bps: c.band_bps,
                strain_bps: c.strain_bps,
                crisis_week: c.crisis_week,
                history_ppm: c.history_ppm,
            })
            .collect(),
        gains_cents: view.gains_cents,
        losses_cents: view.losses_cents,
        open_foreign_sales_cents: view.open_foreign_sales_cents,
    }
}

/// One world commodity price family: where it stands against day one and where it is heading.
#[derive(uniffi::Record)]
pub struct WorldPriceDto {
    pub family: String,
    pub label: String,
    /// Level against the first day of the game, 10,000 meaning unchanged.
    pub index_bps: i32,
    /// Change over the last 13 weeks, in basis points of the earlier level.
    pub trend_bps: i32,
    /// Ids of the products whose import and export prices follow this family.
    pub products: Vec<String>,
}

/// One company's research: technology level, progress to the next and what it buys.
/// One thing the CFO says is costing the company money, with a button that fixes it.
#[derive(uniffi::Record)]
pub struct CfoAdviceDto {
    pub key: String,
    pub headline: String,
    pub detail: String,
    /// Weekly cost in the company's currency cents.
    pub weekly_cents: i64,
    pub action_label: String,
    /// Command intents as JSON; the button submits them in order.
    pub commands: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct ResearchDto {
    pub firm: u64,
    pub level: u32,
    pub max_level: u32,
    /// The industry's frontier: a company can research one level past it.
    pub frontier: u32,
    pub progress_cents: i64,
    pub next_threshold_cents: i64,
    pub success_bps: i32,
    pub research_budget_cents: i64,
    pub marketing_budget_cents: i64,
    /// Input and labour use per unit at this level and the next, in basis points of baseline.
    pub intensity_bps: i32,
    pub next_intensity_bps: i32,
    pub quality_bps: i32,
    pub next_quality_bps: i32,
}

fn laws_dto(laws: &cw_sim::state::Laws, dataset: &cw_data::Dataset) -> LawsDto {
    LawsDto {
        corporate_tax_bps: laws.corporate_tax_bps,
        wage_floor_hourly_cents: laws.wage_floor_cents / 40,
        merger_review_bps: laws.merger_review_bps,
        unions: laws.unions.key().to_owned(),
        common_carrier: laws.common_carrier,
        vertical_review_bps: laws.vertical_review_bps,
        reserve: laws.reserve,
        rescue: laws.rescue.key().to_owned(),
        privatise: laws.privatise,
        quota_bps: laws.quota_bps,
        quota_states: if laws.quota_bps > 0 {
            dataset
                .states
                .iter()
                .enumerate()
                .take(64)
                .filter(|(i, _)| laws.quota_states & (1 << i) != 0)
                .map(|(_, s)| s.name.clone())
                .collect()
        } else {
            Vec::new()
        },
    }
}

/// Everything the new-game screen chooses.
#[derive(uniffi::Record)]
pub struct NewGameDto {
    pub seed: u64,
    pub scenario_id: String,
    /// Start running 60 percent of a company (true) or with cash to found one (false).
    pub guided: bool,
    pub five_year: bool,
    /// "easy", "standard" or "hard".
    pub difficulty: String,
    /// "historic", "calm", "boom" or "downturn".
    pub economy: String,
    /// Starting capital in cents, or none for the difficulty's.
    pub starting_capital_cents: Option<i64>,
    /// How world commodity prices move: "historical", "historical_variance" or "random".
    /// Empty takes the scenario's default.
    pub price_path: String,
    /// Whether the scenario's dated history plays: "historical", "off" or "shuffled". Empty
    /// takes historical.
    #[uniffi(default = "")]
    pub timeline: String,
}

#[uniffi::export]
impl SessionHandle {
    /// A new career from the new-game screen's choices.
    #[uniffi::constructor]
    pub fn start(options: NewGameDto) -> Result<Arc<SessionHandle>, FfiError> {
        let config = cw_session::ScenarioConfig {
            seed: options.seed,
            scenario: if options.guided {
                cw_sim::state::ScenarioKind::Guided
            } else {
                cw_sim::state::ScenarioKind::Founder
            },
            mode: if options.five_year {
                cw_sim::state::ScenarioMode::FiveYear
            } else {
                cw_sim::state::ScenarioMode::Sandbox
            },
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::from_key(&options.difficulty),
            economy_start: cw_sim::state::EconomyStart::from_key(&options.economy),
            starting_capital_cents: options.starting_capital_cents.filter(|c| *c > 0),
            prices: cw_session::PriceSetup {
                scenario_id: options.scenario_id.clone(),
                path: cw_data::PricePath::from_key(&options.price_path),
                timeline: cw_sim::economy::timeline::TimelineMode::from_key(&options.timeline),
            },
        };
        let session = Session::create(config)?;
        Ok(Arc::new(SessionHandle {
            inner: Mutex::new(session),
        }))
    }

    /// The engine fixture constructor: no world commodity prices. Games started from the
    /// new-game screen use `start`.
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
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
        };
        let session = Session::create(config)?;
        Ok(Arc::new(SessionHandle {
            inner: Mutex::new(session),
        }))
    }

    pub fn overview(&self) -> OverviewDto {
        overview_dto(self.lock().overview())
    }

    /// The scenario this career plays in.
    pub fn scenario_id(&self) -> String {
        self.lock().dataset.scenario_id.clone()
    }

    /// The products with this career's reference prices.
    pub fn products(&self) -> Vec<ProductDto> {
        product_dtos(&self.lock().dataset)
    }

    /// The facility templates of this career's scenario.
    pub fn recipes(&self) -> Vec<RecipeDto> {
        recipe_dtos(&self.lock().dataset)
    }

    /// The strategy catalogue with this career's prices and wages.
    pub fn strategies(&self) -> Vec<StrategyDto> {
        strategy_dtos(&self.lock().dataset)
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

    /// Why the clock is stopped and what resumes it, or none while time runs.
    pub fn pause_info(&self) -> Option<PauseInfoDto> {
        self.lock().pause_info().map(pause_info_dto)
    }

    /// Which pauses stop the clock: `decisions` (the default) or `decisions_and_warnings`.
    /// False for an unknown key.
    pub fn set_pause_policy(&self, key: String) -> bool {
        match cw_session::dto::PausePolicy::from_key(&key) {
            Some(policy) => {
                self.lock().set_pause_policy(policy);
                true
            }
            None => false,
        }
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

    /// The scenario's dated history: announced events still ahead, and events that happened.
    pub fn timeline(&self) -> TimelineDto {
        let session = self.lock();
        let world = &session.world;
        TimelineDto {
            mode: world.systemic.timeline.mode.key().to_owned(),
            upcoming: cw_sim::economy::timeline::upcoming(world)
                .into_iter()
                .map(|e| TimelineUpcomingDto {
                    id: e.id,
                    title: e.title,
                    summary: e.summary,
                    year: e.year,
                    week_of_year: e.week_of_year,
                    week: e.tick,
                    weeks_away: e.weeks_away,
                })
                .collect(),
            past: cw_sim::economy::timeline::past(world)
                .into_iter()
                .map(|e| TimelinePastDto {
                    id: e.id,
                    title: e.title,
                    summary: e.summary,
                    year: e.year,
                    week_of_year: e.week_of_year,
                    week: e.tick,
                })
                .collect(),
        }
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
                        leader_firm: leader.map(|(h, _)| h.0).unwrap_or(0),
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
                    population_2000: region.population(),
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
                    storage: own
                        .and_then(|o| world.firms.get(o))
                        .map(|f| storage_cells(world, dataset, f, Some(index as u8)))
                        .unwrap_or_default(),
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
                    stock_weeks: d
                        .plants
                        .first()
                        .and_then(|h| world.facilities.get(*h))
                        .map(|p| p.stock_plan.weeks)
                        .unwrap_or(0),
                    stock_weeks_default: world
                        .firms
                        .get(handle)
                        .map(|f| f.policies.input_cover_weeks.max(1))
                        .unwrap_or(4),
                    seasonal_build: d
                        .plants
                        .first()
                        .and_then(|h| world.facilities.get(*h))
                        .map(|p| p.stock_plan.seasonal)
                        .unwrap_or(true),
                    storage: world
                        .firms
                        .get(handle)
                        .map(|f| storage_cells(world, dataset, f, Some(d.region)))
                        .unwrap_or_default(),
                    ship_kind: d
                        .plants
                        .first()
                        .and_then(|h| world.facilities.get(*h))
                        .map(|p| p.ship.kind.code() as u32)
                        .unwrap_or(0),
                    ship_mode: d
                        .plants
                        .first()
                        .and_then(|h| world.facilities.get(*h))
                        .and_then(|p| p.ship.mode)
                        .map(|m| m as i32)
                        .unwrap_or(-1),
                    ship_carrier: d
                        .plants
                        .first()
                        .and_then(|h| world.facilities.get(*h))
                        .and_then(|p| p.ship.carrier)
                        .map(|h| h.0)
                        .unwrap_or(0),
                    carrier_mode: recipe.transport_mode.clone().unwrap_or_default(),
                    serves: d
                        .plants
                        .first()
                        .and_then(|h| world.facilities.get(*h))
                        .map(|p| {
                            p.serves
                                .iter()
                                .map(|r| dataset.regions[*r as usize].code.clone())
                                .collect()
                        })
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    /// The detail view of one division: financials, strategy, inputs, outputs, plants, labour.
    pub fn division_detail(
        &self,
        firm: u64,
        region: String,
        industry: String,
    ) -> Option<DivisionDetailDto> {
        use cw_sim::economy::division_report;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = cw_core::ids::Handle(firm);
        let region_index = dataset.regions.iter().position(|r| r.code == region)? as u8;
        let industry_index = cw_sim::economy::operations::industry_index(&industry)?;
        let r = division_report::report(world, dataset, handle, region_index, industry_index)?;
        let strategy = cw_sim::economy::sectors::division_strategy(
            world,
            dataset,
            handle,
            region_index,
            industry_index,
        )?;
        let recipe = &dataset.recipes[strategy as usize];
        let name = |code: u16| dataset.products[code as usize].name.clone();
        Some(DivisionDetailDto {
            firm,
            region,
            region_index: region_index as u32,
            industry,
            industry_index: industry_index as u32,
            strategy_name: if recipe.name.is_empty() {
                recipe.id.clone()
            } else {
                recipe.name.clone()
            },
            meaning: r.meaning,
            revenue_cents: r.revenue_cents,
            costs: r
                .costs
                .into_iter()
                .map(|c| DivisionCostDto {
                    key: c.key.to_owned(),
                    cents: c.cents,
                })
                .collect(),
            operating_profit_cents: r.operating_profit_cents,
            margin_bps: r.margin_bps,
            revenue_trend: r.revenue_trend,
            capex_cents: r.capex_cents,
            book_cents: r.book_cents,
            price_mode: r.price_mode.to_owned(),
            price_bps: r.price_bps,
            primary_product: r.primary_product as u32,
            transfer_bps: r.transfer_bps,
            has_group_buyers: r.has_group_buyers,
            stock_weeks: r.stock_weeks,
            stock_weeks_effective: r.stock_weeks_effective,
            inputs: r
                .inputs
                .into_iter()
                .map(|i| DivisionInputDto {
                    product: i.product as u32,
                    product_name: name(i.product),
                    need_milli: i.need_milli,
                    used_milli: i.used_milli,
                    source: i.source,
                    price_cents: i.price_cents,
                    stock_milli: i.stock_milli,
                    cover_weeks_x100: i.cover_weeks_x100,
                    store_class: i.store_class.to_owned(),
                })
                .collect(),
            outputs: r
                .outputs
                .into_iter()
                .map(|o| DivisionOutputDto {
                    product: o.product as u32,
                    product_name: name(o.product),
                    by_product: o.by_product,
                    made_milli: o.made_milli,
                    sold_milli: o.sold_milli,
                    stored_milli: o.stored_milli,
                    price_cents: o.price_cents,
                    market_cents: o.market_cents,
                    buyers: o.buyers,
                    store_class: o.store_class.to_owned(),
                })
                .collect(),
            plants: r
                .plants
                .into_iter()
                .map(|p| DivisionPlantDto {
                    handle: p.handle.0,
                    blocks: p.blocks,
                    status: p.status.to_owned(),
                    utilisation_bps: p.utilisation_bps,
                    age_weeks: p.age_weeks,
                    staff_bps: p.staff_bps,
                    inputs_short: p.inputs_short,
                    capex_cents: p.capex_cents,
                    book_cents: p.book_cents,
                })
                .collect(),
            tiers: r
                .tiers
                .iter()
                .map(|t| DivisionTierDto {
                    employed: t.employed,
                    needed: t.needed,
                    wage_cents: t.wage_cents,
                    regional_wage_cents: t.regional_wage_cents,
                    payroll_cents: t.payroll_cents,
                })
                .collect(),
            union_state: r.union_state.to_owned(),
            union_organisation_bps: r.union_organisation_bps,
            shortage: r.shortage,
        })
    }

    /// Groups, transfer prices, reputation, bonds, ventures and activist campaigns in one view.
    pub fn structure_view(&self) -> StructureViewDto {
        use cw_sim::economy::structure as st;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let mine =
            |firm: cw_core::ids::Handle| cw_sim::economy::events_engine::player_side(world, firm);
        let name = |firm: cw_core::ids::Handle| {
            world
                .firms
                .get(firm)
                .map(|f| f.name.clone())
                .unwrap_or_default()
        };
        let links = world
            .structure
            .links
            .iter()
            .filter_map(|((buyer, seller, product), link)| {
                let contract = world.contracts.get(link.contract)?;
                Some(TransferLinkDto {
                    buyer: buyer.0,
                    buyer_name: name(*buyer),
                    seller: seller.0,
                    seller_name: name(*seller),
                    product: dataset.products[*product as usize].id.clone(),
                    lots_weekly: contract.quantity_weekly.lots(),
                    cost_cents: link.cost_cents,
                    market_cents: link.market_cents,
                    price_cents: link.price_cents,
                    setting_bps: st::transfer_bps(world, *seller, link.region, link.industry),
                    region_index: link.region as u32,
                    industry_index: link.industry as u32,
                    supplier_minority_bps: st::minority_bps(world, *seller),
                    buyer_minority_bps: st::minority_bps(world, *buyer),
                    markdown_bps: st::markdown_bps(world, *seller),
                    squeeze_cents: world.structure.squeeze.get(seller).copied().unwrap_or(0),
                    mine: mine(*seller) || mine(*buyer),
                })
            })
            .collect();
        let mut reputations: Vec<ReputationDto> = world
            .structure
            .reputation
            .iter()
            .filter(|((firm, _, _), _)| mine(*firm))
            .map(|((firm, region, industry), rep)| {
                let premium = if *rep >= st::NEUTRAL_REP {
                    (*rep - st::NEUTRAL_REP) * 1_500 / (10_000 - st::NEUTRAL_REP)
                } else {
                    -(st::NEUTRAL_REP - *rep) * 500 / st::NEUTRAL_REP
                };
                ReputationDto {
                    firm: firm.0,
                    firm_name: name(*firm),
                    region: dataset.regions[*region as usize].code.clone(),
                    industry: cw_sim::state::INDUSTRY_NAMES[*industry as usize].to_owned(),
                    reputation_bps: *rep,
                    premium_bps: premium,
                }
            })
            .collect();
        reputations.sort_by_key(|r| std::cmp::Reverse(r.reputation_bps));
        let bonds = st::bond_board(world)
            .into_iter()
            .map(|line| BondLineDto {
                bond: line.bond.0,
                issuer: line.issuer.0,
                issuer_name: name(line.issuer),
                principal_cents: line.principal_cents,
                coupon_bps: line.coupon_bps,
                weeks_left: line.weeks_left,
                mark_bps: line.mark_bps,
                yield_bps: line.yield_bps,
                spread_now_bps: line.spread_now_bps,
                available: line.outside_qty,
                held: line.investor_qty,
                mine: mine(line.issuer),
            })
            .collect();
        let ventures = world
            .structure
            .jvs
            .iter()
            .filter_map(|j| {
                let f = world.firms.get(j.firm)?;
                Some(VentureDto {
                    venture: j.firm.0,
                    name: f.name.clone(),
                    a: j.a.0,
                    a_name: name(j.a),
                    b: j.b.0,
                    b_name: name(j.b),
                    stake_a_bps: f.shares.vote_bps(cw_sim::state::HolderId::Firm(j.a)),
                    stake_b_bps: f.shares.vote_bps(cw_sim::state::HolderId::Firm(j.b)),
                    formed: j.formed,
                    open_to_exit: world.tick.0 >= j.formed + st::JV_LOCKUP_WEEKS,
                    equity_cents: f.valuation.equity_cents,
                    mine: mine(j.a) || mine(j.b),
                })
            })
            .collect();
        let campaigns = world
            .structure
            .campaigns
            .iter()
            .rev()
            .take(16)
            .map(|c| CampaignDto {
                id: c.id,
                firm: c.target.0,
                firm_name: name(c.target),
                reason: c.reason.key().to_owned(),
                demand: c.demand.key().to_owned(),
                outcome: c.outcome.key().to_owned(),
                justification_bps: c.justification_bps,
                stake_bps: c.stake_bps,
                chance_bps: st::activist_chance_bps(world, c),
                amount_cents: c.amount_cents,
                deadline: c.deadline,
                mine: mine(c.target),
            })
            .collect();
        let stats = &world.structure.stats;
        StructureViewDto {
            links,
            reputations,
            bonds,
            ventures,
            campaigns,
            stats: StructureStatsDto {
                bonds_issued: stats.bonds_issued,
                ventures_formed: stats.jvs_formed,
                ipos: stats.ipos,
                spin_offs: stats.spin_offs,
                carve_outs: stats.carve_outs,
                activist_campaigns: stats.activist_events,
                activist_wins: stats.activist_wins,
                squeeze_cents: stats.squeeze_cents,
                tax_shift_cents: stats.tax_shift_cents,
                repeat_raids: stats.raids_repeat,
            },
            ten_year_bps: cw_sim::economy::macro_cycle::ten_year_yield_bps(world),
        }
    }

    /// What a bond would cost to issue now: the base yield, the credit spread and the figures
    /// behind it.
    pub fn bond_quote(&self, firm: u64, principal_cents: i64, weeks: u32) -> BondQuoteDto {
        let session = self.lock();
        let q = cw_sim::economy::structure::bond_quote(
            &session.world,
            cw_core::ids::Handle(firm),
            principal_cents,
            weeks,
        );
        BondQuoteDto {
            base_bps: q.base_bps,
            spread_bps: q.spread_bps,
            coupon_bps: q.coupon_bps,
            leverage_bps: q.leverage_bps,
            coverage_bps: q.coverage_bps,
            rating: q.rating.to_string(),
            max_principal_cents: q.max_principal_cents,
        }
    }

    /// Options for Command Center pickers the engine lists itself: `bond`, `my_bond`,
    /// `my_venture`, `open_campaign`.
    pub fn structure_picks(&self, kind: String) -> Vec<PickDto> {
        use cw_sim::economy::structure as st;
        let session = self.lock();
        let world = &session.world;
        let mine =
            |firm: cw_core::ids::Handle| cw_sim::economy::events_engine::player_side(world, firm);
        let name = |firm: cw_core::ids::Handle| {
            world
                .firms
                .get(firm)
                .map(|f| f.name.clone())
                .unwrap_or_default()
        };
        match kind.as_str() {
            "bond" => st::bond_board(world)
                .into_iter()
                .filter(|l| l.outside_qty > 0)
                .map(|l| PickDto {
                    value: l.bond.0.to_string(),
                    label: format!(
                        "{}: {} coupon, {} weeks, price {}",
                        name(l.issuer),
                        pct_text(l.coupon_bps as i64),
                        l.weeks_left,
                        pct_text(l.mark_bps)
                    ),
                })
                .collect(),
            "my_bond" => st::bond_board(world)
                .into_iter()
                .filter(|l| l.investor_qty > 0)
                .map(|l| PickDto {
                    value: l.bond.0.to_string(),
                    label: format!(
                        "{}: {} held, {} coupon, price {}",
                        name(l.issuer),
                        l.investor_qty,
                        pct_text(l.coupon_bps as i64),
                        pct_text(l.mark_bps)
                    ),
                })
                .collect(),
            "my_venture" => world
                .structure
                .jvs
                .iter()
                .filter(|j| mine(j.a) || mine(j.b))
                .map(|j| PickDto {
                    value: j.firm.0.to_string(),
                    label: format!("{} ({} and {})", name(j.firm), name(j.a), name(j.b)),
                })
                .collect(),
            "open_campaign" => world
                .structure
                .campaigns
                .iter()
                .filter(|c| c.outcome == st::Outcome::Open && mine(c.target))
                .map(|c| PickDto {
                    value: c.id.to_string(),
                    label: format!(
                        "{}: {} wants {}",
                        name(c.target),
                        c.reason.key().replace('_', " "),
                        c.demand.key()
                    ),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Every lane with capacity or traffic this week, busiest first.
    pub fn lanes(&self) -> Vec<LaneDto> {
        use cw_sim::economy::logistics as lg;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let mine =
            |firm: cw_core::ids::Handle| cw_sim::economy::events_engine::player_side(world, firm);
        let primary = world.investor.primary_group;
        let mut out: Vec<LaneDto> = lg::lane_views(world, dataset)
            .into_iter()
            .map(|view| {
                let key = view.key;
                let stat = world.logistics.stats.get(&key);
                let lane = world.logistics.lanes.get(&key);
                let contracts_mine: i32 = world
                    .logistics
                    .contracts
                    .iter()
                    .filter(|c| c.key == key && mine(c.holder))
                    .map(|c| c.held)
                    .sum();
                LaneDto {
                    a: dataset.regions[key.a as usize].code.clone(),
                    b: dataset.regions[key.b as usize].code.clone(),
                    a_name: dataset.regions[key.a as usize].name.clone(),
                    b_name: dataset.regions[key.b as usize].name.clone(),
                    mode: key.mode as u32,
                    mode_id: dataset.logistics.modes[key.mode as usize].id.clone(),
                    mode_name: dataset.logistics.modes[key.mode as usize].name.clone(),
                    km: view.km,
                    capacity_lots: view.cap,
                    used_lots: view.used,
                    held_lots: view.held,
                    outside_lots: view.outside,
                    fill_bps: view.fill_bps,
                    rate_cents_per_lot: view.rate_cents,
                    outside_rate_cents_per_lot: lg::leg_cents(
                        dataset,
                        key.a,
                        key.b,
                        key.mode,
                        1,
                        world
                            .logistics
                            .at_price_level(dataset.logistics.outside_premium_bps),
                    )
                    .max(1),
                    hot_weeks: stat.map(|s| s.hot_weeks as u32).unwrap_or(0),
                    owners: view
                        .owners
                        .iter()
                        .map(|(firm, lots)| LaneOwnerDto {
                            firm: firm.0,
                            name: world
                                .firms
                                .get(*firm)
                                .map(|f| f.name.clone())
                                .unwrap_or_default(),
                            lots: *lots,
                            mine: mine(*firm),
                        })
                        .collect(),
                    contracts: view.contracts as u32,
                    my_capacity_lots: view
                        .owners
                        .iter()
                        .filter(|(f, _)| mine(*f))
                        .map(|(_, l)| *l)
                        .sum(),
                    my_shipped_lots: lane
                        .map(|l| {
                            l.users
                                .iter()
                                .filter(|(f, _)| mine(*f))
                                .map(|(_, lots)| *lots)
                                .sum()
                        })
                        .unwrap_or(0),
                    my_contract_lots: contracts_mine,
                    reservable_lots: primary
                        .map(|p| lg::reservable_lots(world, p, key))
                        .unwrap_or(0),
                }
            })
            .collect();
        out.sort_by(|x, y| {
            (y.capacity_lots + y.outside_lots)
                .cmp(&(x.capacity_lots + x.outside_lots))
                .then_with(|| (&x.a, &x.b, x.mode).cmp(&(&y.a, &y.b, y.mode)))
        });
        out
    }

    /// Every live lane contract.
    pub fn lane_contracts(&self) -> Vec<LaneContractDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        world
            .logistics
            .contracts
            .iter()
            .filter(|c| c.weeks_left > 0)
            .map(|c| LaneContractDto {
                id: c.id,
                holder: c.holder.0,
                holder_name: world
                    .firms
                    .get(c.holder)
                    .map(|f| f.name.clone())
                    .unwrap_or_default(),
                mine: cw_sim::economy::events_engine::player_side(world, c.holder),
                a: dataset.regions[c.key.a as usize].code.clone(),
                b: dataset.regions[c.key.b as usize].code.clone(),
                a_name: dataset.regions[c.key.a as usize].name.clone(),
                b_name: dataset.regions[c.key.b as usize].name.clone(),
                mode: c.key.mode as u32,
                mode_name: dataset.logistics.modes[c.key.mode as usize].name.clone(),
                lots: c.lots,
                held_lots: c.held,
                used_lots: c.used,
                fee_cents_per_lot: c.fee_per_lot_cents,
                weeks_left: c.weeks_left,
            })
            .collect()
    }

    /// The corridor's freight for the week just closed.
    pub fn freight_summary(&self) -> FreightSummaryDto {
        let session = self.lock();
        let world = &session.world;
        let st = &world.logistics;
        let last = &st.last;
        let moved: i64 = last.lots_by_mode.iter().sum();
        let outside: i64 = last.outside_lots_by_mode.iter().sum();
        FreightSummaryDto {
            carriers: st.carriers.len() as u32,
            lanes_with_capacity: st.lanes.values().filter(|l| l.cap() > 0).count() as u32,
            capacity_lots: st.lanes.values().map(|l| l.cap() as i64).sum(),
            moved_lots: moved,
            outside_share_bps: (outside * 10_000 / moved.max(1)) as i32,
            own_group_lots: last.own_lots,
            contract_lots: last.contract_lots,
            paid_cents: last.cents,
            congested_lanes: st.stats.values().filter(|s| s.hot_weeks >= 4).count() as u32,
        }
    }

    /// Freight for the week just closed, for one division (`facility` is one of its plants) or,
    /// with `facility` zero, for a whole firm including its lane contract fees. Policy choices
    /// are priced against the division's last week of shipments at today's lane prices.
    pub fn shipping_summary(&self, firm: u64, facility: u64) -> ShippingSummaryDto {
        use cw_sim::economy::logistics as lg;
        let mut guard = self.lock();
        let session = &mut *guard;
        let world = &mut session.world;
        let ds = &session.dataset;
        let mut firm = cw_core::ids::Handle(firm);
        let mut region: Option<u8> = None;
        let mut policy: Option<lg::ShipPolicy> = None;
        if facility != 0
            && let Some(plant) = world.facilities.get(cw_core::ids::Handle(facility))
        {
            firm = plant.firm;
            region = Some(plant.region);
            policy = Some(plant.ship);
        }
        let sum = lg::shipping_summary(world, ds, firm, region);
        let total = (sum.own_cents + sum.other_cents + sum.outside_cents).max(1);
        let name = |r: u8| ds.regions[r as usize].name.clone();
        let mode_lower = |m: u8| ds.logistics.modes[m as usize].name.to_lowercase();
        let lanes: Vec<ShippingLaneDto> = sum
            .lanes
            .iter()
            .map(|l| ShippingLaneDto {
                a: ds.regions[l.key.a as usize].code.clone(),
                b: ds.regions[l.key.b as usize].code.clone(),
                a_name: name(l.key.a),
                b_name: name(l.key.b),
                mode: l.key.mode as u32,
                mode_id: ds.logistics.modes[l.key.mode as usize].id.clone(),
                mode_name: ds.logistics.modes[l.key.mode as usize].name.clone(),
                lots: l.lots,
                cents: l.cents,
                outside_lots: l.outside_lots,
                fill_bps: l.fill_bps,
                congested: l.hot_weeks >= lg::SUSTAINED_WEEKS || l.fill_bps >= lg::CONGESTED_BPS,
            })
            .collect();
        let policy_text = match policy {
            None => "Each division picks its own route. Open a division to change it.".to_owned(),
            Some(p) => {
                let mut text = format!("{} route", p.kind.label());
                let top_mode = sum
                    .mode_lots
                    .iter()
                    .enumerate()
                    .filter(|(m, l)| **l > 0 && *m < ds.logistics.modes.len())
                    .max_by_key(|(m, l)| (**l, std::cmp::Reverse(*m)))
                    .map(|(m, _)| m as u8);
                match (top_mode, sum.lanes.first()) {
                    (Some(m), Some(l)) => {
                        text += &format!(
                            ": mostly {}, mainly {} to {}",
                            mode_lower(m),
                            name(l.key.a),
                            name(l.key.b)
                        )
                    }
                    _ => text += ": nothing was shipped in last week",
                }
                if let Some(m) = p.mode
                    && (m as usize) < ds.logistics.modes.len()
                {
                    text += &format!(". Prefers {} when a route allows it", mode_lower(m));
                }
                text + "."
            }
        };
        let kind_blurbs = [
            "Takes the lowest price each week, even when it is slower.",
            "Takes the quickest trip, even when it costs more.",
            "Fills your own carriers first, then pays others. Cheap when you own trucks, rail or ships.",
            "Avoids full lanes and outside carriers, so prices and delivery times hold steady.",
        ];
        let mut kinds: Vec<PolicyOptionDto> = Vec::new();
        let mut modes: Vec<PolicyOptionDto> = Vec::new();
        if let (Some(r), Some(current)) = (region, policy) {
            let base = lg::estimate_policy(world, ds, firm, r, current);
            let change = |est: Option<(i64, i64)>| -> (bool, i64, i32, i64) {
                match (est, base) {
                    (Some((c, o)), Some((b, _))) => {
                        let bps = if b > 0 {
                            ((c - b) * 10_000 / b) as i32
                        } else {
                            0
                        };
                        (true, c, bps, o)
                    }
                    _ => (false, 0, 0, 0),
                }
            };
            for code in 0..4u8 {
                let kind = lg::ShipKind::from_code(code).unwrap_or_default();
                let est =
                    lg::estimate_policy(world, ds, firm, r, lg::ShipPolicy { kind, ..current });
                let (has, cents, bps, outside) = change(est);
                kinds.push(PolicyOptionDto {
                    id: code as i32,
                    label: kind.label().to_owned(),
                    blurb: kind_blurbs[code as usize].to_owned(),
                    selected: current.kind == kind,
                    has_estimate: has,
                    weekly_cents: cents,
                    change_bps: bps,
                    outside_lots: outside,
                });
            }
            let est = lg::estimate_policy(
                world,
                ds,
                firm,
                r,
                lg::ShipPolicy {
                    mode: None,
                    ..current
                },
            );
            let (has, cents, bps, outside) = change(est);
            modes.push(PolicyOptionDto {
                id: -1,
                label: "Any".to_owned(),
                blurb: "No preference: the policy above decides.".to_owned(),
                selected: current.mode.is_none(),
                has_estimate: has,
                weekly_cents: cents,
                change_bps: bps,
                outside_lots: outside,
            });
            for m in 0..ds.logistics.modes.len().min(4) as u8 {
                let def = &ds.logistics.modes[m as usize];
                let est = lg::estimate_policy(
                    world,
                    ds,
                    firm,
                    r,
                    lg::ShipPolicy {
                        mode: Some(m),
                        ..current
                    },
                );
                let (has, cents, bps, outside) = change(est);
                modes.push(PolicyOptionDto {
                    id: m as i32,
                    label: if def.id == "sea" {
                        "Ship".to_owned()
                    } else {
                        def.name.clone()
                    },
                    blurb: format!(
                        "Uses {} whenever a route allows it, even if another mode is cheaper.",
                        def.name.to_lowercase()
                    ),
                    selected: current.mode == Some(m),
                    has_estimate: has,
                    weekly_cents: cents,
                    change_bps: bps,
                    outside_lots: outside,
                });
            }
        }
        let suggestion = sum.suggestion.as_ref().map(|s| match s {
            lg::Suggestion::Build {
                key,
                recipe,
                blocks,
                upfront_cents,
                weekly_saving_cents,
                payback_weeks,
            } => ShippingSuggestionDto {
                kind: "build".to_owned(),
                headline: format!(
                    "Build {} {} for {} to {}",
                    blocks,
                    ds.recipes[*recipe as usize].name.to_lowercase(),
                    name(key.a),
                    name(key.b)
                ),
                a: ds.regions[key.a as usize].code.clone(),
                b: ds.regions[key.b as usize].code.clone(),
                mode: key.mode as u32,
                recipe: *recipe as u32,
                lots: *blocks,
                upfront_cents: *upfront_cents,
                weekly_saving_cents: *weekly_saving_cents,
                payback_weeks: *payback_weeks,
            },
            lg::Suggestion::Reserve {
                key,
                lots,
                weekly_saving_cents,
                ..
            } => ShippingSuggestionDto {
                kind: "reserve".to_owned(),
                headline: format!(
                    "Reserve {} loads on {} to {} by {}",
                    lots,
                    name(key.a),
                    name(key.b),
                    mode_lower(key.mode)
                ),
                a: ds.regions[key.a as usize].code.clone(),
                b: ds.regions[key.b as usize].code.clone(),
                mode: key.mode as u32,
                recipe: 0,
                lots: *lots as i64,
                upfront_cents: 0,
                weekly_saving_cents: *weekly_saving_cents,
                payback_weeks: 0,
            },
        });
        ShippingSummaryDto {
            has_data: sum.lots > 0 || sum.fee_cents > 0,
            weekly_cents: sum.cents,
            own_cents: sum.own_cents,
            other_cents: sum.other_cents,
            outside_cents: sum.outside_cents,
            fee_cents: sum.fee_cents,
            outside_extra_cents: sum.outside_extra_cents,
            lots: sum.lots,
            outside_lots: sum.outside_lots,
            cents_per_lot: (sum.cents - sum.fee_cents) / sum.lots.max(1),
            own_share_bps: (sum.own_cents * 10_000 / total) as i32,
            other_share_bps: (sum.other_cents * 10_000 / total) as i32,
            outside_share_bps: (sum.outside_cents * 10_000 / total) as i32,
            policy_text,
            lanes,
            kinds,
            modes,
            suggestion,
        }
    }

    /// What reserving lots on a lane for a number of weeks would cost and save the player's
    /// group, before any command is sent.
    pub fn reserve_quote(
        &self,
        a: String,
        b: String,
        mode: u32,
        lots: i32,
        weeks: u32,
    ) -> ReserveQuoteDto {
        use cw_sim::economy::logistics as lg;
        let session = self.lock();
        let world = &session.world;
        let ds = &session.dataset;
        let none = ReserveQuoteDto {
            ok: false,
            fee_cents_per_lot: 0,
            spot_cents_per_lot: 0,
            weekly_fee_cents: 0,
            total_cents: 0,
            expected_use_lots: 0,
            weekly_net_cents: 0,
            max_lots: 0,
        };
        let index = |code: &str| ds.regions.iter().position(|r| r.code == code);
        let (Some(ai), Some(bi), Some(firm)) = (index(&a), index(&b), world.investor.primary_group)
        else {
            return none;
        };
        if mode as usize >= ds.logistics.modes.len() {
            return none;
        }
        let key = lg::LaneKey::new(ai as u8, bi as u8, mode as u8);
        match lg::reserve_quote(world, ds, firm, key, lots, weeks) {
            Some(q) => ReserveQuoteDto {
                ok: true,
                fee_cents_per_lot: q.fee_per_lot_cents,
                spot_cents_per_lot: q.spot_per_lot_cents,
                weekly_fee_cents: q.weekly_fee_cents,
                total_cents: q.total_cents,
                expected_use_lots: q.expected_use_lots,
                weekly_net_cents: q.weekly_net_cents,
                max_lots: q.max_lots,
            },
            None => none,
        }
    }

    /// A firm's storage in every region where it holds stock or owns space.
    pub fn storage_cells(&self, firm: u64) -> Vec<StorageCellDto> {
        let session = self.lock();
        let world = &session.world;
        world
            .firms
            .get(cw_core::ids::Handle(firm))
            .map(|f| storage_cells(world, &session.dataset, f, None))
            .unwrap_or_default()
    }

    /// Goods a firm holds back from sale, with any open buy order for them.
    pub fn held_stock(&self, firm: u64) -> Vec<HeldStockDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let Some(entity) = world.firms.get(cw_core::ids::Handle(firm)) else {
            return Vec::new();
        };
        let mut keys: std::collections::BTreeSet<(u16, u8)> =
            entity.storage.held.keys().copied().collect();
        keys.extend(entity.storage.orders.iter().map(|o| (o.product, o.region)));
        keys.into_iter()
            .map(|(product, region)| {
                let order = entity
                    .storage
                    .orders
                    .iter()
                    .find(|o| o.product == product && o.region == region);
                HeldStockDto {
                    product: product as u32,
                    product_name: dataset.products[product as usize].name.clone(),
                    region: dataset.regions[region as usize].code.clone(),
                    region_name: dataset.regions[region as usize].name.clone(),
                    held_lots: entity.storage.held_of(product, region).lots(),
                    order_lots_left: order.map(|o| o.lots_left).unwrap_or(0),
                    order_limit_bps: order.map(|o| o.limit_bps).unwrap_or(0),
                }
            })
            .collect()
    }

    /// A firm's storage use now and its fees, holding costs and spoilage over the last quarter.
    pub fn storage_summary(&self, firm: u64) -> StorageSummaryDto {
        let session = self.lock();
        let world = &session.world;
        let Some(entity) = world.firms.get(cw_core::ids::Handle(firm)) else {
            return StorageSummaryDto {
                used_lots: 0,
                capacity_lots: 0,
                outside_lots: 0,
                fee_cents_13w: 0,
                holding_cents_13w: 0,
                spoiled_cents_13w: 0,
                spoiled_lots_13w: 0,
                rent_cents_13w: 0,
            };
        };
        let quarter = entity.storage.recent(13);
        let last = entity.storage.last;
        StorageSummaryDto {
            used_lots: last.used_lots,
            capacity_lots: last.capacity_lots,
            outside_lots: last.outside_lots,
            fee_cents_13w: quarter.fee_cents,
            holding_cents_13w: quarter.holding_cents,
            spoiled_cents_13w: quarter.spoiled_cents,
            spoiled_lots_13w: quarter.spoiled_milli / 1_000,
            rent_cents_13w: quarter.rent_cents,
        }
    }

    /// The board's mandate in force for the player's primary company, if one was taken. The
    /// 1.2 quarterly objective this call used to return is now the chapter mandate; the full
    /// record is `hall_of_careers`.
    pub fn objective(&self) -> Option<ObjectiveDto> {
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let mandate = world.chapters.active.get(&world.investor.primary_group?)?;
        let (title, detail) = chapters::mandate_copy(dataset, &mandate.kind);
        let progress = cw_sim::economy::chapters::progress(world, dataset, mandate);
        Some(ObjectiveDto {
            title,
            detail,
            progress_bps: progress.bps as i32,
            due_week: mandate.due,
            weeks_left: mandate.due.saturating_sub(world.tick.0),
        })
    }

    /// Closed mandates of the player's chapters, newest first.
    pub fn objective_history(&self) -> Vec<ObjectiveResultDto> {
        use cw_sim::economy::chapters::Outcome;
        let session = self.lock();
        session
            .world
            .chapters
            .log
            .iter()
            .rev()
            .filter(|c| matches!(c.outcome, Some(Outcome::Met | Outcome::Missed)))
            .filter_map(|c| {
                let kind = c.offered.get(c.chosen? as usize)?;
                Some(ObjectiveResultDto {
                    title: chapters::mandate_copy(&session.dataset, kind).0,
                    met: c.outcome == Some(Outcome::Met),
                    closed_week: c.closed,
                    bonus_cents: c.bonus_cents,
                })
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
        if world.firms.get(handle).is_none() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for d in sectors::divisions_of(world, dataset, handle) {
            let (firms, _) = sectors::cell(world, dataset, d.region, d.industry);
            for (rival, _) in firms {
                if let Some(option) =
                    raid_option(world, dataset, handle, rival, d.region, d.industry)
                {
                    out.push(option);
                }
            }
        }
        out
    }

    /// Whether the firm can raid `target` in a market (region code, industry name), with the
    /// quoted option when it can and a one-line reason and fix when it cannot.
    pub fn raid_check(
        &self,
        firm: u64,
        target: u64,
        region: String,
        industry: String,
    ) -> RaidCheckDto {
        use cw_sim::economy::sectors;
        let session = self.lock();
        let world = &session.world;
        let dataset = &session.dataset;
        let me = cw_core::ids::Handle(firm);
        let rival = cw_core::ids::Handle(target);
        let no = |kind: &str, reason: String, needs_build: bool| RaidCheckDto {
            raidable: false,
            kind: kind.to_owned(),
            reason,
            needs_build,
            option: None,
        };
        let region_index = dataset.regions.iter().position(|r| r.code == region);
        let industry_index = cw_sim::state::INDUSTRY_NAMES
            .iter()
            .position(|n| *n == industry);
        let (Some(region_index), Some(industry_index)) = (region_index, industry_index) else {
            return no(
                "unknown",
                "That market could not be found.".to_owned(),
                false,
            );
        };
        let (region_index, industry_index) = (region_index as u8, industry_index as u8);
        let (Some(_), Some(rival_firm)) = (world.firms.get(me), world.firms.get(rival)) else {
            return no("unknown", "That company is gone.".to_owned(), false);
        };
        let rival_name = rival_firm.name.clone();
        let market = format!(
            "{} in {}",
            industry.replace('_', " "),
            dataset.regions[region_index as usize].name
        );
        if me == rival || cw_sim::economy::ownership::same_group(world, me, rival) {
            return no(
                "own_group",
                format!("{rival_name} is part of your own group."),
                false,
            );
        }
        if cw_sim::economy::fx::require_same_currency(world, me, rival).is_err() {
            return no(
                "other_currency",
                format!("{rival_name} keeps its books in another currency, so you cannot raid it."),
                false,
            );
        }
        if sectors::division_plants(world, dataset, me, region_index, industry_index).is_empty() {
            return no(
                "no_division",
                format!("You have no plant in {market}. Build one there to raid {rival_name}."),
                true,
            );
        }
        match raid_option(world, dataset, me, rival, region_index, industry_index) {
            Some(option) => RaidCheckDto {
                raidable: true,
                kind: "ok".to_owned(),
                reason: String::new(),
                needs_build: false,
                option: Some(option),
            },
            None => no(
                "rival_absent",
                format!("{rival_name} has no plants in {market} to take."),
                false,
            ),
        }
    }

    /// Rival groups for the CEO Office card: threats, allies and who can be raided now.
    pub fn rivals(&self, firm: u64) -> Vec<RivalSummaryDto> {
        cast::rivals(&self.lock(), firm)
    }

    /// The tag for one company: its chief, temperament, and any grudge or tie with the firm.
    pub fn rival_tag(&self, firm: u64, you: u64) -> Option<RivalTagDto> {
        let session = self.lock();
        cast::rival_tag(
            &session.world,
            &session.dataset,
            cw_core::ids::Handle(firm),
            cw_core::ids::Handle(you),
        )
    }

    /// The latest rival moves anywhere, newest first, naming the chief behind each.
    pub fn rival_feed(&self, limit: u32) -> Vec<RivalMoveFeedDto> {
        let session = self.lock();
        cast::rival_feed(&session.world, &session.dataset, limit as usize)
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
    /// The corridor commodity board: every traded good with its price and movement.
    pub fn commodity_board(&self) -> Vec<CommodityDto> {
        let session = self.lock();
        cw_session::dto::commodity_board(&session.world, &session.dataset)
            .into_iter()
            .map(|line| CommodityDto {
                product: line.product as u32,
                product_id: line.product_id,
                name: line.name,
                sector: line.sector,
                price_cents: line.price_cents,
                reference_cents: line.reference_cents,
                change_week_bps: line.change_week_bps,
                change_quarter_bps: line.change_quarter_bps,
                history_cents: line.history_cents,
                made_lots: line.made_lots,
                sold_lots: line.sold_lots,
                imported_lots: line.imported_lots,
                exported_lots: line.exported_lots,
                stock_lots: line.stock_lots,
                import_cents: line.import_cents,
                export_cents: line.export_cents,
                yours: line.yours,
            })
            .collect()
    }

    /// The corridor stock board: every operating company's shares.
    pub fn stock_board(&self) -> Vec<StockDto> {
        let session = self.lock();
        cw_session::dto::stock_board(&session.world)
            .into_iter()
            .map(stock_dto)
            .collect()
    }

    /// Every good the firm makes with its price, cost per lot, sales and posture.
    pub fn product_lines(&self, firm: u64) -> Vec<ProductLineDto> {
        let session = self.lock();
        cw_session::dto::product_lines(&session.world, &session.dataset, firm)
            .into_iter()
            .map(|line| ProductLineDto {
                product: line.product as u32,
                product_id: line.product_id,
                name: line.name,
                price_cents: line.price_cents,
                reference_cents: line.reference_cents,
                market_cents: line.market_cents,
                unit_cost_cents: line.unit_cost_cents,
                made_last_week_lots: line.made_last_week_lots,
                sold_last_week_lots: line.sold_last_week_lots,
                stock_lots: line.stock_lots,
                mode: line.mode,
                posture_bps: line.posture_bps,
            })
            .collect()
    }

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
                attacker_ceo: cast::who(world, dataset, r.attacker).0,
                attacker_temperament: cast::who(world, dataset, r.attacker).1,
                defender_ceo: cast::who(world, dataset, r.defender).0,
                defender_temperament: cast::who(world, dataset, r.defender).1,
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
        let handle = cw_core::ids::Handle(firm);
        let owned =
            cw_sim::economy::operations::owned_logistics_divisions(world, &session.dataset, handle);
        let threshold = cw_sim::economy::operations::sprawl_onset(
            entity.logistics_strength,
            owned,
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

    /// Everyone in the character cast.
    pub fn cast_people(&self) -> Vec<PersonDto> {
        cast::people(&self.lock().dataset)
    }

    /// A company's dossier: its chief executive, their style and its recent moves.
    pub fn dossier(&self, firm: u64) -> Option<DossierDto> {
        cast::dossier(&self.lock(), firm)
    }

    /// A company's full page: who runs it, how big it is and, by what the investor may see, its
    /// shares, accounts, plants, holders, structure, recent moves and news.
    pub fn company_page(&self, firm: u64) -> Option<company::CompanyPageDto> {
        company::page(&self.lock(), firm)
    }

    /// The economy, central bank and government.
    pub fn economy(&self) -> EconomyDto {
        let session = self.lock();
        let world = &session.world;
        let m = &world.macro_state;
        let c = &m.cycle;
        let p = &world.politics;
        EconomyDto {
            phase: c.phase.key().to_owned(),
            growth_bps: m.real_growth_bps,
            output_gap_bps: c.output_gap_bps,
            inflation_bps: m.inflation_bps,
            expected_inflation_bps: c.expected_inflation_bps,
            target_inflation_bps: c.target_inflation_bps,
            policy_rate_bps: m.policy_rate_bps,
            prime_rate_bps: cw_sim::economy::macro_cycle::prime_rate_bps(world),
            ten_year_yield_bps: cw_sim::economy::macro_cycle::ten_year_yield_bps(world),
            price_index_bps: m.price_index_bps,
            credit_bps: c.credit_bps,
            next_meeting_week: c.next_meeting,
            chair_name: c.chair.name.clone(),
            chair_stance: c.chair.stance_label().to_owned(),
            chair_term_ends: c.chair.term_ends,
            chair_person: c
                .chair
                .person
                .and_then(|p| session.dataset.cast.get(p as usize))
                .map(|p| p.id.clone())
                .unwrap_or_default(),
            posts: cast::posts(world, &session.dataset),
            decisions: c
                .decisions
                .iter()
                .rev()
                .take(12)
                .map(|d| RateDecisionDto {
                    week: d.week,
                    change_bps: d.change_bps,
                    rate_bps: d.rate_bps,
                    reason: d.reason.clone(),
                })
                .collect(),
            history: c
                .history
                .iter()
                .map(|h| MacroPointDto {
                    week: h.week,
                    gdp_index: h.gdp_index,
                    output_gap_bps: h.output_gap_bps,
                    inflation_bps: h.inflation_bps,
                    policy_rate_bps: h.policy_rate_bps,
                })
                .collect(),
            party_name: cw_sim::economy::politics::party_name(p.party)
                .trim_start_matches("the ")
                .to_owned(),
            party: p.party.key().to_owned(),
            approval_bps: p.approval_bps,
            next_election_week: p.next_election,
            laws: laws_dto(&p.laws, &session.dataset),
            pending_laws: p
                .pending
                .map(|change| laws_dto(&change.laws, &session.dataset)),
            pending_effective_week: p.pending.map(|change| change.effective).unwrap_or(0),
            news: p
                .news
                .iter()
                .rev()
                .take(12)
                .map(|n| NewsDto {
                    week: n.week,
                    text: n.text.clone(),
                    person: n
                        .person
                        .and_then(|p| session.dataset.cast.get(p as usize))
                        .map(|p| p.id.clone()),
                    firm: company::firm_named(&session.world, &n.text).map(|(h, _)| h),
                    firm_name: company::firm_named(&session.world, &n.text).map(|(_, name)| name),
                })
                .collect(),
            reserve: cw_sim::economy::reserve::strategic_goods(&session.dataset)
                .into_iter()
                .filter(|code| p.reserve.lots_of(*code) > 0)
                .map(|code| ReserveLineDto {
                    product: session.dataset.products[code as usize].id.clone(),
                    label: session.dataset.products[code as usize].name.clone(),
                    lots: p.reserve.lots_of(code),
                    phase: match cw_sim::economy::reserve::phase(world, &session.dataset, code) {
                        cw_sim::economy::reserve::Phase::Buying => "buying",
                        cw_sim::economy::reserve::Phase::Selling => "selling",
                        cw_sim::economy::reserve::Phase::Idle => "holding",
                    }
                    .to_owned(),
                })
                .collect(),
            reserve_value_cents: p.reserve.value_cents(),
            quotas: p
                .quota_active
                .iter()
                .map(|(region, product)| QuotaDto {
                    region_name: session.dataset.regions[*region as usize].name.clone(),
                    product: session.dataset.products[*product as usize].id.clone(),
                    label: session.dataset.products[*product as usize].name.clone(),
                    cap_bps: p.laws.quota_bps,
                })
                .collect(),
            incidents: world
                .systemic
                .incidents
                .iter()
                .rev()
                .take(8)
                .map(|i| IncidentDto {
                    week: i.week,
                    kind: cw_sim::economy::systemic::kind_key(i.kind).to_owned(),
                    text: i.text.clone(),
                    weeks_left: (i.week + 1 + i.weeks as u32).saturating_sub(world.tick.0),
                    touches_you: i
                        .firms
                        .iter()
                        .any(|f| cw_sim::economy::events_engine::player_side(world, *f)),
                })
                .collect(),
            state_firms: world
                .firms
                .iter()
                .filter(|(_, f)| f.state_owned_since.is_some() && f.operative())
                .map(|(_, f)| f.name.clone())
                .collect(),
            price_path: world.world_prices.mode.key().to_owned(),
            world_prices: world
                .world_prices
                .families
                .iter()
                .enumerate()
                .map(|(index, family)| WorldPriceDto {
                    family: family.key.clone(),
                    label: family.label.clone(),
                    index_bps: family.index_bps,
                    trend_bps: family.trend_bps(),
                    products: world
                        .world_prices
                        .product_family
                        .iter()
                        .filter(|(_, f)| **f as usize == index)
                        .filter_map(|(code, _)| session.dataset.products.get(*code as usize))
                        .map(|product| product.id.clone())
                        .collect(),
                })
                .collect(),
            fx: session.fx_view().map(fx_dto),
        }
    }

    /// Choose the currency the investor's figures are shown in. False for an unknown id.
    pub fn set_reporting_currency(&self, id: String) -> bool {
        self.lock().set_reporting_currency(&id)
    }

    /// The CFO's top reasons profit is down. Empty unless the company is losing money.
    pub fn cfo_advice(&self, firm: u64) -> Vec<CfoAdviceDto> {
        let session = self.lock();
        cw_session::cfo::advice(&session.world, &session.dataset, firm)
            .into_iter()
            .map(|item| CfoAdviceDto {
                key: item.key,
                headline: item.headline,
                detail: item.detail,
                weekly_cents: item.weekly_cents,
                action_label: item.action_label,
                commands: item.commands,
            })
            .collect()
    }

    /// A company's research position.
    pub fn research(&self, firm: u64) -> Option<ResearchDto> {
        use cw_sim::economy::research::{MAX_TECH_LEVEL, intensity_bps, quality, threshold};
        let session = self.lock();
        let world = &session.world;
        let entity = world.firms.get(cw_core::ids::Handle(firm))?;
        let level = entity.tech_level;
        let next = (level + 1).min(MAX_TECH_LEVEL);
        Some(ResearchDto {
            firm,
            level: level as u32,
            max_level: MAX_TECH_LEVEL as u32,
            frontier: world.macro_state.tech_frontier as u32,
            progress_cents: entity.tech_progress_cents,
            next_threshold_cents: threshold(level),
            success_bps: 7_500,
            research_budget_cents: entity.policies.research_budget_cents,
            marketing_budget_cents: entity.policies.marketing_budget_cents,
            intensity_bps: intensity_bps(level),
            next_intensity_bps: intensity_bps(next),
            quality_bps: quality(level, 0),
            next_quality_bps: quality(next, 0),
        })
    }

    pub fn finance(&self, firm: u64) -> Option<FinanceDto> {
        match self.lock().query(Query::Finance(firm)) {
            cw_session::Detail::Finance(row) => Some(finance_dto(row)),
            _ => None,
        }
    }

    /// A firm's results in four-week periods, oldest first, up to five years.
    pub fn history(&self, firm: u64) -> Vec<PeriodDto> {
        let session = self.lock();
        session
            .world
            .firm_history
            .get(&cw_core::ids::Handle(firm))
            .map(|periods| {
                periods
                    .iter()
                    .map(|p| PeriodDto {
                        first_week: p.first_week,
                        weeks: p.weeks,
                        revenue_cents: p.revenue,
                        cost_of_goods_cents: p.cost_of_goods,
                        wages_cents: p.wages,
                        operating_expense_cents: p.operating_expense,
                        depreciation_cents: p.depreciation,
                        net_interest_cents: p.net_interest,
                        tax_cents: p.tax,
                        other_cents: p.other,
                        net_income_cents: p.net_income,
                        cash_cents: p.cash,
                        assets_cents: p.assets,
                        liabilities_cents: p.liabilities,
                        equity_cents: p.equity,
                        share_price_cents: p.share_price_cents,
                        shares_issued: p.shares_issued,
                    })
                    .collect()
            })
            .unwrap_or_default()
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
                lock: false,
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
    fn the_paper_and_ticker_cross_the_boundary() {
        let career = SessionHandle::start(NewGameDto {
            seed: 9,
            scenario_id: "texas-1981".to_owned(),
            guided: true,
            five_year: false,
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            price_path: String::new(),
            timeline: String::new(),
        })
        .unwrap();
        let mut left = 20;
        while career.overview().tick < 20 && left > 0 {
            career.advance(4);
            career.dismiss_pause();
            left -= 1;
        }
        let now = career.overview().tick;
        let paper = career.paper(now);
        assert_eq!(paper.masthead, "The Gulf Ledger");
        assert_eq!(paper.week, now);
        assert!(paper.markets.open);
        let ticker = career.ticker();
        assert!(ticker.len() <= 3);
        assert_eq!(
            ticker.len(),
            paper.lead.iter().chain(&paper.briefs).take(3).count()
        );
        let archive = career.paper_archive();
        assert_eq!(archive.first(), Some(&now));
        assert_eq!(
            career.paper(archive[archive.len() - 1]).week,
            archive[archive.len() - 1]
        );
        assert_eq!(career.paper(now + 50).week, now, "a future week is today");
    }

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
    fn raid_check_explains_why_and_the_rivals_card_names_the_chief() {
        let session = SessionHandle::new(181, true, true).unwrap();
        session.advance(4);
        let me = session.overview().primary_firm.unwrap();
        let rivals = session.rivals(me);
        assert!(!rivals.is_empty(), "the world has rivals to show");
        assert!(rivals.iter().any(|r| !r.tag.ceo_name.is_empty()));
        // A rival checked in a sector and region where we have no plant must say to build.
        let targets = session.raid_targets(me);
        let rival = rivals[0].tag.firm;
        let check = session.raid_check(me, rival, "ZZZ".to_owned(), "retail".to_owned());
        assert!(!check.raidable);
        assert_eq!(check.kind, "unknown");
        if let Some(option) = targets.first() {
            let ok = session.raid_check(
                me,
                option.target,
                option.region.clone(),
                option.industry.clone(),
            );
            assert!(ok.raidable && ok.option.is_some());
        }
        let own = session.raid_check(me, me, "ZZZ".to_owned(), "retail".to_owned());
        assert!(!own.raidable);
    }

    #[test]
    fn shipping_summaries_and_reserve_quotes_cross_the_boundary() {
        let session = SessionHandle::new(181, true, true).unwrap();
        session.advance(8);
        let firm = session.overview().primary_firm.expect("a primary company");
        // The whole company, then each division by one of its plants.
        let company = session.shipping_summary(firm, 0);
        assert!(company.kinds.is_empty() && company.modes.is_empty());
        assert_eq!(
            company.own_share_bps + company.other_share_bps + company.outside_share_bps > 0,
            company.weekly_cents - company.fee_cents > 0
        );
        for division in session.divisions(firm) {
            let summary = session.shipping_summary(firm, division.plants[0]);
            assert_eq!(summary.kinds.len(), 4);
            assert_eq!(summary.modes.len(), 5);
            assert_eq!(summary.kinds.iter().filter(|k| k.selected).count(), 1);
            assert_eq!(summary.modes.iter().filter(|k| k.selected).count(), 1);
            assert!(summary.policy_text.contains("route"));
        }
        // A quote for a lane that exists is the engine's, and an unknown lane is not ok.
        if let Some(lane) = session.lanes().into_iter().find(|l| l.capacity_lots > 0) {
            let quote = session.reserve_quote(lane.a.clone(), lane.b.clone(), lane.mode, 10, 13);
            assert!(quote.ok);
            assert_eq!(quote.total_cents, quote.weekly_fee_cents * 13);
        }
        assert!(
            !session
                .reserve_quote("none".into(), "nada".into(), 0, 10, 13)
                .ok
        );
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
        // player's own plant. Ten rivals also open with extra plants (seventeen in all) so the
        // oil, gas, iron and food chains start with local producers.
        assert_eq!(
            plants.iter().filter(|p| !p.player).count(),
            28 + 17,
            "every rival plant is on the map"
        );
        assert!(
            plants.iter().any(|p| p.player),
            "the player's plant is marked"
        );
        let catalogue = strategies();
        assert_eq!(catalogue.len(), 69);
        // A refinery lists its by-products for the build screens.
        let refinery = catalogue.iter().find(|s| s.id == "refinery").unwrap();
        assert_eq!(refinery.by_products.len(), 3);
        assert!(
            refinery
                .by_products_text
                .starts_with("plus 36 lots of Gasoline")
        );
        assert!(
            catalogue
                .iter()
                .all(|s| !s.name.is_empty() && s.weekly_output_lots > 0)
        );
        let ops = session.firm_ops(firm).expect("ops for the player's firm");
        // Sprawl is a smooth curve now; this is where it reaches one percent of revenue.
        assert_eq!(ops.sprawl_threshold, 24);
    }

    #[test]
    fn the_catalogues_carry_the_prices_of_the_scenario_they_are_asked_for() {
        let price = |list: &[ProductDto], name: &str| {
            list.iter()
                .find(|p| p.name == name)
                .map(|p| p.reference_price_cents)
                .unwrap()
        };
        let lakes = products_for("great-lakes-2000".to_owned());
        let texas = products_for("texas-1981".to_owned());
        let valley = products_for("silicon-valley-1995".to_owned());
        assert_eq!(products().len(), lakes.len());
        assert!(price(&texas, "Crude oil") > price(&lakes, "Crude oil"));
        assert!(price(&valley, "Crude oil") < price(&lakes, "Crude oil"));
        // The margin the build form quotes follows the same prices.
        let margin = |list: &[StrategyDto]| {
            list.iter()
                .find(|s| s.id == "oil_wells")
                .unwrap()
                .reference_margin_bps
        };
        assert_ne!(
            margin(&strategies_for("texas-1981".to_owned())),
            margin(&strategies())
        );
        assert_eq!(recipes_for("texas-1981".to_owned()).len(), recipes().len());
        // A running career reads its own scenario.
        let career = SessionHandle::start(NewGameDto {
            seed: 3,
            scenario_id: "texas-1981".to_owned(),
            guided: true,
            five_year: false,
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            price_path: String::new(),
            timeline: String::new(),
        })
        .unwrap();
        assert_eq!(career.scenario_id(), "texas-1981");
        assert_eq!(
            price(&career.products(), "Crude oil"),
            price(&texas, "Crude oil")
        );
    }

    /// The timeline DTO lists announced events ahead and events that happened, and never an
    /// event that has not been announced.
    #[test]
    fn the_timeline_lists_only_announced_events_ahead_and_the_past_with_dates() {
        let start = |timeline: &str| {
            SessionHandle::start(NewGameDto {
                seed: 3,
                scenario_id: "texas-1981".to_owned(),
                guided: true,
                five_year: false,
                difficulty: "standard".to_owned(),
                economy: "historic".to_owned(),
                starting_capital_cents: None,
                price_path: String::new(),
                timeline: timeline.to_owned(),
            })
            .unwrap()
        };
        let off = start("off");
        let on = start("");
        assert_eq!(off.timeline().mode, "off");
        assert_eq!(on.timeline().mode, "historical");
        // Texas: tight money in 1981 week 30, noticed 6 weeks ahead from week 24.
        let step = |handle: &SessionHandle, until: u32| {
            while handle.lock().world.tick.0 < until {
                handle.advance(1);
                let mut session = handle.lock();
                for event in session.world.events.pending.iter_mut() {
                    event.resolved = true;
                }
                session.dismiss_pause();
            }
        };
        step(&on, 23);
        let t = on.timeline();
        assert!(
            t.upcoming.is_empty() && t.past.is_empty(),
            "nothing is announced yet"
        );
        step(&on, 24);
        let t = on.timeline();
        assert_eq!(t.upcoming.len(), 1);
        assert_eq!(t.upcoming[0].id, "tx-tight-money");
        assert_eq!(
            (
                t.upcoming[0].year,
                t.upcoming[0].week_of_year,
                t.upcoming[0].weeks_away
            ),
            (1981, 30, 6)
        );
        step(&on, 30);
        let t = on.timeline();
        assert!(t.upcoming.is_empty());
        assert_eq!(
            (
                t.past[0].id.as_str(),
                t.past[0].year,
                t.past[0].week_of_year
            ),
            ("tx-tight-money", 1981, 30)
        );
        step(&off, 30);
        let t = off.timeline();
        assert!(t.upcoming.is_empty() && t.past.is_empty());
    }

    /// Every firm has a face, a name and a logo, and a page shows only what the investor may know.
    #[test]
    fn every_firm_has_a_ceo_a_logo_and_a_page_that_follows_the_information_rules() {
        for id in ["texas-1981", "silicon-valley-1995", "ruhr-rhine-1991"] {
            let session = SessionHandle::start(NewGameDto {
                seed: 11,
                scenario_id: id.to_owned(),
                guided: true,
                five_year: false,
                difficulty: "standard".to_owned(),
                economy: "historic".to_owned(),
                starting_capital_cents: None,
                price_path: String::new(),
                timeline: String::new(),
            })
            .unwrap();
            session.advance(3);
            let (mut ceos, mut kinds) = (std::collections::BTreeSet::new(), [0u32; 3]);
            for firm in session.firms() {
                let page = session.company_page(firm.handle).expect("page");
                assert!(!page.rows_empty_logo(), "{id} {}", firm.name);
                match page.kind.as_str() {
                    "yours" => {
                        kinds[0] += 1;
                        assert!(page.finance.is_some() && page.ceo_is_you);
                    }
                    "listed" => {
                        kinds[1] += 1;
                        assert!(page.stock.is_some());
                    }
                    _ => {
                        kinds[2] += 1;
                        assert!(page.stock.is_none());
                        if !session.lock().can_inspect(firm.handle) {
                            assert!(page.finance.is_none() && page.holders.is_empty());
                            assert!(page.divisions.is_empty());
                        }
                    }
                }
                if page.kind != "yours" {
                    let ceo = page.ceo.expect("every other firm has a chief executive");
                    assert!(ceos.insert(ceo.id.clone()), "{id}: {} shared", ceo.id);
                }
                assert!(
                    !page.regions.is_empty() || page.kind != "listed" || page.size_band == "Small"
                );
            }
            assert!(kinds[1] > 0, "{id}: {kinds:?}");
        }
    }
}
