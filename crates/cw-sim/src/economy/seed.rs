//! World construction for the two release scenarios.
//!
//! Seeding rules come from CONTENT-PACING.md: thirty named fictional firms, two per industry;
//! the rest of the real economy is finite background employment and government; aggregate
//! totals are scaled to one percent of the corridor economy while per-worker wages and
//! per-lot prices keep their authored levels. Named production is subtracted from the
//! background rather than added on top of it.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Ledger, Txn};
use cw_core::money::{Bps, Cents};
use cw_core::quantity::MilliLots;
use cw_core::rng::Rng;
use cw_data::Dataset;

use crate::state::*;

/// Labelled engineering scale: one percent of the corridor economy's workforce.
pub const CORRIDOR_WORKERS: i64 = 60_000;
const TIER_SPLIT_BPS: [i64; 3] = [5_800, 3_200, 1_000];
const PARTICIPATION_BPS: i64 = 9_500;
const HOUSEHOLD_SAVINGS_WEEKS: i64 = 13;
const BASE_POLICY_RATE_BPS: i32 = 600;

const OUTSIDE_INVESTORS: [(&str, i64, i32); 8] = [
    ("Cedar Ridge Holdings", 250_000_000, 10_000),
    ("Lakeshore Pension Fund", 400_000_000, 10_500),
    ("Northwind Capital", 300_000_000, 9_500),
    ("Great Lakes Endowment", 350_000_000, 10_000),
    ("Harbour Point Partners", 200_000_000, 11_000),
    ("Inland Mutual", 275_000_000, 10_000),
    ("Stonegate Trust", 325_000_000, 9_800),
    ("Meridian Investors", 225_000_000, 10_200),
];

const OUTSIDE_LENDERS: [(&str, i64); 4] = [
    ("Lakeshore Commercial Credit", 600_000_000),
    ("Midland Trust Bank", 450_000_000),
    ("Great Lakes Capital Partners", 520_000_000),
    ("Corridor Mutual Lending", 380_000_000),
];

/// Household expenditure weights, summing 10000: food, goods, care, software, telecom, lease.
pub const HOUSEHOLD_WEIGHTS: [i64; 6] = [4_400, 3_000, 400, 500, 900, 800];

fn industry_index(industry: &str) -> u8 {
    INDUSTRY_NAMES
        .iter()
        .position(|name| *name == industry)
        .expect("validated industry") as u8
}

/// Build the opening world with the standard options.
pub fn build(dataset: &Dataset, seed: u64, scenario: ScenarioKind, mode: ScenarioMode) -> World {
    build_with(
        dataset,
        seed,
        scenario,
        mode,
        &crate::state::StartOptions::default(),
    )
}

/// Build the opening world for the new-game screen's difficulty, economy and capital.
pub fn build_with(
    dataset: &Dataset,
    seed: u64,
    scenario: ScenarioKind,
    mode: ScenarioMode,
    options: &crate::state::StartOptions,
) -> World {
    let capital = options.capital();
    let (cycle, opening_rate, opening_inflation) =
        crate::state::Cycle::for_start(options.economy, BASE_POLICY_RATE_BPS);
    let mut world = World {
        tick: Tick::OPENING,
        revision: 0,
        seed,
        dataset_hash: dataset.hash.clone(),
        scenario,
        mode,
        difficulty: options.difficulty,
        investor: Investor {
            name: "Investor".to_owned(),
            cash_cents: capital,
            book: Ledger::default(),
            personal_loans_cents: 0,
            primary_group: None,
            managed_firms: Vec::new(),
            recovery_used: 0,
            last_recovery_week: 0,
            assisted_score: false,
            career_losses_cents: 0,
            realised_gains_cents: 0,
            starting_net_worth_cents: capital,
        },
        firms: Default::default(),
        facilities: Default::default(),
        contracts: Default::default(),
        procurements: Default::default(),
        shipments: Vec::new(),
        loans: Default::default(),
        bonds: Default::default(),
        insurance: Default::default(),
        deals: Default::default(),
        regions: Vec::new(),
        government: Government {
            book: Ledger::default(),
            procurement_budget_cents: 0,
            healthcare_budget_cents: 0,
            training_budget_cents: 0,
            last_tax_receipts_cents: 0,
            tax_receipts_history: Vec::new(),
            week_tax_receipts_cents: 0,
            unpaid_transfer_need_cents: 0,
            bonds_outstanding_cents: 0,
            pending_procurement: BTreeMap::new(),
        },
        gateways: Vec::new(),
        world_prices: Default::default(),
        exchange: Exchange::default(),
        outside: OutsideSectors {
            lenders: OUTSIDE_LENDERS
                .iter()
                .map(|(name, capital)| OutsideHolder {
                    name: (*name).to_owned(),
                    capital_cents: *capital,
                    reservation_factor_bps: 10_000,
                })
                .collect(),
            investors: OUTSIDE_INVESTORS
                .iter()
                .map(|(name, capital, factor)| OutsideHolder {
                    name: (*name).to_owned(),
                    capital_cents: *capital,
                    reservation_factor_bps: *factor,
                })
                .collect(),
            advocacy_book: Ledger::default(),
            legal_services_book: Ledger::default(),
            gateway_book: Ledger::default(),
            gateway_floor_cents: 0,
        },
        macro_state: MacroState {
            policy_rate_bps: opening_rate,
            inflation_bps: opening_inflation,
            real_growth_bps: 300,
            tech_frontier: 2,
            price_index_bps: 10_000,
            regime: None,
            rng: Rng::stream(seed, cw_core::rng::TAG_MACRO),
            cycle,
        },
        events: EventState {
            pending: Vec::new(),
            next_id: 1,
            cooldowns: BTreeMap::new(),
            rng: Rng::stream(seed, cw_core::rng::TAG_EVENTS),
            last_quarter_check: 0,
        },
        news: Vec::new(),
        accepted_commands: Vec::new(),
        command_nonces: BTreeMap::new(),
        next_sequence: 1,
        score: None,
        weeks_with_past_due: 0,
        weeks_elapsed: 0,
        regulatory_breaches: 0,
        failed_worker_settlements: 0,
        group_cache: BTreeMap::new(),
        pending_pause: None,
        construction_orders: BTreeMap::new(),
        raids: Vec::new(),
        objective: None,
        objective_history: Vec::new(),
        price_history: Vec::new(),
        firm_history: BTreeMap::new(),
        politics: crate::state::Politics::default(),
    };

    // Outside gateway capital is finite but real: exports are paid from it, imports add to it.
    // The outside economy is large next to the corridor; weekly pacing in `clear_week` keeps
    // a sustained surplus from emptying it at once.
    let _ = world.outside.gateway_book.commit(
        Tick::OPENING,
        Txn::new()
            .debit(
                cw_core::ledger::Account::Cash,
                50_000_000_000,
                "gateway capital",
            )
            .credit(
                cw_core::ledger::Account::ShareCapital,
                50_000_000_000,
                "gateway capital",
            ),
    );
    build_regions(&mut world, dataset);
    build_government(&mut world, dataset);
    seed_named_firms(&mut world, dataset);
    if scenario == ScenarioKind::Guided {
        seed_player_firm(&mut world, dataset, seed);
    }
    refresh_region_incomes(&mut world, dataset);
    build_gateways(&mut world, dataset);
    seed_supply_contracts(&mut world, dataset);
    crate::economy::procurement::ensure_contracts(&mut world, dataset);
    compute_household_baselines(&mut world, dataset);
    seed_share_marks(&mut world, dataset);

    world.investor.starting_net_worth_cents = investor_net_worth(&world, dataset);
    // Opening registers are the baseline, so a co-investor present at the start is not reported
    // as a new hostile stake in week one.
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let peak = crate::economy::events_engine::outside_peak_bps(&world, handle);
        if let Some(firm) = world.firms.get_mut(handle) {
            firm.outside_peak_bps = peak;
        }
    }
    world
}

