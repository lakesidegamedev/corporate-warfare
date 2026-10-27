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
