//! The paper: a weekly newspaper the engine writes from facts only.
//!
//! Every story is a sentence template over something the world recorded: a raid, a bid, a
//! strike, a rate decision, an election, an incident, a timeline event, a foreshadow notice or
//! the player's quarterly results. Nothing is invented and nothing is stored: a back issue is
//! recomputed from the histories the world already keeps, so it reads the same every time and
//! costs no save space. Facts older than the world keeps (a year of prices, a few dozen
//! political items) simply do not appear in old issues.
//!
//! Stories come from the issue's week and the three weeks before it. They are ranked by whether
//! the player is involved, then by how big the kind of fact is, then by how fresh and how large
//! it is. The top story leads, the next three are briefs.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::economy::events_engine::player_side;
use cw_sim::state::{DealKind, DealState, FirmStatus, World};

/// Longest headline, in characters.
pub const MAX_HEADLINE: usize = 70;
/// Briefs under the lead story.
pub const MAX_BRIEFS: usize = 3;
/// Weeks of facts an issue reads, counting its own.
const WINDOW: u32 = 4;
/// Back issues on offer: a year of weekly prices is what the world keeps.
pub const ARCHIVE_WEEKS: u32 = 52;
/// Most share and commodity moves the markets column shows.
const SHARE_MOVES: usize = 3;
const GOOD_MOVES: usize = 2;
/// Most classified lines.
const MAX_CLASSIFIEDS: usize = 6;

/// What a story opens in the app.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaperLink {
    /// company, region, good, deal, event or none.
    pub kind: String,
    /// The company handle, region index, good code or deal handle.
    pub id: u64,
    /// The timeline event id, for event links.
    pub key: String,
}

impl PaperLink {
    fn none() -> PaperLink {
        PaperLink {
            kind: "none".to_owned(),
            ..PaperLink::default()
        }
    }

    fn company(handle: Handle) -> PaperLink {
        PaperLink {
            kind: "company".to_owned(),
            id: handle.0,
            key: String::new(),
        }
    }

    fn region(index: u8) -> PaperLink {
        PaperLink {
            kind: "region".to_owned(),
            id: index as u64,
            key: String::new(),
        }
    }

    fn deal(handle: Handle) -> PaperLink {
        PaperLink {
            kind: "deal".to_owned(),
            id: handle.0,
            key: String::new(),
        }
    }