fn build_regions(world: &mut World, dataset: &Dataset) {
    let total_weight: i64 = dataset
        .regions
        .iter()
        .map(|r| r.demand_index_bps as i64)
        .sum();
    for region in &dataset.regions {
        let share = region.demand_index_bps as i64;
        let population_total = CORRIDOR_WORKERS * share / total_weight;
        let wage_scale = region.wage_index_bps as i64;
        let mut labour = [LabourTier::default(); 3];
        for tier in 0..3 {
            let population = population_total * TIER_SPLIT_BPS[tier] / 10_000;
            let wage = dataset.baseline_weekly_wage_cents[tier] * wage_scale / 10_000;
            labour[tier] = LabourTier {
                population,
                employed: 0,
                reservation_wage_cents: wage * 70 / 100,
                prevailing_wage_cents: wage,
                search_carry: 0,
            };
        }
        world.regions.push(RegionState {
            code: region.code.clone(),
            name: region.name.clone(),
            demand_index_bps: region.demand_index_bps,
            wage_index_bps: region.wage_index_bps,
            land_index_bps: region.land_index_bps,
            labour,
            households: Household {
                savings_cents: 0,
                last_income_cents: 0,
                baseline_income_cents: 0,
                weights: BTreeMap::new(),
                baseline_lots: BTreeMap::new(),
                last_sale_price_cents: BTreeMap::new(),
                spend_cents: BTreeMap::new(),
            },
            policy: RegionPolicy::default(),
            demand_modifiers: Vec::new(),
            supply_modifiers: Vec::new(),
            organisation_bps: region.opening_organisation_bps,
            catastrophe_until: None,
            background_employed: [0; 3],
            background_wage_cents: labour.map(|tier| tier.prevailing_wage_cents),
            paid_named_wages_cents: 0,
        });
    }
}

fn build_government(world: &mut World, dataset: &Dataset) {
    let receipts = government_weekly_receipts(world, dataset);
    world.government.last_tax_receipts_cents = receipts;
    world.government.tax_receipts_history = vec![receipts; 13];
    let opening = receipts * 13;
    // Fiscal counterparty: the treasury holds real cash issued against a matching bond.
    let _ = world.government.book.commit(
        Tick::OPENING,
        Txn::new()
            .debit(cw_core::ledger::Account::Cash, opening, "fiscal opening")
            .credit(cw_core::ledger::Account::Bonds, opening, "fiscal opening"),
    );
    world.government.bonds_outstanding_cents = opening;
    world.government.procurement_budget_cents = receipts * 80 / 100 * 50 / 100;
    world.government.healthcare_budget_cents = receipts * 80 / 100 * 40 / 100;
    world.government.training_budget_cents = receipts * 80 / 100 * 10 / 100;
}

fn government_weekly_receipts(world: &World, dataset: &Dataset) -> i64 {
    let mut income = 0i64;
    for region in &world.regions {
        for tier in 0..3 {
            let wage = region.labour[tier].prevailing_wage_cents;
            income += region.labour[tier].population * PARTICIPATION_BPS / 10_000 * wage;
        }
    }
    let _ = dataset;
    income * 20 / 100
}

