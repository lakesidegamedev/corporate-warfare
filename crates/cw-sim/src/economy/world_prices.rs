//! World commodity prices: the outside world's price for oil, steel, grain and the rest.
//!
//! Every scenario feeds these into the gateways, so a barrel of fuel or a lot of steel costs
//! more to import and pays more to export when the world price is high. Prices are grouped
//! into families (oil, steel, grain and so on); a family's index starts at 10,000 basis points
//! on day one and moves weekly, and the gateway prices of every product in the family move with
//! it. Three modes:
//!
//! - Historical: the authored anchor points, interpolated weekly, the same every game.
//! - Historical with variance: the same shape with its timing shifted, its depth scaled and a
//!   little weekly noise, all drawn from the game's seed.
//! - Random: a neutral mean-reverting walk. Busts come from the business cycle instead.
//!
//! Interaction with the business cycle (`macro_cycle`), without counting anything twice:
//!
//! - World demand shocks feed the cycle. In the two historical modes the weekly move of the
//!   weighted world index nudges the cycle's momentum (a collapse is a demand shock). The nudge
//!   is also run through a shadow copy of the cycle's own gap dynamics, so the gap it explains
//!   is known.
//! - The cycle tilts the paths a little. A family's price is tilted by the part of the output
//!   gap the world index did not itself cause (gap minus the shadow gap), scaled by how
//!   cyclical the family is. The price a shock moved is never moved again for the same gap.
//! - Random mode has no feed, so the cycle's own busts reach prices only through the tilt.
//!
//! State is saved with the world, including the anchors it was started with, so a loaded game
//! continues exactly.

use std::collections::BTreeMap;

use cw_core::rng::{Rng, TAG_PRICES};
use cw_data::PricePath;
use cw_data::price_paths::PricePathFile;
use serde::{Deserialize, Serialize};

use crate::state::World;

/// Weeks in a quarter, the unit anchors are written in.
const QUARTER_WEEKS: i64 = 13;
const BASE_BPS: i32 = 10_000;
/// A family index never leaves this band, whatever the shocks and tilt do.
pub const INDEX_FLOOR_BPS: i32 = 2_000;
pub const INDEX_CEIL_BPS: i32 = 80_000;
/// Random mode stays within this band around the start.
const RANDOM_BAND_BPS: (i32, i32) = (5_000, 20_000);
/// Weeks of history kept for the trend, plus the current week.
const TREND_WEEKS: usize = 13;
/// Share of the weekly composite move that reaches the cycle's momentum, in percent.
const DEMAND_FEED_PCT: i64 = 8;
const FEED_LIMITS: (i64, i64) = (-15, 10);
/// Share of the unexplained output gap that tilts prices, in percent, before the family's own
/// cyclical weight.
const TILT_PCT: i64 = 30;
/// Half-life of a random-mode deviation is about 22 weeks.
const REVERSION_DIVISOR: i32 = 32;
const MOMENTUM_LIMITS: (i32, i32) = (-160, 100);
const GAP_LIMITS: (i32, i32) = (-700, 600);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriceMode {
    /// No world prices: gateways keep their opening prices apart from inflation.
    #[default]
    Off,
    Historical,
    HistoricalVariance,
    Random,
}

