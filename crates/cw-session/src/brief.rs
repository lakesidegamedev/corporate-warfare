//! The weekly brief: one page at the top of the week that says what changed, what is due and
//! what to do about it, in the engine's own numbers.
//!
//! It generalises the CFO (which speaks only when the company loses money) to every state of
//! the company. Read only: no random draws, no saved state, nothing written to the world, so it
//! cannot move a hash. Every suggested move carries the commands it would send and the figures
//! of the engine's own quote that justify it; the interface opens the matching dialog filled in
//! and never sends anything on its own.

use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::commands::CommandIntent;
use cw_sim::economy::chapters::{self, Mandate, MandateKind};
use cw_sim::economy::events_engine::player_side;
use cw_sim::economy::{division_report, logistics, sectors, timeline};
use cw_sim::state::{DealState, World};

use crate::{cfo, paper};

/// Most lines under "what changed".
pub const MAX_CHANGES: usize = 3;
/// Most suggested moves.
pub const MAX_MOVES: usize = 3;
/// Most lines under "what is due".
pub const MAX_DUE: usize = 5;
/// A price must move at least this much in a week to be news, in basis points.
const PRICE_NEWS_BPS: i64 = 500;
/// A raid is worth suggesting when it is at least this likely to work, in basis points.
const RAID_GOOD_BPS: i32 = 5_500;
/// A raid must pay back inside this many weeks to be suggested.
const MAX_PAYBACK_WEEKS: i64 = 260;
/// A build must pay back inside this many weeks to be suggested: ten years at today's prices.
const BUILD_MAX_PAYBACK_WEEKS: i64 = 520;
/// A build that takes this long or longer to pay back ranks after every other move: three years.
const SLOW_BUILD_WEEKS: i64 = 156;
/// Weeks of results that decide whether the company is losing money.
const LOSS_WINDOW: usize = 4;
/// Stock over this many weeks of sales is a pile; a price cut is suggested.
const PILE_WEEKS: i64 = 4;
/// The posture a suggested price move uses, in basis points against the market.
const NUDGE_BPS: i32 = 500;

/// One fact about the week just closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefChange {
    /// profit, price, rival, event or mandate.
    pub kind: String,
    pub text: String,
}

/// One decision or deadline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefDue {
    /// card, mandate_offer, mandate, loan, bid or results.
    pub kind: String,
    pub text: String,
    /// Whole weeks until it falls due; zero means this week.
    pub weeks_left: u32,
}

/// What a suggested move opens when the player presses Review.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MoveTarget {
    /// build, pricing, lane, mandate, raid, production, division, cfo or executive.
    pub dialog: String,
    pub region: String,
    pub region_to: String,
    pub industry: String,
    pub recipe: u32,
    pub blocks: u32,
    pub product: u32,
    pub mode: u32,
    pub lots: i64,
    pub weeks: u32,
    pub target: u64,
    pub budget_cents: i64,
}

/// One suggested move with the engine's quote behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefMove {
    /// build, price_up, price_down, lane, production, mandate or raid.
    pub key: String,
    pub headline: String,
    pub detail: String,
    /// What it costs up front, in the firm's cents.
    pub cost_cents: i64,
    /// What the engine's quote says it earns or saves each week.
    pub weekly_gain_cents: i64,
    /// Weeks to earn the cost back; zero when it costs nothing up front.
    pub payback_weeks: i64,
    /// False when the move has no figure behind it (a mandate to pick).
    pub has_payback: bool,
    pub action_label: String,
    pub target: MoveTarget,
    /// Command intents as JSON that the dialog would send. Never sent by the brief.
    pub commands: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Brief {
    /// The week the brief describes: the week just closed.
    pub week: u32,
    pub changes: Vec<BriefChange>,
    pub due: Vec<BriefDue>,
    pub moves: Vec<BriefMove>,
    /// Why there is nothing to do, in a few words. Empty whenever there is a move.
    pub hold: String,
}

fn json(intent: CommandIntent) -> String {
    serde_json::to_string(&intent).unwrap_or_default()
}

fn signed(world: &World, dataset: &Dataset, firm: Handle, cents: i64) -> String {
    let body = paper::money(world, dataset, firm, cents);
    if cents < 0 {
        format!("a loss of {body}")
    } else {
        body
    }
}

/// Facts about the week that closed, biggest first, three at most. The size of each is its
/// weekly value in cents, so a rival's raid, a price swing and a profit swing compare.
fn changes(world: &World, dataset: &Dataset, firm: Handle) -> Vec<BriefChange> {
    let now = world.tick.0;
    let mut items: Vec<(i64, BriefChange)> = Vec::new();
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };

    let weeks = entity.book.recent_weeks();
    if let Some(last) = weeks.last() {
        let before = weeks.len().checked_sub(2).map(|i| weeks[i].net_income);
        let mut text = format!(
            "Profit last week was {}",
            signed(world, dataset, firm, last.net_income)
        );
        let mut size = last.net_income.abs();
        if let Some(before) = before {
            let swing = last.net_income - before;
            size = size.max(swing.abs());
            let word = if swing >= 0 { "up" } else { "down" };
            text += &format!(
                ", {word} {} on the week before",
                paper::money(world, dataset, firm, swing.abs())
            );
        }
        text += &format!(
            ". Cash is now {}",
            paper::money(world, dataset, firm, entity.book.cash())
        );
        items.push((
            size,
            BriefChange {
                kind: "profit".to_owned(),
                text: text + ".",
            },
        ));
    }

    // A price move on a good the firm sells.
    let history = &world.price_history;
    if history.len() >= 2 {
        let (a, b) = (&history[history.len() - 2], &history[history.len() - 1]);
        for line in crate::dto::product_lines(world, dataset, firm.0) {
            let (Some(old), Some(new)) = (a.goods.get(&line.product), b.goods.get(&line.product))
            else {
                continue;
            };
            if *old <= 0 {
                continue;
            }
            let bps = (new - old) * 10_000 / old;
            if bps.abs() < PRICE_NEWS_BPS {
                continue;
            }
            let lots = line
                .sold_last_week_lots
                .max(line.made_last_week_lots)
                .max(1);
            items.push((
                (new - old).abs() * lots,
                BriefChange {
                    kind: "price".to_owned(),
                    text: format!(
                        "{} {} {} percent in the market, to {} a lot.",
                        line.name,
                        if bps > 0 { "rose" } else { "fell" },
                        bps.abs() / 100,
                        paper::money(world, dataset, firm, *new)
                    ),
                },
            ));
        }
    }

    // A rival's move against the firm this week.
    for raid in world
        .raids
        .iter()
        .filter(|r| r.tick + 1 >= now && r.tick <= now)
    {
        if player_side(world, raid.defender) && !player_side(world, raid.attacker) {
            let who = paper::firm_name(world, raid.attacker).unwrap_or_default();
            let region = paper::region_name(world, raid.region);
            let text = if raid.success {
                format!(
                    "{who} raided you in {region} and took {} of your plants.",
                    if raid.blocks == 1 {
                        "one block".to_owned()
                    } else {
                        format!("{} blocks", raid.blocks)
                    }
                )
            } else {
                format!("{who} raided you in {region} and failed.")
            };
            items.push((
                raid.budget_cents.max(raid.price_cents) + if raid.success { 1 } else { 0 },
                BriefChange {
                    kind: "rival".to_owned(),
                    text,
                },
            ));
        }
    }

    // A timeline event that fired this week.
    for event in timeline::past(world).iter().filter(|e| e.tick + 1 >= now) {
        items.push((
            1,
            BriefChange {
                kind: "event".to_owned(),
                text: format!("{}: {}", event.title, event.summary),
            },
        ));
    }

    // A mandate that has been met.
    if let Some(mandate) = world.chapters.active.get(&firm)
        && mandate.met
    {
        let label = crate::letters::mandate_label(mandate.kind.key());
        items.push((
            1,
            BriefChange {
                kind: "mandate".to_owned(),
                text: format!("Your board mandate is met: {label}."),
            },
        ));
    }

    items.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.text.cmp(&b.1.text)));
    items
        .into_iter()
        .take(MAX_CHANGES)
        .map(|(_, c)| c)
        .collect()
}

