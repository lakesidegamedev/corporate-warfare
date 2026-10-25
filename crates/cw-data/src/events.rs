//! The authored event deck.
//!
//! An event card never invents a second penalty engine. Each choice names a handler that the
//! simulation implements against the same rules, commands and ledger used everywhere else.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventChoice {
    pub id: String,
    pub label: String,
    pub handler: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventCard {
    pub id: String,
    pub title: String,
    pub trigger: String,
    pub mandatory: bool,
    pub cooldown_weeks: u32,
    pub choices: Vec<EventChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventDeck {
    pub schema: u32,
    #[serde(default)]
    pub classification: String,
    pub events: Vec<EventCard>,
}

impl EventCard {
    pub fn has_handler(&self, handler: &str) -> bool {
        self.choices.iter().any(|choice| choice.handler == handler)
    }
}
