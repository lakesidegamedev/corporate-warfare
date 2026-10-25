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
    pub state: String,
    pub offer_price_cents: i64,
    pub escrow_cents: i64,
    pub committed_shares: i64,
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
}

#[derive(uniffi::Record)]
pub struct ProductDto {
    pub index: u32,
    pub name: String,
    pub reference_price_cents: i64,
    pub kind: String,
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
        state: line.state,
        offer_price_cents: line.offer_price_cents,
        escrow_cents: line.escrow_cents,
        committed_shares: line.committed_shares,
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

    pub fn revision(&self) -> u64 {
        self.lock().revision()
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

    /// Submit any catalogue command as JSON. Returns a typed rejection instead of throwing.
    pub fn submit_json(&self, json: String) -> ReceiptDto {
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
        self.submit_intent(intent)
    }

    /// Set the selected firm's asking price for one product.
    pub fn set_price(&self, firm: u64, product: u32, price_cents: i64) -> ReceiptDto {
        self.submit_intent(cw_sim::commands::CommandIntent::SetPrice {
            firm: cw_core::ids::Handle(firm),
            product: product as u16,
            price_cents,
        })
    }

    /// Set the weekly wage offer for one skill tier.
    pub fn set_wage_offer(&self, firm: u64, tier: u32, wage_cents: i64) -> ReceiptDto {
        self.submit_intent(cw_sim::commands::CommandIntent::SetWageOffer {
            firm: cw_core::ids::Handle(firm),
            tier: tier as u8,
            wage_cents,
        })
    }

    /// Set the production target across the firm's operating facilities.
    pub fn set_production_target(&self, firm: u64, target_bps: i32) -> ReceiptDto {
        self.submit_intent(cw_sim::commands::CommandIntent::SetProductionTarget {
            firm: cw_core::ids::Handle(firm),
            target_bps,
        })
    }

    /// Build a facility from a template index in the published product list.
    pub fn build_facility(&self, firm: u64, region: u32, recipe: u32, blocks: u32) -> ReceiptDto {
        self.submit_intent(cw_sim::commands::CommandIntent::BuildFacility {
            firm: cw_core::ids::Handle(firm),
            region: region as u8,
            recipe: recipe as u16,
            blocks: blocks as u8,
        })
    }

    /// Pay a dividend from the firm's distributable retained earnings.
    pub fn pay_dividend(&self, firm: u64, amount_cents: i64) -> ReceiptDto {
        self.submit_intent(cw_sim::commands::CommandIntent::PayDividend {
            firm: cw_core::ids::Handle(firm),
            amount_cents,
        })
    }
}

impl SessionHandle {
    fn submit_intent(&self, intent: cw_sim::commands::CommandIntent) -> ReceiptDto {
        let mut session = self.lock();
        let revision = session.revision();
        let envelope = cw_sim::commands::CommandEnvelope {
            command_id_high: 0x5357_4946_5400_0000,
            command_id_low: revision,
            actor: 0,
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
}