fn seed_named_firms(world: &mut World, dataset: &Dataset) {
    for (index, seed_firm) in dataset.seed_firms.iter().enumerate() {
        let region = world
            .region_index(&seed_firm.region)
            .expect("validated region");
        let controller = HolderId::Outside((index % OUTSIDE_INVESTORS.len()) as u32);
        let industry = industry_index(&seed_firm.industry);
        let is_financial = seed_firm.industry == "financial";

        let mut firm = Firm {
            name: seed_firm.name.clone(),
            industry,
            region,
            archetype: Some(seed_firm.archetype),
            book: Ledger::default(),
            status: FirmStatus::Operating,
            shares: ShareBook::new(BTreeMap::new()),
            board: Board::new(Tick(13)),
            policies: Policies::default(),
            inventory: BTreeMap::new(),
            facilities: Vec::new(),
            loans: Vec::new(),
            bonds: Vec::new(),
            insurance: Vec::new(),
            contracts_buy: Vec::new(),
            contracts_sell: Vec::new(),
            grade: Grade::B,
            past_due_weeks: 0,
            payout_freeze: false,
            organisation: BTreeMap::new(),
            union: UnionState::default(),
            tech_level: 1,
            tech_progress_cents: 0,
            brand_bps: BTreeMap::new(),
            reputation_bps: 0,
            layoffs: Vec::new(),
            staff_history: Vec::new(),
            outside_peak_bps: 0,
            secondary_industry: None,
            focus_transition_until: 0,
            focus_cooldown_until: 0,
            raid_ready: BTreeMap::new(),
            tax_loss_carry_cents: 0,
            logistics_strength: 0,
            policy_requests: Vec::new(),
            guarantees: Vec::new(),
            last_strategic_week: 0,
            merged_into: None,
            founding_week: Tick::OPENING,
            bank: if seed_firm.recipe == "bank" {
                Some(BankModel {
                    deposit_rate_bps: 400,
                    specialist_concentration: false,
                    depositors: BTreeMap::new(),
                })
            } else {
                None
            },
            insurer: if seed_firm.recipe == "property_insurer" {
                Some(InsurerModel {
                    specialist_concentration: false,
                    unearned_reserve_cents: 0,
                })
            } else {
                None
            },
            valuation: Default::default(),
            reserved_cash_cents: 0,
            last_week_units_sold: BTreeMap::new(),
            sales_history: Vec::new(),
            quality_bps: BTreeMap::new(),
            intercompany_inventory: BTreeMap::new(),
            storage: Default::default(),
        };

        if is_financial {
            seed_financial_firm(world, dataset, &mut firm, seed_firm.public, controller);
            continue;
        } else {
            let wage = |tier: usize| {
                dataset.baseline_weekly_wage_cents[tier]
                    * world.regions[region as usize].wage_index_bps as i64
                    / 10_000
            };
            firm.policies.wage_offer = [wage(0), wage(1), wage(2)];
            // The main plant, then any extra opening plants (each in its own region), so goods
            // that would otherwise be import-only start with a local producer.
            let mut plants: Vec<(u8, u16, u8)> = vec![(
                region,
                dataset.recipe_index[&seed_firm.recipe],
                seed_firm.blocks,
            )];
            for extra in &seed_firm.extra_plants {
                plants.push((
                    world
                        .region_index(&extra.region)
                        .expect("validated extra plant region"),
                    dataset.recipe_index[&extra.recipe],
                    extra.blocks,
                ));
            }
            let facilities: Vec<Facility> = plants
                .iter()
                .map(|(plant_region, recipe_index, blocks)| {
                    open_facility(
                        &mut firm,
                        dataset,
                        *plant_region,
                        *recipe_index,
                        *blocks,
                        Tick::OPENING,
                    )
                })
                .collect();
            let handle = world.firms.insert(firm);
            // The firm must own its facilities: payroll, rent, maintenance and depreciation are
            // all posted per owned facility, so an unowned facility would be free capacity.
            for facility in facilities {
                let facility_handle = world
                    .facilities
                    .insert(facility_with_firm(facility, handle));
                world
                    .firms
                    .get_mut(handle)
                    .unwrap()
                    .facilities
                    .push(facility_handle);
            }
            let mut cash = 0i64;
            let mut inventory_cost = 0i64;
            let mut asset_cost = 0i64;
            for (plant_region, recipe_index, blocks) in &plants {
                let regional_wage = |tier: usize| {
                    dataset.baseline_weekly_wage_cents[tier]
                        * world.regions[*plant_region as usize].wage_index_bps as i64
                        / 10_000
                };
                cash += opening_cash(
                    dataset,
                    *recipe_index,
                    *blocks,
                    &[regional_wage(0), regional_wage(1), regional_wage(2)],
                ) * seed_firm.cash_pct as i64
                    / 100;
                inventory_cost += opening_input_cost(dataset, *recipe_index, *blocks);
                asset_cost += dataset.recipes[*recipe_index as usize].capex_cents * *blocks as i64;
            }
            let loan = asset_cost * 30 / 100;
            let capital = asset_cost + inventory_cost + cash - loan;
            world
                .firms
                .get_mut(handle)
                .unwrap()
                .policies
                .production_target_bps = 10_000;
            let _ = world.firms.get_mut(handle).unwrap().book.commit(
                Tick::OPENING,
                Txn::new()
                    .debit(
                        cw_core::ledger::Account::FixedAssets,
                        asset_cost,
                        "opening assets",
                    )
                    .debit(
                        cw_core::ledger::Account::Inventory,
                        inventory_cost,
                        "opening stock",
                    )
                    .debit(cw_core::ledger::Account::Cash, cash, "opening cash")
                    .credit(cw_core::ledger::Account::Loans, loan, "opening loan")
                    .credit(
                        cw_core::ledger::Account::ShareCapital,
                        capital,
                        "opening capital",
                    ),
            );
            if loan > 0 {
                let loan_handle = world.loans.insert(Loan {
                    borrower: handle,
                    lender: Lender::Outside(0),
                    principal_remaining_cents: loan,
                    rate_bps: BASE_POLICY_RATE_BPS + Grade::B.spread_bps(),
                    weekly_principal_cents: loan / 104,
                    remaining_weeks: 104,
                    collateral: None,
                    secured: false,
                    created: Tick::OPENING,
                    carry: 0,
                    defaulted: false,
                    arrears_cents: 0,
                });
                world.firms.get_mut(handle).unwrap().loans.push(loan_handle);
                if let Some(lender) = world.outside.lenders.get_mut(0) {
                    lender.capital_cents -= loan;
                }
            }
            let shares = world.firms.get_mut(handle).unwrap();
            allocate_initial_shares(shares, seed_firm.public, controller);
        }
    }
}