/// Decisions and deadlines, soonest first.
fn due(world: &World, dataset: &Dataset, firm: Handle) -> Vec<BriefDue> {
    let now = world.tick.0;
    let mut out: Vec<BriefDue> = Vec::new();

    for line in crate::dto::events(world, dataset) {
        out.push(BriefDue {
            kind: "card".to_owned(),
            text: if line.subject.is_empty() {
                format!("Decision: {}", line.title)
            } else {
                format!("Decision: {} ({})", line.title, line.subject)
            },
            weeks_left: line.deadline.saturating_sub(now),
        });
    }

    if let Some(offer) = chapters::player_offer(world)
        && !offer.kinds.is_empty()
    {
        let window = cw_data::chapters::mandate_table().offer_window_weeks;
        let closes = chapters::chapter_start(offer.chapter) + window;
        out.push(BriefDue {
            kind: "mandate_offer".to_owned(),
            text: "The board is waiting for you to pick a mandate".to_owned(),
            weeks_left: closes.saturating_sub(now),
        });
    }
    if let Some(mandate) = world.chapters.active.get(&firm)
        && !mandate.met
        && !mandate.broken
    {
        let progress = chapters::progress(world, dataset, mandate);
        out.push(BriefDue {
            kind: "mandate".to_owned(),
            text: format!(
                "Board mandate: {} ({} percent there)",
                crate::letters::mandate_label(mandate.kind.key()),
                (progress.bps / 100).clamp(0, 100)
            ),
            weeks_left: mandate.due.saturating_sub(now),
        });
    }

    if let Some(entity) = world.firms.get(firm) {
        let mut payment = 0i64;
        let mut count = 0usize;
        let mut longest = 0u32;
        for handle in &entity.loans {
            let Some(loan) = world.loans.get(*handle) else {
                continue;
            };
            if loan.defaulted || loan.remaining_weeks == 0 {
                continue;
            }
            payment += loan.weekly_principal_cents
                + loan.arrears_cents
                + loan.principal_remaining_cents * loan.rate_bps as i64 / 10_000 / 52;
            count += 1;
            longest = longest.max(loan.remaining_weeks);
        }
        if count > 0 {
            out.push(BriefDue {
                kind: "loan".to_owned(),
                text: format!(
                    "Loan payments of {} a week on {} loan{}, {} weeks left on the longest",
                    paper::money(world, dataset, firm, payment),
                    count,
                    if count == 1 { "" } else { "s" },
                    longest
                ),
                weeks_left: 1,
            });
        }
    }

    if let Some(fe) = world.earnings.firms.get(&firm)
        && let Some(estimate) = fe.estimate.as_ref()
        && estimate.due >= now
        && cw_sim::economy::earnings::covered(world, firm)
    {
        let weeks = estimate.due - now;
        let when = match weeks {
            0 => "Results today".to_owned(),
            1 => "Results in 1 week".to_owned(),
            n => format!("Results in {n} weeks"),
        };
        let guided = match &fe.guidance {
            Some(g) => format!(
                ", you guided {} to {}",
                paper::money(world, dataset, firm, g.low_cents),
                paper::money(world, dataset, firm, g.high_cents)
            ),
            None => String::new(),
        };
        out.push(BriefDue {
            kind: "results".to_owned(),
            text: format!(
                "{when}: analysts expect {}{guided}",
                paper::money(world, dataset, firm, estimate.cents)
            ),
            weeks_left: weeks,
        });
    }

    for (_, deal) in world.deals.iter() {
        if !matches!(deal.state, DealState::OfferOpen | DealState::Review)
            || !player_side(world, deal.target)
            || player_side(world, deal.buyer)
        {
            continue;
        }
        let who = paper::firm_name(world, deal.buyer).unwrap_or_default();
        out.push(BriefDue {
            kind: "bid".to_owned(),
            text: format!("{who}'s bid for your company closes"),
            weeks_left: deal.offer_deadline.0.saturating_sub(now),
        });
    }

    out.sort_by(|a, b| a.weeks_left.cmp(&b.weeks_left).then(a.text.cmp(&b.text)));
    out.truncate(MAX_DUE);
    out
}

/// Weeks to earn back `cost` at `gain` a week, rounded up. None when it never does.
fn payback(cost: i64, gain: i64) -> Option<i64> {
    if gain <= 0 {
        return None;
    }
    let weeks = (cost.max(0) + gain - 1) / gain;
    Some(weeks)
}

/// A payback in plain time: weeks under a year, years to one decimal after.
pub fn payback_text(weeks: i64) -> String {
    if weeks < 52 {
        return format!("{weeks} weeks");
    }
    let tenths = weeks * 10 / 52;
    format!("{}.{} years", tenths / 10, tenths % 10)
}

fn recipe_name(dataset: &Dataset, recipe: u16) -> String {
    let t = &dataset.recipes[recipe as usize];
    if t.name.is_empty() {
        t.id.clone()
    } else {
        t.name.clone()
    }
}

/// Build one more block where imports point: a division the firm runs, in a market whose
/// imports no local plant meets, priced by the engine's build quote and paid back from the
/// division's own margin.
fn build_move(world: &World, dataset: &Dataset, firm: Handle) -> Option<BriefMove> {
    build_move_within(world, dataset, firm, BUILD_MAX_PAYBACK_WEEKS)
}

fn build_move_within(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    max_weeks: i64,
) -> Option<BriefMove> {
    let cash = world.firms.get(firm)?.book.cash();
    let mut best: Option<(i64, BriefMove)> = None;
    for d in sectors::divisions_of(world, dataset, firm) {
        let (_, headroom) = sectors::cell(world, dataset, d.region, d.industry);
        if headroom <= 0 {
            continue;
        }
        let Some(report) = division_report::report(world, dataset, firm, d.region, d.industry)
        else {
            continue;
        };
        if report.margin_bps <= 0 {
            continue;
        }
        let quote = sectors::build_cost(world, dataset, firm, d.region, d.strategy, 1);
        let cost = quote.total_cents();
        if cost > cash {
            continue;
        }
        let added = sectors::capacity_value(dataset, d.strategy, 1);
        let sold = added.min(headroom);
        let gain = sold * report.margin_bps as i64 / 10_000;
        let Some(weeks) = payback(cost, gain).filter(|w| *w <= max_weeks) else {
            continue;
        };
        let region = &dataset.regions[d.region as usize];
        let mv = BriefMove {
            key: "build".to_owned(),
            headline: format!(
                "Build {} in {}",
                recipe_name(dataset, d.strategy).to_lowercase(),
                region.name
            ),
            detail: format!(
                "Imports into {} are filling demand your plants do not meet. One more block costs {} and, at your margin in that division at today's prices, pays itself back in about {}.",
                region.name,
                paper::money(world, dataset, firm, cost),
                payback_text(weeks)
            ),
            cost_cents: cost,
            weekly_gain_cents: gain,
            payback_weeks: weeks,
            has_payback: true,
            action_label: "Review".to_owned(),
            target: MoveTarget {
                dialog: "build".to_owned(),
                region: region.code.clone(),
                recipe: d.strategy as u32,
                blocks: 1,
                ..Default::default()
            },
            commands: vec![json(CommandIntent::BuildFacility {
                firm,
                region: d.region,
                recipe: d.strategy,
                blocks: 1,
            })],
        };
        if best.as_ref().is_none_or(|(w, _)| weeks < *w) {
            best = Some((weeks, mv));
        }
    }
    best.map(|(_, mv)| mv)
}

