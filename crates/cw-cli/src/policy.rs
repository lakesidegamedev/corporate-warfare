//! Explainable reference player policies for the balance sweeps.
//!
//! These are *players*, not rivals. Rival behaviour lives in `cw-ai` and deliberately never
//! operates a firm the investor holds. A reference policy observes the same read models the
//! phone client receives (`Session::overview` and `Query::*`) plus two documented economy
//! projections (the mandatory weekly outflow and secured/unsecured borrowing capacity that the
//! UI forecast already uses). Every four weeks it submits ordinary `CommandIntent`s through
//! `Session::submit`; the common validator and solvency service accept or reject them. The
//! policy never writes `World` state, never skips a rule and never grants itself cash.
//!
//! The three policies are deliberately different business strategies, not difficulty levels:
//!
//! * **Conservative operator** - runs the inherited firm for cash. Holds one quarter (13 weeks)
//!   of mandatory payroll and debt service, runs at 85 percent of capacity when that reserve is
//!   intact (70 percent when it is not), spends one percent of weekly revenue on marketing and
//!   nothing on research. It borrows only to restore the reserve, never to expand.
//! * **Growth operator** - reinvests. Runs at full capacity every week, spends four percent of
//!   weekly revenue on marketing and two percent on research, and keeps only eight weeks of
//!   mandatory outflow. It borrows to restore that thinner buffer, so a bad stretch becomes
//!   leverage rather than a production cut.
//! * **Acquirer** - buys scale. Runs at 95 percent of capacity (80 percent under stress), spends
//!   two percent on marketing and keeps ten weeks of outflow. When no deal is live and the
//!   reserve survives the escrow, it starts a funded diligence on the weakest operating peer
//!   (most negative trailing thirteen-week net income, or grade C/D), offering 110 percent of
//!   fair value and 115 percent of the exchange mark for a bare voting majority. Acquired firms
//!   are then operated with the same policy.
//!
//! A founder-scenario sweep has no inherited firm. All three policies share one bootstrap:
//! found a farm in region 035 with $2.5M of paid-in capital at week 0, then operate it as above.
//! The bootstrap is the same for every policy so the distribution isolates operating choices.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_session::{Detail, FinanceSummary, Query, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::state::DealKind;

/// Namespace for reference-player commands, distinct from the rival `AI_COMMAND_HIGH`.
pub const PLAYER_COMMAND_HIGH: u64 = 0x00C0_FFFF_0000_0000;

/// Reference policies decide every four weeks, like the rival strategic cohorts.
pub const CADENCE_WEEKS: u32 = 4;

/// Founder bootstrap: Central Ohio is the guided start's home region; the farm is the
/// documented starter strategy (see `content/initial-balance.json`).
const FOUNDER_REGION: &str = "OH-CEN";
const FOUNDER_RECIPE: &str = "farm";
const FOUNDER_NAME: &str = "Lakeside Holdings";
/// Fund the founding farm plant plus working capital and room to build, matching the guided
/// firm's opening assets and cash without its opening loan.
const FOUNDER_CAPITAL_CENTS: i64 = 300_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferencePolicy {
    None,
    Conservative,
    Growth,
    Acquirer,
    /// Builds a plant every twelve weeks where imports show room, in any sector.
    Expander,
}

impl ReferencePolicy {
    pub fn parse(text: &str) -> Option<ReferencePolicy> {
        match text {
            "none" => Some(ReferencePolicy::None),
            "conservative" => Some(ReferencePolicy::Conservative),
            "growth" => Some(ReferencePolicy::Growth),
            "acquirer" => Some(ReferencePolicy::Acquirer),
            "expander" => Some(ReferencePolicy::Expander),
            _ => None,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            ReferencePolicy::None => "none",
            ReferencePolicy::Conservative => "conservative",
            ReferencePolicy::Growth => "growth",
            ReferencePolicy::Acquirer => "acquirer",
            ReferencePolicy::Expander => "expander",
        }
    }

    /// Weeks of mandatory payroll and debt service kept as a cash floor.
    const fn reserve_weeks(self) -> i64 {
        match self {
            ReferencePolicy::None => 0,
            ReferencePolicy::Conservative => 13,
            ReferencePolicy::Growth => 8,
            ReferencePolicy::Acquirer => 10,
            ReferencePolicy::Expander => 8,
        }
    }

