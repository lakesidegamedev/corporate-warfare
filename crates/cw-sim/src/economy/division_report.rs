//! A division's detail report: financials, operating strategy, inputs, outputs, plants and
//! labour, for the interface.
//!
//! Read only. The ledger is kept per firm, so a division's results are estimated from the
//! plants' own records (last week's output, staffing and wages) and the firm's closed sales,
//! priced at the firm's asking prices today. The report never writes to the world, so it cannot
//! move a hash or a save.

use cw_core::ids::Handle;
use cw_data::Dataset;

use crate::economy::{operations, pricing, sectors, structure};
use crate::state::{BargainState, FacilityStatus, PricePosture, World};

/// One cost line, weekly cents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostLine {
    pub key: &'static str,
    pub cents: i64,
}

/// One input of the division.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputLine {
    pub product: u16,
    pub need_milli: i64,
    pub used_milli: i64,
    pub source: String,
    pub price_cents: i64,
    pub stock_milli: i64,
    /// Weeks of cover in hundredths, zero when the division needs none of it.
    pub cover_weeks_x100: i64,
    pub store_class: &'static str,
}

/// One product the division makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputLine {
    pub product: u16,
    pub by_product: bool,
    pub made_milli: i64,
    pub sold_milli: i64,
    pub stored_milli: i64,
    pub price_cents: i64,
    pub market_cents: i64,
    pub buyers: Vec<String>,
    pub store_class: &'static str,
}

/// One plant of the division.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlantLine {
    pub handle: Handle,
    pub blocks: u32,
    pub status: &'static str,
    pub utilisation_bps: i32,
    pub age_weeks: u32,
    pub staff_bps: i32,
    pub inputs_short: bool,
    pub capex_cents: i64,
    pub book_cents: i64,
}

/// One labour tier of the division.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierLine {
    pub employed: i64,
    pub needed: i64,
    pub wage_cents: i64,
    pub regional_wage_cents: i64,
    pub payroll_cents: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DivisionReport {
    pub revenue_cents: i64,
    pub costs: Vec<CostLine>,
    pub operating_profit_cents: i64,
    pub margin_bps: i32,
    /// Revenue of the last closed weeks at today's prices, oldest first.
    pub revenue_trend: Vec<i64>,
    pub capex_cents: i64,
    pub book_cents: i64,
    pub meaning: String,
    pub price_mode: &'static str,
    pub price_bps: i32,
    pub primary_product: u16,
    pub transfer_bps: i32,
    pub has_group_buyers: bool,
    pub stock_weeks: i32,
    pub stock_weeks_effective: i32,
    pub inputs: Vec<InputLine>,
    pub outputs: Vec<OutputLine>,
    pub plants: Vec<PlantLine>,
    pub tiers: [TierLine; 3],
    pub union_state: &'static str,
    pub union_organisation_bps: i32,
    pub shortage: bool,
}

fn class_key(dataset: &Dataset, product: u16) -> &'static str {
    dataset
        .storage_class_of(product)
        .map(|c| c.key())
        .unwrap_or("")
}

/// The report for one division, None when the firm has no plant of that type in the region.
pub fn report(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
) -> Option<DivisionReport> {
    build(world, dataset, firm, region, industry, true)
}

/// The figures the weekly records keep, at a fraction of the report's cost: revenue, costs,
/// operating profit and every line's made and sold, exactly as `report` has them. The text,
/// buyer names, market prices and revenue trend are left empty.
pub fn figures(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
) -> Option<DivisionReport> {
    build(world, dataset, firm, region, industry, false)
}

