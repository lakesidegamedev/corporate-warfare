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
    /// The state, after a nationalisation. Holds the shares of a state firm until privatised.
    State,
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

/// How a good's asking price is set each week when the player chooses a posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PricePosture {
    /// Basis points against the market price of the good, -5000 to 5000: -2000 undercuts by a fifth.
    Market { bps: i32 },
    /// Basis points over the firm's own cost per lot.
    CostPlus { bps: i32 },
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
    /// Equipment still to be paid for, due when construction completes. A new plant pays its
    /// construction service up front and its equipment on delivery, so a build does not tie up
    /// the whole capex for the build period.
    #[serde(default)]
    pub equipment_due_cents: i64,
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
    /// Temporary capacity cap from an outage card: (cap bps, last affected week exclusive).
    pub outage: Option<(i32, Tick)>,
    /// Standing stock target and seasonal build for this plant's inputs.
    #[serde(default)]
    pub stock_plan: crate::economy::storage::StockPlan,
    /// Standing shipping policy for the freight this division pays for.
    #[serde(default)]
    pub ship: crate::economy::logistics::ShipPolicy,
    /// A carrier's pinned lanes, as the other region of each. Empty means automatic.
    #[serde(default)]
    pub serves: Vec<u8>,
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

    /// Last week's physical output of one product: the primary output as recorded, or a
    /// by-product in its fixed yield to the primary. Zero when the plant does not make it.
    pub fn last_made(&self, dataset: &Dataset, product: &str) -> MilliLots {
        let recipe = &dataset.recipes[self.recipe as usize];
        if recipe.output == product {
            return self.last_output;
        }
        let by = recipe.lots_of(product);
        if by <= 0 || recipe.weekly_output_lots <= 0 {
            return MilliLots::ZERO;
        }
        MilliLots(self.last_output.0 * by / recipe.weekly_output_lots)
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
    /// Weekly logistics and operations programme that builds logistics strength.
    #[serde(default)]
    pub logistics_budget_cents: i64,
    /// Weekly budget for paid skill training. Training moves workers between tiers; it never
    /// creates workers.
    #[serde(default)]
    pub training_budget_cents: i64,
    /// Weekly quality investment by product, bounded by the prescribed quality ceiling.
    #[serde(default)]
    pub quality_budgets_cents: BTreeMap<u16, i64>,
    /// Desired weeks of input cover for automatic purchasing.
    pub input_cover_weeks: i32,
    /// Goods whose price the player set by hand. Every other good a player company makes is
    /// priced automatically from sell-through each week.
    #[serde(default)]
    pub manual_prices: std::collections::BTreeSet<u16>,
    /// Pricing posture by good, after A House Divided: a percentage against the market price
    /// or a markup over the firm's own cost per lot. Applied each week; a posture replaces both
    /// automatic pricing and a hand-set price for that good.
    #[serde(default)]
    pub price_postures: BTreeMap<u16, PricePosture>,
    /// True once the player sets a production target by hand; automatic planning stops until
    /// the player turns it back on.
    #[serde(default)]
    pub manual_production: bool,
}

