//! Government and laws: two parties, an election every four years, and the laws the winner sets.
//!
//! Approval drifts toward a level set by the economy: growth above trend helps, inflation above
//! target and high interest rates hurt, and a recession hurts most. On election week the
//! governing party holds on with a chance equal to its approval, a little better for having the
//! office. A new government announces its programme at once and it takes effect thirteen weeks
//! later: a pro-business government cuts corporate tax, lowers the low-skill wage floor in
//! relative terms, lets bigger mergers through and weakens union law; a pro-labour government
//! does the opposite. Each law then feeds the system that already uses it.

use crate::state::{CyclePhase, LawChange, Laws, Party, PoliticalNews, UnionLaw, World};
use cw_core::rng::Rng;

/// Weeks between elections: four game years.
pub const TERM_WEEKS: u32 = 208;
/// Weeks from an election to the new laws taking effect.
pub const LAW_DELAY_WEEKS: u32 = 13;
const NEWS_KEPT: usize = 40;
const TAG_POLITICS: u64 = 0x504f_4c49_5449_4353;

/// The programme each party brings into office.
pub fn programme(party: Party, current: &Laws) -> Laws {
    match party {
        Party::Business => Laws {
            corporate_tax_bps: (current.corporate_tax_bps - 300).max(1_500),
            wage_floor_cents: current.wage_floor_cents,
            merger_review_bps: 4_000,
            unions: UnionLaw::Weak,
        },
        Party::Labour => Laws {
            corporate_tax_bps: (current.corporate_tax_bps + 300).min(3_800),
            wage_floor_cents: current.wage_floor_cents * 110 / 100,
            merger_review_bps: 3_000,
            unions: UnionLaw::Strong,
        },
    }
}

pub fn party_name(party: Party) -> &'static str {
    match party {
        Party::Business => "the Commerce Party",
        Party::Labour => "the Workers' Party",
    }
}

/// Run one week of politics.
pub fn weekly(world: &mut World) {
    let week = world.tick.0;
    let cycle = &world.macro_state.cycle;
    let inflation = world.macro_state.inflation_bps;
    let rate = world.macro_state.policy_rate_bps;
    let mut target =
        5_000 + cycle.output_gap_bps * 6 - (inflation - 200).max(0) * 4 - (rate - 500).max(0) * 2;
    if cycle.phase == CyclePhase::Recession {
        target -= 800;
    }
    let target = target.clamp(1_500, 8_500);
    let p = &mut world.politics;
    p.approval_bps += (target - p.approval_bps) / 20;

    // Announced laws come into force.
    if let Some(change) = p.pending
        && week >= change.effective
    {
        p.laws = change.laws;
        p.pending = None;
        p.news.push(PoliticalNews {
            week,
            text: format!(
                "New laws in force: corporate tax {}%, low-skill wage floor ${}/hour, merger review at {}% share, {} union law.",
                p.laws.corporate_tax_bps / 100,
                p.laws.wage_floor_cents / 100 / 40,
                p.laws.merger_review_bps / 100,
                p.laws.unions.key()
            ),
        });
    }

    if week >= p.next_election {
        let mut rng =
            Rng::entity_stream(world.seed, TAG_POLITICS, cw_core::ids::Handle(week as u64));
        let hold = (p.approval_bps + 300).clamp(500, 9_500);
        let kept = rng.chance_bps(hold);
        let year = cw_core::calendar::Tick(week).year();
        if kept {
            p.news.push(PoliticalNews {
                week,
                text: format!(
                    "{year} election: {} keeps power with {}% approval.",
                    party_name(p.party),
                    p.approval_bps / 100
                ),
            });
        } else {
            p.party = p.party.other();
            p.since_week = week;
            p.approval_bps = 5_500;
            let laws = programme(p.party, &p.laws);
            p.pending = Some(LawChange {
                announced: week,
                effective: week + LAW_DELAY_WEEKS,
                laws,
            });
            p.news.push(PoliticalNews {
                week,
                text: format!(
                    "{year} election: {} wins. Its programme takes effect in {} weeks: corporate tax {}%, merger review at {}% share, {} union law{}.",
                    party_name(p.party),
                    LAW_DELAY_WEEKS,
                    laws.corporate_tax_bps / 100,
                    laws.merger_review_bps / 100,
                    laws.unions.key(),
                    if laws.wage_floor_cents > p.laws.wage_floor_cents { ", a higher wage floor" } else { "" }
                ),
            });
        }
        p.next_election = week + TERM_WEEKS;
    }
    let excess = p.news.len().saturating_sub(NEWS_KEPT);
    p.news.drain(..excess);

    // The wage floor holds up low-skill background pay everywhere.
    let floor = world.politics.laws.wage_floor_cents;
    for region in &mut world.regions {
        if region.background_wage_cents[0] < floor {
            region.background_wage_cents[0] = floor;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_core::calendar::Tick;

    #[test]
    fn elections_come_every_four_years_and_new_governments_change_the_laws() {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut changed = false;
        for seed in 1..12u64 {
            let mut world =
                seed::build(&dataset, seed, ScenarioKind::Guided, ScenarioMode::Sandbox);
            for week in 1..=(TERM_WEEKS * 3 + 60) {
                world.tick = Tick(week);
                crate::economy::macro_cycle::weekly(&mut world, &dataset);
                weekly(&mut world);
            }
            let elections = world
                .politics
                .news
                .iter()
                .filter(|n| n.text.contains("election"))
                .count();
            assert_eq!(elections, 4, "elections in weeks 45, 253, 461 and 669");
            if world.politics.laws != Laws::default() {
                changed = true;
            }
        }
        assert!(changed, "some seeds change government and the laws follow");
    }
}
