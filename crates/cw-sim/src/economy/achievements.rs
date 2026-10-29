//! Career achievements: a fixed list of milestones, checked once a week from facts the engine
//! already keeps, each earned once and dated. They are stored with the career's chapter state
//! (`World.chapters`, save trailer `CHAP`), never in the postcard payload.
//!
//! The list is code, not content: every entry names the fact it reads, so a test can build the
//! fact by hand. Titles and descriptions are the app's text.

use std::collections::BTreeSet;

use cw_data::Dataset;

use crate::economy::chapters::{Earned, Outcome};
use crate::economy::{ownership, score, sectors};
use crate::state::{DealState, HolderId, World};

pub struct Achievement {
    pub id: &'static str,
    pub title: &'static str,
    pub detail: &'static str,
}

pub const LIST: [Achievement; 20] = [
    Achievement {
        id: "year_one",
        title: "One Year in the Chair",
        detail: "Finish your first chapter.",
    },
    Achievement {
        id: "first_mandate",
        title: "The Board Is Pleased",
        detail: "Meet a board mandate.",
    },
    Achievement {
        id: "five_mandates",
        title: "Trusted Hands",
        detail: "Meet five board mandates.",
    },
    Achievement {
        id: "three_in_a_row",
        title: "A Clean Run",
        detail: "Meet three mandates in a row.",
    },
    Achievement {
        id: "first_raid",
        title: "First Blood",
        detail: "Win a raid on a rival's plants.",
    },
    Achievement {
        id: "first_listing",
        title: "Ringing the Bell",
        detail: "List your main company on the exchange.",
    },
    Achievement {
        id: "subsidiary_listed",
        title: "A Child Goes Public",
        detail: "List a subsidiary on the exchange.",
    },
    Achievement {
        id: "first_merger",
        title: "Two Become One",
        detail: "Complete a merger.",
    },
    Achievement {
        id: "ten_companies",
        title: "A Proper Group",
        detail: "Control ten companies at once.",
    },
    Achievement {
        id: "three_states",
        title: "Across State Lines",
        detail: "Run plants in three states.",
    },
    Achievement {
        id: "five_divisions",
        title: "Many Trades",
        detail: "Run five divisions at once.",
    },
    Achievement {
        id: "market_leader",
        title: "Top of the Market",
        detail: "Hold the largest capacity in a market.",
    },
    Achievement {
        id: "thousand_workers",
        title: "A Thousand on the Payroll",
        detail: "Employ a thousand workers across your group.",
    },
    Achievement {
        id: "net_worth_double",
        title: "Doubled Up",
        detail: "Double your net worth from the start.",
    },
    Achievement {
        id: "net_worth_tenfold",
        title: "Tenfold",
        detail: "Grow your net worth to ten times the start.",
    },
    Achievement {
        id: "debt_free",
        title: "Owe Nobody",
        detail: "Carry no debt in your main company after its first year.",
    },
    Achievement {
        id: "hostile_survived",
        title: "Not for Sale",
        detail: "Survive a hostile bid for one of your companies.",
    },
    Achievement {
        id: "bank_rescued",
        title: "Witness to a Rescue",
        detail: "Live through the state rescue of a bank.",
    },
    Achievement {
        id: "all_events",
        title: "Lived Through It All",
        detail: "See every event of the scenario's history play out.",
    },
    Achievement {
        id: "thirty_years",
        title: "A Thirty Year Career",
        detail: "Reach week 1,560 of your career.",
    },
];

fn earned(world: &World, id: &str) -> bool {
    world.chapters.achievements.iter().any(|e| e.id == id)
}

fn met_count(world: &World) -> usize {
    world
        .chapters
        .results()
        .filter(|o| *o == Outcome::Met)
        .count()
}

fn streak(world: &World) -> usize {
    world
        .chapters
        .results()
        .filter(|o| !matches!(o, Outcome::Lapsed | Outcome::Cancelled))
        .collect::<Vec<_>>()
        .iter()
        .rev()
        .take_while(|o| **o == Outcome::Met)
        .count()
}

