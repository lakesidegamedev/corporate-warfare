//! Which letters the player has opened.
//!
//! Letters are written from the world's records each time they are asked for, so only the read
//! flags are kept. They live outside the hashed world and in their own save trailer, like the
//! history archive: no rule reads them and an older save loads with every letter unread.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Weeks a read flag is kept after its letter's week. Older letters have left the archive.
pub const KEEP_WEEKS: u32 = 60;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailState {
    /// Letter id to the week the letter is dated.
    pub read: BTreeMap<String, u32>,
}

impl MailState {
    pub fn is_empty(&self) -> bool {
        self.read.is_empty()
    }

    pub fn is_read(&self, id: &str) -> bool {
        self.read.contains_key(id)
    }

    /// Mark a letter read, then forget flags for letters too old to show.
    pub fn mark(&mut self, id: &str, letter_week: u32, now: u32) {
        self.read.insert(id.to_owned(), letter_week);
        let oldest = now.saturating_sub(KEEP_WEEKS);
        self.read.retain(|_, week| *week >= oldest);
    }
}