impl Default for Policies {
    fn default() -> Self {
        Policies {
            prices: BTreeMap::new(),
            wage_offer: [0; 3],
            production_target_bps: 10_000,
            marketing_budget_cents: 0,
            research_budget_cents: 0,
            logistics_budget_cents: 0,
            training_budget_cents: 0,
            quality_budgets_cents: BTreeMap::new(),
            input_cover_weeks: 4,
            manual_prices: Default::default(),
            price_postures: Default::default(),
            manual_production: false,
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
    /// Per-depositor claims. The aggregate must equal the bank's deposit liability.
    #[serde(default)]
    pub depositors: BTreeMap<HolderId, i64>,
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
    /// Workers released by week, kept for four weeks so a failed lobbying campaign can be
    /// judged against concurrent mass layoffs.
    pub layoffs: Vec<(u32, i64)>,
    /// Employees at each of the last five week closes, newest last, for churn detection.
    pub staff_history: Vec<i64>,
    /// Largest outside vote share at the last week close, so a hostile stake is reported when
    /// it crosses a quarter, not every week it stays there.
    pub outside_peak_bps: i32,
    /// Optional secondary sector type; `industry` is the primary.
    pub secondary_industry: Option<u8>,
    /// Focus change transition and lock, as tick numbers.
    pub focus_transition_until: u32,
    pub focus_cooldown_until: u32,
    /// Next tick this firm may raid each (region, sector type) cell again.
    #[serde(default)]
    pub raid_ready: BTreeMap<(u8, u8), u32>,
    /// Losses carried forward against future taxable profit, in cents.
    #[serde(default)]
    pub tax_loss_carry_cents: i64,
    /// Logistics and operations strength, 0..200, built by the logistics budget.
    pub logistics_strength: i32,
    pub policy_requests: Vec<PolicyRequest>,
    pub guarantees: Vec<Guarantee>,
    pub last_strategic_week: u32,
    pub merged_into: Option<Handle>,
    /// The week the state took the firm over, while it is state owned.
    #[serde(default)]
    pub state_owned_since: Option<u32>,
    /// The week the firm last received a state rescue.
    #[serde(default)]
    pub rescued_week: u32,
    /// Goods the firm's operating plants use as inputs, refreshed weekly. Read by vertical
    /// merger review.
    #[serde(default)]
    pub inputs_used: Vec<u16>,
    /// The cast member running the firm, for rivals with a named chief executive. Their
    /// temperament is read through `economy::cast::temperament`.
    #[serde(default)]
    pub ceo: Option<crate::economy::cast::PersonId>,
    pub founding_week: Tick,
    pub bank: Option<BankModel>,
    pub insurer: Option<InsurerModel>,
    pub valuation: FirmValue,
    /// Weekly EBITDA a seed firm is assumed to earn until its first quarter closes. See
    /// `valuation::annualised_ebitda_with_prior`.
    #[serde(default)]
    pub ebitda_prior_weekly_cents: Option<i64>,
    /// Unspent reservation for submitted orders, released after settlement.
    pub reserved_cash_cents: i64,
    pub last_week_units_sold: BTreeMap<(u8, u16), MilliLots>,
    /// Thirteen most recent weeks of unit sales by (region, product), oldest first.
    pub sales_history: Vec<BTreeMap<(u8, u16), i64>>,
    pub quality_bps: BTreeMap<u16, i32>,
    /// Intercompany inventory layers by (seller, product, region). The margin is the unrealised
    /// profit still carried in the buyer's inventory and is eliminated from consolidated assets.
    #[serde(default)]
    pub intercompany_inventory: BTreeMap<(Handle, u16, u8), IntercompanyLayer>,
    /// Held goods, buy orders and storage costs. See `economy::storage`.
    #[serde(default)]
    pub storage: crate::economy::storage::StorageBook,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct IntercompanyLayer {
    pub qty: MilliLots,
    pub margin_cents: i64,
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

    /// Record goods received from another group member, including the unrealised margin.
    pub fn record_intercompany_receipt(
        &mut self,
        seller: Handle,
        product: u16,
        region: u8,
        qty: MilliLots,
        margin_cents: i64,
    ) {
        if qty.0 <= 0 {
            return;
        }
        let layer = self
            .intercompany_inventory
            .entry((seller, product, region))
            .or_default();
        layer.qty = MilliLots(layer.qty.0 + qty.0);
        layer.margin_cents += margin_cents.max(0);
    }

    /// Issue stock and release the proportional unrealised margin from intercompany layers.
    pub fn issue_stock(&mut self, product: u16, region: u8, qty: MilliLots) -> Cents {
        let available = self.stock(product, region);
        if qty.0 <= 0 || available.0 <= 0 {
            return Cents::ZERO;
        }
        let take = qty.min(available);
        let ratio_take = take.0;
        let ratio_total = available.0;
        let keys: Vec<(Handle, u16, u8)> = self
            .intercompany_inventory
            .keys()
            .filter(|(_, p, r)| *p == product && *r == region)
            .copied()
            .collect();
        for key in keys {
            let Some(layer) = self.intercompany_inventory.get_mut(&key) else {
                continue;
            };
            if take.0 == available.0 {
                self.intercompany_inventory.remove(&key);
                continue;
            }
            let removed_qty = (layer.qty.0 * ratio_take / ratio_total).min(layer.qty.0);
            let removed_margin = (layer.margin_cents * ratio_take / ratio_total)
                .min(layer.margin_cents)
                .max(0);
            layer.qty = MilliLots(layer.qty.0 - removed_qty);
            layer.margin_cents -= removed_margin;
            if layer.qty.0 <= 0 {
                self.intercompany_inventory.remove(&key);
            }
        }
        let mut carry = 0i64;
        self.stock_lot_mut(product, region).issue(take, &mut carry)
    }

    /// Add one fill to this week's unit sales. A firm can sell the same good in one state to
    /// households, businesses, government and the gateway in a single week; every fill counts.
    pub fn record_sale(&mut self, region: u8, product: u16, qty: MilliLots) {
        let entry = self
            .last_week_units_sold
            .entry((region, product))
            .or_insert(MilliLots::ZERO);
        *entry = entry.checked_add(qty).unwrap_or(*entry);
    }

    /// Unit sales of the last closed week. Outside a tick the open week is empty, so readers
    /// between ticks (reports, rival AI) use this.
    pub fn last_closed_sales(&self) -> Option<&BTreeMap<(u8, u16), i64>> {
        self.sales_history.last()
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
    /// Disposable baseline income after labour tax, used to size the savings supplement.
    #[serde(default)]
    pub baseline_income_cents: i64,
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
    /// Wages actually paid by named employers in the current week. Unpaid deferred wages are
    /// not household income and are not taxed.
    #[serde(default)]
    pub paid_named_wages_cents: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Government {
    pub book: Ledger,
    pub procurement_budget_cents: i64,
    pub healthcare_budget_cents: i64,
    pub training_budget_cents: i64,
    pub last_tax_receipts_cents: i64,
    #[serde(default)]
    pub tax_receipts_history: Vec<i64>,
    #[serde(default)]
    pub week_tax_receipts_cents: i64,
    /// Transfer support that the fiscal budget or treasury could not fund this week.
    #[serde(default)]
    pub unpaid_transfer_need_cents: i64,
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
    /// Lots imported into and exported out of the region last week, and a smoothed import
    /// volume (a quarter weight on the latest week). Imports are demand local plants did not
    /// meet: the public "room to build" signal.
    #[serde(default)]
    pub last_import_lots: i64,
    #[serde(default)]
    pub last_export_lots: i64,
    #[serde(default)]
    pub smoothed_import_lots: i64,
}

/// The `Lender::Outside` index that stands for the government.
pub const STATE_LENDER: u32 = u32::MAX;

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
    /// Principal that fell due and was not paid. It stays owed to this lender, is collected
    /// first at the next service, and keeps the borrower past due until it clears.
    pub arrears_cents: i64,
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

/// Government procurement contract: a funded weekly delivery with a prepaid deposit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Procurement {
    pub supplier: Handle,
    pub region: u8,
    pub quantity_weekly: MilliLots,
    pub price_cents: i64,
    pub remaining_weeks: u32,
    pub next_delivery: Tick,
    /// Deposit still unearned by the supplier.
    pub unearned_cents: i64,
    pub shortfall_weeks: u8,
    pub status: ContractStatus,
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
    /// Seller margin on the delivered goods, eliminated if the buyer is in the same group.
    #[serde(default)]
    pub margin_cents: i64,
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
    /// Week in which a recognised claim becomes payable.
    #[serde(default)]
    pub claim_payable_week: u32,
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
    /// The player investor has tendered its shares; they move only at settlement.
    pub investor_tendered: bool,
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
    FuelShock,
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
    /// The business cycle and the central bank. See `economy::macro_cycle`.
    #[serde(default)]
    pub cycle: Cycle,
}

/// How the economy stands on day one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EconomyStart {
    /// As the scenario's year really was: for 2000, a late boom on stretched credit.
    #[default]
    Historic,
    /// On trend, inflation at target, little credit built up.
    Calm,
    /// Running hot with credit piling up: strong demand now, a bust likely later.
    Boom,
    /// Already in a downturn: weak demand, cheap money, a recovery ahead.
    Downturn,
}

impl EconomyStart {
    pub const fn key(self) -> &'static str {
        match self {
            EconomyStart::Historic => "historic",
            EconomyStart::Calm => "calm",
            EconomyStart::Boom => "boom",
            EconomyStart::Downturn => "downturn",
        }
    }

    pub fn from_key(key: &str) -> EconomyStart {
        match key {
            "calm" => EconomyStart::Calm,
            "boom" => EconomyStart::Boom,
            "downturn" => EconomyStart::Downturn,
            _ => EconomyStart::Historic,
        }
    }
}

/// Choices made on the new-game screen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartOptions {
    pub difficulty: cw_data::Difficulty,
    pub economy: EconomyStart,
    /// Starting capital in place of the difficulty's, if the player set one.
    pub capital_cents: Option<i64>,
}

impl StartOptions {
    pub fn capital(&self) -> i64 {
        self.capital_cents
            .unwrap_or_else(|| self.difficulty.starting_capital_cents())
    }
}

/// Where the economy is in its cycle, for the player and for event cards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CyclePhase {
    #[default]
    Expansion,
    Boom,
    Slowdown,
    Recession,
    Recovery,
}

