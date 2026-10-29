//! Corporate structure and integration (1.7 preview 4).
//!
//! Everything here works the same in every scenario and runs on standing defaults. Players
//! rarely touch a setting; the engine raises an alert only when something needs a decision.
//!
//! * **Group supply and transfer pricing.** Divisions of one group buy each other's output
//!   before the open market does, through standing contracts the engine keeps in step with
//!   what each division burns and makes. Each supplying division carries one transfer price
//!   setting from cost (0) through market (10,000, the default) to market plus a markup.
//!   Selling to a group member below market moves value away from the supplier's minority
//!   shareholders (the squeeze), moves profit between states' tax bills, and a supplier that
//!   loses money for months is marked down by the market.
//! * **Reputation.** Each division carries a reputation built by quality investment and
//!   marketing, decaying slowly. It earns a price premium and loyal customers in the market.
//!   Raids cost reputation as well as plants.
//! * **Bonds.** Priced off the central bank's ten-year yield plus a credit spread from
//!   leverage, coverage and size; traded on a simple board at a mark price.
//! * **Carve-outs, listings, spin-offs, joint ventures.**
//! * **Activists.** Performance-driven campaigns with demands, a vote and consequences.
//! * **Temperament.** Rival decisions here read a weight table so the character AI of a later
//!   preview can replace the defaults without touching the rules.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use cw_core::calendar::Tick;
use cw_core::error::{
    ArithmeticFault, AuthorityReason, DomainError, DomainResult, EntityKind, SolvencyReason,
};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Ledger, Txn};
use cw_core::money::{Cents, allocate_cents};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::economy::{ownership, pricing, sectors};
use crate::state::{
    Bond, CeoRef, ContractStatus, FacilityStatus, FirmStatus, HolderId, INITIAL_SHARES, ShareLot,
    World,
};

/// Reputation of a division nobody has formed a view of yet, on a 0 to 10,000 scale.
pub const NEUTRAL_REP: i32 = 2_500;
/// Transfer setting for "at market", the default. 0 is at cost; above is market plus markup.
pub const TRANSFER_MARKET_BPS: i32 = 10_000;
pub const TRANSFER_MAX_BPS: i32 = 12_000;
/// Weeks after forming before a joint venture partner may buy out or exit.
pub const JV_LOCKUP_WEEKS: u32 = 26;
/// Premium a partner pays to buy the other out, basis points of fair value.
pub const JV_BUYOUT_PREMIUM_BPS: i64 = 11_000;
/// Right of first refusal price when the other partner takes a leaving partner's stake.
pub const JV_EXIT_PARTNER_BPS: i64 = 9_500;
/// Price when a leaving partner's stake goes to the outside market.
pub const JV_EXIT_MARKET_BPS: i64 = 8_500;
/// Weeks of squeeze history that count (the squeeze fades by 1/52 a week).
pub const SQUEEZE_DECAY_DEN: i64 = 52;
/// Marketing cost basis when a division's reputation is updated, weeks.
const LISTING_DISCOUNT_BPS: i64 = 9_500;
const ACTIVIST_COOLDOWN_WEEKS: u32 = 104;
const ACTIVIST_RESPONSE_WEEKS: u32 = 4;

// ---------------------------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------------------------

/// A standing supply contract between two group members, kept in step by `sync_supply`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupplyLink {
    pub contract: Handle,
    /// Seller's division (region, sector type) the transfer setting belongs to.
    pub region: u8,
    pub industry: u8,
    /// The last market price and transfer price the link was priced at, cents a lot.
    pub market_cents: i64,
    pub price_cents: i64,
    pub cost_cents: i64,
}

