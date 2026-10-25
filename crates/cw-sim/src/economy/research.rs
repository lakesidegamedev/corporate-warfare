//! Funded research, technology frontier and brand/quality.
//!
//! Level threshold = $100,000 * (level + 1)^2, capped at frontier + 1. At the threshold a saved
//! 7500 bps success draw resolves once; failure retains half the progress and the spend stays
//! spent. Technology level caps at 20 with two percent additive efficiency per level and a
//! sixty percent floor on input and labour intensity. Brand spend raises a regional product
//! stock by round(spend / baseline category spend * 10000) bps, at most 100 a week, decaying 10
//! a week toward 10000 and clamped to 7500..15000. Quality is 10000 plus 200 per tech level plus
//! up to 1000 from investment, capped at 20000.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;
use cw_core::rng::{Rng, TAG_RESEARCH};

use crate::state::{INDUSTRY_NAMES, World};

pub const RESEARCH_BASE_COST_CENTS: i64 = 10_000_000;
pub const MAX_TECH_LEVEL: u8 = 20;
pub const BRAND_FLOOR: i32 = 7_500;
pub const BRAND_CAP: i32 = 15_000;

pub fn threshold(level: u8) -> i64 {
    let factor = level as i64 + 1;
    RESEARCH_BASE_COST_CENTS * factor * factor
}

/// Spend this week's research budget and advance progress.
pub fn weekly(world: &mut World, dataset: &cw_data::Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        if !firm.operative() {
            continue;
        }
        let budget = firm.policies.research_budget_cents.max(0);
        let tech_level_before = firm.tech_level;
        let _ = tech_level_before;
        if budget > 0 {
            let cash = firm.book.cash().max(0);
            let spend = budget.min(cash);
            if spend > 0 {
                let _ = world.firms.get_mut(handle).unwrap().book.commit(
                    world.tick,
                    Txn::new().credit(Account::Cash, spend, "research").debit(
                        Account::OperatingExpense,
                        spend,
                        "research",
                    ),
                );
                let level = world.firms.get(handle).map(|f| f.tech_level).unwrap_or(0);
                let frontier = world.macro_state.tech_frontier;
                let cap = (frontier + 1).min(MAX_TECH_LEVEL);
                if level < cap {
                    let progress = world.firms.get(handle).unwrap().tech_progress_cents + spend;
                    if progress >= threshold(level) {
                        let mut rng = Rng::entity_stream(world.seed, TAG_RESEARCH, handle);
                        let success = rng.chance_bps(7_500);
                        let entity = world.firms.get_mut(handle).unwrap();
                        if success {
                            entity.tech_level = (entity.tech_level + 1).min(MAX_TECH_LEVEL);
                            entity.tech_progress_cents = 0;
                            let keys: Vec<u16> = entity.quality_bps.keys().copied().collect();
                            let level = entity.tech_level;
                            for key in keys {
                                entity.quality_bps.insert(key, quality(level, 0));
                            }
                        } else {
                            entity.tech_progress_cents = (threshold(level) - spend).max(0) / 2;
                        }
                    } else if let Some(entity) = world.firms.get_mut(handle) {
                        entity.tech_progress_cents = progress;
                    }
                }
            }
        }
        spend_quality(world, dataset, handle);
        spend_marketing(world, dataset, handle);
        // Brand decay toward the neutral 10000 baseline.
        let brand_keys: Vec<(u8, u16)> = world
            .firms
            .get(handle)
            .map(|firm| firm.brand_bps.keys().copied().collect())
            .unwrap_or_default();
        for key in brand_keys {
            if let Some(firm) = world.firms.get_mut(handle)
                && let Some(value) = firm.brand_bps.get_mut(&key)
            {
                if *value > 10_000 {
                    *value = (*value - 10).max(10_000);
                } else if *value < 10_000 {
                    *value = (*value + 10).min(10_000);
                }
            }
        }
    }
}