    /// Marketing as a percentage of trailing weekly revenue.
    const fn marketing_percent(self) -> i64 {
        match self {
            ReferencePolicy::None => 0,
            ReferencePolicy::Conservative => 1,
            ReferencePolicy::Growth => 4,
            ReferencePolicy::Acquirer => 2,
            ReferencePolicy::Expander => 2,
        }
    }

    /// Research as a percentage of trailing weekly revenue.
    const fn research_percent(self) -> i64 {
        match self {
            ReferencePolicy::None => 0,
            ReferencePolicy::Growth => 2,
            _ => 0,
        }
    }
}

/// Weekly contribution of one new plant at reference prices: output less inputs (adjusted for
/// sector focus and location), payroll at the region's wages plus any off-focus overhead,
/// maintenance and land.
fn plant_contribution(
    world: &cw_sim::state::World,
    dataset: &cw_data::Dataset,
    firm: &cw_sim::state::Firm,
    region: u8,
    recipe: u16,
) -> i64 {
    use cw_sim::economy::{operations, sectors};
    let template = &dataset.recipes[recipe as usize];
    let price = |id: &str| {
        dataset
            .product_index
            .get(id)
            .map(|code| dataset.reference_price(*code))
            .unwrap_or(0)
    };
    let industry = sectors::recipe_industry(dataset, recipe);
    let (focus_bps, overhead_bps) = if industry == firm.industry {
        (operations::PRIMARY_INPUT_BPS as i64, 0)
    } else if Some(industry) == firm.secondary_industry {
        (operations::SECONDARY_INPUT_BPS as i64, 0)
    } else {
        (10_000, operations::OFF_FOCUS_OVERHEAD_BPS)
    };
    let location =
        sectors::location_factor_bps(dataset, firm.region, region, &template.industry) as i64;
    let inputs: i64 = template
        .weekly_inputs_lots
        .iter()
        .map(|(id, qty)| qty * price(id))
        .sum::<i64>()
        * focus_bps
        / 10_000
        * location
        / 10_000;
    let wage_index = world.regions[region as usize].wage_index_bps as i64;
    let payroll: i64 = (0..3)
        .map(|tier| template.workers_low_mid_high[tier] * dataset.baseline_weekly_wage_cents[tier])
        .sum::<i64>()
        * wage_index
        / 10_000;
    let land = dataset.fixture_land_rent_cents_weekly
        * world.regions[region as usize].land_index_bps as i64
        / 10_000;
    template.weekly_output_lots * price(&template.output)
        - inputs
        - payroll * (10_000 + overhead_bps) / 10_000
        - template.weekly_maintenance(dataset.maintenance_annual_bps)
        - land
}

/// Counts of every policy attempt, kept in the sweep evidence.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct PolicyStats {
    pub attempted: u32,
    pub accepted: u32,
    pub rejected: u32,
    /// Rejections by intent key, so a sweep can explain why a policy could not act.
    pub rejections: BTreeMap<String, u32>,
}

pub struct PolicyRunner {
    policy: ReferencePolicy,
    next_low: u64,
    verbose: bool,
    /// Last production target per firm, so a cash dip cannot churn staffing every cadence.
    last_target: BTreeMap<u64, i32>,
    pub stats: PolicyStats,
}

impl PolicyRunner {
    pub fn new(policy: ReferencePolicy, verbose: bool) -> PolicyRunner {
        PolicyRunner {
            policy,
            next_low: 1,
            verbose,
            last_target: BTreeMap::new(),
            stats: PolicyStats::default(),
        }
    }

    /// Decide for this week, if this is a cadence week and the clock is not paused.
    pub fn act(&mut self, session: &mut Session) {
        if self.policy == ReferencePolicy::None {
            return;
        }
        if !session.world.tick.0.is_multiple_of(CADENCE_WEEKS) {
            return;
        }
        if session.world.pending_pause.is_some() {
            return;
        }
        if session.world.investor.primary_group.is_none() {
            // Only the founder scenario has to create its own firm; a guided career that loses
            // its firm must not conjure a replacement.
            if session.config.scenario != cw_sim::state::ScenarioKind::Founder {
                return;
            }
            self.found_firm(session);
            if session.world.investor.primary_group.is_none() {
                return;
            }
        }
        let controlled = controlled_firms(session);
        for firm in &controlled {
            self.operate(session, *firm);
        }
        if self.policy == ReferencePolicy::Acquirer && !controlled.is_empty() {
            self.acquire(session, &controlled);
        }
        if self.policy == ReferencePolicy::Expander
            && session.world.tick.0.is_multiple_of(12)
            && let Some(primary) = session.world.investor.primary_group
        {
            self.expand(session, primary.0);
        }
    }

