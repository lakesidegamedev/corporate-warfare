//! Chapters, board mandates and achievements for the app (1.9): what one career knows.
//!
//! The engine keeps the state (`cw_sim::economy::chapters`, `achievements`); this module turns
//! it into plain-language records. Cross-career storage (the Hall of Careers proper) is the app's;
//! `hall_of_careers` returns this career's page of it. Taking a mandate goes through the usual
//! command path (`ChooseMandate`, in the action catalogue).

use cw_data::Dataset;
use cw_sim::economy::achievements;
use cw_sim::economy::chapters::{self, ChapterLog, Mandate, MandateKind, Outcome};
use cw_sim::state::{INDUSTRY_NAMES, World};

use crate::SessionHandle;

/// One mandate the board put on the table, or the one in force.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct MandateDto {
    /// Position in the offer, the number to send with `ChooseMandate`.
    pub index: u32,
    pub key: String,
    pub title: String,
    pub detail: String,
    /// Progress toward the target, 0 to 10,000. Zero for an offer not yet taken.
    pub progress_bps: i32,
    /// A plain sentence on where it stands now.
    pub progress_text: String,
    /// A cash cover week fell short; the mandate can no longer be met this chapter.
    pub lost: bool,
}

/// One chapter of the career.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ChapterDto {
    pub number: u32,
    /// "Chapter 3: The Squeeze".
    pub name: String,
    pub start_week: u32,
    pub end_week: u32,
    /// "current" or "closed".
    pub status: String,
    /// The company whose board made the offer; zero when there was none.
    pub firm: u64,
    /// The three mandates offered (one for a chapter whose offer was thinner).
    pub offered: Vec<MandateDto>,
    /// Index into `offered` of the mandate taken, if one was.
    pub chosen: Option<u32>,
    /// "open" while undecided, then "met", "missed", "lapsed" or "cancelled".
    pub outcome: String,
    pub closed_week: u32,
    pub bonus_cents: i64,
    /// The change to the company's reputation the result made, in basis points.
    pub reputation_bps: i32,
    /// The offer can still be taken: weeks left in the window, else zero.
    pub offer_weeks_left: u32,
}

/// One achievement of the fixed list.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct AchievementDto {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub earned: bool,
    /// The week it was earned, zero while locked.
    pub week: u32,
}

/// This career's page of the Hall of Careers.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct HallDto {
    pub weeks: u32,
    pub chapters: Vec<ChapterDto>,
    pub achievements: Vec<AchievementDto>,
    pub mandates_met: u32,
    pub mandates_missed: u32,
    pub achievements_earned: u32,
}

fn money(cents: i64) -> String {
    let dollars = cents / 100;
    if dollars.abs() >= 1_000 {
        format!("${}K", dollars / 1_000)
    } else {
        format!("${dollars}")
    }
}

fn pct(bps: i64) -> String {
    format!("{}.{}%", bps / 100, (bps % 100).abs() / 10)
}

fn ratio(bps: i64) -> String {
    format!("{}.{:02}", bps / 10_000, (bps % 10_000).abs() / 100)
}

fn industry(index: u8) -> String {
    INDUSTRY_NAMES
        .get(index as usize)
        .map(|n| n.replace('_', " "))
        .unwrap_or_default()
}

fn product(dataset: &Dataset, code: u16) -> String {
    dataset
        .products
        .get(code as usize)
        .map(|p| p.name.to_lowercase())
        .unwrap_or_default()
}

/// Player-facing title and detail of a mandate.
pub(crate) fn mandate_copy(dataset: &Dataset, kind: &MandateKind) -> (String, String) {
    match kind {
        MandateKind::MarketShare { region, industry: i, target_bps } => (
            format!(
                "Take {} of {} in {}",
                pct(*target_bps),
                industry(*i),
                dataset.regions[*region as usize].name
            ),
            "Hold at least this share of the market when the chapter ends. Share counts capacity and unmet imports.".to_owned(),
        ),
        MandateKind::Integrate { product: code } => (
            format!("Make your own {}", product(dataset, *code)),
            format!(
                "Own a working plant that makes {}, which your plants buy today.",
                product(dataset, *code)
            ),
        ),
        MandateKind::ListSubsidiary { .. } => (
            "List a subsidiary".to_owned(),
            "Float one of your group's companies on the exchange before the chapter ends.".to_owned(),
        ),
        MandateKind::CashCover { cover_weeks } => (
            format!("Keep {cover_weeks} weeks of costs in cash"),
            "Cash must stay above this many weeks of the company's running costs in every week of the chapter.".to_owned(),
        ),
        MandateKind::WinRaid => (
            "Win a raid".to_owned(),
            "Take plants from a rival in a raid before the chapter ends.".to_owned(),
        ),
        MandateKind::ProfitGrowth { target_cents, .. } => (
            format!("Lift operating profit to {} a week", money(*target_cents)),
            "Average weekly operating profit over the last 13 weeks, measured when the chapter ends.".to_owned(),
        ),
        MandateKind::DebtToEquity { target_bps } => (
            format!("Cut debt to {} times equity", ratio(*target_bps)),
            "Debt divided by equity must be at or below this when the chapter ends.".to_owned(),
        ),
    }
}

