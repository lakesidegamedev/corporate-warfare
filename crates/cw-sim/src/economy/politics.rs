//! Government and laws: two parties, an election every four years, and the laws the winner sets.
//!
//! Approval drifts toward a level set by the economy: growth above trend helps, inflation above
//! target and high interest rates hurt, and a recession hurts most. On election week the
//! governing party holds on with a chance equal to its approval, a little better for having the
//! office. A new government announces its programme at once and it takes effect thirteen weeks
//! later: a pro-business government cuts corporate tax, lowers the low-skill wage floor in
//! relative terms, lets bigger mergers through and weakens union law; a pro-labour government
//! does the opposite. Each law then feeds the system that already uses it.

use crate::state::{CyclePhase, LawChange, Laws, Party, PoliticalNews, Rescue, UnionLaw, World};
use cw_core::rng::Rng;
use cw_data::Dataset;

/// Weeks between elections: four game years.
pub const TERM_WEEKS: u32 = 208;
/// Weeks from an election to the new laws taking effect.
pub const LAW_DELAY_WEEKS: u32 = 13;
const NEWS_KEPT: usize = 40;
const TAG_POLITICS: u64 = 0x504f_4c49_5449_4353;
/// Output cap in a quota state, as a share of capacity, while the price is soft.
pub const QUOTA_BPS: i32 = 8_000;
/// Quotas bite while a good's price is under this share of its year average.
pub const QUOTA_PRICE_PCT: i64 = 92;

/// The state (index in code order) a region belongs to.
pub fn state_index(ds: &Dataset, region: u8) -> Option<usize> {
    let code = ds.regions.get(region as usize)?.state().to_owned();
    ds.states.iter().position(|s| s.code == code)
}

/// States that live off extraction: the ones a producer government sets quotas in.
fn producer_states(ds: &Dataset) -> u64 {
    ds.states
        .iter()
        .enumerate()
        .take(64)
        .filter(|(_, s)| s.specialization.iter().any(|x| x == "extraction"))
        .fold(0u64, |mask, (i, _)| mask | (1 << i))
}

/// The laws that concern logistics, markets and the state's role in firms, set by the party.
fn with_programme(party: Party, mut laws: Laws, ds: &Dataset) -> Laws {
    match party {
        Party::Business => {
            laws.common_carrier = false;
            laws.vertical_review_bps = 0;
            laws.reserve = false;
            laws.rescue = Rescue::Bailout;
            laws.privatise = true;
            laws.quota_bps = QUOTA_BPS;
            laws.quota_states = producer_states(ds);
        }
        Party::Labour => {
            laws.common_carrier = true;
            laws.vertical_review_bps = 3_500;
            laws.reserve = true;
            laws.rescue = Rescue::Nationalise;
            laws.privatise = false;
            laws.quota_bps = 0;
            laws.quota_states = 0;
        }
    }
    laws
}

/// The programme each party brings into office.
pub fn programme(party: Party, current: &Laws, ds: &Dataset) -> Laws {
    let base = match party {
        Party::Business => Laws {
            corporate_tax_bps: (current.corporate_tax_bps - 300).max(1_500),
            wage_floor_cents: current.wage_floor_cents,
            merger_review_bps: 4_000,
            unions: UnionLaw::Weak,
            ..*current
        },
        Party::Labour => Laws {
            corporate_tax_bps: (current.corporate_tax_bps + 300).min(3_800),
            wage_floor_cents: current.wage_floor_cents * 110 / 100,
            merger_review_bps: 3_000,
            unions: UnionLaw::Strong,
            ..*current
        },
    };
    with_programme(party, base, ds)
}

/// The market and state-role laws of a programme without its tax, wage and union laws.
fn structural(laws: &Laws) -> (bool, i64, bool, Rescue, bool, i32, u64) {
    (
        laws.common_carrier,
        laws.vertical_review_bps,
        laws.reserve,
        laws.rescue,
        laws.privatise,
        laws.quota_bps,
        laws.quota_states,
    )
}

