//! Scenario timelines: dated events from a scenario's history, played through the mechanics the
//! engine already has.
//!
//! A career copies its scenario's events into the save when it starts (`init`), with the dates it
//! will play them on: as authored (Historical), moved by up to 26 weeks either way from the game
//! seed (Shuffled), or none at all (Off). Each week `weekly` announces the events whose notice
//! date has come, and fires the events whose date has come by applying their effects:
//!
//! * a price shock lays a fading overlay on the family index in `world_prices`;
//! * a demand regime is a regional demand modifier, the same kind recessions and booms use;
//! * a stance shift moves the central bank chair;
//! * a credit tightening adds to the lending premium in `macro_cycle`;
//! * a law change sets the law directly;
//! * a systemic event goes through `systemic::force`, which records its own incident;
//! * an import ceiling change scales the gateway capacity for a good.
//!
//! Nothing here draws a random number after `init`, so a saved and reloaded game plays what an
//! unbroken one does. Every fired event is recorded as an incident and a news item, and a notice
//! goes out `foreshadow_weeks` earlier as a news item with no effect.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::rng::Rng;
use cw_data::Dataset;
use cw_data::timeline::{self as data, TimelineFile};
use serde::{Deserialize, Serialize};

use crate::economy::systemic::{self, Incident};
use crate::state::{Laws, PoliticalNews, UnionLaw, World};

const TAG_TIMELINE: u64 = 0x5449_4d45_4c49_4e45;
/// Shuffled dates move by up to this many weeks either way.
pub const SHUFFLE_WEEKS: i64 = 26;
/// Total overlay on one price family, in basis points either way, however many shocks stack.
const SHOCK_LIMIT_BPS: i32 = 6_000;
const NEWS_KEPT: usize = 40;
/// Past events kept for display.
pub const PAST_KEPT: usize = 40;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineMode {
    /// No dated events. What a converted older save plays.
    #[default]
    Off,
    /// The events on their authored dates.
    Historical,
    /// The same events with dates moved by up to 26 weeks from the game seed.
    Shuffled,
}

impl TimelineMode {
    pub const fn key(self) -> &'static str {
        match self {
            TimelineMode::Off => "off",
            TimelineMode::Historical => "historical",
            TimelineMode::Shuffled => "shuffled",
        }
    }

    pub fn from_key(key: &str) -> Option<TimelineMode> {
        match key {
            "off" => Some(TimelineMode::Off),
            "historical" => Some(TimelineMode::Historical),
            "shuffled" => Some(TimelineMode::Shuffled),
            _ => None,
        }
    }
}

/// An effect as the engine keeps it in the save: goods are product codes of the career's
/// dataset, the rest as authored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Effect {
    PriceShock {
        good: u16,
        percent: i32,
        weeks: u32,
    },
    DemandRegime {
        sector: String,
        percent: i32,
        weeks: u32,
    },
    CentralBankStance {
        shift: i32,
    },
    CreditTightening {
        spread_bps: i32,
        weeks: u32,
    },
    LawChange {
        law: String,
        value: i64,
    },
    SystemicEvent {
        event: String,
    },
    ImportCeiling {
        good: u16,
        percent: i32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveEvent {
    pub id: String,
    pub title: String,
    pub summary: String,
    /// The week it fires.
    pub tick: u32,
    pub foreshadow_weeks: u32,
    pub effects: Vec<Effect>,
    /// The notice has gone out.
    pub announced: bool,
    /// The week it fired, or zero while it is still ahead.
    pub fired_at: u32,
}

impl LiveEvent {
    /// The week the notice goes out, or none for an event without one.
    pub fn notice_tick(&self) -> Option<u32> {
        (self.foreshadow_weeks > 0)
            .then(|| self.tick.saturating_sub(self.foreshadow_weeks).max(1))
            .filter(|notice| *notice < self.tick)
    }
}

/// A world price overlay: `bps` at once in week `start`, fading evenly to nothing over `weeks`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveShock {
    pub family: u16,
    pub start: u32,
    pub weeks: u32,
    pub bps: i32,
}

