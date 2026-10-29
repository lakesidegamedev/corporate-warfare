//! Owned, immutable display models. Swift never computes a financial identity.

use cw_data::Dataset;
use cw_sim::state::{DealState, FacilityStatus, FirmStatus, HolderId, World};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Overview {
    pub tick: u32,
    pub week_label: String,
    /// Calendar year of week 1 in this scenario, for labelling any week number.
    pub start_year: i32,
    pub revision: u64,
    /// Investor figures are in the reporting currency.
    pub investor_cash_cents: i64,
    pub investor_net_worth_cents: i64,
    /// The reporting currency id, empty when the scenario has one money.
    pub reporting_currency: String,
    /// The currency the primary company's figures are in, empty when the scenario has one money.
    pub primary_currency: String,
    pub primary_firm: Option<u64>,
    pub primary_name: String,
    pub firm_cash_cents: i64,
    pub forecast_four_week_cents: i64,
    pub last_week_ebit_cents: i64,
    /// Net income over the last closed weeks, up to thirteen: the quarter's profit.
    pub quarter_net_income_cents: i64,
    /// How many closed weeks that covers (fewer than thirteen early in a career).
    pub quarter_weeks: u32,
    pub alerts: Vec<Alert>,
    pub pause: Option<String>,
    /// The plain-text reason for the pause and the one action that resumes play.
    pub pause_info: Option<PauseInfo>,
    /// One primary recommended next action and the reason for it.
    pub next_action_key: String,
    pub next_action_reason: String,
}

/// Which pauses stop the clock. Decisions always do: a card that needs an answer, a board vote
/// or a deal deadline that falls before the next week, and the five-year score sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PausePolicy {
    /// Stop only for a decision that needs an answer or a milestone sheet.
    #[default]
    Decisions,
    /// Also stop for warnings: grace running out, a first strike, a rejected union counter.
    DecisionsAndWarnings,
}

impl PausePolicy {
    pub fn key(self) -> &'static str {
        match self {
            PausePolicy::Decisions => "decisions",
            PausePolicy::DecisionsAndWarnings => "decisions_and_warnings",
        }
    }

    pub fn from_key(key: &str) -> Option<PausePolicy> {
        match key {
            "decisions" => Some(PausePolicy::Decisions),
            "decisions_and_warnings" => Some(PausePolicy::DecisionsAndWarnings),
            _ => None,
        }
    }
}

/// Why the clock is stopped and what resumes it. Every pause has one, and `action` always
/// names something the player can actually do: `answer` opens the card in `event_id`, anything
/// else is cleared with `dismiss_pause` (the Continue button).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseInfo {
    /// The raw engine reason, for logs and support.
    pub reason: String,
    /// `decision`, `deadline`, `warning` or `milestone`.
    pub kind: String,
    /// Short headline in plain words.
    pub title: String,
    /// One or two sentences on what happened and what to do.
    pub detail: String,
    /// `answer` (open the card) or `continue`.
    pub action: String,
    /// The button text.
    pub action_label: String,
    /// The card to open when `action` is `answer`.
    pub event_id: Option<u64>,
    /// The company the pause is about, when there is one.
    pub firm: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    pub key: String,
    pub subject: String,
    pub severity: u8,
    /// A fuller plain-English line with figures and the next step, when the key alone is not enough.
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirmSummary {
    pub handle: u64,
    pub name: String,
    pub industry: String,
    pub region: String,
    /// The currency of cash and equity; empty when the scenario has one money. Fair value and
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
    /// True when the investor has no management right and no completed diligence.
    pub restricted: bool,
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
    /// Cash to bring the plant back from mothballs.
    pub reopen_cost_cents: i64,
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
    /// Goods and materials sold, at carrying cost.
    pub cost_of_goods_cents: i64,
    pub wages_cents: i64,
    /// Overheads: maintenance, rent, marketing, freight, penalties and the like.
    pub operating_expense_cents: i64,
    pub depreciation_cents: i64,
    /// Interest paid less interest earned.
    pub net_interest_cents: i64,
    pub tax_cents: i64,
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
    pub product_id: String,
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
        HolderId::State => "The State".to_owned(),
    }
}