/// A short account of the logistics and state-role laws, for news.
pub fn describe(laws: &Laws) -> String {
    let mut parts: Vec<String> = Vec::new();
    if laws.common_carrier {
        parts.push("common carrier rules for pipelines, rail and ships".to_owned());
    }
    if laws.quota_bps > 0 && laws.quota_states != 0 {
        parts.push(format!(
            "production quotas in {} states",
            laws.quota_states.count_ones()
        ));
    }
    if laws.vertical_review_bps > 0 {
        parts.push(format!(
            "supplier and customer deals reviewed above {}% share",
            laws.vertical_review_bps / 100
        ));
    }
    if laws.reserve {
        parts.push("a strategic reserve".to_owned());
    }
    match laws.rescue {
        Rescue::Bailout => parts.push("bailouts for failing strategic firms".to_owned()),
        Rescue::Nationalise => parts.push("nationalisation of failing strategic firms".to_owned()),
        Rescue::None => {}
    }
    if parts.is_empty() {
        "no new market rules".to_owned()
    } else {
        parts.join(", ")
    }
}

pub fn party_name(party: Party) -> &'static str {
    match party {
        Party::Business => "the Commerce Party",
        Party::Labour => "the Workers' Party",
    }
}

/// Run one week of politics.
pub fn weekly(world: &mut World, ds: &Dataset) {
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
    // The starting government's market and state-role laws are in force from the first week.
    if !p.programmed {
        p.programmed = true;
        p.laws = with_programme(p.party, p.laws, ds);
    }

    // Announced laws come into force.
    if let Some(change) = p.pending
        && week >= change.effective
    {
        p.laws = change.laws;
        p.pending = None;
        p.law_changes += 1;
        p.last_effective = week;
        p.news.push(PoliticalNews {
            person: None,
            week,
            text: format!(
                "New laws in force: corporate tax {}%, low-skill wage floor ${}/hour, merger review at {}% share, {} union law; {}.",
                p.laws.corporate_tax_bps / 100,
                p.laws.wage_floor_cents / 100 / 40,
                p.laws.merger_review_bps / 100,
                p.laws.unions.key(),
                describe(&p.laws)
            ),
        });
    }

    let mut held: Option<bool> = None;
    if week >= p.next_election {
        let mut rng =
            Rng::entity_stream(world.seed, TAG_POLITICS, cw_core::ids::Handle(week as u64));
        // Campaign gifts move the chance a little, never beyond the bounded poll swing.
        let swing = crate::economy::civics::national_swing_bps_for(p.party, &world.civics);
        let hold = (p.approval_bps + 300 + swing).clamp(500, 9_500);
        let kept = rng.chance_bps(hold);
        let year = cw_core::calendar::Tick(week).year_from(world.start_year);
        if kept {
            p.news.push(PoliticalNews {
                person: None,
                week,
                text: format!(
                    "{year} election: {} keeps power with {}% approval.",
                    party_name(p.party),
                    p.approval_bps / 100
                ),
            });
            // A government returned to office reasserts its market programme where it has
            // slipped, for example after the country's first laws were set by hand.
            let again = with_programme(p.party, p.laws, ds);
            if p.pending.is_none() && structural(&again) != structural(&p.laws) {
                p.pending = Some(LawChange {
                    announced: week,
                    effective: week + LAW_DELAY_WEEKS,
                    laws: again,
                });
            }
        } else {
            p.party = p.party.other();
            p.since_week = week;
            p.approval_bps = 5_500;
            let laws = programme(p.party, &p.laws, ds);
            p.pending = Some(LawChange {
                announced: week,
                effective: week + LAW_DELAY_WEEKS,
                laws,
            });
            p.news.push(PoliticalNews {
                person: None,
                week,
                text: format!(
                    "{year} election: {} wins. Its programme takes effect in {} weeks: corporate tax {}%, merger review at {}% share, {} union law{}; {}.",
                    party_name(p.party),
                    LAW_DELAY_WEEKS,
                    laws.corporate_tax_bps / 100,
                    laws.merger_review_bps / 100,
                    laws.unions.key(),
                    if laws.wage_floor_cents > p.laws.wage_floor_cents { ", a higher wage floor" } else { "" },
                    describe(&laws)
                ),
            });
        }
        p.next_election = week + TERM_WEEKS;
        held = Some(kept);
    }
    let excess = p.news.len().saturating_sub(NEWS_KEPT);
    p.news.drain(..excess);
    if let Some(kept) = held {
        // An election was held this week: the race is settled and the next one drawn. The
        // states' governors are chosen in their own races (`civics`).
        crate::economy::civics::national_result(world, ds, kept);
    }

    quotas(world, ds);
    refresh_inputs(world, ds);

    // The wage floor holds up low-skill background pay everywhere.
    let floor = world.politics.laws.wage_floor_cents;
    for region in &mut world.regions {
        if region.background_wage_cents[0] < floor {
            region.background_wage_cents[0] = floor;
        }
    }
}