fn facility_with_firm(mut facility: Facility, firm: Handle) -> Facility {
    facility.firm = firm;
    facility
}

fn seed_financial_firm(
    world: &mut World,
    dataset: &Dataset,
    firm: &mut Firm,
    public: bool,
    controller: HolderId,
) {
    let region = firm.region;
    let wage = |tier: usize| {
        dataset.baseline_weekly_wage_cents[tier]
            * world.regions[region as usize].wage_index_bps as i64
            / 10_000
    };
    firm.policies.wage_offer = [wage(0), wage(1), wage(2)];
    let handle = world
        .firms
        .insert(std::mem::replace(firm, placeholder_firm()));
    let capital = 400_000_000;
    let cash = 250_000_000;
    let _ = world.firms.get_mut(handle).unwrap().book.commit(
        Tick::OPENING,
        Txn::new()
            .debit(cw_core::ledger::Account::Cash, cash, "bank capital")
            .debit(
                cw_core::ledger::Account::OtherAssets,
                capital - cash,
                "bank premises",
            )
            .credit(
                cw_core::ledger::Account::ShareCapital,
                capital,
                "bank capital",
            ),
    );
    allocate_initial_shares(world.firms.get_mut(handle).unwrap(), public, controller);
}

fn placeholder_firm() -> Firm {
    Firm {
        name: String::new(),
        industry: 0,
        region: 0,
        archetype: None,
        book: Ledger::default(),
        status: FirmStatus::Operating,
        shares: ShareBook::new(BTreeMap::new()),
        board: Board::new(Tick(13)),
        policies: Policies::default(),
        inventory: BTreeMap::new(),
        facilities: Vec::new(),
        loans: Vec::new(),
        bonds: Vec::new(),
        insurance: Vec::new(),
        contracts_buy: Vec::new(),
        contracts_sell: Vec::new(),
        grade: Grade::B,
        past_due_weeks: 0,
        payout_freeze: false,
        organisation: BTreeMap::new(),
        union: UnionState::default(),
        tech_level: 1,
        tech_progress_cents: 0,
        brand_bps: BTreeMap::new(),
        reputation_bps: 0,
        layoffs: Vec::new(),
        staff_history: Vec::new(),
        outside_peak_bps: 0,
        secondary_industry: None,
        focus_transition_until: 0,
        focus_cooldown_until: 0,
        raid_ready: BTreeMap::new(),
        tax_loss_carry_cents: 0,
        logistics_strength: 0,
        policy_requests: Vec::new(),
        guarantees: Vec::new(),
        last_strategic_week: 0,
        merged_into: None,
        founding_week: Tick::OPENING,
        bank: None,
        insurer: None,
        valuation: Default::default(),
        reserved_cash_cents: 0,
        last_week_units_sold: BTreeMap::new(),
        sales_history: Vec::new(),
        quality_bps: BTreeMap::new(),
        intercompany_inventory: BTreeMap::new(),
        storage: Default::default(),
    }
}

fn allocate_initial_shares(firm: &mut Firm, public: bool, controller: HolderId) {
    let mut holders = BTreeMap::new();
    let founder_qty = if public {
        INITIAL_SHARES * 35 / 100
    } else {
        INITIAL_SHARES * 60 / 100
    };
    holders.insert(
        controller,
        ShareLot {
            qty: founder_qty,
            cost_cents: founder_qty * 250,
        },
    );
    let remaining = INITIAL_SHARES - founder_qty;
    if public {
        // Public float spread across named institutions, never one hidden pool.
        let institutions = [1u32, 2, 3, 4];
        let per = remaining / institutions.len() as i64;
        for (index, id) in institutions.iter().enumerate() {
            let qty = if index == institutions.len() - 1 {
                remaining - per * (institutions.len() as i64 - 1)
            } else {
                per
            };
            holders.insert(
                HolderId::Outside(*id),
                ShareLot {
                    qty,
                    cost_cents: qty * 250,
                },
            );
        }
        firm.shares.ipo_done = true;
        firm.shares.statement_weeks = 52;
    } else {
        holders.insert(
            HolderId::Outside(7),
            ShareLot {
                qty: remaining,
                cost_cents: remaining * 250,
            },
        );
    }
    firm.shares.issued = INITIAL_SHARES;
    firm.shares.holders = holders;
    firm.board.seats = crate::economy::governance::apportion_seats(&firm.shares);
}

fn open_facility(
    firm: &mut Firm,
    dataset: &Dataset,
    region: u8,
    recipe_index: u16,
    blocks: u8,
    now: Tick,
) -> Facility {
    let recipe = &dataset.recipes[recipe_index as usize];
    let mut facility = Facility {
        firm: Handle::INVALID,
        region,
        recipe: recipe_index,
        blocks,
        status: FacilityStatus::Operating,
        started: now,
        completes: now,
        escrow_cents: 0,
        capex_cents: recipe.capex_cents * blocks as i64,
        accumulated_depreciation: 0,
        workers: [
            recipe.workers_low_mid_high[0] * blocks as i64,
            recipe.workers_low_mid_high[1] * blocks as i64,
            recipe.workers_low_mid_high[2] * blocks as i64,
        ],
        target_bps: 10_000,
        quality_bps: 10_000,
        deposit_remaining: MilliLots::ZERO,
        last_output: MilliLots::ZERO,
        last_inputs_short: false,
        last_staff_ratio_bps: 10_000,
        mothballed_since: None,
        equipment_due_cents: 0,
        outage: None,
        stock_plan: Default::default(),
    };
    if recipe.depletes() {
        facility.deposit_remaining =
            MilliLots::from_lots(recipe.weekly_output_lots * blocks as i64 * 52 * 20);
    }
    for (product_id, weekly) in &recipe.weekly_inputs_lots {
        let product = dataset.product_index[product_id];
        // Services expire weekly, so opening stock in a service is unusable value that would be
        // written off in week one. Only physical inputs are carried into the opening balance.
        if dataset.products[product as usize].kind.is_service() {
            continue;
        }
        let lot = firm.stock_lot_mut(product, region);
        lot.receive(
            MilliLots::from_lots(weekly * blocks as i64 * 2),
            Cents(weekly * blocks as i64 * 2 * dataset.reference_price(product)),
        );
    }
    facility
}