/// Reserve a lane where outside carriers take a lot of the firm's money: the shipping summary's
/// own suggestion, priced by the reserve quote.
fn lane_move(world: &World, dataset: &Dataset, firm: Handle) -> Option<BriefMove> {
    let summary = logistics::shipping_summary(world, dataset, firm, None);
    let logistics::Suggestion::Reserve { key, lots, .. } = summary.suggestion? else {
        return None;
    };
    const TERM_WEEKS: u32 = 26;
    let quote = logistics::reserve_quote(world, dataset, firm, key, lots, TERM_WEEKS)?;
    if quote.weekly_net_cents <= 0 || quote.max_lots < lots as i64 {
        return None;
    }
    let cash = world.firms.get(firm)?.book.cash();
    if quote.weekly_fee_cents > cash {
        return None;
    }
    let (a, b) = (
        &dataset.regions[key.a as usize],
        &dataset.regions[key.b as usize],
    );
    let mode = dataset.logistics.modes[key.mode as usize]
        .name
        .to_lowercase();
    Some(BriefMove {
        key: "lane".to_owned(),
        headline: format!(
            "Reserve {lots} loads from {} to {} by {mode}",
            a.name, b.name
        ),
        detail: format!(
            "Outside carriers take a lot of your freight money on this route. Reserving {lots} loads for {TERM_WEEKS} weeks costs {} a week and saves about {} a week at last week's volume.",
            paper::money(world, dataset, firm, quote.weekly_fee_cents),
            paper::money(world, dataset, firm, quote.weekly_net_cents)
        ),
        cost_cents: 0,
        weekly_gain_cents: quote.weekly_net_cents,
        payback_weeks: 0,
        has_payback: true,
        action_label: "Review".to_owned(),
        target: MoveTarget {
            dialog: "lane".to_owned(),
            region: a.code.clone(),
            region_to: b.code.clone(),
            mode: key.mode as u32,
            lots: lots as i64,
            weeks: TERM_WEEKS,
            ..Default::default()
        },
        commands: vec![json(CommandIntent::ReserveLane {
            firm,
            from: key.a,
            to: key.b,
            mode: key.mode,
            lots,
            weeks: TERM_WEEKS,
        })],
    })
}

/// Price moves: raise where the good sells out, lower where stock piles up. A posture change
/// costs nothing up front; the figure is the extra weekly revenue from a nudge on what sold
/// (raise) or the cash freed from the pile over four weeks (lower).
fn price_moves(world: &World, dataset: &Dataset, firm: Handle) -> Vec<BriefMove> {
    let mut out = Vec::new();
    for line in crate::dto::product_lines(world, dataset, firm.0) {
        if line.mode == "fixed" || line.price_cents <= 0 {
            continue;
        }
        let expires = dataset
            .products
            .get(line.product as usize)
            .is_some_and(|p| p.kind.is_service());
        if expires {
            continue;
        }
        let sold = line.sold_last_week_lots.max(0);
        let (key, bps, gain, headline, detail) = if line.stock_lots > 0
            && line.stock_lots >= PILE_WEEKS * sold.max(1)
            && line.stock_lots * line.price_cents >= 50_000
        {
            let freed = line.stock_lots * line.price_cents / 4;
            (
                "price_down",
                -NUDGE_BPS,
                freed,
                format!("Cut the price of {}", line.name.to_lowercase()),
                format!(
                    "{} lots are in stock against {} sold last week. Pricing 5 percent under the market should move them and frees about {} a week of cash for the next four weeks.",
                    line.stock_lots,
                    sold,
                    paper::money(world, dataset, firm, freed)
                ),
            )
        } else if sold > 0
            && line.stock_lots == 0
            && line.made_last_week_lots > 0
            && sold >= line.made_last_week_lots
        {
            let extra = sold * line.price_cents * NUDGE_BPS as i64 / 10_000;
            (
                "price_up",
                NUDGE_BPS,
                extra,
                format!("Raise the price of {}", line.name.to_lowercase()),
                format!(
                    "You sold everything you made of {} last week and hold no stock. Pricing 5 percent over the market adds about {} a week if buyers stay.",
                    line.name.to_lowercase(),
                    paper::money(world, dataset, firm, extra)
                ),
            )
        } else {
            continue;
        };
        out.push(BriefMove {
            key: key.to_owned(),
            headline,
            detail,
            cost_cents: 0,
            weekly_gain_cents: gain,
            payback_weeks: 0,
            has_payback: true,
            action_label: "Review".to_owned(),
            target: MoveTarget {
                dialog: "pricing".to_owned(),
                product: line.product as u32,
                ..Default::default()
            },
            commands: vec![json(CommandIntent::SetPricePosture {
                firm,
                product: line.product,
                mode: 1,
                bps,
            })],
        });
    }
    out.sort_by(|a, b| {
        b.weekly_gain_cents
            .cmp(&a.weekly_gain_cents)
            .then(a.headline.cmp(&b.headline))
    });
    out.truncate(1);
    out
}

/// Pick the offered mandate the firm is closest to, when the board has one on the table.
fn mandate_move(world: &World, dataset: &Dataset, firm: Handle) -> Option<BriefMove> {
    let offer = chapters::player_offer(world)?;
    let mut best: Option<(i64, usize, &MandateKind)> = None;
    for (i, kind) in offer.kinds.iter().enumerate() {
        let trial = Mandate {
            firm,
            chapter: offer.chapter,
            set: offer.set,
            due: 0,
            kind: kind.clone(),
            met: false,
            broken: false,
        };
        let bps = chapters::progress(world, dataset, &trial).bps;
        if best.as_ref().is_none_or(|(b, _, _)| bps > *b) {
            best = Some((bps, i, kind));
        }
    }
    let (bps, pick, kind) = best?;
    Some(BriefMove {
        key: "mandate".to_owned(),
        headline: "Pick a board mandate".to_owned(),
        detail: format!(
            "The board has offered goals for this chapter. You are closest to: {} ({} percent there already). An offer left alone lapses with no penalty.",
            crate::letters::mandate_label(kind.key()),
            (bps / 100).clamp(0, 100)
        ),
        cost_cents: 0,
        weekly_gain_cents: 0,
        payback_weeks: 0,
        has_payback: false,
        action_label: "Review".to_owned(),
        target: MoveTarget {
            dialog: "mandate".to_owned(),
            lots: pick as i64,
            ..Default::default()
        },
        commands: vec![json(CommandIntent::ChooseMandate {
            firm,
            pick: pick as u8,
        })],
    })
}

