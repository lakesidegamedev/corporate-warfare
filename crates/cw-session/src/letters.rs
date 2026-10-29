//! Letters: the inbox as correspondence from the people in the player's world.
//!
//! Every letter is a few sentences over a fact the world recorded: a raid, a bid, a rate
//! decision, a new law, a strike, a quarter of results, a foreshadow notice or a decision card.
//! Nothing is invented and nothing but the read flags is stored: letters are recomputed from the
//! same records the paper reads, so they read the same every time.
//!
//! The sender is the person who would write: a rival's chief executive (a cast member or a
//! generated chief), the central bank chair, a party leader or governor. Where no person exists
//! the letter comes from an office or a role. Temperament and tone pick the sentence frames from
//! small tables, chosen deterministically by letter id. The facts always come from the records.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_data::cast::extras;
use cw_data::{Dataset, Person, Temperament};
use cw_sim::economy::events_engine::player_side;
use cw_sim::economy::rivalry::Cause;
use cw_sim::state::{DealKind, DealState, HolderId, World};

use crate::paper::{self, PaperLink};

/// Longest letter, in characters, greeting and sign-off excluded.
pub const MAX_CHARS: usize = 600;
/// Weeks the Inbox shows; older letters sit in the archive.
pub const INBOX_WEEKS: u32 = 8;
/// Weeks of letters the engine can still write.
pub const ARCHIVE_WEEKS: u32 = 26;
/// Longest summary line, in characters.
const MAX_SUMMARY: usize = 90;

/// A decision button on a letter that carries a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LetterChoice {
    pub id: String,
    pub label: String,
}

/// Who wrote the letter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sender {
    /// The portrait id (a cast or extras id), or empty when the sender is an office or a role.
    pub person: String,
    pub name: String,
    pub role: String,
    pub organisation: String,
    /// The sender's temperament key, or empty.
    pub temperament: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Letter {
    /// Stable id of the fact the letter reports.
    pub id: String,
    /// rival, ally, bid, rates, government, union, cfo, analyst or decision.
    pub kind: String,
    pub week: u32,
    pub sender: Sender,
    pub salutation: String,
    /// One to three short paragraphs.
    pub paragraphs: Vec<String>,
    pub sign_off: String,
    /// The fact in one line, for the list.
    pub summary: String,
    /// The decision card this letter carries, with its buttons.
    pub event: Option<u64>,
    pub choices: Vec<LetterChoice>,
    /// The card must be answered before time can advance.
    pub mandatory: bool,
    /// The week an answer is due, or zero.
    pub deadline: u32,
    /// What Open goes to.
    pub link: PaperLink,
    pub read: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Grudge,
    Ally,
    Formal,
}

impl Tone {
    const fn index(self) -> usize {
        match self {
            Tone::Grudge => 0,
            Tone::Ally => 1,
            Tone::Formal => 2,
        }
    }
}

const fn t_index(t: Temperament) -> usize {
    match t {
        Temperament::Integrator => 0,
        Temperament::Raider => 1,
        Temperament::Financier => 2,
        Temperament::Speculator => 3,
        Temperament::Builder => 4,
        Temperament::Cautious => 5,
    }
}

/// Opening sentences by temperament, tone and variant. Attitude only: never a fact.
pub(crate) const OPENERS: [[[&str; 2]; 3]; 6] = [
    // integrator
    [
        [
            "I take it personally when someone cuts into my links.",
            "I do not like what this does to our position.",
        ],
        [
            "I value what we are building together.",
            "Good partners make a long chain strong.",
        ],
        [
            "I am writing so there is a clear record.",
            "This concerns a matter between our firms.",
        ],
    ],
    // raider
    [
        ["I will be blunt.", "Take this as a warning."],
        [
            "I rarely say this, but you are good company in a fight.",
            "We make a useful pair.",
        ],
        ["Short note, no ceremony.", "Here is where things stand."],
    ],
    // financier
    [
        [
            "The numbers do not favour how you handled this.",
            "I keep accounts, and I have noted the cost.",
        ],
        [
            "The terms between us look sound to me.",
            "A well-structured arrangement is worth protecting.",
        ],
        [
            "For your records, the position is as follows.",
            "I will keep this to the figures.",
        ],
    ],
    // speculator
    [
        [
            "You made a bold move, and so will I.",
            "Fine. Now it gets interesting.",
        ],
        ["Together we can bet bigger.", "I like where this could go."],
        [
            "Quick note, since things are moving fast.",
            "Here is the latest.",
        ],
    ],
    // builder
    [
        [
            "I build things slowly, and I remember who tore at them.",
            "Taking another firm's plant is not how I do business.",
        ],
        [
            "It is good to work with someone who builds.",
            "I like firms that make real things.",
        ],
        [
            "A plain note about where we stand.",
            "I will say this directly.",
        ],
    ],
    // cautious
    [
        [
            "I would rather avoid conflict, but I cannot let this pass.",
            "This is a step I did not want to see.",
        ],
        [
            "I appreciate how steady our dealings have been.",
            "Careful partners are the best kind.",
        ],
        [
            "I want to put this in writing before anything else happens.",
            "Please read this with care.",
        ],
    ],
];

/// Closing sentences by temperament and tone.
pub(crate) const CLOSERS: [[&str; 3]; 6] = [
    [
        "I expect to hear from you.",
        "Let us keep our links strong.",
        "I look forward to your reply.",
    ],
    [
        "We will meet again in this market.",
        "Call me when you want to move.",
        "That is all.",
    ],
    [
        "I will remember this when terms come up.",
        "I expect we will deal again.",
        "Please keep this on file.",
    ],
    [
        "Let us see who wins the next round.",
        "Call me when the next one comes up.",
        "More soon.",
    ],
    [
        "I will rebuild, and I will remember.",
        "Let us build more.",
        "That is where we stand.",
    ],
    [
        "I hope we can avoid worse.",
        "I hope our good relations continue.",
        "Thank you for your attention.",
    ],
];

/// Sign-offs by temperament, two each.
pub(crate) const SIGN_OFFS: [[&str; 2]; 6] = [
    ["Yours in business", "With regards"],
    ["Regards", "Be seeing you"],
    ["Yours faithfully", "Sincerely"],
    ["Best", "Talk soon"],
    ["Yours truly", "Respectfully"],
    ["Respectfully", "With care"],
];

/// Salutations by tone, two each. `{}` is the firm or the plain title.
pub(crate) const SALUTATIONS: [[&str; 2]; 3] = [
    ["To the chief executive of {},", "To whoever runs {},"],
    ["Dear colleague at {},", "Dear partner at {},"],
    [
        "To the chief executive of {},",
        "Dear chief executive of {},",
    ],
];

