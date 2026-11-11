//! Capacity as the contest (docs/rules/CAPACITY.md).
//!
//! One room-to-build signal, read the same way by every rival and by the player's screens: for a
//! strategy in a state, how much of what it makes the market is short of, what a block of it
//! would earn at this week's prices and wages, what it costs to build today, and how fast it pays
//! back. A traded good's room is the corridor's imports, which a home plant replaces at the
//! import price less freight. A service's or retail good's room is the household demand that
//! went without in the regions a plant there would serve. Plants on order, or opened too
//! recently to show in the smoothed figures, are taken off the room, so a gap draws one round of
//! builders, not every firm that reads it in the same quarter.

use cw_data::{Dataset, ProductKind};

use crate::state::{FacilityStatus, World};

/// Weeks after a plant opens that the smoothed import signal still understates what it supplies.
pub const ROOM_LAG_WEEKS: u32 = 8;

/// Weeks of contribution a build is judged on before any cost of size.
pub const HORIZON_WEEKS: i64 = 156;

/// The shortest horizon a large company's build is judged on.
pub const MIN_HORIZON_WEEKS: i64 = 52;

/// Whether a product clears on the corridor exchange (goods) rather than where it is made.
pub fn corridor(dataset: &Dataset, product: u16) -> bool {
    dataset.products[product as usize].kind == ProductKind::Goods
}

/// Whether households buy the product, so their unmet demand is room to build.
pub fn household_good(dataset: &Dataset, product: u16) -> bool {
    let kind = dataset.products[product as usize].kind;
    kind.is_consumer_goods()
        || matches!(
            kind,
            ProductKind::LocalService | ProductKind::Subscription | ProductKind::Service
        )
}

/// The regions a plant of a local product in `region` sells into.
fn served(dataset: &Dataset, product: u16, region: u8) -> Vec<u8> {
    if !dataset.capacity.local_room {
        return vec![region];
    }
    crate::economy::markets::supply_regions(dataset, region, product)
}

/// Weekly lots of `product` from plants on order, or opened within `ROOM_LAG_WEEKS`, in
/// `regions` (every region when None).
pub fn coming_lots(world: &World, dataset: &Dataset, product: u16, regions: Option<&[u8]>) -> i64 {
    let id = &dataset.products[product as usize].id;
    world
        .facilities
        .iter()
        .map(|(_, plant)| plant)
        .filter(|plant| regions.is_none_or(|list| list.contains(&plant.region)))
        .filter(|plant| match plant.status {
            FacilityStatus::Building => true,
            FacilityStatus::Operating => plant.completes.0 + ROOM_LAG_WEEKS > world.tick.0,
            _ => false,
        })
        .filter(|plant| &dataset.recipes[plant.recipe as usize].output == id)
        .map(|plant| {
            dataset.recipes[plant.recipe as usize].weekly_output_lots * plant.blocks as i64
        })
        .sum()
}

/// Households' unmet and wanted lots of a product last week in `regions`.
pub fn unmet_in(world: &World, product: u16, regions: &[u8]) -> (i64, i64) {
    let (mut unmet, mut wanted) = (0i64, 0i64);
    for region in regions {
        if let Some(households) = world.regions.get(*region as usize).map(|r| &r.households) {
            unmet =
                unmet.saturating_add(households.unmet_milli.get(&product).copied().unwrap_or(0));
            wanted =
                wanted.saturating_add(households.wanted_milli.get(&product).copied().unwrap_or(0));
        }
    }
    (unmet / 1_000, wanted / 1_000)
}

/// Room to build, in lots a week. A good trades on the corridor exchange, so any region's plant
/// replaces any region's imports and its room is the whole corridor's smoothed imports. A local
/// product's room is its imports into the regions a plant here would serve plus, for what
/// households buy, the lots they went without there. Plants coming are taken off either way.
pub fn room_lots(world: &World, dataset: &Dataset, product: u16, region: u8) -> i64 {
    if corridor(dataset, product) {
        let imports: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == product)
            .map(|g| g.smoothed_import_lots)
            .sum();
        return (imports - coming_lots(world, dataset, product, None)).max(0);
    }
    let regions = served(dataset, product, region);
    let imports: i64 = world
        .gateways
        .iter()
        .filter(|g| g.product == product && regions.contains(&g.region))
        .map(|g| g.smoothed_import_lots)
        .sum();
    let unmet = if dataset.capacity.local_room && household_good(dataset, product) {
        unmet_in(world, product, &regions).0
    } else {
        0
    };
    (imports + unmet - coming_lots(world, dataset, product, Some(&regions))).max(0)
}