fn opening_cash(dataset: &Dataset, recipe_index: u16, blocks: u8, wages: &[i64; 3]) -> i64 {
    let recipe = &dataset.recipes[recipe_index as usize];
    let payroll: i64 = recipe
        .workers_low_mid_high
        .iter()
        .zip(wages.iter())
        .map(|(workers, wage)| workers * blocks as i64 * wage)
        .sum();
    let inputs: i64 = recipe
        .weekly_inputs_lots
        .iter()
        .map(|(id, qty)| qty * blocks as i64 * dataset.reference_price(dataset.product_index[id]))
        .sum();
    let maintenance = recipe.weekly_maintenance(dataset.maintenance_annual_bps) * blocks as i64;
    let rent = dataset.fixture_land_rent_cents_weekly * blocks as i64;
    (payroll + inputs + maintenance + rent) * 13
}

fn opening_input_cost(dataset: &Dataset, recipe_index: u16, blocks: u8) -> i64 {
    let recipe = &dataset.recipes[recipe_index as usize];
    recipe
        .weekly_inputs_lots
        .iter()
        .filter(|(id, _)| {
            let code = dataset.product_index[*id];
            !dataset.products[code as usize].kind.is_service()
        })
        .map(|(id, qty)| {
            qty * blocks as i64 * 2 * dataset.reference_price(dataset.product_index[id])
        })
        .sum()
}

fn seed_player_firm(world: &mut World, dataset: &Dataset, seed: u64) {
    let region = world.region_index("OH-CEN").unwrap_or(0);
    let recipe_index = dataset.recipe_index["farm"];
    let mut firm = placeholder_firm();
    firm.name = "Lakeside Holdings".to_owned();
    firm.industry = industry_index("agriculture");
    firm.region = region;
    firm.policies.wage_offer = [
        dataset.baseline_weekly_wage_cents[0]
            * world.regions[region as usize].wage_index_bps as i64
            / 10_000,
        dataset.baseline_weekly_wage_cents[1]
            * world.regions[region as usize].wage_index_bps as i64
            / 10_000,
        dataset.baseline_weekly_wage_cents[2]
            * world.regions[region as usize].wage_index_bps as i64
            / 10_000,
    ];
    let handle = world.firms.insert(firm);
    // The opening farm: five plants of the traditional strategy, the 1.1.0 farm's capacity.
    const PLAYER_FARM_BLOCKS: u8 = 5;
    let asset = dataset.recipes[recipe_index as usize].capex_cents * PLAYER_FARM_BLOCKS as i64;
    let mut facility = open_facility(
        world.firms.get_mut(handle).unwrap(),
        dataset,
        region,
        recipe_index,
        PLAYER_FARM_BLOCKS,
        Tick::OPENING,
    );
    facility.capex_cents = asset;
    facility.target_bps = 10_000;
    let facility_handle = world
        .facilities
        .insert(facility_with_firm(facility, handle));
    world
        .firms
        .get_mut(handle)
        .unwrap()
        .facilities
        .push(facility_handle);
    let inventory_cost = opening_input_cost(dataset, recipe_index, PLAYER_FARM_BLOCKS);
    let cash = 100_000_000;
    let loan = 50_000_000;
    let capital = asset + inventory_cost + cash - loan;
    let _ = world.firms.get_mut(handle).unwrap().book.commit(
        Tick::OPENING,
        Txn::new()
            .debit(
                cw_core::ledger::Account::FixedAssets,
                asset,
                "opening assets",
            )
            .debit(
                cw_core::ledger::Account::Inventory,
                inventory_cost,
                "opening stock",
            )
            .debit(cw_core::ledger::Account::Cash, cash, "opening cash")
            .credit(cw_core::ledger::Account::Loans, loan, "opening loan")
            .credit(
                cw_core::ledger::Account::ShareCapital,
                capital,
                "opening capital",
            ),
    );
    let loan_handle = world.loans.insert(Loan {
        borrower: handle,
        lender: Lender::Outside(1),
        principal_remaining_cents: loan,
        rate_bps: BASE_POLICY_RATE_BPS + Grade::B.spread_bps(),
        weekly_principal_cents: loan / 104,
        remaining_weeks: 104,
        collateral: None,
        secured: false,
        created: Tick::OPENING,
        carry: 0,
        defaulted: false,
        arrears_cents: 0,
    });
    world.firms.get_mut(handle).unwrap().loans.push(loan_handle);

    // Investor subscribes 60 percent of opening equity, outside investors 40 percent.
    let investor_cents = capital * 60 / 100;
    let outside_cents = capital - investor_cents;
    world.investor.cash_cents -= investor_cents;
    let _ = world.investor.book.commit(
        Tick::OPENING,
        Txn::new()
            .credit(
                cw_core::ledger::Account::Cash,
                investor_cents,
                "subscription",
            )
            .debit(
                cw_core::ledger::Account::Investments,
                investor_cents,
                "subscription",
            ),
    );
    let investor_shares = INITIAL_SHARES * 60 / 100;
    let mut holders = BTreeMap::new();
    holders.insert(
        HolderId::Investor,
        ShareLot {
            qty: investor_shares,
            cost_cents: investor_cents,
        },
    );
    holders.insert(
        HolderId::Outside(4),
        ShareLot {
            qty: INITIAL_SHARES - investor_shares,
            cost_cents: outside_cents,
        },
    );
    let firm = world.firms.get_mut(handle).unwrap();
    firm.shares.issued = INITIAL_SHARES;
    firm.shares.holders = holders;
    firm.board.seats = crate::economy::governance::apportion_seats(&firm.shares);
    firm.shares.statement_weeks = 0;
    firm.board.ceo = Some(CeoRef::Investor);
    world.investor.primary_group = Some(handle);
    let _ = seed;
}

