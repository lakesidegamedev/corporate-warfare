//! Politics with faces: governor and leader races, campaign money, state laws and public tenders.
//!
//! Every state has a governor race and the country has a leader race, each between two people
//! (one for each party) with two or three stated positions. A weekly poll follows the economy
//! (unemployment, wages and prices in the state) and campaign money. Money is given with
//! `GiveToCandidate`, disclosed the next week, and raises the candidate's poll by a bounded
//! amount. The national race feeds the same chance the old election used, so with no gifts the
//! result is unchanged; a state's winner sets that state's laws thirteen weeks after the vote.
//!
//! State laws are right to work (union drift), a corporate tax adjustment, environmental rules
//! (build cost and lead time for heavy industry) and a cap on campaign gifts. They apply through
//! the union, tax and build paths that already exist.
//!
//! The state opens tenders for road, port and power works in regions. Firms bid a price and a
//! delivery time with `BidOnTender`; the best bid wins, builds in stages (paying for contractors
//! and materials as it goes) and is paid in stages from the treasury, never more than the
//! treasury holds and the week's procurement budget allows. Finished works change the region:
//! lane capacity, sea terminal access and the power grid score.
//!
//! The state lives outside the postcard payload (`World.civics`, save trailer `POLI`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::economy::cast::PersonId;
use crate::state::{Party, World};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::rng::Rng;
use cw_data::{Dataset, Role};

const TAG_CIVICS: u64 = 0x4349_5649_4353_0001;
/// The office code of the national race; states use their index.
pub const NATIONAL: u8 = 255;
/// Weeks before an election that gifts are allowed and the campaign is "open".
pub const CAMPAIGN_WEEKS: u32 = 26;
/// A governor serves four years, like the leader.
pub const STATE_TERM_WEEKS: u32 = 208;
pub const POLL_KEPT: usize = 26;
/// Most a firm may give to one race, unless the state's law says otherwise ($250,000).
pub const DEFAULT_CAP_CENTS: i64 = 25_000_000;
/// National races allow four times as much.
pub const NATIONAL_CAP_CENTS: i64 = 100_000_000;
pub const MIN_GIFT_CENTS: i64 = 100_000;
/// Gifts to one candidate add up to at most this many points of poll share, approached slowly.
pub const GIFT_MAX_BPS: i64 = 600;
/// Money at which a candidate has half of the maximum bonus ($200,000).
const GIFT_HALF_CENTS: i64 = 20_000_000;
const NEWS_KEPT: usize = 60;
const GIFTS_KEPT: usize = 200;
const TENDER_WEEKS_OPEN: u32 = 4;
const TENDER_KEPT: usize = 40;
/// Tenders open one at a time, this many weeks apart.
const TENDER_EVERY: u32 = 13;
/// Open and unfinished tenders together may commit at most this many weeks of procurement budget.
const COMMIT_WEEKS: i64 = 40;
const MAX_ACTIVE_PER_FIRM: usize = 3;
/// Works run over the estimate by between nothing and this many basis points.
const OVERRUN_MAX_BPS: i64 = 800;
/// Each bidder's own estimate of the cost is off by up to this much either way.
const ESTIMATE_SPREAD_BPS: i64 = 300;
const ROAD_STEP_BPS: i32 = 1_000;
const ROAD_MAX_BPS: i32 = 4_000;
const PORT_STEP_BPS: i32 = 1_500;
const PORT_MAX_BPS: i32 = 4_500;
const GRID_STEP_BPS: i32 = 500;
const GRID_MAX_BPS: i32 = 1_500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PosKey {
    RightToWork,
    CorporateTax,
    Environment,
    CommonCarrier,
    MergerReview,
}

impl PosKey {
    pub const fn key(self) -> &'static str {
        match self {
            PosKey::RightToWork => "right_to_work",
            PosKey::CorporateTax => "corporate_tax",
            PosKey::Environment => "environment",
            PosKey::CommonCarrier => "common_carrier",
            PosKey::MergerReview => "merger_review",
        }
    }
}

/// A stated position. Values: right to work 0/1; corporate tax in bps (a state adjustment, or
/// the national rate); environment 0 lax, 1 standard, 2 strict; common carrier 0/1; merger
/// review in bps of share.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub key: PosKey,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Cast id or generated id; the apps draw the portrait from it.
    pub id: String,
    pub name: String,
    pub party: Party,
    pub cast: Option<PersonId>,
    pub positions: Vec<Position>,
    /// Gifts received for this race.
    pub funds_cents: i64,
}

/// The laws a state sets for itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateLaws {
    pub right_to_work: bool,
    /// Added to the national corporate tax rate for firms headquartered in the state.
    pub tax_delta_bps: i32,
    /// 0 lax, 1 standard, 2 strict.
    pub environment: u8,
    /// Most a firm may give to one race in the state.
    pub cap_cents: i64,
}