/// Public price of a product in a state: the gateway import price where one quotes; for a
/// local product, last week's closing corridor price when the capacity rules are on; else the
/// reference price. No firm sees another firm's costs or order book.
pub fn public_price(world: &World, dataset: &Dataset, product: u16, region: u8) -> i64 {
    if let Some(quote) = world
        .gateways
        .iter()
        .find(|g| g.region == region && g.product == product && g.sell_price_cents > 0)
        .map(|g| g.sell_price_cents)
    {
        return quote;
    }
    if !corridor(dataset, product)
        && dataset.capacity.local_room
        && let Some(price) = world
            .price_history
            .last()
            .and_then(|point| point.goods.get(&product))
    {
        return (*price).max(1);
    }
    dataset.reference_price(product)
}

/// What a plant in `region` nets per lot of a good by replacing imports: the best importing
/// gateway's price less freight from the plant. None when nothing of the good is imported.
pub fn import_netback(world: &World, dataset: &Dataset, product: u16, region: u8) -> Option<i64> {
    world
        .gateways
        .iter()
        .filter(|g| g.product == product && g.smoothed_import_lots > 0 && g.sell_price_cents > 0)
        .map(|g| {
            g.sell_price_cents
                - crate::economy::freight::freight_cost(
                    dataset,
                    region,
                    g.region,
                    cw_core::quantity::MilliLots::from_lots(100),
                ) / 100
        })
        .max()
}

/// Weekly contribution of one block of a strategy in `region`, run from a head office in `home`,
/// at public prices and the region's wages, after maintenance and land.
pub fn block_contribution(
    world: &World,
    dataset: &Dataset,
    home: u8,
    recipe: u16,
    region: u8,
) -> i64 {
    let (revenue, inputs, fixed) = block_parts(world, dataset, home, recipe, region);
    revenue - inputs - fixed
}

/// One block's weekly revenue and inputs at full output, and its fixed costs (full payroll,
/// maintenance and land), at public prices and the region's wages.
pub fn block_parts(
    world: &World,
    dataset: &Dataset,
    home: u8,
    recipe: u16,
    region: u8,
) -> (i64, i64, i64) {
    let template = &dataset.recipes[recipe as usize];
    if !dataset.product_index.contains_key(&template.output) {
        return (0, 0, 0);
    }
    let revenue: i64 = template
        .outputs()
        .filter_map(|(id, lots)| {
            dataset
                .product_index
                .get(id)
                .map(|product| lots * public_price(world, dataset, *product, region))
        })
        .sum();
    let location =
        crate::economy::sectors::location_factor_bps(dataset, home, region, &template.industry)
            as i64;
    let inputs: i64 = template
        .weekly_inputs_lots
        .iter()
        .filter_map(|(id, qty)| {
            dataset
                .product_index
                .get(id)
                .map(|p| qty * public_price(world, dataset, *p, region))
        })
        .sum::<i64>()
        * location
        / 10_000;
    let wage_index = world.regions[region as usize].wage_index_bps as i64;
    let payroll: i64 = (0..3)
        .map(|tier| {
            template.workers_low_mid_high[tier]
                * dataset.baseline_weekly_wage_cents[tier]
                * wage_index
                / 10_000
        })
        .sum();
    let maintenance = template.weekly_maintenance(dataset.maintenance_annual_bps);
    let land = dataset.fixture_land_rent_cents_weekly
        * world.regions[region as usize].land_index_bps as i64
        / 10_000;
    (revenue, inputs, payroll + maintenance + land)
}

/// What an operating plant earns a week at its last week's output, this week's public prices and
/// its region's wages: the variable margin on what it made, less its fixed costs. A plant that
/// makes too little to cover its staff and upkeep reads negative.
pub fn plant_weekly(
    world: &World,
    dataset: &Dataset,
    home: u8,
    plant: &crate::state::Facility,
) -> i64 {
    let (revenue, inputs, fixed) = block_parts(world, dataset, home, plant.recipe, plant.region);
    let blocks = plant.blocks as i64;
    let full = plant.full_weekly_output(dataset).0.max(1) as i128;
    let made = plant.last_output.0.max(0) as i128;
    let margin = ((revenue - inputs) as i128 * blocks as i128 * made / full) as i64;
    margin - fixed * blocks
}

