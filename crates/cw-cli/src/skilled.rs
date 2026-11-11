//! The `skilled` play style: what a careful human could do with the commands a player has.
//!
//! It reads only what the session shows a player: the sector sheets in the dataset, the build
//! quote, the market's import room, the CFO advice and the weekly brief. It never writes world
//! state; every action is an ordinary command through the validator.
//!
//! What it does, in the order a player would think of it:
//!
//! 1. Founds the scenario's own starting company with all the personal cash the owner can
//!    spare (a human would not leave cash idle outside the firm).
//! 2. Runs every plant flat out, spends four percent of sales on marketing (a division's only
//!    defence against a market raid is its marketing budget, see `sectors::raid_quote`), applies
//!    the CFO's fixes when the firm loses money and the brief's lane reservations.
//! 3. Every four weeks puts spare cash into the best next plant: the one whose output imports
//!    already buy (room in the market), that repays its quote inside three years on the sector
//!    sheet, keeping eight weeks of mandatory outflow in hand.
//! 4. Borrows against plant (secured and unsecured capacity) while liabilities stay under 45
//!    percent of assets.
//! 5. Answers cards as an owner would: refuses tender offers (the engine never sells a player's
//!    shares without that answer), defends against a hostile stake, declines paid extras.
//!
//! Earlier drafts that picked sectors by sheet return alone, tried each new sector on probation,
//! or built supplier plants lost money (see the 2.0 skilled report): the sheet ignores whether
//! anyone sells the plant its inputs or buys its output. The import-room test is the market
//! evidence a player can see.

use super::*;

/// Weeks a new company may build before it has to be earning an operating profit.
const SETTLING_WEEKS: u32 = 26;
/// Marketing as a percent of weekly sales.
const MARKETING_PERCENT: i64 = 4;
/// Loan only while liabilities stay under this share of assets, in percent.
const MAX_LEVERAGE_PCT: i64 = 45;
/// Size of one loan request, in cents ($500,000).
const LOAN_STEP_CENTS: i64 = 50_000_000;
/// Cash the founder keeps personally, in cents ($100,000).
const FOUNDER_KEEPS_CENTS: i64 = 10_000_000;

#[derive(Default)]
pub struct SkilledState {
    /// Weeks the weekly brief crashed the engine (a mandate whose due week precedes its set week
    /// makes `chapters::progress` overflow); the harness skips the brief those weeks.
    pub brief_panics: u32,
}

/// The weekly brief, or None when the engine panics building it.
fn safe_brief(session: &Session, firm: u64) -> Option<cw_session::brief::Brief> {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cw_session::brief::weekly_brief(&session.world, &session.dataset, firm)
    }));
    std::panic::set_hook(previous);
    result.ok()
}

impl PolicyRunner {
    /// Found the scenario's starting company with all the spare personal cash.
    pub(super) fn found_skilled(&mut self, session: &mut Session) {
        let (Some(region), Some(recipe)) = (
            session.world.region_index(&session.dataset.player_region),
            session
                .dataset
                .recipe_index
                .get(&session.dataset.player_recipe)
                .copied(),
        ) else {
            return self.found_firm(session);
        };
        let fee = cw_data::Difficulty::Standard.formation_fee_cents();
        let capital = (session.world.investor.cash_cents - fee - FOUNDER_KEEPS_CENTS)
            .max(FOUNDER_CAPITAL_CENTS);
        self.submit(
            session,
            CommandIntent::FoundFirm {
                name: FOUNDER_NAME.to_owned(),
                region,
                capital_cents: capital,
                recipe,
                secondary: None,
            },
        );
    }

    /// One cadence of the skilled player.
    pub(super) fn act_skilled(&mut self, session: &mut Session) {
        let controlled = controlled_firms(session);
        for firm in &controlled {
            self.skilled_operate(session, *firm);
        }
        if let Some(primary) = session.world.investor.primary_group {
            let fin = finance(session, primary.0);
            self.skilled_borrow(session, primary.0, &fin);
            // After the first half year, build only while the firm earns an operating profit.
            if session.world.tick.0 < SETTLING_WEEKS || fin.ebitda_trailing_cents > 0 {
                self.expand(session, primary.0, false);
            }
        }
        if self.policy == ReferencePolicy::Skilled {
            self.skilled_acquire(session, &controlled);
        }
    }

