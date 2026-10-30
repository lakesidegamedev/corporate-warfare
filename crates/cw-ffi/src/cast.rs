//! The character cast for the apps: people, the posts they hold, and a rival's dossier.

use cw_core::ids::Handle;
use cw_data::{Dataset, Mood, Person, Role};
use cw_session::Session;
use cw_sim::state::World;

/// One person of the cast. `id` names the portrait in `cast-portraits.json`.
#[derive(Clone, uniffi::Record)]
pub struct PersonDto {
    pub id: String,
    pub name: String,
    pub age: u32,
    pub home: String,
    pub background: String,
    /// integrator, raider, financier, speculator, builder or cautious.
    pub temperament: String,
    /// What they can be asked to do: ceo, chair, governor, leader, union_leader, journalist,
    /// analyst, cfo, coo or logistics.
    pub roles: Vec<String>,
    /// Central bank stance, dovish negative to hawkish positive, for chair-capable people.
    pub stance: Option<i32>,
    /// neutral, pleased or displeased: which face of the portrait to draw this time.
    pub mood: String,
    /// Whether the person has a 64 by 64 office portrait for large views.
    pub office_portrait: bool,
}

/// A post someone holds: chair, country leader, party leader or governor.
#[derive(Clone, uniffi::Record)]
pub struct PostDto {
    /// chair, country_leader, party_leader or governor.
    pub post: String,
    /// The place or party: a state name, a party name or the central bank.
    pub place: String,
    pub person: PersonDto,
}

/// One thing a rival did lately.
#[derive(Clone, uniffi::Record)]
pub struct MoveDto {
    pub week: u32,
    pub text: String,
}

/// A rival firm's dossier: who runs it, how they play and what it has done lately.
#[derive(Clone, uniffi::Record)]
pub struct DossierDto {
    pub firm: u64,
    pub firm_name: String,
    /// The named chief executive, when the firm has one.
    pub ceo: Option<PersonDto>,
    /// Weeks the current chief executive has served.
    pub ceo_weeks: u32,
    /// The firm's strategic style, from its archetype.
    pub style: String,
    pub moves: Vec<MoveDto>,
    /// Who the firm holds a grudge against, strongest first, in plain words.
    pub grudges: Vec<String>,
    /// Who the firm is friendly with, strongest first, in plain words.
    pub allies: Vec<String>,
    /// The executives it has hired, with the pay a rival would need to offer to move each.
    pub executives: Vec<crate::executives::ExecutivePostDto>,
}

pub fn person_dto(ds: &Dataset, p: u8) -> Option<PersonDto> {
    ds.cast.get(p as usize).map(person)
}

pub(crate) fn person(p: &Person) -> PersonDto {
    PersonDto {
        id: p.id.clone(),
        name: p.name.clone(),
        age: p.age,
        home: p.home.clone(),
        background: p.background.clone(),
        temperament: p.temperament.key().to_owned(),
        roles: p.roles.iter().map(|r: &Role| r.key().to_owned()).collect(),
        stance: p.stance,
        mood: Mood::Neutral.key().to_owned(),
        office_portrait: p.office_portrait,
    }
}

pub fn people(ds: &Dataset) -> Vec<PersonDto> {
    ds.cast.iter().map(person).collect()
}

/// The chair, the country's leader, the opposition leader and the governors.
pub fn posts(world: &World, ds: &Dataset) -> Vec<PostDto> {
    let mut out = Vec::new();
    if let Some(p) = world
        .macro_state
        .cycle
        .chair
        .person
        .and_then(|p| person_dto(ds, p))
    {
        out.push(PostDto {
            post: "chair".to_owned(),
            place: "Central bank".to_owned(),
            person: p,
        });
    }
    let governing = world.politics.party;
    for party in [governing, governing.other()] {
        let Some(p) =
            cw_sim::economy::cast::party_leader(world, party).and_then(|p| person_dto(ds, p))
        else {
            continue;
        };
        let name = cw_sim::economy::politics::party_name(party)
            .trim_start_matches("the ")
            .to_owned();
        out.push(PostDto {
            post: if party == governing {
                "country_leader"
            } else {
                "party_leader"
            }
            .to_owned(),
            place: name,
            person: p,
        });
    }
    for (state, p) in &world.cast.governors {
        if let (Some(info), Some(p)) = (ds.states.get(*state as usize), person_dto(ds, *p)) {
            out.push(PostDto {
                post: "governor".to_owned(),
                place: info.name.clone(),
                person: p,
            });
        }
    }
    out
}

fn holder_name(world: &World, holder: cw_sim::state::HolderId) -> String {
    use cw_sim::state::HolderId;
    match holder {
        HolderId::Investor => "you".to_owned(),
        HolderId::Firm(h) => world
            .firms
            .get(h)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "a former rival".to_owned()),
        _ => "an outside holder".to_owned(),
    }
}