impl CyclePhase {
    pub const fn key(self) -> &'static str {
        match self {
            CyclePhase::Expansion => "expansion",
            CyclePhase::Boom => "boom",
            CyclePhase::Slowdown => "slowdown",
            CyclePhase::Recession => "recession",
            CyclePhase::Recovery => "recovery",
        }
    }
}

/// One week of the economy's closing figures, kept for the charts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroPoint {
    pub week: u32,
    pub gdp_index: i64,
    pub output_gap_bps: i32,
    pub inflation_bps: i32,
    pub policy_rate_bps: i32,
    pub phase: CyclePhase,
}

/// The business cycle: output against its trend, the credit build-up that makes busts likely,
/// expected inflation, and the central bank's meeting calendar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cycle {
    /// Output against its trend: +150 is 1.5 percent above potential.
    pub output_gap_bps: i32,
    /// Weekly push on the gap, carried from week to week.
    pub momentum_bps: i32,
    /// Trend real growth a year.
    pub potential_growth_bps: i32,
    /// Real output and its trend, 1,000,000 at the start.
    pub gdp_index: i64,
    pub potential_index: i64,
    /// Credit built up in good times; the higher it runs the likelier a bust.
    pub credit_bps: i32,
    /// Lingering drag after a bust while borrowers pay debt down.
    pub deleveraging_bps: i32,
    pub expected_inflation_bps: i32,
    pub target_inflation_bps: i32,
    /// The real policy rate that neither pushes nor pulls on the economy.
    pub neutral_real_bps: i32,
    pub next_meeting: u32,
    /// The last decision in basis points: positive is a rise.
    pub last_decision_bps: i32,
    pub phase: CyclePhase,
    pub history: Vec<MacroPoint>,
    /// Who chairs the central bank, and the bank's recent decisions, newest last.
    #[serde(default)]
    pub chair: CentralBanker,
    #[serde(default)]
    pub decisions: Vec<RateDecision>,
}

