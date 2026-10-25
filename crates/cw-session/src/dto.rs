//! Owned, immutable display models. Swift never computes a financial identity.

use cw_data::Dataset;
use cw_sim::state::{DealState, FacilityStatus, FirmStatus, HolderId, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Overview {
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
    pub alerts: Vec<Alert>,
    pub pause: Option<String>,
    /// One primary recommended next action and the reason for it.
    pub next_action_key: String,
    pub next_action_reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub key: String,
    pub subject: String,
    pub severity: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirmSummary {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FacilitySummary {
    pub handle: u64,
    pub firm: u64,
    pub recipe: String,
    pub blocks: u8,
    pub region: String,
    pub status: String,
    pub capacity: i64,
    pub last_output: i64,
    pub workers: [i64; 3],
    pub bottleneck: String,
    pub deposit_remaining: i64,
    pub target_bps: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionSummary {
    pub code: String,
    pub name: String,
    pub population: i64,
    pub employed: i64,
    pub prevailing_wage_cents: [i64; 3],
    pub households_savings_cents: i64,
    pub demand_index_bps: i32,
    pub land_index_bps: i32,
    pub organisation_bps: i32,
    pub distance_km_from_selected: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FinanceSummary {
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
    pub cash_bridge: CashBridge,
    pub recent_weeks: Vec<WeekLine>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CashBridge {
    pub opening_cents: i64,
    pub receipts_cents: i64,
    pub operating_payments_cents: i64,
    pub investment_cents: i64,
    pub financing_cents: i64,
    pub tax_cents: i64,
    pub closing_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeekLine {
    pub week: u32,
    pub revenue_cents: i64,
    pub ebit_cents: i64,
    pub net_income_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OwnershipSummary {
    pub firm: u64,
    pub issued: i64,
    pub treasury: i64,
    pub eligible_votes: i64,
    pub controller: Option<String>,
    pub holders: Vec<HolderLine>,
    pub board_seats: [Option<String>; 5],
    pub next_meeting: u32,
    pub ceo: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HolderLine {
    pub holder: String,
    pub quantity: i64,
    pub votes_bps: i32,
    pub cost_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractLine {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoanLine {
    pub handle: u64,
    pub borrower: String,
    pub lender: String,
    pub principal_cents: i64,
    pub rate_bps: i32,
    pub remaining_weeks: u32,
    pub secured: bool,
    pub defaulted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InsuranceLine {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DealLine {
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
    /// The next boundary the player must act before, when the state has one.
    pub deadline: Option<u32>,
    pub review_deadline: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventLine {
    pub id: u64,
    pub card: String,
    pub title: String,
    pub subject: String,
    pub mandatory: bool,
    pub deadline: u32,
    pub choices: Vec<ChoiceLine>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChoiceLine {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketLine {
    pub product: String,
    pub reference_price_cents: i64,
    pub last_price_cents: i64,
    pub regional_stock_lots: i64,
    pub last_week_sales_lots: i64,
    pub gateway_buy_cents: i64,
    pub gateway_sell_cents: i64,
}

fn holder_name(world: &World, holder: HolderId) -> String {
    match holder {
        HolderId::Investor => "You".to_owned(),
        HolderId::Firm(handle) => world
            .firms
            .get(handle)
            .map(|firm| firm.name.clone())
            .unwrap_or_else(|| "Firm".to_owned()),
        HolderId::Outside(index) => world
            .outside
            .investors
            .get(index as usize)
            .map(|investor| investor.name.clone())
            .unwrap_or_else(|| "Institution".to_owned()),
        HolderId::Treasury => "Treasury".to_owned(),
    }
}

pub fn firm_summary(world: &World, _dataset: &Dataset, handle: u64) -> Option<FirmSummary> {
    let firm = world.firms.get(cw_core::ids::Handle(handle))?;
    Some(FirmSummary {
        handle,
        name: firm.name.clone(),
        industry: cw_sim::state::INDUSTRY_NAMES[firm.industry as usize].to_owned(),
        region: world.regions[firm.region as usize].name.clone(),
        cash_cents: firm.book.cash(),
        equity_cents: firm.book.total_equity(),
        fair_value_cents: firm.shares.fair_cents * firm.shares.eligible_votes(),
        mark_cents: firm.shares.mark_cents,
        grade: firm.grade.key().to_owned(),
        status: match firm.status {
            FirmStatus::Operating => "operating".to_owned(),
            FirmStatus::PastDue { .. } => "past_due".to_owned(),
            FirmStatus::Restructuring { .. } => "restructuring".to_owned(),
            FirmStatus::Liquidating => "liquidating".to_owned(),
            FirmStatus::Merged { .. } => "merged".to_owned(),
            FirmStatus::Dissolved => "dissolved".to_owned(),
        },
        investor_votes_bps: firm.shares.vote_bps(HolderId::Investor),
        controlled: firm.shares.controller() == Some(HolderId::Investor),
        ceo_is_investor: matches!(firm.board.ceo, Some(cw_sim::state::CeoRef::Investor)),
        facilities: firm.facilities.len() as u32,
    })
}

pub fn facility_summary(world: &World, dataset: &Dataset, handle: u64) -> Option<FacilitySummary> {
    let facility = world.facilities.get(cw_core::ids::Handle(handle))?;
    let template = &dataset.recipes[facility.recipe as usize];
    let bottleneck = if facility.last_staff_ratio_bps < 9_000 {
        "staffing"
    } else if facility.last_inputs_short {
        "inputs"
    } else if facility.target_bps < 10_000 {
        "policy target"
    } else {
        "none"
    };
    Some(FacilitySummary {
        handle,
        firm: facility.firm.0,
        recipe: template.id.clone(),
        blocks: facility.blocks,
        region: world.regions[facility.region as usize].name.clone(),
        status: match facility.status {
            FacilityStatus::Building => "building".to_owned(),
            FacilityStatus::Retooling { .. } => "retooling".to_owned(),
            FacilityStatus::Operating => "operating".to_owned(),
            FacilityStatus::Mothballed => "mothballed".to_owned(),
        },
        capacity: facility.full_weekly_output(dataset).lots(),
        last_output: facility.last_output.lots(),
        workers: facility.workers,
        bottleneck: bottleneck.to_owned(),
        deposit_remaining: facility.deposit_remaining.lots(),
        target_bps: facility.target_bps,
    })
}

pub fn region_summary(
    world: &World,
    dataset: &Dataset,
    index: usize,
    from: Option<usize>,
) -> RegionSummary {
    let region = &world.regions[index];
    let population: i64 = region.labour.iter().map(|tier| tier.population).sum();
    let employed: i64 = region.labour.iter().map(|tier| tier.employed).sum();
    RegionSummary {
        code: region.code.clone(),
        name: region.name.clone(),
        population,
        employed,
        prevailing_wage_cents: [
            region.labour[0].prevailing_wage_cents,
            region.labour[1].prevailing_wage_cents,
            region.labour[2].prevailing_wage_cents,
        ],
        households_savings_cents: region.households.savings_cents,
        demand_index_bps: region.demand_index_bps,
        land_index_bps: region.land_index_bps,
        organisation_bps: region.organisation_bps,
        distance_km_from_selected: from
            .and_then(|from| world.regions.get(from))
            .and_then(|other| dataset.lane_km(&other.code, &region.code))
            .unwrap_or(0) as i64,
    }
}

pub fn finance_summary(world: &World, handle: u64) -> Option<FinanceSummary> {
    let firm = world.firms.get(cw_core::ids::Handle(handle))?;
    let recent: Vec<WeekLine> = firm
        .book
        .recent_weeks()
        .iter()
        .map(|week| WeekLine {
            week: week.week.0,
            revenue_cents: week.revenue,
            ebit_cents: week.ebit(),
            net_income_cents: week.net_income,
        })
        .collect();
    let revenue: i64 = recent.iter().map(|week| week.revenue_cents).sum();
    let ebitda: i64 = firm
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.ebitda())
        .sum();
    let net_income: i64 = recent.iter().map(|week| week.net_income_cents).sum();
    Some(FinanceSummary {
        firm: handle,
        cash_cents: firm.book.cash(),
        restricted_cents: firm.book.restricted_cash(),
        inventory_cents: firm.book.balance(cw_core::ledger::Account::Inventory),
        assets_cents: firm.book.total_assets(),
        liabilities_cents: firm.book.total_liabilities(),
        equity_cents: firm.book.total_equity(),
        revenue_trailing_cents: revenue,
        ebitda_trailing_cents: ebitda,
        net_income_trailing_cents: net_income,
        cash_bridge: CashBridge {
            opening_cents: 0,
            receipts_cents: revenue,
            operating_payments_cents: -firm.book.balance(cw_core::ledger::Account::Wages),
            investment_cents: firm.book.balance(cw_core::ledger::Account::WorkInProgress),
            financing_cents: -firm.book.balance(cw_core::ledger::Account::Loans),
            tax_cents: -firm.book.balance(cw_core::ledger::Account::TaxesPayable),
            closing_cents: firm.book.cash(),
        },
        recent_weeks: recent,
    })
}

pub fn ownership_summary(world: &World, handle: u64) -> Option<OwnershipSummary> {
    let firm = world.firms.get(cw_core::ids::Handle(handle))?;
    let holders = firm
        .shares
        .holders
        .iter()
        .map(|(holder, lot)| HolderLine {
            holder: holder_name(world, *holder),
            quantity: lot.qty,
            votes_bps: firm.shares.vote_bps(*holder),
            cost_cents: lot.cost_cents,
        })
        .collect();
    Some(OwnershipSummary {
        firm: handle,
        issued: firm.shares.issued,
        treasury: firm.shares.treasury(),
        eligible_votes: firm.shares.eligible_votes(),
        controller: firm
            .shares
            .controller()
            .map(|holder| holder_name(world, holder)),
        holders,
        board_seats: firm
            .board
            .seats
            .map(|seat| seat.map(|holder| holder_name(world, holder))),
        next_meeting: firm.board.next_meeting.0,
        ceo: match firm.board.ceo {
            Some(cw_sim::state::CeoRef::Investor) => "You".to_owned(),
            Some(cw_sim::state::CeoRef::Manager(_)) => "Appointed manager".to_owned(),
            None => "Vacant".to_owned(),
        },
    })
}

pub fn deals(world: &World) -> Vec<DealLine> {
    world
        .deals
        .iter()
        .map(|(handle, deal)| DealLine {
            handle: handle.0,
            buyer: world
                .firms
                .get(deal.buyer)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            target: world
                .firms
                .get(deal.target)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            buyer_handle: deal.buyer.0,
            target_handle: deal.target.0,
            kind: match deal.kind {
                cw_sim::state::DealKind::SharePurchase => "share_purchase",
                cw_sim::state::DealKind::AssetPurchase => "asset_purchase",
                cw_sim::state::DealKind::Merger => "merger",
            }
            .to_owned(),
            state: deal.state.key().to_owned(),
            offer_price_cents: deal.offer_price_cents,
            escrow_cents: deal.escrow_cents,
            committed_shares: deal.committed_shares,
            deadline: match deal.state {
                cw_sim::state::DealState::Diligence => Some(deal.diligence_until.0),
                cw_sim::state::DealState::OfferOpen => Some(deal.offer_deadline.0),
                cw_sim::state::DealState::Review => deal.review_deadline.map(|tick| tick.0),
                _ => None,
            },
            review_deadline: deal.review_deadline.map(|tick| tick.0),
        })
        .collect()
}

pub fn events(world: &World, dataset: &Dataset) -> Vec<EventLine> {
    world
        .events
        .pending
        .iter()
        .filter(|event| !event.resolved)
        .filter_map(|event| {
            let card = dataset.event(&event.card)?;
            Some(EventLine {
                id: event.id,
                card: card.id.clone(),
                title: card.title.clone(),
                subject: event
                    .subject_firm
                    .and_then(|handle| world.firms.get(handle))
                    .map(|firm| firm.name.clone())
                    .unwrap_or_else(|| {
                        event
                            .region
                            .and_then(|index| world.regions.get(index as usize))
                            .map(|region| region.name.clone())
                            .unwrap_or_default()
                    }),
                mandatory: event.mandatory,
                deadline: event.deadline.0,
                choices: card
                    .choices
                    .iter()
                    .map(|choice| ChoiceLine {
                        id: choice.id.clone(),
                        label: choice.label.clone(),
                    })
                    .collect(),
            })
        })
        .collect()
}

pub fn market(world: &World, dataset: &Dataset, region: u8, product: u16) -> MarketLine {
    let stock: i64 = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.region == region)
        .map(|(_, firm)| firm.stock(product, region).lots())
        .sum();
    let sales: i64 = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.region == region)
        .map(|(_, firm)| {
            firm.last_week_units_sold
                .get(&(region, product))
                .map(|q| q.lots())
                .unwrap_or(0)
        })
        .sum();
    let gateway = world
        .gateways
        .iter()
        .find(|gateway| gateway.region == region && gateway.product == product);
    MarketLine {
        product: dataset.products[product as usize].name.clone(),
        reference_price_cents: dataset.reference_price(product),
        last_price_cents: world.regions[region as usize]
            .households
            .last_sale_price_cents
            .get(&product)
            .copied()
            .unwrap_or_else(|| dataset.reference_price(product)),
        regional_stock_lots: stock,
        last_week_sales_lots: sales,
        gateway_buy_cents: gateway.map(|g| g.buy_price_cents).unwrap_or(0),
        gateway_sell_cents: gateway.map(|g| g.sell_price_cents).unwrap_or(0),
    }
}

pub fn contracts(world: &World, dataset: &Dataset, firm: u64) -> Vec<ContractLine> {
    world
        .contracts
        .iter()
        .filter(|(_, contract)| contract.buyer.0 == firm || contract.seller.0 == firm)
        .map(|(handle, contract)| ContractLine {
            handle: handle.0,
            buyer: world
                .firms
                .get(contract.buyer)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            seller: world
                .firms
                .get(contract.seller)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            product: dataset.products[contract.product as usize].name.clone(),
            quantity_weekly: contract.quantity_weekly.lots(),
            price_cents: contract.price_cents,
            remaining_weeks: contract.remaining_weeks,
            status: format!("{:?}", contract.status).to_lowercase(),
            from_region: world.regions[contract.from_region as usize].name.clone(),
            to_region: world.regions[contract.to_region as usize].name.clone(),
        })
        .collect()
}

pub fn loans(world: &World, firm: u64) -> Vec<LoanLine> {
    world
        .loans
        .iter()
        .filter(|(_, loan)| loan.borrower.0 == firm)
        .map(|(handle, loan)| LoanLine {
            handle: handle.0,
            borrower: world
                .firms
                .get(loan.borrower)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            lender: match loan.lender {
                cw_sim::state::Lender::Outside(index) => world
                    .outside
                    .lenders
                    .get(index as usize)
                    .map(|l| l.name.clone())
                    .unwrap_or_default(),
                cw_sim::state::Lender::Bank(handle) | cw_sim::state::Lender::Firm(handle) => world
                    .firms
                    .get(handle)
                    .map(|f| f.name.clone())
                    .unwrap_or_default(),
            },
            principal_cents: loan.principal_remaining_cents,
            rate_bps: loan.rate_bps,
            remaining_weeks: loan.remaining_weeks,
            secured: loan.secured,
            defaulted: loan.defaulted,
        })
        .collect()
}

pub fn insurance(world: &World, firm: u64) -> Vec<InsuranceLine> {
    world
        .insurance
        .iter()
        .filter(|(_, policy)| policy.insured.0 == firm || policy.insurer.0 == firm)
        .map(|(handle, policy)| InsuranceLine {
            handle: handle.0,
            insurer: world
                .firms
                .get(policy.insurer)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            insured: world
                .firms
                .get(policy.insured)
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            facility: policy.facility.0,
            limit_cents: policy.limit_cents,
            deductible_cents: policy.deductible_cents,
            weekly_premium_cents: policy.weekly_premium_cents,
            status: match policy.status {
                cw_sim::state::PolicyStatus::Active => "active".to_owned(),
                cw_sim::state::PolicyStatus::Expired => "expired".to_owned(),
                cw_sim::state::PolicyStatus::Claimed => "claimed".to_owned(),
            },
            claim_reserve_cents: policy.claim_reserve_cents,
        })
        .collect()
}

pub fn deal_state_key(state: DealState) -> &'static str {
    state.key()
}