fn refresh_region_incomes(world: &mut World, dataset: &Dataset) {
    let _ = dataset;
    let mut named_jobs = vec![[0i64; 3]; world.regions.len()];
    let mut named_payroll = vec![0i64; world.regions.len()];
    for (_, facility) in world.facilities.iter() {
        let firm = match world.firms.get(facility.firm) {
            Some(firm) => firm,
            None => continue,
        };
        let region = facility.region as usize;
        for (tier, workers) in facility.workers.iter().enumerate() {
            named_jobs[region][tier] += workers;
            named_payroll[region] += workers * firm.policies.wage_offer[tier];
        }
    }
    for index in 0..world.regions.len() {
        let mut background = [0i64; 3];
        let mut background_income = 0i64;
        for (tier, tier_state) in world.regions[index].labour.iter().enumerate() {
            let available = tier_state.population * PARTICIPATION_BPS / 10_000;
            background[tier] = (available - named_jobs[index][tier]).max(0);
            background_income +=
                background[tier] * world.regions[index].background_wage_cents[tier];
        }
        let total_income = background_income + named_payroll[index];
        let region = &mut world.regions[index];
        region.background_employed = background;
        region.households.last_income_cents = total_income;
        if region.households.savings_cents == 0 {
            region.households.savings_cents = total_income * 90 / 100 * HOUSEHOLD_SAVINGS_WEEKS;
        }
    }
}

fn compute_household_baselines(world: &mut World, dataset: &Dataset) {
    let consumer_ids = [
        "food_retail",
        "goods_retail",
        "care",
        "software",
        "telecom",
        "lease",
    ];
    let codes: Vec<u16> = consumer_ids
        .iter()
        .map(|id| dataset.product_index[*id])
        .collect();
    for region in &mut world.regions {
        // The opening baseline is disposable income: labour income pays twenty percent tax.
        let disposable = region.households.last_income_cents * 80 / 100;
        region.households.baseline_income_cents = disposable;
        region.households.savings_cents = disposable * 90 / 100 * 13;
        let budget = disposable * 90 / 100;
        let mut weights = BTreeMap::new();
        let mut baseline = BTreeMap::new();
        for (index, code) in codes.iter().enumerate() {
            let weight = HOUSEHOLD_WEIGHTS[index];
            weights.insert(*code, weight);
            let category_budget = budget * weight / 10_000;
            let price = dataset.reference_price(*code).max(1);
            baseline.insert(*code, (category_budget / price).max(1));
        }
        region.households.weights = weights;
        region.households.baseline_lots = baseline;
    }
}

fn build_gateways(world: &mut World, dataset: &Dataset) {
    // Every physical good is tradable through the gateway, and electricity can be imported as
    // a finite local service. Financial contracts and other services stay out of this market.
    let mut products: Vec<&str> = dataset
        .products
        .iter()
        .filter(|product| product.kind == cw_data::ProductKind::Goods)
        .map(|product| product.id.as_str())
        .collect();
    products.push("electricity");
    // Size each gateway to the larger of what the corridor's plants consume and what they
    // produce: raw commodities and goods trade nationally, so a state's mines and mills can
    // always reach the outside market at the gateway's discount, and its plants can always
    // import at its premium.
    let mut corridor_volume: BTreeMap<u16, i64> = BTreeMap::new();
    let mut corridor_output: BTreeMap<u16, i64> = BTreeMap::new();
    for (_, facility) in world.facilities.iter() {
        let recipe = &dataset.recipes[facility.recipe as usize];
        for (id, qty) in &recipe.weekly_inputs_lots {
            let code = dataset.product_index[id.as_str()];
            *corridor_volume.entry(code).or_default() += qty * facility.blocks as i64;
        }
        for (id, lots) in recipe.outputs() {
            if let Some(code) = dataset.product_index.get(id) {
                *corridor_output.entry(*code).or_default() += lots * facility.blocks as i64;
            }
        }
    }
    for (code, output) in corridor_output {
        let entry = corridor_volume.entry(code).or_default();
        *entry = (*entry).max(output);
    }
    // What each region's own plants consume or produce: its gateway must always carry that.
    let mut regional_volume: BTreeMap<(u8, u16), i64> = BTreeMap::new();
    for (_, facility) in world.facilities.iter() {
        let recipe = &dataset.recipes[facility.recipe as usize];
        for (id, qty) in &recipe.weekly_inputs_lots {
            let code = dataset.product_index[id.as_str()];
            *regional_volume.entry((facility.region, code)).or_default() +=
                qty * facility.blocks as i64;
        }
        for (id, lots) in recipe.outputs() {
            if let Some(code) = dataset.product_index.get(id) {
                let entry = regional_volume.entry((facility.region, *code)).or_default();
                *entry = (*entry).max(lots * facility.blocks as i64);
            }
        }
    }
    // The outside market carries one corridor volume per state, as in 1.1.0 when a region was
    // a state; each state's regions split it by demand, and never below their own plants.
    let total_demand: i64 = world
        .regions
        .iter()
        .map(|region| region.demand_index_bps as i64)
        .sum::<i64>()
        .max(1);
    let states = dataset.states.len().max(1) as i64;
    for region in 0..world.regions.len() {
        let share = world.regions[region].demand_index_bps as i64;
        for id in &products {
            let code = dataset.product_index[*id];
            let corridor = corridor_volume.get(&code).copied().unwrap_or(100).max(100);
            let own = regional_volume
                .get(&(region as u8, code))
                .copied()
                .unwrap_or(0);
            let volume = (corridor * states * share / total_demand).max(own);
            let reference = dataset.reference_price(code);
            // Electricity is a bulk local service: the gateway's finite capacity must be able
            // to absorb a generator block's weekly surplus, or unsold power expires and the
            // utility cannot cover its fuel and payroll.
            // The outside market is large relative to the one-percent corridor: size the finite
            // gateway to the corridor's own weekly volume so a shortage is filled and a surplus
            // has a buyer. The spread still makes the gateway a last resort, not free money.
            let capacity = if id == &"electricity" {
                volume.max(100)
            } else {
                volume.max(40)
            };
            world.gateways.push(Gateway {
                region: region as u8,
                product: code,
                capacity_lots: capacity,
                sell_price_cents: reference + reference * 15 / 100,
                buy_price_cents: reference - reference * 15 / 100,
                last_import_lots: 0,
                last_export_lots: 0,
                smoothed_import_lots: 0,
            });
        }
    }
}

