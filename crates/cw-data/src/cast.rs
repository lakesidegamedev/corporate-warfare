//! The character cast: fictional people who run rival firms, chair the central bank, govern
//! states and lead the parties, write for the paper, brief the markets, speak for the unions and
//! can be hired as executives.
//!
//! Content lives in `content/cast.json` and is part of the dataset hash. A person is data: a
//! home scenario, background, temperament and the posts they can hold. The engine fills posts
//! from this list; scenarios only change which people fit best.

use serde::{Deserialize, Serialize};

pub const CAST_JSON: &str = include_str!("../../../content/cast.json");

/// How a CEO tends to play: the weighting a rival's board expects of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Temperament {
    Integrator,
    Raider,
    Financier,
    Speculator,
    Builder,
    Cautious,
}

impl Temperament {
    pub const ALL: [Temperament; 6] = [
        Temperament::Integrator,
        Temperament::Raider,
        Temperament::Financier,
        Temperament::Speculator,
        Temperament::Builder,
        Temperament::Cautious,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Temperament::Integrator => "integrator",
            Temperament::Raider => "raider",
            Temperament::Financier => "financier",
            Temperament::Speculator => "speculator",
            Temperament::Builder => "builder",
            Temperament::Cautious => "cautious",
        }
    }

    pub const fn code(self) -> u8 {
        match self {
            Temperament::Integrator => 0,
            Temperament::Raider => 1,
            Temperament::Financier => 2,
            Temperament::Speculator => 3,
            Temperament::Builder => 4,
            Temperament::Cautious => 5,
        }
    }
}

/// A post a person can be asked to hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Ceo,
    Chair,
    Governor,
    Leader,
    /// Speaks for a scenario's workers in union letters.
    UnionLeader,
    /// Writes the scenario's paper bylines.
    Journalist,
    /// Writes the scenario's analyst notes and earnings letters.
    Analyst,
    /// Chief financial officer for hire.
    Cfo,
    /// Chief operating officer for hire.
    Coo,
    /// Logistics chief for hire.
    Logistics,
}

impl Role {
    /// The key the apps and the content file use.
    pub const fn key(self) -> &'static str {
        match self {
            Role::Ceo => "ceo",
            Role::Chair => "chair",
            Role::Governor => "governor",
            Role::Leader => "leader",
            Role::UnionLeader => "union_leader",
            Role::Journalist => "journalist",
            Role::Analyst => "analyst",
            Role::Cfo => "cfo",
            Role::Coo => "coo",
            Role::Logistics => "logistics",
        }
    }
}

/// How a person looks in one appearance, picked by the session from what just happened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    #[default]
    Neutral,
    /// After a deal, a beat or good news for them.
    Pleased,
    /// After a raid on them, a miss or bad news for them.
    Displeased,
}

impl Mood {
    /// The key the portrait atlas uses for the expression.
    pub const fn key(self) -> &'static str {
        match self {
            Mood::Neutral => "neutral",
            Mood::Pleased => "pleased",
            Mood::Displeased => "displeased",
        }
    }
}

/// The scenario's own person for a role: the first in cast order, or none when the scenario has
/// no one (a custom scenario). Union letters, bylines and analyst notes use this.
pub fn scenario_person<'a>(people: &'a [Person], scenario: &str, role: Role) -> Option<&'a Person> {
    people
        .iter()
        .find(|p| p.scenario == scenario && p.can(role))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    pub age: u32,
    /// The scenario this person belongs to best. Other scenarios may still use them.
    pub scenario: String,
    pub home: String,
    /// Home state code where the scenario has states, else empty.
    #[serde(default)]
    pub state: String,
    pub temperament: Temperament,
    pub roles: Vec<Role>,
    /// Central bank stance from -100 (dovish) to +100 (hawkish), for chair-capable people.
    #[serde(default)]
    pub stance: Option<i32>,
    pub background: String,
    /// How the person writes a letter, one line. Empty for generated people.
    #[serde(default)]
    pub voice: String,
    /// One line on what a person for hire is good at. Empty for everyone else.
    #[serde(default)]
    pub hire_note: String,
    /// Whether the person has a 64 by 64 office portrait.
    #[serde(default)]
    pub office_portrait: bool,
}

impl Person {
    pub fn can(&self, role: Role) -> bool {
        self.roles.contains(&role)
    }
}

/// A generated chief executive for a firm outside the twelve-person cast. Presentation only:
/// extras never hold posts, are not in the dataset hash and do not change with the world.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Extra {
    pub id: String,
    pub name: String,
    pub age: u32,
    pub home: String,
    pub temperament: Temperament,
    pub background: String,
}