fn build(
    world: &World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    industry: u8,
    detail: bool,
) -> Option<DivisionReport> {
    let entity = world.firms.get(firm)?;
    let plant_handles = sectors::division_plants(world, dataset, firm, region, industry);
    let first = world.facilities.get(*plant_handles.first()?)?;
    let recipe = &dataset.recipes[first.recipe as usize];
    let tick = world.tick.0;
    let live = |status: FacilityStatus| status != FacilityStatus::Building;

    // Plants.
    let mut plants = Vec::new();
    let (mut made_full, mut made_real) = (0i64, 0i64);
    let (mut capex, mut book) = (0i64, 0i64);
    for handle in &plant_handles {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        let full = crate::economy::production::season_full_output(world, dataset, plant).0;
        let operating = plant.is_operating();
        if operating {
            made_full += full;
            made_real += plant.last_output.0;
        }
        capex += plant.capex_cents;
        book += (plant.capex_cents - plant.accumulated_depreciation).max(0);
        plants.push(PlantLine {
            handle: *handle,
            blocks: plant.blocks as u32,
            status: match plant.status {
                FacilityStatus::Building => "building",
                FacilityStatus::Retooling { .. } => "retooling",
                FacilityStatus::Operating => "operating",
                FacilityStatus::Mothballed => "mothballed",
            },
            utilisation_bps: if operating && full > 0 {
                (plant.last_output.0 * 10_000 / full).clamp(0, 10_000) as i32
            } else {
                0
            },
            age_weeks: tick.saturating_sub(plant.started.0),
            staff_bps: plant.last_staff_ratio_bps,
            inputs_short: plant.last_inputs_short,
            capex_cents: plant.capex_cents,
            book_cents: (plant.capex_cents - plant.accumulated_depreciation).max(0),
        });
    }
    let util_bps = if made_full > 0 {
        made_real * 10_000 / made_full
    } else {
        0
    };

    // Inputs.
    let mut inputs = Vec::new();
    let mut materials = 0i64;
    for (id, weekly) in &recipe.weekly_inputs_lots {
        let Some(&code) = dataset.product_index.get(id) else {
            continue;
        };
        let service = dataset.products[code as usize].kind.is_service();
        let mut need_milli = 0i64;
        for handle in &plant_handles {
            let Some(plant) = world.facilities.get(*handle) else {
                continue;
            };
            if !plant.is_operating() {
                continue;
            }
            let focus = operations::input_bps(world, dataset, *handle) as i64;
            need_milli += weekly * plant.blocks as i64 * 1_000 * focus / 10_000;
        }
        let used_milli = need_milli * util_bps / 10_000;
        let held = entity.inventory.get(&(code, region)).copied();
        let stock_milli = if service {
            0
        } else {
            held.map(|l| l.qty.0).unwrap_or(0)
        };
        let corridor = pricing::corridor_price(world, dataset, code);
        let group_seller = world
            .structure
            .links
            .iter()
            .find(|((buyer, _, product), _)| *buyer == firm && *product == code)
            .map(|((_, seller, _), _)| *seller);
        let own_plants = entity.facilities.iter().any(|h| {
            world.facilities.get(*h).is_some_and(|p| {
                p.region == region
                    && p.is_operating()
                    && dataset.recipes[p.recipe as usize].makes(id)
            })
        });
        let (source, price) = if let Some(seller) = group_seller {
            let s_industry = world
                .firms
                .get(seller)
                .map(|f| f.industry)
                .unwrap_or_default();
            let (_, _, transfer) =
                structure::transfer_quote(world, dataset, seller, code, region, s_industry);
            if !detail {
                (String::new(), transfer)
            } else {
                (
                    format!(
                        "Group: {}",
                        world
                            .firms
                            .get(seller)
                            .map(|f| f.name.as_str())
                            .unwrap_or("a sister company")
                    ),
                    transfer,
                )
            }
        } else if !detail {
            (String::new(), corridor)
        } else if service {
            ("Local service market".to_owned(), corridor)
        } else if own_plants {
            ("Own plants".to_owned(), corridor)
        } else {
            ("Open market".to_owned(), corridor)
        };
        let carried = held
            .filter(|l| l.qty.0 > 0)
            .map(|l| l.cost_cents * 1_000 / l.qty.0)
            .unwrap_or(price);
        materials += used_milli * carried / 1_000;
        inputs.push(InputLine {
            product: code,
            need_milli,
            used_milli,
            source,
            price_cents: price,
            stock_milli,
            cover_weeks_x100: if need_milli > 0 && !service {
                stock_milli * 100 / need_milli
            } else {
                0
            },
            store_class: if service {
                ""
            } else {
                class_key(dataset, code)
            },
        });
    }

    // Outputs and revenue.
    let mut outputs = Vec::new();
    let mut revenue = 0i64;
    let mut trend_units: Vec<(i64, i64)> = Vec::new();
    let mut primary = 0u16;
    for (index, (id, _)) in recipe.outputs().enumerate() {
        let Some(&code) = dataset.product_index.get(id) else {
            continue;
        };
        if index == 0 {
            primary = code;
        }
        let made: i64 = plant_handles
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .filter(|p| p.is_operating())
            .map(|p| p.last_made(dataset, id).0)
            .sum();
        // The division's share of what the firm makes of this good here.
        let firm_made: i64 = entity
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .filter(|p| p.region == region && p.is_operating())
            .map(|p| p.last_made(dataset, id).0)
            .sum();
        let share_bps = if firm_made > 0 {
            made * 10_000 / firm_made
        } else {
            0
        };
        let sold_lots = entity
            .last_closed_sales()
            .and_then(|w| w.get(&(region, code)).copied())
            .unwrap_or(0);
        let sold_milli = sold_lots * 1_000 * share_bps / 10_000;
        let price = pricing::posture_price(world, dataset, firm, code)
            .or_else(|| entity.policies.prices.get(&code).copied())
            .unwrap_or_else(|| dataset.reference_price(code))
            .max(1);
        revenue += sold_milli * price / 1_000;
        for (i, week) in entity.sales_history.iter().enumerate().filter(|_| detail) {
            let lots = week.get(&(region, code)).copied().unwrap_or(0);
            let milli = lots * 1_000 * share_bps / 10_000;
            if trend_units.len() <= i {
                trend_units.push((0, 0));
            }
            trend_units[i].0 += milli * price / 1_000;
        }
        let stored = entity.stock(code, region).0 * share_bps / 10_000;
        let buyers: Vec<String> = world
            .structure
            .links
            .iter()
            .filter(|_| detail)
            .filter(|((_, seller, product), _)| *seller == firm && *product == code)
            .filter_map(|((buyer, _, _), _)| world.firms.get(*buyer).map(|f| f.name.clone()))
            .collect();
        outputs.push(OutputLine {
            product: code,
            by_product: index > 0,
            made_milli: made,
            sold_milli,
            stored_milli: stored,
            price_cents: price,
            market_cents: if detail {
                pricing::market_price(world, dataset, firm, code)
            } else {
                0
            },
            buyers,
            store_class: class_key(dataset, code),
        });
    }
    let revenue_trend: Vec<i64> = trend_units.iter().map(|(r, _)| *r).collect();

    // Labour and the fixed lines.
    let crew_bps = crate::economy::production::crew_need_bps(world, recipe);
    let mut tiers = [TierLine {
        employed: 0,
        needed: 0,
        wage_cents: 0,
        regional_wage_cents: 0,
        payroll_cents: 0,
    }; 3];
    let mut fixed_maintenance = 0i64;
    let mut fixed_land = 0i64;
    let mut fixed_depreciation = 0i64;
    for handle in &plant_handles {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        for (tier, line) in tiers.iter_mut().enumerate() {
            line.employed += plant.workers[tier];
            if plant.is_operating() {
                line.needed +=
                    recipe.workers_low_mid_high[tier] * plant.blocks as i64 * crew_bps / 10_000;
            }
        }
        if live(plant.status) {
            let r = &dataset.recipes[plant.recipe as usize];
            fixed_maintenance +=
                r.weekly_maintenance(dataset.maintenance_annual_bps) * plant.blocks as i64;
            fixed_land += dataset.fixture_land_rent_cents_weekly;
            fixed_depreciation += r.weekly_depreciation() * plant.blocks as i64;
        }
    }
    let region_state = world.regions.get(region as usize);
    for (tier, line) in tiers.iter_mut().enumerate() {
        line.wage_cents = entity.policies.wage_offer[tier];
        line.regional_wage_cents = region_state
            .map(|r| r.labour[tier].prevailing_wage_cents)
            .unwrap_or(0);
        line.payroll_cents = line.employed * line.wage_cents;
    }
    let payroll: i64 = tiers.iter().map(|t| t.payroll_cents).sum();
    let shortage = tiers.iter().any(|t| t.needed > 0 && t.employed < t.needed);

    let costs = vec![
        CostLine {
            key: "materials",
            cents: materials,
        },
        CostLine {
            key: "wages",
            cents: payroll,
        },
        CostLine {
            key: "maintenance",
            cents: fixed_maintenance,
        },
        CostLine {
            key: "land",
            cents: fixed_land,
        },
        CostLine {
            key: "depreciation",
            cents: fixed_depreciation,
        },
    ];
    let total_cost: i64 = costs.iter().map(|c| c.cents).sum();
    let operating_profit = revenue - total_cost;

    // Strategy.
    let (price_mode, price_bps) = match entity.policies.price_postures.get(&primary) {
        Some(PricePosture::Market { bps }) => ("market", *bps),
        Some(PricePosture::CostPlus { bps }) => ("cost_plus", *bps),
        None if entity.policies.manual_prices.contains(&primary) => ("manual", 0),
        None => ("auto", 0),
    };
    let inputs_text = recipe
        .weekly_inputs_lots
        .keys()
        .filter(|_| detail)
        .filter_map(|id| dataset.product_index.get(id))
        .map(|c| dataset.products[*c as usize].name.to_lowercase())
        .collect::<Vec<_>>()
        .join(", ");
    let made_text = recipe
        .outputs()
        .filter(|_| detail)
        .filter_map(|(id, _)| dataset.product_index.get(id))
        .map(|c| dataset.products[*c as usize].name.to_lowercase())
        .collect::<Vec<_>>()
        .join(" and ");
    let meaning = if !detail {
        String::new()
    } else if inputs_text.is_empty() {
        format!("Makes {made_text} with no bought inputs.")
    } else {
        format!("Turns {inputs_text} into {made_text}.")
    };
    let has_group_buyers = world
        .structure
        .links
        .keys()
        .any(|(_, seller, _)| *seller == firm);
    let plan = first.stock_plan.weeks;
    let (union_state, union_org) = if entity.union.region == region {
        (
            match entity.union.state {
                BargainState::Idle => "none",
                BargainState::Ballot => "balloting",
                BargainState::Demanding { .. } => "demanding",
                BargainState::Striking { .. } => "striking",
            },
            entity.union.organisation_bps,
        )
    } else {
        ("none", 0)
    };

    Some(DivisionReport {
        revenue_cents: revenue,
        costs,
        operating_profit_cents: operating_profit,
        margin_bps: if revenue > 0 {
            (operating_profit * 10_000 / revenue).clamp(-99_999, 99_999) as i32
        } else {
            0
        },
        revenue_trend,
        capex_cents: capex,
        book_cents: book,
        meaning,
        price_mode,
        price_bps,
        primary_product: primary,
        transfer_bps: structure::transfer_bps(world, firm, region, industry),
        has_group_buyers,
        stock_weeks: plan,
        stock_weeks_effective: if plan > 0 {
            plan
        } else {
            entity.policies.input_cover_weeks.max(1)
        },
        inputs,
        outputs,
        plants,
        tiers,
        union_state,
        union_organisation_bps: union_org,
        shortage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    fn first_division(world: &World, dataset: &Dataset) -> (Handle, u8, u8) {
        let firm = world.investor.primary_group.unwrap();
        let d = &sectors::divisions_of(world, dataset, firm)[0];
        (firm, d.region, d.industry)
    }

    #[test]
    fn payroll_and_plants_match_the_plant_records() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 711, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let (firm, region, industry) = first_division(&world, &dataset);
        let r = report(&world, &dataset, firm, region, industry).unwrap();
        let entity = world.firms.get(firm).unwrap();
        let handles = sectors::division_plants(&world, &dataset, firm, region, industry);
        let expect: i64 = handles
            .iter()
            .map(|h| {
                let p = world.facilities.get(*h).unwrap();
                (0..3)
                    .map(|t| p.workers[t] * entity.policies.wage_offer[t])
                    .sum::<i64>()
            })
            .sum();
        let wages = r.costs.iter().find(|c| c.key == "wages").unwrap().cents;
        assert_eq!(wages, expect);
        assert_eq!(r.plants.len(), handles.len());
        assert_eq!(
            r.operating_profit_cents,
            r.revenue_cents - r.costs.iter().map(|c| c.cents).sum::<i64>()
        );
        let book: i64 = handles
            .iter()
            .map(|h| {
                let p = world.facilities.get(*h).unwrap();
                p.capex_cents - p.accumulated_depreciation
            })
            .sum();
        assert_eq!(r.book_cents, book);
    }

    #[test]
    fn a_plant_running_at_known_output_reports_known_utilisation_and_materials() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 711, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let (firm, region, industry) = first_division(&world, &dataset);
        let handles = sectors::division_plants(&world, &dataset, firm, region, industry);
        // Half of this week's real full run, the season's yield included.
        let mut halves = 0i64;
        for h in &handles {
            let full = crate::economy::production::season_full_output(
                &world,
                &dataset,
                world.facilities.get(*h).unwrap(),
            );
            halves += full.0 / 2;
            let p = world.facilities.get_mut(*h).unwrap();
            p.status = FacilityStatus::Operating;
            p.last_output = cw_core::quantity::MilliLots(full.0 / 2);
        }
        let r = report(&world, &dataset, firm, region, industry).unwrap();
        assert!(r.plants.iter().all(|p| p.utilisation_bps == 5_000));
        for input in &r.inputs {
            assert_eq!(input.used_milli, input.need_milli / 2);
        }
        for output in r.outputs.iter().filter(|o| !o.by_product) {
            assert_eq!(output.made_milli, halves);
        }
    }

    #[test]
    fn a_region_without_plants_has_no_report() {
        let dataset = Dataset::load().unwrap();
        let world = seed::build(&dataset, 711, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let (firm, region, industry) = first_division(&world, &dataset);
        let empty = (0..dataset.regions.len() as u8)
            .find(|r| sectors::division_plants(&world, &dataset, firm, *r, industry).is_empty())
            .unwrap();
        assert!(report(&world, &dataset, firm, empty, industry).is_none());
        assert!(report(&world, &dataset, firm, region, industry).is_some());
    }
}