/// Extra lender spread in force until the week `until`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squeeze {
    pub spread_bps: i32,
    pub until: u32,
}

/// The timeline cursor: the events a career plays, which have been announced and which have
/// fired, and the overlays they have in force.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineState {
    pub mode: TimelineMode,
    pub events: Vec<LiveEvent>,
    pub shocks: Vec<ActiveShock>,
    pub squeezes: Vec<Squeeze>,
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn convert(effect: &data::Effect, dataset: &Dataset) -> Option<Effect> {
    Some(match effect {
        data::Effect::PriceShock {
            good,
            percent,
            weeks,
        } => Effect::PriceShock {
            good: *dataset.product_index.get(good)?,
            percent: *percent,
            weeks: *weeks,
        },
        data::Effect::DemandRegime {
            sector,
            percent,
            weeks,
        } => Effect::DemandRegime {
            sector: sector.clone(),
            percent: *percent,
            weeks: *weeks,
        },
        data::Effect::CentralBankStance { shift } => Effect::CentralBankStance { shift: *shift },
        data::Effect::CreditTightening { spread_bps, weeks } => Effect::CreditTightening {
            spread_bps: *spread_bps,
            weeks: *weeks,
        },
        data::Effect::LawChange { law, value } => Effect::LawChange {
            law: law.clone(),
            value: *value,
        },
        data::Effect::SystemicEvent { event } => Effect::SystemicEvent {
            event: event.clone(),
        },
        data::Effect::ImportCeiling { good, percent } => Effect::ImportCeiling {
            good: *dataset.product_index.get(good)?,
            percent: *percent,
        },
    })
}

/// The live events of a scenario's timeline in the given mode, with their play dates. Shuffled
/// moves each by a draw keyed on the seed and the event's own id, so adding an event to the file
/// does not move another's date.
pub fn live_events(
    file: &TimelineFile,
    start_year: i32,
    mode: TimelineMode,
    seed: u64,
    dataset: &Dataset,
) -> Vec<LiveEvent> {
    if mode == TimelineMode::Off {
        return Vec::new();
    }
    let mut events: Vec<LiveEvent> = file
        .events
        .iter()
        .map(|event| {
            let mut tick = event.tick(start_year) as i64;
            if mode == TimelineMode::Shuffled {
                let mut rng = Rng::entity_stream(seed, TAG_TIMELINE, Handle(fnv(&event.id)));
                tick += rng.uniform_inclusive(-SHUFFLE_WEEKS, SHUFFLE_WEEKS);
            }
            LiveEvent {
                id: event.id.clone(),
                title: event.title.clone(),
                summary: event.summary.clone(),
                tick: tick.max(2) as u32,
                foreshadow_weeks: event.foreshadow_weeks,
                effects: event
                    .effects
                    .iter()
                    .filter_map(|effect| convert(effect, dataset))
                    .collect(),
                announced: false,
                fired_at: 0,
            }
        })
        .collect();
    events.sort_by(|a, b| a.tick.cmp(&b.tick).then_with(|| a.id.cmp(&b.id)));
    events
}

/// Set up the timeline for a new career. A scenario with no timeline file plays none.
pub fn init(
    world: &mut World,
    dataset: &Dataset,
    scenario_id: &str,
    mode: TimelineMode,
    seed: u64,
) {
    let file = match TimelineFile::load(scenario_id, dataset) {
        Ok(Some(file)) => file,
        _ => {
            world.systemic.timeline = TimelineState::default();
            return;
        }
    };
    let start = data::start_year(scenario_id).unwrap_or(world.start_year);
    world.systemic.timeline = TimelineState {
        mode,
        events: live_events(&file, start, mode, seed, dataset),
        shocks: Vec::new(),
        squeezes: Vec::new(),
    };
}

fn news(world: &mut World, text: String) {
    world.politics.news.push(PoliticalNews {
        person: None,
        week: world.tick.0,
        text,
    });
    let excess = world.politics.news.len().saturating_sub(NEWS_KEPT);
    world.politics.news.drain(..excess);
}

