//! Validated immutable datasets and authored content.
//!
//! Datasets are built from pinned in-repo JSON and never downloaded at runtime. A dataset
//! hash is recorded in every save and replay so an app upgrade cannot silently remap region
//! or product rows. Authored game calibration is visibly separated from real observations:
//! region codes and official names come from the verified 2004 BEA roster, while county
//! membership, populations, wages, demand and lane distances are labelled engineering
//! calibration until a provenance-traceable crosswalk is recovered.

pub mod balance;
pub mod events;
pub mod firms;
pub mod geography;

use std::collections::BTreeMap;

use cw_core::hash::{CanonicalHasher, DOMAIN_STATE};
use serde::{Deserialize, Serialize};

pub use balance::{Balance, Product, ProductKind, Recipe};
pub use events::EventCard;
pub use firms::{Archetype, SeedFirm};
pub use geography::{Conditions, Geography, Lane, Region};

pub const BALANCE_JSON: &str = include_str!("../../../content/initial-balance.json");
pub const GEOGRAPHY_JSON: &str = include_str!("../../../content/geography/states.json");
pub const EVENTS_JSON: &str = include_str!("../../../content/events/events.json");
pub const FIRMS_JSON: &str = include_str!("../../../content/firms.json");

/// Every industry required for release. Financial is served by the bank and insurer models.
pub const INDUSTRIES: [&str; 15] = [
    "agriculture",
    "extraction",
    "energy",
    "chemical_industries",
    "manufacturing",
    "construction",
    "logistics",
    "retail",
    "healthcare",
    "financial",
    "technology",
    "media",
    "telecommunications",
    "real_estate",
    "defense",
];

#[derive(Clone, Debug)]
pub struct Dataset {
    pub products: Vec<Product>,
    pub product_index: BTreeMap<String, u16>,
    pub recipes: Vec<Recipe>,
    pub recipe_index: BTreeMap<String, u16>,
    pub regions: Vec<Region>,
    pub region_index: BTreeMap<String, u8>,
    pub lanes: Vec<Lane>,
    pub events: Vec<EventCard>,
    pub seed_firms: Vec<SeedFirm>,
    pub baseline_weekly_wage_cents: [i64; 3],
    pub maintenance_annual_bps: i32,
    pub fixture_land_rent_cents_weekly: i64,
    pub geometry_note: String,
    pub hash: String,
}

impl Dataset {
    /// Load the pinned in-repo dataset and validate it.
    pub fn load() -> Result<Dataset, Vec<String>> {
        let balance: Balance = serde_json::from_str(BALANCE_JSON)
            .map_err(|error| vec![format!("initial-balance.json: {error}")])?;
        let geography: Geography = serde_json::from_str(GEOGRAPHY_JSON)
            .map_err(|error| vec![format!("states.json: {error}")])?;
        let events: events::EventDeck = serde_json::from_str(EVENTS_JSON)
            .map_err(|error| vec![format!("events.json: {error}")])?;
        let firms: firms::SeedFirms = serde_json::from_str(FIRMS_JSON)
            .map_err(|error| vec![format!("firms.json: {error}")])?;

        let mut products = balance.products;
        let mut recipes = balance.recipes;
        products.sort_by(|a, b| a.id.cmp(&b.id));
        recipes.sort_by(|a, b| a.id.cmp(&b.id));
        let geometry_note = geography.geometry_note();
        let mut regions = geography.regions;
        regions.sort_by(|a, b| a.code.cmp(&b.code));
        let mut seed_firms = firms.firms;
        seed_firms.sort_by(|a, b| a.id.cmp(&b.id));

        let mut product_index = BTreeMap::new();
        for (index, product) in products.iter().enumerate() {
            product_index.insert(product.id.clone(), index as u16);
        }
        let mut recipe_index = BTreeMap::new();
        for (index, recipe) in recipes.iter().enumerate() {
            recipe_index.insert(recipe.id.clone(), index as u16);
        }
        let mut region_index = BTreeMap::new();
        for (index, region) in regions.iter().enumerate() {
            region_index.insert(region.code.clone(), index as u8);
        }

        let dataset = Dataset {
            products,
            product_index,
            recipes,
            recipe_index,
            regions,
            region_index,
            lanes: geography.lanes,
            events: events.events,
            seed_firms,
            baseline_weekly_wage_cents: balance.baseline_weekly_wage_cents,
            maintenance_annual_bps: balance.maintenance_annual_bps,
            fixture_land_rent_cents_weekly: balance.fixture_land_rent_cents_weekly,
            geometry_note,
            hash: String::new(),
        };
        let errors = dataset.validate();
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut dataset = dataset;
        dataset.hash = dataset.compute_hash();
        Ok(dataset)
    }

