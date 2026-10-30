//! Politics for the apps (1.9): the races with their candidates, polls and laws, and the public
//! tenders. The engine keeps the state (`cw_sim::economy::civics`); gifts and bids go through the
//! ordinary command path (`GiveToCandidate`, `BidOnTender`, in the action catalogue).

use cw_data::Dataset;
use cw_sim::economy::civics::{self, Candidate, PosKey, Race, StateLaws, TenderStatus};
use cw_sim::state::{Party, World};

use crate::SessionHandle;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PoliticalCandidateDto {
    /// Names the portrait.
    pub id: String,
    pub name: String,
    /// business or labour.
    pub party: String,
    /// Stated positions in plain words.
    pub positions: Vec<String>,
    pub funds_cents: i64,
    /// Poll points (basis points) the gifts have added.
    pub bonus_bps: i32,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ElectionRaceDto {
    /// The number to send with `GiveToCandidate` (255 is the national race).
    pub office: u32,
    pub place: String,
    pub governor: String,
    pub governor_id: String,
    pub governor_party: String,
    pub election_week: u32,
    pub weeks_left: u32,
    /// Gifts are allowed now.
    pub open: bool,
    /// The business candidate's share, 0 to 10,000.
    pub poll_bps: i32,
    /// Weekly poll history, thousandths, oldest first.
    pub history: Vec<i32>,
    pub candidates: Vec<PoliticalCandidateDto>,
    pub laws_in_force: Vec<String>,
    pub laws_pending: Vec<String>,
    pub pending_week: u32,
    pub cap_cents: i64,
    /// What your company has given to this race.
    pub you_gave_cents: i64,
    /// The company has plants in this state.
    pub near_you: bool,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PublicTenderDto {
    /// The number to send with `BidOnTender`.
    pub id: u32,
    pub place: String,
    pub state: String,
    /// road, port or power.
    pub kind: String,
    pub kind_label: String,
    pub budget_cents: i64,
    pub closes_week: u32,
    pub min_weeks: u32,
    pub max_weeks: u32,
    /// open, building, done or lapsed.
    pub status: String,
    pub bids: u32,
    pub your_price_cents: i64,
    pub your_weeks: u32,
    pub winner: String,
    pub price_cents: i64,
    pub promised_weeks: u32,
    /// Share of the works built, 0 to 10,000.
    pub progress_bps: i32,
    pub paid_cents: i64,
    /// What the works cost the contractor at the quickest and the slowest delivery.
    pub cost_quick_cents: i64,
    pub cost_slow_cents: i64,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct RegionLawsDto {
    pub region: String,
    pub state: String,
    pub lines: Vec<String>,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct PoliticsDto {
    pub week: u32,
    pub ready: bool,
    pub races: Vec<ElectionRaceDto>,
    pub tenders: Vec<PublicTenderDto>,
    pub region_laws: Vec<RegionLawsDto>,
    /// The warning shown beside the Give button.
    pub disclosure_note: String,
}

fn party_key(p: Party) -> String {
    p.key().to_owned()
}

fn position_text(k: PosKey, v: i32) -> String {
    match k {
        PosKey::RightToWork => {
            if v != 0 {
                "Right to work".to_owned()
            } else {
                "Keep union rules".to_owned()
            }
        }
        PosKey::CorporateTax => {
            if v.abs() >= 1_000 {
                format!("Corporate tax {}%", v / 100)
            } else if v < 0 {
                format!("Cut corporate tax {} points", -v / 100)
            } else {
                format!("Raise corporate tax {} points", v / 100)
            }
        }
        PosKey::Environment => match v {
            0 => "Lighter environmental rules".to_owned(),
            2 => "Stricter environmental rules".to_owned(),
            _ => "Keep environmental rules".to_owned(),
        },
        PosKey::CommonCarrier => {
            if v != 0 {
                "Common carrier rules".to_owned()
            } else {
                "No common carrier rules".to_owned()
            }
        }
        PosKey::MergerReview => format!("Merger review at {}% share", v / 100),
    }
}

fn candidate(c: &Candidate, race: &Race, index: usize) -> PoliticalCandidateDto {
    PoliticalCandidateDto {
        id: c.id.clone(),
        name: c.name.clone(),
        party: party_key(c.party),
        positions: c
            .positions
            .iter()
            .map(|p| position_text(p.key, p.value))
            .collect(),
        funds_cents: c.funds_cents,
        bonus_bps: race.bonus_bps(index),
    }
}

fn law_lines(laws: &StateLaws) -> Vec<String> {
    vec![
        if laws.right_to_work {
            "Right to work: unions organise more slowly".to_owned()
        } else {
            "Union rules as the nation sets them".to_owned()
        },
        format!(
            "Corporate tax {:+} points on the national rate",
            laws.tax_delta_bps / 100
        ),
        format!(
            "{} environmental rules for heavy industry",
            ["Lighter", "Standard", "Stricter"][laws.environment.min(2) as usize]
        ),
        format!(
            "Campaign gifts capped at ${} per company",
            laws.cap_cents / 100
        ),
    ]
}

pub fn politics(world: &World, ds: &Dataset, firm: Option<u64>) -> PoliticsDto {
    let me = firm.map(cw_core::ids::Handle);
    let week = world.tick.0;
    let c = &world.civics;
    let mut mine: Vec<usize> = me
        .and_then(|h| world.firms.get(h))
        .map(|f| {
            f.facilities
                .iter()
                .filter_map(|h| world.facilities.get(*h))
                .filter_map(|p| c.state_of(p.region))
                .collect()
        })
        .unwrap_or_default();
    mine.sort_unstable();
    mine.dedup();
    let mut races: Vec<ElectionRaceDto> = Vec::new();
    if let Some(n) = c.national.as_ref() {
        let leader = world.politics.party;
        races.push(ElectionRaceDto {
            office: civics::NATIONAL as u32,
            place: "The country".to_owned(),
            governor: n
                .candidates
                .get(match leader {
                    Party::Business => 0,
                    Party::Labour => 1,
                })
                .map(|x| x.name.clone())
                .unwrap_or_default(),
            governor_id: String::new(),
            governor_party: party_key(leader),
            election_week: n.election,
            weeks_left: n.election.saturating_sub(week),
            open: n.open(week),
            poll_bps: n.poll_bps,
            history: n.history.iter().map(|x| *x as i32).collect(),
            candidates: n
                .candidates
                .iter()
                .enumerate()
                .map(|(i, x)| candidate(x, n, i))
                .collect(),
            laws_in_force: Vec::new(),
            laws_pending: Vec::new(),
            pending_week: 0,
            cap_cents: civics::NATIONAL_CAP_CENTS,
            you_gave_cents: me.map_or(0, |h| n.given_by(h)),
            near_you: true,
        });
    }
    for (i, s) in c.states.iter().enumerate() {
        let r = &s.race;
        races.push(ElectionRaceDto {
            office: i as u32,
            place: ds.states[i].name.clone(),
            governor: s.governor.name.clone(),
            governor_id: s.governor.id.clone(),
            governor_party: party_key(s.governor.party),
            election_week: r.election,
            weeks_left: r.election.saturating_sub(week),
            open: r.open(week),
            poll_bps: r.poll_bps,
            history: r.history.iter().map(|x| *x as i32).collect(),
            candidates: r
                .candidates
                .iter()
                .enumerate()
                .map(|(k, x)| candidate(x, r, k))
                .collect(),
            laws_in_force: law_lines(&s.laws),
            laws_pending: s
                .pending
                .as_ref()
                .map(|(_, l)| law_lines(l))
                .unwrap_or_default(),
            pending_week: s.pending.as_ref().map_or(0, |(w, _)| *w),
            cap_cents: s.laws.cap_cents,
            you_gave_cents: me.map_or(0, |h| r.given_by(h)),
            near_you: mine.contains(&i),
        });
    }
    // Races near the company first, then the soonest vote.
    races.sort_by_key(|r| (!r.near_you, r.weeks_left, r.office));
    let tenders = c
        .tenders
        .iter()
        .rev()
        .take(20)
        .map(|t| {
            let mine = me.and_then(|h| t.bids.iter().find(|b| b.firm == h));
            PublicTenderDto {
                id: t.id,
                place: world.regions[t.region as usize].name.clone(),
                state: ds
                    .states
                    .get(t.state as usize)
                    .map(|s| s.name.clone())
                    .unwrap_or_default(),
                kind: t.kind.key().to_owned(),
                kind_label: t.kind.label().to_owned(),
                budget_cents: t.budget_cents,
                closes_week: t.closes,
                min_weeks: t.min_weeks,
                max_weeks: t.max_weeks,
                status: match t.status {
                    TenderStatus::Open => "open",
                    TenderStatus::Building => "building",
                    TenderStatus::Done => "done",
                    TenderStatus::Lapsed => "lapsed",
                }
                .to_owned(),
                bids: t.bids.len() as u32,
                your_price_cents: mine.map_or(0, |b| b.price_cents),
                your_weeks: mine.map_or(0, |b| b.weeks),
                winner: t
                    .winner
                    .and_then(|w| world.firm(w))
                    .map(|f| f.name.clone())
                    .unwrap_or_default(),
                price_cents: t.price_cents,
                promised_weeks: t.promised_weeks,
                progress_bps: if t.cost_cents > 0 {
                    (t.cost_paid_cents * 10_000 / t.cost_cents).clamp(0, 10_000) as i32
                } else {
                    0
                },
                paid_cents: t.paid_cents,
                cost_quick_cents: civics::delivery_cost(world, t, t.min_weeks),
                cost_slow_cents: civics::delivery_cost(world, t, t.max_weeks),
            }
        })
        .collect();
    let region_laws = ds
        .regions
        .iter()
        .enumerate()
        .filter_map(|(r, region)| {
            let s = c.state_of(r as u8).and_then(|i| c.states.get(i))?;
            let mut lines = law_lines(&s.laws);
            lines.truncate(3);
            let i = c.infra.get(r).copied().unwrap_or_default();
            if i.road_bps > 0 || i.port_bps > 0 || i.grid_bps > 0 {
                lines.push(format!(
                    "Public works: roads +{}%, port +{}%, grid +{}",
                    i.road_bps / 100,
                    i.port_bps / 100,
                    i.grid_bps / 100
                ));
            }
            Some(RegionLawsDto {
                region: region.code.clone(),
                state: s.governor.name.clone(),
                lines,
            })
        })
        .collect();
    PoliticsDto {
        week,
        ready: c.ready,
        races,
        tenders,
        region_laws,
        disclosure_note: "Gifts are printed in the paper next week with your company's name. The other side will hold it against you a little; your candidate's side will remember a win.".to_owned(),
    }
}

#[uniffi::export]
impl SessionHandle {
    /// Races, polls, candidates, state laws and public tenders, with what the company has given.
    pub fn politics(&self, firm: Option<u64>) -> PoliticsDto {
        let session = self.lock();
        politics(&session.world, &session.dataset, firm)
    }
}