/// The overlay on a price family this week, in basis points of its index.
pub fn price_shock_bps(world: &World, family: u16) -> i32 {
    let now = world.tick.0;
    world
        .systemic
        .timeline
        .shocks
        .iter()
        .filter(|s| s.family == family && now >= s.start && now < s.start + s.weeks)
        .map(|s| (s.bps as i64 * (s.start + s.weeks - now) as i64 / s.weeks as i64) as i32)
        .sum::<i32>()
        .clamp(-SHOCK_LIMIT_BPS, SHOCK_LIMIT_BPS)
}

/// Extra lender spread from credit tightenings in force, in basis points.
pub fn credit_spread_bps(world: &World) -> i32 {
    let now = world.tick.0;
    world
        .systemic
        .timeline
        .squeezes
        .iter()
        .filter(|s| now < s.until)
        .map(|s| s.spread_bps)
        .sum()
}

/// Run the week's announcements and events. Runs before the world prices and the business cycle,
/// so a shock or squeeze fired this week is in force for them this week.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let now = world.tick.0;
    let timeline = &mut world.systemic.timeline;
    timeline.shocks.retain(|s| now < s.start + s.weeks);
    timeline.squeezes.retain(|s| now < s.until);
    if timeline.events.is_empty() {
        return;
    }
    let mut notices = Vec::new();
    let mut firing = Vec::new();
    for (index, event) in timeline.events.iter_mut().enumerate() {
        if event.fired_at != 0 {
            continue;
        }
        if !event.announced
            && let Some(notice) = event.notice_tick()
            && now >= notice
            && now < event.tick
        {
            event.announced = true;
            notices.push((event.tick - now, event.title.clone()));
        }
        if now >= event.tick {
            event.fired_at = now;
            firing.push(index);
        }
    }
    for (weeks, title) in notices {
        let when = if weeks == 1 {
            "next week".to_owned()
        } else {
            format!("in about {weeks} weeks")
        };
        news(world, format!("Coming {when}: {title}."));
    }
    for index in firing {
        let event = world.systemic.timeline.events[index].clone();
        for effect in &event.effects {
            apply(world, dataset, &event.id, effect);
        }
        let text = format!("{}. {}", event.title, event.summary);
        news(world, text.clone());
        let weeks = event
            .effects
            .iter()
            .map(|e| match e {
                Effect::PriceShock { weeks, .. }
                | Effect::DemandRegime { weeks, .. }
                | Effect::CreditTightening { weeks, .. } => *weeks,
                _ => 0,
            })
            .max()
            .unwrap_or(0);
        systemic::record_timeline(
            world,
            Incident {
                week: now,
                kind: systemic::TIMELINE,
                a: 0,
                b: 0,
                mode: 0,
                weeks: weeks.min(u16::MAX as u32) as u16,
                firms: Vec::new(),
                text,
            },
        );
    }
}

