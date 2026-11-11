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
const MAX_LEVERAGE_PCT: i64 = 40;
/// Weeks a new plant trades before it is judged.
const PROVING_WEEKS: u32 = 13;
/// Weeks of mandatory outflow kept in cash after any build.
const RESERVE_WEEKS: i64 = 26;
/// A plant must repay its quote within this many weeks of its (trust-discounted) profit.
const MAX_PAYBACK_WEEKS: i64 = 104;
/// Cash the founder keeps personally, in cents ($100,000).
const FOUNDER_KEEPS_CENTS: i64 = 10_000_000;

/// Score, recipe, region, cost, discounted weekly profit, industry, blocks, from the sheet.
type Candidate = (i64, u16, u8, i64, i64, u8, i64, bool);

/// The plant the careful script last ordered and is waiting to judge.
#[derive(Clone, Debug)]
pub(super) struct SkilledBuild {
    plant: Handle,
    key: (u8, u8),
    recipe: u16,
    region: u8,
    /// Chosen from the sheet and import room test rather than the opening signal.
    from_sheet: bool,
    /// Weekly profit it was expected to add, after the trust discount.
    expected: i64,
    /// What its division earned a week before the order.
    base_profit: i64,
    capex: i64,
}

/// What the careful script remembers between cadences.
#[derive(Clone, Debug)]
pub(super) struct SkilledState {
    pending: Option<SkilledBuild>,
    /// How far to believe the engine's opening signal, in basis points, from its own record.
    trust_bps: i64,
    /// The same for the sheet figure plus the import room test (the older signal).
    sheet_trust_bps: i64,
    passed: u32,
    strikes: u32,
    /// Recipes and (recipe, region) pairs that failed their quarter: never again.
    failed_recipes: std::collections::BTreeMap<u16, u32>,
    failed_places: std::collections::BTreeSet<(u16, u8)>,
    /// Recipes whose plants passed: a little more belief in them.
    proven_recipes: std::collections::BTreeSet<u16>,
    pause_until: u32,
    defend_until: u32,
    pub builds: u32,
    pub cuts: u32,
    /// Bids placed for plants on offer.
    pub bids: u32,
}

impl Default for SkilledState {
    fn default() -> Self {
        SkilledState {
            pending: None,
            trust_bps: 5_000,
            sheet_trust_bps: 5_000,
            passed: 0,
            strikes: 0,
            failed_recipes: Default::default(),
            failed_places: Default::default(),
            proven_recipes: Default::default(),
            pause_until: 0,
            defend_until: 0,
            builds: 0,
            cuts: 0,
            bids: 0,
        }
    }
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

    /// One cadence of the skilled player: operate, defend, cut what loses, then grow by one
    /// plant at a time, each only after the last has proved itself.
    pub(super) fn act_skilled(&mut self, session: &mut Session) {
        let controlled = controlled_firms(session);
        for firm in &controlled {
            self.skilled_operate(session, *firm);
        }
        if let Some(primary) = session.world.investor.primary_group {
            self.skilled_defend(session, primary.0);
            self.skilled_judge(session, primary.0);
            self.skilled_cut(session, primary.0);
            self.skilled_grow(session, primary.0);
        }
        if self.policy == ReferencePolicy::Skilled && self.skilled.passed >= 2 {
            self.skilled_acquire(session, &controlled);
        }
        if self.policy == ReferencePolicy::Skilled
            && let Some(primary) = session.world.investor.primary_group
        {
            self.skilled_bid(session, primary.0);
        }
    }