    fn skilled_operate(&mut self, session: &mut Session, firm: u64) {
        let handle = Handle(firm);
        let finance = finance(session, firm);
        if self.last_target.get(&firm) != Some(&10_000) {
            self.submit(
                session,
                CommandIntent::SetProductionTarget {
                    firm: handle,
                    target_bps: 10_000,
                    lock: false,
                },
            );
            self.last_target.insert(firm, 10_000);
        }
        self.submit(
            session,
            CommandIntent::SetMarketingBudget {
                firm: handle,
                amount_cents: percent_of(weekly_revenue(&finance), MARKETING_PERCENT),
            },
        );
        // The CFO speaks only when the last four weeks lost money; do what it says.
        for item in cw_session::cfo::advice(&session.world, &session.dataset, firm) {
            for command in &item.commands {
                if let Ok(intent) = serde_json::from_str::<CommandIntent>(command) {
                    self.submit(session, intent);
                }
            }
        }
        let Some(brief) = safe_brief(session, firm) else {
            self.skilled.brief_panics += 1;
            return;
        };
        for mv in brief.moves.iter().filter(|mv| mv.key == "lane") {
            for command in &mv.commands {
                if let Ok(intent) = serde_json::from_str::<CommandIntent>(command) {
                    self.submit(session, intent);
                }
            }
        }
    }

    /// Borrow one step against plant when leverage is modest and the firm is in good standing.
    fn skilled_borrow(&mut self, session: &mut Session, firm: u64, fin: &FinanceSummary) {
        // A careful owner does not borrow while the firm is losing money.
        if fin.assets_cents <= 0
            || fin.net_income_trailing_cents <= 0
            || fin.ebitda_trailing_cents <= 0
        {
            return;
        }
        let capacity = cw_sim::economy::credit::secured_capacity(&session.world, Handle(firm))
            + cw_sim::economy::credit::unsecured_capacity(&session.world, Handle(firm));
        // Debt stays under 45 percent of assets and under two years of trailing operating profit.
        let earnings_cap = fin.ebitda_trailing_cents.saturating_mul(4 * 2) - fin.liabilities_cents;
        let headroom =
            (fin.assets_cents * MAX_LEVERAGE_PCT / 100 - fin.liabilities_cents).min(earnings_cap);
        let amount = LOAN_STEP_CENTS.min(capacity).min(headroom) / 1_000_000 * 1_000_000;
        if amount > 0 {
            self.submit(
                session,
                CommandIntent::RequestLoan {
                    firm: Handle(firm),
                    amount_cents: amount,
                },
            );
        }
    }

    /// Put idle cash into the best earner a rival offers: the highest trailing profit against
    /// fair value among operating firms the player can inspect, bought as a bare majority at 110
    /// percent of fair value or 115 percent of the mark, when the escrow leaves a reserve intact.
    fn skilled_acquire(&mut self, session: &mut Session, controlled: &[u64]) {
        let buyer_handles: Vec<Handle> = controlled.iter().map(|firm| Handle(*firm)).collect();
        if session
            .world
            .deals
            .iter()
            .any(|(_, deal)| buyer_handles.contains(&deal.buyer) && !deal.state.terminal())
        {
            return;
        }
        let Some(buyer) = controlled
            .iter()
            .copied()
            .max_by_key(|firm| finance(session, *firm).cash_cents)
        else {
            return;
        };
        let buyer_finance = finance(session, buyer);
        let reserve =
            cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, Handle(buyer))
                .saturating_mul(10)
                .max(300_000_000);
        let firms = match session.query(Query::Firms) {
            Detail::Firms(rows) => rows,
            _ => return,
        };
        let mut candidates: Vec<(i64, u64)> = Vec::new();
        for firm in &firms {
            if firm.controlled || firm.investor_votes_bps != 0 || firm.status != "operating" {
                continue;
            }
            let Some(fin) = inspect(session, firm.handle) else {
                continue;
            };
            if firm.fair_value_cents <= 0 || fin.net_income_trailing_cents <= 0 {
                continue;
            }
            // Annualised trailing profit over fair value, in basis points.
            let yield_bps =
                fin.net_income_trailing_cents.saturating_mul(4 * 10_000) / firm.fair_value_cents;
            if yield_bps >= 800 {
                candidates.push((yield_bps, firm.handle));
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, target) in candidates {
            let Some(deal) = acquisition_terms(session, target) else {
                continue;
            };
            if buyer_finance.cash_cents - deal.escrow_cents < reserve {
                continue;
            }
            self.submit(
                session,
                CommandIntent::StartDiligence {
                    buyer: Handle(buyer),
                    target: Handle(target),
                    kind: DealKind::SharePurchase,
                    price_cents: deal.offer_cents,
                    max_shares: deal.max_shares,
                },
            );
            return;
        }
    }