/// Record the goods each firm's operating plants use, for vertical merger review.
fn refresh_inputs(world: &mut World, ds: &Dataset) {
    let mut used: std::collections::BTreeMap<cw_core::ids::Handle, Vec<u16>> =
        std::collections::BTreeMap::new();
    for (_, plant) in world.facilities.iter() {
        if plant.status != crate::state::FacilityStatus::Operating {
            continue;
        }
        let entry = used.entry(plant.firm).or_default();
        for id in ds.recipes[plant.recipe as usize].weekly_inputs_lots.keys() {
            if let Some(code) = ds.product_index.get(id) {
                entry.push(*code);
            }
        }
    }
    for (handle, mut list) in used {
        list.sort_unstable();
        list.dedup();
        if let Some(firm) = world.firms.get_mut(handle) {
            firm.inputs_used = list;
        }
    }
}

/// Production quotas (prorationing). In each quota state, while an extraction good's price is
/// soft, plants there may make only the quota share of capacity. Quotas ride on the regions'
/// supply modifiers, so the plant's own target is untouched and output returns the week the
/// price recovers or the law is repealed.
fn quotas(world: &mut World, ds: &Dataset) {
    for region in &mut world.regions {
        region.supply_modifiers.retain(|(tag, ..)| tag != "quota");
    }
    world.politics.quota_active.clear();
    let laws = world.politics.laws;
    if laws.quota_bps <= 0 || laws.quota_bps >= 10_000 || laws.quota_states == 0 {
        return;
    }
    let goods = crate::economy::reserve::strategic_goods(ds);
    let mut soft: Vec<u16> = Vec::new();
    for product in goods {
        if let Some((now, mean)) = crate::economy::reserve::band(world, ds, product)
            && now * 100 < mean * QUOTA_PRICE_PCT
        {
            soft.push(product);
        }
    }
    if soft.is_empty() {
        return;
    }
    let mut cells: Vec<(u8, u16)> = Vec::new();
    for (_, plant) in world.facilities.iter() {
        if plant.status != crate::state::FacilityStatus::Operating {
            continue;
        }
        let recipe = &ds.recipes[plant.recipe as usize];
        if recipe.industry != "extraction" {
            continue;
        }
        let Some(product) = ds.product_index.get(&recipe.output).copied() else {
            continue;
        };
        let in_quota_state = state_index(ds, plant.region)
            .is_some_and(|i| i < 64 && laws.quota_states & (1 << i) != 0);
        if in_quota_state && soft.contains(&product) {
            cells.push((plant.region, product));
        }
    }
    cells.sort_unstable();
    cells.dedup();
    let until = cw_core::calendar::Tick(world.tick.0 + 2);
    for (region, product) in &cells {
        world.regions[*region as usize].supply_modifiers.push((
            "quota".to_owned(),
            Some(*product),
            laws.quota_bps - 10_000,
            until,
        ));
    }
    world.politics.quota_active = cells;
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
                weekly(&mut world, &dataset);
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