    /// Structural validation. Any error is a data defect, never a gameplay outcome.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.products.len() != 34 {
            errors.push(format!(
                "expected 34 products, found {}",
                self.products.len()
            ));
        }
        // Every operating sector offers a real choice of strategies, each named for the player.
        let mut strategies: BTreeMap<&str, usize> = BTreeMap::new();
        for recipe in &self.recipes {
            *strategies.entry(recipe.industry.as_str()).or_default() += 1;
            if recipe.name.trim().is_empty() {
                errors.push(format!("strategy {} has no display name", recipe.id));
            }
        }
        for (industry, count) in &strategies {
            if *count < 3 {
                errors.push(format!(
                    "sector {industry} offers {count} strategies, needs at least 3"
                ));
            }
        }
        if self.regions.len() < 8 {
            errors.push(format!(
                "corridor needs at least 8 regions, found {}",
                self.regions.len()
            ));
        }
        if self.events.len() != 24 {
            errors.push(format!(
                "expected 24 event cards, found {}",
                self.events.len()
            ));
        }
        if self.seed_firms.len() != 30 {
            errors.push(format!(
                "expected 30 named firms, found {}",
                self.seed_firms.len()
            ));
        }

        for product in &self.products {
            if product.reference_price_cents < 0 {
                errors.push(format!("product {} has negative price", product.id));
            }
            if (product.reference_price_cents == 0)
                != (product.kind == ProductKind::FinancialContract)
            {
                errors.push(format!(
                    "product {} price/financial-kind mismatch",
                    product.id
                ));
            }
        }
        if self.product_index.len() != self.products.len() {
            errors.push("duplicate product ID".to_owned());
        }

        for recipe in &self.recipes {
            if !INDUSTRIES.contains(&recipe.industry.as_str()) {
                errors.push(format!("recipe {} has unknown industry", recipe.id));
            }
            match self.product_index.get(&recipe.output) {
                None => errors.push(format!("recipe {} output is unknown", recipe.id)),
                Some(index) => {
                    if self.products[*index as usize].kind == ProductKind::FinancialContract {
                        errors.push(format!(
                            "recipe {} produces a financial contract",
                            recipe.id
                        ));
                    }
                }
            }
            for input in recipe.weekly_inputs_lots.keys() {
                if !self.product_index.contains_key(input) {
                    errors.push(format!("recipe {} input {} is unknown", recipe.id, input));
                }
            }
            if recipe.weekly_output_lots <= 0 || recipe.capex_cents <= 0 || recipe.build_weeks == 0
            {
                errors.push(format!(
                    "recipe {} has non-positive physical constants",
                    recipe.id
                ));
            }
        }
        let covered: Vec<&str> = self.recipes.iter().map(|r| r.industry.as_str()).collect();
        for industry in INDUSTRIES {
            if industry != "financial" && !covered.contains(&industry) {
                errors.push(format!("industry {industry} has no operating template"));
            }
        }
        if self.recipe_index.len() != self.recipes.len() {
            errors.push("duplicate recipe ID".to_owned());
        }

        for region in &self.regions {
            if region.code.len() != 2 || !region.code.chars().all(|c| c.is_ascii_uppercase()) {
                errors.push(format!(
                    "region code {} is not a two letter state code",
                    region.code
                ));
            }
            if region.wage_index_bps <= 0 || region.demand_index_bps <= 0 {
                errors.push(format!(
                    "region {} has non-positive calibration",
                    region.code
                ));
            }
        }
        if self.region_index.len() != self.regions.len() {
            errors.push("duplicate region code".to_owned());
        }
        for left in 0..self.regions.len() {
            for right in (left + 1)..self.regions.len() {
                if self
                    .lane_km(&self.regions[left].code, &self.regions[right].code)
                    .is_none()
                {
                    errors.push(format!(
                        "missing lane {}<->{}",
                        self.regions[left].code, self.regions[right].code
                    ));
                }
            }
        }

        let mut event_ids = std::collections::BTreeSet::new();
        for card in &self.events {
            if !event_ids.insert(card.id.as_str()) {
                errors.push(format!("duplicate event {}", card.id));
            }
            if card.choices.len() < 2 {
                errors.push(format!("event {} needs at least two choices", card.id));
            }
            if card.choices.iter().any(|choice| choice.handler.is_empty()) {
                errors.push(format!("event {} has a choice without a handler", card.id));
            }
        }

        let mut per_industry: BTreeMap<&str, usize> = BTreeMap::new();
        for firm in &self.seed_firms {
            *per_industry.entry(firm.industry.as_str()).or_default() += 1;
            if !INDUSTRIES.contains(&firm.industry.as_str()) {
                errors.push(format!("firm {} has unknown industry", firm.id));
            }
            if !self.region_index.contains_key(&firm.region) {
                errors.push(format!("firm {} has unknown region", firm.id));
            }
            if firm.blocks == 0 || firm.blocks > 4 {
                errors.push(format!("firm {} block count out of range", firm.id));
            }
            if firm.recipe != "bank"
                && firm.recipe != "property_insurer"
                && !self.recipe_index.contains_key(&firm.recipe)
            {
                errors.push(format!("firm {} has unknown recipe", firm.id));
            }
        }
        for industry in INDUSTRIES {
            if per_industry.get(industry).copied().unwrap_or(0) != 2 {
                errors.push(format!(
                    "industry {industry} does not have exactly two named firms"
                ));
            }
        }
        errors
    }

    pub fn lane_km(&self, left: &str, right: &str) -> Option<u32> {
        if left == right {
            return Some(0);
        }
        self.lanes
            .iter()
            .find(|lane| (lane.a == left && lane.b == right) || (lane.a == right && lane.b == left))
            .map(|lane| lane.km)
    }

    pub fn product(&self, id: &str) -> Option<&Product> {
        self.product_index
            .get(id)
            .map(|index| &self.products[*index as usize])
    }

    pub fn product_code(&self, id: &str) -> Option<u16> {
        self.product_index.get(id).copied()
    }

    pub fn recipe(&self, id: &str) -> Option<&Recipe> {
        self.recipe_index
            .get(id)
            .map(|index| &self.recipes[*index as usize])
    }

    pub fn region(&self, code: &str) -> Option<&Region> {
        self.region_index
            .get(code)
            .map(|index| &self.regions[*index as usize])
    }

    pub fn reference_price(&self, product: u16) -> i64 {
        self.products[product as usize].reference_price_cents
    }

    pub fn event(&self, id: &str) -> Option<&EventCard> {
        self.events.iter().find(|card| card.id == id)
    }

    pub fn compute_hash(&self) -> String {
        let mut hasher = CanonicalHasher::new(DOMAIN_STATE);
        hasher.string("cw-dataset-v1");
        for product in &self.products {
            hasher
                .string(&product.id)
                .u8(product.kind.code())
                .i64(product.reference_price_cents);
        }
        for recipe in &self.recipes {
            hasher
                .string(&recipe.id)
                .string(&recipe.industry)
                .string(&recipe.output)
                .i64(recipe.weekly_output_lots)
                .i64(recipe.capex_cents)
                .u32(recipe.build_weeks)
                .u32(recipe.life_years);
            for (input, quantity) in &recipe.weekly_inputs_lots {
                hasher.string(input).i64(*quantity);
            }
            for worker in recipe.workers_low_mid_high {
                hasher.i64(worker);
            }
        }
        for region in &self.regions {
            hasher
                .string(&region.code)
                .string(&region.name)
                .i32(region.wage_index_bps)
                .i32(region.demand_index_bps)
                .i32(region.land_index_bps)
                .i32(region.opening_organisation_bps);
        }
        for lane in &self.lanes {
            hasher.string(&lane.a).string(&lane.b).u32(lane.km);
        }
        for card in &self.events {
            hasher.string(&card.id).u32(card.cooldown_weeks);
            for choice in &card.choices {
                hasher.string(&choice.id).string(&choice.handler);
            }
        }
        for firm in &self.seed_firms {
            hasher
                .string(&firm.id)
                .string(&firm.industry)
                .string(&firm.region)
                .string(&firm.recipe)
                .u8(firm.blocks)
                .bool(firm.public);
        }
        hasher.finish_hex()
    }
}