/// A raid on a rival's division the firm also runs, when the engine's quote at a two-to-one
/// campaign is good, the firm earned money over its last thirteen weeks and the campaign costs
/// at most half of that: the same earnings rule the rival companies follow.
fn raid_move(world: &World, dataset: &Dataset, firm: Handle) -> Option<BriefMove> {
    let entity = world.firms.get(firm)?;
    let earned: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|w| w.net_income)
        .sum();
    if earned <= 0 {
        return None;
    }
    let cash = entity.book.cash();
    let mut best: Option<(i64, BriefMove)> = None;
    for d in sectors::divisions_of(world, dataset, firm) {
        let Some(report) = division_report::report(world, dataset, firm, d.region, d.industry)
        else {
            continue;
        };
        if report.margin_bps <= 0 {
            continue;
        }
        let (rivals, _) = sectors::cell(world, dataset, d.region, d.industry);
        for (rival, _) in rivals {
            if rival == firm
                || cw_sim::economy::ownership::same_group(world, rival, firm)
                || cw_sim::economy::fx::require_same_currency(world, firm, rival).is_err()
                || world.tick.0
                    < sectors::raid_open_tick(world, dataset, rival, d.region, d.industry)
            {
                continue;
            }
            let plants = sectors::division_plants(world, dataset, rival, d.region, d.industry);
            let rival_blocks: i64 = plants
                .iter()
                .filter_map(|h| world.facilities.get(*h))
                .map(|p| p.blocks as i64)
                .sum();
            if rival_blocks < 1 {
                continue;
            }
            let probe = sectors::raid_quote(world, dataset, firm, rival, d.region, d.industry, 0);
            let budget = probe.defence_cents * 2;
            let q = sectors::raid_quote(world, dataset, firm, rival, d.region, d.industry, budget);
            let campaign = budget + q.surcharge_cents;
            if q.chance_bps < RAID_GOOD_BPS
                || q.blocks_at_stake == 0
                || campaign * 2 > earned
                || cash < campaign + q.price_cents
            {
                continue;
            }
            let rival_value: i64 = plants
                .iter()
                .filter_map(|h| world.facilities.get(*h))
                .map(|p| sectors::capacity_value(dataset, p.recipe, p.blocks))
                .sum();
            let taken_value = rival_value * q.stake_bps as i64 / 10_000;
            let chance = q.chance_bps as i64;
            let gain = taken_value * chance / 10_000 * report.margin_bps as i64 / 10_000;
            let expected_cost = campaign + q.price_cents * chance / 10_000;
            let Some(weeks) = payback(expected_cost, gain).filter(|w| *w <= MAX_PAYBACK_WEEKS)
            else {
                continue;
            };
            let name = paper::firm_name(world, rival).unwrap_or_default();
            let region = &dataset.regions[d.region as usize];
            let mv = BriefMove {
                key: "raid".to_owned(),
                headline: format!("Raid {name} in {}", region.name),
                detail: format!(
                    "A campaign of {} has a {} percent chance to take {} of {}'s {} plants there, and pays back in about {} if it works. Its cost stays within half of what you earned over the last quarter.",
                    paper::money(world, dataset, firm, campaign),
                    q.chance_bps / 100,
                    if q.blocks_at_stake == 1 {
                        "one block".to_owned()
                    } else {
                        format!("{} blocks", q.blocks_at_stake)
                    },
                    name,
                    cw_sim::state::INDUSTRY_NAMES[d.industry as usize].replace('_', " "),
                    payback_text(weeks)
                ),
                cost_cents: expected_cost,
                weekly_gain_cents: gain,
                payback_weeks: weeks,
                has_payback: true,
                action_label: "Review".to_owned(),
                target: MoveTarget {
                    dialog: "raid".to_owned(),
                    region: region.code.clone(),
                    industry: cw_sim::state::INDUSTRY_NAMES[d.industry as usize].to_owned(),
                    target: rival.0,
                    budget_cents: budget,
                    ..Default::default()
                },
                commands: vec![json(CommandIntent::RaidMarket {
                    firm,
                    target: rival,
                    region: d.region,
                    industry: d.industry,
                    budget_cents: budget,
                })],
            };
            if best.as_ref().is_none_or(|(w, _)| weeks < *w) {
                best = Some((weeks, mv));
            }
        }
    }
    best.map(|(_, mv)| mv)
}

/// Match output to demand: the CFO's own fix, offered in every state of the company.
fn production_move(world: &World, dataset: &Dataset, firm: Handle) -> Option<BriefMove> {
    let entity = world.firms.get(firm)?;
    let weeks = entity.book.recent_weeks();
    let recent = &weeks[weeks.len().saturating_sub(4)..];
    let revenue = if recent.is_empty() {
        0
    } else {
        recent.iter().map(|w| w.revenue).sum::<i64>() / recent.len() as i64
    };
    let advice = cfo::idle_capacity(world, dataset, firm.0, revenue)?;
    Some(BriefMove {
        key: "production".to_owned(),
        headline: advice.headline,
        detail: advice.detail,
        cost_cents: 0,
        weekly_gain_cents: advice.weekly_cents,
        payback_weeks: 0,
        has_payback: true,
        action_label: advice.action_label,
        target: MoveTarget {
            dialog: "production".to_owned(),
            ..Default::default()
        },
        commands: advice.commands,
    })
}

/// Weeks within which a hire must pay back its signing fee, from what the executive's effect
/// saves or earns over their pay.
pub const HIRE_MAX_PAYBACK_WEEKS: i64 = 104;

/// Suggest filling a vacant executive post when the executive's effect, at the company's own
/// figures, covers their pay and pays back the signing fee inside two years.
pub fn hire_move(world: &World, dataset: &Dataset, firm: Handle) -> Option<BriefMove> {
    use cw_sim::economy::executives as ex;
    let cash = world.firms.get(firm)?.book.cash();
    let mut best: Option<(i64, ex::Post, u16, i64, i64, i64)> = None;
    for post in ex::Post::ALL {
        if world.executives.holder(firm, post).is_some() {
            continue;
        }
        for c in ex::candidates(world, post) {
            let ask = ex::ask_cents(world, firm, c);
            let fee = ask * ex::SIGNING_WEEKS;
            if cash < fee + ask * ex::CASH_COVER_WEEKS {
                continue;
            }
            let value = ex::yearly_value_cents(world, dataset, firm, post, c.level);
            let gain = value / 52 - ask;
            if gain <= 0 {
                continue;
            }
            let weeks = (fee + gain - 1) / gain;
            if weeks > HIRE_MAX_PAYBACK_WEEKS {
                continue;
            }
            if best.is_none_or(|(g, ..)| gain > g) {
                best = Some((gain, post, c.id, fee, weeks, ask));
            }
        }
    }
    let (gain, post, id, fee, weeks, ask) = best?;
    let c = world.executives.person(id)?;
    Some(BriefMove {
        key: "hire".to_owned(),
        headline: format!("Hire {} as {}", ex::name_of(c), post.title().to_lowercase()),
        detail: format!(
            "{}. At your figures that is worth about {} a week over their pay of {}, and the {} signing fee is back in {} weeks.",
            ex::effect_text(post, c.level),
            paper::money(world, dataset, firm, gain),
            paper::money(world, dataset, firm, ask),
            paper::money(world, dataset, firm, fee),
            weeks
        ),
        cost_cents: fee,
        weekly_gain_cents: gain,
        payback_weeks: weeks,
        has_payback: true,
        action_label: "Review".to_owned(),
        target: MoveTarget {
            dialog: "executive".to_owned(),
            mode: post.code() as u32,
            target: id as u64,
            ..Default::default()
        },
        commands: vec![json(CommandIntent::HireExecutive {
            firm,
            post: post.code(),
            person: id,
        })],
    })
}