/// Every id whose fact holds now, in list order. Pure in the world.
pub fn holds(world: &World, dataset: &Dataset) -> Vec<&'static str> {
    let player: Vec<_> = world
        .firms
        .iter()
        .filter(|(h, f)| {
            f.operative() && ownership::ultimate_controller(world, *h) == Some(HolderId::Investor)
        })
        .map(|(h, _)| h)
        .collect();
    let now = world.tick.0;
    let primary = world
        .investor
        .primary_group
        .and_then(|h| world.firms.get(h));
    let player_set: BTreeSet<_> = player.iter().copied().collect();
    let mut divisions = 0usize;
    let mut states = BTreeSet::new();
    let mut workers = 0i64;
    for firm in &player {
        for division in sectors::divisions_of(world, dataset, *firm) {
            divisions += 1;
            states.insert(dataset.regions[division.region as usize].state().to_owned());
        }
        if let Some(entity) = world.firms.get(*firm) {
            for plant in entity
                .facilities
                .iter()
                .filter_map(|h| world.facilities.get(*h))
            {
                workers += plant.workers.iter().sum::<i64>();
            }
        }
    }
    let start_worth = world.investor.starting_net_worth_cents;
    let worth = score::personal_net_worth(world);
    let timeline = &world.systemic.timeline;
    let mut out = Vec::new();
    for item in LIST.iter() {
        if earned(world, item.id) {
            continue;
        }
        let holds = match item.id {
            "year_one" => world.chapters.results().next().is_some(),
            "first_mandate" => met_count(world) >= 1,
            "five_mandates" => met_count(world) >= 5,
            "three_in_a_row" => streak(world) >= 3,
            "first_raid" => world
                .raids
                .iter()
                .any(|r| r.success && player_set.contains(&r.attacker)),
            "first_listing" => primary.is_some_and(|f| f.shares.ipo_done),
            "subsidiary_listed" => player.iter().any(|h| {
                world.firms.get(*h).is_some_and(|f| {
                    f.shares.ipo_done && matches!(f.shares.controller(), Some(HolderId::Firm(_)))
                })
            }),
            "first_merger" => world.deals.iter().any(|(_, d)| {
                d.kind == crate::state::DealKind::Merger
                    && d.state == DealState::Settled
                    && player_set.contains(&d.buyer)
            }),
            "ten_companies" => player.len() >= 10,
            "three_states" => states.len() >= 3,
            "five_divisions" => divisions >= 5,
            "market_leader" => now % 4 == 0 && score::footprint(world, dataset).2 >= 1,
            "thousand_workers" => workers >= 1_000,
            "net_worth_double" => start_worth > 0 && worth >= start_worth * 2,
            "net_worth_tenfold" => start_worth > 0 && worth >= start_worth * 10,
            "debt_free" => {
                now > 52
                    && primary.is_some_and(|f| {
                        !f.facilities.is_empty() && f.book.debt() == 0 && f.book.cash() > 0
                    })
            }
            "hostile_survived" => world.deals.iter().any(|(_, d)| {
                d.hostile
                    && player_set.contains(&d.target)
                    && matches!(
                        d.state,
                        DealState::Withdrawn | DealState::Expired | DealState::Rejected
                    )
            }),
            "bank_rescued" => world
                .firms
                .iter()
                .any(|(_, f)| f.bank.is_some() && f.rescued_week > 0),
            "all_events" => {
                !player.is_empty()
                    && !timeline.events.is_empty()
                    && timeline.events.iter().all(|e| e.fired_at > 0)
            }
            "thirty_years" => !player.is_empty() && now >= 30 * 52,
            _ => false,
        };
        if holds {
            out.push(item.id);
        }
    }
    out
}

/// Earn what now holds and has not been earned. A career with no player company earns nothing.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    if world.investor.primary_group.is_none() {
        return;
    }
    let now = world.tick.0;
    let fresh: Vec<&'static str> = holds(world, dataset)
        .into_iter()
        .filter(|id| !earned(world, id))
        .collect();
    for id in fresh {
        world.chapters.achievements.push(Earned {
            id: id.to_owned(),
            tick: now,
        });
    }
}