/// Extra a block of goods earns by selling where they are imported instead of at home.
pub fn netback_gain(world: &World, dataset: &Dataset, recipe: u16, region: u8) -> i64 {
    dataset.recipes[recipe as usize]
        .outputs()
        .filter_map(|(id, lots)| {
            let product = dataset.product_index.get(id).copied()?;
            if !corridor(dataset, product) {
                return None;
            }
            Some(
                import_netback(world, dataset, product, region)
                    .map(|net| net - public_price(world, dataset, product, region))
                    .unwrap_or(0)
                    * lots,
            )
        })
        .sum()
}

/// The horizon a company running `plants` plants judges a new one on: the content's span of
/// plants is free, each one past it shortens the horizon, so a large company needs a faster
/// payback to add more. A smooth cost of size in place of a hard cap.
pub fn horizon_weeks(dataset: &Dataset, plants: i64) -> i64 {
    let rules = &dataset.capacity;
    if rules.span_plants <= 0 {
        return HORIZON_WEEKS;
    }
    let past = (plants - rules.span_plants).max(0);
    (HORIZON_WEEKS - past * rules.span_weeks_per_plant).max(MIN_HORIZON_WEEKS)
}

/// One opening: would one more block of a strategy in a state pay, and how fast.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opening {
    pub recipe: u16,
    pub region: u8,
    pub product: u16,
    /// Whether the output trades on the corridor exchange (else it sells where it is made).
    pub traded: bool,
    /// Lots a week the market is short of, net of plants coming.
    pub room_lots: i64,
    /// One block's weekly output.
    pub block_lots: i64,
    /// What a lot fetches: the import netback for a traded good, the public price otherwise.
    pub price_cents: i64,
    /// That price against the product's reference, basis points.
    pub price_vs_reference_bps: i64,
    /// Households going without in the regions served, lots a week, and what they asked for.
    pub unmet_lots: i64,
    pub wanted_lots: i64,
    /// One block's weekly contribution at this week's prices and wages.
    pub weekly_contribution_cents: i64,
    /// What one block costs to build today (`sectors::build_price`), before any entry fee.
    pub build_cost_cents: i64,
    /// Weeks of contribution that pay the build back; None when it never does.
    pub payback_weeks: Option<i64>,
}

/// The opening for one strategy in one state, from a head office in `home`.
pub fn opening(
    world: &World,
    dataset: &Dataset,
    home: u8,
    recipe: u16,
    region: u8,
) -> Option<Opening> {
    let template = dataset.recipes.get(recipe as usize)?;
    let product = dataset.product_index.get(&template.output).copied()?;
    let traded = corridor(dataset, product);
    let room = room_lots(world, dataset, product, region);
    let (unmet, wanted) = if traded {
        (0, 0)
    } else {
        unmet_in(world, product, &served(dataset, product, region))
    };
    let price = if traded {
        import_netback(world, dataset, product, region)
            .unwrap_or_else(|| public_price(world, dataset, product, region))
    } else {
        public_price(world, dataset, product, region)
    };
    let reference = crate::economy::pricing::price_reference(world, dataset, product).max(1);
    let contribution = block_contribution(world, dataset, home, recipe, region)
        + netback_gain(world, dataset, recipe, region);
    let cost = crate::economy::sectors::build_price(world, dataset, template.capex_cents);
    Some(Opening {
        recipe,
        region,
        product,
        traded,
        room_lots: room,
        block_lots: template.weekly_output_lots,
        price_cents: price,
        price_vs_reference_bps: price * 10_000 / reference,
        unmet_lots: unmet,
        wanted_lots: wanted,
        weekly_contribution_cents: contribution,
        build_cost_cents: cost,
        payback_weeks: (contribution > 0).then(|| (cost + contribution - 1) / contribution),
    })
}

/// Every opening in the corridor where the room takes at least a fifth of a block and a block
/// pays back, fastest payback first. What the player's sector board shows.
pub fn openings(world: &World, dataset: &Dataset, home: u8) -> Vec<Opening> {
    let mut found = Vec::new();
    for (index, template) in dataset.recipes.iter().enumerate() {
        if template.storage_class.is_some() {
            continue;
        }
        for region in 0..world.regions.len() as u8 {
            let Some(open) = opening(world, dataset, home, index as u16, region) else {
                continue;
            };
            if open.room_lots * 5 >= template.weekly_output_lots && open.payback_weeks.is_some() {
                found.push(open);
            }
        }
    }
    found.sort_by_key(|o| (o.payback_weeks.unwrap_or(i64::MAX), o.recipe, o.region));
    found
}