/// Opening and closing lines for a sender who is an office or a role, with no temperament,
/// by kind of letter.
pub(crate) const OFFICE_FRAMES: [(&str, &str, &str); 9] = [
    (
        "decision",
        "This is an official notice for your attention.",
        "Please let us have your answer in good time.",
    ),
    (
        "rates",
        "This is the bank's note on the latest decision.",
        "We will write again after the next meeting.",
    ),
    (
        "government",
        "I am writing about a change that affects your business.",
        "My office is open to your questions.",
    ),
    (
        "union",
        "This is a notice on behalf of the workers.",
        "We would like to talk.",
    ),
    (
        "cfo",
        "A short memo on the books.",
        "Tell me if you want to act on this.",
    ),
    (
        "analyst",
        "A note from the desk on what we are watching.",
        "We will keep you posted.",
    ),
    (
        "rival",
        "I am writing on behalf of our firm.",
        "Please read this with care.",
    ),
    (
        "ally",
        "I am writing on behalf of our firm.",
        "We value the connection.",
    ),
    (
        "bid",
        "I am writing on behalf of our firm.",
        "Please read this with care.",
    ),
];

fn office_frames(kind: &str) -> (&'static str, &'static str) {
    OFFICE_FRAMES
        .iter()
        .find(|(k, _, _)| *k == kind)
        .map(|(_, o, c)| (*o, *c))
        .unwrap_or((OFFICE_FRAMES[0].1, OFFICE_FRAMES[0].2))
}

fn fnv(text: &str, salt: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn pick<'a>(id: &str, salt: u64, options: &[&'a str]) -> &'a str {
    options[(fnv(id, salt) % options.len() as u64) as usize]
}

/// No em or en dashes in anything the player reads, whatever the source text holds.
fn plain(text: &str) -> String {
    text.replace(" \u{2014} ", ", ")
        .replace(" \u{2013} ", ", ")
        .replace(['\u{2014}', '\u{2013}'], ", ")
}