/// Established inter-region supply relationships. Without these the corridor has no trade
/// beyond its own region, so vertical chains never meet and every producer starves.
fn seed_supply_contracts(world: &mut World, dataset: &Dataset) {
    let mut suppliers: BTreeMap<u16, Vec<Handle>> = BTreeMap::new();
    let mut supplier_region: BTreeMap<(Handle, u16), (i64, u8)> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.operative() {
            continue;
        }
        for facility_handle in &firm.facilities {
            let Some(facility) = world.facilities.get(*facility_handle) else {
                continue;
            };
            let recipe = &dataset.recipes[facility.recipe as usize];
            for (id, lots) in recipe.outputs() {
                if let Some(code) = dataset.product_index.get(id) {
                    let list = suppliers.entry(*code).or_default();
                    if !list.contains(&handle) {
                        list.push(handle);
                    }
                    // Goods sit where the plant made them, which is not always the firm's
                    // headquarters: deliver from the region of its largest plant for the good.
                    let weight = lots * facility.blocks as i64;
                    let best = supplier_region
                        .entry((handle, *code))
                        .or_insert((0, facility.region));
                    if weight > best.0 {
                        *best = (weight, facility.region);
                    }
                }
            }
        }
    }
    type BuyerNeeds = Vec<(Handle, u8, Vec<(u16, i64)>)>;
    let buyers: BuyerNeeds = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative())
        .map(|(handle, firm)| {
            let mut needs: BTreeMap<u16, i64> = BTreeMap::new();
            for facility_handle in &firm.facilities {
                let Some(facility) = world.facilities.get(*facility_handle) else {
                    continue;
                };
                let recipe = &dataset.recipes[facility.recipe as usize];
                for (id, qty) in &recipe.weekly_inputs_lots {
                    if id == "electricity" {
                        continue;
                    }
                    if let Some(code) = dataset.product_index.get(id) {
                        *needs.entry(*code).or_default() += qty * facility.blocks as i64;
                    }
                }
            }
            (handle, firm.region, needs.into_iter().collect())
        })
        .collect();
    for (buyer, buyer_region, needs) in buyers {
        for (product, qty) in needs {
            let Some(candidates) = suppliers.get(&product) else {
                continue;
            };
            // Split the buyer's weekly need across every eligible supplier, so no producer is
            // left without a route to market just because another firm was listed first.
            let eligible: Vec<Handle> = candidates
                .iter()
                .copied()
                .filter(|handle| *handle != buyer)
                .collect();
            if eligible.is_empty() || qty <= 0 {
                continue;
            }
            // Split by supplier capacity, not equally: a one-block farm cannot honour the same
            // weekly volume as a three-block one, and over-committing it caused penalties.
            let mut capacities: Vec<(Handle, i64)> = eligible
                .iter()
                .map(|seller| {
                    let capacity: i64 = world
                        .firms
                        .get(*seller)
                        .map(|firm| {
                            firm.facilities
                                .iter()
                                .filter_map(|handle| world.facilities.get(*handle))
                                .map(|facility| {
                                    let recipe = &dataset.recipes[facility.recipe as usize];
                                    recipe.lots_of(&dataset.products[product as usize].id)
                                        * facility.blocks as i64
                                })
                                .sum::<i64>()
                        })
                        .unwrap_or(0);
                    (*seller, capacity.max(1))
                })
                .collect();
            capacities.sort_by_key(|(handle, _)| *handle);
            let total_capacity: i64 = capacities.iter().map(|(_, capacity)| *capacity).sum();
            for (seller, capacity) in capacities {
                let share = (qty * capacity / total_capacity.max(1)).max(1);
                let seller_region = supplier_region
                    .get(&(seller, product))
                    .map(|(_, region)| *region)
                    .or_else(|| world.firms.get(seller).map(|firm| firm.region))
                    .unwrap_or(buyer_region);
                let contract = crate::state::Contract {
                    buyer,
                    seller,
                    product,
                    from_region: seller_region,
                    to_region: buyer_region,
                    quantity_weekly: MilliLots::from_lots(share),
                    price_cents: dataset.reference_price(product),
                    penalty_bps: 500,
                    escrow_cents: 0,
                    remaining_weeks: 52,
                    next_delivery: Tick(1),
                    status: crate::state::ContractStatus::Active,
                    shortfalls: MilliLots::ZERO,
                    auto_renew: true,
                };
                let handle = world.contracts.insert(contract);
                if let Some(entity) = world.firms.get_mut(buyer) {
                    entity.contracts_buy.push(handle);
                }
                if let Some(entity) = world.firms.get_mut(seller) {
                    entity.contracts_sell.push(handle);
                }
            }
        }
    }
}

