//! The rival's move selector: a utility ranking over the moves a firm could make this week.
//!
//! The move generators in this crate decide what is legal and sensible for the firm's books.
//! The selector decides which of those moves the firm's chief executive actually makes. Each
//! discretionary move has a class, an expected value in cents (the generator's own projection)
//! and a stance toward whoever it is aimed at. Its utility is the expected value scaled by
//!
//! * the temperament's weight for the class (integrators buy suppliers, raiders raid and lock
//!   lanes, financiers bid and float, speculators stockpile, builders build, the cautious sit
//!   on cash);
//! * the grudge the firm holds against the target, which makes hostile moves likelier, and the
//!   alliance it has with the target, which vetoes them and favours partnerships;
//! * difficulty: aggression raises or lowers hostile moves, and sharpness is how well the firm
//!   reads the value of its options (a blunt firm misjudges them by a deterministic margin).
//!
//! A move below its class weight is passed over on a stable draw seeded by the world, the firm,
//! the week and the class, so a reloaded save chooses identically. The survivors are ranked by
//! utility, then by command and target, and the best few are kept. Routine moves (prices,
//! budgets, wages, shipping policy, selling stock, answering deadlines) bypass the selector.
//! Every move still goes through the common command validator: the selector has no powers the
//! player lacks.

use cw_core::ids::Handle;
use cw_core::rng::Rng;
use cw_data::{Archetype, Dataset, Difficulty, Temperament};
use cw_sim::commands::CommandIntent;
use cw_sim::economy::rivalry;
use cw_sim::state::World;

use crate::Proposal;

const TAG_SELECT: u64 = 0x5345_4c45_4354_4149;
/// Most discretionary moves one firm makes in a slot.
pub const SLOT_MOVES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    Build,
    Retool,
    Raid,
    Takeover,
    Integrate,
    Bond,
    Venture,
    Listing,
    SpinOff,
    Carrier,
    Lane,
    Storage,
    Speculate,
}

impl Class {
    pub const ALL: [Class; 13] = [
        Class::Build,
        Class::Retool,
        Class::Raid,
        Class::Takeover,
        Class::Integrate,
        Class::Bond,
        Class::Venture,
        Class::Listing,
        Class::SpinOff,
        Class::Carrier,
        Class::Lane,
        Class::Storage,
        Class::Speculate,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Class::Build => "build",
            Class::Retool => "retool",
            Class::Raid => "raid",
            Class::Takeover => "takeover",
            Class::Integrate => "integrate",
            Class::Bond => "bond",
            Class::Venture => "venture",
            Class::Listing => "listing",
            Class::SpinOff => "spin_off",
            Class::Carrier => "carrier",
            Class::Lane => "lane",
            Class::Storage => "storage",
            Class::Speculate => "speculate",
        }
    }

    /// Moves that take something from another group.
    pub const fn hostile(self) -> bool {
        matches!(
            self,
            Class::Raid | Class::Takeover | Class::Integrate | Class::Lane
        )
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// Percent weight of each class by temperament, in `Class::ALL` order. 100 is neutral.
const fn row(t: Temperament) -> [i64; 13] {
    match t {
        //                   Bld Rtl Rd  Tk  Int Bnd Ven Lst Spn Car Lan Sto Spe
        Temperament::Integrator => [
            130, 110, 70, 100, 230, 110, 130, 100, 100, 150, 100, 160, 70,
        ],
        Temperament::Raider => [100, 110, 230, 140, 70, 90, 60, 90, 90, 110, 180, 70, 80],
        Temperament::Financier => [80, 90, 70, 190, 110, 210, 140, 200, 200, 90, 90, 70, 100],
        Temperament::Speculator => [80, 90, 80, 90, 70, 100, 100, 100, 100, 100, 110, 200, 240],
        Temperament::Builder => [210, 140, 50, 60, 90, 110, 90, 70, 80, 130, 80, 90, 60],
        Temperament::Cautious => [70, 80, 30, 40, 50, 50, 80, 60, 70, 90, 50, 90, 50],
    }
}

/// How a firm plays: its chief executive's temperament, or the habit of its archetype.
pub fn style(world: &World, dataset: &Dataset, firm: Handle) -> Temperament {
    if let Some(t) = cw_sim::economy::cast::temperament(world, dataset, firm) {
        return t;
    }
    match world.firms.get(firm).and_then(|f| f.archetype) {
        Some(Archetype::BrandBuilder) => Temperament::Integrator,
        Some(Archetype::Consolidator) => Temperament::Financier,
        Some(Archetype::Vulture) => Temperament::Raider,
        _ => Temperament::Builder,
    }
}

/// Percent weight a temperament puts on a class.
pub fn weight(t: Temperament, class: Class) -> i64 {
    row(t)[class.index()]
}

/// Archetype weights on [cost saving, growth, quality/brand, R&D, acquisition], pulled three
/// parts in four toward the chief executive's temperament.
pub fn blended_weights(archetype: Option<Archetype>, t: Temperament) -> [i64; 5] {
    let habit = archetype.unwrap_or(Archetype::CostLeader).weights();
    let person: [i64; 5] = match t {
        Temperament::Integrator => [2, 4, 2, 2, 3],
        Temperament::Raider => [4, 2, 4, 1, 5],
        Temperament::Financier => [2, 2, 1, 2, 5],
        Temperament::Speculator => [2, 2, 1, 3, 2],
        Temperament::Builder => [3, 5, 2, 3, 1],
        Temperament::Cautious => [4, 2, 1, 1, 1],
    };
    let mut out = [0i64; 5];
    for i in 0..5 {
        out[i] = (habit[i] + 3 * person[i] + 2) / 4;
    }
    out
}

/// Percent by which cash reserves are padded: the cautious hoard.
pub fn thrift_pct(t: Temperament) -> i64 {
    match t {
        Temperament::Cautious => 150,
        _ => 100,
    }
}

/// Percent of the usual marketing budget: raiders fight marketing wars.
pub fn marketing_pct(t: Temperament) -> i64 {
    match t {
        Temperament::Raider => 170,
        Temperament::Builder | Temperament::Integrator => 110,
        Temperament::Cautious => 70,
        _ => 100,
    }
}

/// Percent aggression: how readily the firm makes hostile moves.
pub fn aggression_pct(difficulty: Difficulty) -> i64 {
    match difficulty {
        Difficulty::Easy => 60,
        Difficulty::Standard => 100,
        Difficulty::Hard => 140,
    }
}

/// Percent spread of the firm's misjudgement of a move's value: sharpness.
pub fn blur_pct(difficulty: Difficulty) -> u64 {
    match difficulty {
        Difficulty::Easy => 40,
        Difficulty::Standard => 15,
        Difficulty::Hard => 0,
    }
}

/// What the firm feels about whoever a move is aimed at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stance {
    pub grudge: i32,
    pub ally: i32,
}