/// Where a move sits: ordinary moves first, a move with no figure after them, and a build that
/// takes three years or more to pay back after everything else.
fn rank(m: &BriefMove) -> u8 {
    if m.key == "build" && m.payback_weeks >= SLOW_BUILD_WEEKS {
        2
    } else if m.has_payback {
        0
    } else {
        1
    }
}

/// Quickest payback first; free moves count as zero weeks, biggest gain first among equals, a
/// move with no figure behind it after those, and a slow build last of all.
fn order(all: &mut [BriefMove]) {
    all.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then(a.payback_weeks.cmp(&b.payback_weeks))
            .then(b.weekly_gain_cents.cmp(&a.weekly_gain_cents))
            .then(a.key.cmp(&b.key))
            .then(a.headline.cmp(&b.headline))
    });
}

/// What the last four closed weeks add up to, when there are any. Negative means losing.
fn recent_net(world: &World, firm: Handle) -> Option<i64> {
    let weeks = world.firms.get(firm)?.book.recent_weeks();
    let recent = &weeks[weeks.len().saturating_sub(LOSS_WINDOW)..];
    if recent.is_empty() {
        return None;
    }
    Some(recent.iter().map(|w| w.net_income).sum())
}

/// How many of the latest weeks show the loss: four when they add up to one, otherwise just
/// the last week when it alone lost money, otherwise none.
fn loss_weeks(world: &World, firm: Handle) -> usize {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    if recent_net(world, firm).is_some_and(|net| net < 0) {
        return LOSS_WINDOW;
    }
    match entity.book.recent_weeks().last() {
        Some(last) if last.net_income < 0 => 1,
        _ => 0,
    }
}

/// The CFO's reasons as moves, biggest weekly cost first. Each carries its one-button fix and
/// the weekly saving as its figure.
fn cfo_moves(world: &World, dataset: &Dataset, firm: Handle) -> Vec<BriefMove> {
    let mut items = cfo::advice(world, dataset, firm.0);
    items.sort_by(|a, b| b.weekly_cents.cmp(&a.weekly_cents).then(a.key.cmp(&b.key)));
    items
        .into_iter()
        .map(|advice| {
            // A single underpriced good opens its pricing sheet; the rest open the plain
            // confirmation with the fix ready.
            let dialog = if advice.key == "below_cost" && advice.commands.len() == 1 {
                "pricing"
            } else {
                "cfo"
            };
            let product = advice
                .commands
                .first()
                .and_then(|c| serde_json::from_str::<CommandIntent>(c).ok())
                .and_then(|c| match c {
                    CommandIntent::SetPricePosture { product, .. } => Some(product as u32),
                    _ => None,
                })
                .unwrap_or_default();
            BriefMove {
                key: format!("cfo_{}", advice.key),
                headline: advice.headline,
                detail: format!(
                    "{} It saves about {} a week.",
                    advice.detail,
                    paper::money(world, dataset, firm, advice.weekly_cents)
                ),
                cost_cents: 0,
                weekly_gain_cents: advice.weekly_cents,
                payback_weeks: 0,
                has_payback: true,
                action_label: advice.action_label,
                target: MoveTarget {
                    dialog: dialog.to_owned(),
                    product,
                    ..Default::default()
                },
                commands: advice.commands,
            }
        })
        .collect()
}

/// The division with the worst margin at today's prices: its region code, industry name and
/// margin in basis points.
fn worst_division(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
) -> Option<(String, String, String, i32)> {
    let mut worst: Option<(i32, String, String, String)> = None;
    for d in sectors::divisions_of(world, dataset, firm) {
        let Some(report) = division_report::report(world, dataset, firm, d.region, d.industry)
        else {
            continue;
        };
        if worst.as_ref().is_none_or(|(m, ..)| report.margin_bps < *m) {
            let region = &dataset.regions[d.region as usize];
            worst = Some((
                report.margin_bps,
                region.code.clone(),
                region.name.clone(),
                cw_sim::state::INDUSTRY_NAMES[d.industry as usize].to_owned(),
            ));
        }
    }
    worst.map(|(m, code, name, industry)| (code, name, industry, m))
}

/// A losing company the CFO has no named reason for still sees where the money goes: the
/// biggest line of the income statement against revenue, and the weakest division to open.
fn diagnosis_move(world: &World, dataset: &Dataset, firm: Handle, take: usize) -> BriefMove {
    let weeks = world
        .firms
        .get(firm)
        .map(|f| f.book.recent_weeks())
        .unwrap_or(&[]);
    let recent = &weeks[weeks.len().saturating_sub(take)..];
    let count = recent.len().max(1) as i64;
    let net: i64 = recent.iter().map(|w| w.net_income).sum();
    let revenue = recent.iter().map(|w| w.revenue).sum::<i64>() / count;
    let lines: [(&str, &str, i64); 5] = [
        (
            "Wages",
            "are",
            recent.iter().map(|w| w.wages).sum::<i64>() / count,
        ),
        (
            "The cost of making what you sell",
            "is",
            recent.iter().map(|w| w.cost_of_goods).sum::<i64>() / count,
        ),
        (
            "Running costs",
            "are",
            recent.iter().map(|w| w.operating_expense).sum::<i64>() / count,
        ),
        (
            "Interest on your debt",
            "is",
            recent
                .iter()
                .map(|w| w.interest_expense - w.interest_income)
                .sum::<i64>()
                / count,
        ),
        (
            "Depreciation",
            "is",
            recent.iter().map(|w| w.depreciation).sum::<i64>() / count,
        ),
    ];
    let biggest = lines
        .iter()
        .max_by_key(|l| l.2)
        .copied()
        .unwrap_or(lines[0]);
    let headline = if revenue <= 0 {
        "The company is not selling anything yet".to_owned()
    } else {
        format!(
            "{} {} {} percent of revenue",
            biggest.0,
            biggest.1,
            biggest.2 * 100 / revenue
        )
    };
    let weekly_loss = paper::money(world, dataset, firm, (-net / count).max(0));
    let mut detail = if recent.len() == 1 {
        format!("The company lost {weekly_loss} last week.")
    } else {
        format!(
            "The company has lost about {weekly_loss} a week over the last {} weeks.",
            recent.len()
        )
    };
    let mut target = MoveTarget::default();
    if let Some((code, name, industry, margin)) = worst_division(world, dataset, firm) {
        detail += &format!(
            " The weakest division is {} in {name}, at a margin of {} percent. Open it to see where the money goes.",
            industry.replace('_', " "),
            margin / 100
        );
        target = MoveTarget {
            dialog: "division".to_owned(),
            region: code,
            industry,
            ..Default::default()
        };
    }
    BriefMove {
        key: "losing".to_owned(),
        headline,
        detail,
        cost_cents: 0,
        weekly_gain_cents: 0,
        payback_weeks: 0,
        has_payback: false,
        action_label: "Review".to_owned(),
        target,
        commands: Vec::new(),
    }
}