/// Difficulty is a published rule set, never hidden income.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    #[default]
    Standard,
}

impl Difficulty {
    pub const fn starting_capital_cents(self) -> i64 {
        500_000_000
    }

    pub const fn formation_fee_cents(self) -> i64 {
        2_500_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_dataset_loads_and_validates() {
        let dataset = Dataset::load().expect("dataset must validate");
        assert_eq!(dataset.products.len(), 34);
        assert_eq!(dataset.recipes.len(), 53);
        assert_eq!(dataset.regions.len(), 8);
        assert_eq!(dataset.events.len(), 24);
        assert_eq!(dataset.seed_firms.len(), 30);
        assert_eq!(dataset.hash.len(), 64);
    }

    #[test]
    fn the_playable_states_are_unique_and_labelled() {
        let dataset = Dataset::load().unwrap();
        assert_eq!(dataset.regions.len(), 8);
        let mut codes = std::collections::BTreeSet::new();
        for region in &dataset.regions {
            assert!(codes.insert(region.code.clone()), "duplicate region code");
            assert_eq!(
                region.states,
                vec![region.code.clone()],
                "one region is one state"
            );
            assert!(
                region.population_2000 > 1_000_000,
                "{} population",
                region.code
            );
            assert!(region.wage_index_bps > 0);
            assert!(region.demand_index_bps > 0);
            let c = region.conditions;
            for value in [c.power_grid_bps, c.road_bps, c.broadband_bps, c.skill_bps] {
                assert!((0..=10_000).contains(&value));
            }
        }
        // Census 2000 resident counts, sum of the eight states.
        let total: i64 = dataset.regions.iter().map(|r| r.population_2000).sum();
        assert_eq!(total, 76_898_986);
        assert!(
            dataset.geometry_note.contains("authored condition indices"),
            "the note labels the authored part: {}",
            dataset.geometry_note
        );
    }

    #[test]
    fn every_region_pair_has_a_lane() {
        let dataset = Dataset::load().unwrap();
        for left in &dataset.regions {
            for right in &dataset.regions {
                assert!(dataset.lane_km(&left.code, &right.code).is_some());
            }
        }
        assert_eq!(dataset.lane_km("OH", "OH"), Some(0));
        assert_eq!(dataset.lane_km("OH", "MI"), dataset.lane_km("MI", "OH"));
    }

    #[test]
    fn dataset_hash_is_stable() {
        let first = Dataset::load().unwrap();
        let second = Dataset::load().unwrap();
        assert_eq!(first.hash, second.hash);
    }

    #[test]
    fn every_industry_has_two_named_firms_and_a_recipe() {
        let dataset = Dataset::load().unwrap();
        for industry in INDUSTRIES {
            let count = dataset
                .seed_firms
                .iter()
                .filter(|f| f.industry == industry)
                .count();
            assert_eq!(count, 2, "{industry}");
            if industry != "financial" {
                assert!(
                    dataset.recipes.iter().any(|r| r.industry == industry),
                    "{industry}"
                );
            }
        }
    }

    #[test]
    fn financial_products_never_enter_goods_recipes() {
        let dataset = Dataset::load().unwrap();
        for recipe in &dataset.recipes {
            for input in recipe.weekly_inputs_lots.keys() {
                let product = dataset.product(input).unwrap();
                assert_ne!(product.kind, ProductKind::FinancialContract);
            }
        }
    }
}