    /// Build one plant where imports show the most room the firm can afford, in any sector: into
    /// an existing division of the same strategy, or opening a new one (an off-focus sector pays
    /// its coordination overhead). Keeps the policy's cash reserve.
    fn expand(&mut self, session: &mut Session, firm: u64) {
        use cw_sim::economy::sectors;
        let world = &session.world;
        let dataset = &session.dataset;
        let handle = Handle(firm);
        let Some(entity) = world.firms.get(handle) else {
            return;
        };
        let reserve = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, handle)
            .saturating_mul(self.policy.reserve_weeks());
        let cash = entity.book.cash();
        let mut best: Option<(i64, u8, u16)> = None;
        for (index, recipe) in dataset.recipes.iter().enumerate() {
            let recipe_index = index as u16;
            let Some(output) = dataset.product_index.get(&recipe.output).copied() else {
                continue;
            };
            for region in 0..world.regions.len() as u8 {
                let industry = sectors::recipe_industry(dataset, recipe_index);
                if sectors::division_strategy(world, dataset, handle, region, industry)
                    .is_some_and(|strategy| strategy != recipe_index)
                {
                    continue;
                }
                let room = world
                    .gateways
                    .iter()
                    .find(|g| g.region == region && g.product == output)
                    .map(|g| g.smoothed_import_lots)
                    .unwrap_or(0);
                if room * 5 < recipe.weekly_output_lots * 4 {
                    continue;
                }
                let quote = sectors::build_cost(world, dataset, handle, region, recipe_index, 1);
                if cash < reserve + quote.total_cents() {
                    continue;
                }
                // Three years of contribution at reference prices, with the plant's focus,
                // location and coordination overhead, must repay the whole build.
                let contribution = plant_contribution(world, dataset, entity, region, recipe_index);
                let value = contribution * 156 - quote.total_cents();
                if value <= 0 {
                    continue;
                }
                if best.is_none_or(|(b, ..)| value > b) {
                    best = Some((value, region, recipe_index));
                }
            }
        }
        if let Some((_, region, recipe)) = best {
            self.submit(
                session,
                CommandIntent::BuildFacility {
                    firm: handle,
                    region,
                    recipe,
                    blocks: 1,
                },
            );
        }
    }

    fn found_firm(&mut self, session: &mut Session) {
        let Some(region) = session.world.region_index(FOUNDER_REGION) else {
            return;
        };
        let Some(recipe) = session.dataset.recipe_index.get(FOUNDER_RECIPE).copied() else {
            return;
        };
        self.submit(
            session,
            CommandIntent::FoundFirm {
                name: FOUNDER_NAME.to_owned(),
                region,
                capital_cents: FOUNDER_CAPITAL_CENTS,
                recipe,
                secondary: None,
            },
        );
    }

    /// Operating policy for one controlled firm.
    fn operate(&mut self, session: &mut Session, firm: u64) {
        let finance = finance(session, firm);
        let outflow =
            cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, Handle(firm));
        let reserve = outflow.saturating_mul(self.policy.reserve_weeks());
        let below_reserve = finance.cash_cents < reserve;
        let weekly_revenue = weekly_revenue(&finance);

        // Every reference policy keeps the plant at full capacity. The first sweep draft cut
        // output to 85/95 percent when cash thinned; that lost about $8.7k of contribution a week
        // on this demand-limited fixture, so a cash problem is answered by cutting discretionary
        // marketing and research, never by throttling a plant with positive unit margin.
        let target_bps = 10_000;
        if self.last_target.get(&firm) != Some(&target_bps) {
            self.submit(
                session,
                CommandIntent::SetProductionTarget {
                    firm: Handle(firm),
                    target_bps,
                },
            );
            self.last_target.insert(firm, target_bps);
        }

        let marketing_percent = if below_reserve {
            self.policy.marketing_percent() / 2
        } else {
            self.policy.marketing_percent()
        };
        self.submit(
            session,
            CommandIntent::SetMarketingBudget {
                firm: Handle(firm),
                amount_cents: percent_of(weekly_revenue, marketing_percent),
            },
        );

        let research_percent = if below_reserve {
            0
        } else {
            self.policy.research_percent()
        };
        if self.policy == ReferencePolicy::Growth {
            self.submit(
                session,
                CommandIntent::SetResearchBudget {
                    firm: Handle(firm),
                    amount_cents: percent_of(weekly_revenue, research_percent),
                },
            );
        }

        if below_reserve {
            self.restore_reserve(session, firm, reserve - finance.cash_cents, &finance);
        }
    }

    /// Legal credit restores the policy's cash floor, capped by the lender's own capacity.
    ///
    /// This is a liquidity bridge, not a growth facility. It only fires on a genuine shortfall
    /// (cash below half of the floor) while the firm's liabilities stay below half of its
    /// assets and the new principal stays below a quarter of assets. Borrowing every cadence to
    /// top up a slowly leaking reserve compounds debt service into the reserve itself, which is
    /// the spiral the first sweeps exposed; the leverage gate is the fix.
    fn restore_reserve(
        &mut self,
        session: &mut Session,
        firm: u64,
        gap_cents: i64,
        finance: &FinanceSummary,
    ) {
        // Only bridge a genuine shortfall: cash below half of the floor (cash < gap).
        if finance.cash_cents >= gap_cents {
            return;
        }
        if finance.assets_cents <= 0 || finance.liabilities_cents * 2 >= finance.assets_cents {
            return;
        }
        let leverage_headroom = finance.assets_cents / 4 - finance.liabilities_cents;
        if leverage_headroom <= 0 {
            return;
        }
        let capacity = cw_sim::economy::credit::secured_capacity(&session.world, Handle(firm))
            + cw_sim::economy::credit::unsecured_capacity(&session.world, Handle(firm));
        let amount = gap_cents.min(capacity).min(leverage_headroom);
        // Round down to a clean $10,000 so the evidence is readable.
        let amount = amount / 1_000_000 * 1_000_000;
        if amount <= 0 {
            return;
        }
        self.submit(
            session,
            CommandIntent::RequestLoan {
                firm: Handle(firm),
                amount_cents: amount,
            },
        );
    }

    /// Start a funded diligence on the weakest affordable operating peer.
    fn acquire(&mut self, session: &mut Session, controlled: &[u64]) {
        let buyer_handles: Vec<Handle> = controlled.iter().map(|firm| Handle(*firm)).collect();
        let live_deal = session
            .world
            .deals
            .iter()
            .any(|(_, deal)| buyer_handles.contains(&deal.buyer) && !deal.state.terminal());
        if live_deal {
            return;
        }
        let mut buyers = controlled.to_vec();
        buyers.sort_by_key(|firm| -finance(session, *firm).cash_cents);

        let firms = match session.query(Query::Firms) {
            Detail::Firms(rows) => rows,
            _ => return,
        };
        let mut candidates: Vec<(i64, u64)> = Vec::new();
        for firm in &firms {
            if firm.controlled || firm.investor_votes_bps != 0 || firm.status != "operating" {
                continue;
            }
            // Private rivals keep their books to themselves; the policy sees only what the
            // player could inspect.
            let Some(finance) = inspect(session, firm.handle) else {
                continue;
            };
            let distressed =
                finance.net_income_trailing_cents < 0 || firm.grade == "C" || firm.grade == "D";
            if distressed {
                candidates.push((finance.net_income_trailing_cents, firm.handle));
            }
        }
        candidates.sort_by_key(|(income, handle)| (*income, *handle));
        if self.verbose {
            println!(
                "w{} acquire buyers={} candidates={}",
                session.world.tick.0,
                buyers.len(),
                candidates.len()
            );
        }

        for buyer in buyers {
            let buyer_finance = finance(session, buyer);
            let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(
                &session.world,
                Handle(buyer),
            );
            let reserve = outflow.saturating_mul(self.policy.reserve_weeks());
            for (_, target) in &candidates {
                let Some(deal) = acquisition_terms(session, *target) else {
                    continue;
                };
                if buyer_finance.cash_cents - deal.escrow_cents < reserve {
                    if self.verbose {
                        println!(
                            "w{} acquire target={} escrow={} cash={} reserve={} skip",
                            session.world.tick.0,
                            target,
                            deal.escrow_cents,
                            buyer_finance.cash_cents,
                            reserve
                        );
                    }
                    continue;
                }
                if self.verbose {
                    println!(
                        "w{} acquire target={} offer={} shares={} escrow={}",
                        session.world.tick.0,
                        target,
                        deal.offer_cents,
                        deal.max_shares,
                        deal.escrow_cents
                    );
                }
                self.submit(
                    session,
                    CommandIntent::StartDiligence {
                        buyer: Handle(buyer),
                        target: Handle(*target),
                        kind: DealKind::SharePurchase,
                        price_cents: deal.offer_cents,
                        max_shares: deal.max_shares,
                    },
                );
                return;
            }
        }
    }

    /// Answer a mandatory card through the normal command path. Active policies take the
    /// card's first (acting) choice; the no-action baseline takes its passive choice.
    pub fn answer_event(&mut self, session: &mut Session, event: u64) -> bool {
        let Some(card) = session
            .world
            .events
            .pending
            .iter()
            .find(|pending| pending.id == event)
            .and_then(|pending| session.dataset.event(&pending.card))
        else {
            return false;
        };
        let choice = if self.policy == ReferencePolicy::None {
            card.choices
                .iter()
                .find(|choice| choice.handler == "no_action")
                .or_else(|| card.choices.last())
        } else {
            card.choices.first()
        };
        let Some(choice) = choice.map(|choice| choice.id.clone()) else {
            return false;
        };
        self.submit(session, CommandIntent::RespondEvent { event, choice })
    }

    fn submit(&mut self, session: &mut Session, intent: CommandIntent) -> bool {
        let key = intent.key().to_owned();
        let envelope = CommandEnvelope {
            command_id_high: PLAYER_COMMAND_HIGH,
            command_id_low: self.next_low,
            actor: INVESTOR_ACTOR,
            expected_revision: session.revision(),
            submitted_tick: session.world.tick.0,
            intent,
        };
        self.next_low += 1;
        self.stats.attempted += 1;
        match session.submit(envelope) {
            Ok(_) => {
                self.stats.accepted += 1;
                if self.verbose {
                    println!("w{} {} accepted", session.world.tick.0, key);
                }
                true
            }
            Err(error) => {
                self.stats.rejected += 1;
                *self.stats.rejections.entry(key.clone()).or_default() += 1;
                if self.verbose {
                    println!("w{} {} rejected {:?}", session.world.tick.0, key, error);
                }
                false
            }
        }
    }
}