/// A year of a firm's plants' standard EBITDA: output at reference less inputs at reference,
/// baseline payroll, maintenance and land.
fn expected_annual_ebitda(world: &World, dataset: &Dataset, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let weekly: i64 = entity
        .facilities
        .iter()
        .filter_map(|plant| world.facilities.get(*plant))
        .map(|plant| {
            let recipe = &dataset.recipes[plant.recipe as usize];
            let revenue = recipe.output_value(|id| {
                dataset
                    .product_index
                    .get(id)
                    .map(|code| dataset.reference_price(*code))
                    .unwrap_or(0)
            });
            let inputs: i64 = recipe
                .weekly_inputs_lots
                .iter()
                .filter_map(|(id, qty)| {
                    dataset
                        .product_index
                        .get(id)
                        .map(|code| qty * dataset.reference_price(*code))
                })
                .sum();
            let payroll = recipe.baseline_weekly_payroll(dataset.baseline_weekly_wage_cents);
            let fixed = recipe.weekly_maintenance(dataset.maintenance_annual_bps)
                + dataset.fixture_land_rent_cents_weekly;
            (revenue - inputs - payroll - fixed) * plant.blocks as i64
        })
        .sum();
    weekly * 52
}

fn seed_share_marks(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let financial = world
            .firms
            .get(handle)
            .map(|f| f.bank.is_some() || f.insurer.is_some())
            .unwrap_or(false);
        let value = crate::economy::valuation::base_value(
            &world.firms.get(handle).unwrap().book,
            Tick::OPENING,
            financial,
        );
        // No week has closed yet, so the opening valuation reads its plants' standard earnings
        // at reference prices and baseline wages instead of the book floor alone; otherwise
        // every share would re-rate in step over the first quarter as real earnings arrive.
        let mut value = value;
        if !financial {
            let expected = expected_annual_ebitda(world, dataset, handle);
            let enterprise = value.enterprise_cents.max(expected.max(0) * 6 / 2);
            value.equity_cents = (value.equity_cents - value.enterprise_cents + enterprise).max(0);
            value.enterprise_cents = enterprise;
        }
        let shares = &mut world.firms.get_mut(handle).unwrap().shares;
        let issued = shares.eligible_votes().max(1);
        shares.fair_cents = value.equity_cents / issued;
        shares.mark_cents = shares.fair_cents;
        shares.last_trade_cents = shares.fair_cents;
        world.firms.get_mut(handle).unwrap().valuation = value;
    }
    let _ = dataset;
}

/// Personal net worth: cash + direct holdings at fair value - personal liabilities.
pub fn investor_net_worth(world: &World, dataset: &Dataset) -> i64 {
    let mut worth = world.investor.cash_cents - world.investor.personal_loans_cents;
    for (handle, firm) in world.firms.iter() {
        let qty = firm.shares.qty(HolderId::Investor);
        if qty > 0 {
            worth += qty * firm.shares.fair_cents;
        }
        let _ = handle;
    }
    let _ = dataset;
    worth
}

/// Basis points helper used across seeding and market pricing.
pub fn bps_of(value: i64, bps: i32) -> i64 {
    Bps(bps).of(Cents(value)).map(|cents| cents.0).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn founder_and_guided_worlds_open_balanced() {
        let dataset = Dataset::load().unwrap();
        for scenario in [ScenarioKind::Founder, ScenarioKind::Guided] {
            let world = build(&dataset, 7, scenario, ScenarioMode::FiveYear);
            assert_eq!(
                world.firms.len(),
                if scenario == ScenarioKind::Guided {
                    31
                } else {
                    30
                }
            );
            for (_, firm) in world.firms.iter() {
                assert!(firm.book.is_balanced(), "{} not balanced", firm.name);
                assert_eq!(
                    firm.book.total_assets()
                        - firm.book.total_liabilities()
                        - firm.book.total_equity(),
                    0,
                    "{} balance sheet identity",
                    firm.name
                );
                assert!(firm.book.cash() > 0, "{} has cash", firm.name);
            }
            for region in &world.regions {
                // Every scaled worker is accounted for by named or background employment.
                let population: i64 = region.labour.iter().map(|tier| tier.population).sum();
                assert!(population > 0);
            }
        }
    }

    #[test]
    fn seeded_operating_firms_start_fully_staffed() {
        let dataset = Dataset::load().unwrap();
        let world = build(&dataset, 12, ScenarioKind::Founder, ScenarioMode::Sandbox);
        for (_, firm) in world.firms.iter() {
            for handle in &firm.facilities {
                let facility = world.facilities.get(*handle).unwrap();
                assert_eq!(
                    facility.workers,
                    facility.full_workers(&dataset),
                    "{} starts staffed",
                    firm.name
                );
            }
        }
    }

    #[test]
    fn guided_investor_owns_sixty_percent_and_firm_has_no_magic_cash() {
        let dataset = Dataset::load().unwrap();
        let world = build(&dataset, 11, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.expect("guided primary firm");
        let firm = world.firm(primary).unwrap();
        let votes = firm.shares.vote_bps(HolderId::Investor);
        assert!((5_900..=6_100).contains(&votes), "votes {votes}");
        assert_eq!(
            firm.book.balance(cw_core::ledger::Account::Cash),
            100_000_000
        );
        assert!(world.investor.cash_cents > 0);
    }
}