impl Default for StateLaws {
    fn default() -> Self {
        StateLaws {
            right_to_work: false,
            tax_delta_bps: 0,
            environment: 1,
            cap_cents: DEFAULT_CAP_CENTS,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Race {
    /// State index, or [`NATIONAL`].
    pub office: u8,
    pub election: u32,
    /// Business candidate first, Labour second.
    pub candidates: Vec<Candidate>,
    /// The business candidate's poll share, 0 to 10,000.
    pub poll_bps: i32,
    pub history: Vec<i16>,
    pub opened: u32,
    /// What each firm has given to this race: (firm, candidate index) to cents.
    pub given: BTreeMap<(Handle, u8), i64>,
}

impl Race {
    pub fn bonus_bps(&self, candidate: usize) -> i32 {
        let f = self
            .candidates
            .get(candidate)
            .map(|c| c.funds_cents)
            .unwrap_or(0)
            .max(0);
        (GIFT_MAX_BPS * f / (f + GIFT_HALF_CENTS)) as i32
    }

    /// Whether gifts are allowed in `week`.
    pub fn open(&self, week: u32) -> bool {
        self.election > week && self.election - week <= CAMPAIGN_WEEKS
    }

    pub fn given_by(&self, firm: Handle) -> i64 {
        (0..2)
            .map(|c| self.given.get(&(firm, c)).copied().unwrap_or(0))
            .sum()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateCivic {
    pub governor: Candidate,
    pub laws: StateLaws,
    pub pending: Option<(u32, StateLaws)>,
    pub race: Race,
    pub since_week: u32,
    /// Reference wage and price level the mood is measured against; they drift toward the
    /// present over a year.
    pub wage_ref: i64,
    pub price_ref: i64,
    pub mood_bps: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gift {
    pub firm: Handle,
    pub week: u32,
    pub office: u8,
    pub candidate: u8,
    pub cents: i64,
    pub disclosed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenderKind {
    Road,
    Port,
    Power,
}

impl TenderKind {
    pub const fn key(self) -> &'static str {
        match self {
            TenderKind::Road => "road",
            TenderKind::Port => "port",
            TenderKind::Power => "power",
        }
    }
    pub const fn label(self) -> &'static str {
        match self {
            TenderKind::Road => "road upgrade",
            TenderKind::Port => "port expansion",
            TenderKind::Power => "power line",
        }
    }
    /// Weeks of the treasury's weekly procurement budget the works are worth.
    const fn budget_weeks(self) -> i64 {
        match self {
            TenderKind::Road => 6,
            TenderKind::Port => 10,
            TenderKind::Power => 8,
        }
    }
    /// Quickest and slowest delivery in weeks.
    const fn weeks(self) -> (u32, u32) {
        match self {
            TenderKind::Road => (8, 26),
            TenderKind::Port => (13, 39),
            TenderKind::Power => (10, 30),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TenderStatus {
    Open,
    Building,
    Done,
    Lapsed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bid {
    pub firm: Handle,
    pub price_cents: i64,
    pub weeks: u32,
    pub week: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tender {
    pub id: u32,
    pub state: u8,
    pub region: u8,
    pub kind: TenderKind,
    /// The most the state will pay.
    pub budget_cents: i64,
    pub opened: u32,
    pub closes: u32,
    pub min_weeks: u32,
    pub max_weeks: u32,
    pub bids: Vec<Bid>,
    pub status: TenderStatus,
    pub winner: Option<Handle>,
    pub price_cents: i64,
    pub promised_weeks: u32,
    pub started: u32,
    /// What the winner expects the works to cost it.
    pub cost_cents: i64,
    pub cost_paid_cents: i64,
    pub paid_cents: i64,
    pub finished: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CivicNews {
    pub week: u32,
    /// "poll", "debate", "gift", "result", "tender", "award", "letter".
    pub kind: String,
    pub text: String,
    pub firm: Option<Handle>,
    /// Cast or generated id of the person the item is about, for the portrait.
    pub person: String,
    /// The state, or [`NATIONAL`].
    pub office: u8,
}

/// Works completed in a region, in basis points of improvement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Infra {
    pub road_bps: i32,
    pub port_bps: i32,
    pub grid_bps: i32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CivicState {
    pub ready: bool,
    pub states: Vec<StateCivic>,
    pub national: Option<Race>,
    pub region_state: Vec<u8>,
    pub infra: Vec<Infra>,
    pub gifts: Vec<Gift>,
    pub tenders: Vec<Tender>,
    pub next_tender: u32,
    pub news: Vec<CivicNews>,
    /// Treasury paid to contractors so far, for the audit.
    pub tender_paid_cents: i64,
    pub tenders_done: u32,
}

impl CivicState {
    pub fn is_empty(&self) -> bool {
        *self == CivicState::default()
    }

    pub fn state_of(&self, region: u8) -> Option<usize> {
        match self.region_state.get(region as usize) {
            Some(i) if *i != u8::MAX => Some(*i as usize),
            _ => None,
        }
    }

    pub fn race(&self, office: u8) -> Option<&Race> {
        if office == NATIONAL {
            self.national.as_ref()
        } else {
            self.states.get(office as usize).map(|s| &s.race)
        }
    }

    fn race_mut(&mut self, office: u8) -> Option<&mut Race> {
        if office == NATIONAL {
            self.national.as_mut()
        } else {
            self.states.get_mut(office as usize).map(|s| &mut s.race)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Effects on the rest of the game

/// Corporate tax adjustment, in bps, for a firm headquartered in `region`.
pub fn tax_delta_bps(world: &World, region: u8) -> i32 {
    world
        .civics
        .state_of(region)
        .and_then(|i| world.civics.states.get(i))
        .map(|s| s.laws.tax_delta_bps)
        .unwrap_or(0)
}

/// Whether the region's state has a right-to-work law.
pub fn right_to_work(world: &World, region: u8) -> bool {
    world
        .civics
        .state_of(region)
        .and_then(|i| world.civics.states.get(i))
        .is_some_and(|s| s.laws.right_to_work)
}

/// (cost, lead time) in bps of normal for a plant of `industry` in `region`. Heavy industry
/// (extraction, energy, chemicals, manufacturing) pays for the state's environmental rules.
pub fn build_factors(world: &World, region: u8, industry: &str) -> (i64, i64) {
    let heavy = matches!(
        industry,
        "extraction" | "energy" | "chemical_industries" | "manufacturing"
    );
    if !heavy {
        return (10_000, 10_000);
    }
    let level = world
        .civics
        .state_of(region)
        .and_then(|i| world.civics.states.get(i))
        .map(|s| s.laws.environment)
        .unwrap_or(1);
    match level {
        0 => (9_500, 9_000),
        2 => (10_800, 11_500),
        _ => (10_000, 10_000),
    }
}

pub fn road_bps(world: &World, region: u8) -> i32 {
    world
        .civics
        .infra
        .get(region as usize)
        .map_or(0, |i| i.road_bps)
}

pub fn port_bps(world: &World, region: u8) -> i32 {
    world
        .civics
        .infra
        .get(region as usize)
        .map_or(0, |i| i.port_bps)
}

/// The region's power grid score: the data's score plus the works finished there, at most 100%.
pub fn grid_bps(world: &World, base: i32, region: u8) -> i32 {
    let extra = world
        .civics
        .infra
        .get(region as usize)
        .map_or(0, |i| i.grid_bps);
    (base + extra).min(10_000).max(base)
}

/// Extra poll points the national race gives the incumbent party over the other, from gifts.
pub fn national_swing_bps(world: &World, incumbent: Party) -> i32 {
    national_swing_bps_for(incumbent, &world.civics)
}

pub fn national_swing_bps_for(incumbent: Party, civics: &CivicState) -> i32 {
    let Some(race) = civics.national.as_ref() else {
        return 0;
    };
    let (mine, theirs) = match incumbent {
        Party::Business => (0, 1),
        Party::Labour => (1, 0),
    };
    race.bonus_bps(mine) - race.bonus_bps(theirs)
}

// ---------------------------------------------------------------------------------------------
// Setup

fn party_index(party: Party) -> usize {
    match party {
        Party::Business => 0,
        Party::Labour => 1,
    }
}

fn state_name(ds: &Dataset, state: u8) -> String {
    ds.states
        .get(state as usize)
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

fn positions_for(party: Party, rng: &mut Rng) -> Vec<Position> {
    let step = 100 * rng.uniform_inclusive(1, 3) as i32;
    let tax = match party {
        Party::Business => -step,
        Party::Labour => step,
    };
    let mut out = vec![Position {
        key: PosKey::CorporateTax,
        value: tax,
    }];
    let rtw = Position {
        key: PosKey::RightToWork,
        value: (party == Party::Business) as i32,
    };
    let env = Position {
        key: PosKey::Environment,
        value: match (party, rng.chance_bps(5_000)) {
            (Party::Business, true) => 0,
            (Party::Business, false) => 1,
            (Party::Labour, true) => 2,
            (Party::Labour, false) => 1,
        },
    };
    // Two or three stated positions: tax always, then one or both of the others.
    match rng.uniform_below(3) {
        0 => out.push(rtw),
        1 => out.push(env),
        _ => {
            out.push(rtw);
            out.push(env);
        }
    }
    out
}

fn used_ids(c: &CivicState) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for s in &c.states {
        ids.push(s.governor.id.clone());
        ids.extend(s.race.candidates.iter().map(|x| x.id.clone()));
    }
    if let Some(r) = &c.national {
        ids.extend(r.candidates.iter().map(|x| x.id.clone()));
    }
    ids
}

/// A person for the race: a free cast member who can hold the role, else a generated face.
fn pick_person(
    world: &World,
    ds: &Dataset,
    role: Role,
    avoid: &[String],
    rng: &mut Rng,
) -> (String, String, Option<PersonId>) {
    let pool: Vec<PersonId> = crate::economy::cast::free_people(world, ds, role)
        .into_iter()
        .filter(|p| !avoid.contains(&ds.cast[*p as usize].id))
        .collect();
    if !pool.is_empty() && rng.chance_bps(7_000) {
        let p = pool[rng.uniform_below(pool.len() as u64) as usize];
        let person = &ds.cast[p as usize];
        return (person.id.clone(), person.name.clone(), Some(p));
    }
    let extras = cw_data::cast::extras();
    if extras.is_empty() {
        return (
            format!("candidate_{}", rng.next_u64() % 1000),
            "Candidate".to_owned(),
            None,
        );
    }
    // Generated faces from the far end of the list, so they rarely meet a hired chief.
    for _ in 0..extras.len() {
        let e = &extras[extras.len() - 1 - rng.uniform_below(extras.len() as u64) as usize];
        if !avoid.contains(&e.id) {
            return (e.id.clone(), e.name.clone(), None);
        }
    }
    let e = &extras[0];
    (e.id.clone(), e.name.clone(), None)
}

fn governor_from_cast(
    world: &World,
    ds: &Dataset,
    state: usize,
) -> Option<(String, String, Option<PersonId>)> {
    let p = *world.cast.governors.get(&(state as u8))?;
    let person = ds.cast.get(p as usize)?;
    Some((person.id.clone(), person.name.clone(), Some(p)))
}

/// Laws a governor's positions set on top of the current ones.
fn apply_positions(base: StateLaws, positions: &[Position]) -> StateLaws {
    let mut laws = base;
    for p in positions {
        match p.key {
            PosKey::RightToWork => laws.right_to_work = p.value != 0,
            PosKey::CorporateTax => laws.tax_delta_bps = p.value,
            PosKey::Environment => laws.environment = p.value.clamp(0, 2) as u8,
            _ => {}
        }
    }
    laws
}

fn fresh_state_race(
    world: &World,
    ds: &Dataset,
    state: u8,
    incumbent: &Candidate,
    election: u32,
    rng: &mut Rng,
) -> Race {
    let avoid = used_ids(&world.civics);
    let mut avoid = avoid;
    avoid.push(incumbent.id.clone());
    let (id, name, cast) = pick_person(world, ds, Role::Governor, &avoid, rng);
    let other = incumbent.party.other();
    let challenger = Candidate {
        id,
        name,
        party: other,
        cast,
        positions: positions_for(other, rng),
        funds_cents: 0,
    };
    let mut running = incumbent.clone();
    running.positions = positions_for(running.party, rng);
    running.funds_cents = 0;
    let mut candidates = vec![running, challenger];
    candidates.sort_by_key(|c| party_index(c.party));
    let _ = state;
    Race {
        office: state,
        election,
        candidates,
        poll_bps: 5_000,
        history: Vec::new(),
        opened: world.tick.0,
        given: BTreeMap::new(),
    }
}

fn national_candidates(world: &World, ds: &Dataset, rng: &mut Rng) -> Vec<Candidate> {
    let programme_b =
        crate::economy::politics::programme(Party::Business, &world.politics.laws, ds);
    let programme_l = crate::economy::politics::programme(Party::Labour, &world.politics.laws, ds);
    let mut avoid = used_ids(&world.civics);
    let mut out = Vec::new();
    for (slot, party, laws) in [
        (0usize, Party::Business, programme_b),
        (1, Party::Labour, programme_l),
    ] {
        let named = world.cast.leaders[slot].and_then(|p| ds.cast.get(p as usize).map(|x| (x, p)));
        let (id, name, cast) = match named {
            Some((person, p)) => (person.id.clone(), person.name.clone(), Some(p)),
            None => pick_person(world, ds, Role::Leader, &avoid, rng),
        };
        avoid.push(id.clone());
        out.push(Candidate {
            id,
            name,
            party,
            cast,
            positions: vec![
                Position {
                    key: PosKey::CorporateTax,
                    value: laws.corporate_tax_bps,
                },
                Position {
                    key: PosKey::CommonCarrier,
                    value: laws.common_carrier as i32,
                },
                Position {
                    key: PosKey::MergerReview,
                    value: laws.merger_review_bps as i32,
                },
            ],
            funds_cents: 0,
        });
    }
    out
}

fn init(world: &mut World, ds: &Dataset) {
    let regions = ds.regions.len();
    let mut region_state = vec![u8::MAX; regions];
    for (r, slot) in region_state.iter_mut().enumerate() {
        if let Some(i) = crate::economy::politics::state_index(ds, r as u8) {
            *slot = i as u8;
        }
    }
    world.civics.region_state = region_state;
    world.civics.infra = vec![Infra::default(); regions];
    let mut states: Vec<StateCivic> = Vec::new();
    for i in 0..ds.states.len().min(63) {
        let mut rng = Rng::entity_stream(world.seed, TAG_CIVICS, Handle(i as u64 + 1));
        let party = if rng.chance_bps(5_000) {
            Party::Business
        } else {
            Party::Labour
        };
        let (id, name, cast) = governor_from_cast(world, ds, i).unwrap_or_else(|| {
            let avoid = used_ids(&world.civics);
            pick_person(world, ds, Role::Governor, &avoid, &mut rng)
        });
        let governor = Candidate {
            id,
            name,
            party,
            cast,
            positions: positions_for(party, &mut rng),
            funds_cents: 0,
        };
        let laws = apply_positions(StateLaws::default(), &governor.positions);
        let election = 45 + 52 * (i as u32 % 4);
        // Placeholder so `used_ids` sees this governor while the later states are drawn.
        world.civics.states.push(StateCivic {
            governor: governor.clone(),
            laws,
            pending: None,
            race: Race {
                office: i as u8,
                election,
                candidates: vec![governor.clone()],
                poll_bps: 5_000,
                history: Vec::new(),
                opened: 0,
                given: BTreeMap::new(),
            },
            since_week: 0,
            wage_ref: 0,
            price_ref: 0,
            mood_bps: 0,
        });
        states.push(world.civics.states.last().unwrap().clone());
    }
    for (i, s) in states.iter().enumerate() {
        let mut rng = Rng::entity_stream(world.seed, TAG_CIVICS, Handle(1000 + i as u64));
        let race = fresh_state_race(world, ds, i as u8, &s.governor, s.race.election, &mut rng);
        let (wage, price) = state_economy(world, ds, i);
        let target = &mut world.civics.states[i];
        target.race = race;
        target.wage_ref = wage;
        target.price_ref = price;
    }
    let mut rng = Rng::entity_stream(world.seed, TAG_CIVICS, Handle(5000));
    let candidates = national_candidates(world, ds, &mut rng);
    world.civics.national = Some(Race {
        office: NATIONAL,
        election: world.politics.next_election,
        candidates,
        poll_bps: 5_000,
        history: Vec::new(),
        opened: world.tick.0,
        given: BTreeMap::new(),
    });
    world.civics.ready = true;
}

// ---------------------------------------------------------------------------------------------
// Polls

/// (average low-skill prevailing wage, price level) across the state's regions.
fn state_economy(world: &World, ds: &Dataset, state: usize) -> (i64, i64) {
    let regions = regions_of(world, state);
    let _ = ds;
    let n = regions.len().max(1) as i64;
    let wage: i64 = regions
        .iter()
        .map(|r| world.regions[*r as usize].labour[0].prevailing_wage_cents)
        .sum::<i64>()
        / n;
    (wage.max(1), world.macro_state.price_index_bps.max(1))
}

fn regions_of(world: &World, state: usize) -> Vec<u8> {
    world
        .civics
        .region_state
        .iter()
        .enumerate()
        .filter(|(_, s)| **s as usize == state)
        .map(|(r, _)| r as u8)
        .collect()
}

fn unemployment_bps(world: &World, state: usize) -> i64 {
    let (mut pop, mut employed) = (0i64, 0i64);
    for r in regions_of(world, state) {
        for tier in &world.regions[r as usize].labour {
            pop += tier.population;
            employed += tier.employed;
        }
    }
    if pop <= 0 {
        return 600;
    }
    ((pop - employed).max(0) * 10_000 / pop).clamp(0, 10_000)
}

/// How the state feels about its government: better with low unemployment and real wages
/// rising, worse with prices rising. Basis points, about -1500 to 1500.
fn mood(world: &World, ds: &Dataset, state: usize) -> i32 {
    let s = &world.civics.states[state];
    let (wage, price) = state_economy(world, ds, state);
    let wage_dev = (wage - s.wage_ref) * 10_000 / s.wage_ref.max(1);
    let price_dev = (price - s.price_ref) * 10_000 / s.price_ref.max(1);
    let jobs = 600 - unemployment_bps(world, state);
    (jobs / 2 + wage_dev / 2 - price_dev / 2).clamp(-1_500, 1_500) as i32
}

fn poll_state(world: &mut World, ds: &Dataset, state: usize) {
    let m = mood(world, ds, state);
    let (wage, price) = state_economy(world, ds, state);
    let s = &mut world.civics.states[state];
    s.mood_bps = m;
    // The references follow the present over a year, so only recent change moves the mood.
    s.wage_ref += (wage - s.wage_ref) / 52;
    s.price_ref += (price - s.price_ref) / 52;
    let incumbent = s.governor.party;
    let inc_idx = party_index(incumbent);
    let swing = s.race.bonus_bps(inc_idx) - s.race.bonus_bps(1 - inc_idx);
    // The sitting governor starts level with the challenger and is judged on the economy.
    let target_inc = (5_000 + m + swing).clamp(1_500, 8_500);
    let target_business = if incumbent == Party::Business {
        target_inc
    } else {
        10_000 - target_inc
    };
    s.race.poll_bps += (target_business - s.race.poll_bps) / 8;
    record_poll(&mut s.race);
}

fn record_poll(race: &mut Race) {
    race.history.push((race.poll_bps / 10) as i16);
    let excess = race.history.len().saturating_sub(POLL_KEPT);
    race.history.drain(..excess);
}

fn poll_national(world: &mut World) {
    let incumbent = world.politics.party;
    let approval = world.politics.approval_bps;
    let swing = national_swing_bps(world, incumbent);
    let hold = (approval + 300 + swing).clamp(500, 9_500);
    let business = if incumbent == Party::Business {
        hold
    } else {
        10_000 - hold
    };
    if let Some(race) = world.civics.national.as_mut() {
        race.poll_bps = business;
        race.election = world.politics.next_election;
        record_poll(race);
    }
}

// ---------------------------------------------------------------------------------------------
// Gifts

/// Why a gift cannot be made, or the amount that will be taken.
pub fn check_gift(
    world: &World,
    firm: Handle,
    office: u8,
    candidate: u8,
    cents: i64,
) -> Result<i64, &'static str> {
    let race = world.civics.race(office).ok_or("no_race")?;
    if candidate > 1 || race.candidates.get(candidate as usize).is_none() {
        return Err("no_candidate");
    }
    if !race.open(world.tick.0) {
        return Err("campaign_closed");
    }
    if cents < MIN_GIFT_CENTS {
        return Err("gift_too_small");
    }
    let cap = cap_for(world, office);
    if race.given_by(firm) + cents > cap {
        return Err("over_cap");
    }
    let entity = world.firms.get(firm).ok_or("no_firm")?;
    if !entity.operative() {
        return Err("firm_not_operating");
    }
    if entity.book.cash() - entity.reserved_cash_cents < cents {
        return Err("insufficient_cash");
    }
    Ok(cents)
}

pub fn cap_for(world: &World, office: u8) -> i64 {
    if office == NATIONAL {
        NATIONAL_CAP_CENTS
    } else {
        world
            .civics
            .states
            .get(office as usize)
            .map_or(DEFAULT_CAP_CENTS, |s| s.laws.cap_cents)
    }
}

/// `GiveToCandidate`: cash leaves the firm for the campaign (the outside advocacy sector, like a
/// lobbying fee), the candidate's funds rise and the gift is disclosed next week.
pub fn give(
    world: &mut World,
    firm: Handle,
    office: u8,
    candidate: u8,
    cents: i64,
) -> Result<(), &'static str> {
    check_gift(world, firm, office, candidate, cents)?;
    let tick = world.tick;
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            tick,
            Txn::new()
                .credit(Account::Cash, cents, "campaign contribution")
                .debit(Account::OperatingExpense, cents, "campaign contribution"),
        );
    }
    let _ = world.outside.advocacy_book.commit(
        tick,
        Txn::new()
            .debit(Account::Cash, cents, "campaign contribution")
            .credit(Account::Revenue, cents, "campaign contribution"),
    );
    if let Some(race) = world.civics.race_mut(office) {
        *race.given.entry((firm, candidate)).or_default() += cents;
        race.candidates[candidate as usize].funds_cents += cents;
    }
    world.civics.gifts.push(Gift {
        firm,
        week: tick.0,
        office,
        candidate,
        cents,
        disclosed: false,
    });
    let excess = world.civics.gifts.len().saturating_sub(GIFTS_KEPT);
    world.civics.gifts.drain(..excess);
    Ok(())
}

fn nudge_reputation(world: &mut World, firm: Handle, delta: i32) {
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.reputation_bps = (entity.reputation_bps + delta).clamp(-2_000, 4_000);
    }
}

/// Reputation weight of a gift: a hundredth of a basis point per cent over a threshold, at most
/// 100 bps for the largest gift the law allows.
fn weight(cents: i64) -> i32 {
    (cents / 200_000).clamp(0, 100) as i32
}

fn money_text(cents: i64) -> String {
    let dollars = cents / 100;
    if dollars >= 1_000_000 {
        format!(
            "${}.{} million",
            dollars / 1_000_000,
            dollars % 1_000_000 / 100_000
        )
    } else {
        format!("${}", with_commas(dollars))
    }
}

fn with_commas(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn push_news(
    world: &mut World,
    kind: &str,
    text: String,
    firm: Option<Handle>,
    person: &str,
    office: u8,
) {
    let week = world.tick.0;
    world.civics.news.push(CivicNews {
        week,
        kind: kind.to_owned(),
        text,
        firm,
        person: person.to_owned(),
        office,
    });
    let excess = world.civics.news.len().saturating_sub(NEWS_KEPT);
    world.civics.news.drain(..excess);
}

/// Disclose last week's gifts: the paper reports them and the other side holds a grudge.
fn disclose(world: &mut World, ds: &Dataset) {
    let week = world.tick.0;
    let due: Vec<usize> = world
        .civics
        .gifts
        .iter()
        .enumerate()
        .filter(|(_, g)| !g.disclosed && g.week < week)
        .map(|(i, _)| i)
        .collect();
    for i in due {
        let g = world.civics.gifts[i].clone();
        world.civics.gifts[i].disclosed = true;
        let Some(race) = world.civics.race(g.office) else {
            continue;
        };
        let Some(cand) = race.candidates.get(g.candidate as usize) else {
            continue;
        };
        let (name, id) = (cand.name.clone(), cand.id.clone());
        let giver = world
            .firm(g.firm)
            .map(|f| f.name.clone())
            .unwrap_or_default();
        let office = if g.office == NATIONAL {
            "the national leadership".to_owned()
        } else {
            format!("governor of {}", state_name(ds, g.office))
        };
        push_news(
            world,
            "gift",
            format!(
                "{giver} gave {} to {name}, a candidate for {office}.",
                money_text(g.cents)
            ),
            Some(g.firm),
            &id,
            g.office,
        );
        // The other side holds it against the giver, a little.
        nudge_reputation(world, g.firm, -(weight(g.cents) / 4));
    }
}

/// After a vote: the winner's donors earn favour, the loser's a grudge. Reputation only.
fn settle_gifts(world: &mut World, race: &Race, winner: usize) {
    for ((firm, cand), cents) in &race.given {
        let w = weight(*cents);
        if *cand as usize == winner {
            nudge_reputation(world, *firm, w / 2);
        } else {
            nudge_reputation(world, *firm, -(w / 4));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Elections

fn resolve_state(world: &mut World, ds: &Dataset, i: usize) {
    let week = world.tick.0;
    let race = world.civics.states[i].race.clone();
    let mut rng = Rng::entity_stream(
        world.seed,
        TAG_CIVICS,
        Handle(((week as u64) << 8) | i as u64 | 1 << 40),
    );
    let poll = race.poll_bps.clamp(500, 9_500);
    let winner = if rng.chance_bps(poll) { 0 } else { 1 };
    let won = race.candidates[winner].clone();
    let lost = race.candidates[1 - winner].clone();
    let name = state_name(ds, i as u8);
    let changed = won.id != world.civics.states[i].governor.id;
    push_news(
        world,
        "result",
        format!(
            "{name} election: {} ({}) {} with {}% in the final poll against {} ({}). {}",
            won.name,
            crate::economy::politics::party_name(won.party),
            if changed {
                "wins the governorship"
            } else {
                "is re-elected"
            },
            if winner == 0 {
                poll / 100
            } else {
                (10_000 - poll) / 100
            },
            lost.name,
            crate::economy::politics::party_name(lost.party),
            describe_positions(&won.positions),
        ),
        None,
        &won.id,
        i as u8,
    );
    settle_gifts(world, &race, winner);
    let laws = apply_positions(world.civics.states[i].laws, &won.positions);
    {
        let s = &mut world.civics.states[i];
        s.governor = won.clone();
        s.since_week = week;
        s.pending = Some((week + crate::economy::politics::LAW_DELAY_WEEKS, laws));
    }
    // The cast list follows: a named winner holds the post, a generated one frees it.
    match won.cast {
        Some(p) => {
            world.cast.governors.insert(i as u8, p);
        }
        None => {
            world.cast.governors.remove(&(i as u8));
        }
    }
    let mut rng = Rng::entity_stream(
        world.seed,
        TAG_CIVICS,
        Handle(((week as u64) << 8) | i as u64 | 2 << 40),
    );
    let next = fresh_state_race(world, ds, i as u8, &won, week + STATE_TERM_WEEKS, &mut rng);
    world.civics.states[i].race = next;
}

fn describe_positions(positions: &[Position]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for p in positions {
        parts.push(match p.key {
            PosKey::RightToWork => {
                if p.value != 0 {
                    "right to work".to_owned()
                } else {
                    "keeping union rules".to_owned()
                }
            }
            PosKey::CorporateTax => {
                if p.value < 0 {
                    format!("corporate tax down {} points", -p.value / 100)
                } else {
                    format!("corporate tax up {} points", p.value / 100)
                }
            }
            PosKey::Environment => match p.value {
                0 => "lighter environmental rules".to_owned(),
                2 => "stricter environmental rules".to_owned(),
                _ => "current environmental rules".to_owned(),
            },
            PosKey::CommonCarrier => {
                if p.value != 0 {
                    "common carrier rules".to_owned()
                } else {
                    "no common carrier rules".to_owned()
                }
            }
            PosKey::MergerReview => format!("merger review at {}% share", p.value / 100),
        });
    }
    format!("Promises: {}.", parts.join(", "))
}

/// Called by `politics::weekly` once the national vote is decided (`kept` is whether the
/// governing party held on). Settles gifts, writes the news and draws the next race.
pub fn national_result(world: &mut World, ds: &Dataset, kept: bool) {
    let Some(race) = world.civics.national.clone() else {
        return;
    };
    // `party` is already the winner when the government changed.
    let winner_party = world.politics.party;
    let winner = party_index(winner_party);
    settle_gifts(world, &race, winner);
    if let Some(w) = race.candidates.get(winner) {
        let l = &race.candidates[1 - winner];
        push_news(
            world,
            "result",
            format!(
                "Leader election: {} ({}) {} against {} ({}). {}",
                w.name,
                crate::economy::politics::party_name(w.party),
                if kept {
                    "is returned to office"
                } else {
                    "wins the leadership"
                },
                l.name,
                crate::economy::politics::party_name(l.party),
                describe_positions(&w.positions),
            ),
            None,
            &w.id,
            NATIONAL,
        );
    }
    // The next race is drawn once the new programme is known.
    let mut rng = Rng::entity_stream(world.seed, TAG_CIVICS, Handle(6000 + world.tick.0 as u64));
    let candidates = national_candidates(world, ds, &mut rng);
    world.civics.national = Some(Race {
        office: NATIONAL,
        election: world.politics.next_election,
        candidates,
        poll_bps: 5_000,
        history: Vec::new(),
        opened: world.tick.0,
        given: BTreeMap::new(),
    });
}

// ---------------------------------------------------------------------------------------------
// Tenders

/// Weeks between new tenders: the usual quarter when the treasury is near its reserve, faster
/// when it is flush, and none (a very long wait) when it is under half of it.
pub fn tender_interval(stance_bps: i64) -> u32 {
    match stance_bps {
        ..=4_999 => u32::MAX,
        15_000.. => TENDER_EVERY / 2,
        _ => TENDER_EVERY,
    }
}

fn open_tender(world: &mut World, ds: &Dataset) {
    let week = world.tick.0;
    let budget = world.government.procurement_budget_cents;
    if budget <= 0 || ds.states.is_empty() {
        return;
    }
    let committed: i64 = world
        .civics
        .tenders
        .iter()
        .filter(|t| matches!(t.status, TenderStatus::Open | TenderStatus::Building))
        .map(|t| t.budget_cents - t.paid_cents)
        .sum();
    if committed >= budget * COMMIT_WEEKS {
        return;
    }
    let n = world.civics.next_tender;
    let kind = match n % 3 {
        0 => TenderKind::Road,
        1 => TenderKind::Power,
        _ => TenderKind::Port,
    };
    // The region with the least finished works of this kind, in a state taken in turn.
    let state = (n as usize * 7 + week as usize / TENDER_EVERY as usize) % ds.states.len();
    let mut candidates: Vec<u8> = regions_of(world, state);
    if kind == TenderKind::Port {
        let sea = ds.logistics.mode_index("sea");
        candidates.retain(|r| {
            sea.is_some_and(|m| crate::economy::logistics::has_access(ds, &world.logistics, m, *r))
                || ds.public_ports.get(*r as usize).copied().unwrap_or(false)
        });
    }
    // A place that drew no bid last time goes to the back of the line.
    let lapsed_before = |r: u8| {
        world
            .civics
            .tenders
            .iter()
            .filter(|t| t.region == r && t.kind == kind && t.status == TenderStatus::Lapsed)
            .count()
    };
    candidates.sort_by_key(|r| {
        let i = world
            .civics
            .infra
            .get(*r as usize)
            .copied()
            .unwrap_or_default();
        (
            lapsed_before(*r),
            match kind {
                TenderKind::Road => i.road_bps,
                TenderKind::Port => i.port_bps,
                TenderKind::Power => i.grid_bps,
            },
            *r,
        )
    });
    let Some(&region) = candidates.first() else {
        world.civics.next_tender += 1;
        return;
    };
    let (min_weeks, max_weeks) = kind.weeks();
    // The most the state will pay follows what building costs where the works are.
    let land = world.regions[region as usize]
        .land_index_bps
        .clamp(8_000, 13_000) as i64;
    let price = (budget * kind.budget_weeks() * land / 10_000).max(10_000_000);
    let id = world.civics.next_tender;
    world.civics.next_tender += 1;
    world.civics.tenders.push(Tender {
        id,
        state: state as u8,
        region,
        kind,
        budget_cents: price,
        opened: week,
        closes: week + TENDER_WEEKS_OPEN,
        min_weeks,
        max_weeks,
        bids: Vec::new(),
        status: TenderStatus::Open,
        winner: None,
        price_cents: 0,
        promised_weeks: 0,
        started: 0,
        cost_cents: 0,
        cost_paid_cents: 0,
        paid_cents: 0,
        finished: 0,
    });
    let place = world.regions[region as usize].name.clone();
    push_news(
        world,
        "tender",
        format!(
            "{} opens tenders for a {} in {place}, up to {}. Bids close in {} weeks.",
            state_name(ds, state as u8),
            kind.label(),
            money_text(price),
            TENDER_WEEKS_OPEN
        ),
        None,
        "",
        state as u8,
    );
}

/// The cost to deliver the works in `weeks`, for the bid form and the rivals: the base share of
/// the budget, scaled by local land prices, plus a premium for hurrying.
pub fn delivery_cost(world: &World, t: &Tender, weeks: u32) -> i64 {
    let land = world.regions[t.region as usize]
        .land_index_bps
        .clamp(8_000, 13_000) as i64;
    let base = t.budget_cents * 70 / 100 * land / 10_000;
    let span = (t.max_weeks - t.min_weeks).max(1) as i64;
    let hurry = (t
        .max_weeks
        .saturating_sub(weeks.clamp(t.min_weeks, t.max_weeks))) as i64;
    base * (10_000 + hurry * 1_200 / span) / 10_000
}

/// How far the works run over the estimate, in basis points: one figure per tender, drawn from
/// the world seed so every client agrees.
pub fn overrun_bps(world: &World, tender: u32) -> i64 {
    let mut rng = Rng::entity_stream(world.seed, TAG_CIVICS ^ 0x0E4A, Handle(tender as u64 + 1));
    rng.uniform_inclusive(0, OVERRUN_MAX_BPS)
}

/// One bidder's error in reading the cost, in basis points either way.
fn estimate_error_bps(world: &World, tender: u32, firm: Handle) -> i64 {
    let mut rng = Rng::entity_stream(
        world.seed,
        TAG_CIVICS ^ 0x0E57 ^ ((tender as u64) << 20),
        firm,
    );
    rng.uniform_inclusive(-ESTIMATE_SPREAD_BPS, ESTIMATE_SPREAD_BPS)
}

/// Cash a contractor needs on hand to start: a month of the work, since the state pays as the
/// work goes and the contractor does not fund the whole job.
fn working_capital(cost: i64, weeks: u32) -> i64 {
    cost * 4 / weeks.max(4) as i64
}

fn active_for(world: &World, firm: Handle) -> usize {
    world
        .civics
        .tenders
        .iter()
        .filter(|t| t.status == TenderStatus::Building && t.winner == Some(firm))
        .count()
}

pub fn check_bid(
    world: &World,
    _ds: &Dataset,
    firm: Handle,
    tender: u32,
    price: i64,
    weeks: u32,
) -> Result<(), &'static str> {
    let t = world
        .civics
        .tenders
        .iter()
        .find(|t| t.id == tender)
        .ok_or("no_tender")?;
    if t.status != TenderStatus::Open || world.tick.0 > t.closes {
        return Err("tender_closed");
    }
    if weeks < t.min_weeks || weeks > t.max_weeks {
        return Err("weeks_out_of_range");
    }
    if price <= 0 || price > t.budget_cents {
        return Err("price_out_of_range");
    }
    let entity = world.firms.get(firm).ok_or("no_firm")?;
    if !entity.operative() {
        return Err("firm_not_operating");
    }
    if active_for(world, firm) >= MAX_ACTIVE_PER_FIRM {
        return Err("too_many_works");
    }
    Ok(())
}

pub fn bid(
    world: &mut World,
    ds: &Dataset,
    firm: Handle,
    tender: u32,
    price: i64,
    weeks: u32,
) -> Result<(), &'static str> {
    check_bid(world, ds, firm, tender, price, weeks)?;
    let week = world.tick.0;
    if let Some(t) = world.civics.tenders.iter_mut().find(|t| t.id == tender) {
        t.bids.retain(|b| b.firm != firm);
        t.bids.push(Bid {
            firm,
            price_cents: price,
            weeks,
            week,
        });
    }
    Ok(())
}

fn bid_score(t: &Tender, b: &Bid) -> i64 {
    // A week of delivery is worth a hundredth of the budget to the state.
    b.price_cents + b.weeks as i64 * t.budget_cents / 100
}

fn award(world: &mut World, ds: &Dataset) {
    let week = world.tick.0;
    let ids: Vec<u32> = world
        .civics
        .tenders
        .iter()
        .filter(|t| t.status == TenderStatus::Open && week >= t.closes)
        .map(|t| t.id)
        .collect();
    for id in ids {
        let Some(pos) = world.civics.tenders.iter().position(|t| t.id == id) else {
            continue;
        };
        let t = world.civics.tenders[pos].clone();
        let mut bids = t.bids.clone();
        bids.sort_by_key(|b| (bid_score(&t, b), b.firm));
        let mut chosen: Option<Bid> = None;
        for b in bids {
            let cost = delivery_cost(world, &t, b.weeks);
            let cash = world
                .firm(b.firm)
                .map(|f| f.book.cash() - f.reserved_cash_cents)
                .map(|c| crate::economy::fx::firm_to_numeraire(world, ds, b.firm, c))
                .unwrap_or(0);
            if world.firm(b.firm).is_some_and(|f| f.operative())
                && cash >= working_capital(cost, b.weeks)
                && active_for(world, b.firm) < MAX_ACTIVE_PER_FIRM
            {
                chosen = Some(b);
                break;
            }
        }
        let place = world.regions[t.region as usize].name.clone();
        match chosen {
            Some(b) => {
                // Works run over the estimate everybody bid on, by a figure nobody sees until the
                // bill comes in, so the lowest bidder is the one most likely to have been too
                // hopeful (the winner's curse).
                let cost = delivery_cost(world, &t, b.weeks) * (10_000 + overrun_bps(world, t.id))
                    / 10_000;
                let firm_name = world
                    .firm(b.firm)
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                {
                    let tt = &mut world.civics.tenders[pos];
                    tt.status = TenderStatus::Building;
                    tt.winner = Some(b.firm);
                    tt.price_cents = b.price_cents;
                    tt.promised_weeks = b.weeks;
                    tt.started = week;
                    tt.cost_cents = cost;
                }
                push_news(
                    world,
                    "award",
                    format!(
                        "{firm_name} wins the {} in {place} for {} in {} weeks, against {} other bids.",
                        t.kind.label(),
                        money_text(b.price_cents),
                        b.weeks,
                        t.bids.len().saturating_sub(1)
                    ),
                    Some(b.firm),
                    "",
                    t.state,
                );
            }
            None => {
                world.civics.tenders[pos].status = TenderStatus::Lapsed;
                push_news(
                    world,
                    "tender",
                    format!(
                        "The {} in {place} drew no usable bid and lapses.",
                        t.kind.label()
                    ),
                    None,
                    "",
                    t.state,
                );
            }
        }
    }
}

fn build_works(world: &mut World, ds: &Dataset) {
    let week = world.tick.0;
    let ids: Vec<u32> = world
        .civics
        .tenders
        .iter()
        .filter(|t| t.status == TenderStatus::Building)
        .map(|t| t.id)
        .collect();
    let steel = ds.product_code("steel");
    for id in ids {
        let Some(pos) = world.civics.tenders.iter().position(|t| t.id == id) else {
            continue;
        };
        let t = world.civics.tenders[pos].clone();
        let Some(firm) = t.winner else { continue };
        if !world.firm(firm).is_some_and(|f| f.operative()) {
            world.civics.tenders[pos].status = TenderStatus::Lapsed;
            push_news(
                world,
                "tender",
                format!(
                    "The {} in {} is abandoned when its contractor fails.",
                    t.kind.label(),
                    world.regions[t.region as usize].name
                ),
                None,
                "",
                t.state,
            );
            continue;
        }
        // The contractor pays for the week's work: construction sold by contractors in the
        // region, with the rest bought from the outside construction sector.
        let slice =
            (t.cost_cents / t.promised_weeks.max(1) as i64).min(t.cost_cents - t.cost_paid_cents);
        // Prices, costs and the treasury are in the numeraire; a contractor in another money
        // pays its bills and is paid in its own, converted at the week's rate.
        let cash = world
            .firm(firm)
            .map(|f| f.book.cash() - f.reserved_cash_cents)
            .map(|c| crate::economy::fx::firm_to_numeraire(world, ds, firm, c))
            .unwrap_or(0);
        let mut worked = 0;
        if slice > 0 && cash >= slice {
            let slice_own = crate::economy::fx::numeraire_to_firm(world, ds, firm, slice);
            if let Some(f) = world.firms.get_mut(firm) {
                let _ = f.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, slice_own, "public works cost")
                        .debit(Account::CostOfGoods, slice_own, "public works cost"),
                );
            }
            crate::commands::pay_construction_service(
                world,
                ds,
                firm,
                t.region,
                slice_own,
                "public works",
            );
            worked = slice;
            // Materials: steel demand in the region while the works run.
            if let Some(p) = steel {
                world.regions[t.region as usize].demand_modifiers.push((
                    "tender".to_owned(),
                    Some(p),
                    300,
                    cw_core::calendar::Tick(week + 2),
                ));
            }
        }
        let paid_cost = t.cost_paid_cents + worked;
        // The treasury pays as the work progresses, as soon as it can, so a contractor's books show
        // a steady margin and not a lump a quarter.
        let progress_bps = if t.cost_cents > 0 {
            paid_cost * 10_000 / t.cost_cents
        } else {
            10_000
        };
        let owed_bps = progress_bps.min(10_000);
        let late = week.saturating_sub(t.started + t.promised_weeks);
        let mut owed = t.price_cents * owed_bps / 10_000;
        if owed_bps >= 10_000 && late > 0 {
            // A late finish forfeits a tenth of the price per week, at most half.
            owed -= t.price_cents * (late.min(5) as i64 * 1_000) / 10_000;
        }
        let due = (owed - t.paid_cents).max(0);
        let cap = (world.government.procurement_budget_cents / 2).max(0);
        let pay = due.min(cap).min(world.government.book.cash().max(0));
        if pay > 0 {
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, pay, "public works stage")
                    .debit(Account::OperatingExpense, pay, "public works stage"),
            );
            let pay_own = crate::economy::fx::numeraire_to_firm(world, ds, firm, pay);
            if let Some(f) = world.firms.get_mut(firm) {
                let _ = f.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, pay_own, "public works stage")
                        .credit(Account::Revenue, pay_own, "public works stage"),
                );
            }
            world.civics.tender_paid_cents += pay;
        }
        let tt = &mut world.civics.tenders[pos];
        tt.cost_paid_cents = paid_cost;
        tt.paid_cents += pay;
        let finished =
            paid_cost >= tt.cost_cents && week >= tt.started + tt.promised_weeks.min(tt.min_weeks);
        let settled = tt.paid_cents >= owed && owed_bps >= 10_000;
        if finished && settled {
            tt.status = TenderStatus::Done;
            tt.finished = week;
            let (kind, region, state) = (tt.kind, tt.region, tt.state);
            let firm_name = world.firm(firm).map(|f| f.name.clone()).unwrap_or_default();
            if let Some(i) = world.civics.infra.get_mut(region as usize) {
                match kind {
                    TenderKind::Road => i.road_bps = (i.road_bps + ROAD_STEP_BPS).min(ROAD_MAX_BPS),
                    TenderKind::Port => i.port_bps = (i.port_bps + PORT_STEP_BPS).min(PORT_MAX_BPS),
                    TenderKind::Power => {
                        i.grid_bps = (i.grid_bps + GRID_STEP_BPS).min(GRID_MAX_BPS)
                    }
                }
            }
            world.civics.tenders_done += 1;
            push_news(
                world,
                "award",
                format!(
                    "The {} in {} is finished by {firm_name}.",
                    kind.label(),
                    world.regions[region as usize].name
                ),
                Some(firm),
                "",
                state,
            );
        }
    }
    // Tell the freight model about new capacity next week through `begin_week`; nothing to do.
    let keep = world.civics.tenders.len().saturating_sub(TENDER_KEPT);
    if keep > 0 {
        let mut dropped = 0;
        world.civics.tenders.retain(|t| {
            if dropped < keep && matches!(t.status, TenderStatus::Done | TenderStatus::Lapsed) {
                dropped += 1;
                false
            } else {
                true
            }
        });
    }
}

// ---------------------------------------------------------------------------------------------
// The week

pub fn weekly(world: &mut World, ds: &Dataset) {
    if ds.states.is_empty() || ds.regions.is_empty() {
        return;
    }
    if !world.civics.ready {
        // Wait for the cast to be drawn so the named governors are taken.
        if world.tick.0 < 2 {
            return;
        }
        init(world, ds);
    }
    let week = world.tick.0;
    poll_national(world);
    for i in 0..world.civics.states.len() {
        // Announced state laws come into force.
        if let Some((effective, laws)) = world.civics.states[i].pending
            && week >= effective
        {
            world.civics.states[i].laws = laws;
            world.civics.states[i].pending = None;
            let name = state_name(ds, i as u8);
            let g = world.civics.states[i].governor.clone();
            push_news(
                world,
                "result",
                format!(
                    "New laws in {name}: {} corporate tax {:+} points, gifts capped at {}, {} environmental rules.",
                    if laws.right_to_work {
                        "right to work,"
                    } else {
                        "union rules kept,"
                    },
                    laws.tax_delta_bps / 100,
                    money_text(laws.cap_cents),
                    ["lighter", "current", "stricter"][laws.environment.min(2) as usize],
                ),
                None,
                &g.id,
                i as u8,
            );
        }
        poll_state(world, ds, i);
        if week >= world.civics.states[i].race.election {
            resolve_state(world, ds, i);
        }
    }
    // A debate a month before each vote.
    for i in 0..world.civics.states.len() {
        let r = &world.civics.states[i].race;
        if r.election > week && r.election - week == 4 {
            let name = state_name(ds, i as u8);
            let (a, b) = (r.candidates[0].clone(), r.candidates[1].clone());
            let lead = if r.poll_bps >= 5_000 { &a } else { &b };
            push_news(
                world,
                "debate",
                format!(
                    "{name} governor's debate: {} and {} trade blows over {}. {} leads the poll.",
                    a.name,
                    b.name,
                    debate_topic(&a, &b),
                    lead.name
                ),
                None,
                &lead.id,
                i as u8,
            );
        }
    }
    if let Some(r) = world.civics.national.as_ref()
        && r.election > week
        && r.election - week == 4
    {
        let (a, b) = (r.candidates[0].clone(), r.candidates[1].clone());
        let lead = if r.poll_bps >= 5_000 {
            a.clone()
        } else {
            b.clone()
        };
        push_news(
            world,
            "debate",
            format!(
                "Leaders' debate: {} and {} face off. {} leads the poll.",
                a.name, b.name, lead.name
            ),
            None,
            &lead.id,
            NATIONAL,
        );
    }
    disclose(world, ds);
    governor_letters(world, ds);
    if week > 8 && week.is_multiple_of(tender_interval(crate::economy::fiscal::stance_bps(world))) {
        open_tender(world, ds);
    }
    award(world, ds);
    build_works(world, ds);
}

fn debate_topic(a: &Candidate, b: &Candidate) -> &'static str {
    for p in &a.positions {
        if b.positions
            .iter()
            .any(|q| q.key == p.key && q.value != p.value)
        {
            return match p.key {
                PosKey::RightToWork => "right to work",
                PosKey::CorporateTax => "corporate tax",
                PosKey::Environment => "environmental rules",
                PosKey::CommonCarrier => "common carrier rules",
                PosKey::MergerReview => "merger review",
            };
        }
    }
    "the economy"
}

/// Governors write to the firm with the most plant in their state, a few times a year.
fn governor_letters(world: &mut World, ds: &Dataset) {
    let week = world.tick.0;
    for i in 0..world.civics.states.len() {
        if !(week + i as u32 * 5).is_multiple_of(26) {
            continue;
        }
        let mut best: Option<(i64, Handle)> = None;
        for (_, plant) in world.facilities.iter() {
            if plant.status != crate::state::FacilityStatus::Operating {
                continue;
            }
            if world.civics.state_of(plant.region) != Some(i) {
                continue;
            }
            let blocks: i64 = world
                .facilities
                .iter()
                .filter(|(_, p)| p.firm == plant.firm && world.civics.state_of(p.region) == Some(i))
                .map(|(_, p)| p.blocks as i64)
                .sum();
            if blocks >= 4
                && best.is_none_or(|(b, h)| blocks > b || (blocks == b && plant.firm < h))
            {
                best = Some((blocks, plant.firm));
            }
        }
        let Some((blocks, firm)) = best else { continue };
        let s = &world.civics.states[i];
        let name = state_name(ds, i as u8);
        let law = if s.laws.right_to_work {
            "right to work"
        } else {
            "our union rules"
        };
        let text = format!(
            "{} has {blocks} blocks of plant in {name}. The state stands behind {law} and a corporate tax adjustment of {:+} points. A contribution to the coming race is capped at {}.",
            world.firm(firm).map(|f| f.name.clone()).unwrap_or_default(),
            s.laws.tax_delta_bps / 100,
            money_text(s.laws.cap_cents),
        );
        let g = s.governor.id.clone();
        push_news(world, "letter", text, Some(firm), &g, i as u8);
    }
}

// ---------------------------------------------------------------------------------------------
// Rivals

/// What a rival firm would give, to whom, in which race. Rivals back the side whose positions
/// help them (they like right to work when they employ many, lax rules when they run heavy
/// plants, no common carrier when they own carriers) and give by temperament.
pub fn rival_gift(world: &World, ds: &Dataset, firm: Handle) -> Option<(u8, u8, i64)> {
    let entity = world.firms.get(firm)?;
    let temperament = entity
        .ceo
        .and_then(|p| ds.cast.get(p as usize))
        .map(|p| p.temperament)?;
    use cw_data::Temperament::*;
    let share_bps = match temperament {
        Cautious => 0,
        Financier => 2_000,
        Integrator | Builder => 4_000,
        Speculator => 6_000,
        Raider => 9_000,
    };
    if share_bps == 0 {
        return None;
    }
    let week = world.tick.0;
    // One decision a month, staggered by firm.
    if !(week + (firm.0 as u32 % 4)).is_multiple_of(4) {
        return None;
    }
    let cash = entity.book.cash() - entity.reserved_cash_cents;
    // The race closest to the firm's plants that is open.
    let mut states: Vec<usize> = entity
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
        .filter_map(|p| world.civics.state_of(p.region))
        .collect();
    states.sort_unstable();
    states.dedup();
    let mut offices: Vec<u8> = states.into_iter().map(|s| s as u8).collect();
    offices.push(NATIONAL);
    for office in offices {
        let Some(race) = world.civics.race(office) else {
            continue;
        };
        if !race.open(week) {
            continue;
        }
        let cap = cap_for(world, office);
        let room = cap - race.given_by(firm);
        if room < MIN_GIFT_CENTS {
            continue;
        }
        // Which candidate helps this firm: heavy or large employers lean to business positions;
        // a carrier owner dislikes common carrier rules; a firm that relies on unions' goodwill
        // (low reputation) leans the other way.
        let heavy = entity
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .any(|p| {
                matches!(
                    ds.recipes[p.recipe as usize].industry.as_str(),
                    "extraction" | "energy" | "chemical_industries" | "manufacturing"
                )
            });
        let carrier = world.logistics.carriers.iter().any(|c| c.firm == firm);
        let employs: i64 = entity
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .map(|p| p.workers.iter().sum::<i64>())
            .sum();
        let mut lean = 0i32;
        if heavy {
            lean += 1;
        }
        if carrier && office == NATIONAL {
            lean += 1;
        }
        if employs > 400 {
            lean += 1;
        }
        let pick = if lean >= 2 {
            0
        } else if lean == 0 {
            1
        } else if entity.reputation_bps >= 0 {
            0
        } else {
            1
        };
        // A gift is an expense, so a rival gives only out of what it earned: no more than a tenth
        // of the quarter's profit, and nothing in a quarter that lost money.
        let earned: i64 = entity
            .book
            .recent_weeks()
            .iter()
            .map(|w| w.net_income)
            .sum();
        if earned <= 0 {
            continue;
        }
        let amount = (cap * share_bps as i64 / 10_000)
            .min(room)
            .min(cash / 50)
            .min(earned / 10);
        if amount >= MIN_GIFT_CENTS {
            return Some((office, pick, amount));
        }
    }
    None
}

/// What a rival would bid on an open tender, if it wants the work: a price with a margin set by
/// temperament over its own cost for the delivery time, and the quickest time it can afford.
pub fn rival_bid(world: &World, ds: &Dataset, firm: Handle) -> Option<(u32, i64, u32)> {
    let entity = world.firms.get(firm)?;
    if !entity.operative() || active_for(world, firm) >= 2 {
        return None;
    }
    let is_builder = entity.industry == 5 || !entity.facilities.is_empty();
    if !is_builder {
        return None;
    }
    let temperament = entity
        .ceo
        .and_then(|p| ds.cast.get(p as usize))
        .map(|p| p.temperament);
    use cw_data::Temperament::*;
    let margin_bps = match temperament {
        Some(Raider) | Some(Speculator) => 800,
        Some(Cautious) | Some(Financier) => 1_100,
        _ => 900,
    };
    let cash = crate::economy::fx::firm_to_numeraire(
        world,
        ds,
        firm,
        entity.book.cash() - entity.reserved_cash_cents,
    );
    for t in world
        .civics
        .tenders
        .iter()
        .filter(|t| t.status == TenderStatus::Open)
    {
        if t.bids.iter().any(|b| b.firm == firm) {
            continue;
        }
        let weeks = (t.min_weeks + t.max_weeks) / 2;
        let cost = delivery_cost(world, t, weeks);
        if cash < working_capital(cost, weeks) {
            continue;
        }
        // The bidder's own reading of the cost, plus its margin.
        let estimate = cost * (10_000 + estimate_error_bps(world, t.id, firm)) / 10_000;
        let price = estimate * (10_000 + margin_bps) / 10_000;
        if price <= t.budget_cents {
            return Some((t.id, price, weeks));
        }
    }
    None
}