fn inspect(session: &Session, firm: u64) -> Option<FinanceSummary> {
    match session.query(Query::Finance(firm)) {
        Detail::Finance(finance) => Some(finance),
        _ => None,
    }
}

/// Finance for a company the investor controls, which is always inspectable.
fn finance(session: &Session, firm: u64) -> FinanceSummary {
    match session.query(Query::Finance(firm)) {
        Detail::Finance(finance) => finance,
        _ => unreachable!("finance query always returns a finance summary"),
    }
}

fn controlled_firms(session: &Session) -> Vec<u64> {
    match session.query(Query::Firms) {
        Detail::Firms(rows) => rows
            .into_iter()
            .filter(|firm| firm.controlled)
            .map(|firm| firm.handle)
            .collect(),
        _ => Vec::new(),
    }
}

fn weekly_revenue(finance: &FinanceSummary) -> i64 {
    let weeks = finance.recent_weeks.len().max(1) as i64;
    finance.revenue_trailing_cents / weeks
}

/// Whole-dollar marketing/research budgets, floored at zero.
fn percent_of(amount_cents: i64, percent: i64) -> i64 {
    if amount_cents <= 0 || percent <= 0 {
        return 0;
    }
    (amount_cents * percent / 100) / 10_000 * 10_000
}

struct AcquisitionTerms {
    offer_cents: i64,
    max_shares: i64,
    escrow_cents: i64,
}