/// Repeat raids on one rival: how many recently, and when the last one was.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct RaidHeat {
    pub count: u16,
    pub last: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JointVenture {
    /// The venture's own firm; its share register holds the partners' stated stakes.
    pub firm: Handle,
    pub a: Handle,
    pub b: Handle,
    pub formed: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Demand {
    Dividend,
    Divest,
    Ceo,
    Restitution,
}

impl Demand {
    pub const fn key(self) -> &'static str {
        match self {
            Demand::Dividend => "dividend",
            Demand::Divest => "divest",
            Demand::Ceo => "ceo",
            Demand::Restitution => "restitution",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    FallingPrice,
    MissedGoals,
    Squeeze,
    DividendCut,
}

impl Reason {
    pub const fn key(self) -> &'static str {
        match self {
            Reason::FallingPrice => "falling_price",
            Reason::MissedGoals => "missed_goals",
            Reason::Squeeze => "squeeze",
            Reason::DividendCut => "dividend_cut",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Open,
    Conceded,
    Compromised,
    ActivistWon,
    ActivistLost,
}

impl Outcome {
    pub const fn key(self) -> &'static str {
        match self {
            Outcome::Open => "open",
            Outcome::Conceded => "conceded",
            Outcome::Compromised => "compromised",
            Outcome::ActivistWon => "activist_won",
            Outcome::ActivistLost => "activist_lost",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    Concede,
    Compromise,
    Fight,
}

/// An activist campaign against one firm.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Campaign {
    pub id: u32,
    pub target: Handle,
    /// The outside investor who leads it.
    pub activist: u32,
    pub stake_bps: i32,
    pub reason: Reason,
    pub demand: Demand,
    /// How well founded the complaint is, 0 to 10,000. Drives the vote and the court.
    pub justification_bps: i32,
    pub amount_cents: i64,
    pub opened: u32,
    pub deadline: u32,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub bonds_issued: u32,
    pub bond_principal_cents: i64,
    pub bond_defaults: u32,
    pub jvs_formed: u32,
    pub jv_buyouts: u32,
    pub jv_exits: u32,
    pub ipos: u32,
    pub spin_offs: u32,
    pub carve_outs: u32,
    pub activist_events: u32,
    pub activist_wins: u32,
    pub squeeze_cents: i64,
    /// Corporate tax saved (positive) or paid extra by pricing inside groups, cents.
    pub tax_shift_cents: i64,
    pub group_lots_moved: i64,
    pub raids_repeat: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StructureState {
    /// Transfer setting by supplying division (firm, region, sector type); absent is market.
    pub transfer: BTreeMap<(Handle, u8, u8), i32>,
    pub links: BTreeMap<(Handle, Handle, u16), SupplyLink>,
    /// Value moved off a firm's minority shareholders, fading over about a year, cents.
    pub squeeze: BTreeMap<Handle, i64>,
    /// Consecutive weeks a group supplier has lost money.
    pub captive_weeks: BTreeMap<Handle, u16>,
    pub reputation: BTreeMap<(Handle, u8, u8), i32>,
    /// Reputation by (firm, region, product) for the market, rebuilt weekly from `reputation`.
    pub rep_product: BTreeMap<(Handle, u8, u16), i32>,
    pub raid_heat: BTreeMap<(Handle, Handle), RaidHeat>,
    pub jvs: Vec<JointVenture>,
    pub campaigns: Vec<Campaign>,
    pub next_campaign: u32,
    pub last_activist: BTreeMap<Handle, u32>,
    pub peak_mark: BTreeMap<Handle, i64>,
    /// Consecutive weeks a firm's mark has sat 40 percent or more under its peak.
    #[serde(default)]
    pub weak_price_weeks: BTreeMap<Handle, u16>,
    /// Consecutive weeks a firm has lost money.
    #[serde(default)]
    pub loss_weeks: BTreeMap<Handle, u16>,
    /// Consecutive weeks a firm has made money.
    #[serde(default)]
    pub profit_weeks: BTreeMap<Handle, u16>,
    /// When a firm last carved a division out.
    #[serde(default)]
    pub carved: BTreeMap<Handle, u32>,
    pub dividend_quarters: BTreeMap<Handle, u16>,
    pub last_dividend: BTreeMap<Handle, u32>,
    pub bond_basis: BTreeMap<(Handle, HolderId), i64>,
    pub ceo_changes: BTreeMap<Handle, u16>,
    pub stats: Stats,
}

// ---------------------------------------------------------------------------------------------
// Temperament hooks
// ---------------------------------------------------------------------------------------------

/// Weights that steer how a rival uses the structure verbs. The character cast drives
/// `temperament`; the rules below only read this table. All in basis points around 10,000 as
/// "typical" except where noted.
#[derive(Clone, Copy, Debug)]
pub struct Temperament {
    /// Appetite for pricing inside the group to move profit between tax bills.
    pub tax_shifting: i32,
    /// How readily the firm borrows in the bond market.
    pub leverage: i32,
    /// Willingness to list or spin off parts of the group.
    pub listing: i32,
    /// Willingness to share a division with a partner.
    pub partnering: i32,
    /// Resistance to activists: 0 concedes early, 10,000 fights almost everything.
    pub resistance: i32,
}

impl Default for Temperament {
    fn default() -> Self {
        Temperament {
            tax_shifting: 5_000,
            leverage: 5_000,
            listing: 5_000,
            partnering: 5_000,
            resistance: 5_000,
        }
    }
}

/// How each cast temperament plays the structure verbs.
fn cast_profile(style: cw_data::Temperament) -> Temperament {
    use cw_data::Temperament as Style;
    let base = Temperament::default();
    match style {
        Style::Integrator => Temperament {
            partnering: 7_500,
            listing: 4_000,
            tax_shifting: 4_500,
            resistance: 6_000,
            ..base
        },
        Style::Raider => Temperament {
            tax_shifting: 7_500,
            leverage: 7_500,
            partnering: 3_500,
            resistance: 8_000,
            ..base
        },
        Style::Financier => Temperament {
            tax_shifting: 6_500,
            leverage: 8_500,
            listing: 7_500,
            partnering: 6_000,
            ..base
        },
        Style::Speculator => Temperament {
            leverage: 7_000,
            listing: 6_500,
            partnering: 6_500,
            resistance: 4_000,
            ..base
        },
        Style::Builder => Temperament {
            tax_shifting: 4_000,
            leverage: 3_500,
            listing: 3_500,
            partnering: 5_500,
            resistance: 6_500,
        },
        Style::Cautious => Temperament {
            tax_shifting: 3_500,
            leverage: 2_500,
            listing: 4_500,
            resistance: 3_000,
            ..base
        },
    }
}

/// The weight table for a firm: the rival archetype's habits, pulled three parts in four toward
/// its chief executive's temperament when the firm has a named one from the cast.
pub fn temperament(world: &World, dataset: &Dataset, firm: Handle) -> Temperament {
    let habit = archetype_temperament(world, firm);
    let Some(style) = crate::economy::cast::temperament(world, dataset, firm) else {
        return habit;
    };
    let person = cast_profile(style);
    let blend = |habit: i32, person: i32| (habit + 3 * person) / 4;
    Temperament {
        tax_shifting: blend(habit.tax_shifting, person.tax_shifting),
        leverage: blend(habit.leverage, person.leverage),
        listing: blend(habit.listing, person.listing),
        partnering: blend(habit.partnering, person.partnering),
        resistance: blend(habit.resistance, person.resistance),
    }
}

fn archetype_temperament(world: &World, firm: Handle) -> Temperament {
    use cw_data::Archetype::*;
    let base = Temperament::default();
    match world.firms.get(firm).and_then(|f| f.archetype) {
        Some(CostLeader) => Temperament {
            tax_shifting: 7_500,
            leverage: 6_500,
            ..base
        },
        Some(BrandBuilder) => Temperament {
            listing: 3_500,
            resistance: 6_500,
            ..base
        },
        Some(Consolidator) => Temperament {
            partnering: 7_000,
            listing: 6_500,
            leverage: 7_000,
            ..base
        },
        Some(Technologist) => Temperament {
            partnering: 6_500,
            leverage: 3_500,
            resistance: 6_000,
            ..base
        },
        Some(Vulture) => Temperament {
            tax_shifting: 8_000,
            leverage: 8_000,
            resistance: 3_500,
            ..base
        },
        None => base,
    }
}

// ---------------------------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------------------------

fn post(book: &mut Ledger, tick: Tick, lines: &[(Account, i64)], memo: &str) {
    let mut txn = Txn::new();
    for (account, delta) in lines {
        txn = txn.signed(*account, *delta, memo);
    }
    if txn.lines.is_empty() {
        return;
    }
    let _ = book.commit(tick, txn);
}

fn firm_missing(handle: Handle) -> DomainError {
    DomainError::InvalidEntity {
        handle: handle.0,
        kind: EntityKind::Firm,
    }
}

fn out_of_range() -> DomainError {
    DomainError::Arithmetic {
        detail: ArithmeticFault::OutOfRange,
    }
}

fn wrong_phase(expected: &'static str, actual: &'static str) -> DomainError {
    DomainError::WrongPhase { expected, actual }
}

/// Basis points of a firm's votes held by whoever does not control it. Zero when nobody does.
pub fn minority_bps(world: &World, firm: Handle) -> i32 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    match entity.shares.controller() {
        Some(holder) => (10_000 - entity.shares.vote_bps(holder)).clamp(0, 10_000),
        None => 0,
    }
}

/// Corporate tax rate a firm pays on its next profit, basis points.
pub fn tax_rate_bps(world: &World, firm: Handle) -> i32 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let policy = &world.regions[entity.region as usize].policy;
    let modifier = if policy
        .tax_modifier_until
        .map(|until| until.0 > world.tick.0)
        .unwrap_or(false)
    {
        policy.tax_modifier_bps
    } else {
        0
    };
    (world.politics.laws.corporate_tax_bps + modifier).clamp(0, 10_000)
}

fn weekly_outflow_buffer(world: &World, firm: Handle) -> i64 {
    crate::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4
}

fn trailing_net_income(world: &World, firm: Handle) -> i64 {
    world
        .firms
        .get(firm)
        .map(|f| f.book.recent_weeks().iter().map(|w| w.net_income).sum())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Transfer pricing and group supply
// ---------------------------------------------------------------------------------------------

/// The transfer setting of a supplying division: 0 at cost, 10,000 at market (default), up to
/// 12,000 for market plus a fifth.
pub fn transfer_bps(world: &World, seller: Handle, region: u8, industry: u8) -> i32 {
    world
        .structure
        .transfer
        .get(&(seller, region, industry))
        .copied()
        .unwrap_or(TRANSFER_MARKET_BPS)
}

/// Set a supplying division's transfer setting.
pub fn set_transfer(
    world: &mut World,
    dataset: &Dataset,
    seller: Handle,
    region: u8,
    industry: u8,
    bps: i32,
) -> DomainResult<()> {
    if !(0..=TRANSFER_MAX_BPS).contains(&bps) {
        return Err(out_of_range());
    }
    if sectors::division_plants(world, dataset, seller, region, industry).is_empty() {
        return Err(wrong_phase("own_division", "no_division"));
    }
    if bps == TRANSFER_MARKET_BPS {
        world.structure.transfer.remove(&(seller, region, industry));
    } else {
        world
            .structure
            .transfer
            .insert((seller, region, industry), bps);
    }
    Ok(())
}

/// Cost, market and transfer price of a lot of `product` from `seller`, cents.
pub fn transfer_quote(
    world: &World,
    dataset: &Dataset,
    seller: Handle,
    product: u16,
    region: u8,
    industry: u8,
) -> (i64, i64, i64) {
    let market = pricing::market_price(world, dataset, seller, product).max(1);
    let cost = pricing::unit_cost(world, dataset, seller, product)
        .unwrap_or(market)
        .max(1);
    let bps = transfer_bps(world, seller, region, industry) as i64;
    let price = if bps <= TRANSFER_MARKET_BPS as i64 {
        cost + (market - cost) * bps / TRANSFER_MARKET_BPS as i64
    } else {
        market * bps / TRANSFER_MARKET_BPS as i64
    };
    (cost, market, price.max(1))
}

/// Group members by ultimate controller, groups of two or more only, in stable order.
fn groups(world: &World) -> Vec<Vec<Handle>> {
    let mut by_root: BTreeMap<HolderId, Vec<Handle>> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.operative() {
            continue;
        }
        if let Some(root) = ownership::ultimate_controller(world, handle) {
            by_root.entry(root).or_default().push(handle);
        }
    }
    by_root
        .into_values()
        .filter(|members| members.len() >= 2)
        .map(|mut members| {
            members.sort_unstable();
            members
        })
        .collect()
}

/// Weekly lots a firm's operating plants burn of each product in each region.
fn weekly_needs(world: &World, dataset: &Dataset, firm: Handle) -> BTreeMap<(u16, u8), i64> {
    let mut needs: BTreeMap<(u16, u8), i64> = BTreeMap::new();
    let Some(entity) = world.firms.get(firm) else {
        return needs;
    };
    for plant in entity
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .filter(|p| p.status == FacilityStatus::Operating)
    {
        let recipe = &dataset.recipes[plant.recipe as usize];
        for (input, lots) in &recipe.weekly_inputs_lots {
            let Some(code) = dataset.product_index.get(input) else {
                continue;
            };
            let run = lots * plant.blocks as i64 * plant.target_bps as i64 / 10_000;
            *needs.entry((*code, plant.region)).or_default() += run;
        }
    }
    needs
}

/// Weekly lots a firm's operating plants make of each product, with the region and sector
/// type of the biggest producing plant.
fn weekly_supply(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
) -> BTreeMap<u16, (i64, u8, u8, i64)> {
    // product -> (total lots, region, industry, lots at that plant)
    let mut supply: BTreeMap<u16, (i64, u8, u8, i64)> = BTreeMap::new();
    let Some(entity) = world.firms.get(firm) else {
        return supply;
    };
    for plant in entity
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .filter(|p| p.status == FacilityStatus::Operating)
    {
        let recipe = &dataset.recipes[plant.recipe as usize];
        if recipe.storage_class.is_some() || recipe.transport_mode.is_some() {
            continue;
        }
        let industry = sectors::recipe_industry(dataset, plant.recipe);
        for (output, lots) in recipe.outputs() {
            let Some(code) = dataset.product_index.get(output) else {
                continue;
            };
            let made = lots * plant.blocks as i64 * plant.target_bps as i64 / 10_000;
            let entry = supply
                .entry(*code)
                .or_insert((0, plant.region, industry, 0));
            entry.0 += made;
            if made > entry.3 {
                entry.1 = plant.region;
                entry.2 = industry;
                entry.3 = made;
            }
        }
    }
    supply
}

/// (buyer, seller, product).
type SupplyKey = (Handle, Handle, u16);
/// Lots, from region, to region, sector type, spare.
type SupplyWant = (i64, u8, u8, u8, u8);
/// Product to (total lots, region, sector type, lots at that plant).
type SupplyMap = BTreeMap<u16, (i64, u8, u8, i64)>;

/// Keep one standing contract for every (buyer, seller, product) a group needs, priced at the
/// seller's transfer price, ahead of the week's deliveries. Group members therefore get first
/// call on each other's stock: contracts settle before any open-market sale.
pub fn sync_supply(world: &mut World, dataset: &Dataset) {
    let mut wanted: BTreeMap<SupplyKey, SupplyWant> = BTreeMap::new();
    for members in groups(world) {
        let supplies: BTreeMap<Handle, SupplyMap> = members
            .iter()
            .map(|m| (*m, weekly_supply(world, dataset, *m)))
            .collect();
        let mut remaining: BTreeMap<(Handle, u16), i64> = BTreeMap::new();
        for (seller, products) in &supplies {
            for (product, (total, ..)) in products {
                remaining.insert((*seller, *product), *total);
            }
        }
        for buyer in &members {
            for ((product, to_region), need) in weekly_needs(world, dataset, *buyer) {
                let mut need = need;
                for seller in &members {
                    if need <= 0 {
                        break;
                    }
                    if seller == buyer {
                        continue;
                    }
                    let Some((_, from_region, industry, _)) =
                        supplies.get(seller).and_then(|s| s.get(&product))
                    else {
                        continue;
                    };
                    let left = remaining.get(&(*seller, product)).copied().unwrap_or(0);
                    let lots = left.min(need);
                    if lots <= 0 {
                        continue;
                    }
                    remaining.insert((*seller, product), left - lots);
                    need -= lots;
                    wanted.insert(
                        (*buyer, *seller, product),
                        (lots, *from_region, to_region, *industry, 0),
                    );
                }
            }
        }
    }
    // Retire links nobody needs now.
    let stale: Vec<(Handle, Handle, u16)> = world
        .structure
        .links
        .keys()
        .filter(|key| !wanted.contains_key(key))
        .copied()
        .collect();
    for key in stale {
        if let Some(link) = world.structure.links.remove(&key) {
            retire_contract(world, link.contract, key.0, key.1);
        }
    }
    for ((buyer, seller, product), (lots, from_region, to_region, industry, _)) in wanted {
        let (cost, market, price) =
            transfer_quote(world, dataset, seller, product, from_region, industry);
        let quantity = MilliLots::from_lots(lots.max(1));
        let existing = world
            .structure
            .links
            .get(&(buyer, seller, product))
            .map(|l| l.contract)
            .filter(|c| {
                world
                    .contracts
                    .get(*c)
                    .is_some_and(|k| k.status == ContractStatus::Active)
            });
        let contract = match existing {
            Some(handle) => {
                if let Some(k) = world.contracts.get_mut(handle) {
                    k.price_cents = price;
                    k.quantity_weekly = quantity;
                    k.from_region = from_region;
                    k.to_region = to_region;
                    k.remaining_weeks = k.remaining_weeks.max(2);
                    k.penalty_bps = 0;
                    k.auto_renew = true;
                }
                handle
            }
            None => {
                let handle = world.contracts.insert(crate::state::Contract {
                    buyer,
                    seller,
                    product,
                    from_region,
                    to_region,
                    quantity_weekly: quantity,
                    price_cents: price,
                    penalty_bps: 0,
                    escrow_cents: 0,
                    remaining_weeks: 52,
                    next_delivery: Tick(world.tick.0),
                    status: ContractStatus::Active,
                    shortfalls: MilliLots::ZERO,
                    auto_renew: true,
                });
                if let Some(f) = world.firms.get_mut(buyer) {
                    f.contracts_buy.push(handle);
                }
                if let Some(f) = world.firms.get_mut(seller) {
                    f.contracts_sell.push(handle);
                }
                handle
            }
        };
        world.structure.links.insert(
            (buyer, seller, product),
            SupplyLink {
                contract,
                region: from_region,
                industry,
                market_cents: market,
                price_cents: price,
                cost_cents: cost,
            },
        );
    }
}

fn retire_contract(world: &mut World, contract: Handle, buyer: Handle, seller: Handle) {
    world.contracts.remove(contract);
    if let Some(f) = world.firms.get_mut(buyer) {
        f.contracts_buy.retain(|c| *c != contract);
    }
    if let Some(f) = world.firms.get_mut(seller) {
        f.contracts_sell.retain(|c| *c != contract);
    }
}

/// A group delivery happened: measure what the transfer price moved between the two firms.
pub fn note_delivery(world: &mut World, contract: Handle, qty: MilliLots, price: i64) {
    let Some(((buyer, seller, _), link)) = world
        .structure
        .links
        .iter()
        .find(|(_, link)| link.contract == contract)
        .map(|(k, v)| (*k, v.clone()))
    else {
        return;
    };
    world.structure.stats.group_lots_moved += qty.lots();
    let diff = price - link.market_cents;
    let value = (diff as i128 * qty.0 as i128 / 1_000) as i64;
    if value == 0 {
        return;
    }
    // Profit moved from the buyer to the seller (value > 0) or the other way round. The loser's
    // minority shareholders bear their share of it.
    let loser = if value < 0 { seller } else { buyer };
    let minority = minority_bps(world, loser) as i64;
    let squeezed = value.abs() * minority / 10_000;
    if squeezed > 0 {
        *world.structure.squeeze.entry(loser).or_default() += squeezed;
        world.structure.stats.squeeze_cents += squeezed;
    }
    // Tax: profit that lands in the lower-taxed firm saves the group the difference.
    let rate_buyer = tax_rate_bps(world, buyer) as i64;
    let rate_seller = tax_rate_bps(world, seller) as i64;
    world.structure.stats.tax_shift_cents += value * (rate_buyer - rate_seller) / 10_000;
}

/// How far the market marks a firm's equity down, basis points: a supplier inside a group that
/// has lost money for months is worth less, rising 0.5 points a week to a quarter.
pub fn markdown_bps(world: &World, firm: Handle) -> i32 {
    let weeks = world
        .structure
        .captive_weeks
        .get(&firm)
        .copied()
        .unwrap_or(0) as i32;
    (weeks.saturating_sub(13) * 50).clamp(0, 2_500)
}

fn update_captive(world: &mut World) {
    let suppliers: std::collections::BTreeSet<Handle> =
        world.structure.links.keys().map(|(_, s, _)| *s).collect();
    let all: Vec<Handle> = world.structure.captive_weeks.keys().copied().collect();
    for supplier in suppliers.iter().copied().chain(all) {
        let losing = trailing_net_income(world, supplier) < 0
            && world
                .firms
                .get(supplier)
                .is_some_and(|f| f.book.recent_weeks().len() >= 4);
        let is_supplier = suppliers.contains(&supplier);
        let entry = world.structure.captive_weeks.entry(supplier).or_default();
        if is_supplier && losing {
            *entry = entry.saturating_add(1);
        } else {
            *entry = entry.saturating_sub(2);
        }
    }
    world.structure.captive_weeks.retain(|_, w| *w > 0);
}

// ---------------------------------------------------------------------------------------------
// Reputation
// ---------------------------------------------------------------------------------------------

/// A division's reputation, 0 to 10,000.
pub fn reputation(world: &World, firm: Handle, region: u8, industry: u8) -> i32 {
    world
        .structure
        .reputation
        .get(&(firm, region, industry))
        .copied()
        .unwrap_or(NEUTRAL_REP)
}

/// Reputation of a firm's offer of `product` from `region`, for the market.
pub fn product_reputation(world: &World, firm: Handle, region: u8, product: u16) -> i32 {
    world
        .structure
        .rep_product
        .get(&(firm, region, product))
        .copied()
        .unwrap_or(NEUTRAL_REP)
}

/// The premium reputation earns a seller in the market, basis points on perceived quality:
/// up to fifteen percent at the top, down to minus five at the bottom. Customers pay it as
/// price tolerance and loyalty: a reputable seller keeps its share at a higher price.
pub fn loyalty_premium_bps(world: &World, firm: Handle, region: u8, product: u16) -> i64 {
    let rep = product_reputation(world, firm, region, product) as i64;
    if rep >= NEUTRAL_REP as i64 {
        (rep - NEUTRAL_REP as i64) * 1_500 / (10_000 - NEUTRAL_REP as i64)
    } else {
        -(NEUTRAL_REP as i64 - rep) * 500 / NEUTRAL_REP as i64
    }
}

fn update_reputation(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, f)| f.operative())
        .map(|(h, _)| h)
        .collect();
    let mut live: std::collections::BTreeSet<(Handle, u8, u8)> = Default::default();
    let mut products: BTreeMap<(Handle, u8, u16), i32> = BTreeMap::new();
    for handle in handles {
        let divisions = sectors::divisions_of(world, dataset, handle);
        if divisions.is_empty() {
            continue;
        }
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        let weeks = firm.book.recent_weeks();
        let revenue = weeks.iter().map(|w| w.revenue).sum::<i64>() / (weeks.len().max(1) as i64);
        let marketing = firm.policies.marketing_budget_cents.max(0);
        // Marketing as a share of revenue, spread over the firm's divisions.
        let marketing_bps = if revenue > 0 {
            marketing * 10_000 / revenue / divisions.len() as i64
        } else {
            0
        };
        let marketing_term = (marketing_bps * 5).clamp(0, 3_000);
        for division in divisions {
            let mut quality_term = 0i64;
            let mut brand_term = 0i64;
            let mut seen = 0i64;
            let mut outputs: Vec<u16> = Vec::new();
            for plant in division
                .plants
                .iter()
                .filter_map(|h| world.facilities.get(*h))
            {
                let recipe = &dataset.recipes[plant.recipe as usize];
                for (output, _) in recipe.outputs() {
                    if let Some(code) = dataset.product_index.get(output) {
                        outputs.push(*code);
                    }
                }
            }
            outputs.sort_unstable();
            outputs.dedup();
            for code in &outputs {
                let q = firm
                    .quality_bps
                    .get(code)
                    .copied()
                    .unwrap_or_else(|| crate::economy::research::quality(firm.tech_level, 0))
                    as i64;
                quality_term += ((q - 10_000) * 2).clamp(-2_000, 6_000);
                let b = firm
                    .brand_bps
                    .get(&(division.region, *code))
                    .copied()
                    .unwrap_or(10_000) as i64;
                brand_term += (b - 10_000).clamp(0, 3_000);
                seen += 1;
            }
            let seen = seen.max(1);
            let target =
                (NEUTRAL_REP as i64 + quality_term / seen + marketing_term + brand_term / seen)
                    .clamp(0, 10_000) as i32;
            let key = (handle, division.region, division.industry);
            let current = world
                .structure
                .reputation
                .get(&key)
                .copied()
                .unwrap_or(NEUTRAL_REP);
            // Earned slowly, lost more slowly still.
            let next = if target > current {
                current + ((target - current) / 26).max(1).min(target - current)
            } else {
                current - ((current - target) / 52).max(if current > target { 1 } else { 0 })
            };
            world
                .structure
                .reputation
                .insert(key, next.clamp(0, 10_000));
            live.insert(key);
            for code in outputs {
                products.insert((handle, division.region, code), next.clamp(0, 10_000));
            }
        }
    }
    world
        .structure
        .reputation
        .retain(|key, _| live.contains(key));
    world.structure.rep_product = products;
}

