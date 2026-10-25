//! Authoritative world state.
//!
//! Everything future-affecting is here and serialised into saves: ledgers, holdings, RNG
//! states, queues, schedules, carry remainders and pending decisions. Nothing here reads the
//! clock, the network or the host filesystem.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::ids::{Handle, Slab};
use cw_core::ledger::Ledger;
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_core::rng::Rng;
use cw_data::{Archetype, Dataset, Difficulty};
use serde::{Deserialize, Serialize};

use crate::economy::valuation::FirmValue;

pub const INDUSTRY_NAMES: [&str; 15] = [
    "agriculture",
    "extraction",
    "energy",
    "chemical_industries",
    "manufacturing",
    "construction",
    "logistics",
    "retail",
    "healthcare",
    "financial",
    "technology",
    "media",
    "telecommunications",
    "real_estate",
    "defense",
];

/// Control requires strictly more than half of eligible votes.
pub const CONTROL_NUMERATOR: i64 = 1;
pub const CONTROL_DENOMINATOR: i64 = 2;
/// Fictional squeeze-out threshold from CORPORATE.md.
pub const SQUEEZE_OUT_BPS: i64 = 9_000;
pub const INITIAL_SHARES: i64 = 1_000_000;
pub const BOARD_SEATS: usize = 5;
pub const PAST_DUE_GRACE_WEEKS: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HolderId {
    Investor,
    Firm(Handle),
    Outside(u32),
    Treasury,
}

impl HolderId {
    pub const fn is_external(self) -> bool {
        matches!(self, HolderId::Outside(_))
    }