impl PriceMode {
    pub const fn key(self) -> &'static str {
        match self {
            PriceMode::Off => "off",
            PriceMode::Historical => "historical",
            PriceMode::HistoricalVariance => "historical_variance",
            PriceMode::Random => "random",
        }
    }

    fn from_path(path: PricePath) -> PriceMode {
        match path {
            PricePath::Historical => PriceMode::Historical,
            PricePath::HistoricalVariance => PriceMode::HistoricalVariance,
            PricePath::Random => PriceMode::Random,
        }
    }

    pub fn follows_history(self) -> bool {
        matches!(self, PriceMode::Historical | PriceMode::HistoricalVariance)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyState {
    pub key: String,
    pub label: String,
    pub weight: u32,
    pub cyclical_pct: u32,
    /// How this family moves. A family with no authored path in a historical game runs random.
    pub mode: PriceMode,
    /// Anchor points as (week, index in basis points).
    pub anchors: Vec<(i64, i32)>,
    pub shift_weeks: i32,
    pub depth_pct: i32,
    pub vol_bps: i32,
    /// The shaped path's value on day one, used to start every variant exactly at 10,000.
    pub start_bps: i32,
    pub noise_bps: i32,
    /// The path before the cycle's tilt.
    pub raw_bps: i32,
    /// The index the gateways follow: the path plus the tilt.
    pub index_bps: i32,
    /// The last weeks of the index, oldest first, for the trend.
    pub recent: Vec<i32>,
    pub rng: Rng,
}

impl FamilyState {
    /// Change over the last quarter, in basis points of the earlier index.
    pub fn trend_bps(&self) -> i32 {
        let Some(then) = self.recent.first().copied().filter(|v| *v > 0) else {
            return 0;
        };
        ((self.index_bps as i64 - then as i64) * 10_000 / then as i64) as i32
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldPrices {
    pub mode: PriceMode,
    pub families: Vec<FamilyState>,
    /// Product code to index into `families`.
    pub product_family: BTreeMap<u16, u16>,
    /// What the world index has pushed into the cycle's momentum and gap, tracked on a shadow
    /// copy of the cycle's dynamics so the tilt can leave it out.
    pub shadow_momentum_bps: i32,
    pub shadow_gap_bps: i32,
    pub composite_bps: i32,
    /// The output gap when the game began. Day-one prices already reflect it, so the tilt
    /// measures the gap against it.
    pub baseline_gap_bps: i32,
}

impl Default for WorldPrices {
    fn default() -> Self {
        WorldPrices {
            mode: PriceMode::Off,
            families: Vec::new(),
            product_family: BTreeMap::new(),
            shadow_momentum_bps: 0,
            shadow_gap_bps: 0,
            composite_bps: BASE_BPS,
            baseline_gap_bps: 0,
        }
    }
}

impl WorldPrices {
    pub fn family_of(&self, product: u16) -> Option<&FamilyState> {
        self.product_family
            .get(&product)
            .and_then(|index| self.families.get(*index as usize))
    }
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Linear interpolation between anchors; flat before the first and after the last.
pub fn interpolate_bps(anchors: &[(i64, i32)], week: i64) -> i32 {
    let Some(first) = anchors.first() else {
        return BASE_BPS;
    };
    if week <= first.0 {
        return first.1;
    }
    for pair in anchors.windows(2) {
        let (w0, v0) = pair[0];
        let (w1, v1) = pair[1];
        if week <= w1 {
            let span = (w1 - w0).max(1) as i128;
            let value = v0 as i128 + (v1 as i128 - v0 as i128) * (week - w0) as i128 / span;
            return value as i32;
        }
    }
    anchors.last().map(|a| a.1).unwrap_or(BASE_BPS)
}

/// The shaped historical value in a variance game: shifted in time, scaled in depth.
fn shaped_bps(family: &FamilyState, week: i64) -> i32 {
    let position = (week + family.shift_weeks as i64).max(0);
    let base = interpolate_bps(&family.anchors, position);
    let deviation = base as i64 - BASE_BPS as i64;
    (BASE_BPS as i64 + deviation * family.depth_pct as i64 / 100).max(1_500) as i32
}

/// Set up world prices for a new career. Products named in the content that do not exist in
/// this dataset are ignored; families with no existing product are left out.
pub fn init(
    world: &mut World,
    dataset: &cw_data::Dataset,
    file: &PricePathFile,
    scenario_id: &str,
    path: PricePath,
    seed: u64,
) {
    let requested = PriceMode::from_path(path);
    let scenario = file.scenarios.get(scenario_id);
    let mut product_family = BTreeMap::new();
    let mut used: Vec<&String> = Vec::new();
    for (product, family) in &file.products {
        let (Some(code), Some(info)) = (
            dataset.product_index.get(product),
            file.families.get(family),
        ) else {
            continue;
        };
        if info.listed_only && !scenario.is_some_and(|s| s.families.contains_key(family)) {
            continue;
        }
        if !used.contains(&family) {
            used.push(family);
        }
        product_family.insert(*code, family);
    }
    used.sort();
    let mut global = Rng::stream(seed, TAG_PRICES);
    let global_shift = global.uniform_inclusive(-20, 20) as i32;
    let mut families = Vec::new();
    for key in &used {
        let info = &file.families[*key];
        let anchors: Vec<(i64, i32)> = scenario
            .and_then(|s| s.families.get(*key))
            .map(|points| {
                points
                    .iter()
                    .map(|p| (p[0] as i64 * QUARTER_WEEKS, p[1] * 100))
                    .collect()
            })
            .unwrap_or_default();
        let mode = if requested.follows_history() && anchors.is_empty() {
            PriceMode::Random
        } else {
            requested
        };
        let mut rng = Rng::stream(seed ^ fnv(key), TAG_PRICES);
        let shift = global_shift + rng.uniform_inclusive(-10, 10) as i32;
        let depth = rng.uniform_inclusive(60, 140) as i32;
        let vol = rng.uniform_inclusive(250, 450) as i32;
        let mut family = FamilyState {
            key: (*key).clone(),
            label: info.label.clone(),
            weight: info.weight,
            cyclical_pct: info.cyclical_pct,
            mode,
            anchors,
            shift_weeks: shift,
            depth_pct: depth,
            vol_bps: vol,
            start_bps: BASE_BPS,
            noise_bps: 0,
            raw_bps: BASE_BPS,
            index_bps: BASE_BPS,
            recent: vec![BASE_BPS],
            rng,
        };
        family.start_bps = shaped_bps(&family, 0);
        families.push(family);
    }
    let index_of: BTreeMap<&String, u16> = used
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, index as u16))
        .collect();
    let product_family = product_family
        .into_iter()
        .map(|(code, family)| (code, index_of[family]))
        .collect();
    let any_history = families.iter().any(|f| f.mode.follows_history());
    world.world_prices = WorldPrices {
        mode: if requested.follows_history() && !any_history {
            PriceMode::Random
        } else {
            requested
        },
        families,
        product_family,
        shadow_momentum_bps: 0,
        shadow_gap_bps: 0,
        composite_bps: BASE_BPS,
        baseline_gap_bps: world.macro_state.cycle.output_gap_bps,
    };
}

/// Advance every family one week, tilt it by the cycle, move the gateways and feed the cycle.
/// Runs just before the business cycle's own weekly step.
pub fn weekly(world: &mut World) {
    if world.world_prices.mode == PriceMode::Off || world.world_prices.families.is_empty() {
        return;
    }
    let week = world.tick.0 as i64;
    let mut state = std::mem::take(&mut world.world_prices);
    let unexplained_gap =
        world.macro_state.cycle.output_gap_bps - state.baseline_gap_bps - state.shadow_gap_bps;
    let mut before = Vec::with_capacity(state.families.len());
    let (mut weighted, mut weights) = (0i64, 0i64);
    for (index, family) in state.families.iter_mut().enumerate() {
        before.push(family.index_bps);
        family.raw_bps = match family.mode {
            PriceMode::Historical => interpolate_bps(&family.anchors, week),
            PriceMode::HistoricalVariance => {
                let noise =
                    family.noise_bps * 85 / 100 + family.rng.uniform_inclusive(-80, 80) as i32;
                family.noise_bps = noise;
                let shaped = shaped_bps(family, week) as i64;
                (shaped * BASE_BPS as i64 / family.start_bps.max(1) as i64
                    * (BASE_BPS as i64 + noise as i64)
                    / BASE_BPS as i64) as i32
            }
            _ => {
                let x = family.raw_bps;
                let shock = family
                    .rng
                    .uniform_inclusive(-family.vol_bps as i64, family.vol_bps as i64)
                    as i32;
                (x + (BASE_BPS - x) / REVERSION_DIVISOR + shock)
                    .clamp(RANDOM_BAND_BPS.0, RANDOM_BAND_BPS.1)
            }
        };
        // A timeline shock is a world price move like any other: it feeds the cycle with the
        // path and is tilted with it, but is never written into the path itself.
        let shock = crate::economy::timeline::price_shock_bps(world, index as u16) as i64;
        if family.mode.follows_history() {
            weighted += family.raw_bps as i64 * (BASE_BPS as i64 + shock).max(1) / BASE_BPS as i64
                * family.weight as i64;
            weights += family.weight as i64;
        }
        let tilt = unexplained_gap as i64 * TILT_PCT / 100 * family.cyclical_pct as i64 / 100;
        family.index_bps = (family.raw_bps as i64 * (BASE_BPS as i64 + tilt + shock)
            / BASE_BPS as i64)
            .clamp(INDEX_FLOOR_BPS as i64, INDEX_CEIL_BPS as i64) as i32;
        family.recent.push(family.index_bps);
        let excess = family.recent.len().saturating_sub(TREND_WEEKS + 1);
        family.recent.drain(..excess);
    }

    // Gateway prices follow their family's change, the same way inflation scales them.
    for gateway in &mut world.gateways {
        let Some(&index) = state.product_family.get(&gateway.product) else {
            continue;
        };
        let (old, new) = (
            before[index as usize],
            state.families[index as usize].index_bps,
        );
        if old == new || old <= 0 {
            continue;
        }
        let scale = |price: i64| -> i64 {
            ((price as i128 * new as i128 + old as i128 / 2) / old as i128).max(1) as i64
        };
        gateway.sell_price_cents = scale(gateway.sell_price_cents);
        gateway.buy_price_cents = scale(gateway.buy_price_cents);
    }

    // The world demand shock reaches the cycle, and a shadow copy of the cycle's own damping
    // records how much of the gap that explains.
    if weights > 0 {
        let composite = (weighted / weights) as i32;
        let impulse = ((composite as i64 - state.composite_bps as i64) * DEMAND_FEED_PCT / 100)
            .clamp(FEED_LIMITS.0, FEED_LIMITS.1) as i32;
        state.composite_bps = composite;
        if week > 0 && impulse != 0 {
            let cycle = &mut world.macro_state.cycle;
            cycle.momentum_bps =
                (cycle.momentum_bps + impulse).clamp(MOMENTUM_LIMITS.0, MOMENTUM_LIMITS.1);
        }
        let m = (state.shadow_momentum_bps + impulse).clamp(MOMENTUM_LIMITS.0, MOMENTUM_LIMITS.1);
        let m = m * 93 / 100 - state.shadow_gap_bps * 2 / 100;
        state.shadow_momentum_bps = m;
        state.shadow_gap_bps = (state.shadow_gap_bps + m / 5).clamp(GAP_LIMITS.0, GAP_LIMITS.1);
    }
    world.world_prices = state;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_core::calendar::Tick;

    fn setup(seed_value: u64, scenario: &str, path: PricePath) -> (World, cw_data::Dataset) {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = seed::build(
            &dataset,
            seed_value,
            ScenarioKind::Guided,
            ScenarioMode::Sandbox,
        );
        init(
            &mut world,
            &dataset,
            &PricePathFile::load(),
            scenario,
            path,
            seed_value,
        );
        (world, dataset)
    }

    fn run(world: &mut World, weeks: u32) -> Vec<i32> {
        let oil = world
            .world_prices
            .families
            .iter()
            .position(|f| f.key == "oil")
            .unwrap();
        let mut series = Vec::new();
        for week in 0..weeks {
            world.tick = Tick(week);
            weekly(world);
            series.push(world.world_prices.families[oil].index_bps);
        }
        series
    }

    #[test]
    fn historical_follows_the_authored_shape_and_is_the_same_every_seed() {
        let (mut a, _) = setup(1, "great-lakes-2000", PricePath::Historical);
        let (mut b, _) = setup(99, "great-lakes-2000", PricePath::Historical);
        let oil_a = run(&mut a, 600);
        let oil_b = run(&mut b, 600);
        assert_eq!(oil_a, oil_b, "no randomness in the historical path");
        assert_eq!(oil_a[0], 10_000);
        let peak = oil_a[..520].iter().copied().max().unwrap();
        let after = oil_a[34 * 13 + 13..].iter().copied().min().unwrap();
        assert!(
            peak >= 40_000,
            "the 2008 peak is several times the start: {peak}"
        );
        assert!(after <= 25_000, "the crash follows: {after}");
        assert!(oil_a[8 * 13] < 10_000, "2002 oil is below the 2000 level");
    }

    #[test]
    fn variance_is_deterministic_per_seed_and_differs_across_seeds() {
        let run_seed = |seed| {
            let (mut w, _) = setup(seed, "great-lakes-2000", PricePath::HistoricalVariance);
            run(&mut w, 520)
        };
        assert_eq!(run_seed(7), run_seed(7));
        assert_ne!(run_seed(7), run_seed(8));
    }

    #[test]
    fn variance_keeps_the_shape_within_bounds() {
        for seed in 1..=16u64 {
            let (mut w, _) = setup(seed, "great-lakes-2000", PricePath::HistoricalVariance);
            let oil = run(&mut w, 520);
            assert!(
                (9_800..=10_200).contains(&oil[0]),
                "starts at the base: {}",
                oil[0]
            );
            let peak = *oil.iter().max().unwrap();
            let trough_after = oil[oil.iter().position(|v| *v == peak).unwrap()..]
                .iter()
                .copied()
                .min()
                .unwrap();
            assert!((25_000..=70_000).contains(&peak), "seed {seed} peak {peak}");
            assert!(trough_after < peak * 8 / 10, "seed {seed} still has a bust");
            assert!(
                oil.iter()
                    .all(|v| (INDEX_FLOOR_BPS..=INDEX_CEIL_BPS).contains(v))
            );
        }
    }

    #[test]
    fn random_stays_neutral_and_mean_reverting() {
        let mut means = Vec::new();
        for seed in 1..=20u64 {
            let (mut w, _) = setup(seed, "great-lakes-2000", PricePath::Random);
            let oil = run(&mut w, 780);
            assert!(
                oil.iter().all(|v| (4_000..=21_000).contains(v)),
                "seed {seed}"
            );
            means.push(oil.iter().map(|v| *v as i64).sum::<i64>() / oil.len() as i64);
        }
        let grand = means.iter().sum::<i64>() / means.len() as i64;
        assert!((9_000..=11_000).contains(&grand), "no drift: {grand}");
        assert!(means.iter().any(|m| *m != means[0]), "paths differ");
    }

    #[test]
    fn a_scenario_without_data_runs_random() {
        let (w, _) = setup(3, "atlantis-2050", PricePath::Historical);
        assert_eq!(w.world_prices.mode, PriceMode::Random);
        assert!(
            w.world_prices
                .families
                .iter()
                .all(|f| f.mode == PriceMode::Random)
        );
    }

    #[test]
    fn gateway_prices_follow_the_family_index() {
        let (mut world, dataset) = setup(5, "great-lakes-2000", PricePath::Historical);
        let fuel = dataset.product_index["fuel"];
        let start: Vec<(i64, i64)> = world
            .gateways
            .iter()
            .filter(|g| g.product == fuel)
            .map(|g| (g.sell_price_cents, g.buy_price_cents))
            .collect();
        let oil = run(&mut world, 24 * 13 + 1);
        let index = *oil.last().unwrap() as i64;
        for (gateway, (sell, buy)) in world
            .gateways
            .iter()
            .filter(|g| g.product == fuel)
            .zip(start)
        {
            let want_sell = sell * index / 10_000;
            let want_buy = buy * index / 10_000;
            assert!(
                (gateway.sell_price_cents - want_sell).abs() * 500 <= want_sell,
                "sell"
            );
            assert!(
                (gateway.buy_price_cents - want_buy).abs() * 500 <= want_buy,
                "buy"
            );
        }
        let unmapped = dataset.product_index["goods"];
        assert!(world.world_prices.family_of(unmapped).is_none());
    }

    #[test]
    fn the_crash_feeds_the_cycle_and_the_tilt_leaves_that_part_out() {
        let (mut hist, _) = setup(4, "great-lakes-2000", PricePath::Historical);
        let (mut rand, _) = setup(4, "great-lakes-2000", PricePath::Random);
        let mut lowest_shadow = 0;
        for week in 0..(40 * 13) {
            hist.tick = Tick(week);
            rand.tick = Tick(week);
            weekly(&mut hist);
            weekly(&mut rand);
            lowest_shadow = lowest_shadow.min(hist.world_prices.shadow_gap_bps);
            assert_eq!(
                rand.world_prices.shadow_gap_bps, 0,
                "random mode feeds nothing"
            );
        }
        assert!(
            lowest_shadow < -20,
            "the 2008 collapse is a demand shock: {lowest_shadow}"
        );

        // With the whole gap explained by the shadow, the tilt is zero.
        hist.world_prices.baseline_gap_bps = 0;
        hist.world_prices.shadow_gap_bps = -300;
        hist.macro_state.cycle.output_gap_bps = -300;
        hist.tick = Tick(5);
        let before: Vec<i32> = hist
            .world_prices
            .families
            .iter()
            .map(|f| f.raw_bps)
            .collect();
        weekly(&mut hist);
        for (family, _) in hist.world_prices.families.iter().zip(before) {
            assert_eq!(
                family.index_bps,
                family.raw_bps.clamp(INDEX_FLOOR_BPS, INDEX_CEIL_BPS)
            );
        }
        // An unexplained gap tilts cyclical families down, and grain least.
        hist.world_prices.shadow_gap_bps = 0;
        hist.tick = Tick(6);
        weekly(&mut hist);
        let oil = hist
            .world_prices
            .families
            .iter()
            .find(|f| f.key == "oil")
            .unwrap();
        let grain = hist
            .world_prices
            .families
            .iter()
            .find(|f| f.key == "grain")
            .unwrap();
        assert!(oil.index_bps < oil.raw_bps);
        assert!(
            (oil.raw_bps - oil.index_bps) as i64 * grain.raw_bps as i64
                > (grain.raw_bps - grain.index_bps) as i64 * oil.raw_bps as i64
        );
    }

    #[test]
    fn state_survives_a_save_and_continues_identically() {
        let (mut world, _) = setup(11, "texas-1981", PricePath::HistoricalVariance);
        run(&mut world, 120);
        let bytes = postcard::to_allocvec(&world.world_prices).unwrap();
        let loaded: WorldPrices = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.families, world.world_prices.families);
        let mut copy = world.clone();
        copy.world_prices = loaded;
        for week in 120..300 {
            world.tick = Tick(week);
            copy.tick = Tick(week);
            weekly(&mut world);
            weekly(&mut copy);
        }
        assert_eq!(world.world_prices.families, copy.world_prices.families);
        assert_eq!(
            crate::state::canonical_hash(&world),
            crate::state::canonical_hash(&copy)
        );
    }

    #[test]
    fn texas_oil_collapses_by_1986() {
        let (mut w, _) = setup(2, "texas-1981", PricePath::Historical);
        let oil = run(&mut w, 6 * 52);
        let low = oil.iter().copied().min().unwrap();
        assert!((3_000..=4_500).contains(&low), "trough {low}");
        assert!(oil[0] == 10_000);
    }
}