    /// The owner's answer to a card.
    pub(super) fn skilled_choice(card: &str) -> Option<&'static str> {
        Some(match card {
            "tender_offer" => "refuse",
            "fuel_spike" => "policy",
            "training_grant" => "decline",
            "tech_opportunity" => "normal",
            "demand_boom" => "hold",
            "hostile_stake" => "diligence",
            "bank_surge" => "liquidity",
            _ => return None,
        })
    }
}

/// One place to put a plant, priced from what the build dialog and the market show.
#[derive(Clone, Debug)]
pub struct Opportunity {
    pub recipe: u16,
    pub region: u8,
    pub opens_division: bool,
    pub cost_cents: i64,
    pub weekly_cents: i64,
    /// Share of the plant's output value that imports into the region already buy, in percent.
    pub room_pct: i64,
    /// Lots of the output sold in the region last week, all sellers.
    pub sales_lots: i64,
    pub plant_lots: i64,
}

pub fn scan(session: &Session, firm: u64) -> Vec<Opportunity> {
    use cw_sim::economy::sectors;
    let world = &session.world;
    let dataset = &session.dataset;
    let handle = Handle(firm);
    let Some(entity) = world.firms.get(handle) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (index, recipe) in dataset.recipes.iter().enumerate() {
        let recipe_index = index as u16;
        let Some(output) = dataset.product_index.get(&recipe.output).copied() else {
            continue;
        };
        let industry = sectors::recipe_industry(dataset, recipe_index);
        for region in 0..world.regions.len() as u8 {
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
            let room_pct = if recipe.weekly_output_lots > 0 {
                room * 100 / recipe.weekly_output_lots
            } else {
                0
            };
            let sales_lots = match session.query(Query::Market(region, output)) {
                Detail::Market(line) => line.last_week_sales_lots,
                _ => 0,
            };
            let quote = sectors::build_cost(world, dataset, handle, region, recipe_index, 1);
            let weekly = plant_contribution(world, dataset, entity, region, recipe_index);
            out.push(Opportunity {
                recipe: recipe_index,
                region,
                opens_division: quote.opens_division,
                cost_cents: quote.total_cents(),
                weekly_cents: weekly,
                room_pct,
                sales_lots,
                plant_lots: recipe.weekly_output_lots,
            });
        }
    }
    out
}