/// Spend this week's quality investment. Each $1,000 spent adds one quality basis point, at most
/// twenty-five a week, and quality never exceeds technology quality plus the prescribed 1,000 bps
/// investment ceiling.
fn spend_quality(world: &mut World, dataset: &cw_data::Dataset, handle: Handle) {
    let Some(firm) = world.firms.get(handle) else {
        return;
    };
    let budgets: Vec<(u16, i64)> = firm
        .policies
        .quality_budgets_cents
        .iter()
        .filter(|(_, amount)| **amount > 0)
        .map(|(product, amount)| (*product, *amount))
        .collect();
    if budgets.is_empty() {
        return;
    }
    for (product, budget) in budgets {
        let cash = world
            .firm(handle)
            .map(|f| f.book.cash())
            .unwrap_or(0)
            .max(0);
        let spend = budget.min(cash);
        if spend <= 0 {
            continue;
        }
        let tech = world.firm(handle).map(|f| f.tech_level).unwrap_or(0);
        let ceiling = quality(tech, 1_000);
        let current = world
            .firm(handle)
            .and_then(|f| f.quality_bps.get(&product).copied())
            .unwrap_or_else(|| quality(tech, 0));
        if current >= ceiling {
            continue;
        }
        let gain = (spend / 100_000).clamp(0, 25) as i32;
        let next = (current + gain).min(ceiling);
        if next == current {
            continue;
        }
        if let Some(entity) = world.firms.get_mut(handle) {
            let _ = entity.book.commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .credit(cw_core::ledger::Account::Cash, spend, "quality investment")
                    .debit(
                        cw_core::ledger::Account::OperatingExpense,
                        spend,
                        "quality investment",
                    ),
            );
            entity.quality_bps.insert(product, next);
        }
    }
    let _ = dataset;
}

/// Spend this week's marketing budget: half buys advertising from media firms in the corridor,
/// half builds brand stock. Advertising demand comes only from these real budgets.
fn spend_marketing(world: &mut World, dataset: &cw_data::Dataset, handle: Handle) {
    let Some(firm) = world.firms.get(handle) else {
        return;
    };
    let budget = firm.policies.marketing_budget_cents.max(0);
    if budget <= 0 {
        return;
    }
    let cash = firm.book.cash().max(0);
    let spend = budget.min(cash);
    if spend <= 0 {
        return;
    }
    let media_share = spend / 2;
    let brand_share = spend - media_share;
    let region = firm.region;
    let output_code = firm
        .facilities
        .iter()
        .filter_map(|facility| world.facilities.get(*facility))
        .find_map(|facility| {
            dataset
                .recipe(&dataset.recipes[facility.recipe as usize].id)
                .and_then(|recipe| dataset.product_code(&recipe.output))
        });
    if media_share > 0 {
        let media: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(_, candidate)| candidate.industry == 11 && candidate.operative())
            .map(|(media_handle, _)| media_handle)
            .collect();
        if media.is_empty() {
            let _ = world.outside.gateway_book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, media_share, "advertising")
                    .credit(Account::Revenue, media_share, "advertising"),
            );
        } else {
            let share = media_share / media.len() as i64;
            let mut remaining = media_share;
            for media_handle in media {
                let paid = share.min(remaining);
                if paid > 0 {
                    if let Some(media_firm) = world.firms.get_mut(media_handle) {
                        let _ = media_firm.book.commit(
                            world.tick,
                            Txn::new().debit(Account::Cash, paid, "advertising").credit(
                                Account::Revenue,
                                paid,
                                "advertising",
                            ),
                        );
                    }
                    remaining -= paid;
                }
            }
        }
        if let Some(firm) = world.firms.get_mut(handle) {
            let _ = firm.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, media_share, "marketing media")
                    .debit(Account::OperatingExpense, media_share, "marketing media"),
            );
        }
    }
    if brand_share > 0
        && let Some(code) = output_code
    {
        let baseline = dataset.reference_price(code).max(1) * 100;
        spend_brand(world, handle, region, code, brand_share, baseline);
    }
}

/// Spend on brand for one regional product market.
pub fn spend_brand(
    world: &mut World,
    firm: Handle,
    region: u8,
    product: u16,
    spend_cents: i64,
    baseline_weekly_category_spend: i64,
) -> i32 {
    if spend_cents <= 0 || baseline_weekly_category_spend <= 0 {
        return 0;
    }
    let cash = world
        .firms
        .get(firm)
        .map(|f| f.book.cash())
        .unwrap_or(0)
        .max(0);
    let spend = spend_cents.min(cash);
    if spend <= 0 {
        return 0;
    }
    let gain = round_even_div(
        spend as i128 * 10_000,
        baseline_weekly_category_spend as i128,
    ) as i32;
    let gain = gain.clamp(0, 100);
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new().credit(Account::Cash, spend, "brand").debit(
                Account::OperatingExpense,
                spend,
                "brand",
            ),
        );
        let entry = entity.brand_bps.entry((region, product)).or_insert(10_000);
        *entry = (*entry + gain).clamp(BRAND_FLOOR, BRAND_CAP);
        return gain;
    }
    0
}