// ---------------------------------------------------------------------------------------------
// Raids
// ---------------------------------------------------------------------------------------------

/// Repeat raids on one rival within the memory window: one fewer for every quarter since the last.
fn heat_now(world: &World, attacker: Handle, defender: Handle) -> i64 {
    let heat = world
        .structure
        .raid_heat
        .get(&(attacker, defender))
        .copied()
        .unwrap_or_default();
    (heat.count as i64 - (world.tick.0.saturating_sub(heat.last) / 13) as i64).max(0)
}

/// What raiding the same rival again soon costs on top of the campaign budget: legal fees and
/// retaliation, forty percent of the budget for every raid still on the record.
pub fn raid_surcharge(world: &World, attacker: Handle, defender: Handle, budget: i64) -> i64 {
    budget.max(0) * 4_000 * heat_now(world, attacker, defender) / 10_000
}

/// Pay a raid's repeat-offender surcharge to the outside legal sector.
pub fn charge_raid_surcharge(world: &mut World, attacker: Handle, amount: i64) {
    if amount <= 0 {
        return;
    }
    if let Some(entity) = world.firms.get_mut(attacker) {
        post(
            &mut entity.book,
            world.tick,
            &[
                (Account::Cash, -amount),
                (Account::OperatingExpense, amount),
            ],
            "raid legal costs",
        );
    }
    post(
        &mut world.outside.legal_services_book,
        world.tick,
        &[(Account::Cash, amount), (Account::Revenue, -amount)],
        "raid legal costs",
    );
}

/// Record a raid: heat for the raider, reputation for both sides.
pub fn note_raid(world: &mut World, record: &crate::state::RaidRecord, stake_bps: i32) {
    crate::economy::rivalry::on_raid(world, record);
    let heat_before = heat_now(world, record.attacker, record.defender);
    let tick = world.tick.0;
    let entry = world
        .structure
        .raid_heat
        .entry((record.attacker, record.defender))
        .or_default();
    entry.count = (heat_before as u16).saturating_add(1);
    entry.last = tick;
    if heat_before > 0 {
        world.structure.stats.raids_repeat += 1;
    }
    // The defender's customers hear about it; a taken division loses more.
    let hit = if record.success {
        300 + stake_bps / 5
    } else {
        100
    };
    if let Some(rep) =
        world
            .structure
            .reputation
            .get_mut(&(record.defender, record.region, record.industry))
    {
        *rep = (*rep - hit).max(0);
    }
    // A raider that keeps going after the same rival is seen for what it is.
    if heat_before > 0
        && let Some(rep) =
            world
                .structure
                .reputation
                .get_mut(&(record.attacker, record.region, record.industry))
    {
        *rep = (*rep - 120 * heat_before as i32).max(0);
    }
}

// ---------------------------------------------------------------------------------------------
// Bonds
// ---------------------------------------------------------------------------------------------

/// A bond's price at issue: base yield, spread and the figures behind it.
#[derive(Clone, Copy, Debug, Default)]
pub struct BondQuote {
    pub base_bps: i32,
    pub spread_bps: i32,
    pub coupon_bps: i32,
    pub leverage_bps: i32,
    /// Trailing annual EBITDA over annual interest with this bond, basis points (10,000 is 1x).
    pub coverage_bps: i32,
    /// Letter for display: A under 150 points of spread, B under 300, C under 600, else D.
    pub rating: char,
    /// Largest issue the market would take at this firm's credit, cents.
    pub max_principal_cents: i64,
}

/// Yield a bond of `weeks` to run is priced off: the central bank's ten-year yield for ten
/// years and more, blending toward the policy rate for shorter terms.
pub fn bond_base_bps(world: &World, weeks: u32) -> i32 {
    let long = crate::economy::macro_cycle::ten_year_yield_bps(world);
    let short = world.macro_state.policy_rate_bps;
    let w = weeks.min(520) as i64;
    (short as i64 + (long - short) as i64 * w / 520) as i32
}

/// Credit spread over the base yield for borrowing `extra` more, basis points. Rises smoothly
/// with leverage, thin coverage and size; there is no grade gate, only a price.
pub fn credit_spread(world: &World, firm: Handle, extra: i64) -> (i32, i32, i32) {
    let Some(entity) = world.firms.get(firm) else {
        return (1_500, 20_000, 0);
    };
    let ledger = &entity.book;
    let tangible = ledger.net_tangible_assets().max(1);
    let debt_after = ledger.debt() + extra;
    let leverage = (debt_after as i128 * 10_000 / tangible as i128).clamp(0, 20_000) as i64;
    let ebitda: i64 = ledger
        .recent_weeks()
        .iter()
        .map(|w| w.ebitda())
        .sum::<i64>()
        * 52
        / (ledger.recent_weeks().len().clamp(1, 13) as i64);
    let interest_now: i64 = entity
        .loans
        .iter()
        .filter_map(|h| world.loans.get(*h))
        .map(|l| l.principal_remaining_cents * l.rate_bps as i64 / 10_000)
        .sum::<i64>()
        + entity
            .bonds
            .iter()
            .filter_map(|h| world.bonds.get(*h))
            .map(|b| b.principal_cents * b.coupon_bps as i64 / 10_000)
            .sum::<i64>();
    let interest_after = interest_now + extra * 600 / 10_000;
    // A firm with under a month of results has no coverage to judge yet; it is priced on its
    // assets and size alone, as if it covered its interest twice.
    let coverage = if ledger.recent_weeks().len() < 4 {
        20_000
    } else if interest_after <= 0 {
        200_000
    } else if ebitda <= 0 {
        0
    } else {
        (ebitda as i128 * 10_000 / interest_after as i128).min(200_000) as i64
    };
    let lev_term = leverage * 250 / 10_000 + leverage * leverage * 600 / 100_000_000;
    let cov_term = if coverage <= 0 {
        900
    } else {
        (3_000_000 / coverage.max(3_000)).min(900)
    };
    let equity = ledger.total_equity().max(1);
    let size_term = (extra * 150 / equity).clamp(0, 300);
    let spread = (75 + lev_term + cov_term + size_term).clamp(75, 2_500);
    (spread as i32, leverage as i32, coverage as i32)
}