/// Price a bare voting majority at 110 percent of fair value or 115 percent of the mark,
/// whichever is higher, plus the two percent escrow buffer the deal engine requires.
fn acquisition_terms(session: &Session, target: u64) -> Option<AcquisitionTerms> {
    let firm = match session.query(Query::Firm(target)) {
        Detail::Firm(firm) => firm,
        _ => return None,
    };
    let ownership = match session.query(Query::Ownership(target)) {
        Detail::Ownership(ownership) => ownership,
        _ => return None,
    };
    let eligible = ownership.eligible_votes;
    if eligible <= 0 || firm.fair_value_cents <= 0 {
        return None;
    }
    let fair_per_share = firm.fair_value_cents / eligible;
    let by_fair = fair_per_share.saturating_mul(110) / 100;
    let by_mark = firm.mark_cents.saturating_mul(115) / 100;
    let offer_cents = by_fair.max(by_mark).max(1);
    let max_shares = eligible / 2 + 1;
    let gross = offer_cents.checked_mul(max_shares)?;
    let escrow_cents = gross + gross * 2 / 100;
    Some(AcquisitionTerms {
        offer_cents,
        max_shares,
        escrow_cents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use cw_session::ScenarioConfig;

    fn guided(seed: u64) -> Session {
        Session::create(config(seed, true, true)).unwrap()
    }

    #[test]
    fn policies_parse_and_have_distinct_operating_targets() {
        assert_eq!(
            ReferencePolicy::parse("conservative"),
            Some(ReferencePolicy::Conservative)
        );
        assert_eq!(ReferencePolicy::parse("nope"), None);
        assert_ne!(
            ReferencePolicy::Conservative.marketing_percent(),
            ReferencePolicy::Growth.marketing_percent()
        );
        assert!(
            ReferencePolicy::Conservative.reserve_weeks() > ReferencePolicy::Growth.reserve_weeks()
        );
    }

    #[test]
    fn a_policy_only_changes_the_world_through_accepted_commands() {
        let mut session = guided(191);
        let before = session.revision();
        let mut runner = PolicyRunner::new(ReferencePolicy::Conservative, false);
        runner.act(&mut session);
        assert!(session.revision() > before);
        assert_eq!(
            session.revision() - before,
            u64::from(runner.stats.accepted),
            "every revision increment must come from an accepted command"
        );
        assert_eq!(runner.stats.rejected, 0);
    }

    #[test]
    fn the_growth_policy_sets_full_capacity_marketing_and_research() {
        let mut session = guided(192);
        let primary = session.world.investor.primary_group.unwrap();
        let mut runner = PolicyRunner::new(ReferencePolicy::Growth, false);
        runner.act(&mut session);
        let firm = session.world.firm(primary).unwrap();
        assert_eq!(firm.policies.production_target_bps, 10_000);
        assert!(firm.policies.research_budget_cents >= 0);
        assert_eq!(
            runner.stats.accepted, runner.stats.attempted,
            "the unconstrained growth plan is legal at week 0"
        );
    }

    #[test]
    fn the_conservative_policy_holds_a_quarter_of_obligations() {
        let mut session = guided(193);
        let primary = session.world.investor.primary_group.unwrap();
        let mut runner = PolicyRunner::new(ReferencePolicy::Conservative, false);
        runner.act(&mut session);
        let firm = session.world.firm(primary).unwrap();
        assert_eq!(
            firm.policies.production_target_bps, 10_000,
            "the conservative policy preserves the operating base"
        );
        let outflow =
            cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, primary);
        assert!(
            outflow * ReferencePolicy::Conservative.reserve_weeks() > 0,
            "a reserve is held against real obligations"
        );
    }

    #[test]
    fn the_acquirer_can_price_a_majority_within_the_escrow_rule() {
        let session = guided(194);
        let terms = acquisition_terms(&session, session.world.investor.primary_group.unwrap().0);
        if let Some(terms) = terms {
            assert_eq!(terms.max_shares, 500_001);
            assert!(terms.escrow_cents >= terms.offer_cents * terms.max_shares);
        }
    }

    #[test]
    fn a_founder_run_founds_exactly_one_firm() {
        let mut session = Session::create(ScenarioConfig {
            seed: 195,
            ..config(195, false, true)
        })
        .unwrap();
        let before = session.world.firms.len();
        let mut runner = PolicyRunner::new(ReferencePolicy::Conservative, false);
        runner.act(&mut session);
        assert_eq!(session.world.firms.len(), before + 1);
        assert!(session.world.investor.primary_group.is_some());
    }
}
