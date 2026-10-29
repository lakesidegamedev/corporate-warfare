//! The weekly brief for the apps: what changed, what is due and what to do about it.

use cw_session::brief::{self, Brief, BriefChange, BriefDue, BriefMove, MoveTarget};

use crate::SessionHandle;

#[derive(Clone, uniffi::Record)]
pub struct BriefChangeDto {
    /// profit, price, rival, event or mandate.
    pub kind: String,
    pub text: String,
}

#[derive(Clone, uniffi::Record)]
pub struct BriefDueDto {
    /// card, mandate_offer, mandate, loan or bid.
    pub kind: String,
    pub text: String,
    pub weeks_left: u32,
}

/// Where Review goes and how the dialog is filled in. Only the fields of the dialog named in
/// `dialog` carry values.
#[derive(Clone, uniffi::Record)]
pub struct BriefTargetDto {
    /// build, pricing, lane, mandate, raid, production, division or cfo.
    pub dialog: String,
    pub region: String,
    pub region_to: String,
    pub industry: String,
    pub recipe: u32,
    pub blocks: u32,
    pub product: u32,
    pub mode: u32,
    /// Lots for a lane; the offered position for a mandate.
    pub lots: i64,
    pub weeks: u32,
    pub target: u64,
    pub budget_cents: i64,
}

#[derive(Clone, uniffi::Record)]
pub struct BriefMoveDto {
    pub key: String,
    pub headline: String,
    pub detail: String,
    pub cost_cents: i64,
    pub weekly_gain_cents: i64,
    pub payback_weeks: i64,
    pub has_payback: bool,
    pub action_label: String,
    pub target: BriefTargetDto,
    /// Command intents as JSON the dialog would send. The brief never sends them.
    pub commands: Vec<String>,
}

#[derive(Clone, uniffi::Record)]
pub struct BriefDto {
    pub week: u32,
    pub changes: Vec<BriefChangeDto>,
    pub due: Vec<BriefDueDto>,
    pub moves: Vec<BriefMoveDto>,
    /// Why there is nothing to do; empty whenever there is a move.
    pub hold: String,
}

fn change_dto(c: BriefChange) -> BriefChangeDto {
    BriefChangeDto {
        kind: c.kind,
        text: c.text,
    }
}

fn due_dto(d: BriefDue) -> BriefDueDto {
    BriefDueDto {
        kind: d.kind,
        text: d.text,
        weeks_left: d.weeks_left,
    }
}

fn target_dto(t: MoveTarget) -> BriefTargetDto {
    BriefTargetDto {
        dialog: t.dialog,
        region: t.region,
        region_to: t.region_to,
        industry: t.industry,
        recipe: t.recipe,
        blocks: t.blocks,
        product: t.product,
        mode: t.mode,
        lots: t.lots,
        weeks: t.weeks,
        target: t.target,
        budget_cents: t.budget_cents,
    }
}

fn move_dto(m: BriefMove) -> BriefMoveDto {
    BriefMoveDto {
        key: m.key,
        headline: m.headline,
        detail: m.detail,
        cost_cents: m.cost_cents,
        weekly_gain_cents: m.weekly_gain_cents,
        payback_weeks: m.payback_weeks,
        has_payback: m.has_payback,
        action_label: m.action_label,
        target: target_dto(m.target),
        commands: m.commands,
    }
}

fn brief_dto(b: Brief) -> BriefDto {
    BriefDto {
        week: b.week,
        changes: b.changes.into_iter().map(change_dto).collect(),
        due: b.due.into_iter().map(due_dto).collect(),
        moves: b.moves.into_iter().map(move_dto).collect(),
        hold: b.hold,
    }
}

#[uniffi::export]
impl SessionHandle {
    /// The weekly brief for a company: up to three changes, what is due and up to three
    /// suggested moves with the engine's quote behind each. Read only.
    pub fn weekly_brief(&self, firm: u64) -> BriefDto {
        let session = self.lock();
        brief_dto(brief::weekly_brief(&session.world, &session.dataset, firm))
    }
}

#[cfg(test)]
mod tests {
    use crate::NewGameDto;

    #[test]
    fn the_brief_crosses_the_boundary_and_changes_nothing() {
        let career = crate::SessionHandle::start(NewGameDto {
            seed: 9,
            scenario_id: "great-lakes-2000".to_owned(),
            guided: true,
            five_year: false,
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            price_path: String::new(),
            timeline: String::new(),
        })
        .unwrap();
        for _ in 0..8 {
            career.advance(4);
            career.dismiss_pause();
        }
        let firm = career.overview().primary_firm.unwrap();
        let before = career.canonical_hash();
        let revision = career.revision();
        let brief = career.weekly_brief(firm);
        assert_eq!(brief.week, career.overview().tick);
        assert!(brief.changes.len() <= 3 && brief.moves.len() <= 3);
        assert!(
            !brief.changes.is_empty(),
            "a trading company always has a profit line"
        );
        for m in &brief.moves {
            assert!(!m.target.dialog.is_empty());
            assert!(m.commands.iter().all(|c| !c.is_empty()));
        }
        if brief.moves.is_empty() {
            assert!(!brief.hold.is_empty());
        }
        // Reading it twice agrees, and moves no hash and no revision.
        let again = career.weekly_brief(firm);
        assert_eq!(brief.changes.len(), again.changes.len());
        assert_eq!(brief.moves.len(), again.moves.len());
        assert_eq!(before, career.canonical_hash());
        assert_eq!(revision, career.revision());
        assert_eq!(career.weekly_brief(u64::MAX).moves.len(), 0);
    }
}