    fn event(id: &str) -> PaperLink {
        PaperLink {
            kind: "event".to_owned(),
            id: 0,
            key: id.to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Story {
    /// Stable id of the fact the story reports.
    pub id: String,
    pub headline: String,
    pub body: String,
    pub byline: String,
    pub link: PaperLink,
    /// The week the fact happened.
    pub week: u32,
    /// The player is party to it.
    pub yours: bool,
}

/// One share on the markets column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareMove {
    pub firm: u64,
    pub name: String,
    pub price_cents: i64,
    pub change_bps: i64,
}

/// One good on the markets column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoodMove {
    pub product: u16,
    pub name: String,
    pub price_cents: i64,
    pub change_bps: i64,
    /// Why the market screen says it moved. Filled by the caller that can compute drivers; empty
    /// in a bare paper.
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Markets {
    /// Whether the week has closing prices at all.
    pub open: bool,
    /// Market-cap weighted move of every listed share on the week, in basis points.
    pub index_change_bps: i64,
    pub shares: Vec<ShareMove>,
    pub goods: Vec<GoodMove>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classified {
    /// bid, restructuring, liquidation.
    pub kind: String,
    pub text: String,
    pub link: PaperLink,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paper {
    pub week: u32,
    pub masthead: String,
    pub date_line: String,
    pub lead: Option<Story>,
    pub briefs: Vec<Story>,
    pub markets: Markets,
    pub classifieds: Vec<Classified>,
}

/// The paper's name for a scenario's base id.
pub fn masthead_for(scenario_id: &str) -> &'static str {
    match scenario_id {
        "great-lakes-2000" => "The Corridor Times",
        "texas-1981" => "The Gulf Ledger",
        "silicon-valley-1995" => "The Valley Wire",
        "ruhr-rhine-1991" => "Rhein-Ruhr Kurier",
        _ => "The Weekly Ledger",
    }
}

/// Cut a headline to the limit on a word boundary.
pub fn clip(text: &str, max: usize) -> String {
    let text = text.trim().trim_end_matches('.');
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max).collect();
    let mut cut = match cut.rfind(' ') {
        Some(at) if at > max / 2 => cut[..at].to_owned(),
        _ => cut,
    };
    // Never end on a dangling joining word.
    loop {
        let trimmed = cut.trim_end_matches([',', ';', ':', ' ']);
        let last = trimmed.rsplit(' ').next().unwrap_or("");
        if matches!(
            last.to_lowercase().as_str(),
            "in" | "of"
                | "for"
                | "to"
                | "and"
                | "the"
                | "a"
                | "an"
                | "at"
                | "on"
                | "by"
                | "from"
                | "with"
        ) && trimmed.len() > last.len()
        {
            cut = trimmed[..trimmed.len() - last.len()].to_owned();
        } else {
            return trimmed.to_owned();
        }
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// The first one or two sentences of a text.
fn lead_sentences(text: &str, count: usize) -> String {
    let mut out = String::new();
    let mut seen = 0;
    for part in text.split_inclusive(". ") {
        out.push_str(part);
        seen += 1;
        if seen >= count {
            break;
        }
    }
    out.trim().to_owned()
}

fn money(world: &World, dataset: &Dataset, firm: Handle, cents: i64) -> String {
    let symbol = if world.fx.active {
        let index = cw_sim::economy::fx::currency_of(world, firm) as usize;
        dataset
            .currencies
            .as_ref()
            .and_then(|set| set.currencies.get(index))
            .map(|def| def.symbol.clone())
            .unwrap_or_else(|| "$".to_owned())
    } else {
        "$".to_owned()
    };
    let whole = cents.abs() / 100;
    let body = if whole >= 1_000_000_000 {
        format!(
            "{}.{} billion",
            whole / 1_000_000_000,
            whole % 1_000_000_000 / 100_000_000
        )
    } else if whole >= 1_000_000 {
        format!(
            "{}.{} million",
            whole / 1_000_000,
            whole % 1_000_000 / 100_000
        )
    } else if whole >= 10_000 {
        format!("{} thousand", whole / 1_000)
    } else {
        whole.to_string()
    };
    format!("{symbol}{body}")
}

fn percent(bps: i64) -> String {
    format!("{}.{:02}", bps.abs() / 100, bps.abs() % 100)
}

fn firm_name(world: &World, firm: Handle) -> Option<String> {
    world.firms.get(firm).map(|f| f.name.clone())
}

/// The firm whose name appears in a text, preferring the longest name.
fn firm_in(world: &World, text: &str) -> Option<Handle> {
    world
        .firms
        .iter()
        .filter(|(_, f)| f.name.len() >= 4 && text.contains(f.name.as_str()))
        .max_by_key(|(h, f)| (f.name.len(), std::cmp::Reverse(h.0)))
        .map(|(h, _)| h)
}

fn firm_by_exact_name(world: &World, name: &str) -> Option<Handle> {
    world
        .firms
        .iter()
        .find(|(_, f)| f.name == name)
        .map(|(h, _)| h)
}

fn region_name(world: &World, index: u8) -> String {
    world
        .regions
        .get(index as usize)
        .map(|r| r.name.clone())
        .unwrap_or_default()
}

/// A story before ranking.
struct Candidate {
    story: Story,
    /// How big this kind of fact is, 1 to 9.
    base: i64,
    /// Size of the money or capacity involved, in cents.
    size: i64,
}

impl Candidate {
    fn score(&self, week: u32) -> i64 {
        let age = week.saturating_sub(self.story.week) as i64;
        let yours = if self.story.yours { 1_000_000_000 } else { 0 };
        yours + self.base * 1_000_000 - age * 100_000 + (self.size / 10_000).clamp(0, 99_999)
    }
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    id: String,
    week: u32,
    headline: &str,
    body: String,
    byline: &str,
    link: PaperLink,
    yours: bool,
    base: i64,
    size: i64,
) -> Candidate {
    Candidate {
        story: Story {
            id,
            headline: clip(headline, MAX_HEADLINE),
            body,
            byline: byline.to_owned(),
            link,
            week,
            yours,
        },
        base,
        size,
    }
}

fn in_window(fact_week: u32, week: u32) -> bool {
    fact_week >= 1 && fact_week <= week && week - fact_week < WINDOW
}

pub fn weekly_paper(world: &World, dataset: &Dataset, week: u32) -> Paper {
    let week = week.clamp(1, world.tick.0.max(1));
    let mut found: Vec<Candidate> = Vec::new();
    raids(world, dataset, week, &mut found);
    rival_moves(world, week, &mut found);
    deals(world, dataset, week, &mut found);
    labour_and_news(world, week, &mut found);
    rate_decisions(world, week, &mut found);
    incidents(world, week, &mut found);
    timeline(world, dataset, week, &mut found);
    politics(world, week, &mut found);
    distress(world, week, &mut found);
    results(world, dataset, week, &mut found);

    found.sort_by(|a, b| {
        b.score(week)
            .cmp(&a.score(week))
            .then_with(|| a.story.id.cmp(&b.story.id))
    });
    found.dedup_by(|a, b| a.story.id == b.story.id);
    let mut stories = found.into_iter().map(|c| c.story);
    let lead = stories.next();
    let briefs = stories.take(MAX_BRIEFS).collect();

    let scenario = dataset.scenario_id.as_str();
    let at = Tick(week);
    Paper {
        week,
        masthead: masthead_for(scenario).to_owned(),
        date_line: format!(
            "Week {}, {}  Issue {}",
            at.week_of_year(),
            at.year_from(world.start_year),
            week
        ),
        lead,
        briefs,
        markets: markets(world, dataset, week),
        classifieds: classifieds(world, week),
    }
}

/// The weeks with a back issue, newest first.
pub fn archive_weeks(world: &World) -> Vec<u32> {
    let now = world.tick.0.max(1);
    let oldest = now.saturating_sub(ARCHIVE_WEEKS - 1).max(1);
    (oldest..=now).rev().collect()
}

/// The top headlines of the latest issue, for the ticker.
pub fn ticker(paper: &Paper) -> Vec<String> {
    paper
        .lead
        .iter()
        .chain(paper.briefs.iter())
        .take(3)
        .map(|s| s.headline.clone())
        .collect()
}

fn raids(world: &World, dataset: &Dataset, week: u32, out: &mut Vec<Candidate>) {
    for raid in world.raids.iter().filter(|r| in_window(r.tick, week)) {
        let (Some(attacker), Some(defender)) = (
            firm_name(world, raid.attacker),
            firm_name(world, raid.defender),
        ) else {
            continue;
        };
        let industry = cw_sim::state::INDUSTRY_NAMES
            .get(raid.industry as usize)
            .copied()
            .unwrap_or("business")
            .to_lowercase();
        let region = region_name(world, raid.region);
        let yours = player_side(world, raid.attacker) || player_side(world, raid.defender);
        let (headline, body, base) = if raid.success {
            (
                format!("{attacker} raids {defender}"),
                format!(
                    "{attacker} won {} block{} of {industry} business from {defender} in {region}.",
                    raid.blocks,
                    if raid.blocks == 1 { "" } else { "s" },
                ),
                8,
            )
        } else {
            (
                format!("{defender} beats off a raid by {attacker}"),
                format!(
                    "{attacker} spent {} on a raid on {defender}'s {industry} business in {region} and gained nothing.",
                    money(world, dataset, raid.attacker, raid.budget_cents)
                ),
                6,
            )
        };
        out.push(candidate(
            format!("raid:{}:{}:{}", raid.tick, raid.attacker.0, raid.defender.0),
            raid.tick,
            &headline,
            body,
            "Business desk",
            PaperLink::company(raid.defender),
            yours,
            base,
            raid.budget_cents,
        ));
    }
}

fn rival_moves(world: &World, week: u32, out: &mut Vec<Candidate>) {
    for mv in world
        .rivalry
        .moves
        .iter()
        .filter(|m| in_window(m.week, week))
    {
        let Some(name) = firm_name(world, mv.firm) else {
            continue;
        };
        let (headline, body, base) = match mv.class.as_str() {
            // Bids are reported from the deal itself, so a rival's logged bid adds nothing.
            "takeover" | "integrate" => continue,
            "venture" => (
                format!("{name} forms a venture"),
                format!("{name} {}", lower_first(&mv.text)),
                3,
            ),
            "build" => (
                format!("{name} breaks ground on a plant"),
                format!("{name} {}", lower_first(&mv.text)),
                2,
            ),
            _ if mv.text.starts_with("Wound itself down") => (
                format!("{name} winds itself down"),
                format!("{name} {}", lower_first(&mv.text)),
                7,
            ),
            _ => continue,
        };
        out.push(candidate(
            format!("move:{}:{}:{}", mv.week, mv.firm.0, mv.class),
            mv.week,
            &headline,
            body,
            "Business desk",
            PaperLink::company(mv.firm),
            player_side(world, mv.firm),
            base,
            0,
        ));
    }
}

fn deals(world: &World, dataset: &Dataset, week: u32, out: &mut Vec<Candidate>) {
    for (handle, deal) in world.deals.iter() {
        if matches!(deal.state, DealState::Draft | DealState::Funded) {
            continue;
        }
        // A deal records when it started and its offer deadline but not the week it ended, so a
        // finished deal is dated by its deadline.
        let at = if deal.state.terminal() {
            deal.offer_deadline.0.max(deal.started.0)
        } else {
            deal.started.0
        };
        if !in_window(at, week) {
            continue;
        }
        let (Some(buyer), Some(target)) =
            (firm_name(world, deal.buyer), firm_name(world, deal.target))
        else {
            continue;
        };
        let kind = match deal.kind {
            DealKind::SharePurchase => "takeover",
            DealKind::AssetPurchase => "asset purchase",
            DealKind::Merger => "merger",
        };
        let hostile = if deal.hostile { "hostile " } else { "" };
        let price = money(world, dataset, deal.buyer, deal.offer_price_cents);
        let (headline, body, base) = match deal.state {
            DealState::Settled => (
                format!("{buyer} wins {target}"),
                format!(
                    "The {hostile}{kind} of {target} by {buyer} went through at {price} a share."
                ),
                8,
            ),
            DealState::Rejected => (
                format!("Bid for {target} rejected"),
                format!("{target} turned down the {hostile}{kind} offer from {buyer}."),
                6,
            ),
            DealState::Withdrawn => (
                format!("{buyer} drops its bid for {target}"),
                format!("{buyer} withdrew its {hostile}{kind} offer for {target}."),
                5,
            ),
            DealState::Expired => (
                format!("Bid for {target} lapses"),
                format!("The {hostile}{kind} offer from {buyer} for {target} ran out of time."),
                5,
            ),
            _ => (
                format!("{buyer} bids for {target}"),
                format!(
                    "{buyer} has launched a {hostile}{kind} bid for {target} at {price} a share."
                ),
                7,
            ),
        };
        out.push(candidate(
            format!("deal:{}:{:?}", handle.0, deal.state),
            at,
            &headline,
            body,
            "Business desk",
            PaperLink::deal(handle),
            player_side(world, deal.buyer) || player_side(world, deal.target),
            base,
            deal.escrow_cents,
        ));
    }
}

fn labour_and_news(world: &World, week: u32, out: &mut Vec<Candidate>) {
    for (i, item) in world
        .news
        .iter()
        .enumerate()
        .filter(|(_, n)| in_window(n.week.0, week))
    {
        let firm = firm_by_exact_name(world, &item.subject);
        let link = firm.map(PaperLink::company).unwrap_or_else(PaperLink::none);
        let yours = firm.is_some_and(|f| player_side(world, f));
        let id = format!("news:{}:{}", item.week.0, i);
        let s = &item.subject;
        let (headline, body, base) = if item.key == "news.strike" {
            (
                format!("Workers strike at {s}"),
                format!("Workers at {s} have walked out. Local demand is lower for four weeks."),
                6,
            )
        } else if item.key.starts_with("news.activist.") {
            (
                format!("Activist investor targets {s}"),
                format!("An activist campaign has opened against {s}."),
                5,
            )
        } else if item.key == "news.lobbying_backlash" {
            (
                format!("Lobbying backlash hits {s}"),
                format!("News of lobbying has hurt the reputation of {s}."),
                3,
            )
        } else if item.key == "fx_band_crisis" {
            (
                format!("{s} breaks its trading band"),
                format!(
                    "The {s} moved outside its trading band. The band has been widened to {} percent.",
                    percent(item.amount_cents)
                ),
                6,
            )
        } else {
            continue;
        };
        out.push(candidate(
            id,
            item.week.0,
            &headline,
            body,
            "Business desk",
            link,
            yours,
            base,
            0,
        ));
    }
}

fn rate_decisions(world: &World, week: u32, out: &mut Vec<Candidate>) {
    let cycle = &world.macro_state.cycle;
    for decision in cycle.decisions.iter().filter(|d| in_window(d.week, week)) {
        let rate = percent(decision.rate_bps as i64);
        let (verb, headline, base) = match decision.change_bps.signum() {
            1 => (
                "raised",
                format!("Central bank raises rates to {rate} percent"),
                6,
            ),
            -1 => (
                "cut",
                format!("Central bank cuts rates to {rate} percent"),
                6,
            ),
            _ => (
                "held",
                format!("Central bank holds rates at {rate} percent"),
                3,
            ),
        };
        let who = if decision.week >= cycle.chair.appointed_week && !cycle.chair.name.is_empty() {
            format!("Chair {}", cycle.chair.name)
        } else {
            "The central bank".to_owned()
        };
        let body = format!(
            "{who} {verb} the policy rate, now {rate} percent, citing {}.",
            decision.reason
        );
        out.push(candidate(
            format!("rate:{}", decision.week),
            decision.week,
            &headline,
            body,
            "Economy desk",
            PaperLink::none(),
            false,
            base,
            0,
        ));
    }
}

fn incidents(world: &World, week: u32, out: &mut Vec<Candidate>) {
    for (i, incident) in world
        .systemic
        .incidents
        .iter()
        .enumerate()
        .filter(|(_, x)| in_window(x.week, week))
    {
        if incident.kind == cw_sim::economy::systemic::TIMELINE {
            continue;
        }
        let headline = incident
            .text
            .split(':')
            .next()
            .unwrap_or(&incident.text)
            .to_owned();
        let mut body = incident.text.clone();
        let names: Vec<String> = incident
            .firms
            .iter()
            .filter_map(|f| firm_name(world, *f))
            .take(3)
            .collect();
        if !names.is_empty() {
            body.push_str(&format!(" Affected: {}.", names.join(", ")));
        }
        let region = (incident.a as usize) < world.regions.len();
        out.push(candidate(
            format!("incident:{}:{}", incident.week, i),
            incident.week,
            &headline,
            body,
            "News desk",
            if region {
                PaperLink::region(incident.a)
            } else {
                PaperLink::none()
            },
            incident.firms.iter().any(|f| player_side(world, *f)),
            5,
            incident.weeks as i64 * 1_000_000,
        ));
    }
}

fn timeline(world: &World, dataset: &Dataset, week: u32, out: &mut Vec<Candidate>) {
    let masthead = masthead_for(dataset.scenario_id.as_str());
    for event in &world.systemic.timeline.events {
        if event.fired_at != 0 && in_window(event.fired_at, week) {
            out.push(candidate(
                format!("event:{}", event.id),
                event.fired_at,
                &event.title,
                lead_sentences(&event.summary, 2),
                "Staff reporter",
                PaperLink::event(&event.id),
                false,
                8,
                0,
            ));
        } else if let Some(notice) = event.notice_tick()
            && in_window(notice, week)
            && event.tick > week
            && (event.fired_at == 0 || event.fired_at > week)
        {
            let away = event.tick - week;
            let when = if away <= 1 {
                "next week".to_owned()
            } else {
                format!("in about {away} weeks")
            };
            out.push(candidate(
                format!("hint:{}", event.id),
                notice,
                &format!("Analysts expect: {}", event.title),
                format!(
                    "Analysts at {masthead} expect this {when}. {}",
                    lead_sentences(&event.summary, 1)
                ),
                "Markets desk",
                PaperLink::event(&event.id),
                false,
                5,
                0,
            ));
        }
    }
}

fn politics(world: &World, week: u32, out: &mut Vec<Candidate>) {
    let timeline_texts: Vec<String> = world
        .systemic
        .timeline
        .events
        .iter()
        .map(|e| format!("{}. {}", e.title, e.summary))
        .collect();
    for (i, item) in world
        .politics
        .news
        .iter()
        .enumerate()
        .filter(|(_, n)| in_window(n.week, week))
    {
        let text = item.text.as_str();
        // Incidents, timeline events and their notices are reported from their own records.
        if text.starts_with("Coming ")
            || timeline_texts.iter().any(|t| t == text)
            || world.systemic.incidents.iter().any(|x| x.text == text)
        {
            continue;
        }
        let (base, desk) = if text.contains("election") {
            (7, "Politics desk")
        } else if text.starts_with("New laws") {
            (6, "Politics desk")
        } else if text.contains("central bank") {
            (4, "Economy desk")
        } else {
            (3, "News desk")
        };
        let first = text.split(". ").next().unwrap_or(text);
        let firm = firm_in(world, text);
        out.push(candidate(
            format!("politics:{}:{}", item.week, i),
            item.week,
            first,
            lead_sentences(text, 2),
            desk,
            firm.map(PaperLink::company).unwrap_or_else(PaperLink::none),
            firm.is_some_and(|f| player_side(world, f)),
            base,
            0,
        ));
    }
}

fn distress(world: &World, week: u32, out: &mut Vec<Candidate>) {
    for (handle, firm) in world.firms.iter() {
        if let FirmStatus::PastDue { since } = firm.status
            && in_window(since.0, week)
        {
            out.push(candidate(
                format!("pastdue:{}:{}", since.0, handle.0),
                since.0,
                &format!("{} misses a payment", firm.name),
                format!(
                    "{} missed a due payment. Its payouts are frozen while a grace period runs.",
                    firm.name
                ),
                "Business desk",
                PaperLink::company(handle),
                player_side(world, handle),
                6,
                0,
            ));
        }
    }
}

fn results(world: &World, dataset: &Dataset, week: u32, out: &mut Vec<Candidate>) {
    if !Tick(week).is_quarter_boundary() {
        return;
    }
    let Some(handle) = world.investor.primary_group else {
        return;
    };
    let Some(firm) = world.firms.get(handle) else {
        return;
    };
    let quarter: Vec<_> = firm
        .book
        .recent_weeks()
        .iter()
        .filter(|w| w.week.0 <= week && week - w.week.0 < cw_core::calendar::WEEKS_PER_QUARTER)
        .collect();
    if quarter.len() < 4 {
        return;
    }
    let revenue: i64 = quarter.iter().map(|w| w.revenue).sum();
    let net: i64 = quarter.iter().map(|w| w.net_income).sum();
    let (headline, verb) = if net >= 0 {
        (format!("{} posts a quarterly profit", firm.name), "earned")
    } else {
        (format!("{} posts a quarterly loss", firm.name), "lost")
    };
    out.push(candidate(
        format!("results:{week}"),
        week,
        &headline,
        format!(
            "{} took in {} over the quarter and {verb} {} after costs and tax.",
            firm.name,
            money(world, dataset, handle, revenue),
            money(world, dataset, handle, net.abs()),
        ),
        "Business desk",
        PaperLink::company(handle),
        true,
        9,
        revenue.abs(),
    ));
}

fn markets(world: &World, dataset: &Dataset, week: u32) -> Markets {
    let history = &world.price_history;
    let Some(now_at) = history.iter().rposition(|p| p.week <= week) else {
        return Markets::default();
    };
    if now_at == 0 {
        return Markets::default();
    }
    let (now, before) = (&history[now_at], &history[now_at - 1]);

    let mut shares: Vec<ShareMove> = Vec::new();
    let (mut cap_now, mut cap_before) = (0i128, 0i128);
    for (handle, price) in &now.shares {
        let Some(firm) = world.firms.get(*handle) else {
            continue;
        };
        if !firm.on_board() {
            continue;
        }
        let Some(prev) = before.shares.get(handle).copied().filter(|p| *p > 0) else {
            continue;
        };
        cap_now += *price as i128 * firm.shares.issued as i128;
        cap_before += prev as i128 * firm.shares.issued as i128;
        let change = (*price - prev) * 10_000 / prev;
        if change != 0 {
            shares.push(ShareMove {
                firm: handle.0,
                name: firm.name.clone(),
                price_cents: *price,
                change_bps: change,
            });
        }
    }
    shares.sort_by(|a, b| {
        b.change_bps
            .abs()
            .cmp(&a.change_bps.abs())
            .then_with(|| a.firm.cmp(&b.firm))
    });
    shares.truncate(SHARE_MOVES);
    let index_change_bps = if cap_before > 0 {
        ((cap_now - cap_before) * 10_000 / cap_before) as i64
    } else {
        0
    };

    let mut goods: Vec<GoodMove> = Vec::new();
    for (code, price) in &now.goods {
        let Some(product) = dataset.products.get(*code as usize) else {
            continue;
        };
        if product.reference_price_cents <= 0 || product.kind.is_financial() {
            continue;
        }
        let Some(prev) = before.goods.get(code).copied().filter(|p| *p > 0) else {
            continue;
        };
        let change = (*price - prev) * 10_000 / prev;
        if change != 0 {
            goods.push(GoodMove {
                product: *code,
                name: product.name.clone(),
                price_cents: *price,
                change_bps: change,
                reason: String::new(),
            });
        }
    }
    goods.sort_by(|a, b| {
        b.change_bps
            .abs()
            .cmp(&a.change_bps.abs())
            .then_with(|| a.product.cmp(&b.product))
    });
    goods.truncate(GOOD_MOVES);

    Markets {
        open: true,
        index_change_bps,
        shares,
        goods,
    }
}

fn classifieds(world: &World, week: u32) -> Vec<Classified> {
    let mut out: Vec<Classified> = Vec::new();
    for (handle, deal) in world.deals.iter() {
        let open = matches!(
            deal.state,
            DealState::Diligence | DealState::OfferOpen | DealState::Review
        );
        if !open || deal.started.0 > week || (week != world.tick.0 && deal.offer_deadline.0 < week)
        {
            continue;
        }
        let (Some(buyer), Some(target)) =
            (firm_name(world, deal.buyer), firm_name(world, deal.target))
        else {
            continue;
        };
        out.push(Classified {
            kind: "bid".to_owned(),
            text: format!(
                "OPEN BID. {buyer} for {target}. Offer open until week {}.",
                deal.offer_deadline.0
            ),
            link: PaperLink::deal(handle),
        });
    }
    // Company status is only known as of today.
    if week == world.tick.0 {
        for (handle, firm) in world.firms.iter() {
            match firm.status {
                FirmStatus::Restructuring { until } => out.push(Classified {
                    kind: "restructuring".to_owned(),
                    text: format!(
                        "NOTICE. {} is restructuring under court protection until week {}.",
                        firm.name, until.0
                    ),
                    link: PaperLink::company(handle),
                }),
                FirmStatus::Liquidating => out.push(Classified {
                    kind: "liquidation".to_owned(),
                    text: format!("NOTICE. {} is being wound up.", firm.name),
                    link: PaperLink::company(handle),
                }),
                _ => {}
            }
        }
    }
    out.truncate(MAX_CLASSIFIEDS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PriceSetup, ScenarioConfig, Session};
    use cw_sim::economy::timeline::{LiveEvent, TimelineMode};
    use cw_sim::state::{RaidRecord, ScenarioKind, ScenarioMode};

    fn session(scenario: &str) -> Session {
        Session::create(ScenarioConfig {
            seed: 4401,
            scenario: ScenarioKind::Guided,
            mode: ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: PriceSetup {
                scenario_id: scenario.to_owned(),
                path: None,
                timeline: Some(TimelineMode::Off),
            },
        })
        .unwrap()
    }

    fn quiet(scenario: &str, weeks: u32) -> Session {
        let mut s = session(scenario);
        s.world.tick = Tick(weeks);
        s.world.raids.clear();
        s.world.news.clear();
        s.world.politics.news.clear();
        s.world.systemic.incidents.clear();
        s.world.systemic.timeline.events.clear();
        s.world.macro_state.cycle.decisions.clear();
        s.world.rivalry.moves.clear();
        s
    }

    fn two_firms(s: &Session) -> (Handle, Handle) {
        let mut handles: Vec<Handle> = s
            .world
            .firms
            .iter()
            .filter(|(h, _)| !player_side(&s.world, *h))
            .map(|(h, _)| h)
            .collect();
        handles.sort();
        (handles[0], handles[1])
    }

    #[test]
    fn mastheads_follow_the_scenario() {
        assert_eq!(masthead_for("great-lakes-2000"), "The Corridor Times");
        assert_eq!(masthead_for("texas-1981"), "The Gulf Ledger");
        assert_eq!(masthead_for("silicon-valley-1995"), "The Valley Wire");
        assert_eq!(masthead_for("ruhr-rhine-1991"), "Rhein-Ruhr Kurier");
    }

    #[test]
    fn a_quiet_week_is_a_short_paper() {
        let s = quiet("great-lakes-2000", 30);
        let paper = weekly_paper(&s.world, &s.dataset, 30);
        assert_eq!(paper.masthead, "The Corridor Times");
        assert!(
            paper.lead.is_none(),
            "nothing happened, so nothing is made up"
        );
        assert!(paper.briefs.is_empty());
        assert!(ticker(&paper).is_empty());
    }

    #[test]
    fn a_raid_makes_the_lead() {
        let mut s = quiet("great-lakes-2000", 30);
        let (a, d) = two_firms(&s);
        s.world.raids.push(RaidRecord {
            tick: 30,
            attacker: a,
            defender: d,
            region: 0,
            industry: 2,
            budget_cents: 500_000_000,
            chance_bps: 4_000,
            success: true,
            blocks: 2,
            price_cents: 100,
        });
        let paper = weekly_paper(&s.world, &s.dataset, 30);
        let lead = paper.lead.expect("a raid leads a quiet week");
        let (an, dn) = (
            s.world.firms.get(a).unwrap().name.clone(),
            s.world.firms.get(d).unwrap().name.clone(),
        );
        assert!(lead.headline.contains(&an) && lead.headline.contains(&dn));
        assert!(lead.headline.chars().count() <= MAX_HEADLINE);
        assert!(lead.body.contains("2 blocks"), "{}", lead.body);
        assert_eq!(lead.link.kind, "company");
        assert_eq!(lead.link.id, d.0);
        // The same raid is still news three weeks later and gone after that.
        let mut later = s.world.clone();
        later.tick = Tick(33);
        assert!(weekly_paper(&later, &s.dataset, 33).lead.is_some());
        later.tick = Tick(34);
        assert!(weekly_paper(&later, &s.dataset, 34).lead.is_none());
    }

    #[test]
    fn the_player_comes_before_size() {
        let mut s = quiet("great-lakes-2000", 30);
        let (a, d) = two_firms(&s);
        let you = s.world.investor.primary_group.unwrap();
        let raid = |attacker, defender, budget| RaidRecord {
            tick: 30,
            attacker,
            defender,
            region: 0,
            industry: 2,
            budget_cents: budget,
            chance_bps: 4_000,
            success: true,
            blocks: 1,
            price_cents: 100,
        };
        s.world.raids.push(raid(a, d, 900_000_000_000));
        s.world.raids.push(raid(a, you, 1_000_000));
        let paper = weekly_paper(&s.world, &s.dataset, 30);
        assert!(
            paper.lead.unwrap().yours,
            "a small raid on you outranks a big rival raid"
        );
        assert_eq!(paper.briefs.len(), 1);
    }

    #[test]
    fn a_timeline_notice_becomes_a_hint() {
        let mut s = quiet("texas-1981", 58);
        s.world.systemic.timeline.events.push(LiveEvent {
            id: "test-bank-failure".to_owned(),
            title: "Penn Square style bank failure".to_owned(),
            summary: "A large lender to the oil patch fails. Credit tightens.".to_owned(),
            tick: 64,
            foreshadow_weeks: 8,
            effects: Vec::new(),
            announced: true,
            fired_at: 0,
        });
        let paper = weekly_paper(&s.world, &s.dataset, 58);
        let lead = paper.lead.expect("the notice is news");
        assert_eq!(paper.masthead, "The Gulf Ledger");
        assert!(
            lead.headline.starts_with("Analysts expect"),
            "{}",
            lead.headline
        );
        assert!(
            lead.body
                .contains("Analysts at The Gulf Ledger expect this in about 6 weeks")
        );
        assert_eq!(lead.link.kind, "event");
        assert_eq!(lead.link.key, "test-bank-failure");
        assert_eq!(lead.byline, "Markets desk");
        // Once it fires it reports as a plain event, not a hint.
        s.world.systemic.timeline.events[0].fired_at = 64;
        s.world.tick = Tick(64);
        let fired = weekly_paper(&s.world, &s.dataset, 64).lead.unwrap();
        assert_eq!(fired.headline, "Penn Square style bank failure");
        assert!(!fired.body.contains("Analysts"));
    }

    #[test]
    fn rate_decisions_carry_the_banks_reason() {
        let mut s = quiet("great-lakes-2000", 30);
        s.world
            .macro_state
            .cycle
            .decisions
            .push(cw_sim::state::RateDecision {
                week: 30,
                change_bps: 25,
                rate_bps: 575,
                reason: "inflation running above target".to_owned(),
            });
        let lead = weekly_paper(&s.world, &s.dataset, 30).lead.unwrap();
        assert_eq!(lead.headline, "Central bank raises rates to 5.75 percent");
        assert!(lead.body.contains("citing inflation running above target"));
    }

    #[test]
    fn headlines_are_cut_to_the_limit_without_dashes() {
        let long = "A very long headline about a company doing something remarkable in a region that goes on";
        let cut = clip(long, MAX_HEADLINE);
        assert!(cut.chars().count() <= MAX_HEADLINE);
        assert!(!cut.ends_with(' '));
        for c in ['\u{2013}', '\u{2014}'] {
            assert!(!cut.contains(c));
        }
    }

    #[test]
    fn a_cut_headline_never_ends_on_a_joining_word() {
        let cut = clip(
            "Brazoria Assembly manufacturing raids Brazos Cattle and Grain in the region",
            64,
        );
        assert_eq!(
            cut,
            "Brazoria Assembly manufacturing raids Brazos Cattle and Grain"
        );
    }

    #[test]
    fn a_played_career_makes_a_paper_with_markets_and_no_dashes() {
        let mut s = session("great-lakes-2000");
        s.advance(30);
        let now = s.world.tick.0;
        let paper = weekly_paper(&s.world, &s.dataset, now);
        assert!(paper.markets.open, "prices have closed by week {now}");
        let again = weekly_paper(&s.world, &s.dataset, now);
        assert_eq!(paper, again, "deterministic");
        let mut text = paper.masthead.clone() + &paper.date_line;
        for story in paper.lead.iter().chain(paper.briefs.iter()) {
            assert!(story.headline.chars().count() <= MAX_HEADLINE);
            text += &story.headline;
            text += &story.body;
        }
        for c in ['\u{2013}', '\u{2014}'] {
            assert!(!text.contains(c));
        }
        assert_eq!(archive_weeks(&s.world).first(), Some(&now));
    }
}