pub fn firm_summary(
    world: &World,
    _dataset: &Dataset,
    handle: u64,
    inspect: bool,
) -> Option<FirmSummary> {
    let firm = world.firms.get(cw_core::ids::Handle(handle))?;
    Some(FirmSummary {
        handle,
        name: firm.name.clone(),
        industry: cw_sim::state::INDUSTRY_NAMES[firm.industry as usize].to_owned(),
        region: world.regions[firm.region as usize].name.clone(),
        currency: currency_id(world, _dataset, handle),
        cash_cents: if inspect { firm.book.cash() } else { 0 },
        equity_cents: if inspect { firm.book.total_equity() } else { 0 },
        fair_value_cents: firm.shares.fair_cents * firm.shares.eligible_votes(),
        mark_cents: firm.shares.mark_cents,
        grade: if inspect {
            firm.grade.key().to_owned()
        } else {
            "?".to_owned()
        },
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
        facilities: if inspect {
            firm.facilities.len() as u32
        } else {
            0
        },
        restricted: !inspect,
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
        reopen_cost_cents: cw_sim::economy::operations::reopen_cost(facility),
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
            cost_of_goods_cents: week.cost_of_goods,
            wages_cents: week.wages,
            operating_expense_cents: week.operating_expense - week.other,
            depreciation_cents: week.depreciation,
            net_interest_cents: week.interest_expense - week.interest_income,
            tax_cents: week.tax,
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
        // A rival's card is the rival's to answer; the player's inbox and alerts never carry it.
        .filter(|event| {
            event
                .subject_firm
                .filter(|firm| world.firms.contains(*firm))
                .map(|firm| cw_sim::economy::events_engine::player_side(world, firm))
                .unwrap_or(true)
        })
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
    // Stock and sales belong to the state where the goods sit and sell, whichever state the
    // owning firm is headquartered in. Sales come from the last closed week: between ticks the
    // open week is empty.
    let stock: i64 = world
        .firms
        .iter()
        .map(|(_, firm)| firm.stock(product, region).lots())
        .sum();
    let sales: i64 = world
        .firms
        .iter()
        .filter_map(|(_, firm)| firm.last_closed_sales())
        .filter_map(|week| week.get(&(region, product)).copied())
        .sum();
    let gateway = world
        .gateways
        .iter()
        .find(|gateway| gateway.region == region && gateway.product == product);
    let reference = dataset.reference_price(product);
    // Consumer goods clear against households and record a sale price. Producer goods trade
    // between firms: quote the lowest local ask among sellers holding stock, never above what
    // the gateway charges to import.
    let household = world.regions[region as usize]
        .households
        .last_sale_price_cents
        .get(&product)
        .copied();
    let lowest_ask = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.stock(product, region).lots() > 0)
        .map(|(_, firm)| {
            firm.policies
                .prices
                .get(&product)
                .copied()
                .unwrap_or(reference)
        })
        .min();
    let import = gateway
        .map(|g| g.sell_price_cents)
        .filter(|price| *price > 0);
    let last_price = household
        .or(match (lowest_ask, import) {
            (Some(ask), Some(import)) => Some(ask.min(import)),
            (ask, import) => ask.or(import),
        })
        .unwrap_or(reference);
    MarketLine {
        product: dataset.products[product as usize].name.clone(),
        product_id: dataset.products[product as usize].id.clone(),
        reference_price_cents: reference,
        last_price_cents: last_price,
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
                cw_sim::state::Lender::Outside(cw_sim::state::STATE_LENDER) => {
                    "The State".to_owned()
                }
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

/// One good a firm makes, as its pricing panel shows it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductLine {
    pub product: u16,
    pub product_id: String,
    pub name: String,
    pub price_cents: i64,
    pub reference_cents: i64,
    /// Median asking price of the other firms that make the good.
    pub market_cents: i64,
    /// Cost of one lot at the firm's plants' full run rate; 0 when it has no operating plant.
    pub unit_cost_cents: i64,
    pub made_last_week_lots: i64,
    pub sold_last_week_lots: i64,
    /// Stock beyond the firm's own input cover.
    pub stock_lots: i64,
    /// "auto", "market", "cost_plus" or "fixed".
    pub mode: String,
    pub posture_bps: i32,
}

pub fn product_lines(world: &World, dataset: &Dataset, firm: u64) -> Vec<ProductLine> {
    use cw_sim::economy::pricing;
    let handle = cw_core::ids::Handle(firm);
    let Some(entity) = world.firms.get(handle) else {
        return Vec::new();
    };
    pricing::outputs(world, dataset, handle)
        .into_iter()
        .map(|product| {
            let id = dataset.products[product as usize].id.clone();
            let made: i64 = entity
                .facilities
                .iter()
                .filter_map(|plant| world.facilities.get(*plant))
                .map(|plant| plant.last_made(dataset, &id).lots())
                .sum();
            let sold: i64 = entity
                .last_closed_sales()
                .map(|week| {
                    week.iter()
                        .filter(|((_, p), _)| *p == product)
                        .map(|(_, qty)| *qty)
                        .sum()
                })
                .unwrap_or(0);
            let reference = dataset.reference_price(product);
            let (mode, posture_bps) = match entity.policies.price_postures.get(&product) {
                Some(cw_sim::state::PricePosture::Market { bps }) => ("market", *bps),
                Some(cw_sim::state::PricePosture::CostPlus { bps }) => ("cost_plus", *bps),
                None if entity.policies.manual_prices.contains(&product) => ("fixed", 0),
                None => ("auto", 0),
            };
            ProductLine {
                product,
                name: dataset.products[product as usize].name.clone(),
                product_id: id,
                price_cents: entity
                    .policies
                    .prices
                    .get(&product)
                    .copied()
                    .unwrap_or(reference),
                reference_cents: reference,
                market_cents: pricing::market_price(world, dataset, handle, product),
                unit_cost_cents: pricing::unit_cost(world, dataset, handle, product).unwrap_or(0),
                made_last_week_lots: made,
                sold_last_week_lots: sold,
                stock_lots: pricing::free_stock(world, dataset, handle, product),
                mode: mode.to_owned(),
                posture_bps,
            }
        })
        .collect()
}

/// Basis-point change from the closing price `weeks` back to `now`; 0 without history.
fn change_bps(history: &[i64], weeks: usize, now: i64) -> i64 {
    if history.len() <= weeks {
        return history
            .first()
            .map(|then| (now - then) * 10_000 / (*then).max(1))
            .unwrap_or(0);
    }
    let then = history[history.len() - 1 - weeks].max(1);
    (now - then) * 10_000 / then
}

/// One good on the corridor commodity board.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommodityLine {
    pub product: u16,
    pub product_id: String,
    pub name: String,
    /// The sector whose strategies make it, or "other".
    pub sector: String,
    pub price_cents: i64,
    pub reference_cents: i64,
    pub change_week_bps: i64,
    pub change_quarter_bps: i64,
    /// Closing prices, oldest first, up to a year.
    pub history_cents: Vec<i64>,
    pub made_lots: i64,
    pub sold_lots: i64,
    pub imported_lots: i64,
    pub exported_lots: i64,
    pub stock_lots: i64,
    pub import_cents: i64,
    pub export_cents: i64,
    /// True when one of the player's companies makes it.
    pub yours: bool,
}

/// True when a good has a place in this game: some strategy makes it and a plant in the world
/// makes or uses it, or it has traded or sits in stock. A good nothing in the game makes or
/// uses would show a placeholder price and no trade, so the board leaves it out until a plant
/// for it is built.
fn listed(world: &World, dataset: &Dataset, code: u16) -> bool {
    let id = &dataset.products[code as usize].id;
    // Storage space is a plant's own capacity, charged as fees; it never clears on the exchange.
    if !dataset.recipes.iter().any(|recipe| recipe.makes(id))
        || dataset
            .recipes
            .iter()
            .any(|recipe| recipe.makes(id) && recipe.storage_class.is_some())
    {
        return false;
    }
    world.facilities.iter().any(|(_, plant)| {
        let recipe = &dataset.recipes[plant.recipe as usize];
        recipe.makes(id) || recipe.weekly_inputs_lots.contains_key(id)
    }) || world
        .gateways
        .iter()
        .any(|g| g.product == code && (g.last_import_lots > 0 || g.last_export_lots > 0))
        || world
            .firms
            .iter()
            .any(|(_, f)| f.inventory.keys().any(|(p, _)| *p == code))
}

pub fn commodity_board(world: &World, dataset: &Dataset) -> Vec<CommodityLine> {
    use cw_sim::economy::pricing;
    let player_outputs: std::collections::BTreeSet<u16> = world
        .firms
        .iter()
        .filter(|(handle, _)| cw_sim::economy::events_engine::player_side(world, *handle))
        .flat_map(|(handle, _)| pricing::outputs(world, dataset, handle))
        .collect();
    (0..dataset.products.len() as u16)
        .filter(|code| {
            let product = &dataset.products[*code as usize];
            product.reference_price_cents > 0
                && !product.kind.is_financial()
                && listed(world, dataset, *code)
        })
        .map(|code| {
            let product = &dataset.products[code as usize];
            let sector = dataset
                .recipes
                .iter()
                .find(|recipe| recipe.output == product.id)
                .or_else(|| {
                    dataset
                        .recipes
                        .iter()
                        .find(|recipe| recipe.makes(&product.id))
                })
                .map(|recipe| recipe.industry.clone())
                .unwrap_or_else(|| "other".to_owned());
            let price = pricing::corridor_price(world, dataset, code);
            let mut history: Vec<i64> = world
                .price_history
                .iter()
                .filter_map(|point| point.goods.get(&code).copied())
                .collect();
            // Before the first close, the board still shows today's price as its one point.
            if history.is_empty() {
                history.push(price);
            }
            let made: i64 = world
                .facilities
                .iter()
                .map(|(_, f)| f.last_made(dataset, &product.id).lots())
                .sum();
            let sold: i64 = world
                .firms
                .iter()
                .filter_map(|(_, f)| f.last_closed_sales())
                .flat_map(|week| week.iter())
                .filter(|((_, p), _)| *p == code)
                .map(|(_, q)| *q)
                .sum();
            let gateways = world.gateways.iter().filter(|g| g.product == code);
            let (imported, exported, import, export) =
                gateways.fold((0i64, 0i64, i64::MAX, 0i64), |(i, e, low, high), g| {
                    (
                        i + g.last_import_lots,
                        e + g.last_export_lots,
                        if g.sell_price_cents > 0 {
                            low.min(g.sell_price_cents)
                        } else {
                            low
                        },
                        high.max(g.buy_price_cents),
                    )
                });
            let stock: i64 = world
                .firms
                .iter()
                .flat_map(|(_, f)| f.inventory.iter())
                .filter(|((p, _), _)| *p == code)
                .map(|(_, lot)| lot.qty.lots())
                .sum();
            CommodityLine {
                product: code,
                product_id: product.id.clone(),
                name: product.name.clone(),
                sector,
                price_cents: price,
                reference_cents: product.reference_price_cents,
                change_week_bps: change_bps(&history, 1, price),
                change_quarter_bps: change_bps(&history, 13, price),
                history_cents: history,
                made_lots: made,
                sold_lots: sold,
                imported_lots: imported,
                exported_lots: exported,
                stock_lots: stock,
                import_cents: if import == i64::MAX { 0 } else { import },
                export_cents: export,
                yours: player_outputs.contains(&code),
            }
        })
        .collect()
}

/// One company on the corridor stock board.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StockLine {
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
    /// Net income over the last closed weeks (up to thirteen), and how many weeks.
    pub earnings_cents: i64,
    pub earnings_weeks: u32,
    /// Shares the player holds personally.
    pub your_shares: i64,
    /// Share of the stock held outside the controlling holder, in basis points.
    pub float_bps: i64,
    pub controlled_by_you: bool,
}

