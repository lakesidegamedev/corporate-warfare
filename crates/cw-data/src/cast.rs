//! The character cast: fictional people who run rival firms, chair the central bank, govern
//! states and lead the parties.
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
}

impl Person {
    pub fn can(&self, role: Role) -> bool {
        self.roles.contains(&role)
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