fn grudge_reason(cause: cw_sim::economy::rivalry::Cause) -> &'static str {
    use cw_sim::economy::rivalry::Cause;
    match cause {
        Cause::Raid => "lost plants to them",
        Cause::FailedRaid => "they came after its markets",
        Cause::LaneLockout => "squeezed out of a freight lane",
        Cause::HostileBid => "they bid for it uninvited",
        Cause::Venture => "a venture soured",
        Cause::Deal => "a deal soured",
    }
}

fn ally_reason(cause: cw_sim::economy::rivalry::Cause) -> &'static str {
    use cw_sim::economy::rivalry::Cause;
    match cause {
        Cause::Venture => "partners in a venture",
        _ => "closed a deal together",
    }
}

pub fn dossier(session: &Session, firm: u64) -> Option<DossierDto> {
    let world = &session.world;
    let handle = Handle(firm);
    let entity = world.firms.get(handle)?;
    let ceo = crate::company::resolved_ceo(session, handle);
    let since = world.cast.appointed.get(&handle).copied().unwrap_or(0);
    let name_of = |h: Handle| {
        world
            .firms
            .get(h)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "a former rival".to_owned())
    };
    let mut moves: Vec<MoveDto> = Vec::new();
    for m in world.cast.moves.iter().filter(|m| m.firm == handle) {
        moves.push(MoveDto {
            week: m.week,
            text: m.text.clone(),
        });
    }
    for m in world.rivalry.moves.iter().filter(|m| m.firm == handle) {
        moves.push(MoveDto {
            week: m.week,
            text: m.text.clone(),
        });
    }
    // The chief's look follows their latest raid: pleased after taking plants or holding them,
    // displeased after losing plants or a failed raid; otherwise friendly or bitter ties decide.
    let mut mood = (0u32, Mood::Neutral);
    for raid in world
        .raids
        .iter()
        .filter(|r| r.attacker == handle || r.defender == handle)
    {
        if raid.tick >= mood.0 {
            mood = (
                raid.tick,
                if (raid.attacker == handle) == raid.success {
                    Mood::Pleased
                } else {
                    Mood::Displeased
                },
            );
        }
        let industry = cw_sim::state::INDUSTRY_NAMES
            .get(raid.industry as usize)
            .copied()
            .unwrap_or("a sector");
        let place = world
            .regions
            .get(raid.region as usize)
            .map(|r| r.name.clone())
            .unwrap_or_default();
        let text = match (raid.attacker == handle, raid.success) {
            (true, true) => format!(
                "Raided {} in {place} ({industry}) and took {} plants.",
                name_of(raid.defender),
                raid.blocks
            ),
            (true, false) => format!(
                "Raided {} in {place} ({industry}) and failed.",
                name_of(raid.defender)
            ),
            (false, true) => format!(
                "Lost {} plants in {place} ({industry}) to {}.",
                raid.blocks,
                name_of(raid.attacker)
            ),
            (false, false) => format!(
                "Held off {} in {place} ({industry}).",
                name_of(raid.attacker)
            ),
        };
        moves.push(MoveDto {
            week: raid.tick,
            text,
        });
    }
    moves.sort_by_key(|m| std::cmp::Reverse(m.week));
    moves.truncate(8);
    let style = entity
        .archetype
        .map(|a| a.key().replace('_', " "))
        .unwrap_or_else(|| "unknown".to_owned());
    let strength = |score: i32| match score {
        s if s >= 6_000 => "bitter",
        s if s >= 3_000 => "strong",
        _ => "mild",
    };
    let grudges: Vec<String> = cw_sim::economy::rivalry::grudges_of(world, handle)
        .into_iter()
        .map(|(holder, m)| {
            format!(
                "{} grudge against {}: {}.",
                strength(m.score),
                holder_name(world, holder),
                grudge_reason(m.cause)
            )
        })
        .take(3)
        .collect();
    let allies: Vec<String> = cw_sim::economy::rivalry::allies_of(world, handle)
        .into_iter()
        .map(|(holder, m)| {
            format!(
                "Friendly with {}: {}.",
                holder_name(world, holder),
                ally_reason(m.cause)
            )
        })
        .take(3)
        .collect();
    let ceo = ceo.map(|mut p| {
        p.mood = if mood.1 != Mood::Neutral {
            mood.1
        } else if !grudges.is_empty() {
            Mood::Displeased
        } else if !allies.is_empty() {
            Mood::Pleased
        } else {
            Mood::Neutral
        }
        .key()
        .to_owned();
        p
    });
    Some(DossierDto {
        firm,
        grudges,
        allies,
        executives: crate::executives::posts(world, handle, true),
        firm_name: entity.name.clone(),
        ceo_weeks: if ceo.is_some() {
            world.tick.0.saturating_sub(since)
        } else {
            0
        },
        ceo,
        style,
        moves,
    })
}