pub fn bond_quote(world: &World, firm: Handle, principal: i64, weeks: u32) -> BondQuote {
    let (spread, leverage, coverage) = credit_spread(world, firm, principal);
    let base = bond_base_bps(world, weeks);
    let rating = match spread {
        s if s < 150 => 'A',
        s if s < 300 => 'B',
        s if s < 600 => 'C',
        _ => 'D',
    };
    let equity = world
        .firms
        .get(firm)
        .map(|f| f.book.total_equity())
        .unwrap_or(0)
        .max(0);
    BondQuote {
        base_bps: base,
        spread_bps: spread,
        coupon_bps: (base + spread).max(100),
        leverage_bps: leverage,
        coverage_bps: coverage,
        rating,
        max_principal_cents: equity,
    }
}

pub const MIN_BOND_CENTS: i64 = 25_000_000;
pub const MIN_BOND_WEEKS: u32 = 26;
pub const MAX_BOND_WEEKS: u32 = 780;
/// The market will not lend past this spread at all, basis points.
pub const MAX_BOND_SPREAD_BPS: i32 = 1_200;

/// Issue a bond to the outside investors at the quoted coupon. `max_coupon_bps` is the most the
/// issuer will pay (zero accepts the market price).
pub fn issue_bond(
    world: &mut World,
    firm: Handle,
    principal: i64,
    weeks: u32,
    max_coupon_bps: i32,
) -> DomainResult<Handle> {
    let entity = world.firms.get(firm).ok_or_else(|| firm_missing(firm))?;
    if principal < MIN_BOND_CENTS || !(MIN_BOND_WEEKS..=MAX_BOND_WEEKS).contains(&weeks) {
        return Err(out_of_range());
    }
    if entity.payout_freeze || !matches!(entity.status, FirmStatus::Operating) {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: SolvencyReason::PastDue,
        });
    }
    let quote = bond_quote(world, firm, principal, weeks);
    if quote.spread_bps > MAX_BOND_SPREAD_BPS || quote.leverage_bps > 15_000 {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: SolvencyReason::CreditGradeTooLow,
        });
    }
    if max_coupon_bps > 0 && quote.coupon_bps > max_coupon_bps {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: SolvencyReason::CoverageTooLow,
        });
    }
    // Investors fill the book in order; the issue needs their combined capital.
    let capital: i64 = world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum();
    if capital < principal {
        return Err(DomainError::InsufficientCash {
            entity: 0,
            required: principal,
            available: capital,
        });
    }
    let mut holders = BTreeMap::new();
    let mut left = principal;
    for (index, holder) in world.outside.investors.iter_mut().enumerate() {
        if left <= 0 {
            break;
        }
        let take = holder.capital_cents.min(left);
        if take > 0 {
            holder.capital_cents -= take;
            holders.insert(HolderId::Outside(index as u32), take);
            left -= take;
        }
    }
    let fee = principal * 200 / 10_000;
    if let Some(entity) = world.firms.get_mut(firm) {
        post(
            &mut entity.book,
            world.tick,
            &[
                (Account::Cash, principal - fee),
                (Account::OperatingExpense, fee),
                (Account::Bonds, -principal),
            ],
            "bond issue",
        );
    }
    post(
        &mut world.outside.legal_services_book,
        world.tick,
        &[(Account::Cash, fee), (Account::Revenue, -fee)],
        "bond fee",
    );
    let handle = world.bonds.insert(Bond {
        issuer: firm,
        principal_cents: principal,
        coupon_bps: quote.coupon_bps,
        maturity: Tick(world.tick.0 + weeks),
        issued: world.tick,
        holders,
        carry: 0,
        next_coupon: Tick(world.tick.0 + 13),
    });
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.bonds.push(handle);
    }
    world.structure.stats.bonds_issued += 1;
    world.structure.stats.bond_principal_cents += principal;
    Ok(handle)
}

/// The bond's price per 10,000 of par, clean, bonds trade at: par moved by the gap between the
/// issuer's yield now and the coupon, times the years left.
pub fn bond_mark_bps(world: &World, bond: Handle) -> i64 {
    let Some(b) = world.bonds.get(bond) else {
        return 10_000;
    };
    let left = b.maturity.0.saturating_sub(world.tick.0);
    let (spread, ..) = credit_spread(world, b.issuer, 0);
    let now = bond_base_bps(world, left) as i64 + spread as i64;
    let years = (left as i64 * 100 / 52).max(0);
    let mark = 10_000 - (now - b.coupon_bps as i64) * years / 100;
    let distressed = world
        .firms
        .get(b.issuer)
        .is_some_and(|f| !matches!(f.status, FirmStatus::Operating));
    let mark = if distressed { mark.min(6_000) } else { mark };
    mark.clamp(3_000, 11_500)
}

fn accrued(bond: &Bond, tick: u32, qty: i64) -> i64 {
    let weeks = tick.saturating_sub(bond.issued.0) % 13;
    cw_core::money::round_even_div(
        qty as i128 * bond.coupon_bps as i128 * weeks as i128,
        10_000 * 52,
    ) as i64
}

/// The investor buys (`buy`) or sells bonds at the mark plus accrued coupon, against the outside
/// investors.
pub fn trade_bond(world: &mut World, bond: Handle, qty: i64, buy: bool) -> DomainResult<()> {
    let b = world
        .bonds
        .get(bond)
        .cloned()
        .ok_or(DomainError::InvalidEntity {
            handle: bond.0,
            kind: EntityKind::Bond,
        })?;
    if qty <= 0 {
        return Err(out_of_range());
    }
    let mark = bond_mark_bps(world, bond);
    let clean = qty * mark / 10_000;
    let dirty = clean + accrued(&b, world.tick.0, qty);
    if buy {
        let seller = (0..world.outside.investors.len())
            .find(|i| {
                b.holders
                    .get(&HolderId::Outside(*i as u32))
                    .copied()
                    .unwrap_or(0)
                    >= qty
            })
            .ok_or(DomainError::InsufficientStock {
                product: 0,
                required: qty,
                available: b
                    .holders
                    .iter()
                    .filter(|(h, _)| h.is_external())
                    .map(|(_, q)| *q)
                    .max()
                    .unwrap_or(0),
            })?;
        if world.investor.cash_cents < dirty {
            return Err(DomainError::InsufficientCash {
                entity: 0,
                required: dirty,
                available: world.investor.cash_cents,
            });
        }
        world.investor.cash_cents -= dirty;
        post(
            &mut world.investor.book,
            world.tick,
            &[(Account::Cash, -dirty), (Account::Investments, dirty)],
            "bond purchase",
        );
        if let Some(h) = world.outside.investors.get_mut(seller) {
            h.capital_cents += dirty;
        }
        if let Some(stored) = world.bonds.get_mut(bond) {
            if let Some(q) = stored.holders.get_mut(&HolderId::Outside(seller as u32)) {
                *q -= qty;
            }
            *stored.holders.entry(HolderId::Investor).or_default() += qty;
        }
        *world
            .structure
            .bond_basis
            .entry((bond, HolderId::Investor))
            .or_default() += dirty;
        Ok(())
    } else {
        let held = b.holders.get(&HolderId::Investor).copied().unwrap_or(0);
        if held < qty {
            return Err(DomainError::InsufficientStock {
                product: 0,
                required: qty,
                available: held,
            });
        }
        let buyer = (0..world.outside.investors.len())
            .find(|i| world.outside.investors[*i].capital_cents >= dirty)
            .ok_or(DomainError::InsufficientCash {
                entity: 0,
                required: dirty,
                available: 0,
            })?;
        let basis_all = world
            .structure
            .bond_basis
            .get(&(bond, HolderId::Investor))
            .copied()
            .unwrap_or(held);
        let released = (basis_all as i128 * qty as i128 / held as i128) as i64;
        world.investor.cash_cents += dirty;
        let gain = dirty - released;
        post(
            &mut world.investor.book,
            world.tick,
            &[
                (Account::Cash, dirty),
                (Account::Investments, -released),
                (Account::OtherIncome, -gain),
            ],
            "bond sale",
        );
        if let Some(h) = world.outside.investors.get_mut(buyer) {
            h.capital_cents -= dirty;
        }
        if let Some(stored) = world.bonds.get_mut(bond) {
            if let Some(q) = stored.holders.get_mut(&HolderId::Investor) {
                *q -= qty;
            }
            *stored
                .holders
                .entry(HolderId::Outside(buyer as u32))
                .or_default() += qty;
            stored.holders.retain(|_, q| *q > 0);
        }
        let key = (bond, HolderId::Investor);
        if held == qty {
            world.structure.bond_basis.remove(&key);
        } else if let Some(b) = world.structure.bond_basis.get_mut(&key) {
            *b -= released;
        }
        Ok(())
    }
}

/// One bond on the board.
#[derive(Clone, Debug)]
pub struct BondLine {
    pub bond: Handle,
    pub issuer: Handle,
    pub principal_cents: i64,
    pub coupon_bps: i32,
    pub weeks_left: u32,
    pub mark_bps: i64,
    /// Yield to a buyer at the mark, roughly: coupon over price.
    pub yield_bps: i64,
    pub spread_now_bps: i32,
    pub outside_qty: i64,
    pub investor_qty: i64,
}

pub fn bond_board(world: &World) -> Vec<BondLine> {
    let mut lines: Vec<BondLine> = world
        .bonds
        .iter()
        .map(|(handle, b)| {
            let mark = bond_mark_bps(world, handle);
            let left = b.maturity.0.saturating_sub(world.tick.0);
            BondLine {
                bond: handle,
                issuer: b.issuer,
                principal_cents: b.principal_cents,
                coupon_bps: b.coupon_bps,
                weeks_left: left,
                mark_bps: mark,
                yield_bps: b.coupon_bps as i64 * 10_000 / mark.max(1),
                spread_now_bps: credit_spread(world, b.issuer, 0).0,
                outside_qty: b
                    .holders
                    .iter()
                    .filter(|(h, _)| h.is_external())
                    .map(|(_, q)| *q)
                    .sum(),
                investor_qty: b.holders.get(&HolderId::Investor).copied().unwrap_or(0),
            }
        })
        .collect();
    lines.sort_by_key(|l| l.bond);
    lines
}

/// A bond ended unpaid.
pub fn note_bond_default(world: &mut World) {
    world.structure.stats.bond_defaults += 1;
}

/// Book of how much of a returned principal payment was cost: the basis a holder had.
pub fn release_basis(
    world: &mut World,
    bond: Handle,
    holder: HolderId,
    share: i64,
    held: i64,
) -> i64 {
    let key = (bond, holder);
    let basis = world
        .structure
        .bond_basis
        .get(&key)
        .copied()
        .unwrap_or(held);
    let released = if held > 0 {
        (basis as i128 * share.min(held) as i128 / held as i128) as i64
    } else {
        0
    };
    if let Some(b) = world.structure.bond_basis.get_mut(&key) {
        *b = (*b - released).max(0);
    }
    released
}