/// The central bank chair: a stance from dovish (-100) to hawkish (+100) and a four-year term.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CentralBanker {
    pub name: String,
    pub stance: i32,
    pub appointed_week: u32,
    pub term_ends: u32,
    /// The cast member in the chair, when there is one.
    #[serde(default)]
    pub person: Option<crate::economy::cast::PersonId>,
}

impl Default for CentralBanker {
    fn default() -> Self {
        CentralBanker {
            name: "Alan Whitcombe".to_owned(),
            stance: 35,
            appointed_week: 0,
            term_ends: 130,
            person: None,
        }
    }
}

impl CentralBanker {
    pub fn stance_label(&self) -> &'static str {
        match self.stance {
            s if s >= 50 => "hawkish",
            s if s >= 15 => "leaning hawkish",
            s if s > -15 => "centrist",
            s if s > -50 => "leaning dovish",
            _ => "dovish",
        }
    }
}

/// One central bank meeting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateDecision {
    pub week: u32,
    pub change_bps: i32,
    pub rate_bps: i32,
    pub reason: String,
}

impl Cycle {
    /// The opening cycle for a chosen starting economy.
    pub fn for_start(start: EconomyStart, policy_rate_bps: i32) -> (Cycle, i32, i32) {
        let mut c = Cycle::default();
        let (rate, inflation) = match start {
            EconomyStart::Historic => (policy_rate_bps, 300),
            EconomyStart::Calm => {
                c.output_gap_bps = 0;
                c.credit_bps = 150;
                c.expected_inflation_bps = 200;
                c.phase = CyclePhase::Expansion;
                (425, 200)
            }
            EconomyStart::Boom => {
                c.output_gap_bps = 350;
                c.momentum_bps = 20;
                c.credit_bps = 800;
                c.expected_inflation_bps = 280;
                c.phase = CyclePhase::Boom;
                (550, 320)
            }
            EconomyStart::Downturn => {
                c.output_gap_bps = -300;
                c.momentum_bps = -15;
                c.credit_bps = 100;
                c.deleveraging_bps = 300;
                c.expected_inflation_bps = 150;
                c.phase = CyclePhase::Recession;
                (300, 120)
            }
        };
        c.gdp_index = c.potential_index * (10_000 + c.output_gap_bps as i64) / 10_000;
        (c, rate, inflation)
    }
}