pub fn stock_board(world: &World) -> Vec<StockLine> {
    world
        .firms
        .iter()
        .filter(|(_, firm)| firm.on_board())
        .map(|(handle, firm)| {
            let price = firm.shares.mark_cents.max(1);
            let history: Vec<i64> = world
                .price_history
                .iter()
                .filter_map(|point| point.shares.get(&handle).copied())
                .collect();
            let weeks = firm.book.recent_weeks();
            let outside: i64 = firm
                .shares
                .holders
                .iter()
                .filter(|(holder, _)| matches!(holder, HolderId::Outside(_)))
                .map(|(_, lot)| lot.qty)
                .sum();
            StockLine {
                firm: handle.0,
                name: firm.name.clone(),
                industry: cw_sim::state::INDUSTRY_NAMES[firm.industry as usize].to_owned(),
                region: world.regions[firm.region as usize].name.clone(),
                price_cents: price,
                fair_cents: firm.shares.fair_cents,
                change_week_bps: change_bps(&history, 1, price),
                change_quarter_bps: change_bps(&history, 13, price),
                history_cents: history,
                shares_issued: firm.shares.issued,
                market_cap_cents: price.saturating_mul(firm.shares.issued),
                earnings_cents: weeks.iter().map(|week| week.net_income).sum(),
                earnings_weeks: weeks.len() as u32,
                your_shares: firm.shares.qty(HolderId::Investor),
                float_bps: outside * 10_000 / firm.shares.issued.max(1),
                controlled_by_you: cw_sim::economy::events_engine::player_side(world, handle),
            }
        })
        .collect()
}