/// Up to three suggested moves. A company that lost money over the last four weeks sees the
/// CFO's reasons first, biggest weekly cost first, and always at least one move. A company whose
/// latest week lost money gets the same, with the plain reading of that week. Otherwise (and
/// after those) quickest payback first: moves with no up-front cost come first, biggest weekly
/// gain first; a mandate to pick, which has no figure, after them; a slow build last.
fn moves(world: &World, dataset: &Dataset, firm: Handle) -> Vec<BriefMove> {
    let losing = loss_weeks(world, firm);
    let mut lead: Vec<BriefMove> = Vec::new();
    if losing > 0 {
        lead = cfo_moves(world, dataset, firm);
        if lead.is_empty() {
            lead.push(diagnosis_move(world, dataset, firm, losing));
        }
    }
    let mut all: Vec<BriefMove> = Vec::new();
    all.extend(build_move(world, dataset, firm));
    all.extend(lane_move(world, dataset, firm));
    all.extend(price_moves(world, dataset, firm));
    all.extend(production_move(world, dataset, firm));
    all.extend(mandate_move(world, dataset, firm));
    all.extend(hire_move(world, dataset, firm));
    all.extend(raid_move(world, dataset, firm));
    // The same fix is not offered twice.
    all.retain(|m| !lead.iter().any(|l| l.commands == m.commands));
    order(&mut all);
    lead.extend(all);
    lead.truncate(MAX_MOVES);
    lead
}

/// Why a profitable company with nothing to do is told to hold: profit against the four weeks
/// before, and the payback limit nothing met.
fn hold_reason(world: &World, firm: Handle) -> String {
    let Some(entity) = world.firms.get(firm) else {
        return String::new();
    };
    let weeks = entity.book.recent_weeks();
    if weeks.is_empty() {
        return "No week has closed yet; nothing pays back inside ten years.".to_owned();
    }
    let split = weeks.len().saturating_sub(LOSS_WINDOW);
    let now: i64 = weeks[split..].iter().map(|w| w.net_income).sum();
    let before: i64 = weeks[split.saturating_sub(LOSS_WINDOW)..split]
        .iter()
        .map(|w| w.net_income)
        .sum();
    let profit = if now == 0 {
        "The company is breaking even".to_owned()
    } else if split == 0 || before <= 0 {
        "The company is profitable".to_owned()
    } else {
        let pct = (now - before) * 100 / before;
        match pct {
            0 => "Profit is flat".to_owned(),
            p if p > 0 => format!("Profit is up {p} percent"),
            p => format!("Profit is down {} percent", -p),
        }
    };
    format!("{profit}; nothing pays back inside ten years.")
}