    /// Bid for a plant on offer (2.0) when the engine's opening figures for it pass the same
    /// test a build must: the weekly contribution, discounted by the script's trust in the
    /// opening signal, repays the price within `MAX_PAYBACK_WEEKS`, and the price leaves the
    /// build reserve in cash. The bid is the most that test allows, capped at the cost of
    /// building the same blocks today scaled to the share not yet written off, and never under a
    /// break-up lot's scrap price. One open bid at a time; places and recipes that failed their
    /// quarter, and divisions that lose money, are skipped as for a build.
    fn skilled_bid(&mut self, session: &mut Session, firm: u64) {
        use cw_sim::economy::sectors;
        let now = session.world.tick.0;
        let handle = Handle(firm);
        if session.world.pending_pause.is_some() || now < SETTLING_WEEKS {
            return;
        }
        let lots = session.plant_lots();
        if lots.iter().any(|lot| lot.player_bid_cents > 0) {
            return;
        }
        let fin = finance(session, firm);
        let Some(entity) = session.world.firm(handle) else {
            return;
        };
        if fin.ebitda_trailing_cents <= 0 {
            return;
        }
        let plants = entity.facilities.len() as i64;
        let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, handle);
        let reserve = outflow
            .saturating_mul(RESERVE_WEEKS)
            .saturating_mul(plants + 1)
            / plants.max(1);
        let spare = entity.book.cash() - reserve;
        let trust = self.skilled.trust_bps;
        let mut best: Option<(i64, u32, i64)> = None;
        for lot in &lots {
            if lot.yours || lot.weeks_left == 0 || !lot.bids.is_empty() || lot.weekly_cents <= 0 {
                continue;
            }
            let Some(plant) = session.world.facilities.get(Handle(lot.facility)) else {
                continue;
            };
            let recipe = plant.recipe;
            let industry = sectors::recipe_industry(&session.dataset, recipe);
            if self.skilled.failed_places.contains(&(recipe, lot.region))
                || self
                    .skilled
                    .failed_recipes
                    .get(&recipe)
                    .is_some_and(|n| *n >= 2)
                || self
                    .skilled_division_profit(
                        session,
                        firm,
                        (lot.region, industry),
                        now.saturating_sub(26),
                    )
                    .is_some_and(|(p, r)| r > 0 && p < 0)
            {
                continue;
            }
            let discounted = lot.weekly_cents * trust / 10_000;
            let price = discounted
                .saturating_mul(MAX_PAYBACK_WEEKS)
                .min(lot.replacement_cents)
                .min(spare);
            if price <= 0 || price < lot.reserve_cents {
                continue;
            }
            // Quickest payback first.
            let score = discounted * 10_000 / price;
            if best.is_none_or(|(b, ..)| score > b) {
                best = Some((score, lot.lot, price));
            }
        }
        let Some((_, lot, price)) = best else {
            return;
        };
        if self.submit(
            session,
            CommandIntent::BidForPlant {
                firm: handle,
                lot,
                price_cents: price,
            },
        ) {
            self.skilled.bids += 1;
            if self.verbose_skilled {
                println!("w{now} BID lot {lot} price {price} trust {trust}");
            }
        }
    }

    /// Mean weekly operating profit and revenue of a division over the firm's closed calendar
    /// quarters that began at or after `since` (the open quarter is lumpy and left out).
    /// None when no such quarter has closed.
    fn skilled_division_profit(
        &self,
        session: &Session,
        firm: u64,
        key: (u8, u8),
        since: u32,
    ) -> Option<(i64, i64)> {
        let quarters = session.world.archive.quarters.get(&Handle(firm))?;
        let closed = quarters.split_last()?.1;
        let (mut profit, mut revenue, mut weeks) = (0i64, 0i64, 0i64);
        for quarter in closed
            .iter()
            .filter(|q| q.first_week >= since && q.weeks >= 12)
        {
            if let Some(row) = quarter
                .divisions
                .iter()
                .find(|d| (d.region, d.industry) == key)
            {
                profit += row.profit;
                revenue += row.revenue;
            }
            weeks += quarter.weeks as i64;
        }
        (weeks > 0).then(|| (profit / weeks, revenue / weeks))
    }

    /// Raid alert naming the company: lift marketing (the division's only defence) for a
    /// quarter.
    fn skilled_defend(&mut self, session: &mut Session, firm: u64) {
        let now = session.world.tick.0;
        if session
            .world
            .raids
            .iter()
            .any(|raid| raid.defender.0 == firm && raid.tick + 3 >= now)
        {
            self.skilled.defend_until = now + 13;
        }
        if now < self.skilled.defend_until {
            let finance = finance(session, firm);
            self.submit(
                session,
                CommandIntent::SetMarketingBudget {
                    firm: Handle(firm),
                    amount_cents: percent_of(weekly_revenue(&finance), MARKETING_PERCENT * 2),
                },
            );
        }
    }

    /// Judge the plant last ordered once it has traded a quarter: did its division earn the
    /// plant's expected profit on top of what it earned before? Adjust trust in the engine's
    /// opening signal by how it actually did.
    fn skilled_judge(&mut self, session: &mut Session, firm: u64) {
        let Some(build) = self.skilled.pending.clone() else {
            return;
        };
        let now = session.world.tick.0;
        let Some(plant) = session.world.facilities.get(build.plant) else {
            self.skilled.pending = None;
            return;
        };
        if plant.status == cw_sim::state::FacilityStatus::Building
            || now < plant.completes.0 + PROVING_WEEKS
        {
            return;
        }
        let since = plant.completes.0;
        let Some((profit, revenue)) = self.skilled_division_profit(session, firm, build.key, since)
        else {
            return;
        };
        let realised = profit - build.base_profit;
        let ratio_bps =
            (realised.saturating_mul(10_000) / build.expected.max(1)).clamp(-10_000, 20_000);
        let passed = ratio_bps >= 4_000 && realised * 208 >= build.capex && revenue > 0;
        let trust = if build.from_sheet {
            &mut self.skilled.sheet_trust_bps
        } else {
            &mut self.skilled.trust_bps
        };
        *trust = ((*trust + ratio_bps.max(0)) / 2).clamp(3_000, 10_000);
        let trust_now = *trust;
        if self.verbose_skilled {
            println!(
                "w{now} JUDGE plant realised {realised} expected {} ratio {ratio_bps} -> {} trust {}",
                build.expected,
                if passed { "pass" } else { "fail" },
                trust_now
            );
        }
        self.skilled.pending = None;
        if passed {
            self.skilled.passed += 1;
            self.skilled.strikes = 0;
            self.skilled.proven_recipes.insert(build.recipe);
        } else {
            self.skilled.strikes += 1;
            // Selling next to nothing means the signal was wrong for this good: one strike
            // is enough to drop the recipe; a mere shortfall needs two.
            *self.skilled.failed_recipes.entry(build.recipe).or_default() +=
                if realised <= 0 { 2 } else { 1 };
            self.skilled
                .failed_places
                .insert((build.recipe, build.region));
            // A plant that loses money is mothballed; a failure also pauses building.
            let operating = session.world.firm(Handle(firm)).map_or(0, |f| {
                f.facilities
                    .iter()
                    .filter(|h| {
                        session
                            .world
                            .facilities
                            .get(**h)
                            .is_some_and(|p| p.is_operating())
                    })
                    .count()
            });
            if realised <= 0 && profit < 0 && operating >= 2 {
                self.submit(
                    session,
                    CommandIntent::MothballFacility {
                        facility: build.plant,
                    },
                );
            }
            self.skilled.pause_until = now + 13;
        }
    }

    /// Stop and cut: a division that has lost money for a quarter on revenue loses a plant to
    /// the mothball, newest first, one per cadence.
    fn skilled_cut(&mut self, session: &mut Session, firm: u64) {
        use cw_sim::economy::sectors;
        let now = session.world.tick.0;
        let mut victim: Option<(i64, Handle)> = None;
        for division in sectors::divisions_of(&session.world, &session.dataset, Handle(firm)) {
            let key = (division.region, division.industry);
            let operating: Vec<Handle> =
                division
                    .plants
                    .iter()
                    .copied()
                    .filter(|h| {
                        session.world.facilities.get(*h).is_some_and(|p| {
                            p.is_operating() && now >= p.completes.0 + PROVING_WEEKS
                        })
                    })
                    .collect();
            // Judge only a division whose every plant has traded a quarter.
            if operating.len() != division.plants.len() || operating.is_empty() {
                continue;
            }
            let newest = session
                .world
                .facilities
                .get(*operating.last().unwrap())
                .map_or(0, |p| p.completes.0);
            let Some((profit, revenue)) = self.skilled_division_profit(session, firm, key, newest)
            else {
                continue;
            };
            if revenue <= 0 || profit >= 0 {
                continue;
            }
            // A division with no sales is a feeder, judged with the firm, not alone.
            if victim.is_none_or(|(p, _)| profit < p) {
                victim = operating.last().map(|h| (profit, *h));
            }
        }
        let operating_total = session.world.firm(Handle(firm)).map_or(0, |f| {
            f.facilities
                .iter()
                .filter(|h| {
                    session
                        .world
                        .facilities
                        .get(**h)
                        .is_some_and(|p| p.is_operating())
                })
                .count()
        });
        if let Some((_, plant)) = victim.filter(|_| operating_total >= 2)
            && self.submit(session, CommandIntent::MothballFacility { facility: plant })
        {
            self.skilled.cuts += 1;
            self.skilled.pause_until = self.skilled.pause_until.max(now + 26);
        }
    }

    /// One new plant at a time, only on proof, inside the cash reserve and the borrowing rules.
    fn skilled_grow(&mut self, session: &mut Session, firm: u64) {
        use cw_sim::economy::sectors;
        let now = session.world.tick.0;
        let handle = Handle(firm);
        if self.skilled.pending.is_some()
            || now < self.skilled.pause_until
            || session.world.pending_pause.is_some()
        {
            return;
        }
        let fin = finance(session, firm);
        let Some(entity) = session.world.firm(handle) else {
            return;
        };
        // Nothing may be under construction, and the firm must have traded a quarter at a profit.
        let building = entity.facilities.iter().any(|h| {
            session
                .world
                .facilities
                .get(*h)
                .is_some_and(|p| p.status == cw_sim::state::FacilityStatus::Building)
        });
        let plants = entity.facilities.len() as i64;
        if building
            || now < SETTLING_WEEKS
            || fin.ebitda_trailing_cents <= 0
            || fin.net_income_trailing_cents <= 0
        {
            return;
        }
        let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, handle);
        let reserve = outflow
            .saturating_mul(RESERVE_WEEKS)
            .saturating_mul(plants + 1)
            / plants.max(1);
        let trust = self.skilled.trust_bps;
        let openings = session.capacity_openings(80);
        let mut best: Option<Candidate> = None;
        for open in &openings {
            let Some(&recipe) = session.dataset.recipe_index.get(&open.recipe_id) else {
                continue;
            };
            let industry = sectors::recipe_industry(&session.dataset, recipe);
            if self.skilled.failed_places.contains(&(recipe, open.region))
                || self
                    .skilled
                    .failed_recipes
                    .get(&recipe)
                    .is_some_and(|n| *n >= 2)
            {
                continue;
            }
            if sectors::division_strategy(
                &session.world,
                &session.dataset,
                handle,
                open.region,
                industry,
            )
            .is_some_and(|strategy| strategy != recipe)
            {
                continue;
            }
            // A division that already loses money gets nothing more.
            if self
                .skilled_division_profit(
                    session,
                    firm,
                    (open.region, industry),
                    now.saturating_sub(26),
                )
                .is_some_and(|(p, r)| r > 0 && p < 0)
            {
                continue;
            }
            // A recipe that has already passed may be built bigger, as far as the room allows.
            let blocks: i64 = if self.skilled.proven_recipes.contains(&recipe) {
                (open.room_lots * 4 / 5 / open.block_lots.max(1)).clamp(1, 3)
            } else {
                1
            };
            let quote = sectors::build_cost(
                &session.world,
                &session.dataset,
                handle,
                open.region,
                recipe,
                blocks as _,
            );
            let cost = quote.total_cents();
            let discounted = open.weekly_contribution_cents * blocks * trust / 10_000;
            if discounted <= 0 || cost <= 0 || cost > discounted * MAX_PAYBACK_WEEKS {
                continue;
            }
            // Quick payback first.
            let mut score = discounted * 10_000 / cost;
            if self.skilled.proven_recipes.contains(&recipe) {
                score *= 4;
            }
            if best.is_none_or(|(b, ..)| score > b) {
                best = Some((
                    score,
                    recipe,
                    open.region,
                    cost,
                    discounted,
                    industry,
                    blocks,
                    false,
                ));
            }
        }
        // The older signal: the sector sheet with imports already buying the output.
        let sheet_trust = self.skilled.sheet_trust_bps;
        for o in scan(session, firm) {
            let recipe = o.recipe;
            let industry = sectors::recipe_industry(&session.dataset, recipe);
            if o.room_pct < 80
                || o.weekly_cents <= 0
                || self.skilled.failed_places.contains(&(recipe, o.region))
                || self
                    .skilled
                    .failed_recipes
                    .get(&recipe)
                    .is_some_and(|n| *n >= 2)
                || self
                    .skilled_division_profit(
                        session,
                        firm,
                        (o.region, industry),
                        now.saturating_sub(26),
                    )
                    .is_some_and(|(p, r)| r > 0 && p < 0)
            {
                continue;
            }
            let discounted = o.weekly_cents * sheet_trust / 10_000;
            if discounted <= 0 || o.cost_cents <= 0 || o.cost_cents > discounted * MAX_PAYBACK_WEEKS
            {
                continue;
            }
            let mut score = discounted * 10_000 / o.cost_cents;
            if self.skilled.proven_recipes.contains(&recipe) {
                score *= 4;
            }
            if best.is_none_or(|(b, ..)| score > b) {
                best = Some((
                    score,
                    recipe,
                    o.region,
                    o.cost_cents,
                    discounted,
                    industry,
                    1,
                    true,
                ));
            }
        }
        let Some((_, recipe, region, cost, discounted, industry, blocks, from_sheet)) = best else {
            return;
        };
        let cash = fin.cash_cents;
        let need = cost + reserve;
        if cash < need {
            // Borrow only against proven earnings, only for the shortfall.
            let proven = self.skilled.passed >= 1;
            let capacity = cw_sim::economy::credit::secured_capacity(&session.world, handle)
                + cw_sim::economy::credit::unsecured_capacity(&session.world, handle);
            let earnings_cap =
                fin.ebitda_trailing_cents.saturating_mul(4 * 2) - fin.liabilities_cents;
            let headroom = (fin.assets_cents * MAX_LEVERAGE_PCT / 100 - fin.liabilities_cents)
                .min(earnings_cap);
            let amount = (need - cash + 4_999_999) / 5_000_000 * 5_000_000;
            // Weekly debt service of everything borrowed must stay under half of weekly operating profit.
            let service = (fin.liabilities_cents + amount) / 104;
            if !proven
                || amount > capacity
                || amount > headroom
                || service * 2 > fin.ebitda_trailing_cents / 13
            {
                return;
            }
            if !self.submit(
                session,
                CommandIntent::RequestLoan {
                    firm: handle,
                    amount_cents: amount,
                },
            ) {
                return;
            }
        }
        let before: std::collections::BTreeSet<u64> = session
            .world
            .firm(handle)
            .map(|f| f.facilities.iter().map(|h| h.0).collect())
            .unwrap_or_default();
        if !self.submit(
            session,
            CommandIntent::BuildFacility {
                firm: handle,
                region,
                recipe,
                blocks: blocks as _,
            },
        ) {
            return;
        }
        let Some(plant) = session.world.firm(handle).and_then(|f| {
            f.facilities
                .iter()
                .find(|h| !before.contains(&h.0))
                .copied()
        }) else {
            return;
        };
        let key = (region, industry);
        let base_profit = self
            .skilled_division_profit(session, firm, key, now.saturating_sub(26))
            .map_or(0, |(p, _)| p);
        self.skilled.pending = Some(SkilledBuild {
            plant,
            key,
            recipe,
            region,
            from_sheet,
            expected: discounted,
            base_profit,
            capex: cost,
        });
        self.skilled.builds += 1;
        if self.verbose_skilled {
            println!(
                "w{now} BUILD recipe {recipe} region {region} cost {cost} discounted {discounted} trust {trust}"
            );
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
        let brief = cw_session::brief::weekly_brief(&session.world, &session.dataset, firm);
        for mv in brief.moves.iter().filter(|mv| mv.key == "lane") {
            for command in &mv.commands {
                if let Ok(intent) = serde_json::from_str::<CommandIntent>(command) {
                    self.submit(session, intent);
                }
            }
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

/// `cw-cli story`: one career, week by week, from the engine's own figures. Prints a line a
/// week for the primary company (cash, debt, operating profit, plants) and, every `--every`
/// weeks and whenever a plant appears, each division's real weekly operating profit and input
/// fill against what the sheet expected when the plant was built.
pub fn story(arguments: &[String]) {
    use cw_sim::economy::{division_report, sectors};
    let scenario =
        crate::flag(arguments, "--scenario-id").unwrap_or_else(|| "great-lakes-2000".into());
    let seed: u64 = crate::flag(arguments, "--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let weeks: u32 = crate::flag(arguments, "--weeks")
        .and_then(|v| v.parse().ok())
        .unwrap_or(260);
    let every: u32 = crate::flag(arguments, "--every")
        .and_then(|v| v.parse().ok())
        .unwrap_or(13);
    let policy = crate::flag(arguments, "--policy")
        .and_then(|name| ReferencePolicy::parse(&name))
        .unwrap_or(ReferencePolicy::Skilled);
    let mut session = Session::create(crate::arc::arc_config_pub(seed, &scenario)).unwrap();
    let mut runner = PolicyRunner::new(policy, false);
    let mut known: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut last_week = u32::MAX;
    crate::drive_with(&mut session, weeks, &mut runner, &mut |session, _| {
        let week = session.world.tick.0;
        if week == last_week {
            return;
        }
        last_week = week;
        let Some(primary) = session.world.investor.primary_group else {
            return;
        };
        let Some(entity) = session.world.firm(primary) else {
            return;
        };
        let fin = finance(session, primary.0);
        let mut events = Vec::new();
        let mut live: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
        for handle in &entity.facilities {
            live.insert(handle.0);
            if !known.insert(handle.0) {
                continue;
            }
            let Some(plant) = session.world.facilities.get(*handle) else {
                continue;
            };
            let expected = plant_contribution(
                &session.world,
                &session.dataset,
                entity,
                plant.region,
                plant.recipe,
            ) * plant.blocks as i64;
            events.push(format!(
                "BUILT {} {} x{} capex {} sheet_expected_wk {}",
                session.dataset.recipes[plant.recipe as usize].id,
                session.dataset.regions[plant.region as usize].code,
                plant.blocks,
                plant.capex_cents / 100,
                expected / 100
            ));
        }
        known.retain(|handle| live.contains(handle));
        let last = fin.recent_weeks.last();
        println!(
            "w{week:>3} cash {:>9} debt {:>9} assets {:>9} rev {:>8} ebit {:>8} ni {:>8} plants {}",
            fin.cash_cents / 100,
            fin.liabilities_cents / 100,
            fin.assets_cents / 100,
            last.map_or(0, |w| w.revenue_cents / 100),
            last.map_or(0, |w| w.ebit_cents / 100),
            last.map_or(0, |w| w.net_income_cents / 100),
            entity.facilities.len()
        );
        for event in &events {
            println!("      {event}");
        }
        if week % every == 0 || !events.is_empty() {
            for division in sectors::divisions_of(&session.world, &session.dataset, primary) {
                if let Some(report) = division_report::report(
                    &session.world,
                    &session.dataset,
                    primary,
                    division.region,
                    division.industry,
                ) {
                    let short: Vec<String> = report
                        .inputs
                        .iter()
                        .filter(|input| input.need_milli > 0)
                        .map(|input| {
                            format!(
                                "{}:{}%",
                                session.dataset.products[input.product as usize].id,
                                input.used_milli * 100 / input.need_milli
                            )
                        })
                        .collect();
                    let made: Vec<String> = report
                        .outputs
                        .iter()
                        .map(|o| {
                            format!(
                                "{}:made {} sold {} price {} market {}",
                                session.dataset.products[o.product as usize].id,
                                o.made_milli / 1000,
                                o.sold_milli / 1000,
                                o.price_cents,
                                o.market_cents
                            )
                        })
                        .collect();
                    println!("      out [{}]", made.join(" "));
                    println!(
                        "      div {} {} plants {} blocks {} rev {} opprofit {} shortage {} inputs [{}]",
                        session.dataset.regions[division.region as usize].code,
                        session.dataset.recipes[division.strategy as usize].id,
                        division.plants.len(),
                        division.blocks,
                        report.revenue_cents / 100,
                        report.operating_profit_cents / 100,
                        report.shortage,
                        short.join(" ")
                    );
                }
            }
        }
    });
    let overview = session.overview();
    println!(
        "END w{} net_worth {}",
        session.world.tick.0,
        overview.investor_net_worth_cents / 100
    );
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