/// The currency id a firm keeps its books in; empty when the scenario has one money.
pub fn currency_id(world: &World, dataset: &Dataset, handle: u64) -> String {
    if !world.fx.active {
        return String::new();
    }
    let index = cw_sim::economy::fx::currency_of(world, cw_core::ids::Handle(handle)) as usize;
    dataset
        .currencies
        .as_ref()
        .and_then(|set| set.currencies.get(index))
        .map(|def| def.id.clone())
        .unwrap_or_default()
}

/// One currency as the economy card shows it, against the reporting currency.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrencyView {
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
    pub crisis_week: Option<u32>,
    /// Weekly value in the reporting currency, millionths, oldest first, up to 52 weeks.
    pub history_ppm: Vec<i64>,
}

/// The foreign exchange picture for the economy card.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FxView {
    pub reporting: String,
    pub reporting_symbol: String,
    pub currencies: Vec<CurrencyView>,
    /// Foreign exchange gains and losses booked by the investor's companies, in the reporting
    /// currency, and the foreign sales still waiting to be collected.
    pub gains_cents: i64,
    pub losses_cents: i64,
    pub open_foreign_sales_cents: i64,
}

pub fn fx_view(world: &World, dataset: &Dataset, reporting: u8) -> Option<FxView> {
    if !world.fx.active {
        return None;
    }
    let set = dataset.currencies.as_ref()?;
    let fx = &world.fx;
    let rep_history = |back: usize| -> i64 {
        if reporting == 0 {
            return cw_sim::economy::fx::PPM;
        }
        let history = &fx.currencies[reporting as usize].history;
        history
            .len()
            .checked_sub(back + 1)
            .and_then(|i| history.get(i))
            .copied()
            .unwrap_or(cw_sim::economy::fx::PPM)
    };
    let currencies = set
        .currencies
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let st = &fx.currencies[index];
            let chair = if index == 0 {
                let c = &world.macro_state.cycle.chair;
                (c.name.clone(), c.stance_label().to_owned())
            } else {
                fx.chairs
                    .get(index)
                    .copied()
                    .flatten()
                    .and_then(|person| dataset.cast.get(person as usize))
                    .map(|person| {
                        let banker = cw_sim::state::CentralBanker {
                            name: person.name.clone(),
                            stance: person.stance.unwrap_or(0),
                            appointed_week: 0,
                            term_ends: 0,
                            person: None,
                        };
                        (banker.name.clone(), banker.stance_label().to_owned())
                    })
                    .unwrap_or_default()
            };
            let relative = |value: i64, back: usize| -> i64 {
                cw_core::money::round_even_div(
                    value as i128 * cw_sim::economy::fx::PPM as i128,
                    rep_history(back).max(1) as i128,
                ) as i64
            };
            let len = st.history.len();
            let history_ppm: Vec<i64> = (0..len.min(52))
                .rev()
                .map(|back| {
                    let value = if index == 0 {
                        cw_sim::economy::fx::PPM
                    } else {
                        st.history[len - 1 - back]
                    };
                    relative(value, back)
                })
                .collect();
            let now = history_ppm
                .last()
                .copied()
                .unwrap_or(cw_sim::economy::fx::PPM);
            let then = history_ppm
                .len()
                .checked_sub(14)
                .and_then(|i| history_ppm.get(i))
                .copied()
                .unwrap_or(history_ppm.first().copied().unwrap_or(now));
            CurrencyView {
                id: def.id.clone(),
                name: def.name.clone(),
                symbol: def.symbol.clone(),
                country: def.country.clone(),
                central_bank: def.central_bank.clone(),
                chair_name: chair.0,
                chair_stance: chair.1,
                value_ppm: fx.rate_ppm(index as u8, reporting),
                trend_bps: if then > 0 {
                    ((now - then) * 10_000 / then) as i32
                } else {
                    0
                },
                policy_rate_bps: st.policy_rate_bps,
                inflation_bps: st.inflation_bps,
                pegged: def.peg.is_some(),
                band_bps: st.band_bps,
                strain_bps: st.strain_bps,
                crisis_week: st.crisis_week,
                history_ppm,
            }
        })
        .collect();
    let (mut gains, mut losses) = (0i64, 0i64);
    for handle in world.investor_firms() {
        if let Some(pnl) = fx.pnl.get(&handle) {
            let own = cw_sim::economy::fx::currency_of(world, handle);
            gains += fx.convert(pnl.gains, own, reporting);
            losses += fx.convert(pnl.losses, own, reporting);
        }
    }
    let mut open = 0i64;
    for r in &fx.receivables {
        if world.investor_firms().contains(&r.seller) {
            open += fx.convert(r.booked, r.to, reporting);
        }
    }
    let reporting_def = set.currencies.get(reporting as usize)?;
    Some(FxView {
        reporting: reporting_def.id.clone(),
        reporting_symbol: reporting_def.symbol.clone(),
        currencies,
        gains_cents: gains,
        losses_cents: losses,
        open_foreign_sales_cents: open,
    })
}