/// Where a mandate stands now, as a sentence.
pub(crate) fn progress_text(
    world: &World,
    dataset: &Dataset,
    mandate: &Mandate,
    progress: chapters::Progress,
) -> String {
    match &mandate.kind {
        MandateKind::MarketShare { target_bps, .. } => format!(
            "You hold {} of the market. The target is {}.",
            pct(progress.now),
            pct(*target_bps)
        ),
        MandateKind::Integrate { product: code } => {
            if mandate.met {
                format!("You make {} yourself now.", product(dataset, *code))
            } else {
                format!("You do not make {} yet.", product(dataset, *code))
            }
        }
        MandateKind::ListSubsidiary { .. } => {
            if mandate.met {
                "A subsidiary is listed.".to_owned()
            } else {
                "No subsidiary has listed yet.".to_owned()
            }
        }
        MandateKind::CashCover { cover_weeks } => {
            if mandate.broken {
                "Cash fell below the line, so this mandate is lost.".to_owned()
            } else {
                format!(
                    "You hold {}.{} weeks of costs in cash. The line is {}.",
                    progress.now / 10,
                    progress.now % 10,
                    cover_weeks
                )
            }
        }
        MandateKind::WinRaid => {
            if mandate.met {
                "You won a raid.".to_owned()
            } else {
                "No raid won this chapter yet.".to_owned()
            }
        }
        MandateKind::ProfitGrowth { target_cents, .. } => format!(
            "Operating profit is {} a week. The target is {}.",
            money(progress.now),
            money(*target_cents)
        ),
        MandateKind::DebtToEquity { target_bps } => {
            let _ = world;
            if progress.bps >= 10_000 {
                format!(
                    "Debt is {} times equity, inside the target of {}.",
                    ratio(progress.now),
                    ratio(*target_bps)
                )
            } else {
                format!(
                    "Debt is {} times equity. The target is {}.",
                    ratio(progress.now),
                    ratio(*target_bps)
                )
            }
        }
    }
}

fn offered_dto(dataset: &Dataset, index: usize, kind: &MandateKind) -> MandateDto {
    let (title, detail) = mandate_copy(dataset, kind);
    MandateDto {
        index: index as u32,
        key: kind.key().to_owned(),
        title,
        detail,
        progress_bps: 0,
        progress_text: String::new(),
        lost: false,
    }
}

fn outcome_key(outcome: Option<Outcome>) -> &'static str {
    match outcome {
        None => "open",
        Some(Outcome::Met) => "met",
        Some(Outcome::Missed) => "missed",
        Some(Outcome::Lapsed) => "lapsed",
        Some(Outcome::Cancelled) => "cancelled",
    }
}

fn chapter_dto(world: &World, dataset: &Dataset, log: &ChapterLog) -> ChapterDto {
    let current = log.outcome.is_none();
    let window = cw_data::chapters::mandate_table().offer_window_weeks;
    let mut offered: Vec<MandateDto> = log
        .offered
        .iter()
        .enumerate()
        .map(|(i, kind)| offered_dto(dataset, i, kind))
        .collect();
    let active = log
        .firm
        .and_then(|f| world.chapters.active.get(&f))
        .filter(|m| m.chapter == log.number);
    if let (Some(index), Some(mandate)) = (log.chosen, active) {
        let progress = chapters::progress(world, dataset, mandate);
        if let Some(slot) = offered.get_mut(index as usize) {
            slot.progress_bps = progress.bps as i32;
            slot.progress_text = progress_text(world, dataset, mandate, progress);
            slot.lost = progress.broken;
        }
    }
    let offer_weeks_left = log
        .firm
        .and_then(|f| world.chapters.offers.get(&f))
        .filter(|o| o.chapter == log.number)
        .map(|o| (o.set + window).saturating_sub(world.tick.0))
        .unwrap_or(0);
    ChapterDto {
        number: log.number,
        name: log.name.clone(),
        start_week: log.start,
        end_week: chapters::chapter_start(log.number + 1) - 1,
        status: if current { "current" } else { "closed" }.to_owned(),
        firm: log.firm.map(|f| f.0).unwrap_or(0),
        offered,
        chosen: log.chosen.map(u32::from),
        outcome: outcome_key(log.outcome).to_owned(),
        closed_week: log.closed,
        bonus_cents: log.bonus_cents,
        reputation_bps: log.reputation_bps,
        offer_weeks_left,
    }
}

pub(crate) fn hall(world: &World, dataset: &Dataset) -> HallDto {
    let chapters_out: Vec<ChapterDto> = world
        .chapters
        .log
        .iter()
        .map(|log| chapter_dto(world, dataset, log))
        .collect();
    let met = world
        .chapters
        .results()
        .filter(|o| *o == Outcome::Met)
        .count() as u32;
    let missed = world
        .chapters
        .results()
        .filter(|o| *o == Outcome::Missed)
        .count() as u32;
    let list: Vec<AchievementDto> = achievements::LIST
        .iter()
        .map(|item| {
            let tick = world
                .chapters
                .achievements
                .iter()
                .find(|e| e.id == item.id)
                .map(|e| e.tick);
            AchievementDto {
                id: item.id.to_owned(),
                title: item.title.to_owned(),
                detail: item.detail.to_owned(),
                earned: tick.is_some(),
                week: tick.unwrap_or(0),
            }
        })
        .collect();
    HallDto {
        weeks: world.tick.0,
        achievements_earned: list.iter().filter(|a| a.earned).count() as u32,
        chapters: chapters_out,
        achievements: list,
        mandates_met: met,
        mandates_missed: missed,
    }
}

#[uniffi::export]
impl SessionHandle {
    /// This career's chapters (oldest first, the current one last), the mandates offered and
    /// taken with their outcomes and live progress, and the achievements with their dates.
    pub fn hall_of_careers(&self) -> HallDto {
        let session = self.lock();
        hall(&session.world, &session.dataset)
    }
}