/// Effective input/labour intensity after technology efficiency.
pub fn intensity_bps(tech_level: u8) -> i32 {
    (10_000 - tech_level as i32 * 200).max(6_000)
}

/// Quality from technology and bounded investment.
pub fn quality(tech_level: u8, investment_bps: i32) -> i32 {
    (10_000 + tech_level as i32 * 200 + investment_bps.clamp(0, 1_000)).min(20_000)
}

pub fn industry_of(name: &str) -> Option<u8> {
    INDUSTRY_NAMES
        .iter()
        .position(|industry| *industry == name)
        .map(|index| index as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds_follow_the_squared_rule() {
        assert_eq!(threshold(0), 10_000_000);
        assert_eq!(threshold(1), 40_000_000);
        assert_eq!(threshold(2), 90_000_000);
        assert_eq!(threshold(4), 250_000_000);
    }

    #[test]
    fn quality_investment_is_bounded_and_funded() {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            821,
            crate::state::ScenarioKind::Guided,
            crate::state::ScenarioMode::Sandbox,
        );
        let firm = world.investor.primary_group.unwrap();
        let facility = world.firm(firm).unwrap().facilities[0];
        let recipe = &dataset.recipes[world.facilities.get(facility).unwrap().recipe as usize];
        let product = dataset.product_code(&recipe.output).unwrap();
        let tech = world.firm(firm).unwrap().tech_level;
        let baseline = quality(tech, 0);
        if let Some(entity) = world.firms.get_mut(firm) {
            entity
                .book
                .commit(
                    world.tick,
                    cw_core::ledger::Txn::new()
                        .debit(cw_core::ledger::Account::Cash, 200_000_000, "funding")
                        .credit(
                            cw_core::ledger::Account::ShareCapital,
                            200_000_000,
                            "funding",
                        ),
                )
                .unwrap();
            entity
                .policies
                .quality_budgets_cents
                .insert(product, 1_000_000);
        }
        let cash_before = world.firm(firm).unwrap().book.cash();
        weekly(&mut world, &dataset);
        let first = world
            .firm(firm)
            .unwrap()
            .quality_bps
            .get(&product)
            .copied()
            .unwrap();
        assert!(first > baseline, "quality rises with investment");
        assert!(first <= baseline + 25, "at most twenty-five bps a week");
        assert!(
            world.firm(firm).unwrap().book.cash() < cash_before,
            "the budget is spent"
        );
        for _ in 0..80 {
            weekly(&mut world, &dataset);
        }
        let final_quality = world
            .firm(firm)
            .unwrap()
            .quality_bps
            .get(&product)
            .copied()
            .unwrap();
        assert!(
            final_quality <= quality(tech, 1_000),
            "quality is capped by technology plus the investment ceiling"
        );
        assert!(world.firm(firm).unwrap().book.is_balanced());
    }

    #[test]
    fn marketing_budget_buys_real_advertising_from_media_firms() {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            192,
            crate::state::ScenarioKind::Guided,
            crate::state::ScenarioMode::Sandbox,
        );
        let firm = world.investor.primary_group.unwrap();
        let media_revenue_before: i64 = world
            .firms
            .iter()
            .filter(|(_, candidate)| candidate.industry == 11)
            .map(|(_, candidate)| -candidate.book.balance(cw_core::ledger::Account::Revenue))
            .sum();
        let cash_before = world.firm(firm).unwrap().book.cash();
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .policies
            .marketing_budget_cents = 1_000_000;
        weekly(&mut world, &dataset);
        let media_revenue_after: i64 = world
            .firms
            .iter()
            .filter(|(_, candidate)| candidate.industry == 11)
            .map(|(_, candidate)| -candidate.book.balance(cw_core::ledger::Account::Revenue))
            .sum();
        assert!(
            media_revenue_after > media_revenue_before,
            "media firms earn advertising"
        );
        assert!(
            world.firm(firm).unwrap().book.cash() < cash_before,
            "the budget is spent"
        );
    }

    #[test]
    fn efficiency_and_quality_are_bounded() {
        assert_eq!(intensity_bps(0), 10_000);
        assert_eq!(intensity_bps(20), 6_000);
        assert_eq!(intensity_bps(40), 6_000);
        assert_eq!(quality(20, 1_000), 15_000);
        assert_eq!(quality(0, 0), 10_000);
    }
}