impl Default for Cycle {
    fn default() -> Self {
        Cycle {
            output_gap_bps: 150,
            momentum_bps: 0,
            potential_growth_bps: 300,
            gdp_index: 1_015_000,
            potential_index: 1_000_000,
            credit_bps: 600,
            deleveraging_bps: 0,
            expected_inflation_bps: 250,
            target_inflation_bps: 200,
            neutral_real_bps: 200,
            next_meeting: 6,
            last_decision_bps: 0,
            phase: CyclePhase::Boom,
            history: Vec::new(),
            chair: CentralBanker::default(),
            decisions: Vec::new(),
        }
    }
}

/// Weeks of economy history kept: five years.
pub const MACRO_HISTORY_WEEKS: usize = 260;

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
    /// The deal a tender card is about. Tender cards are addressed to the investor as a
    /// shareholder, not to the target company, so they carry the deal instead of a subject.
    pub deal: Option<Handle>,
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
    /// Cash the outside account keeps this week. It may draw at most one percent of its
    /// opening cash net of the week's import receipts, so a corridor running a trade surplus
    /// sees exports taper toward balance instead of the account emptying in one step.
    #[serde(default)]
    pub gateway_floor_cents: i64,
}

impl OutsideSectors {
    /// Cash the gateway may still spend this week on exports and public works.
    pub fn gateway_spendable(&self) -> i64 {
        (self.gateway_book.cash() - self.gateway_floor_cents).max(0)
    }
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

fn default_start_year() -> i32 {
    cw_core::calendar::START_YEAR
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
    /// Growth of personal net worth over the start.
    pub return_component: i32,
    #[serde(default)]
    pub expansion_component: i32,
    #[serde(default)]
    pub leadership_component: i32,
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
    /// Calendar year of week 1 in the scenario's own history (1981 for Texas).
    #[serde(default = "default_start_year")]
    pub start_year: i32,
    pub scenario: ScenarioKind,
    pub mode: ScenarioMode,
    pub difficulty: Difficulty,
    pub investor: Investor,
    pub firms: Slab<Firm>,
    pub facilities: Slab<Facility>,
    pub contracts: Slab<Contract>,
    #[serde(default)]
    pub procurements: Slab<Procurement>,
    pub shipments: Vec<Shipment>,
    pub loans: Slab<Loan>,
    pub bonds: Slab<Bond>,
    pub insurance: Slab<InsurancePolicy>,
    pub deals: Slab<Deal>,
    pub regions: Vec<RegionState>,
    pub government: Government,
    pub gateways: Vec<Gateway>,
    /// The outside world's commodity price paths. See `economy::world_prices`.
    #[serde(default)]
    pub world_prices: crate::economy::world_prices::WorldPrices,
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
    /// The character cast's posts and chief executive turnover. See `economy::cast`.
    #[serde(default)]
    pub cast: crate::economy::cast::CastState,
    /// Construction lots already committed to build orders this week, by contractor.
    #[serde(default)]
    pub construction_orders: BTreeMap<Handle, i64>,
    /// The most recent market raids, newest last, bounded.
    #[serde(default)]
    pub raids: Vec<RaidRecord>,
    /// The board's current quarterly objective for the player's primary company.
    #[serde(default)]
    pub objective: Option<crate::economy::objectives::Objective>,
    #[serde(default)]
    pub objective_history: Vec<crate::economy::objectives::ObjectiveResult>,
    /// Weekly closing prices of every good and share, oldest first, at most a year.
    #[serde(default)]
    pub price_history: Vec<PricePoint>,
    /// Each firm's results in four-week periods with closing balances, oldest first, at most
    /// five years. Presentation history for the stats screens; no rule reads it.
    #[serde(default)]
    pub firm_history: BTreeMap<Handle, Vec<PeriodClose>>,
    /// The governing party, the next election and the laws it sets. See `economy::politics`.
    #[serde(default)]
    pub politics: Politics,
    /// Freight lanes, carriers and lane contracts. See `economy::logistics`.
    #[serde(default)]
    pub logistics: crate::economy::logistics::LogisticsState,
    /// Pipeline spills, strikes, fires, lane outages and harvest failures. See
    /// `economy::systemic`.
    #[serde(default)]
    pub systemic: crate::economy::systemic::SystemicState,
    /// Transfer pricing, reputation, bonds, joint ventures and activists. See
    /// `economy::structure`.
    #[serde(default)]
    pub structure: crate::economy::structure::StructureState,
    /// Grudges, alliances and rival move log. See `economy::rivalry`.
    #[serde(default)]
    pub rivalry: crate::economy::rivalry::RivalryState,
    /// Exchange rates, foreign sales awaiting collection and recorded foreign exchange gains.
    /// Not part of the world's postcard encoding, so a world with one money encodes and hashes
    /// exactly as it did before currencies: a world with several saves it in its own trailer
    /// and `canonical_hash` folds it in. See `economy::fx`.
    #[serde(skip)]
    pub fx: crate::economy::fx::FxState,
    /// Long presentation history: ten years of weekly closes and each firm's weekly and
    /// quarterly statements with cash flow and divisions. Outside the postcard encoding and the
    /// hash; saved in its own trailer, so an older save loads with it empty. No rule reads it.
    /// See `economy::history`.
    #[serde(skip)]
    pub archive: crate::economy::history::Archive,
}

/// Who governs. The two parties differ on tax, the wage floor, merger review and unions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Party {
    #[default]
    Business,
    Labour,
}