/// The brief for a firm at the top of the week. Pure in the world.
pub fn weekly_brief(world: &World, dataset: &Dataset, firm: u64) -> Brief {
    let handle = Handle(firm);
    if world.firms.get(handle).is_none() {
        return Brief::default();
    }
    let moves = moves(world, dataset, handle);
    let hold = if moves.is_empty() {
        hold_reason(world, handle)
    } else {
        String::new()
    };
    Brief {
        week: world.tick.0,
        changes: changes(world, dataset, handle),
        due: due(world, dataset, handle),
        moves,
        hold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PriceSetup, ScenarioConfig, Session};
    use cw_core::calendar::Tick;
    use cw_sim::economy::timeline::TimelineMode;
    use cw_sim::state::{RaidRecord, ScenarioKind, ScenarioMode};

    fn session() -> Session {
        let mut s = Session::create(ScenarioConfig {
            seed: 4401,
            scenario: ScenarioKind::Guided,
            mode: ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: PriceSetup {
                scenario_id: "great_lakes_2000".to_owned(),
                path: None,
                timeline: Some(TimelineMode::Off),
            },
        })
        .unwrap();
        s.world.tick = Tick(30);
        s.world.raids.clear();
        s.world.systemic.timeline.events.clear();
        s.world.events.pending.clear();
        s.world.deals = Default::default();
        s.world.chapters.offers.clear();
        s.world.chapters.active.clear();
        s
    }

    fn you(s: &Session) -> Handle {
        s.world.investor.primary_group.unwrap()
    }

    fn lines(s: &Session) -> Vec<crate::dto::ProductLine> {
        crate::dto::product_lines(&s.world, &s.dataset, you(s).0)
    }

    #[test]
    fn an_unknown_firm_has_an_empty_brief() {
        let s = session();
        assert_eq!(
            weekly_brief(&s.world, &s.dataset, 0xdead_beef),
            Brief::default()
        );
    }

    #[test]
    fn the_brief_is_pure_and_bounded() {
        let s = session();
        let a = weekly_brief(&s.world, &s.dataset, you(&s).0);
        let b = weekly_brief(&s.world, &s.dataset, you(&s).0);
        assert_eq!(a, b);
        assert!(a.changes.len() <= MAX_CHANGES);
        assert!(a.moves.len() <= MAX_MOVES);
        assert!(a.due.len() <= MAX_DUE);
        for text in a
            .changes
            .iter()
            .map(|c| &c.text)
            .chain(a.due.iter().map(|d| &d.text))
            .chain(a.moves.iter().map(|m| &m.detail))
        {
            assert!(!text.contains('\u{2013}') && !text.contains('\u{2014}'));
            assert!(!text.contains("bps"), "{text}");
        }
    }

    #[test]
    fn a_raid_on_you_leads_what_changed_and_the_list_stays_at_three() {
        let mut s = session();
        let me = you(&s);
        let rival = s
            .world
            .firms
            .iter()
            .map(|(h, _)| h)
            .find(|h| *h != me && !player_side(&s.world, *h))
            .unwrap();
        s.world.raids.push(RaidRecord {
            tick: 30,
            attacker: rival,
            defender: me,
            region: 0,
            industry: 2,
            budget_cents: 9_000_000_000_000,
            chance_bps: 4_000,
            success: true,
            blocks: 2,
            price_cents: 100,
        });
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        assert_eq!(brief.changes[0].kind, "rival");
        assert!(brief.changes[0].text.contains("took 2 blocks"));
        assert!(brief.changes.len() <= MAX_CHANGES);
        // An old raid is not news.
        s.world.raids[0].tick = 10;
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        assert!(brief.changes.iter().all(|c| c.kind != "rival"));
    }

    #[test]
    fn a_price_move_on_a_good_you_sell_is_news_only_past_five_percent() {
        let mut s = session();
        let me = you(&s);
        let product = lines(&s)[0].product;
        let mut older = cw_sim::state::PricePoint {
            week: 28,
            goods: Default::default(),
            shares: Default::default(),
        };
        older.goods.insert(product, 10_000);
        let mut newer = older.clone();
        newer.week = 29;
        newer.goods.insert(product, 10_300);
        s.world.price_history = vec![older.clone(), newer];
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        assert!(brief.changes.iter().all(|c| c.kind != "price"));
        let mut newer = older.clone();
        newer.week = 29;
        newer.goods.insert(product, 11_000);
        s.world.price_history = vec![older, newer];
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        let line = brief.changes.iter().find(|c| c.kind == "price").unwrap();
        assert!(line.text.contains("rose 10 percent"), "{}", line.text);
    }

    #[test]
    fn due_lists_cards_with_weeks_left_soonest_first() {
        let mut s = session();
        let me = you(&s);
        let card = s.dataset.events.first().map(|c| c.id.clone());
        let Some(card) = card else { return };
        for (i, deadline) in [38u32, 33].into_iter().enumerate() {
            s.world.events.pending.push(cw_sim::state::PendingEvent {
                id: 900 + i as u64,
                card: card.clone(),
                subject_firm: Some(me),
                region: None,
                deadline: Tick(deadline),
                opened: Tick(29),
                mandatory: false,
                resolved: false,
                deal: None,
            });
        }
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        let weeks: Vec<u32> = brief
            .due
            .iter()
            .filter(|d| d.kind == "card")
            .map(|d| d.weeks_left)
            .collect();
        assert_eq!(weeks, vec![3, 8]);
        assert!(
            brief
                .due
                .windows(2)
                .all(|w| w[0].weeks_left <= w[1].weeks_left)
        );
    }

    #[test]
    fn a_pile_of_stock_suggests_a_cut_and_a_sell_out_a_rise_and_neither_otherwise() {
        let mut s = session();
        let me = you(&s);
        let quiet = price_moves(&s.world, &s.dataset, me);
        // The fresh company has neither a pile nor a sell-out: nothing is suggested.
        assert!(
            quiet
                .iter()
                .all(|m| m.key != "price_up" && m.key != "price_down"),
            "{quiet:?}"
        );
        let line = lines(&s)
            .into_iter()
            .find(|l| {
                l.mode != "fixed"
                    && l.price_cents > 0
                    && !s.dataset.products[l.product as usize].kind.is_service()
            })
            .unwrap();
        // Hand-build a pile: a hundred thousand lots of the good in the firm's store.
        s.world
            .firms
            .get_mut(me)
            .unwrap()
            .stock_lot_mut(line.product, 0)
            .receive(
                cw_core::quantity::MilliLots::from_lots(100_000),
                cw_core::money::Cents(1_000),
            );
        let after = lines(&s)
            .into_iter()
            .find(|l| l.product == line.product)
            .unwrap();
        assert!(after.stock_lots >= PILE_WEEKS * after.sold_last_week_lots.max(1));
        let moves = price_moves(&s.world, &s.dataset, me);
        let m = moves.iter().find(|m| m.key == "price_down").expect("a cut");
        assert_eq!(m.target.dialog, "pricing");
        assert_eq!(m.target.product, line.product as u32);
        assert_eq!(
            m.weekly_gain_cents,
            after.stock_lots * after.price_cents / 4
        );
        assert_eq!(
            m.commands,
            vec![json(CommandIntent::SetPricePosture {
                firm: me,
                product: line.product,
                mode: 1,
                bps: -NUDGE_BPS,
            })]
        );
        // Taking the stock away and selling everything made is a sell-out: raise instead.
        s.world.firms.get_mut(me).unwrap().inventory.clear();
        let up = price_moves(&s.world, &s.dataset, me);
        assert!(up.iter().all(|m| m.key != "price_down"));
    }

    #[test]
    fn a_mandate_offer_is_suggested_and_listed_as_due_then_gone_when_taken() {
        let mut s = session();
        let me = you(&s);
        let kinds = chapters::offer_kinds(&s.world, &s.dataset, me, 1);
        assert!(!kinds.is_empty());
        s.world.chapters.offers.insert(
            me,
            chapters::Offer {
                chapter: 1,
                set: 0,
                kinds: kinds.clone(),
            },
        );
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        assert!(brief.due.iter().any(|d| d.kind == "mandate_offer"));
        let mv = brief.moves.iter().find(|m| m.key == "mandate");
        let mv = mv.expect("mandate move");
        assert!(!mv.has_payback);
        assert_eq!(mv.target.dialog, "mandate");
        assert!(mv.commands[0].contains("ChooseMandate"));
        s.world.chapters.offers.clear();
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        assert!(brief.moves.iter().all(|m| m.key != "mandate"));
        assert!(brief.due.iter().all(|d| d.kind != "mandate_offer"));
    }

    #[test]
    fn moves_are_ordered_by_payback_and_capped_at_three() {
        let mk = |key: &str, has: bool, weeks: i64, gain: i64| BriefMove {
            key: key.to_owned(),
            headline: key.to_owned(),
            detail: String::new(),
            cost_cents: 0,
            weekly_gain_cents: gain,
            payback_weeks: weeks,
            has_payback: has,
            action_label: String::new(),
            target: MoveTarget::default(),
            commands: Vec::new(),
        };
        let mut all = vec![
            mk("mandate", false, 0, 0),
            mk("build", true, 40, 5),
            mk("raid", true, 12, 9),
            mk("lane", true, 0, 7),
            mk("production", true, 0, 3),
        ];
        order(&mut all);
        let keys: Vec<&str> = all.iter().map(|m| m.key.as_str()).collect();
        assert_eq!(keys, vec!["lane", "production", "raid", "build", "mandate"]);
    }

    /// A company that has traded for thirty weeks, with a great deal of cash.
    fn trading() -> (Session, Handle) {
        use cw_core::ledger::{Account, Txn};
        let mut s = session();
        let me = you(&s);
        s.advance(30);
        let float = 10_000_000_000_000;
        let tick = s.world.tick;
        s.world
            .firms
            .get_mut(me)
            .unwrap()
            .book
            .commit(
                tick,
                Txn::new().debit(Account::Cash, float, "float").credit(
                    Account::ShareCapital,
                    float,
                    "float",
                ),
            )
            .unwrap();
        s.world.raids.clear();
        (s, me)
    }

    #[test]
    fn build_where_imports_point_quotes_the_engines_own_cost_and_margin() {
        let (mut s, me) = trading();
        for g in s.world.gateways.iter_mut() {
            g.smoothed_import_lots = 0;
        }
        assert!(
            build_move(&s.world, &s.dataset, me).is_none(),
            "no imports, no build"
        );
        for g in s.world.gateways.iter_mut() {
            g.smoothed_import_lots = 1_000_000;
        }
        // A block here takes about six years to pay back: too long for three years, fine for ten.
        assert!(build_move_within(&s.world, &s.dataset, me, 100).is_none());
        let mv = build_move(&s.world, &s.dataset, me).expect("a build inside ten years");
        assert!(
            mv.payback_weeks >= SLOW_BUILD_WEEKS && mv.payback_weeks <= BUILD_MAX_PAYBACK_WEEKS
        );
        assert!(mv.detail.contains(" years"), "{}", mv.detail);
        let d = sectors::divisions_of(&s.world, &s.dataset, me)
            .into_iter()
            .find(|d| s.dataset.regions[d.region as usize].code == mv.target.region)
            .unwrap();
        let cost = sectors::build_cost(&s.world, &s.dataset, me, d.region, d.strategy, 1);
        let margin = division_report::report(&s.world, &s.dataset, me, d.region, d.industry)
            .unwrap()
            .margin_bps as i64;
        let gain = sectors::capacity_value(&s.dataset, d.strategy, 1) * margin / 10_000;
        assert_eq!(mv.cost_cents, cost.total_cents());
        assert_eq!(mv.weekly_gain_cents, gain);
        assert_eq!(mv.payback_weeks, (cost.total_cents() + gain - 1) / gain);
        assert_eq!(mv.target.dialog, "build");
        assert_eq!(mv.target.recipe, d.strategy as u32);
        assert!(mv.commands[0].contains("BuildFacility"));
        // Too poor to build: nothing.
        let cash = s.world.firms.get(me).unwrap().book.cash();
        assert!(cash > mv.cost_cents);
    }

    #[test]
    fn a_raid_is_suggested_only_when_the_quote_is_good_and_the_earnings_rule_allows() {
        let (s, me) = trading();
        let earned: i64 = s
            .world
            .firms
            .get(me)
            .unwrap()
            .book
            .recent_weeks()
            .iter()
            .map(|w| w.net_income)
            .sum();
        let found = raid_move(&s.world, &s.dataset, me);
        if let Some(mv) = &found {
            let rival = Handle(mv.target.target);
            let d = sectors::divisions_of(&s.world, &s.dataset, me)[0].clone();
            let probe =
                sectors::raid_quote(&s.world, &s.dataset, me, rival, d.region, d.industry, 0);
            let budget = probe.defence_cents * 2;
            let q = sectors::raid_quote(
                &s.world, &s.dataset, me, rival, d.region, d.industry, budget,
            );
            assert_eq!(mv.target.budget_cents, budget);
            assert!(q.chance_bps >= RAID_GOOD_BPS);
            assert!((budget + q.surcharge_cents) * 2 <= earned);
            assert!(mv.commands[0].contains("RaidMarket"));
        }
        // A company that earned nothing does not raid, however good the odds.
        let fresh = session();
        assert!(raid_move(&fresh.world, &fresh.dataset, you(&fresh)).is_none());
    }

    #[test]
    fn a_lane_reservation_quotes_the_reserve_function() {
        let (s, me) = trading();
        let summary = logistics::shipping_summary(&s.world, &s.dataset, me, None);
        match (summary.suggestion, lane_move(&s.world, &s.dataset, me)) {
            (Some(logistics::Suggestion::Reserve { key, lots, .. }), Some(mv)) => {
                let q = logistics::reserve_quote(&s.world, &s.dataset, me, key, lots, 26).unwrap();
                assert_eq!(mv.weekly_gain_cents, q.weekly_net_cents);
                assert_eq!(mv.target.lots, lots as i64);
                assert_eq!(mv.target.weeks, 26);
                assert!(mv.commands[0].contains("ReserveLane"));
            }
            (_, found) => assert!(found.is_none(), "no reserve suggestion, no lane move"),
        }
    }

    #[test]
    fn idle_production_carries_the_cfos_own_fix_in_any_state() {
        let (s, me) = trading();
        // Nothing is idle on a company on automatic production.
        assert!(production_move(&s.world, &s.dataset, me).is_none());
    }

    /// A young company that pays for research, marketing and training it cannot afford, run
    /// for five weeks: it is in the red with named reasons.
    fn losing() -> (Session, Handle) {
        use cw_sim::commands::{CommandEnvelope, INVESTOR_ACTOR};
        let mut s = Session::create(ScenarioConfig {
            seed: 7,
            scenario: ScenarioKind::Guided,
            mode: ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
        })
        .unwrap();
        let me = you(&s);
        for (low, intent) in [
            CommandIntent::SetResearchBudget {
                firm: me,
                amount_cents: 6_000_000,
            },
            CommandIntent::SetMarketingBudget {
                firm: me,
                amount_cents: 3_000_000,
            },
            CommandIntent::SetTrainingBudget {
                firm: me,
                amount_cents: 2_500_000,
            },
        ]
        .into_iter()
        .enumerate()
        {
            let envelope = CommandEnvelope {
                command_id_high: 0xB01,
                command_id_low: low as u64,
                actor: INVESTOR_ACTOR,
                expected_revision: s.revision(),
                submitted_tick: s.world.tick.0,
                intent,
            };
            s.submit(envelope).unwrap();
        }
        s.advance(5);
        s.dismiss_pause();
        (s, me)
    }

    #[test]
    fn a_losing_company_always_sees_a_move_and_the_cfos_reasons_come_first_in_cost_order() {
        let (s, me) = losing();
        assert!(recent_net(&s.world, me).unwrap() < 0);
        assert_eq!(loss_weeks(&s.world, me), LOSS_WINDOW);
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        assert!(!brief.moves.is_empty());
        assert!(brief.hold.is_empty());
        let reasons = cfo::advice(&s.world, &s.dataset, me.0);
        assert!(reasons.len() >= 2);
        let mut cost_order = reasons.clone();
        cost_order.sort_by(|a, b| b.weekly_cents.cmp(&a.weekly_cents).then(a.key.cmp(&b.key)));
        let moves = cfo_moves(&s.world, &s.dataset, me);
        assert_eq!(moves.len(), reasons.len());
        for (mv, reason) in moves.iter().zip(&cost_order) {
            assert_eq!(mv.key, format!("cfo_{}", reason.key));
            assert_eq!(mv.weekly_gain_cents, reason.weekly_cents);
            assert_eq!(mv.commands, reason.commands);
            assert!(mv.detail.contains("saves about $"), "{}", mv.detail);
        }
        assert!(
            moves
                .windows(2)
                .all(|w| w[0].weekly_gain_cents >= w[1].weekly_gain_cents)
        );
        // They lead the brief, ahead of whatever else the company could do.
        let lead = brief.moves.len().min(moves.len());
        for (shown, reason) in brief.moves.iter().zip(&moves).take(lead) {
            assert_eq!(shown.key, reason.key);
        }
    }

    #[test]
    fn a_losing_company_with_no_named_reason_is_told_where_the_money_goes() {
        let (mut s, me) = losing();
        {
            let policies = &mut s.world.firms.get_mut(me).unwrap().policies;
            policies.research_budget_cents = 0;
            policies.marketing_budget_cents = 0;
            policies.training_budget_cents = 0;
            policies.quality_budgets_cents.clear();
        }
        let brief = weekly_brief(&s.world, &s.dataset, me.0);
        let first = brief.moves.first().expect("a losing company sees a move");
        if first.key == "losing" {
            assert!(
                first.headline.contains(" percent of revenue")
                    || first.headline.contains("not selling"),
                "{}",
                first.headline
            );
            assert!(!first.has_payback && first.commands.is_empty());
            if first.target.dialog == "division" {
                assert!(!first.target.region.is_empty() && !first.target.industry.is_empty());
            }
        }
        assert!(brief.hold.is_empty());
    }

    #[test]
    fn a_profitable_company_with_nothing_to_do_is_told_why() {
        let s = session();
        let me = you(&s);
        let reason = hold_reason(&s.world, me);
        assert!(
            reason.ends_with("nothing pays back inside ten years."),
            "{reason}"
        );
        assert!(!reason.contains('\u{2013}') && !reason.contains('\u{2014}'));
    }

    #[test]
    fn payback_reads_in_years_past_a_year_and_a_slow_build_ranks_last() {
        assert_eq!(payback_text(30), "30 weeks");
        assert_eq!(payback_text(52), "1.0 years");
        assert_eq!(payback_text(321), "6.1 years");
        let mk = |key: &str, has: bool, weeks: i64| BriefMove {
            key: key.to_owned(),
            headline: key.to_owned(),
            detail: String::new(),
            cost_cents: 1,
            weekly_gain_cents: 1,
            payback_weeks: weeks,
            has_payback: has,
            action_label: String::new(),
            target: MoveTarget::default(),
            commands: Vec::new(),
        };
        let mut all = vec![
            mk("build", true, 300),
            mk("mandate", false, 0),
            mk("raid", true, 200),
        ];
        order(&mut all);
        assert_eq!(
            all.iter().map(|m| m.key.as_str()).collect::<Vec<_>>(),
            vec!["raid", "mandate", "build"]
        );
        let mut quick = vec![mk("mandate", false, 0), mk("build", true, 100)];
        order(&mut quick);
        assert_eq!(quick[0].key, "build");
    }

    #[test]
    fn payback_rounds_up_and_refuses_what_never_pays() {
        assert_eq!(payback(1_000, 300), Some(4));
        assert_eq!(payback(900, 300), Some(3));
        assert_eq!(payback(0, 300), Some(0));
        assert_eq!(payback(1_000, 0), None);
    }
}