pub fn explore(arguments: &[String]) {
    let scenario =
        crate::flag(arguments, "--scenario-id").unwrap_or_else(|| "great-lakes-2000".into());
    let seed: u64 = crate::flag(arguments, "--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let mut session = Session::create(crate::arc::arc_config_pub(seed, &scenario)).unwrap();
    let mut runner = PolicyRunner::new(ReferencePolicy::Passive, false);
    runner.found_firm(&mut session);
    let firm = session.world.investor.primary_group.unwrap().0;
    let mut opps = scan(&session, firm);
    opps.sort_by_key(|o| -(o.weekly_cents * 10_000 / o.cost_cents.max(1)));
    println!("{} candidates", opps.len());
    if arguments.iter().any(|a| a == "--all") {
        for o in &opps {
            let r = &session.dataset.recipes[o.recipe as usize];
            println!(
                "ALL {} {} {} {} {} {} {}",
                r.id,
                session.dataset.regions[o.region as usize].code,
                o.cost_cents,
                o.weekly_cents,
                o.room_pct,
                o.sales_lots,
                o.plant_lots
            );
        }
        return;
    }
    for o in opps.iter().take(40) {
        let r = &session.dataset.recipes[o.recipe as usize];
        println!(
            "{:<28} {:<8} open={} cost={:>9} weekly={:>8} annual_roi={:>4}% room={:>4}%",
            r.id,
            session.dataset.regions[o.region as usize].code,
            o.opens_division as u8,
            o.cost_cents / 100,
            o.weekly_cents / 100,
            o.weekly_cents * 52 * 100 / o.cost_cents.max(1),
            o.room_pct
        );
    }
    let pos = opps.iter().filter(|o| o.weekly_cents > 0).count();
    println!("positive contribution: {pos}");
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub recipe: u16,
    pub region: u8,
    pub accepted: bool,
    pub cost_cents: i64,
    pub ebitda_week_cents: i64,
    pub revenue_week_cents: i64,
    pub raided: bool,
}

/// Found a company straight into one plant and measure what it really earns a week.
pub fn probe_one(
    seed: u64,
    scenario: &str,
    recipe: u16,
    region: u8,
    capital: i64,
    weeks: u32,
) -> Probe {
    let mut session = Session::create(crate::arc::arc_config_pub(seed, scenario)).unwrap();
    let mut probe = Probe {
        recipe,
        region,
        accepted: false,
        cost_cents: 0,
        ebitda_week_cents: 0,
        revenue_week_cents: 0,
        raided: false,
    };
    let mut runner = PolicyRunner::new(ReferencePolicy::Passive, false);
    let ok = runner.submit(
        &mut session,
        CommandIntent::FoundFirm {
            name: "Probe".to_owned(),
            region,
            capital_cents: capital,
            recipe,
            secondary: None,
        },
    );
    if !ok {
        return probe;
    }
    probe.accepted = true;
    let firm = session.world.investor.primary_group.unwrap();
    probe.cost_cents = session
        .world
        .firm(firm)
        .map(|f| {
            f.facilities
                .iter()
                .filter_map(|h| session.world.facilities.get(*h))
                .map(|p| p.capex_cents)
                .sum()
        })
        .unwrap_or(0);
    runner.submit(
        &mut session,
        CommandIntent::SetProductionTarget {
            firm,
            target_bps: 10_000,
            lock: false,
        },
    );
    crate::drive_with(
        &mut session,
        weeks,
        &mut PolicyRunner::new(ReferencePolicy::None, false),
        &mut |_, _| {},
    );
    if let Some(f) = session.world.firm(firm) {
        let recent = f.book.recent_weeks();
        let n = recent.len().max(1) as i64;
        probe.ebitda_week_cents = recent.iter().map(|w| w.ebitda()).sum::<i64>() / n;
        probe.revenue_week_cents = recent.iter().map(|w| w.revenue).sum::<i64>() / n;
        probe.raided = session
            .world
            .raids
            .iter()
            .any(|r| r.defender == firm && r.success);
    }
    probe
}

pub fn probe_all(arguments: &[String]) {
    let scenario =
        crate::flag(arguments, "--scenario-id").unwrap_or_else(|| "great-lakes-2000".into());
    let seed: u64 = crate::flag(arguments, "--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let weeks: u32 = crate::flag(arguments, "--weeks")
        .and_then(|v| v.parse().ok())
        .unwrap_or(70);
    let capital: i64 = crate::flag(arguments, "--capital")
        .and_then(|v| v.parse().ok())
        .unwrap_or(300_000_000);
    let session = Session::create(crate::arc::arc_config_pub(seed, &scenario)).unwrap();
    let recipes = session.dataset.recipes.len() as u16;
    let regions = session.world.regions.len() as u8;
    let region_list: Vec<u8> = match crate::flag(arguments, "--regions") {
        Some(list) => list.split(',').filter_map(|x| x.parse().ok()).collect(),
        None => (0..regions).collect(),
    };
    let jobs: Vec<(u16, u8)> = (0..recipes)
        .flat_map(|r| region_list.iter().map(move |g| (r, *g)))
        .collect();
    let threads = 8usize;
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for t in 0..threads {
            let jobs = &jobs;
            let results = &results;
            let scenario = scenario.clone();
            scope.spawn(move || {
                for (i, (r, g)) in jobs.iter().enumerate() {
                    if i % threads != t {
                        continue;
                    }
                    let p = probe_one(seed, &scenario, *r, *g, capital, weeks);
                    results.lock().unwrap().push(p);
                }
            });
        }
    });
    let mut all = results.into_inner().unwrap();
    all.retain(|p| p.accepted);
    all.sort_by_key(|p| -(p.ebitda_week_cents * 10_000 / p.cost_cents.max(1)));
    println!("recipe region cost_k ebitda_wk revenue_wk ann_roi% raided");
    for p in &all {
        let r = &session.dataset.recipes[p.recipe as usize];
        println!(
            "{:<28} {:<7} {:>8} {:>8} {:>8} {:>5} {}",
            r.id,
            session.dataset.regions[p.region as usize].code,
            p.cost_cents / 100_000,
            p.ebitda_week_cents / 100,
            p.revenue_week_cents / 100,
            p.ebitda_week_cents * 52 * 100 / p.cost_cents.max(1),
            p.raided as u8
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use cw_session::ScenarioConfig;
    use cw_sim::state::HolderId;

    fn founder(seed: u64) -> Session {
        Session::create(ScenarioConfig {
            seed,
            ..config(seed, false, false)
        })
        .unwrap()
    }

    #[test]
    fn skilled_refuses_a_tender_offer_and_never_sells_its_own_shares() {
        assert_eq!(PolicyRunner::skilled_choice("tender_offer"), Some("refuse"));
        let mut session = founder(301);
        let mut runner = PolicyRunner::new(ReferencePolicy::Skilled, false);
        for _ in 0..200 {
            session.advance(1);
            runner.act(&mut session);
        }
        assert!(
            !runner.stats.rejections.contains_key("respond_event"),
            "{:?}",
            runner.stats
        );
        let primary = session.world.investor.primary_group.expect("a company");
        assert_eq!(
            session
                .world
                .firm(primary)
                .unwrap()
                .shares
                .vote_bps(HolderId::Investor),
            10_000
        );
    }

    #[test]
    fn skilled_founds_with_all_the_spare_personal_cash() {
        let mut session = founder(302);
        let before = session.world.investor.cash_cents;
        let mut runner = PolicyRunner::new(ReferencePolicy::Skilled, false);
        runner.act(&mut session);
        let primary = session.world.investor.primary_group.expect("founded");
        let capital = session.world.firm(primary).unwrap().book.cash();
        assert!(
            capital > FOUNDER_CAPITAL_CENTS && capital < before,
            "capital {capital} of {before}"
        );
        assert!(session.world.investor.cash_cents >= FOUNDER_KEEPS_CENTS - 1);
    }

    #[test]
    fn the_noise_policy_changes_no_world_state_but_one_random_draw() {
        let mut quiet = founder(303);
        let mut noisy = founder(303);
        let mut none = PolicyRunner::new(ReferencePolicy::None, false);
        let mut noise = PolicyRunner::new(ReferencePolicy::Noise, false);
        for _ in 0..8 {
            none.act(&mut quiet);
            noise.act(&mut noisy);
            quiet.advance(1);
            noisy.advance(1);
        }
        assert_eq!(noise.stats.attempted, 0, "noise submits no command");
        assert_eq!(quiet.world.firms.len(), noisy.world.firms.len());
        assert_eq!(
            quiet.world.investor.cash_cents, noisy.world.investor.cash_cents,
            "the player's money is untouched"
        );
        assert_eq!(noise.stats.accepted, 0);
    }
}
