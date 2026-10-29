//! Letters for the apps: the inbox as correspondence, with read flags.

use cw_session::Session;
use cw_session::letters::{self, Letter, LetterChoice, Sender};

use crate::SessionHandle;
use crate::paper::PaperLinkDto;

/// Who wrote a letter. `person` names the portrait in `cast-portraits.json`, or is empty when the
/// sender is an office or a role.
#[derive(Clone, uniffi::Record)]
pub struct LetterSenderDto {
    pub person: String,
    pub name: String,
    pub role: String,
    pub organisation: String,
    /// integrator, raider, financier, speculator, builder or cautious; empty for an office.
    pub temperament: String,
}

#[derive(Clone, uniffi::Record)]
pub struct LetterChoiceDto {
    pub id: String,
    pub label: String,
}

#[derive(Clone, uniffi::Record)]
pub struct LetterDto {
    pub id: String,
    /// rival, ally, bid, rates, government, union, cfo, analyst or decision.
    pub kind: String,
    pub week: u32,
    pub sender: LetterSenderDto,
    pub salutation: String,
    pub paragraphs: Vec<String>,
    pub sign_off: String,
    /// The fact in one line, for the list.
    pub summary: String,
    /// The decision card the letter carries; answer it with `respond`.
    pub event: Option<u64>,
    pub choices: Vec<LetterChoiceDto>,
    pub mandatory: bool,
    /// The week an answer is due, or zero.
    pub deadline: u32,
    /// What Open goes to: company, region, good, deal, event or none.
    pub link: PaperLinkDto,
    pub read: bool,
}

fn sender_dto(s: Sender) -> LetterSenderDto {
    LetterSenderDto {
        person: s.person,
        name: s.name,
        role: s.role,
        organisation: s.organisation,
        temperament: s.temperament,
    }
}

fn choice_dto(c: LetterChoice) -> LetterChoiceDto {
    LetterChoiceDto {
        id: c.id,
        label: c.label,
    }
}

fn letter_dto(l: Letter) -> LetterDto {
    LetterDto {
        id: l.id,
        kind: l.kind,
        week: l.week,
        sender: sender_dto(l.sender),
        salutation: l.salutation,
        paragraphs: l.paragraphs,
        sign_off: l.sign_off,
        summary: l.summary,
        event: l.event,
        choices: l.choices.into_iter().map(choice_dto).collect(),
        mandatory: l.mandatory,
        deadline: l.deadline,
        link: PaperLinkDto {
            kind: l.link.kind,
            id: l.link.id,
            key: l.link.key,
        },
        read: l.read,
    }
}

#[uniffi::export]
impl SessionHandle {
    /// Letters dated `since_week` or later: decisions pinned on top, then newest first. The
    /// Inbox asks for the last few weeks; the archive asks for more.
    pub fn letters(&self, since_week: u32) -> Vec<LetterDto> {
        let session = self.lock();
        letters::letters(&session.world, &session.dataset, since_week)
            .into_iter()
            .map(letter_dto)
            .collect()
    }

    /// Mark letters read. Unknown ids are ignored. Read flags live outside the hashed world.
    pub fn mark_letters_read(&self, ids: Vec<String>) {
        let mut session = self.lock();
        let Session { world, dataset, .. } = &mut *session;
        letters::mark_read(world, dataset, &ids);
    }

    /// Unread letters in the Inbox window, for the badge.
    pub fn unread_letters(&self) -> u32 {
        let session = self.lock();
        letters::unread_count(&session.world, &session.dataset) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NewGameDto;
    use std::sync::Arc;

    fn career(seed: u64, scenario: &str) -> Arc<SessionHandle> {
        SessionHandle::start(NewGameDto {
            seed,
            scenario_id: scenario.to_owned(),
            guided: true,
            five_year: false,
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            price_path: String::new(),
            timeline: String::new(),
        })
        .unwrap()
    }

    #[test]
    fn letters_cross_the_boundary_and_reading_clears_the_badge() {
        let career = career(9, "texas-1981");
        let mut left = 60;
        while career.overview().tick < 40 && left > 0 {
            career.advance(4);
            career.dismiss_pause();
            left -= 1;
        }
        let now = career.overview().tick;
        let all = career.letters(1);
        assert!(!all.is_empty(), "forty weeks of play write some letters");
        for l in &all {
            assert!(!l.id.is_empty() && !l.paragraphs.is_empty() && l.paragraphs.len() <= 3);
            assert!(!l.sender.name.is_empty() && !l.salutation.is_empty());
            assert!(l.paragraphs.join(" ").chars().count() <= letters::MAX_CHARS);
            assert!(l.week <= now);
        }
        let recent = career.letters(now.saturating_sub(letters::INBOX_WEEKS));
        let unread = career.unread_letters();
        assert_eq!(unread as usize, recent.iter().filter(|l| !l.read).count());
        let ids: Vec<String> = recent.iter().map(|l| l.id.clone()).collect();
        career.mark_letters_read(ids);
        assert_eq!(career.unread_letters(), 0);
        assert!(
            career
                .letters(1)
                .iter()
                .filter(|l| ids_in(&recent, l))
                .all(|l| l.read)
        );
        // A save keeps the flags.
        let bytes = career.export_save().unwrap();
        let restored = crate::import_save(bytes).unwrap();
        assert_eq!(restored.unread_letters(), 0);
    }

    fn ids_in(recent: &[LetterDto], l: &LetterDto) -> bool {
        recent.iter().any(|r| r.id == l.id)
    }
}