impl Party {
    pub const fn key(self) -> &'static str {
        match self {
            Party::Business => "business",
            Party::Labour => "labour",
        }
    }

    pub const fn other(self) -> Party {
        match self {
            Party::Business => Party::Labour,
            Party::Labour => Party::Business,
        }
    }
}

/// How easily workplaces organise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnionLaw {
    Weak,
    #[default]
    Standard,
    Strong,
}

impl UnionLaw {
    pub const fn key(self) -> &'static str {
        match self {
            UnionLaw::Weak => "weak",
            UnionLaw::Standard => "standard",
            UnionLaw::Strong => "strong",
        }
    }

    /// Weekly organisation rise where pay is under the market, and decay where it is not.
    pub const fn drift_bps(self) -> (i32, i32) {
        match self {
            UnionLaw::Weak => (15, 8),
            UnionLaw::Standard => (25, 5),
            UnionLaw::Strong => (40, 3),
        }
    }
}

/// What the government does with a failing strategic firm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rescue {
    /// Nothing: the normal restructuring or liquidation runs.
    #[default]
    None,
    /// A state loan clears the arrears and the firm stays private.
    Bailout,
    /// The state buys the shares and runs the firm until it is privatised.
    Nationalise,
}

impl Rescue {
    pub const fn key(self) -> &'static str {
        match self {
            Rescue::None => "none",
            Rescue::Bailout => "bailout",
            Rescue::Nationalise => "nationalise",
        }
    }
}