    pub const fn is_treasury(self) -> bool {
        matches!(self, HolderId::Treasury)
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ShareLot {
    pub qty: i64,
    /// Total acquisition cost in cents, used for realised gain/loss reporting.
    pub cost_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareBook {
    /// All issued shares, including treasury.
    pub issued: i64,
    pub holders: BTreeMap<HolderId, ShareLot>,
    /// Reference exchange mark, smoothed one eighth toward fair value weekly.
    pub mark_cents: i64,
    pub fair_cents: i64,
    pub last_trade_cents: i64,
    pub ipo_done: bool,
    /// Quarters of published statements available.
    pub statement_weeks: u32,
}

impl ShareBook {
    pub fn new(holders: BTreeMap<HolderId, ShareLot>) -> Self {
        let issued = holders.values().map(|lot| lot.qty).sum();
        ShareBook {
            issued,
            holders,
            mark_cents: 0,
            fair_cents: 0,
            last_trade_cents: 0,
            ipo_done: false,
            statement_weeks: 0,
        }
    }

    pub fn treasury(&self) -> i64 {
        self.holders
            .get(&HolderId::Treasury)
            .map(|lot| lot.qty)
            .unwrap_or(0)
    }

    pub fn eligible_votes(&self) -> i64 {
        self.issued - self.treasury()
    }

    pub fn qty(&self, holder: HolderId) -> i64 {
        self.holders.get(&holder).map(|lot| lot.qty).unwrap_or(0)
    }

    pub fn add(&mut self, holder: HolderId, qty: i64, cost_cents: i64) {
        if qty <= 0 {
            return;
        }
        let lot = self.holders.entry(holder).or_default();
        lot.qty += qty;
        lot.cost_cents += cost_cents;
    }

    /// Transfer shares between holders, moving acquisition cost proportionally.
    pub fn transfer(&mut self, from: HolderId, to: HolderId, qty: i64, paid_cents: i64) {
        if qty <= 0 {
            return;
        }
        let moved_cost = {
            let lot = self.holders.get(&from).copied().unwrap_or_default();
            if lot.qty <= 0 {
                0
            } else {
                (lot.cost_cents as i128 * qty as i128 / lot.qty as i128) as i64
            }
        };
        if let Some(lot) = self.holders.get_mut(&from) {
            lot.qty -= qty;
            lot.cost_cents -= moved_cost;
            if lot.qty == 0 {
                self.holders.remove(&from);
            }
        }
        self.add(to, qty, paid_cents);
    }

    /// Holder with strictly more than half of eligible votes.
    pub fn controller(&self) -> Option<HolderId> {
        let eligible = self.eligible_votes();
        if eligible <= 0 {
            return None;
        }
        let mut best: Option<(HolderId, i64)> = None;
        for (holder, lot) in &self.holders {
            if holder.is_treasury() {
                continue;
            }
            if lot.qty * CONTROL_DENOMINATOR > eligible * CONTROL_NUMERATOR
                && best.map(|(_, qty)| lot.qty > qty).unwrap_or(true)
            {
                best = Some((*holder, lot.qty));
            }
        }
        best.map(|(holder, _)| holder)
    }

    /// Voting share in basis points, treasury excluded.
    pub fn vote_bps(&self, holder: HolderId) -> i32 {
        let eligible = self.eligible_votes();
        if eligible <= 0 {
            return 0;
        }
        ((self.qty(holder) as i128 * 10_000) / eligible as i128) as i32
    }

    pub fn meets_squeeze_out(&self, holder: HolderId) -> bool {
        if holder.is_treasury() {
            return false;
        }
        let eligible = self.eligible_votes();
        eligible > 0
            && self.qty(holder) as i128 * 10_000 >= eligible as i128 * SQUEEZE_OUT_BPS as i128
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Grade {
    A,
    B,
    C,
    D,
}

impl Grade {
    pub const fn spread_bps(self) -> i32 {
        match self {
            Grade::A => 200,
            Grade::B => 400,
            Grade::C => 700,
            Grade::D => 1_200,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Grade::A => "A",
            Grade::B => "B",
            Grade::C => "C",
            Grade::D => "D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FirmStatus {
    Operating,
    /// Missed a due payment; payouts frozen, grace running.
    PastDue {
        since: Tick,
    },
    Restructuring {
        until: Tick,
    },
    Liquidating,
    Merged {
        into: Handle,
    },
    Dissolved,
}

impl FirmStatus {
    pub const fn operative(self) -> bool {
        matches!(self, FirmStatus::Operating | FirmStatus::PastDue { .. })
    }

    pub const fn can_trade(self) -> bool {
        matches!(self, FirmStatus::Operating)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FacilityStatus {
    Building,
    Retooling { until: Tick },
    Operating,
    Mothballed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockLot {
    pub qty: MilliLots,
    pub cost_cents: i64,
}

impl StockLot {
    /// Moving-average receipt: quantity increases and actual delivered cost is added.
    pub fn receive(&mut self, qty: MilliLots, cost: Cents) {
        if qty.0 <= 0 {
            return;
        }
        self.qty = MilliLots(self.qty.0 + qty.0);
        self.cost_cents += cost.0;
    }

    /// Issue stock at moving-average cost with a persisted sub-cent remainder.
    pub fn issue(&mut self, qty: MilliLots, carry: &mut i64) -> Cents {
        let take = qty.min(self.qty).max_zero();
        if take.0 <= 0 || self.qty.0 <= 0 {
            return Cents::ZERO;
        }
        // cost * take / qty with a persisted remainder so the last unit takes the rest.
        let numerator = self.cost_cents as i128 * take.0 as i128 + *carry as i128;
        let denominator = self.qty.0 as i128;
        let mut cost = (numerator / denominator) as i64;
        let remainder = (numerator % denominator) as i64;
        if take.0 == self.qty.0 {
            cost = self.cost_cents;
            *carry = 0;
        } else {
            *carry = remainder;
        }
        self.qty = MilliLots(self.qty.0 - take.0);
        self.cost_cents -= cost;
        if self.qty.0 == 0 {
            self.cost_cents = 0;
            *carry = 0;
        }
        Cents(cost)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Facility {
    pub firm: Handle,
    pub region: u8,
    pub recipe: u16,
    pub blocks: u8,
    pub status: FacilityStatus,
    pub started: Tick,
    pub completes: Tick,
    pub escrow_cents: i64,
    pub capex_cents: i64,
    pub accumulated_depreciation: i64,
    /// Workers actually employed this week by tier.
    pub workers: [i64; 3],
    /// Weekly production target as basis points of full capacity.
    pub target_bps: i32,
    pub quality_bps: i32,
    pub deposit_remaining: MilliLots,
    pub last_output: MilliLots,
    pub last_inputs_short: bool,
    pub last_staff_ratio_bps: i32,
    pub mothballed_since: Option<Tick>,
}

impl Facility {
    pub fn full_workers(&self, dataset: &Dataset) -> [i64; 3] {
        let recipe = &dataset.recipes[self.recipe as usize];
        [
            recipe.workers_low_mid_high[0] * self.blocks as i64,
            recipe.workers_low_mid_high[1] * self.blocks as i64,
            recipe.workers_low_mid_high[2] * self.blocks as i64,
        ]
    }

    pub fn full_weekly_output(&self, dataset: &Dataset) -> MilliLots {
        MilliLots::from_lots(
            dataset.recipes[self.recipe as usize].weekly_output_lots * self.blocks as i64,
        )
    }

    pub fn is_operating(&self) -> bool {
        self.status == FacilityStatus::Operating
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policies {
    /// Per-product asking price; falls back to the regional reference price.
    pub prices: BTreeMap<u16, i64>,
    /// Weekly wage offer by tier.
    pub wage_offer: [i64; 3],
    /// Share of full capacity targeted for production.
    pub production_target_bps: i32,
    pub marketing_budget_cents: i64,
    pub research_budget_cents: i64,
    /// Desired weeks of input cover for automatic purchasing.
    pub input_cover_weeks: i32,
}

impl Default for Policies {
    fn default() -> Self {
        Policies {
            prices: BTreeMap::new(),
            wage_offer: [0; 3],
            production_target_bps: 10_000,
            marketing_budget_cents: 0,
            research_budget_cents: 0,
            input_cover_weeks: 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CeoRef {
    Investor,
    Manager(u32),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Board {
    pub seats: [Option<HolderId>; BOARD_SEATS],
    pub next_meeting: Tick,
    pub ceo: Option<CeoRef>,
    pub debt_limit_bps: i32,
    pub capex_limit_bps: i32,
    pub pending_votes: Vec<BoardVote>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoardVote {
    pub id: u32,
    pub subject: String,
    pub deadline: Tick,
    pub yes: u8,
    pub no: u8,
    pub resolved: bool,
    pub passed: bool,
}

impl Board {
    pub fn new(meeting: Tick) -> Self {
        Board {
            seats: [None; BOARD_SEATS],
            next_meeting: meeting,
            ceo: None,
            debt_limit_bps: 6_000,
            capex_limit_bps: 2_500,
            pending_votes: Vec::new(),
        }
    }

    pub fn seats_held(&self, holder: HolderId) -> usize {
        self.seats
            .iter()
            .filter(|seat| **seat == Some(holder))
            .count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BargainState {
    #[default]
    Idle,
    Ballot,
    Demanding {
        deadline: Tick,
    },
    Striking {
        until: Tick,
        started: Tick,
    },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UnionState {
    pub region: u8,
    pub organisation_bps: i32,
    pub state: BargainState,
    pub last_demand_week: u32,
    pub last_offer_cents: i64,
    pub demand_cents: i64,
    pub ballot_support_bps: i32,
    pub settlement_cost_cents: i64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Guarantee {
    pub beneficiary: Handle,
    pub claim_cents: i64,
    pub max_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BankModel {
    pub deposit_rate_bps: i32,
    pub specialist_concentration: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InsurerModel {
    pub specialist_concentration: bool,
    pub unearned_reserve_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Firm {
    pub name: String,
    pub industry: u8,
    pub region: u8,
    pub archetype: Option<Archetype>,
    pub book: Ledger,
    pub status: FirmStatus,
    pub shares: ShareBook,
    pub board: Board,
    pub policies: Policies,
    /// Opening stock by (product, region) with moving-average carrying cost.
    pub inventory: BTreeMap<(u16, u8), StockLot>,
    pub facilities: Vec<Handle>,
    pub loans: Vec<Handle>,
    pub bonds: Vec<Handle>,
    pub insurance: Vec<Handle>,
    pub contracts_buy: Vec<Handle>,
    pub contracts_sell: Vec<Handle>,
    pub grade: Grade,
    pub past_due_weeks: u8,
    pub payout_freeze: bool,
    pub organisation: BTreeMap<u8, i32>,
    pub union: UnionState,
    pub tech_level: u8,
    pub tech_progress_cents: i64,
    pub brand_bps: BTreeMap<(u8, u16), i32>,
    pub reputation_bps: i32,
    pub policy_requests: Vec<PolicyRequest>,
    pub guarantees: Vec<Guarantee>,
    pub last_strategic_week: u32,
    pub merged_into: Option<Handle>,
    pub founding_week: Tick,
    pub bank: Option<BankModel>,
    pub insurer: Option<InsurerModel>,
    pub valuation: FirmValue,
    /// Unspent reservation for submitted orders, released after settlement.
    pub reserved_cash_cents: i64,
    pub last_week_units_sold: BTreeMap<(u8, u16), MilliLots>,
    /// Thirteen most recent weeks of unit sales by (region, product), oldest first.
    pub sales_history: Vec<BTreeMap<(u8, u16), i64>>,
    pub quality_bps: BTreeMap<u16, i32>,
}

impl Firm {
    pub fn operative(&self) -> bool {
        self.status.operative()
    }

    pub fn stock(&self, product: u16, region: u8) -> MilliLots {
        self.inventory
            .get(&(product, region))
            .map(|lot| lot.qty)
            .unwrap_or(MilliLots::ZERO)
    }

    pub fn stock_lot_mut(&mut self, product: u16, region: u8) -> &mut StockLot {
        self.inventory.entry((product, region)).or_default()
    }

    /// Trailing thirteen-week unit sales in one regional product market.
    pub fn trailing_units(&self, region: u8, product: u16) -> i64 {
        self.sales_history
            .iter()
            .filter_map(|week| week.get(&(region, product)).copied())
            .sum()
    }

    /// Close one week of sales into the bounded history.
    pub fn close_sales_week(&mut self) {
        let week: BTreeMap<(u8, u16), i64> = self
            .last_week_units_sold
            .iter()
            .map(|(key, qty)| (*key, qty.lots()))
            .collect();
        self.sales_history.push(week);
        if self.sales_history.len() > 13 {
            self.sales_history.remove(0);
        }
        self.last_week_units_sold.clear();
    }

    pub fn total_employees(&self, facilities: &Slab<Facility>) -> i64 {
        self.facilities
            .iter()
            .filter_map(|handle| facilities.get(*handle))
            .map(|facility| facility.workers.iter().sum::<i64>())
            .sum()
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct LabourTier {
    /// Workers physically present in the region, including the employed.
    pub population: i64,
    pub employed: i64,
    pub reservation_wage_cents: i64,
    pub prevailing_wage_cents: i64,
    /// Milli-worker accumulator for voluntary search.
    pub search_carry: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Household {
    pub savings_cents: i64,
    pub last_income_cents: i64,
    /// Fixed expenditure weights summing 10000.
    pub weights: BTreeMap<u16, i64>,
    pub baseline_lots: BTreeMap<u16, i64>,
    pub last_sale_price_cents: BTreeMap<u16, i64>,
    pub spend_cents: BTreeMap<u16, i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RegionPolicy {
    pub tax_modifier_bps: i32,
    pub tax_modifier_until: Option<Tick>,
    pub permit_bonus_until: Option<Tick>,
    pub training_grant_until: Option<Tick>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionState {
    pub code: String,
    pub name: String,
    pub demand_index_bps: i32,
    pub wage_index_bps: i32,
    pub land_index_bps: i32,
    pub labour: [LabourTier; 3],
    pub households: Household,
    pub policy: RegionPolicy,
    /// Active demand modifiers: (cause tag, product, bps, until).
    pub demand_modifiers: Vec<(String, Option<u16>, i32, Tick)>,
    /// Active supply/yield modifiers, applied to physical output rather than demand.
    #[serde(default)]
    pub supply_modifiers: Vec<(String, Option<u16>, i32, Tick)>,
    pub organisation_bps: i32,
    pub catastrophe_until: Option<Tick>,
    /// Background employers absorb the rest of the scaled workforce.
    pub background_employed: [i64; 3],
    pub background_wage_cents: [i64; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Government {
    pub book: Ledger,
    pub procurement_budget_cents: i64,
    pub healthcare_budget_cents: i64,
    pub training_budget_cents: i64,
    pub last_tax_receipts_cents: i64,
    pub bonds_outstanding_cents: i64,
    pub pending_procurement: BTreeMap<u16, MilliLots>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gateway {
    pub region: u8,
    pub product: u16,
    pub capacity_lots: i64,
    /// Price at which the gateway sells into the region.
    pub sell_price_cents: i64,
    /// Price at which the gateway buys out of the region.
    pub buy_price_cents: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lender {
    Outside(u32),
    Bank(Handle),
    Firm(Handle),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Loan {
    pub borrower: Handle,
    pub lender: Lender,
    pub principal_remaining_cents: i64,
    pub rate_bps: i32,
    pub weekly_principal_cents: i64,
    pub remaining_weeks: u32,
    pub collateral: Option<Handle>,
    pub secured: bool,
    pub created: Tick,
    pub carry: i64,
    pub defaulted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bond {
    pub issuer: Handle,
    pub principal_cents: i64,
    pub coupon_bps: i32,
    pub maturity: Tick,
    pub issued: Tick,
    pub holders: BTreeMap<HolderId, i64>,
    pub carry: i64,
    pub next_coupon: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractStatus {
    Active,
    Completed,
    Cancelled,
    Defaulted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub buyer: Handle,
    pub seller: Handle,
    pub product: u16,
    pub from_region: u8,
    pub to_region: u8,
    pub quantity_weekly: MilliLots,
    pub price_cents: i64,
    pub penalty_bps: i32,
    pub escrow_cents: i64,
    pub remaining_weeks: u32,
    pub next_delivery: Tick,
    pub status: ContractStatus,
    pub shortfalls: MilliLots,
    /// Baseline relationships renew at term instead of vanishing after a year.
    pub auto_renew: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shipment {
    pub seller: Handle,
    pub buyer: Handle,
    pub product: u16,
    pub quantity: MilliLots,
    pub from_region: u8,
    pub to_region: u8,
    pub arrive: Tick,
    pub unit_price_cents: i64,
    /// Goods value actually paid by the buyer, carried into inventory at receipt.
    pub goods_value_cents: i64,
    pub freight_cents: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyStatus {
    Active,
    Expired,
    Claimed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InsurancePolicy {
    pub insurer: Handle,
    pub insured: Handle,
    pub facility: Handle,
    pub region: u8,
    pub limit_cents: i64,
    pub deductible_cents: i64,
    pub weekly_premium_cents: i64,
    pub start: Tick,
    pub term_weeks: u32,
    pub earned_premium_cents: i64,
    pub claim_reserve_cents: i64,
    pub status: PolicyStatus,
    pub specialist: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DealKind {
    SharePurchase,
    AssetPurchase,
    Merger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DealState {
    Draft,
    Funded,
    Diligence,
    OfferOpen,
    Review,
    Settled,
    Withdrawn,
    Expired,
    Rejected,
}

impl DealState {
    pub const fn terminal(self) -> bool {
        matches!(
            self,
            DealState::Settled | DealState::Withdrawn | DealState::Expired | DealState::Rejected
        )
    }

    pub const fn key(self) -> &'static str {
        match self {
            DealState::Draft => "draft",
            DealState::Funded => "funded",
            DealState::Diligence => "diligence",
            DealState::OfferOpen => "offer_open",
            DealState::Review => "review",
            DealState::Settled => "settled",
            DealState::Withdrawn => "withdrawn",
            DealState::Expired => "expired",
            DealState::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deal {
    pub buyer: Handle,
    pub target: Handle,
    pub kind: DealKind,
    pub state: DealState,
    /// Maximum cash price plus fee held in escrow.
    pub escrow_cents: i64,
    pub offer_price_cents: i64,
    pub max_shares: i64,
    pub committed_shares: i64,
    pub started: Tick,
    pub diligence_until: Tick,
    pub offer_deadline: Tick,
    pub review_deadline: Option<Tick>,
    /// Frozen fair value per share at bid time, to avoid bidding against own mark push.
    pub frozen_fair_cents: i64,
    pub review_required: bool,
    pub remedy_share_bps: i32,
    pub hostile: bool,
    pub diligence_paid: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyRequest {
    pub kind: PolicyKind,
    pub region: u8,
    pub fee_cents: i64,
    pub created: Tick,
    pub decision: Tick,
    pub success: bool,
    pub resolved: bool,
    pub cooldown_until: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyKind {
    PermitAcceleration,
    TrainingGrant,
    TaxAdjustment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegimeKind {
    Recession,
    Boom,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MacroState {
    pub policy_rate_bps: i32,
    pub inflation_bps: i32,
    pub real_growth_bps: i32,
    pub tech_frontier: u8,
    pub price_index_bps: i64,
    pub regime: Option<(RegimeKind, Tick)>,
    pub rng: Rng,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingEvent {
    pub id: u64,
    pub card: String,
    pub subject_firm: Option<Handle>,
    pub region: Option<u8>,
    pub deadline: Tick,
    pub opened: Tick,
    pub mandatory: bool,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventState {
    pub pending: Vec<PendingEvent>,
    pub next_id: u64,
    /// (card, subject) -> last week opened.
    pub cooldowns: BTreeMap<(String, u64), u32>,
    pub rng: Rng,
    pub last_quarter_check: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShareOrder {
    pub holder: HolderId,
    pub firm: Handle,
    pub side: OrderSide,
    pub qty: i64,
    pub limit_price_cents: i64,
    pub reserved_cents: i64,
    pub placed: Tick,
    pub expires: Tick,
    pub filled: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Exchange {
    pub orders: Slab<ShareOrder>,
    pub last_clear: Tick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutsideHolder {
    pub name: String,
    pub capital_cents: i64,
    pub reservation_factor_bps: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutsideSectors {
    /// Lender and institutional investor capital, finite by construction.
    pub lenders: Vec<OutsideHolder>,
    pub investors: Vec<OutsideHolder>,
    pub advocacy_book: Ledger,
    pub legal_services_book: Ledger,
    /// Outside gateway settlement account. Imports and exports post here exactly once.
    pub gateway_book: Ledger,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Investor {
    pub name: String,
    pub cash_cents: i64,
    pub book: Ledger,
    pub personal_loans_cents: i64,
    pub primary_group: Option<Handle>,
    pub managed_firms: Vec<Handle>,
    pub recovery_used: u8,
    pub last_recovery_week: u32,
    pub assisted_score: bool,
    pub career_losses_cents: i64,
    pub realised_gains_cents: i64,
    pub starting_net_worth_cents: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioKind {
    Founder,
    Guided,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScenarioMode {
    Sandbox,
    FiveYear,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Score {
    pub week: Tick,
    pub return_component: i32,
    pub resilience_component: i32,
    pub control_component: i32,
    pub conduct_component: i32,
    pub total: i32,
    pub assisted: bool,
    pub personal_net_worth_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewsItem {
    pub week: Tick,
    pub key: String,
    pub subject: String,
    pub amount_cents: i64,
    pub quantity: MilliLots,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AcceptedCommand {
    pub sequence: u64,
    pub command_id_high: u64,
    pub command_id_low: u64,
    pub actor: Handle,
    pub submitted: Tick,
    pub accepted: Tick,
    pub revision: u64,
    pub intent_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub tick: Tick,
    pub revision: u64,
    pub seed: u64,
    pub dataset_hash: String,
    pub scenario: ScenarioKind,
    pub mode: ScenarioMode,
    pub difficulty: Difficulty,
    pub investor: Investor,
    pub firms: Slab<Firm>,
    pub facilities: Slab<Facility>,
    pub contracts: Slab<Contract>,
    pub shipments: Vec<Shipment>,
    pub loans: Slab<Loan>,
    pub bonds: Slab<Bond>,
    pub insurance: Slab<InsurancePolicy>,
    pub deals: Slab<Deal>,
    pub regions: Vec<RegionState>,
    pub government: Government,
    pub gateways: Vec<Gateway>,
    pub exchange: Exchange,
    pub outside: OutsideSectors,
    pub macro_state: MacroState,
    pub events: EventState,
    pub news: Vec<NewsItem>,
    pub accepted_commands: Vec<AcceptedCommand>,
    pub command_nonces: BTreeMap<u128, Tick>,
    pub next_sequence: u64,
    pub score: Option<Score>,
    /// Weeks in which the investor carried past-due liabilities, for the resilience score.
    pub weeks_with_past_due: u32,
    pub weeks_elapsed: u32,
    pub regulatory_breaches: u32,
    pub failed_worker_settlements: u32,
    /// Group membership cache, invalidated on ownership transitions.
    pub group_cache: BTreeMap<Handle, Vec<Handle>>,
    pub pending_pause: Option<String>,
}

impl World {
    pub fn firm(&self, handle: Handle) -> Option<&Firm> {
        self.firms.get(handle)
    }

    pub fn firm_mut(&mut self, handle: Handle) -> Option<&mut Firm> {
        self.firms.get_mut(handle)
    }

    pub fn region_index(&self, code: &str) -> Option<u8> {
        self.regions
            .iter()
            .position(|region| region.code == code)
            .map(|index| index as u8)
    }

    pub fn region(&self, index: u8) -> &RegionState {
        &self.regions[index as usize]
    }

    pub fn region_mut(&mut self, index: u8) -> &mut RegionState {
        &mut self.regions[index as usize]
    }

    /// Combined demand modifier for one regional product market, in basis points.
    ///
    /// Each active cause composes once; expired modifiers are ignored and pruned by the tick.
    pub fn demand_ratio_bps(&self, region: u8, product: Option<u16>) -> i64 {
        let Some(state) = self.regions.get(region as usize) else {
            return 10_000;
        };
        let mut ratio = 10_000i64;
        for (_, subject, bps, until) in &state.demand_modifiers {
            if until.0 <= self.tick.0 {
                continue;
            }
            if let Some(subject) = subject
                && Some(*subject) != product
            {
                continue;
            }
            ratio = ratio * (10_000 + *bps as i64) / 10_000;
        }
        ratio.clamp(3_000, 20_000)
    }

    /// Combined physical supply modifier for one regional product, in basis points.
    pub fn supply_ratio_bps(&self, region: u8, product: u16) -> i64 {
        let Some(state) = self.regions.get(region as usize) else {
            return 10_000;
        };
        let mut ratio = 10_000i64;
        for (_, subject, bps, until) in &state.supply_modifiers {
            if until.0 <= self.tick.0 {
                continue;
            }
            if let Some(subject) = subject
                && *subject != product
            {
                continue;
            }
            ratio = ratio * (10_000 + *bps as i64) / 10_000;
        }
        ratio.clamp(1_000, 20_000)
    }

    /// Drop modifiers whose window has closed.
    pub fn prune_modifiers(&mut self) {
        for region in &mut self.regions {
            region
                .demand_modifiers
                .retain(|(_, _, _, until)| until.0 > self.tick.0);
        }
    }

    pub fn investor_holds(&self, firm: Handle) -> i64 {
        self.firms
            .get(firm)
            .map(|firm| firm.shares.qty(HolderId::Investor))
            .unwrap_or(0)
    }

    /// Firms whose votes the investor directly controls.
    pub fn investor_firms(&self) -> Vec<Handle> {
        self.firms
            .iter()
            .filter(|(_, firm)| firm.shares.controller() == Some(HolderId::Investor))
            .map(|(handle, _)| handle)
            .collect()
    }
}

/// Canonical state hash used by replay and determinism tests.
///
/// The postcard encoding is deterministic: struct field order is declaration order, maps are
/// ordered, and no floats are stored. The hash covers authoritative state only.
pub fn canonical_hash(world: &World) -> String {
    match postcard::to_allocvec(world) {
        Ok(bytes) => {
            cw_core::hash::hex(&cw_core::hash::digest(cw_core::hash::DOMAIN_STATE, &bytes))
        }
        Err(_) => String::from("unhashable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lot(qty: i64, cost: i64) -> ShareLot {
        ShareLot {
            qty,
            cost_cents: cost,
        }
    }

    #[test]
    fn exactly_fifty_percent_is_not_control() {
        let mut holders = BTreeMap::new();
        holders.insert(HolderId::Investor, lot(500, 500));
        holders.insert(HolderId::Outside(0), lot(500, 500));
        let book = ShareBook::new(holders);
        assert_eq!(book.controller(), None);
        assert!(!book.meets_squeeze_out(HolderId::Investor));
    }

    #[test]
    fn above_fifty_percent_is_control() {
        let mut holders = BTreeMap::new();
        holders.insert(HolderId::Investor, lot(501, 501));
        holders.insert(HolderId::Outside(0), lot(499, 499));
        let book = ShareBook::new(holders);
        assert_eq!(book.controller(), Some(HolderId::Investor));
    }

    #[test]
    fn treasury_has_no_votes() {
        let mut holders = BTreeMap::new();
        holders.insert(HolderId::Investor, lot(400, 400));
        holders.insert(HolderId::Treasury, lot(600, 600));
        let book = ShareBook::new(holders);
        assert_eq!(book.eligible_votes(), 400);
        assert_eq!(book.controller(), Some(HolderId::Investor));
        assert!(!book.meets_squeeze_out(HolderId::Treasury));
        assert_eq!(book.vote_bps(HolderId::Investor), 10_000);
    }

    #[test]
    fn transfer_moves_cost_basis_proportionally() {
        let mut holders = BTreeMap::new();
        holders.insert(HolderId::Investor, lot(1000, 100_000));
        let mut book = ShareBook::new(holders);
        book.transfer(HolderId::Investor, HolderId::Outside(1), 250, 40_000);
        assert_eq!(book.qty(HolderId::Investor), 750);
        assert_eq!(book.qty(HolderId::Outside(1)), 250);
        assert_eq!(book.holders[&HolderId::Investor].cost_cents, 75_000);
        assert_eq!(book.holders[&HolderId::Outside(1)].cost_cents, 40_000);
        assert_eq!(book.issued, 1000);
    }

    #[test]
    fn demand_modifiers_compose_once_and_expire() {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            701,
            ScenarioKind::Founder,
            ScenarioMode::Sandbox,
        );
        world.tick = Tick(10);
        world.regions[0]
            .demand_modifiers
            .push(("recession".to_owned(), None, -1_000, Tick(20)));
        world.regions[0]
            .demand_modifiers
            .push(("crop".to_owned(), Some(0), -500, Tick(20)));
        world.regions[0]
            .demand_modifiers
            .push(("old".to_owned(), None, -2_000, Tick(9)));
        let product = world.demand_ratio_bps(0, Some(0));
        assert_eq!(
            product, 8_550,
            "10 percent and 5 percent compose, expired ignored"
        );
        assert_eq!(
            world.demand_ratio_bps(0, Some(1)),
            9_000,
            "product-specific cause skipped"
        );
        world.tick = Tick(25);
        world.prune_modifiers();
        assert!(world.regions[0].demand_modifiers.is_empty());
        assert_eq!(world.demand_ratio_bps(0, Some(0)), 10_000);
    }

    #[test]
    fn stock_issue_consumes_carrying_cost_exactly() {
        let mut lot = StockLot::default();
        lot.receive(MilliLots::from_lots(3), Cents(1000));
        let mut carry = 0;
        let first = lot.issue(MilliLots(1000), &mut carry);
        let second = lot.issue(MilliLots(1000), &mut carry);
        let third = lot.issue(MilliLots(1000), &mut carry);
        assert_eq!(first.0 + second.0 + third.0, 1000);
        assert_eq!(lot.qty, MilliLots::ZERO);
        assert_eq!(lot.cost_cents, 0);
        assert_eq!(carry, 0);
    }
}