/// A candidate: a proposal with its class, expected value and stance.
#[derive(Clone, Debug)]
pub struct Move {
    pub class: Option<Class>,
    pub ev: i64,
    pub stance: Stance,
    pub proposal: Proposal,
}

impl Move {
    pub fn routine(proposal: Proposal) -> Move {
        Move {
            class: None,
            ev: 0,
            stance: Stance::default(),
            proposal,
        }
    }
}

/// Class of a discretionary intent, or none for routine ones.
pub fn classify(dataset: &Dataset, intent: &CommandIntent) -> Option<Class> {
    Some(match intent {
        CommandIntent::BuildFacility { recipe, .. } => {
            let r = dataset.recipes.get(*recipe as usize)?;
            if r.storage_class.is_some() {
                Class::Storage
            } else if r.transport_mode.is_some() {
                Class::Carrier
            } else {
                Class::Build
            }
        }
        CommandIntent::RetoolFacility { .. } => Class::Retool,
        CommandIntent::RaidMarket { .. } => Class::Raid,
        CommandIntent::StartDiligence { .. } => Class::Takeover,
        CommandIntent::IssueBond { .. } => Class::Bond,
        CommandIntent::FormJointVenture { .. } | CommandIntent::BuyOutVenture { .. } => {
            Class::Venture
        }
        CommandIntent::ListSubsidiary { .. } | CommandIntent::CarveOutDivision { .. } => {
            Class::Listing
        }
        CommandIntent::SpinOffSubsidiary { .. } => Class::SpinOff,
        CommandIntent::ReserveLane { .. } => Class::Lane,
        CommandIntent::BuyToStore { .. } => Class::Speculate,
        _ => return None,
    })
}

