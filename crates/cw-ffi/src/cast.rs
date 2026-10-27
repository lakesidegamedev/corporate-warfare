//! The character cast for the apps: people, the posts they hold, and a rival's dossier.

use cw_core::ids::Handle;
use cw_data::{Dataset, Person, Role};
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
    /// What they can be asked to do: ceo, chair, governor, leader.
    pub roles: Vec<String>,
    /// Central bank stance, dovish negative to hawkish positive, for chair-capable people.
    pub stance: Option<i32>,
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
}

pub fn person_dto(ds: &Dataset, p: u8) -> Option<PersonDto> {
    ds.cast.get(p as usize).map(person)
}

fn person(p: &Person) -> PersonDto {
    PersonDto {
        id: p.id.clone(),
        name: p.name.clone(),
        age: p.age,
        home: p.home.clone(),
        background: p.background.clone(),
        temperament: p.temperament.key().to_owned(),
        roles: p
            .roles
            .iter()
            .map(|r| {
                match r {
                    Role::Ceo => "ceo",
                    Role::Chair => "chair",
                    Role::Governor => "governor",
                    Role::Leader => "leader",
                }
                .to_owned()
            })
            .collect(),
        stance: p.stance,
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

pub fn dossier(session: &Session, firm: u64) -> Option<DossierDto> {
    let world = &session.world;
    let ds = &session.dataset;
    let handle = Handle(firm);
    let entity = world.firms.get(handle)?;
    let ceo = entity.ceo.and_then(|p| person_dto(ds, p));
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
    for raid in world
        .raids
        .iter()
        .filter(|r| r.attacker == handle || r.defender == handle)
    {
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
    moves.sort_by(|a, b| b.week.cmp(&a.week));
    moves.truncate(8);
    let style = entity
        .archetype
        .map(|a| a.key().replace('_', " "))
        .unwrap_or_else(|| "unknown".to_owned());
    Some(DossierDto {
        firm,
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