/// The laws companies live under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Laws {
    /// Tax on quarterly profit.
    pub corporate_tax_bps: i32,
    /// Floor on low-skill weekly wages, in cents.
    pub wage_floor_cents: i64,
    /// Combined regional share at which a merger goes to review.
    pub merger_review_bps: i64,
    pub unions: UnionLaw,
    /// Owned pipelines, rail and ships sell spare capacity to anyone at a regulated rate and
    /// cannot lock a lane up with contracts.
    #[serde(default)]
    pub common_carrier: bool,
    /// Share of a supplier's or customer's regional market at which buying it goes to review.
    /// Zero means vertical deals are not reviewed.
    #[serde(default)]
    pub vertical_review_bps: i64,
    /// The government buys and stores commodities in busts and sells in spikes.
    #[serde(default)]
    pub reserve: bool,
    #[serde(default)]
    pub rescue: Rescue,
    /// State firms are sold back to the market once they are sound.
    #[serde(default)]
    pub privatise: bool,
    /// Output cap for extraction goods in quota states, as a share of capacity, applied while
    /// their price is depressed. Zero means no quotas.
    #[serde(default)]
    pub quota_bps: i32,
    /// Bit i is set when the state at index i (states sorted by code) has quotas.
    #[serde(default)]
    pub quota_states: u64,
}

impl Default for Laws {
    fn default() -> Self {
        Laws {
            corporate_tax_bps: 2_500,
            wage_floor_cents: 34_000,
            merger_review_bps: 3_500,
            unions: UnionLaw::Standard,
            common_carrier: false,
            vertical_review_bps: 0,
            reserve: false,
            rescue: Rescue::None,
            privatise: false,
            quota_bps: 0,
            quota_states: 0,
        }
    }
}

/// A law change announced now and in force from `effective`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LawChange {
    pub announced: u32,
    pub effective: u32,
    pub laws: Laws,
}