/// The holder a move is aimed at, if it has one.
pub fn target_of(world: &World, intent: &CommandIntent) -> Vec<Handle> {
    match intent {
        CommandIntent::RaidMarket { target, .. } => vec![*target],
        CommandIntent::StartDiligence { target, .. } => vec![*target],
        CommandIntent::FormJointVenture { partner, .. } => vec![*partner],
        CommandIntent::BuyOutVenture { venture, firm } => world
            .structure
            .jvs
            .iter()
            .find(|j| j.firm == *venture)
            .map(|j| vec![if j.a == *firm { j.b } else { j.a }])
            .unwrap_or_default(),
        CommandIntent::ReserveLane {
            firm,
            from,
            to,
            mode,
            ..
        } => {
            let key = cw_sim::economy::logistics::LaneKey::new(*from, *to, *mode);
            world
                .logistics
                .lanes
                .get(&key)
                .map(|lane| {
                    lane.users
                        .iter()
                        .filter(|(f, lots)| {
                            *lots > 0 && !cw_sim::economy::ownership::same_group(world, *f, *firm)
                        })
                        .map(|(f, _)| *f)
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// The strongest grudge and alliance the firm has among a move's targets.
pub fn stance_for(world: &World, firm: Handle, intent: &CommandIntent) -> Stance {
    let mut stance = Stance::default();
    for target in target_of(world, intent) {
        stance.grudge = stance.grudge.max(rivalry::grudge(world, firm, target));
        stance.ally = stance.ally.max(rivalry::alliance(world, firm, target));
    }
    stance
}

/// Percent multiplier on a class's weight from the firm's stance toward the target.
fn stance_pct(class: Class, stance: Stance) -> i64 {
    let g = stance.grudge as i64;
    let a = stance.ally as i64;
    if class.hostile() {
        // A grudge makes the strike likelier; a friend is not struck.
        if a >= 5_000 {
            return 0;
        }
        (100 + g / 40) * (10_000 - a * 2) / 10_000
    } else if class == Class::Venture {
        // Partners are chosen among friends and never among enemies.
        if g >= 5_000 {
            return 0;
        }
        (100 + a / 50) * (10_000 - g) / 10_000
    } else {
        100
    }
}

fn draw(world: &World, firm: Handle, salt: u64, bound: u64) -> u64 {
    let mixed = firm.0 ^ ((world.tick.0 as u64) << 40) ^ (salt << 56);
    Rng::entity_stream(world.seed, TAG_SELECT, Handle(mixed)).uniform_below(bound)
}

/// A move's utility in arbitrary units (cents scaled by percent), for ranking and tests.
pub fn utility(world: &World, t: Temperament, mv: &Move, firm: Handle) -> i64 {
    let Some(class) = mv.class else {
        return mv.ev;
    };
    let mut pct = weight(t, class) * stance_pct(class, mv.stance) / 100;
    if class.hostile() {
        pct = pct * aggression_pct(world.difficulty) / 100;
    }
    let blur = blur_pct(world.difficulty);
    let seen = if blur == 0 {
        mv.ev
    } else {
        let wobble = draw(world, firm, class as u64 + 32, blur * 2 + 1) as i64;
        mv.ev * (100 - blur as i64 + wobble) / 100
    };
    seen / 100 * pct
}

/// Choose this slot's moves. Routine moves pass; discretionary ones are filtered by their
/// effective class weight, ranked by utility and cut to [`SLOT_MOVES`].
pub fn choose(world: &World, dataset: &Dataset, firm: Handle, moves: Vec<Move>) -> Vec<Proposal> {
    let t = style(world, dataset, firm);
    let mut out: Vec<Proposal> = Vec::new();
    let mut ranked: Vec<(i64, Move)> = Vec::new();
    for mv in moves {
        let Some(class) = mv.class else {
            out.push(mv.proposal);
            continue;
        };
        let mut pct = weight(t, class) * stance_pct(class, mv.stance) / 100;
        if class.hostile() {
            pct = pct * aggression_pct(world.difficulty) / 100;
        }
        // A move below neutral is passed over in proportion on a stable draw.
        if pct < 100 && (draw(world, firm, class as u64, 100) as i64) >= pct {
            continue;
        }
        let u = utility(world, t, &mv, firm);
        ranked.push((u, mv));
    }
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.proposal.intent.key().cmp(b.1.proposal.intent.key()))
            .then_with(|| target_key(&a.1.proposal.intent).cmp(&target_key(&b.1.proposal.intent)))
    });
    out.extend(
        ranked
            .into_iter()
            .take(SLOT_MOVES)
            .map(|(_, mv)| mv.proposal),
    );
    out
}

fn target_key(intent: &CommandIntent) -> u64 {
    match intent {
        CommandIntent::RaidMarket { target, .. } | CommandIntent::StartDiligence { target, .. } => {
            target.0
        }
        CommandIntent::BuildFacility { recipe, region, .. } => {
            ((*recipe as u64) << 8) | *region as u64
        }
        CommandIntent::BuyToStore {
            product, region, ..
        } => ((*product as u64) << 8) | *region as u64,
        _ => 0,
    }
}