/// Who a rival firm is at a glance: its chief executive and how they play, for tags on rows.
#[derive(Clone, uniffi::Record)]
pub struct RivalTagDto {
    pub firm: u64,
    pub firm_name: String,
    /// Empty when the firm has no named chief executive.
    pub ceo_name: String,
    /// Names the portrait; empty with no chief executive.
    pub ceo_id: String,
    /// integrator, raider, financier, speculator, builder or cautious; empty with no chief.
    pub temperament: String,
    /// Strength of the grudge this firm holds against you, zero when none is active.
    pub grudge_bps: i32,
    /// True when the firm and you are tied by a venture or a friendly deal.
    pub ally: bool,
}

/// One rival for the CEO Office card: the tag, how many markets you share and your best raid.
#[derive(Clone, uniffi::Record)]
pub struct RivalSummaryDto {
    pub tag: RivalTagDto,
    pub shared_markets: u32,
    /// The raid with the best odds at the suggested budget, when you can raid them now.
    pub best_raid: Option<crate::RaidOptionDto>,
}

/// One thing a rival did lately, with who was behind it.
#[derive(Clone, uniffi::Record)]
pub struct RivalMoveFeedDto {
    pub week: u32,
    pub firm: u64,
    pub firm_name: String,
    pub ceo_name: String,
    pub temperament: String,
    pub text: String,
}

fn ceo_of(world: &World, ds: &Dataset, firm: Handle) -> Option<PersonDto> {
    world
        .firms
        .get(firm)
        .and_then(|f| f.ceo)
        .and_then(|p| person_dto(ds, p))
}

/// Chief executive name and temperament for a firm; empty strings when it has none.
pub fn who(world: &World, ds: &Dataset, firm: Handle) -> (String, String) {
    ceo_of(world, ds, firm)
        .map(|p| (p.name, p.temperament))
        .unwrap_or_default()
}

pub fn rival_tag(world: &World, ds: &Dataset, firm: Handle, you: Handle) -> Option<RivalTagDto> {
    use cw_sim::economy::rivalry::{ACTIVE, alliance, grudge};
    let entity = world.firms.get(firm)?;
    let ceo = ceo_of(world, ds, firm);
    let grudge_bps = grudge(world, firm, you);
    Some(RivalTagDto {
        firm: firm.0,
        firm_name: entity.name.clone(),
        ceo_name: ceo.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        ceo_id: ceo.as_ref().map(|p| p.id.clone()).unwrap_or_default(),
        temperament: ceo.map(|p| p.temperament).unwrap_or_default(),
        grudge_bps: if grudge_bps >= ACTIVE { grudge_bps } else { 0 },
        ally: alliance(world, firm, you) >= ACTIVE,
    })
}

/// Every operating rival group's lead firm with a plant, threats first, then allies, then the
/// ones you can raid now by their odds.
pub fn rivals(session: &Session, you: u64) -> Vec<RivalSummaryDto> {
    use cw_sim::economy::{ownership::same_group, sectors};
    let world = &session.world;
    let ds = &session.dataset;
    let you = Handle(you);
    if world.firms.get(you).is_none() {
        return Vec::new();
    }
    let mine = sectors::divisions_of(world, ds, you);
    let mut out = Vec::new();
    for rival in world.firms.handles_sorted() {
        let Some(entity) = world.firms.get(rival) else {
            continue;
        };
        if rival == you
            || !entity.operative()
            || entity.facilities.is_empty()
            || same_group(world, rival, you)
            || cw_sim::economy::rivalry::root(world, rival) != cw_sim::state::HolderId::Firm(rival)
        {
            continue;
        }
        let Some(tag) = rival_tag(world, ds, rival, you) else {
            continue;
        };
        let mut shared = 0u32;
        let mut best: Option<crate::RaidOptionDto> = None;
        for d in &mine {
            if sectors::division_plants(world, ds, rival, d.region, d.industry).is_empty() {
                continue;
            }
            shared += 1;
            if let Some(option) = crate::raid_option(world, ds, you, rival, d.region, d.industry)
                && best
                    .as_ref()
                    .is_none_or(|b| option.quote.chance_bps > b.quote.chance_bps)
            {
                best = Some(option);
            }
        }
        out.push(RivalSummaryDto {
            tag,
            shared_markets: shared,
            best_raid: best,
        });
    }
    out.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.tag.grudge_bps),
            !r.tag.ally,
            std::cmp::Reverse(r.best_raid.as_ref().map_or(-1, |b| b.quote.chance_bps)),
            r.tag.firm,
        )
    });
    out
}

/// The latest rival moves anywhere, newest first, naming the chief behind each.
pub fn rival_feed(world: &World, ds: &Dataset, limit: usize) -> Vec<RivalMoveFeedDto> {
    world
        .rivalry
        .moves
        .iter()
        .rev()
        .take(limit)
        .map(|m| {
            let (ceo_name, temperament) = who(world, ds, m.firm);
            RivalMoveFeedDto {
                week: m.week,
                firm: m.firm.0,
                firm_name: world
                    .firms
                    .get(m.firm)
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| "a former rival".to_owned()),
                ceo_name,
                temperament,
                text: m.text.clone(),
            }
        })
        .collect()
}