// ---------------------------------------------------------------------------------------------
// Carve-outs, listings, spin-offs
// ---------------------------------------------------------------------------------------------

/// Move a division into a new wholly owned subsidiary, with working capital from its parent.
pub fn carve_out(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
    name: String,
) -> DomainResult<Handle> {
    let divisions = sectors::divisions_of(world, dataset, firm);
    if divisions.len() < 2 {
        return Err(wrong_phase("two_divisions", "one_division"));
    }
    let plants = sectors::division_plants(world, dataset, firm, region, industry);
    if plants.is_empty() {
        return Err(wrong_phase("own_division", "no_division"));
    }
    if plants.iter().any(|h| {
        world.facilities.get(*h).is_none_or(|p| {
            !matches!(
                p.status,
                FacilityStatus::Operating | FacilityStatus::Mothballed
            )
        })
    }) {
        return Err(wrong_phase("built_division", "under_construction"));
    }
    let (capex, dep) = plants
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .fold((0i64, 0i64), |(c, d), p| {
            (c + p.capex_cents, d + p.accumulated_depreciation)
        });
    let book = capex - dep;
    if book <= 0 {
        return Err(out_of_range());
    }
    // Working capital: a twentieth of book, and half a year of upkeep, rent and payroll.
    let upkeep: i64 = plants
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .map(|p| {
            let r = &dataset.recipes[p.recipe as usize];
            (r.weekly_maintenance(dataset.maintenance_annual_bps)
                + dataset.fixture_land_rent_cents_weekly * 2
                + r.baseline_weekly_payroll(dataset.baseline_weekly_wage_cents))
                * p.blocks as i64
                * 26
        })
        .sum();
    let working = (book / 20).max(10_000_000) + upkeep;
    let parent = world.firms.get(firm).ok_or_else(|| firm_missing(firm))?;
    let buffer = weekly_outflow_buffer(world, firm);
    if parent.book.cash() < working + buffer {
        return Err(DomainError::InsufficientCash {
            entity: firm.0,
            required: working + buffer,
            available: parent.book.cash(),
        });
    }
    let mut child = crate::commands::blank_firm(world, dataset, name, industry, region);
    child.policies.wage_offer = parent.policies.wage_offer;
    child.policies.prices = parent.policies.prices.clone();
    child.tech_level = parent.tech_level;
    child.quality_bps = parent.quality_bps.clone();
    child.brand_bps = parent.brand_bps.clone();
    child.board.ceo = (ownership::ultimate_controller(world, firm) == Some(HolderId::Investor))
        .then_some(CeoRef::Investor);
    let contributed = book + working;
    child.shares.issued = INITIAL_SHARES;
    child.shares.holders.insert(
        HolderId::Firm(firm),
        ShareLot {
            qty: INITIAL_SHARES,
            cost_cents: contributed,
        },
    );
    let tick = world.tick;
    post(
        &mut child.book,
        tick,
        &[
            (Account::Cash, working),
            (Account::FixedAssets, capex),
            (Account::AccumulatedDepreciation, -dep),
            (Account::ShareCapital, -contributed),
        ],
        "carve-out",
    );
    child.board.seats = crate::economy::governance::apportion_seats(&child.shares);
    let child_handle = world.firms.insert(child);
    if let Some(parent) = world.firms.get_mut(firm) {
        post(
            &mut parent.book,
            tick,
            &[
                (Account::Investments, contributed),
                (Account::AccumulatedDepreciation, dep),
                (Account::FixedAssets, -capex),
                (Account::Cash, -working),
            ],
            "carve-out",
        );
        parent.facilities.retain(|h| !plants.contains(h));
    }
    for plant in &plants {
        if let Some(p) = world.facilities.get_mut(*plant) {
            p.firm = child_handle;
        }
    }
    if let Some(c) = world.firms.get_mut(child_handle) {
        c.facilities = plants.clone();
    }
    move_division_contracts(world, dataset, firm, child_handle, region, &plants);
    // The division's reputation goes with it.
    if let Some(rep) = world.structure.reputation.remove(&(firm, region, industry)) {
        world
            .structure
            .reputation
            .insert((child_handle, region, industry), rep);
    }
    ownership::invalidate(world, firm);
    world.structure.carved.insert(firm, world.tick.0);
    world.structure.stats.carve_outs += 1;
    Ok(child_handle)
}

/// A carved-out division takes its trade with it: the parent's supply contracts shipped from the
/// division's state for a good its plants make, and its purchase contracts delivered to that state
/// for an input they use that no plant the parent keeps there uses, pass to the new company. Left
/// behind, the parent defaulted every week on sales it could no longer make and paid for inputs
/// it could no longer use. A purchase contract into that state for an input that neither the new
/// company's plants nor any plant the parent keeps there uses ends, with no penalty on either
/// side: the parent has nothing there to feed. Contracts holding escrow stay with the parent,
/// which holds the money.
fn move_division_contracts(
    world: &mut World,
    dataset: &Dataset,
    parent: Handle,
    child: Handle,
    region: u8,
    plants: &[Handle],
) {
    let uses = |world: &World, list: &[Handle]| {
        let mut outputs = std::collections::BTreeSet::new();
        let mut inputs = std::collections::BTreeSet::new();
        for plant in list.iter().filter_map(|h| world.facilities.get(*h)) {
            if plant.region != region {
                continue;
            }
            let recipe = &dataset.recipes[plant.recipe as usize];
            for (id, _) in recipe.outputs() {
                outputs.extend(dataset.product_index.get(id).copied());
            }
            for id in recipe.weekly_inputs_lots.keys() {
                inputs.extend(dataset.product_index.get(id).copied());
            }
        }
        (outputs, inputs)
    };
    let (made, used) = uses(world, plants);
    let kept: Vec<Handle> = world
        .firms
        .get(parent)
        .map(|f| f.facilities.clone())
        .unwrap_or_default();
    let (_, still_used) = uses(world, &kept);
    let mut ending: Vec<Handle> = Vec::new();
    let moving: Vec<(Handle, bool)> = world
        .contracts
        .iter()
        .filter(|(_, c)| c.status == crate::state::ContractStatus::Active && c.escrow_cents == 0)
        .filter_map(|(h, c)| {
            if c.seller == parent && c.from_region == region && made.contains(&c.product) {
                Some((h, true))
            } else if c.buyer == parent && c.to_region == region && !still_used.contains(&c.product)
            {
                if used.contains(&c.product) {
                    Some((h, false))
                } else {
                    ending.push(h);
                    None
                }
            } else {
                None
            }
        })
        .collect();
    for handle in ending {
        let seller = world.contracts.get(handle).map(|c| c.seller);
        if let Some(c) = world.contracts.get_mut(handle) {
            c.status = crate::state::ContractStatus::Cancelled;
        }
        if let Some(p) = world.firms.get_mut(parent) {
            p.contracts_buy.retain(|h| *h != handle);
        }
        if let Some(s) = seller.and_then(|s| world.firms.get_mut(s)) {
            s.contracts_sell.retain(|h| *h != handle);
        }
    }
    for (handle, sell) in moving {
        if let Some(c) = world.contracts.get_mut(handle) {
            if sell {
                c.seller = child;
            } else {
                c.buyer = child;
            }
        }
        if let Some(p) = world.firms.get_mut(parent) {
            p.contracts_sell.retain(|h| *h != handle);
            p.contracts_buy.retain(|h| *h != handle);
        }
        if let Some(c) = world.firms.get_mut(child) {
            if sell {
                c.contracts_sell.push(handle);
            } else {
                c.contracts_buy.push(handle);
            }
        }
    }
}

pub fn controlling_parent(world: &World, sub: Handle) -> DomainResult<Handle> {
    match world
        .firms
        .get(sub)
        .ok_or_else(|| firm_missing(sub))?
        .shares
        .controller()
    {
        Some(HolderId::Firm(parent)) => Ok(parent),
        _ => Err(wrong_phase("subsidiary", "not_a_subsidiary")),
    }
}