fn apply(world: &mut World, dataset: &Dataset, id: &str, effect: &Effect) {
    let now = world.tick.0;
    match effect {
        Effect::PriceShock {
            good,
            percent,
            weeks,
        } => {
            if world.world_prices.mode == crate::economy::world_prices::PriceMode::Off {
                return;
            }
            if let Some(family) = world.world_prices.product_family.get(good).copied() {
                world.systemic.timeline.shocks.push(ActiveShock {
                    family,
                    start: now,
                    weeks: (*weeks).max(1),
                    bps: percent * 100,
                });
            }
        }
        Effect::DemandRegime {
            sector,
            percent,
            weeks,
        } => {
            let subjects: Vec<Option<u16>> = if sector == "all" {
                vec![None]
            } else {
                let mut codes: Vec<u16> = dataset
                    .recipes
                    .iter()
                    .filter(|r| &r.industry == sector)
                    .filter_map(|r| dataset.product_index.get(&r.output).copied())
                    .collect();
                codes.sort_unstable();
                codes.dedup();
                codes.into_iter().map(Some).collect()
            };
            let until = Tick(now + weeks);
            for region in &mut world.regions {
                for subject in &subjects {
                    region.demand_modifiers.push((
                        format!("timeline:{id}"),
                        *subject,
                        percent * 100,
                        until,
                    ));
                }
            }
        }
        Effect::CentralBankStance { shift } => {
            let chair = &mut world.macro_state.cycle.chair;
            chair.stance = (chair.stance + shift).clamp(-100, 100);
        }
        Effect::CreditTightening { spread_bps, weeks } => {
            world.systemic.timeline.squeezes.push(Squeeze {
                spread_bps: *spread_bps,
                until: now + weeks,
            });
        }
        Effect::LawChange { law, value } => set_law(&mut world.politics.laws, law, *value),
        Effect::SystemicEvent { event } => {
            systemic::force(world, dataset, event);
        }
        Effect::ImportCeiling { good, percent } => {
            for gateway in world.gateways.iter_mut().filter(|g| g.product == *good) {
                gateway.capacity_lots =
                    (gateway.capacity_lots * (100 + *percent as i64) / 100).max(1);
            }
        }
    }
}

fn set_law(laws: &mut Laws, law: &str, value: i64) {
    match law {
        "corporate_tax_bps" => laws.corporate_tax_bps = value as i32,
        "wage_floor_cents" => laws.wage_floor_cents = value,
        "merger_review_bps" => laws.merger_review_bps = value,
        "unions" => {
            laws.unions = match value {
                0 => UnionLaw::Weak,
                2 => UnionLaw::Strong,
                _ => UnionLaw::Standard,
            }
        }
        "common_carrier" => laws.common_carrier = value != 0,
        "vertical_review_bps" => laws.vertical_review_bps = value,
        "reserve" => laws.reserve = value != 0,
        "privatise" => laws.privatise = value != 0,
        "quota_bps" => laws.quota_bps = value as i32,
        _ => {}
    }
}

fn year_of(start: i32, tick: u32) -> i32 {
    start + (tick.saturating_sub(1) / cw_core::calendar::WEEKS_PER_YEAR) as i32
}

/// An event the player has been told about and that has not happened yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upcoming {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub year: i32,
    pub week_of_year: u32,
    pub tick: u32,
    pub weeks_away: u32,
}

/// An event that has happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Past {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub year: i32,
    pub week_of_year: u32,
    pub tick: u32,
}

/// Events announced and still ahead, soonest first. Events that have not been announced are
/// never listed, whatever their date.
pub fn upcoming(world: &World) -> Vec<Upcoming> {
    let now = world.tick.0;
    let start = world.start_year;
    world
        .systemic
        .timeline
        .events
        .iter()
        .filter(|e| e.announced && e.fired_at == 0)
        .map(|e| {
            let at = Tick(e.tick);
            Upcoming {
                id: e.id.clone(),
                title: e.title.clone(),
                summary: e.summary.clone(),
                year: year_of(start, at.0),
                week_of_year: at.week_of_year(),
                tick: e.tick,
                weeks_away: e.tick.saturating_sub(now),
            }
        })
        .collect()
}

/// Events that have happened, newest first, with the date they fired on.
pub fn past(world: &World) -> Vec<Past> {
    let start = world.start_year;
    let mut out: Vec<Past> = world
        .systemic
        .timeline
        .events
        .iter()
        .filter(|e| e.fired_at != 0)
        .map(|e| {
            let at = Tick(e.fired_at);
            Past {
                id: e.id.clone(),
                title: e.title.clone(),
                summary: e.summary.clone(),
                year: year_of(start, at.0),
                week_of_year: at.week_of_year(),
                tick: e.fired_at,
            }
        })
        .collect();
    out.sort_by(|a, b| b.tick.cmp(&a.tick).then_with(|| a.id.cmp(&b.id)));
    out.truncate(PAST_KEPT);
    out
}