/// The chief executive of a firm, named: a cast member if the engine appointed one, else a
/// generated chief picked from the name and the firm's style. Generated ones are spread so no
/// two firms in a world share one. The investor's own firms have none.
pub fn chief(world: &World, dataset: &Dataset, firm: Handle) -> Option<Person> {
    let entity = world.firms.get(firm)?;
    if let Some(person) = entity.ceo {
        return dataset.cast.get(person as usize).cloned();
    }
    if player_side(world, firm) {
        return None;
    }
    let pool = extras();
    if pool.is_empty() {
        return None;
    }
    let mut taken: Vec<usize> = Vec::new();
    let mut pick_at = None;
    for handle in world.firms.handles_sorted() {
        let Some(f) = world.firms.get(handle) else {
            continue;
        };
        if f.ceo.is_some() || player_side(world, handle) {
            continue;
        }
        let best = f
            .archetype
            .map(|a| {
                pool.iter()
                    .map(|e| cw_sim::economy::cast::temperament_match(a, e.temperament))
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let fits: Vec<usize> = (0..pool.len())
            .filter(|i| {
                f.archetype.is_none_or(|a| {
                    cw_sim::economy::cast::temperament_match(a, pool[*i].temperament) == best
                })
            })
            .collect();
        let mut at = (fnv(&f.name.to_lowercase(), 0) % fits.len() as u64) as usize;
        let mut chosen = fits[at];
        let mut tries = 0;
        while taken.contains(&chosen) && tries < fits.len() {
            at = (at + 1) % fits.len();
            chosen = fits[at];
            tries += 1;
        }
        if taken.contains(&chosen) {
            chosen = (0..pool.len())
                .find(|i| !taken.contains(i))
                .unwrap_or(chosen);
        }
        taken.push(chosen);
        if handle == firm {
            pick_at = Some(chosen);
            break;
        }
    }
    Some(pool[pick_at?].person())
}

/// A rival's chief at the time of a fact: nobody named before the current chief took the post.
fn chief_then(world: &World, dataset: &Dataset, firm: Handle, week: u32) -> Option<Person> {
    let appointed = world.cast.appointed.get(&firm).copied().unwrap_or(0);
    if world.firms.get(firm)?.ceo.is_some() && week < appointed {
        return None;
    }
    chief(world, dataset, firm)
}

fn person_sender(p: &Person, role: &str, organisation: &str) -> Sender {
    Sender {
        person: p.id.clone(),
        name: p.name.clone(),
        role: role.to_owned(),
        organisation: organisation.to_owned(),
        temperament: p.temperament.key().to_owned(),
    }
}

fn office_sender(name: &str, role: &str, organisation: &str) -> Sender {
    Sender {
        person: String::new(),
        name: name.to_owned(),
        role: role.to_owned(),
        organisation: organisation.to_owned(),
        temperament: String::new(),
    }
}

/// The sender of a rival firm's letter: its chief, or "Chief executive" when it has none then.
fn rival_sender(world: &World, dataset: &Dataset, firm: Handle, week: u32) -> Sender {
    let org = paper::firm_name(world, firm).unwrap_or_default();
    match chief_then(world, dataset, firm, week) {
        Some(p) => person_sender(&p, "Chief executive", &org),
        None => office_sender(
            &format!("The chief executive of {org}"),
            "Chief executive",
            &org,
        ),
    }
}

fn temperament_of(sender: &Sender) -> Option<Temperament> {
    Temperament::ALL
        .iter()
        .copied()
        .find(|t| t.key() == sender.temperament)
}

/// What a letter is built from, before the frames and the length limit.
struct Draft {
    id: String,
    kind: &'static str,
    week: u32,
    sender: Sender,
    tone: Tone,
    /// Who the letter is addressed to, for the greeting.
    addressee: String,
    /// The facts, one paragraph each; the first doubles as the summary.
    facts: Vec<String>,
    link: PaperLink,
}

struct Card {
    event: u64,
    choices: Vec<LetterChoice>,
    mandatory: bool,
    deadline: u32,
}

fn finish(world: &World, d: Draft, card: Option<Card>) -> Letter {
    let temperament = temperament_of(&d.sender);
    let (open, close, sign_off) = match temperament {
        Some(t) => {
            let i = t_index(t);
            (
                pick(&d.id, 1, &OPENERS[i][d.tone.index()]).to_owned(),
                CLOSERS[i][d.tone.index()].to_owned(),
                pick(&d.id, 3, &SIGN_OFFS[i]).to_owned(),
            )
        }
        None => {
            let (open, close) = office_frames(d.kind);
            (
                open.to_owned(),
                close.to_owned(),
                "Yours faithfully".to_owned(),
            )
        }
    };
    let salutation =
        SALUTATIONS[d.tone.index()][(fnv(&d.id, 2) % 2) as usize].replace("{}", &d.addressee);
    let mut facts = d.facts.iter().map(|f| plain(f));
    let first = facts.next().unwrap_or_default();
    let mut paragraphs = vec![format!("{open} {first}")];
    paragraphs.extend(facts);
    paragraphs.push(close);
    // Over the limit: the closing line goes first, then later facts, then the first is clipped.
    let total = |p: &[String]| p.iter().map(|s| s.chars().count()).sum::<usize>();
    while total(&paragraphs) > MAX_CHARS && paragraphs.len() > 1 {
        paragraphs.pop();
    }
    if total(&paragraphs) > MAX_CHARS {
        let clipped = paper::clip(&paragraphs[0], MAX_CHARS - 1);
        paragraphs[0] = format!("{clipped}.");
    }
    let summary = paper::clip(&plain(&d.facts[0]), MAX_SUMMARY);
    let read = world.mail.is_read(&d.id);
    let (event, choices, mandatory, deadline) = match card {
        Some(c) => (Some(c.event), c.choices, c.mandatory, c.deadline),
        None => (None, Vec::new(), false, 0),
    };
    Letter {
        id: d.id,
        kind: d.kind.to_owned(),
        week: d.week,
        sender: d.sender,
        salutation,
        paragraphs,
        sign_off,
        summary,
        event,
        choices,
        mandatory,
        deadline,
        link: d.link,
        read,
    }
}

fn industry_name(industry: u8) -> String {
    cw_sim::state::INDUSTRY_NAMES
        .get(industry as usize)
        .copied()
        .unwrap_or("business")
        .to_lowercase()
}

fn blocks_word(blocks: u8) -> String {
    if blocks == 1 {
        "1 block".to_owned()
    } else {
        format!("{blocks} blocks")
    }
}

fn raid_letters(world: &World, dataset: &Dataset, since: u32, out: &mut Vec<Letter>) {
    for raid in world.raids.iter().filter(|r| r.tick >= since) {
        let (Some(attacker), Some(defender)) = (
            paper::firm_name(world, raid.attacker),
            paper::firm_name(world, raid.defender),
        ) else {
            continue;
        };
        let (you_attack, you_defend) = (
            player_side(world, raid.attacker),
            player_side(world, raid.defender),
        );
        if you_attack == you_defend {
            continue;
        }
        let industry = industry_name(raid.industry);
        let region = paper::region_name(world, raid.region);
        let (writer, yours) = if you_attack {
            (raid.defender, &attacker)
        } else {
            (raid.attacker, &defender)
        };
        let fact = match (you_attack, raid.success) {
            (true, true) => format!(
                "{yours} took {} of {industry} business from us in {region}.",
                blocks_word(raid.blocks)
            ),
            (true, false) => format!(
                "{yours} spent {} on a raid on our {industry} business in {region} and gained nothing.",
                paper::money(world, dataset, raid.attacker, raid.budget_cents)
            ),
            (false, true) => format!(
                "We took {} of {industry} business from {yours} in {region}.",
                blocks_word(raid.blocks)
            ),
            // A failed raid on you is not a letter: the raider has nothing to say.
            (false, false) => continue,
        };
        out.push(finish(
            world,
            Draft {
                id: format!("raid:{}:{}:{}", raid.tick, raid.attacker.0, raid.defender.0),
                kind: "rival",
                week: raid.tick,
                sender: rival_sender(world, dataset, writer, raid.tick),
                tone: Tone::Grudge,
                addressee: yours.clone(),
                facts: vec![fact],
                link: PaperLink::company(writer),
            },
            None,
        ));
    }
}

fn ally_letters(world: &World, dataset: &Dataset, since: u32, out: &mut Vec<Letter>) {
    for ((a, b), memory) in &world.rivalry.alliances {
        if memory.last < since || memory.score <= 0 {
            continue;
        }
        let other = match (a, b) {
            (HolderId::Investor, HolderId::Firm(f)) | (HolderId::Firm(f), HolderId::Investor) => *f,
            _ => continue,
        };
        let yours = world
            .investor
            .primary_group
            .and_then(|g| paper::firm_name(world, g))
            .unwrap_or_else(|| "your company".to_owned());
        let fact = match memory.cause {
            Cause::Venture => format!("Our joint venture with {yours} is up and running."),
            Cause::Deal => format!("We closed a deal with {yours}."),
            _ => continue,
        };
        out.push(finish(
            world,
            Draft {
                id: format!("ally:{}:{}:{}", other.0, memory.cause.key(), memory.last),
                kind: "ally",
                week: memory.last,
                sender: rival_sender(world, dataset, other, memory.last),
                tone: Tone::Ally,
                addressee: yours,
                facts: vec![fact],
                link: PaperLink::company(other),
            },
            None,
        ));
    }
}

fn bid_letters(world: &World, dataset: &Dataset, since: u32, out: &mut Vec<Letter>) {
    for (handle, deal) in world.deals.iter() {
        if !matches!(
            deal.state,
            DealState::Diligence | DealState::OfferOpen | DealState::Review
        ) || deal.started.0 < since
        {
            continue;
        }
        if !player_side(world, deal.target) || player_side(world, deal.buyer) {
            continue;
        }
        let (Some(buyer), Some(target)) = (
            paper::firm_name(world, deal.buyer),
            paper::firm_name(world, deal.target),
        ) else {
            continue;
        };
        let kind = match deal.kind {
            DealKind::SharePurchase => "takeover",
            DealKind::AssetPurchase => "asset purchase",
            DealKind::Merger => "merger",
        };
        let hostile = if deal.hostile { "hostile " } else { "" };
        let price = paper::money(world, dataset, deal.buyer, deal.offer_price_cents);
        let until = paper::date_phrase(world, deal.offer_deadline);
        let fact = format!(
            "{buyer} has launched a {hostile}{kind} bid for {target} at {price} a share. \
             The offer stays open until {until}."
        );
        out.push(finish(
            world,
            Draft {
                id: format!("bid:{}", handle.0),
                kind: "bid",
                week: deal.started.0,
                sender: rival_sender(world, dataset, deal.buyer, deal.started.0),
                tone: if deal.hostile {
                    Tone::Grudge
                } else {
                    Tone::Formal
                },
                addressee: target,
                facts: vec![fact],
                link: PaperLink::deal(handle),
            },
            None,
        ));
    }
}

fn rate_letters(world: &World, dataset: &Dataset, since: u32, out: &mut Vec<Letter>) {
    let cycle = &world.macro_state.cycle;
    let home = player_home(world);
    for decision in cycle.decisions.iter().filter(|d| d.week >= since) {
        let rate = paper::percent(decision.rate_bps as i64);
        let verb = match decision.change_bps.signum() {
            1 => "raised",
            -1 => "cut",
            _ => "held",
        };
        let mut fact = format!("The bank {verb} the policy rate, now {rate} percent");
        if decision.change_bps != 0 {
            fact.push_str(&format!(
                ", a move of {} basis points",
                decision.change_bps.abs()
            ));
        }
        fact.push_str(&format!(
            ". Our reason: {}.",
            paper::lower_first(decision.reason.trim_end_matches('.'))
        ));
        let in_chair = decision.week >= cycle.chair.appointed_week;
        let sender = match (in_chair, cycle.chair.person) {
            (true, Some(p)) => dataset
                .cast
                .get(p as usize)
                .map(|person| person_sender(person, "Chair of the central bank", "Central bank"))
                .unwrap_or_else(|| {
                    office_sender(
                        &cycle.chair.name,
                        "Chair of the central bank",
                        "Central bank",
                    )
                }),
            _ => office_sender(
                if in_chair && !cycle.chair.name.is_empty() {
                    &cycle.chair.name
                } else {
                    "The central bank"
                },
                "Chair of the central bank",
                "Central bank",
            ),
        };
        out.push(finish(
            world,
            Draft {
                id: format!("rate:{}", decision.week),
                kind: "rates",
                week: decision.week,
                sender,
                tone: Tone::Formal,
                addressee: home.clone(),
                facts: vec![fact],
                link: PaperLink::none(),
            },
            None,
        ));
    }
}

/// The player's main company, for greetings.
fn player_home(world: &World) -> String {
    world
        .investor
        .primary_group
        .and_then(|g| paper::firm_name(world, g))
        .unwrap_or_else(|| "your company".to_owned())
}

/// The governor of the state the player's headquarters sits in, if the cast names one.
fn home_governor<'a>(world: &World, dataset: &'a Dataset) -> Option<(&'a Person, String)> {
    let firm = world.firms.get(world.investor.primary_group?)?;
    let region = dataset.regions.get(firm.region as usize)?;
    for code in &region.states {
        let at = dataset.states.iter().position(|s| &s.code == code)?;
        if let Some(p) = world.cast.governors.get(&(at as u8)) {
            return Some((
                dataset.cast.get(*p as usize)?,
                dataset.states[at].name.clone(),
            ));
        }
    }
    None
}

fn government_letters(
    world: &World,
    dataset: &Dataset,
    stories: &[paper::Story],
    since: u32,
    out: &mut Vec<Letter>,
) {
    for story in stories.iter().filter(|s| s.week >= since) {
        if !story.id.starts_with("politics:") {
            continue;
        }
        let laws = story.headline.starts_with("New laws");
        let election = story.body.contains("election");
        if !laws && !election {
            continue;
        }
        let addressee = player_home(world);
        let governor = if laws {
            home_governor(world, dataset)
        } else {
            None
        };
        let (sender, tone) = if let Some((p, state)) = governor {
            (
                person_sender(p, &format!("Governor of {state}"), &state),
                Tone::Formal,
            )
        } else if let Some(p) =
            cw_sim::economy::cast::country_leader(world).and_then(|p| dataset.cast.get(p as usize))
        {
            let party = cw_sim::economy::politics::party_name(world.politics.party)
                .trim_start_matches("the ")
                .to_owned();
            (
                person_sender(p, "Leader of the government", &party),
                Tone::Formal,
            )
        } else {
            (
                office_sender("The government", "Office of the government", "Government"),
                Tone::Formal,
            )
        };
        out.push(finish(
            world,
            Draft {
                id: story.id.clone(),
                kind: "government",
                week: story.week,
                sender,
                tone,
                addressee,
                facts: vec![story.body.clone()],
                link: story.link.clone(),
            },
            None,
        ));
    }
}

fn union_letters(world: &World, since: u32, out: &mut Vec<Letter>) {
    for (i, item) in world.news.iter().enumerate() {
        if item.key != "news.strike" || item.week.0 < since {
            continue;
        }
        let Some((handle, _)) = world.firms.iter().find(|(_, f)| f.name == item.subject) else {
            continue;
        };
        if !player_side(world, handle) {
            continue;
        }
        let s = &item.subject;
        out.push(finish(
            world,
            Draft {
                id: format!("news:{}:{}", item.week.0, i),
                kind: "union",
                week: item.week.0,
                sender: office_sender(
                    "Union representative",
                    "Union representative",
                    &format!("Workers at {s}"),
                ),
                tone: Tone::Formal,
                addressee: s.clone(),
                facts: vec![format!(
                    "Workers at {s} have walked out. Local demand is lower for four weeks."
                )],
                link: PaperLink::company(handle),
            },
            None,
        ));
    }
}

fn analyst_letters(
    world: &World,
    dataset: &Dataset,
    stories: &[paper::Story],
    since: u32,
    out: &mut Vec<Letter>,
) {
    let masthead = paper::masthead_for(dataset.scenario_id.as_str());
    for story in stories.iter().filter(|s| s.week >= since) {
        let Some(key) = story.id.strip_prefix("hint:") else {
            continue;
        };
        let Some(event) = world.systemic.timeline.events.iter().find(|e| e.id == key) else {
            continue;
        };
        let away = event.tick.saturating_sub(story.week);
        let when = if away <= 1 {
            "next week".to_owned()
        } else {
            format!("in about {away} weeks")
        };
        out.push(finish(
            world,
            Draft {
                id: story.id.clone(),
                kind: "analyst",
                week: story.week,
                sender: office_sender("Markets analyst", "Analyst", masthead),
                tone: Tone::Formal,
                addressee: player_home(world),
                facts: vec![format!(
                    "We expect this {when}: {}. {}",
                    event.title,
                    paper::lead_sentences(&event.summary, 1)
                )],
                link: story.link.clone(),
            },
            None,
        ));
    }
}

/// How much of a quarter's sales is profit before a quarter counts as strong, in percent.
const STRONG_MARGIN_PCT: i64 = 10;

fn cfo_letters(world: &World, dataset: &Dataset, since: u32, out: &mut Vec<Letter>) {
    let Some(handle) = world.investor.primary_group else {
        return;
    };
    let Some(firm) = world.firms.get(handle) else {
        return;
    };
    let now = world.tick.0;
    let sender = office_sender(
        "Your chief financial officer",
        "Chief financial officer",
        &firm.name,
    );
    let weeks = firm.book.recent_weeks();
    // A strong quarter: a short good-news memo at each quarter boundary the books still hold.
    for boundary in (since.max(1)..=now).filter(|w| Tick(*w).is_quarter_boundary()) {
        let quarter: Vec<_> = weeks
            .iter()
            .filter(|w| {
                w.week.0 <= boundary && boundary - w.week.0 < cw_core::calendar::WEEKS_PER_QUARTER
            })
            .collect();
        if quarter.len() < 4 {
            continue;
        }
        let revenue: i64 = quarter.iter().map(|w| w.revenue).sum();
        let net: i64 = quarter.iter().map(|w| w.net_income).sum();
        if net <= 0 || net * 100 < revenue * STRONG_MARGIN_PCT {
            continue;
        }
        out.push(finish(
            world,
            Draft {
                id: format!("cfo:q:{boundary}"),
                kind: "cfo",
                week: boundary,
                sender: sender.clone(),
                tone: Tone::Formal,
                addressee: firm.name.clone(),
                facts: vec![format!(
                    "A strong quarter. We took in {} and kept {} after costs and tax.",
                    paper::money(world, dataset, handle, revenue),
                    paper::money(world, dataset, handle, net),
                )],
                link: PaperLink::company(handle),
            },
            None,
        ));
    }
    // A losing company: the CFO's reasons, for the current four-week period only, since the
    // advice reads the policies as they stand.
    let advice = crate::cfo::advice(world, dataset, handle.0);
    if !advice.is_empty() && weeks.len() >= 2 {
        let recent = &weeks[weeks.len().saturating_sub(4)..];
        let net: i64 = recent.iter().map(|w| w.net_income).sum();
        if net < 0 && now >= since {
            let mut facts = vec![format!(
                "The company lost {} over the last four weeks.",
                paper::money(world, dataset, handle, net.abs())
            )];
            for item in advice.iter().take(2) {
                facts.push(format!(
                    "{}, at about {} a week.",
                    item.headline,
                    paper::money(world, dataset, handle, item.weekly_cents)
                ));
            }
            out.push(finish(
                world,
                Draft {
                    id: format!("cfo:{}", now / 4),
                    kind: "cfo",
                    week: now,
                    sender,
                    tone: Tone::Formal,
                    addressee: firm.name.clone(),
                    facts,
                    link: PaperLink::company(handle),
                },
                None,
            ));
        }
    }
}

/// The office that brings each kind of decision card: name, role.
fn card_office(card: &str) -> (&'static str, &'static str) {
    match card {
        "covenant_warning" | "bank_surge" => ("Your bank", "Credit department"),
        "insurance_catastrophe" => ("Your insurer", "Claims department"),
        "antitrust_review" => ("Competition authority", "Case officer"),
        "permit_delay" => ("Permits office", "Case officer"),
        "quality_recall" => ("Product safety office", "Case officer"),
        "training_grant" => ("Labour department", "Grants office"),
        "union_drive" | "wage_negotiation" | "strike_escalation" => {
            ("Union representative", "Union representative")
        }
        "supplier_default" => ("Purchasing manager", "Purchasing"),
        "hostile_stake" => ("Share register", "Registrar"),
        "talent_poaching" => ("Head of people", "Human resources"),
        "recession" | "demand_boom" | "lobbying_backlash" | "fuel_spike" => {
            ("Your economist", "Economics office")
        }
        _ => ("Your operations director", "Operations"),
    }
}

fn decision_letters(world: &World, dataset: &Dataset, since: u32, out: &mut Vec<Letter>) {
    for line in crate::dto::events(world, dataset) {
        let Some(pending) = world.events.pending.iter().find(|e| e.id == line.id) else {
            continue;
        };
        if pending.opened.0 < since && !pending.mandatory {
            continue;
        }
        let (name, role) = card_office(&line.card);
        let subject = line.subject.clone();
        // A tender offer comes from the bidder's own chief.
        let bidder = if line.card == "tender_offer" {
            pending
                .deal
                .and_then(|d| world.deals.get(d))
                .map(|d| d.buyer)
        } else {
            None
        };
        let organisation = if subject.is_empty() {
            name.to_owned()
        } else {
            subject.clone()
        };
        let sender = match bidder {
            Some(b) => rival_sender(world, dataset, b, pending.opened.0),
            None if name == "Union representative" => {
                office_sender(name, role, &format!("Workers at {organisation}"))
            }
            None => office_sender(name, role, &organisation),
        };
        let deadline_text = paper::date_phrase(world, Tick(line.deadline));
        let about = if subject.is_empty() {
            format!("{}.", line.title)
        } else {
            format!("{}, concerning {subject}.", line.title)
        };
        let options = line
            .choices
            .iter()
            .map(|c| c.label.trim_end_matches('.').to_owned())
            .collect::<Vec<_>>();
        let mut facts = vec![format!("{about} We need your answer by {deadline_text}.")];
        if !options.is_empty() {
            facts.push(format!("Your options: {}.", options.join(", or ")));
        }
        let link = match (bidder, pending.deal, pending.subject_firm) {
            (_, Some(d), _) => PaperLink::deal(d),
            (_, _, Some(f)) => PaperLink::company(f),
            _ => PaperLink::none(),
        };
        out.push(finish(
            world,
            Draft {
                id: format!("card:{}", line.id),
                kind: "decision",
                week: pending.opened.0,
                sender,
                tone: Tone::Formal,
                addressee: if subject.is_empty() {
                    player_home(world)
                } else {
                    subject
                },
                facts,
                link,
            },
            Some(Card {
                event: line.id,
                choices: line
                    .choices
                    .iter()
                    .map(|c| LetterChoice {
                        id: c.id.clone(),
                        label: c.label.clone(),
                    })
                    .collect(),
                mandatory: line.mandatory,
                deadline: line.deadline,
            }),
        ));
    }
}

/// Every letter dated `since_week` or later, decisions first (mandatory and soonest due on
/// top), then newest first. Decision letters are listed while their card waits whatever their
/// age.
pub fn letters(world: &World, dataset: &Dataset, since_week: u32) -> Vec<Letter> {
    let now = world.tick.0;
    let since = since_week.max(now.saturating_sub(ARCHIVE_WEEKS)).max(1);
    // Facts the paper reads, over every four-week window back to `since`.
    let mut stories: Vec<paper::Story> = Vec::new();
    let mut at = now.max(1);
    loop {
        stories.extend(paper::stories(world, dataset, at));
        if at <= since + 3 {
            break;
        }
        at -= 4;
    }
    stories.sort_by(|a, b| a.id.cmp(&b.id));
    stories.dedup_by(|a, b| a.id == b.id);

    let mut out: Vec<Letter> = Vec::new();
    raid_letters(world, dataset, since, &mut out);
    ally_letters(world, dataset, since, &mut out);
    bid_letters(world, dataset, since, &mut out);
    rate_letters(world, dataset, since, &mut out);
    government_letters(world, dataset, &stories, since, &mut out);
    union_letters(world, since, &mut out);
    analyst_letters(world, dataset, &stories, since, &mut out);
    cfo_letters(world, dataset, since, &mut out);
    decision_letters(world, dataset, 1, &mut out);

    out.sort_by(|a, b| {
        let pin = |l: &Letter| match (l.event.is_some(), l.mandatory) {
            (true, true) => 0,
            (true, false) => 1,
            _ => 2,
        };
        pin(a)
            .cmp(&pin(b))
            .then_with(|| {
                if a.event.is_some() && b.event.is_some() {
                    a.deadline.cmp(&b.deadline)
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .then_with(|| b.week.cmp(&a.week))
            .then_with(|| a.id.cmp(&b.id))
    });
    out.dedup_by(|a, b| a.id == b.id);
    out
}

/// Letters in the Inbox window that are unread: the badge.
pub fn unread_count(world: &World, dataset: &Dataset) -> usize {
    let since = world.tick.0.saturating_sub(INBOX_WEEKS);
    letters(world, dataset, since)
        .iter()
        .filter(|l| !l.read)
        .count()
}

/// Mark letters read by id. Unknown ids are ignored.
pub fn mark_read(world: &mut World, dataset: &Dataset, ids: &[String]) {
    let now = world.tick.0;
    let found = letters(world, dataset, 1);
    for id in ids {
        if let Some(l) = found.iter().find(|l| &l.id == id) {
            world.mail.mark(id, l.week, now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PriceSetup, ScenarioConfig, Session};
    use cw_core::ledger::{Account, Txn};
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
        s.world.rivalry.alliances.clear();
        s.world.events.pending.clear();
        s.world.deals = Default::default();
        s
    }

    fn rival(s: &Session, nth: usize) -> Handle {
        let mut handles: Vec<Handle> = s
            .world
            .firms
            .iter()
            .filter(|(_, f)| f.archetype.is_some())
            .map(|(h, _)| h)
            .filter(|h| !player_side(&s.world, *h))
            .collect();
        handles.sort();
        handles[nth]
    }

    fn you(s: &Session) -> Handle {
        s.world.investor.primary_group.unwrap()
    }

    fn raid(attacker: Handle, defender: Handle, success: bool) -> RaidRecord {
        RaidRecord {
            tick: 30,
            attacker,
            defender,
            region: 0,
            industry: 2,
            budget_cents: 500_000_000,
            chance_bps: 4_000,
            success,
            blocks: 3,
            price_cents: 100,
        }
    }

    fn text(l: &Letter) -> String {
        l.paragraphs.join(" ")
    }

    /// The rules every letter keeps, whatever its kind.
    fn check(l: &Letter) {
        let body = text(l);
        assert!(
            (1..=3).contains(&l.paragraphs.len()),
            "{}: {} paragraphs",
            l.id,
            l.paragraphs.len()
        );
        assert!(body.chars().count() <= MAX_CHARS, "{}: {body}", l.id);
        for part in [
            &body,
            &l.salutation,
            &l.sign_off,
            &l.summary,
            &l.sender.name,
        ] {
            for c in ['\u{2013}', '\u{2014}'] {
                assert!(!part.contains(c), "{}: dash in {part}", l.id);
            }
        }
        assert!(l.summary.chars().count() <= MAX_SUMMARY);
        assert!(!l.salutation.contains("{}"));
        assert!(!l.sender.name.is_empty() && !l.sender.role.is_empty());
    }

    /// The opening line of a person-sent letter comes from the sender's temperament row.
    fn assert_voice(l: &Letter, tone: Tone) {
        let t = temperament_of(&l.sender).expect("a person sender has a temperament");
        let row = &OPENERS[t_index(t)][tone.index()];
        assert!(
            row.iter().any(|o| l.paragraphs[0].starts_with(o)),
            "{}: opener not from the {} table: {}",
            l.id,
            t.key(),
            l.paragraphs[0]
        );
        assert!(
            SIGN_OFFS[t_index(t)].contains(&l.sign_off.as_str()),
            "{}",
            l.sign_off
        );
    }

    #[test]
    fn the_frame_tables_are_plain() {
        for row in OPENERS.iter().flatten().flatten() {
            assert!(!row.contains(['\u{2013}', '\u{2014}']) && row.chars().count() < 80);
        }
        for t in CLOSERS.iter().flatten().chain(SIGN_OFFS.iter().flatten()) {
            assert!(!t.contains(['\u{2013}', '\u{2014}']));
        }
        for (_, o, c) in OFFICE_FRAMES {
            assert!(!o.contains(['\u{2013}', '\u{2014}']) && !c.contains(['\u{2013}', '\u{2014}']));
        }
        for t in SALUTATIONS.iter().flatten() {
            assert!(!t.contains(['\u{2013}', '\u{2014}']));
        }
    }

    #[test]
    fn a_raid_on_you_is_a_letter_from_the_raider() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 0), you(&s));
        s.world.raids.push(raid(r, y, true));
        let found = letters(&s.world, &s.dataset, 25);
        let l = found.iter().find(|l| l.kind == "rival").expect("a letter");
        check(l);
        let chief = chief(&s.world, &s.dataset, r).unwrap();
        assert_eq!(l.sender.person, chief.id);
        assert_eq!(l.sender.name, chief.name);
        assert_eq!(l.sender.organisation, s.world.firms.get(r).unwrap().name);
        assert_voice(l, Tone::Grudge);
        let body = text(l);
        assert!(
            body.contains("3 blocks") && body.contains(&s.world.firms.get(y).unwrap().name),
            "{body}"
        );
        assert_eq!(l.link.kind, "company");
        assert_eq!(l.link.id, r.0);
        assert_eq!(l.week, 30);
        assert!(!l.read && l.event.is_none());
    }

    #[test]
    fn a_raid_by_you_is_a_letter_from_the_defender() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 1), you(&s));
        s.world.raids.push(raid(y, r, false));
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        check(&l);
        assert_eq!(l.sender.organisation, s.world.firms.get(r).unwrap().name);
        let body = text(&l);
        assert!(
            body.contains("gained nothing") && body.contains("$5.0 million"),
            "{body}"
        );
        // A failed raid on you is not a letter, and raids between two rivals are not either.
        let mut t = quiet("great-lakes-2000", 30);
        let (a, b) = (rival(&t, 0), rival(&t, 1));
        t.world.raids.push(raid(a, you(&t), false));
        t.world.raids.push(raid(a, b, true));
        assert!(letters(&t.world, &t.dataset, 25).is_empty());
    }

    #[test]
    fn a_joint_venture_is_a_friendly_letter() {
        let mut s = quiet("great-lakes-2000", 30);
        let r = rival(&s, 0);
        s.world.rivalry.alliances.insert(
            (HolderId::Investor, HolderId::Firm(r)),
            cw_sim::economy::rivalry::Memory {
                score: 5_000,
                cause: Cause::Venture,
                last: 29,
            },
        );
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        check(&l);
        assert_eq!(l.kind, "ally");
        assert_voice(&l, Tone::Ally);
        assert!(text(&l).contains("joint venture"));
        assert_eq!(l.sender.organisation, s.world.firms.get(r).unwrap().name);
    }

    #[test]
    fn a_bid_on_you_comes_from_the_bidder() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 0), you(&s));
        let h = s.world.deals.insert(cw_sim::state::Deal {
            buyer: r,
            target: y,
            kind: DealKind::SharePurchase,
            state: DealState::OfferOpen,
            escrow_cents: 1,
            offer_price_cents: 4_250,
            max_shares: 10,
            committed_shares: 0,
            started: Tick(29),
            diligence_until: Tick(29),
            offer_deadline: Tick(35),
            review_deadline: None,
            frozen_fair_cents: 4_000,
            review_required: false,
            remedy_share_bps: 0,
            hostile: true,
            diligence_paid: true,
            investor_tendered: false,
        });
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        check(&l);
        assert_eq!(l.kind, "bid");
        assert_voice(&l, Tone::Grudge);
        let body = text(&l);
        assert!(
            body.contains("hostile takeover bid") && body.contains("$42 a share"),
            "{body}"
        );
        assert!(body.contains("week "), "{body}");
        assert_eq!((l.link.kind.as_str(), l.link.id), ("deal", h.0));
    }

    #[test]
    fn a_rate_decision_comes_from_the_chair_with_the_banks_reason() {
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
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        check(&l);
        assert_eq!(l.kind, "rates");
        let body = text(&l);
        assert!(body.contains("raised") && body.contains("5.75 percent"));
        assert!(body.contains("inflation running above target"));
        assert_eq!(l.sender.role, "Chair of the central bank");
        match s.world.macro_state.cycle.chair.person {
            Some(p) if s.world.macro_state.cycle.chair.appointed_week <= 30 => {
                assert_eq!(l.sender.person, s.dataset.cast[p as usize].id);
                assert_voice(&l, Tone::Formal);
            }
            _ => assert!(l.sender.person.is_empty()),
        }
    }

    #[test]
    fn a_law_change_comes_from_a_governor_or_the_leader() {
        let mut s = quiet("great-lakes-2000", 30);
        let law = "New laws in force: corporate tax 28%, low-skill wage floor $4/hour, merger review at 25% share, standard union law; Business party.";
        s.world.politics.news.push(cw_sim::state::PoliticalNews {
            week: 30,
            text: law.to_owned(),
            person: None,
        });
        let l = letters(&s.world, &s.dataset, 25)
            .into_iter()
            .find(|l| l.kind == "government")
            .expect("a letter");
        check(&l);
        assert!(text(&l).contains("corporate tax 28%"), "{}", text(&l));
        assert!(
            l.sender.role.starts_with("Governor of") || l.sender.role == "Leader of the government",
            "{}",
            l.sender.role
        );
        assert_voice(&l, Tone::Formal);
        // An election result goes to the leader.
        s.world.politics.news.clear();
        s.world.politics.news.push(cw_sim::state::PoliticalNews {
            week: 30,
            text: "1982 election: Business party keeps power with 52% approval.".to_owned(),
            person: None,
        });
        let e = letters(&s.world, &s.dataset, 25)
            .into_iter()
            .find(|l| l.kind == "government")
            .unwrap();
        assert_eq!(e.sender.role, "Leader of the government");
        assert!(text(&e).contains("52% approval"));
    }

    #[test]
    fn a_strike_at_your_company_is_a_letter_from_the_union() {
        let mut s = quiet("great-lakes-2000", 30);
        let name = s.world.firms.get(you(&s)).unwrap().name.clone();
        s.world.news.push(cw_sim::state::NewsItem {
            week: Tick(30),
            key: "news.strike".to_owned(),
            subject: name.clone(),
            amount_cents: 0,
            quantity: Default::default(),
        });
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        check(&l);
        assert_eq!(l.kind, "union");
        assert_eq!(l.sender.organisation, format!("Workers at {name}"));
        assert!(text(&l).contains("walked out"));
        // A strike at a rival is the paper's, not a letter.
        let mut t = quiet("great-lakes-2000", 30);
        let other = t.world.firms.get(rival(&t, 0)).unwrap().name.clone();
        t.world.news.push(cw_sim::state::NewsItem {
            week: Tick(30),
            key: "news.strike".to_owned(),
            subject: other,
            amount_cents: 0,
            quantity: Default::default(),
        });
        assert!(letters(&t.world, &t.dataset, 25).is_empty());
    }

    #[test]
    fn a_foreshadow_notice_is_a_letter_from_the_analyst() {
        let mut s = quiet("texas-1981", 76);
        s.world.systemic.timeline.events.push(LiveEvent {
            id: "test-bank-failure".to_owned(),
            title: "Penn Square style bank failure".to_owned(),
            summary: "A large lender to the oil patch fails. Credit tightens.".to_owned(),
            tick: 82,
            foreshadow_weeks: 8,
            effects: Vec::new(),
            announced: true,
            fired_at: 0,
        });
        let l = letters(&s.world, &s.dataset, 70)
            .into_iter()
            .find(|l| l.kind == "analyst")
            .expect("the notice is a letter");
        check(&l);
        let body = text(&l);
        assert!(
            body.contains("Penn Square style bank failure") && body.contains("in about 8 weeks"),
            "{body}"
        );
        assert!(body.contains("A large lender to the oil patch fails."));
        assert_eq!(l.sender.organisation, "The Gulf Ledger");
        assert_eq!(l.link.key, "test-bank-failure");
    }

    fn book_weeks(s: &mut Session, first: u32, last: u32, revenue: i64, cost: i64) {
        let handle = you(s);
        let firm = s.world.firms.get_mut(handle).unwrap();
        if cost > 0 {
            let float = cost * 40;
            firm.book
                .commit(
                    Tick(first),
                    Txn::new().debit(Account::Cash, float, "float").credit(
                        Account::ShareCapital,
                        float,
                        "float",
                    ),
                )
                .unwrap();
        }
        for week in first..=last {
            let mut txn = Txn::new().debit(Account::Cash, revenue, "sales").credit(
                Account::Revenue,
                revenue,
                "sales",
            );
            if cost > 0 {
                txn = txn.debit(Account::OperatingExpense, cost, "costs").credit(
                    Account::Cash,
                    cost,
                    "costs",
                );
            }
            firm.book.commit(Tick(week), txn).unwrap();
            firm.book.close_week(Tick(week));
        }
    }

    #[test]
    fn a_strong_quarter_is_a_short_memo_from_the_cfo() {
        let mut s = quiet("great-lakes-2000", 26);
        book_weeks(&mut s, 14, 26, 1_000_000_00, 0);
        let l = letters(&s.world, &s.dataset, 20)
            .into_iter()
            .find(|l| l.id == "cfo:q:26")
            .expect("a good-news memo");
        check(&l);
        assert_eq!(l.sender.role, "Chief financial officer");
        assert!(l.sender.person.is_empty());
        assert!(text(&l).starts_with(office_frames("cfo").0));
        assert!(text(&l).contains("strong quarter"));
        // Thin margins are not a strong quarter.
        let mut t = quiet("great-lakes-2000", 26);
        book_weeks(&mut t, 14, 26, 1_000_000_00, 995_000_00);
        assert!(
            letters(&t.world, &t.dataset, 20)
                .iter()
                .all(|l| l.kind != "cfo")
        );
    }

    #[test]
    fn a_losing_company_hears_the_cfos_reasons() {
        let mut s = quiet("great-lakes-2000", 30);
        let handle = you(&s);
        s.world
            .firms
            .get_mut(handle)
            .unwrap()
            .policies
            .research_budget_cents = 5_000_000_00;
        book_weeks(&mut s, 24, 30, 1_000_000_00, 6_000_000_00);
        let advice = crate::cfo::advice(&s.world, &s.dataset, handle.0);
        assert!(!advice.is_empty(), "the setup should make the CFO speak");
        let l = letters(&s.world, &s.dataset, 25)
            .into_iter()
            .find(|l| l.kind == "cfo")
            .expect("a memo");
        check(&l);
        assert!(text(&l).contains("lost"), "{}", text(&l));
        assert!(text(&l).contains(&advice[0].headline) || l.paragraphs.len() < 3);
    }

    #[test]
    fn a_decision_card_arrives_from_its_office_and_keeps_its_buttons() {
        let mut s = quiet("great-lakes-2000", 30);
        let handle = you(&s);
        s.world.events.pending.push(cw_sim::state::PendingEvent {
            id: 7,
            card: "covenant_warning".to_owned(),
            subject_firm: Some(handle),
            region: None,
            deadline: Tick(33),
            opened: Tick(30),
            mandatory: true,
            resolved: false,
            deal: None,
        });
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        check(&l);
        assert_eq!(l.kind, "decision");
        assert_eq!(l.sender.name, "Your bank");
        assert_eq!(l.event, Some(7));
        assert!(l.mandatory && l.deadline == 33);
        let ids: Vec<&str> = l.choices.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["raise", "renegotiate"]);
        let body = text(&l);
        assert!(
            body.contains("Covenant warning") && body.contains("We need your answer by week"),
            "{body}"
        );
        assert_eq!(l.link.kind, "company");
    }

    #[test]
    fn decisions_sit_on_top_mandatory_first() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 0), you(&s));
        s.world.raids.push(raid(r, y, true));
        for (id, card, mandatory, due) in [
            (1, "tech_opportunity", false, 40),
            (2, "covenant_warning", true, 33),
        ] {
            s.world.events.pending.push(cw_sim::state::PendingEvent {
                id,
                card: card.to_owned(),
                subject_firm: Some(y),
                region: None,
                deadline: Tick(due),
                opened: Tick(28),
                mandatory,
                resolved: false,
                deal: None,
            });
        }
        let found = letters(&s.world, &s.dataset, 25);
        let order: Vec<(&str, Option<u64>)> =
            found.iter().map(|l| (l.kind.as_str(), l.event)).collect();
        assert_eq!(
            order,
            [
                ("decision", Some(2)),
                ("decision", Some(1)),
                ("rival", None)
            ]
        );
    }

    #[test]
    fn a_rival_without_a_chief_then_writes_as_its_chief_executive() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 0), you(&s));
        s.world.raids.push(raid(r, y, true));
        // The current chief took the post after the raid.
        s.world.firms.get_mut(r).unwrap().ceo = Some(0);
        s.world.cast.appointed.insert(r, 40);
        let l = letters(&s.world, &s.dataset, 25).remove(0);
        assert!(l.sender.person.is_empty(), "no named chief yet");
        assert!(l.sender.name.starts_with("The chief executive of "));
        check(&l);
    }

    #[test]
    fn letters_are_the_same_every_time_and_reading_one_clears_it() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 0), you(&s));
        s.world.raids.push(raid(r, y, true));
        let a = letters(&s.world, &s.dataset, 25);
        assert_eq!(a, letters(&s.world, &s.dataset, 25));
        assert_eq!(unread_count(&s.world, &s.dataset), 1);
        let hash = cw_sim::state::canonical_hash(&s.world);
        mark_read(
            &mut s.world,
            &s.dataset,
            &[a[0].id.clone(), "nonsense".to_owned()],
        );
        assert_eq!(unread_count(&s.world, &s.dataset), 0);
        assert!(letters(&s.world, &s.dataset, 25)[0].read);
        assert_eq!(
            cw_sim::state::canonical_hash(&s.world),
            hash,
            "read flags are not hashed"
        );
        assert_eq!(s.world.mail.read.len(), 1);
    }

    #[test]
    fn an_old_letter_moves_to_the_archive_and_then_out() {
        let mut s = quiet("great-lakes-2000", 30);
        let (r, y) = (rival(&s, 0), you(&s));
        s.world.raids.push(raid(r, y, true));
        s.world.tick = Tick(30 + INBOX_WEEKS + 2);
        assert!(letters(&s.world, &s.dataset, s.world.tick.0 - INBOX_WEEKS).is_empty());
        assert_eq!(
            letters(&s.world, &s.dataset, 1).len(),
            1,
            "still in the archive"
        );
        assert_eq!(unread_count(&s.world, &s.dataset), 0);
        s.world.tick = Tick(30 + ARCHIVE_WEEKS + 1);
        assert!(letters(&s.world, &s.dataset, 1).is_empty());
    }

    #[test]
    fn a_long_fact_is_cut_to_the_limit() {
        let s = quiet("great-lakes-2000", 30);
        let long = "The company posted a figure that goes on and on about nothing in particular. "
            .repeat(12);
        let l = finish(
            &s.world,
            Draft {
                id: "test:long".to_owned(),
                kind: "rival",
                week: 30,
                sender: office_sender("Someone", "Role", "Org"),
                tone: Tone::Formal,
                addressee: "Org".to_owned(),
                facts: vec![long],
                link: PaperLink::none(),
            },
            None,
        );
        check(&l);
        assert!(text(&l).ends_with('.'));
    }
}