/// Float part of a subsidiary on the exchange. The parent sells existing shares to outside
/// investors at a discount to the mark and keeps control.
pub fn list_subsidiary(world: &mut World, sub: Handle, stake_bps: i32) -> DomainResult<i64> {
    if !(500..=4_900).contains(&stake_bps) {
        return Err(out_of_range());
    }
    let parent = controlling_parent(world, sub)?;
    let entity = world.firms.get(sub).ok_or_else(|| firm_missing(sub))?;
    if entity.book.total_equity() <= 0 || entity.shares.statement_weeks < 13 {
        return Err(DomainError::SolvencyRestriction {
            entity: sub.0,
            reason: SolvencyReason::NegativeEquity,
        });
    }
    let eligible = entity.shares.eligible_votes();
    let held = entity.shares.qty(HolderId::Firm(parent));
    let sell = eligible * stake_bps as i64 / 10_000;
    if sell <= 0 || (held - sell) * 2 <= eligible {
        return Err(DomainError::SolvencyRestriction {
            entity: sub.0,
            reason: SolvencyReason::CoverageTooLow,
        });
    }
    let reference = if entity.shares.mark_cents > 0 {
        entity.shares.mark_cents
    } else {
        entity.shares.fair_cents
    };
    let price = (reference * LISTING_DISCOUNT_BPS / 10_000).max(1);
    let proceeds = sell * price;
    let fee = (proceeds * 200 / 10_000).max(10_000_000).min(proceeds);
    let capital: i64 = world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum();
    if capital < proceeds {
        return Err(DomainError::InsufficientCash {
            entity: 0,
            required: proceeds,
            available: capital,
        });
    }
    // Outside investors take it in order of capital, largest first by index for determinism.
    let mut taken: Vec<(u32, i64, i64)> = Vec::new();
    let mut left_qty = sell;
    for (index, holder) in world.outside.investors.iter_mut().enumerate() {
        if left_qty <= 0 {
            break;
        }
        let affordable = holder.capital_cents / price;
        let qty = affordable.min(left_qty);
        if qty > 0 {
            holder.capital_cents -= qty * price;
            taken.push((index as u32, qty, qty * price));
            left_qty -= qty;
        }
    }
    if left_qty > 0 {
        // Roll back: capital was not enough in whole shares.
        for (index, _, paid) in taken {
            world.outside.investors[index as usize].capital_cents += paid;
        }
        return Err(DomainError::InsufficientCash {
            entity: 0,
            required: proceeds,
            available: capital,
        });
    }
    let cost_basis = {
        let lot = world
            .firms
            .get(sub)
            .map(|f| {
                f.shares
                    .holders
                    .get(&HolderId::Firm(parent))
                    .copied()
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if lot.qty > 0 {
            (lot.cost_cents as i128 * sell as i128 / lot.qty as i128) as i64
        } else {
            0
        }
    };
    let net = proceeds - fee;
    if let Some(entity) = world.firms.get_mut(sub) {
        for (index, qty, paid) in &taken {
            entity.shares.transfer(
                HolderId::Firm(parent),
                HolderId::Outside(*index),
                *qty,
                *paid,
            );
        }
        entity.shares.ipo_done = true;
    }
    let gain = proceeds - cost_basis;
    if let Some(p) = world.firms.get_mut(parent) {
        post(
            &mut p.book,
            world.tick,
            &[
                (Account::Cash, net),
                (Account::OperatingExpense, fee),
                (Account::Investments, -cost_basis),
                (Account::OtherIncome, -gain),
            ],
            "subsidiary listing",
        );
    }
    post(
        &mut world.outside.legal_services_book,
        world.tick,
        &[(Account::Cash, fee), (Account::Revenue, -fee)],
        "listing fee",
    );
    ownership::invalidate(world, parent);
    world.structure.stats.ipos += 1;
    Ok(net)
}

/// Hand a subsidiary to the parent's shareholders in proportion to their shares. The parent
/// ceases to hold it.
pub fn spin_off(world: &mut World, sub: Handle) -> DomainResult<()> {
    let parent = controlling_parent(world, sub)?;
    let lot = world
        .firms
        .get(sub)
        .ok_or_else(|| firm_missing(sub))?
        .shares
        .holders
        .get(&HolderId::Firm(parent))
        .copied()
        .unwrap_or_default();
    if lot.qty <= 0 {
        return Err(wrong_phase("subsidiary", "no_stake"));
    }
    let holders: Vec<(HolderId, i64)> = world
        .firms
        .get(parent)
        .ok_or_else(|| firm_missing(parent))?
        .shares
        .holders
        .iter()
        .filter(|(h, l)| !h.is_treasury() && l.qty > 0)
        .filter(|(h, _)| match h {
            HolderId::Firm(other) => !ownership::would_create_cycle(world, *other, sub),
            _ => true,
        })
        .map(|(h, l)| (*h, l.qty))
        .collect();
    if holders.is_empty() {
        return Err(wrong_phase("shareholders", "none"));
    }
    let weights: Vec<(u64, i64)> = holders
        .iter()
        .enumerate()
        .map(|(i, (_, q))| (i as u64, *q))
        .collect();
    let split = allocate_cents(Cents(lot.qty), &weights);
    let cost_split = allocate_cents(Cents(lot.cost_cents), &weights);
    let tick = world.tick;
    for (i, (holder, _)) in holders.iter().enumerate() {
        let qty = split[i].1.0;
        let cost = cost_split[i].1.0;
        if qty <= 0 {
            continue;
        }
        if let Some(s) = world.firms.get_mut(sub) {
            s.shares
                .transfer(HolderId::Firm(parent), *holder, qty, cost);
        }
        match holder {
            HolderId::Investor => post(
                &mut world.investor.book,
                tick,
                &[(Account::Investments, cost), (Account::OtherIncome, -cost)],
                "spin-off",
            ),
            HolderId::Firm(h) => {
                if let Some(f) = world.firms.get_mut(*h) {
                    post(
                        &mut f.book,
                        tick,
                        &[(Account::Investments, cost), (Account::OtherIncome, -cost)],
                        "spin-off",
                    );
                }
            }
            _ => {}
        }
    }
    // The parent drops the stake from its books against retained earnings, then share capital.
    if let Some(p) = world.firms.get_mut(parent) {
        let distributable = (-p.book.balance(Account::RetainedEarnings)).max(0);
        let from_re = lot.cost_cents.min(distributable);
        post(
            &mut p.book,
            tick,
            &[
                (Account::RetainedEarnings, from_re),
                (Account::ShareCapital, lot.cost_cents - from_re),
                (Account::Investments, -lot.cost_cents),
            ],
            "spin-off",
        );
    }
    ownership::invalidate(world, parent);
    world.structure.stats.spin_offs += 1;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Joint ventures
// ---------------------------------------------------------------------------------------------

/// Capital a joint venture needs: the build, its entry fee, a fifth again, and upkeep and land
/// rent through the build and half a year of operation.
pub fn jv_capital(dataset: &Dataset, recipe: u16, blocks: u8) -> i64 {
    let template = &dataset.recipes[recipe as usize];
    let build = template.capex_cents * blocks as i64 - template.capex_cents / 2;
    let fixed = template.weekly_maintenance(dataset.maintenance_annual_bps)
        + dataset.fixture_land_rent_cents_weekly * 2;
    let payroll = template.baseline_weekly_payroll(dataset.baseline_weekly_wage_cents);
    let upkeep = fixed * blocks as i64 * template.build_weeks as i64
        + (fixed + payroll) * blocks as i64 * 26;
    (build + sectors::ENTRY_FEE_CENTS) * 125 / 100 + upkeep
}

/// Two groups co-own a new firm that builds one division. `stake_a_bps` is the first partner's
/// stake; each pays capital in proportion. A stake over half makes the venture a subsidiary of
/// that partner; an even split leaves it independent, run by the standing rules.
#[allow(clippy::too_many_arguments)]
pub fn form_jv(
    world: &mut World,
    dataset: &Dataset,
    a: Handle,
    b: Handle,
    region: u8,
    recipe: u16,
    blocks: u8,
    stake_a_bps: i32,
    name: String,
) -> DomainResult<Handle> {
    if !(2_000..=8_000).contains(&stake_a_bps) {
        return Err(out_of_range());
    }
    if a == b || ownership::same_group(world, a, b) {
        return Err(wrong_phase("two_groups", "same_group"));
    }
    for f in [a, b] {
        if !world.firms.get(f).is_some_and(|x| x.operative()) {
            return Err(firm_missing(f));
        }
    }
    if dataset.recipes.get(recipe as usize).is_none() || blocks == 0 {
        return Err(out_of_range());
    }
    if (region as usize) >= world.regions.len() {
        return Err(DomainError::InvalidEntity {
            handle: region as u64,
            kind: EntityKind::Region,
        });
    }
    let probe = crate::commands::blank_firm(
        world,
        dataset,
        name.clone(),
        sectors::recipe_industry(dataset, recipe),
        region,
    );
    let _ = probe;
    let industry = sectors::recipe_industry(dataset, recipe);
    let capital = jv_capital(dataset, recipe, blocks);
    let share_a = capital * stake_a_bps as i64 / 10_000;
    let share_b = capital - share_a;
    for (firm, need) in [(a, share_a), (b, share_b)] {
        let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
        let buffer = weekly_outflow_buffer(world, firm);
        if cash < need + buffer {
            return Err(DomainError::InsufficientCash {
                entity: firm.0,
                required: need + buffer,
                available: cash,
            });
        }
    }
    let mut jv = crate::commands::blank_firm(world, dataset, name, industry, region);
    jv.shares.issued = INITIAL_SHARES;
    let qty_a = INITIAL_SHARES * stake_a_bps as i64 / 10_000;
    jv.shares.holders.insert(
        HolderId::Firm(a),
        ShareLot {
            qty: qty_a,
            cost_cents: share_a,
        },
    );
    jv.shares.holders.insert(
        HolderId::Firm(b),
        ShareLot {
            qty: INITIAL_SHARES - qty_a,
            cost_cents: share_b,
        },
    );
    let tick = world.tick;
    post(
        &mut jv.book,
        tick,
        &[(Account::Cash, capital), (Account::ShareCapital, -capital)],
        "joint venture capital",
    );
    let jv_handle = world.firms.insert(jv);
    for (firm, paid) in [(a, share_a), (b, share_b)] {
        if let Some(f) = world.firms.get_mut(firm) {
            post(
                &mut f.book,
                tick,
                &[(Account::Investments, paid), (Account::Cash, -paid)],
                "joint venture capital",
            );
        }
    }
    // A venture one partner controls inherits that partner's ceo arrangement.
    if let Some(controller) = world
        .firms
        .get(jv_handle)
        .and_then(|f| f.shares.controller())
        && ownership::ultimate_controller(world, jv_handle) == Some(HolderId::Investor)
        && matches!(controller, HolderId::Firm(_))
        && let Some(f) = world.firms.get_mut(jv_handle)
    {
        f.board.ceo = Some(CeoRef::Investor);
    }
    ownership::invalidate(world, a);
    ownership::invalidate(world, b);
    // Seats are filled from the stakes at once, so the controlling partner can authorise the
    // venture's first build without waiting for a board meeting.
    if let Some(f) = world.firms.get_mut(jv_handle) {
        f.board.seats = crate::economy::governance::apportion_seats(&f.shares);
    }
    let built = crate::commands::commission_plant(
        world,
        dataset,
        crate::commands::INVESTOR_ACTOR,
        jv_handle,
        region,
        recipe,
        blocks,
    );
    if let Err(error) = built {
        // Unwind: nothing was built, so return the capital and drop the venture.
        let returned = world
            .firms
            .get(jv_handle)
            .map(|f| f.book.cash())
            .unwrap_or(0);
        for (firm, paid) in [(a, share_a), (b, share_b)] {
            if let Some(f) = world.firms.get_mut(firm) {
                post(
                    &mut f.book,
                    tick,
                    &[(Account::Cash, paid), (Account::Investments, -paid)],
                    "joint venture unwound",
                );
            }
        }
        let _ = returned;
        world.firms.remove(jv_handle);
        ownership::invalidate(world, a);
        ownership::invalidate(world, b);
        return Err(error);
    }
    world.structure.jvs.push(JointVenture {
        firm: jv_handle,
        a,
        b,
        formed: world.tick.0,
    });
    world.structure.stats.jvs_formed += 1;
    crate::economy::rivalry::on_venture(world, a, b);
    Ok(jv_handle)
}

fn jv_of(world: &World, jv: Handle) -> DomainResult<JointVenture> {
    world
        .structure
        .jvs
        .iter()
        .find(|j| j.firm == jv)
        .cloned()
        .ok_or(wrong_phase("joint_venture", "not_a_venture"))
}

fn stake_price(world: &World, jv: Handle, partner: Handle, basis_bps: i64) -> (i64, i64) {
    let Some(f) = world.firms.get(jv) else {
        return (0, 0);
    };
    let qty = f.shares.qty(HolderId::Firm(partner));
    let fair = f.shares.fair_cents.max(f.shares.mark_cents).max(1);
    (qty, qty * fair * basis_bps / 10_000)
}

/// Move a joint venture stake between two holders at a price, with the books to match.
fn move_stake(
    world: &mut World,
    jv: Handle,
    seller: Handle,
    buyer: HolderId,
    qty: i64,
    price: i64,
) {
    let tick = world.tick;
    let cost_basis = world
        .firms
        .get(jv)
        .map(|f| {
            let lot = f
                .shares
                .holders
                .get(&HolderId::Firm(seller))
                .copied()
                .unwrap_or_default();
            if lot.qty > 0 {
                (lot.cost_cents as i128 * qty as i128 / lot.qty as i128) as i64
            } else {
                0
            }
        })
        .unwrap_or(0);
    if let Some(f) = world.firms.get_mut(jv) {
        f.shares.transfer(HolderId::Firm(seller), buyer, qty, price);
    }
    if let Some(f) = world.firms.get_mut(seller) {
        post(
            &mut f.book,
            tick,
            &[
                (Account::Cash, price),
                (Account::Investments, -cost_basis),
                (Account::OtherIncome, -(price - cost_basis)),
            ],
            "joint venture stake sale",
        );
    }
    match buyer {
        HolderId::Firm(b) => {
            if let Some(f) = world.firms.get_mut(b) {
                post(
                    &mut f.book,
                    tick,
                    &[(Account::Investments, price), (Account::Cash, -price)],
                    "joint venture stake purchase",
                );
            }
        }
        HolderId::Outside(i) => {
            if let Some(h) = world.outside.investors.get_mut(i as usize) {
                h.capital_cents -= price;
            }
        }
        _ => {}
    }
}

/// A partner buys the other out at fair value plus a premium.
pub fn buyout_jv(world: &mut World, jv: Handle, buyer: Handle) -> DomainResult<()> {
    let venture = jv_of(world, jv)?;
    if world.tick.0 < venture.formed + JV_LOCKUP_WEEKS {
        return Err(wrong_phase("after_lockup", "locked_up"));
    }
    let seller = if buyer == venture.a {
        venture.b
    } else if buyer == venture.b {
        venture.a
    } else {
        return Err(DomainError::NoAuthority {
            actor: buyer.0,
            reason: AuthorityReason::NotOwner,
        });
    };
    let (qty, price) = stake_price(world, jv, seller, JV_BUYOUT_PREMIUM_BPS);
    let cash = world.firms.get(buyer).map(|f| f.book.cash()).unwrap_or(0);
    if qty <= 0 || cash < price + weekly_outflow_buffer(world, buyer) {
        return Err(DomainError::InsufficientCash {
            entity: buyer.0,
            required: price,
            available: cash,
        });
    }
    move_stake(world, jv, seller, HolderId::Firm(buyer), qty, price);
    world.structure.jvs.retain(|j| j.firm != jv);
    ownership::invalidate(world, buyer);
    ownership::invalidate(world, seller);
    world.structure.stats.jv_buyouts += 1;
    Ok(())
}

/// A partner leaves: the other partner has first refusal at a small discount to fair value; if it
/// cannot pay, the stake goes to the outside market at a larger discount.
pub fn exit_jv(world: &mut World, jv: Handle, seller: Handle) -> DomainResult<()> {
    let venture = jv_of(world, jv)?;
    if world.tick.0 < venture.formed + JV_LOCKUP_WEEKS {
        return Err(wrong_phase("after_lockup", "locked_up"));
    }
    let partner = if seller == venture.a {
        venture.b
    } else if seller == venture.b {
        venture.a
    } else {
        return Err(DomainError::NoAuthority {
            actor: seller.0,
            reason: AuthorityReason::NotOwner,
        });
    };
    let (qty, partner_price) = stake_price(world, jv, seller, JV_EXIT_PARTNER_BPS);
    if qty <= 0 {
        return Err(wrong_phase("stake", "none"));
    }
    let partner_cash = world.firms.get(partner).map(|f| f.book.cash()).unwrap_or(0);
    if world.firms.get(partner).is_some_and(|f| f.operative())
        && partner_cash >= partner_price + weekly_outflow_buffer(world, partner)
    {
        move_stake(
            world,
            jv,
            seller,
            HolderId::Firm(partner),
            qty,
            partner_price,
        );
        world.structure.jvs.retain(|j| j.firm != jv);
    } else {
        let (_, market_price) = stake_price(world, jv, seller, JV_EXIT_MARKET_BPS);
        let buyer = (0..world.outside.investors.len())
            .find(|i| world.outside.investors[*i].capital_cents >= market_price)
            .ok_or(DomainError::InsufficientCash {
                entity: 0,
                required: market_price,
                available: 0,
            })?;
        move_stake(
            world,
            jv,
            seller,
            HolderId::Outside(buyer as u32),
            qty,
            market_price,
        );
        world.structure.jvs.retain(|j| j.firm != jv);
    }
    ownership::invalidate(world, seller);
    ownership::invalidate(world, partner);
    world.structure.stats.jv_exits += 1;
    Ok(())
}

/// Ventures share out surplus cash every quarter in proportion to their stakes.
fn jv_weekly(world: &mut World) {
    let alive: Vec<JointVenture> = world
        .structure
        .jvs
        .iter()
        .filter(|j| world.firms.get(j.firm).is_some_and(|f| f.operative()))
        .cloned()
        .collect();
    world.structure.jvs = alive.clone();
    if !world.tick.0.is_multiple_of(13) {
        return;
    }
    for venture in alive {
        let Some(f) = world.firms.get(venture.firm) else {
            continue;
        };
        if f.facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .any(|p| p.status == FacilityStatus::Building)
        {
            continue;
        }
        let surplus = (f.book.cash() - weekly_outflow_buffer(world, venture.firm) * 2).max(0);
        if surplus > 0 {
            let _ = crate::economy::equity::pay_dividend(world, venture.firm, surplus * 6 / 10);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Activists
// ---------------------------------------------------------------------------------------------

/// A dividend paid: kept for the dividend-cut test.
pub fn note_dividend(world: &mut World, firm: Handle) {
    let tick = world.tick.0;
    let last = world
        .structure
        .last_dividend
        .get(&firm)
        .copied()
        .unwrap_or(0);
    if last == 0 || tick.saturating_sub(last) > 13 {
        let q = world.structure.dividend_quarters.entry(firm).or_default();
        *q = if last > 0 && tick.saturating_sub(last) <= 26 {
            q.saturating_add(1)
        } else {
            1
        };
    }
    world.structure.last_dividend.insert(firm, tick);
}

fn outside_vote_bps(world: &World, firm: Handle) -> i32 {
    world
        .firms
        .get(firm)
        .map(|f| {
            f.shares
                .holders
                .keys()
                .filter(|h| h.is_external())
                .map(|h| f.shares.vote_bps(*h))
                .sum()
        })
        .unwrap_or(0)
}

fn controller_hold_bps(world: &World, firm: Handle) -> i32 {
    world
        .firms
        .get(firm)
        .and_then(|f| f.shares.controller().map(|c| f.shares.vote_bps(c)))
        .unwrap_or(0)
}

fn player_run(world: &World, firm: Handle) -> bool {
    crate::economy::events_engine::player_side(world, firm)
}

fn campaign_open(world: &World, firm: Handle) -> bool {
    world
        .structure
        .campaigns
        .iter()
        .any(|c| c.target == firm && c.outcome == Outcome::Open)
}

/// The strongest case against a firm this quarter, if any.
fn grievance(world: &World, dataset: &Dataset, firm: Handle) -> Option<(Reason, i32, i64)> {
    let f = world.firms.get(firm)?;
    let mut best: Option<(Reason, i32, i64)> = None;
    let mut consider = |reason: Reason, justification: i32, amount: i64| {
        if justification > 0 && best.is_none_or(|(_, j, _)| justification > j) {
            best = Some((reason, justification, amount));
        }
    };
    // A share price far under its peak.
    let peak = world.structure.peak_mark.get(&firm).copied().unwrap_or(0);
    if peak > 0 && f.shares.mark_cents > 0 {
        let fall = 10_000 - f.shares.mark_cents * 10_000 / peak;
        // Only a long slump counts: half a year far under the peak while losing money.
        let weak = world
            .structure
            .weak_price_weeks
            .get(&firm)
            .copied()
            .unwrap_or(0);
        let losing = world.structure.loss_weeks.get(&firm).copied().unwrap_or(0);
        if fall >= 5_500 && weak >= 39 && losing >= 52 && world.tick.0 >= 78 {
            consider(
                Reason::FallingPrice,
                (fall * 12 / 10).clamp(0, 10_000) as i32,
                0,
            );
        }
    }
    // Two missed board mandates in a row, for the player's primary company. A chapter the player
    // never took a mandate in does not count either way.
    if world.investor.primary_group == Some(firm) {
        let missed = world
            .chapters
            .results()
            .filter(|o| {
                !matches!(
                    o,
                    crate::economy::chapters::Outcome::Lapsed
                        | crate::economy::chapters::Outcome::Cancelled
                )
            })
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .take(2)
            .filter(|o| **o == crate::economy::chapters::Outcome::Missed)
            .count();
        if missed >= 2 {
            consider(Reason::MissedGoals, 6_500, 0);
        }
    }
    // Pricing inside the group at the expense of minority holders.
    let squeeze_year = world.structure.squeeze.get(&firm).copied().unwrap_or(0);
    let equity = f.valuation.equity_cents.max(1);
    if minority_bps(world, firm) > 0 && squeeze_year > 0 {
        let pct_bps = squeeze_year * 10_000 / equity;
        if pct_bps >= 300 {
            consider(
                Reason::Squeeze,
                (pct_bps * 20).clamp(0, 10_000) as i32,
                squeeze_year,
            );
        }
    }
    // A dividend payer that stopped while it can well afford one.
    let quarters = world
        .structure
        .dividend_quarters
        .get(&firm)
        .copied()
        .unwrap_or(0);
    let last = world
        .structure
        .last_dividend
        .get(&firm)
        .copied()
        .unwrap_or(0);
    if quarters >= 3
        && world.tick.0.saturating_sub(last) >= 26
        && f.book.cash() > weekly_outflow_buffer(world, firm) * 2
        && !f.payout_freeze
    {
        consider(Reason::DividendCut, 5_500, 0);
    }
    let _ = dataset;
    best
}

fn pick_demand(world: &World, dataset: &Dataset, firm: Handle, reason: Reason) -> Demand {
    let divisions = sectors::divisions_of(world, dataset, firm).len();
    let subs = ownership::controlled_by(world, HolderId::Firm(firm)).len();
    let can_divest = divisions >= 2 || subs >= 1;
    match reason {
        Reason::Squeeze => Demand::Restitution,
        Reason::DividendCut => Demand::Dividend,
        Reason::FallingPrice if can_divest => Demand::Divest,
        Reason::FallingPrice | Reason::MissedGoals => {
            if player_run(world, firm) {
                Demand::Dividend
            } else {
                Demand::Ceo
            }
        }
    }
}

/// Quarterly scan: open a campaign against a firm with outside shareholders that has given them
/// cause. One at a time per firm, a year apart.
fn scan_activists(world: &mut World, dataset: &Dataset) {
    let tick = world.tick.0;
    let handles: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    for firm in handles {
        let Some(f) = world.firms.get(firm) else {
            continue;
        };
        if !f.operative() || f.shares.statement_weeks < 26 || f.state_owned_since.is_some() {
            continue;
        }
        if (firm.slot() % 13) != tick % 13 {
            continue;
        }
        let float = outside_vote_bps(world, firm);
        if float < 500 || campaign_open(world, firm) {
            continue;
        }
        if world
            .structure
            .last_activist
            .get(&firm)
            .is_some_and(|last| tick < last + ACTIVIST_COOLDOWN_WEEKS)
        {
            continue;
        }
        let Some((reason, justification, amount)) = grievance(world, dataset, firm) else {
            continue;
        };
        let demand = pick_demand(world, dataset, firm, reason);
        let activist = (0..world.outside.investors.len())
            .max_by_key(|i| {
                (
                    world.outside.investors[*i].capital_cents,
                    std::cmp::Reverse(*i),
                )
            })
            .unwrap_or(0) as u32;
        let stake = (300 + justification / 10).min(float);
        open_campaign(
            world,
            firm,
            activist,
            stake,
            reason,
            demand,
            justification,
            amount,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn open_campaign(
    world: &mut World,
    firm: Handle,
    activist: u32,
    stake_bps: i32,
    reason: Reason,
    demand: Demand,
    justification_bps: i32,
    amount_cents: i64,
) {
    let id = world.structure.next_campaign;
    world.structure.next_campaign += 1;
    let tick = world.tick.0;
    world.structure.last_activist.insert(firm, tick);
    world.structure.stats.activist_events += 1;
    world.structure.campaigns.push(Campaign {
        id,
        target: firm,
        activist,
        stake_bps,
        reason,
        demand,
        justification_bps,
        amount_cents,
        opened: tick,
        deadline: tick + ACTIVIST_RESPONSE_WEEKS,
        outcome: Outcome::Open,
    });
    world.news.push(crate::state::NewsItem {
        week: world.tick,
        key: format!("news.activist.{}", reason.key()),
        subject: world
            .firms
            .get(firm)
            .map(|f| f.name.clone())
            .unwrap_or_default(),
        amount_cents: 0,
        quantity: MilliLots::ZERO,
    });
    if world.structure.campaigns.len() > 64 {
        world.structure.campaigns.remove(0);
    }
}

/// Chance the activist prevails at the vote or in court, basis points.
pub fn activist_chance_bps(world: &World, c: &Campaign) -> i32 {
    let base = c.justification_bps as i64 * 6 / 10
        + (c.stake_bps as i64 * 2).min(2_000)
        + outside_vote_bps(world, c.target) as i64 / 5
        - controller_hold_bps(world, c.target) as i64 * 3 / 10;
    base.clamp(500, 9_000) as i32
}

/// A rival's answer, from its temperament.
pub fn ai_stance(world: &World, dataset: &Dataset, c: &Campaign) -> Stance {
    let t = temperament(world, dataset, c.target);
    let j = c.justification_bps;
    if j >= 9_000 - t.resistance / 10 {
        Stance::Concede
    } else if j >= 7_000 + t.resistance / 5 {
        Stance::Compromise
    } else {
        Stance::Fight
    }
}

fn fee(world: &mut World, firm: Handle, amount: i64, memo: &str) {
    let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
    let paid = amount.min(cash).max(0);
    if paid <= 0 {
        return;
    }
    if let Some(f) = world.firms.get_mut(firm) {
        post(
            &mut f.book,
            world.tick,
            &[(Account::Cash, -paid), (Account::OperatingExpense, paid)],
            memo,
        );
    }
    post(
        &mut world.outside.legal_services_book,
        world.tick,
        &[(Account::Cash, paid), (Account::Revenue, -paid)],
        memo,
    );
}

fn apply_demand(world: &mut World, dataset: &Dataset, c: &Campaign, scale_pct: i64) {
    let firm = c.target;
    match c.demand {
        Demand::Dividend => {
            let cash = world.firms.get(firm).map(|f| f.book.cash()).unwrap_or(0);
            let spare = (cash - weekly_outflow_buffer(world, firm) * 2).max(0);
            let amount = spare * 25 / 100 * scale_pct / 100;
            if amount > 0 && crate::economy::equity::pay_dividend(world, firm, amount).is_ok() {
                note_dividend(world, firm);
            }
        }
        Demand::Divest => {
            // The weakest controlled subsidiary is floated; failing that the weakest division is
            // carved out and, on a full concession, spun off to shareholders.
            let subs = ownership::controlled_by(world, HolderId::Firm(firm));
            let weakest_sub = subs
                .into_iter()
                .min_by_key(|s| (trailing_net_income(world, *s), *s));
            if let Some(sub) = weakest_sub {
                let stake = if scale_pct >= 100 { 4_000 } else { 2_000 };
                let _ = list_subsidiary(world, sub, stake);
            } else if !player_run(world, firm) {
                // A rival has nobody to run a spun-off division: it pays out instead.
                let mut as_dividend = c.clone();
                as_dividend.demand = Demand::Dividend;
                apply_demand(world, dataset, &as_dividend, scale_pct);
            } else {
                let divisions = sectors::divisions_of(world, dataset, firm);
                if divisions.len() >= 2 {
                    let weakest = divisions
                        .iter()
                        .min_by_key(|d| (d.blocks, d.region, d.industry))
                        .cloned();
                    if let Some(d) = weakest {
                        let name = format!(
                            "{} {}",
                            world
                                .firms
                                .get(firm)
                                .map(|f| f.name.clone())
                                .unwrap_or_default(),
                            crate::state::INDUSTRY_NAMES[d.industry as usize]
                        );
                        if let Ok(child) =
                            carve_out(world, dataset, firm, d.region, d.industry, name)
                            && scale_pct >= 100
                        {
                            let _ = spin_off(world, child);
                        }
                    }
                }
            }
        }
        Demand::Ceo => {
            let changes = world.structure.ceo_changes.entry(firm).or_default();
            *changes += 1;
            let next = 1_000 + *changes as u32;
            if let Some(f) = world.firms.get_mut(firm) {
                f.board.ceo = Some(CeoRef::Manager(next));
            }
            let equity = world
                .firms
                .get(firm)
                .map(|f| f.valuation.equity_cents)
                .unwrap_or(0);
            fee(world, firm, equity / 1_000, "chief executive severance");
            // The named chief executive goes too, when the firm has one.
            let mut rng = cw_core::rng::Rng::entity_stream(
                world.seed,
                0x4143_5449_5645_4345,
                Handle(firm.0 ^ (world.tick.0 as u64) << 40),
            );
            crate::economy::cast::replace(
                world,
                dataset,
                firm,
                "after an activist campaign",
                &mut rng,
            );
        }
        Demand::Restitution => {
            let amount = c.amount_cents * scale_pct / 100;
            // The controller pays the damage back into the firm that was shorted, and prices at
            // market again.
            if let Some(HolderId::Firm(parent)) =
                world.firms.get(firm).and_then(|f| f.shares.controller())
            {
                let cash = world.firms.get(parent).map(|f| f.book.cash()).unwrap_or(0);
                let pay = amount.min((cash - weekly_outflow_buffer(world, parent)).max(0));
                if pay > 0 {
                    let tick = world.tick;
                    if let Some(p) = world.firms.get_mut(parent) {
                        post(
                            &mut p.book,
                            tick,
                            &[(Account::Investments, pay), (Account::Cash, -pay)],
                            "restitution",
                        );
                    }
                    if let Some(f) = world.firms.get_mut(firm) {
                        post(
                            &mut f.book,
                            tick,
                            &[(Account::Cash, pay), (Account::ShareCapital, -pay)],
                            "restitution",
                        );
                    }
                }
            }
            if scale_pct >= 50 {
                world.structure.transfer.retain(|(f, _, _), _| *f != firm);
                let links: Vec<(Handle, Handle, u16)> = world
                    .structure
                    .links
                    .keys()
                    .filter(|(b, s, _)| *b == firm || *s == firm)
                    .copied()
                    .collect();
                for (b, s, _) in links {
                    world
                        .structure
                        .transfer
                        .retain(|(f, _, _), _| *f != b && *f != s);
                }
            }
            world.structure.squeeze.remove(&firm);
        }
    }
}

/// Settle a campaign with the board's stance.
pub fn resolve_campaign(
    world: &mut World,
    dataset: &Dataset,
    id: u32,
    stance: Stance,
    ignored: bool,
) {
    let Some(index) = world.structure.campaigns.iter().position(|c| c.id == id) else {
        return;
    };
    let mut c = world.structure.campaigns[index].clone();
    if c.outcome != Outcome::Open {
        return;
    }
    let equity = world
        .firms
        .get(c.target)
        .map(|f| f.valuation.equity_cents)
        .unwrap_or(0);
    let chance = activist_chance_bps(world, &c) + if ignored { 1_000 } else { 0 };
    match stance {
        Stance::Concede => {
            apply_demand(world, dataset, &c, 100);
            c.outcome = Outcome::Conceded;
        }
        Stance::Compromise => {
            apply_demand(world, dataset, &c, 50);
            fee(world, c.target, equity / 1_000, "activist settlement");
            c.outcome = Outcome::Compromised;
        }
        Stance::Fight => {
            let mut rng = cw_core::rng::Rng::entity_stream(
                world.seed ^ ((c.id as u64) << 24) ^ ((world.tick.0 as u64) << 8),
                cw_core::rng::TAG_POLICY,
                c.target,
            );
            if rng.chance_bps(chance.clamp(0, 10_000)) {
                apply_demand(world, dataset, &c, 100);
                fee(world, c.target, equity * 4 / 1_000, "activist damages");
                c.outcome = Outcome::ActivistWon;
                world.structure.stats.activist_wins += 1;
            } else {
                fee(world, c.target, equity * 15 / 10_000, "legal costs");
                c.outcome = Outcome::ActivistLost;
            }
        }
    }
    world.structure.campaigns[index] = c;
}

/// The player answers a campaign against one of their companies. Stances: 0 concede, 1
/// negotiate, 2 fight.
pub fn respond_campaign(
    world: &mut World,
    dataset: &Dataset,
    id: u32,
    stance: u8,
) -> DomainResult<()> {
    let c = world
        .structure
        .campaigns
        .iter()
        .find(|c| c.id == id)
        .cloned()
        .ok_or(DomainError::InvalidEntity {
            handle: id as u64,
            kind: EntityKind::Event,
        })?;
    if c.outcome != Outcome::Open {
        return Err(wrong_phase("open", "settled"));
    }
    if !player_run(world, c.target) {
        return Err(DomainError::NoAuthority {
            actor: 0,
            reason: AuthorityReason::NotOwner,
        });
    }
    let stance = match stance {
        0 => Stance::Concede,
        1 => Stance::Compromise,
        2 => Stance::Fight,
        _ => return Err(out_of_range()),
    };
    resolve_campaign(world, dataset, id, stance, false);
    Ok(())
}

/// Campaigns nobody answered resolve at their deadline by the board's standing temperament.
fn settle_campaigns(world: &mut World, dataset: &Dataset) {
    let due: Vec<Campaign> = world
        .structure
        .campaigns
        .iter()
        .filter(|c| c.outcome == Outcome::Open && world.tick.0 >= c.deadline)
        .cloned()
        .collect();
    for c in due {
        if !world.firms.get(c.target).is_some_and(|f| f.operative()) {
            if let Some(stored) = world.structure.campaigns.iter_mut().find(|s| s.id == c.id) {
                stored.outcome = Outcome::ActivistLost;
            }
            continue;
        }
        let stance = ai_stance(world, dataset, &c);
        resolve_campaign(world, dataset, c.id, stance, false);
    }
}

// ---------------------------------------------------------------------------------------------
// Weekly
// ---------------------------------------------------------------------------------------------

/// Everything that runs once a week after the exchange clears.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    // Squeeze fades, suppliers losing money age, reputation moves.
    for value in world.structure.squeeze.values_mut() {
        *value -= *value / SQUEEZE_DECAY_DEN;
    }
    world.structure.squeeze.retain(|_, v| *v > 100);
    update_captive(world);
    update_reputation(world, dataset);
    // Raid heat is kept only while it still counts.
    let tick = world.tick.0;
    world
        .structure
        .raid_heat
        .retain(|_, h| h.count as u32 > tick.saturating_sub(h.last) / 13);
    world
        .structure
        .links
        .retain(|(b, s, _), _| world.firms.contains(*b) && world.firms.contains(*s));
    // Bond bases only for bonds that still exist.
    let live: std::collections::BTreeSet<Handle> = world.bonds.iter().map(|(h, _)| h).collect();
    world
        .structure
        .bond_basis
        .retain(|(bond, _), _| live.contains(bond));
    // Peak marks, with a slow fade so an old peak stops counting.
    let marks: Vec<(Handle, i64)> = world
        .firms
        .iter()
        .map(|(h, f)| (h, f.shares.mark_cents))
        .collect();
    for (handle, mark) in marks {
        let peak = world.structure.peak_mark.entry(handle).or_insert(mark);
        *peak = mark.max(*peak - *peak / 200);
        let weak = mark > 0 && mark * 100 <= *peak * 45;
        let weeks = world.structure.weak_price_weeks.entry(handle).or_default();
        *weeks = if weak { weeks.saturating_add(1) } else { 0 };
        let lost = world
            .firms
            .get(handle)
            .and_then(|f| f.book.recent_weeks().last().map(|w| w.net_income < 0))
            .unwrap_or(false);
        let losing = world.structure.loss_weeks.entry(handle).or_default();
        *losing = if lost { losing.saturating_add(1) } else { 0 };
        let gained = world
            .firms
            .get(handle)
            .and_then(|f| f.book.recent_weeks().last().map(|w| w.net_income > 0))
            .unwrap_or(false);
        let profit = world.structure.profit_weeks.entry(handle).or_default();
        *profit = if gained { profit.saturating_add(1) } else { 0 };
    }
    world
        .structure
        .profit_weeks
        .retain(|h, w| *w > 0 && world.firms.contains(*h));
    world
        .structure
        .weak_price_weeks
        .retain(|h, w| *w > 0 && world.firms.contains(*h));
    world
        .structure
        .loss_weeks
        .retain(|h, w| *w > 0 && world.firms.contains(*h));
    world
        .structure
        .peak_mark
        .retain(|h, _| world.firms.contains(*h));
    jv_weekly(world);
    scan_activists(world, dataset);
    settle_campaigns(world, dataset);
}