/// One line of political news.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoliticalNews {
    pub week: u32,
    pub text: String,
    /// The cast member the item is about, shown beside it.
    #[serde(default)]
    pub person: Option<crate::economy::cast::PersonId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Politics {
    pub party: Party,
    pub since_week: u32,
    pub next_election: u32,
    /// The governing party's approval, 0 to 10,000.
    pub approval_bps: i32,
    pub laws: Laws,
    pub pending: Option<LawChange>,
    pub news: Vec<PoliticalNews>,
    /// The first week's programme for the starting party has been put in force.
    #[serde(default)]
    pub programmed: bool,
    /// Laws that changed so far, counting each announced change that took effect.
    #[serde(default)]
    pub law_changes: u32,
    /// The week the latest announced change took effect.
    #[serde(default)]
    pub last_effective: u32,
    /// The strategic reserve. See `economy::reserve`.
    #[serde(default)]
    pub reserve: crate::economy::reserve::Reserve,
    /// Rescues so far: bailouts, nationalisations and privatisations.
    #[serde(default)]
    pub bailouts: u32,
    #[serde(default)]
    pub nationalisations: u32,
    #[serde(default)]
    pub privatisations: u32,
    /// Regions whose extraction goods are under a quota this week, for display and rivals.
    #[serde(default)]
    pub quota_active: Vec<(u8, u16)>,
}

impl Default for Politics {
    fn default() -> Self {
        Politics {
            party: Party::Business,
            since_week: 0,
            // Week 45 is early November of the first game year, whatever the scenario's start year.
            next_election: 45,
            approval_bps: 5_000,
            laws: Laws::default(),
            pending: None,
            news: Vec::new(),
            programmed: false,
            law_changes: 0,
            last_effective: 0,
            reserve: Default::default(),
            bailouts: 0,
            nationalisations: 0,
            privatisations: 0,
            quota_active: Vec::new(),
        }
    }
}

/// Four weeks of one firm's results, with balances and share price at the period's close.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeriodClose {
    pub first_week: u32,
    pub weeks: u32,
    pub revenue: i64,
    pub cost_of_goods: i64,
    pub wages: i64,
    pub operating_expense: i64,
    pub depreciation: i64,
    pub net_interest: i64,
    pub tax: i64,
    pub other: i64,
    pub net_income: i64,
    pub cash: i64,
    pub assets: i64,
    pub liabilities: i64,
    pub equity: i64,
    pub share_price_cents: i64,
    pub shares_issued: i64,
}

/// Weeks in one history period.
pub const PERIOD_WEEKS: u32 = 4;
/// History periods kept per firm: five years.
pub const HISTORY_PERIODS: usize = 65;

/// One week's closing prices: each good's corridor price and each firm's share price.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricePoint {
    pub week: u32,
    pub goods: BTreeMap<u16, i64>,
    pub shares: BTreeMap<Handle, i64>,
}

/// Weeks of closing prices the world keeps.
pub const PRICE_HISTORY_WEEKS: usize = 52;

/// One market raid and its outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaidRecord {
    pub tick: u32,
    pub attacker: Handle,
    pub defender: Handle,
    pub region: u8,
    pub industry: u8,
    pub budget_cents: i64,
    pub chance_bps: i32,
    pub success: bool,
    pub blocks: u8,
    pub price_cents: i64,
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

    /// Weekly construction capacity of one contractor, in whole lots.
    pub fn construction_capacity(&self, dataset: &cw_data::Dataset, firm: Handle) -> i64 {
        let Some(entity) = self.firms.get(firm) else {
            return 0;
        };
        entity
            .facilities
            .iter()
            .filter_map(|handle| self.facilities.get(*handle))
            .filter(|facility| facility.status == crate::state::FacilityStatus::Operating)
            .map(|facility| {
                let recipe = &dataset.recipes[facility.recipe as usize];
                if recipe.output == "construction" {
                    recipe.weekly_output_lots * facility.blocks as i64 * facility.target_bps as i64
                        / 10_000
                } else {
                    0
                }
            })
            .sum()
    }

    /// Clear the week's construction commitments after public works have been served.
    pub fn clear_construction_orders(&mut self) {
        self.construction_orders.clear();
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
        Ok(mut bytes) => {
            if world.fx.active {
                match postcard::to_allocvec(&world.fx) {
                    Ok(fx) => bytes.extend_from_slice(&fx),
                    Err(_) => return String::from("unhashable"),
                }
            }
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