impl Extra {
    pub fn person(&self) -> Person {
        Person {
            id: self.id.clone(),
            name: self.name.clone(),
            age: self.age,
            scenario: String::new(),
            home: self.home.clone(),
            state: String::new(),
            temperament: self.temperament,
            roles: vec![Role::Ceo],
            stance: None,
            background: self.background.clone(),
            voice: String::new(),
            hire_note: String::new(),
            office_portrait: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CastFile {
    pub schema: u32,
    #[serde(default)]
    pub classification: String,
    #[serde(default)]
    pub note: String,
    pub people: Vec<Person>,
    /// Generated chief executives for firms the cast does not reach (tools/cast_portraits.py).
    #[serde(default)]
    pub extras: Vec<Extra>,
}

/// The generated chief executives, in file order.
pub fn extras() -> &'static [Extra] {
    static EXTRAS: std::sync::OnceLock<Vec<Extra>> = std::sync::OnceLock::new();
    EXTRAS.get_or_init(|| {
        serde_json::from_str::<CastFile>(CAST_JSON)
            .map(|file| file.extras)
            .unwrap_or_default()
    })
}

/// Hire indices at or above this are named people for hire (see [`hire`]); below it they are
/// generated chiefs in [`extras`]. Saved candidates hold the index, so both ranges are stable.
pub const NAMED_HIRE_BASE: u16 = 1000;

/// A person who can be hired, from either list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hire {
    pub person: Person,
    /// Whether the person is a named member of the cast (not a generated chief).
    pub named: bool,
}

/// The named people for hire, in cast order: those who can be a chief financial, operating or
/// logistics officer. Hire index `NAMED_HIRE_BASE + n` is the nth of them.
pub fn named_hires() -> &'static [Person] {
    static HIRES: std::sync::OnceLock<Vec<Person>> = std::sync::OnceLock::new();
    HIRES.get_or_init(|| {
        serde_json::from_str::<CastFile>(CAST_JSON)
            .map(|file| {
                file.people
                    .into_iter()
                    .filter(|p| p.can(Role::Cfo) || p.can(Role::Coo) || p.can(Role::Logistics))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// The person behind a hire index: a generated chief below [`NAMED_HIRE_BASE`], a named person
/// for hire at or above it.
pub fn hire(index: u16) -> Option<Hire> {
    if index >= NAMED_HIRE_BASE {
        named_hires()
            .get((index - NAMED_HIRE_BASE) as usize)
            .map(|p| Hire {
                person: p.clone(),
                named: true,
            })
    } else {
        extras().get(index as usize).map(|e| Hire {
            person: e.person(),
            named: false,
        })
    }
}

/// Content problems with the cast, for dataset validation.
pub fn validate(people: &[Person]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for p in people {
        if !seen.insert(p.id.as_str()) {
            errors.push(format!("cast: duplicate id {}", p.id));
        }
        if p.can(Role::Chair) != p.stance.is_some() {
            errors.push(format!(
                "cast: {} needs a stance exactly when chair-capable",
                p.id
            ));
        }
        if p.stance.is_some_and(|s| !(-100..=100).contains(&s)) {
            errors.push(format!("cast: {} stance out of range", p.id));
        }
        if p.roles.is_empty() {
            errors.push(format!("cast: {} has no roles", p.id));
        }
    }
    if people.len() > 200 {
        errors.push("cast: too many people".to_owned());
    }
    errors
}

#[cfg(test)]
mod extras_tests {
    use super::*;

    #[test]
    fn the_cast_has_forty_people_with_the_first_twelve_in_place() {
        let file: CastFile = serde_json::from_str(CAST_JSON).unwrap();
        assert_eq!(file.people.len(), 40);
        assert!(validate(&file.people).is_empty());
        let first: Vec<&str> = file.people.iter().take(12).map(|p| p.id.as_str()).collect();
        assert_eq!(
            first,
            [
                "walter_pruitt",
                "marisol_ybarra",
                "desmond_broussard",
                "helen_kowalczyk",
                "kenji_watanabe_moore",
                "gloria_abernathy",
                "priya_raman",
                "joel_rosenthal",
                "linh_tran",
                "ingrid_vossberg",
                "emre_kaplan",
                "tobias_wendt"
            ]
        );
        for sc in [
            "texas-1981",
            "ruhr-rhine-1991",
            "silicon-valley-1995",
            "great-lakes-2000",
        ] {
            for role in [Role::UnionLeader, Role::Journalist, Role::Analyst] {
                assert!(scenario_person(&file.people, sc, role).is_some(), "{sc}");
            }
        }
    }

    #[test]
    fn named_hires_come_before_generated_ones_in_the_index_space() {
        let named = named_hires();
        assert_eq!(named.len(), 8);
        let h = hire(NAMED_HIRE_BASE).unwrap();
        assert!(h.named && h.person.id == named[0].id);
        assert!(hire(NAMED_HIRE_BASE + named.len() as u16).is_none());
        let g = hire(0).unwrap();
        assert!(!g.named && g.person.id == extras()[0].id);
    }

    #[test]
    fn extras_are_unique_and_do_not_reuse_the_cast() {
        let file: CastFile = serde_json::from_str(CAST_JSON).unwrap();
        assert!(file.extras.len() >= 64);
        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for p in &file.people {
            ids.insert(p.id.clone());
            names.insert(p.name.clone());
        }
        for e in &file.extras {
            assert!(ids.insert(e.id.clone()), "duplicate id {}", e.id);
            assert!(names.insert(e.name.clone()), "duplicate name {}", e.name);
        }
    }
}
