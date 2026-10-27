//! Validated immutable datasets and authored content.
//!
//! Datasets are built from pinned in-repo JSON and never downloaded at runtime. A dataset
//! hash is recorded in every save and replay so an app upgrade cannot silently remap region
//! or product rows. Authored game calibration is visibly separated from real observations:
//! region codes and official names come from the verified 2004 BEA roster, while county
//! membership, populations, wages, demand and lane distances are labelled engineering
//! calibration until a provenance-traceable crosswalk is recovered.

pub mod balance;
pub mod cast;
pub mod custom;
pub mod events;
pub mod firms;
pub mod geography;
pub mod logistics;
pub mod price_paths;

use std::collections::BTreeMap;

use cw_core::hash::{CanonicalHasher, DOMAIN_STATE};
use serde::{Deserialize, Serialize};

pub use balance::{Balance, BalanceOverlay, Product, ProductKind, Recipe, StorageClass};
pub use cast::{Person, Role, Temperament};
pub use custom::{CustomLaws, CustomScenario, PricePath, RegionAdjust, RivalSetting};
pub use events::EventCard;
pub use firms::{Archetype, ExtraPlant, SeedFirm};
pub use geography::{Conditions, Geography, Lane, Region, StateInfo};
pub use logistics::{GatewayLink, LogisticsData, LogisticsOverlay, ModeDef, SeaSystem};

pub const BALANCE_JSON: &str = include_str!("../../../content/initial-balance.json");
pub const GEOGRAPHY_JSON: &str = include_str!("../../../content/geography/regions.json");
pub const EVENTS_JSON: &str = include_str!("../../../content/events/events.json");
pub const FIRMS_JSON: &str = include_str!("../../../content/firms.json");
/// Scenario catalogue for the start screen. Presentation only: not part of the dataset hash.
pub const SCENARIOS_JSON: &str = include_str!("../../../content/scenarios.json");

/// The scenario whose content sits at the top level of `content/`, and the one `Dataset::load`
/// builds. Every other scenario with its own data keeps it under `content/scenarios/<id>/`.
pub const DEFAULT_SCENARIO: &str = "great-lakes-2000";

/// The content one scenario selects from the shared products, strategies and events: its
/// geography, its seed firms and, where it differs, its freight terminals and starting balance.
#[derive(Clone, Copy, Debug)]
pub struct ScenarioContent {
    pub id: &'static str,
    pub geography_json: &'static str,
    pub firms_json: &'static str,
    /// Public rail, ports, sea systems and gateway legs; None keeps the shared file's.
    pub logistics_json: Option<&'static str>,
    /// Wage level and opening prices; None keeps the shared balance.
    pub balance_json: Option<&'static str>,
    /// The region a guided career's opening company is in, and the strategy it runs there.
    pub player_region: &'static str,
    pub player_recipe: &'static str,
}

/// Every scenario that has its own dataset. A scenario not listed here uses the default one.
pub const SCENARIO_CONTENT: [ScenarioContent; 3] = [
    ScenarioContent {
        id: DEFAULT_SCENARIO,
        geography_json: GEOGRAPHY_JSON,
        firms_json: FIRMS_JSON,
        logistics_json: None,
        balance_json: None,
        player_region: "OH-CEN",
        player_recipe: "farm",
    },
    ScenarioContent {
        id: "texas-1981",
        geography_json: include_str!("../../../content/scenarios/texas-1981/regions.json"),
        firms_json: include_str!("../../../content/scenarios/texas-1981/firms.json"),
        logistics_json: Some(include_str!(
            "../../../content/scenarios/texas-1981/logistics.json"
        )),
        balance_json: Some(include_str!(
            "../../../content/scenarios/texas-1981/balance.json"
        )),
        player_region: "TX-PMB",
        player_recipe: "oil_wells",
    },
    ScenarioContent {
        id: "silicon-valley-1995",
        geography_json: include_str!("../../../content/scenarios/silicon-valley-1995/regions.json"),
        firms_json: include_str!("../../../content/scenarios/silicon-valley-1995/firms.json"),
        logistics_json: Some(include_str!(
            "../../../content/scenarios/silicon-valley-1995/logistics.json"
        )),
        balance_json: Some(include_str!(
            "../../../content/scenarios/silicon-valley-1995/balance.json"
        )),
        player_region: "CA-SJC",
        player_recipe: "hardware_maker",
    },
];

/// The content for a scenario id, or the default scenario's when the id has none of its own.
pub fn scenario_content(id: &str) -> &'static ScenarioContent {
    SCENARIO_CONTENT
        .iter()
        .find(|content| content.id == id)
        .unwrap_or(&SCENARIO_CONTENT[0])
}

/// One playable scenario as listed on the start screen.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub country: String,
    pub start_year: u32,
    pub summary: String,
    pub center_lon_milli: i32,
    pub center_lat_milli: i32,
    /// False for scenarios shown as coming soon.
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub region: String,
    /// Bundled art, without extension.
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub tagline: String,
    /// How world commodity prices move unless the player picks otherwise: "historical",
    /// "historical_variance" or "random".
    #[serde(default)]
    pub default_price_path: String,
}

impl Scenario {
    /// The scenario's default world price path mode.
    pub fn price_path(&self) -> PricePath {
        PricePath::from_key(&self.default_price_path).unwrap_or(PricePath::Historical)
    }
}

#[derive(Deserialize)]
struct ScenarioFile {
    scenarios: Vec<Scenario>,
}

/// The bundled scenarios, in display order.
pub fn scenarios() -> Vec<Scenario> {
    serde_json::from_str::<ScenarioFile>(SCENARIOS_JSON)
        .map(|file| file.scenarios)
        .unwrap_or_default()
}

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
    /// The scenario this dataset is the content of.
    pub scenario_id: String,
    /// Where a guided career's opening company is, and the strategy it runs.
    pub player_region: String,
    pub player_recipe: String,
    pub products: Vec<Product>,
    pub product_index: BTreeMap<String, u16>,
    pub recipes: Vec<Recipe>,
    pub recipe_index: BTreeMap<String, u16>,
    pub regions: Vec<Region>,
    pub region_index: BTreeMap<String, u8>,
    /// Playable states, the political layer over regions, sorted by code.
    pub states: Vec<StateInfo>,
    pub lanes: Vec<Lane>,
    pub events: Vec<EventCard>,
    pub seed_firms: Vec<SeedFirm>,
    /// The character cast, in file order. A person's index is their id in saves.
    pub cast: Vec<Person>,
    pub baseline_weekly_wage_cents: [i64; 3],
    pub maintenance_annual_bps: i32,
    pub fixture_land_rent_cents_weekly: i64,
    pub geometry_note: String,
    /// Freight modes, public terminals and sea systems.
    pub logistics: LogisticsData,
    /// Land distance in kilometres between every pair of regions, row major, zero on the diagonal.
    pub km_matrix: Vec<u32>,
    /// Sea distance between ports of one sea system, row major; zero when there is no sea lane.
    pub sea_matrix: Vec<u32>,
    /// Regions that have a public rail yard and a public port from the start.
    pub public_rail: Vec<bool>,
    pub public_ports: Vec<bool>,
    pub hash: String,
}

impl Dataset {
    /// The state a region belongs to.
    pub fn state_of(&self, region: u8) -> Option<&StateInfo> {
        let code = self.regions.get(region as usize)?.state();
        self.states.iter().find(|state| state.code == code)
    }

    /// Indices of every region in a state, in region order.
    pub fn regions_in_state(&self, state: &str) -> Vec<u8> {
        self.regions
            .iter()
            .enumerate()
            .filter(|(_, region)| region.state() == state)
            .map(|(index, _)| index as u8)
            .collect()
    }
}

impl Dataset {
    /// Load the default scenario's pinned in-repo dataset and validate it.
    pub fn load() -> Result<Dataset, Vec<String>> {
        Self::load_scenario(DEFAULT_SCENARIO)
    }

    /// The dataset whose hash a save or replay recorded, if any bundled scenario produces it.
    pub fn for_hash(hash: &str) -> Option<Dataset> {
        SCENARIO_CONTENT
            .iter()
            .filter_map(|content| Dataset::load_scenario(content.id).ok())
            .find(|dataset| dataset.hash == hash)
    }

    /// Load one scenario's pinned in-repo dataset and validate it. A scenario with no content
    /// of its own gets the default scenario's.
    pub fn load_scenario(scenario_id: &str) -> Result<Dataset, Vec<String>> {
        let content = scenario_content(scenario_id);
        let mut balance: Balance = serde_json::from_str(BALANCE_JSON)
            .map_err(|error| vec![format!("initial-balance.json: {error}")])?;
        if let Some(overlay) = content.balance_json {
            let overlay: BalanceOverlay = serde_json::from_str(overlay)
                .map_err(|error| vec![format!("{} balance.json: {error}", content.id)])?;
            overlay.apply(&mut balance).map_err(|error| vec![error])?;
        }
        let balance = balance.migrate().map_err(|error| vec![error])?;
        let geography: Geography = serde_json::from_str(content.geography_json)
            .map_err(|error| vec![format!("{} regions.json: {error}", content.id)])?;
        let events: events::EventDeck = serde_json::from_str(EVENTS_JSON)
            .map_err(|error| vec![format!("events.json: {error}")])?;
        let firms: firms::SeedFirms = serde_json::from_str(content.firms_json)
            .map_err(|error| vec![format!("{} firms.json: {error}", content.id)])?;

        let cast: cast::CastFile = serde_json::from_str(cast::CAST_JSON)
            .map_err(|error| vec![format!("cast.json: {error}")])?;

        let mut products = balance.products;
        let mut recipes = balance.recipes;
        products.sort_by(|a, b| a.id.cmp(&b.id));
        recipes.sort_by(|a, b| a.id.cmp(&b.id));
        let geometry_note = geography.geometry_note();
        let mut regions = geography.regions;
        regions.sort_by(|a, b| a.code.cmp(&b.code));
        let mut states = geography.states;
        states.sort_by(|a, b| a.code.cmp(&b.code));
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

        let logistics = match content.logistics_json {
            Some(overlay) => LogisticsData::load_with(overlay),
            None => LogisticsData::load(),
        }
        .map_err(|error| vec![error])?;
        let n = regions.len();
        let mut km_matrix = vec![0u32; n * n];
        for lane in &geography.lanes {
            if let (Some(a), Some(b)) = (region_index.get(&lane.a), region_index.get(&lane.b)) {
                km_matrix[*a as usize * n + *b as usize] = lane.km;
                km_matrix[*b as usize * n + *a as usize] = lane.km;
            }
        }
        let mut sea_matrix = vec![0u32; n * n];
        for system in &logistics.sea_systems {
            let ports: Vec<usize> = system
                .ports
                .iter()
                .filter_map(|code| region_index.get(code).map(|i| *i as usize))
                .collect();
            for &a in &ports {
                for &b in &ports {
                    if a != b {
                        let km = km_matrix[a * n + b] as i64 * system.factor_bps / 10_000;
                        sea_matrix[a * n + b] = km.max(1) as u32;
                    }
                }
            }
        }
        let flags = |codes: &[String]| -> Vec<bool> {
            let mut out = vec![false; n];
            for code in codes {
                if let Some(i) = region_index.get(code) {
                    out[*i as usize] = true;
                }
            }
            out
        };
        let public_rail = flags(&logistics.public_rail);
        let public_ports = flags(&logistics.public_ports);
        let dataset = Dataset {
            scenario_id: content.id.to_owned(),
            player_region: content.player_region.to_owned(),
            player_recipe: content.player_recipe.to_owned(),
            products,
            product_index,
            recipes,
            recipe_index,
            regions,
            region_index,
            states,
            lanes: geography.lanes,
            events: events.events,
            seed_firms,
            cast: cast.people,
            baseline_weekly_wage_cents: balance.baseline_weekly_wage_cents,
            maintenance_annual_bps: balance.maintenance_annual_bps,
            fixture_land_rent_cents_weekly: balance.fixture_land_rent_cents_weekly,
            geometry_note,
            logistics,
            km_matrix,
            sea_matrix,
            public_rail,
            public_ports,
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
        let mut errors = cast::validate(&self.cast);
        if self.products.len() < 34 {
            errors.push(format!(
                "expected at least 34 products, found {}",
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
        // The default scenario names exactly thirty firms, two per industry. A scenario with
        // its own roster may name more where its economy is concentrated, but never fewer.
        let roster_ok = if self.scenario_id == DEFAULT_SCENARIO {
            self.seed_firms.len() == 30
        } else {
            self.seed_firms.len() >= 30
        };
        if !roster_ok {
            errors.push(format!(
                "expected 30 named firms or more, found {}",
                self.seed_firms.len()
            ));
        }
        if !self.recipe_index.contains_key(&self.player_recipe) {
            errors.push(format!(
                "the opening strategy {} is not in the scenario",
                self.player_recipe
            ));
        }
        if !self.region_index.contains_key(&self.player_region) {
            errors.push(format!(
                "the opening region {} is not in the scenario",
                self.player_region
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
            for by in &recipe.by_products {
                match self.product_index.get(&by.product) {
                    None => errors.push(format!(
                        "recipe {} by-product {} is unknown",
                        recipe.id, by.product
                    )),
                    Some(index) => {
                        if self.products[*index as usize].kind == ProductKind::FinancialContract {
                            errors.push(format!(
                                "recipe {} makes a financial contract as a by-product",
                                recipe.id
                            ));
                        }
                    }
                }
                if by.product == recipe.output || by.weekly_lots <= 0 {
                    errors.push(format!(
                        "recipe {} by-product {} repeats the output or has no yield",
                        recipe.id, by.product
                    ));
                }
                if recipe
                    .by_products
                    .iter()
                    .filter(|other| other.product == by.product)
                    .count()
                    > 1
                {
                    errors.push(format!(
                        "recipe {} lists by-product {} twice",
                        recipe.id, by.product
                    ));
                }
            }
            if recipe.storage_class.is_some() {
                let space = self
                    .product_index
                    .get(&recipe.output)
                    .map(|i| &self.products[*i as usize]);
                if recipe.industry != "logistics"
                    || !recipe.by_products.is_empty()
                    || !space.is_some_and(|p| p.kind.is_service())
                {
                    errors.push(format!(
                        "storage recipe {} must be a logistics service with no by-products",
                        recipe.id
                    ));
                }
            }
            if let Some(mode) = &recipe.transport_mode {
                let carries = self.logistics.mode_index(mode).is_some();
                let freight = self
                    .product_index
                    .get(&recipe.output)
                    .map(|i| &self.products[*i as usize]);
                if !carries
                    || recipe.industry != "logistics"
                    || recipe.storage_class.is_some()
                    || !freight.is_some_and(|p| p.kind == ProductKind::TransportService)
                {
                    errors.push(format!(
                        "carrier recipe {} needs a known mode, the logistics sector and freight output",
                        recipe.id
                    ));
                }
                if recipe.terminal
                    && !self
                        .logistics
                        .modes
                        .iter()
                        .any(|m| &m.id == mode && m.needs_terminal)
                {
                    errors.push(format!(
                        "terminal recipe {} is for a mode with no terminals",
                        recipe.id
                    ));
                }
            } else if recipe.terminal {
                errors.push(format!("terminal recipe {} has no mode", recipe.id));
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
            // Region codes are STATE-AREA: a two letter state code, a hyphen, two or three
            // upper-case letters, and the prefix must be the region's own state.
            let valid = match region.code.split_once('-') {
                Some((state, area)) => {
                    state.len() == 2
                        && state.chars().all(|c| c.is_ascii_uppercase())
                        && (2..=3).contains(&area.len())
                        && area.chars().all(|c| c.is_ascii_uppercase())
                        && region.states == vec![state.to_owned()]
                }
                None => false,
            };
            if !valid {
                errors.push(format!(
                    "region code {} is not STATE-AREA inside its own state",
                    region.code
                ));
            }
            if !self.states.iter().any(|state| state.code == region.state()) {
                errors.push(format!("region {} names an unknown state", region.code));
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
        for state in &self.states {
            if state.specialization.len() != 2
                || state
                    .specialization
                    .iter()
                    .any(|industry| !INDUSTRIES.contains(&industry.as_str()))
            {
                errors.push(format!(
                    "state {} needs two known specialized sectors",
                    state.code
                ));
            }
            if self.regions_in_state(&state.code).len() < 2 {
                errors.push(format!("state {} has fewer than two regions", state.code));
            }
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

        for mode in &self.logistics.modes {
            if mode.rate_milli_cents_per_lot_km <= 0 || mode.speed_km_week == 0 || mode.min_lots < 1
            {
                errors.push(format!(
                    "freight mode {} has non-positive constants",
                    mode.id
                ));
            }
            for product in &mode.excluded_products {
                if !self.product_index.contains_key(product) {
                    errors.push(format!(
                        "mode {} excludes unknown product {product}",
                        mode.id
                    ));
                }
            }
        }
        if self.logistics.modes.first().map(|m| m.id.as_str()) != Some("truck")
            || self.logistics.modes.len() > 8
        {
            errors.push("freight modes must start with truck and number at most eight".to_owned());
        }
        for code in self
            .logistics
            .public_rail
            .iter()
            .chain(&self.logistics.public_ports)
            .chain(self.logistics.gateway_links.iter().map(|l| &l.region))
            .chain(
                self.logistics
                    .sea_systems
                    .iter()
                    .flat_map(|s| s.ports.iter()),
            )
        {
            if !self.region_index.contains_key(code) {
                errors.push(format!("logistics names unknown region {code}"));
            }
        }
        let logistics_ok = {
            let l = &self.logistics;
            l.outside_premium_bps >= l.empty_lane_bps
                && l.empty_lane_bps > l.own_cost_bps
                && l.own_cost_bps > 0
                && (1..=10_000).contains(&l.contract_discount_bps)
        };
        if !logistics_ok {
            errors.push("freight price levels must satisfy own < empty <= outside".to_owned());
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
            // Plants are a fifth of the 1.1.0 unit, so seeds hold up to twenty.
            if firm.blocks == 0 || firm.blocks > 20 {
                errors.push(format!("firm {} block count out of range", firm.id));
            }
            for code in &firm.serves {
                if !self.region_index.contains_key(code) {
                    errors.push(format!("firm {} pins unknown region {code}", firm.id));
                }
            }
            if firm.recipe != "bank"
                && firm.recipe != "property_insurer"
                && !self.recipe_index.contains_key(&firm.recipe)
            {
                errors.push(format!("firm {} has unknown recipe", firm.id));
            }
            // A firm runs one strategy per sector in a region, so no two of its plants share both.
            let mut divisions: Vec<(&str, &str)> = Vec::new();
            if let Some(main) = self.recipe(&firm.recipe) {
                divisions.push((main.industry.as_str(), firm.region.as_str()));
            }
            for plant in &firm.extra_plants {
                if let Some(recipe) = self.recipe(&plant.recipe) {
                    let key = (recipe.industry.as_str(), plant.region.as_str());
                    if divisions.contains(&key) {
                        errors.push(format!(
                            "firm {} has two plants in one sector and region ({} in {})",
                            firm.id, key.0, key.1
                        ));
                    }
                    divisions.push(key);
                }
                if !self.recipe_index.contains_key(&plant.recipe) {
                    errors.push(format!("firm {} extra plant has unknown recipe", firm.id));
                }
                if !self.region_index.contains_key(&plant.region) {
                    errors.push(format!("firm {} extra plant has unknown region", firm.id));
                }
                for code in &plant.serves {
                    if !self.region_index.contains_key(code) {
                        errors.push(format!(
                            "firm {} extra plant pins unknown region {code}",
                            firm.id
                        ));
                    }
                }
                if plant.blocks == 0 || plant.blocks > 20 {
                    errors.push(format!(
                        "firm {} extra plant block count out of range",
                        firm.id
                    ));
                }
            }
        }
        for industry in INDUSTRIES {
            let count = per_industry.get(industry).copied().unwrap_or(0);
            let ok = if self.scenario_id == DEFAULT_SCENARIO {
                count == 2
            } else {
                count >= 2
            };
            if !ok {
                errors.push(format!(
                    "industry {industry} does not have its named firms (found {count})"
                ));
            }
        }
        errors
    }

    /// Land distance between two regions by index; zero within a region.
    pub fn km(&self, a: u8, b: u8) -> u32 {
        let n = self.regions.len();
        self.km_matrix
            .get(a as usize * n + b as usize)
            .copied()
            .unwrap_or(0)
    }

    /// Sea distance between two ports of one sea system, if they share one.
    pub fn sea_km(&self, a: u8, b: u8) -> Option<u32> {
        let n = self.regions.len();
        self.sea_matrix
            .get(a as usize * n + b as usize)
            .copied()
            .filter(|km| *km > 0)
    }

    /// Whether a freight mode may carry a good at all: its storage class and the exclusion list.
    pub fn mode_carries(&self, mode: u8, product: u16) -> bool {
        let Some(def) = self.logistics.modes.get(mode as usize) else {
            return false;
        };
        let item = &self.products[product as usize];
        let class = if item.kind.is_service() || item.kind.is_financial() {
            return false;
        } else {
            item.storage_class
        };
        def.classes.contains(&class) && !def.excluded_products.contains(&item.id)
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

    /// The store a good needs, or None for services and financial contracts, which are never held.
    pub fn storage_class_of(&self, product: u16) -> Option<StorageClass> {
        let product = &self.products[product as usize];
        (!product.kind.is_service() && !product.kind.is_financial())
            .then_some(product.storage_class)
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
                .i64(product.reference_price_cents)
                .u32(product.spoilage_bps_weekly)
                .u8(product.storage_class.code())
                .u32(product.holding_bps_weekly);
        }
        for recipe in &self.recipes {
            hasher
                .string(&recipe.id)
                .string(&recipe.industry)
                .string(&recipe.output)
                .i64(recipe.weekly_output_lots)
                .i64(recipe.capex_cents)
                .u32(recipe.build_weeks)
                .u32(recipe.life_years)
                .u8(recipe.storage_class.map(|c| c.code() + 1).unwrap_or(0))
                .string(recipe.transport_mode.as_deref().unwrap_or(""))
                .bool(recipe.terminal);
            for (input, quantity) in &recipe.weekly_inputs_lots {
                hasher.string(input).i64(*quantity);
            }
            for by in &recipe.by_products {
                hasher.string(&by.product).i64(by.weekly_lots);
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
        let l = &self.logistics;
        hasher
            .i64(l.outside_premium_bps)
            .i64(l.empty_lane_bps)
            .i64(l.own_cost_bps)
            .i64(l.contract_discount_bps);
        for mode in &l.modes {
            hasher
                .string(&mode.id)
                .i64(mode.rate_milli_cents_per_lot_km)
                .u32(mode.speed_km_week)
                .u32(mode.reach_km)
                .i64(mode.min_lots)
                .bool(mode.needs_terminal)
                .bool(mode.needs_infrastructure)
                .bool(mode.outside_backstop)
                .bool(mode.road_sensitive);
            for class in &mode.classes {
                hasher.u8(class.code());
            }
            for product in &mode.excluded_products {
                hasher.string(product);
            }
        }
        for flag in self.public_rail.iter().chain(&self.public_ports) {
            hasher.bool(*flag);
        }
        for km in &self.sea_matrix {
            hasher.u32(*km);
        }
        for link in &l.gateway_links {
            hasher
                .string(&link.region)
                .string(&link.gateway)
                .u32(link.km);
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
            for code in &firm.serves {
                hasher.string(code);
            }
            for plant in &firm.extra_plants {
                hasher
                    .string(&plant.recipe)
                    .string(&plant.region)
                    .u8(plant.blocks);
                for code in &plant.serves {
                    hasher.string(code);
                }
            }
        }
        for person in &self.cast {
            hasher
                .string(&person.id)
                .string(&person.scenario)
                .string(&person.state)
                .u8(person.temperament.code())
                .i64(person.stance.unwrap_or(0) as i64);
            for role in &person.roles {
                hasher.u8(*role as u8);
            }
        }
        hasher.finish_hex()
    }
}

/// Difficulty is a published rule set, never hidden income.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    /// More starting capital; rivals never attack the player's markets.
    Easy,
    #[default]
    Standard,
    /// Less starting capital; every rival fights for market share.
    Hard,
}

impl Difficulty {
    pub const fn starting_capital_cents(self) -> i64 {
        match self {
            Difficulty::Easy => 800_000_000,
            Difficulty::Standard => 500_000_000,
            Difficulty::Hard => 300_000_000,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Standard => "standard",
            Difficulty::Hard => "hard",
        }
    }

    pub fn from_key(key: &str) -> Difficulty {
        match key {
            "easy" => Difficulty::Easy,
            "hard" => Difficulty::Hard,
            _ => Difficulty::Standard,
        }
    }

    pub const fn formation_fee_cents(self) -> i64 {
        2_500_000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scenario_catalogue_lists_the_first_scenario() {
        let scenarios = scenarios();
        assert!(!scenarios.is_empty(), "content/scenarios.json parses");
        let first = &scenarios[0];
        assert_eq!(first.id, "great-lakes-2000");
        assert_eq!(first.start_year, 2000);
        assert!(first.summary.len() > 20);
    }

    #[test]
    fn pinned_dataset_loads_and_validates() {
        let dataset = Dataset::load().expect("dataset must validate");
        assert_eq!(dataset.products.len(), 44);
        assert_eq!(dataset.recipes.len(), 68);
        assert_eq!(dataset.regions.len(), 31);
        assert_eq!(dataset.states.len(), 8);
        assert_eq!(dataset.events.len(), 24);
        assert_eq!(dataset.seed_firms.len(), 30);
        assert_eq!(dataset.hash.len(), 64);
    }

    #[test]
    fn regions_sit_inside_eight_labelled_states() {
        let dataset = Dataset::load().unwrap();
        assert_eq!(dataset.states.len(), 8);
        assert_eq!(dataset.regions.len(), 31);
        for region in &dataset.regions {
            assert!(
                region.population_2000 > 200_000,
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
        let total: i64 = dataset.states.iter().map(|s| s.population_2000).sum();
        assert_eq!(total, 76_898_986);
        // Region populations come from county figures and agree with the state counts to
        // within a tenth of a percent.
        for state in &dataset.states {
            let regions: i64 = dataset
                .regions_in_state(&state.code)
                .iter()
                .map(|r| dataset.regions[*r as usize].population_2000)
                .sum();
            assert!((regions - state.population_2000).abs() * 1_000 < state.population_2000);
        }
        let ohio = dataset.region_index["OH-NE"];
        assert_eq!(dataset.state_of(ohio).unwrap().name, "Ohio");
    }

    #[test]
    fn every_region_pair_has_a_lane() {
        let dataset = Dataset::load().unwrap();
        for left in &dataset.regions {
            for right in &dataset.regions {
                assert!(dataset.lane_km(&left.code, &right.code).is_some());
            }
        }
        assert_eq!(dataset.lane_km("OH-CEN", "OH-CEN"), Some(0));
        assert_eq!(
            dataset.lane_km("OH-CEN", "MI-DET"),
            dataset.lane_km("MI-DET", "OH-CEN")
        );
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

    #[test]
    fn a_refinery_makes_four_products_in_a_fixed_split() {
        let dataset = Dataset::load().unwrap();
        let refinery = dataset.recipe("refinery").unwrap();
        let outputs: Vec<(&str, i64)> = refinery.outputs().collect();
        assert_eq!(
            outputs,
            vec![
                ("fuel", 26),
                ("gasoline", 36),
                ("jet_fuel", 8),
                ("petrochem_feedstock", 12)
            ]
        );
        assert!(refinery.makes("gasoline") && !refinery.makes("steel"));
        assert_eq!(refinery.lots_of("jet_fuel"), 8);
        assert_eq!(refinery.lots_of("steel"), 0);
        // Crude is a real product: wells make it, only a refinery turns it into fuel.
        assert_eq!(dataset.recipe("oil_wells").unwrap().output, "crude_oil");
        assert!(refinery.weekly_inputs_lots.contains_key("crude_oil"));
        // Scrap is a by-product of steel works, stamping and vehicle assembly, and feeds steel.
        for id in ["metal_works", "component_works", "vehicle_assembly"] {
            assert!(dataset.recipe(id).unwrap().makes("scrap"), "{id}");
        }
        assert!(
            dataset
                .recipe("electric_arc")
                .unwrap()
                .weekly_inputs_lots
                .contains_key("scrap")
        );
    }

    #[test]
    fn output_value_counts_every_product_the_plant_makes() {
        let dataset = Dataset::load().unwrap();
        let price = |id: &str| dataset.reference_price(dataset.product_index[id]);
        let refinery = dataset.recipe("refinery").unwrap();
        let expected = 26 * price("fuel")
            + 36 * price("gasoline")
            + 8 * price("jet_fuel")
            + 12 * price("petrochem_feedstock");
        assert_eq!(refinery.output_value(price), expected);
        let farm = dataset.recipe("farm").unwrap();
        assert_eq!(
            farm.output_value(price),
            farm.weekly_output_lots * price("food_raw")
        );
    }

    #[test]
    fn a_schema_one_balance_file_migrates_unchanged() {
        let mut balance: Balance = serde_json::from_str(BALANCE_JSON).unwrap();
        // A schema 1 file has no by-products or spoilage and still reads as the same plants.
        balance.schema = 1;
        for recipe in &mut balance.recipes {
            recipe.by_products.clear();
        }
        let migrated = balance.migrate().unwrap();
        assert_eq!(migrated.schema, Balance::SCHEMA);
        assert!(migrated.recipes.iter().all(|r| r.by_products.is_empty()));
        let mut future: Balance = serde_json::from_str(BALANCE_JSON).unwrap();
        future.schema = Balance::SCHEMA + 1;
        assert!(future.migrate().is_err());
        let mut ancient: Balance = serde_json::from_str(BALANCE_JSON).unwrap();
        ancient.schema = 0;
        assert!(ancient.migrate().is_err());
    }

    #[test]
    fn bad_by_products_are_rejected() {
        let mut dataset = Dataset::load().unwrap();
        let refinery = dataset.recipe_index["refinery"] as usize;
        dataset.recipes[refinery]
            .by_products
            .push(balance::ByProduct {
                product: "fuel".to_owned(),
                weekly_lots: 5,
            });
        dataset.recipes[refinery]
            .by_products
            .push(balance::ByProduct {
                product: "no_such_good".to_owned(),
                weekly_lots: 5,
            });
        let errors = dataset.validate();
        assert!(
            errors.iter().any(|e| e.contains("repeats the output")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.contains("is unknown")),
            "{errors:?}"
        );
    }

    #[test]
    fn no_firm_runs_two_strategies_of_one_sector_in_one_region() {
        let mut dataset = Dataset::load().unwrap();
        let firm = dataset
            .seed_firms
            .iter_mut()
            .find(|f| f.id == "cuyahoga_metal")
            .unwrap();
        firm.extra_plants.push(ExtraPlant {
            recipe: "steel_mill".to_owned(),
            region: firm.region.clone(),
            blocks: 1,
            serves: Vec::new(),
        });
        let errors = dataset.validate();
        assert!(
            errors.iter().any(|e| e.contains("one sector and region")),
            "{errors:?}"
        );
    }

    #[test]
    fn goods_that_were_import_only_now_have_a_local_producer() {
        let dataset = Dataset::load().unwrap();
        for product in [
            "natural_gas",
            "timber",
            "pharmaceuticals",
            "rare_earth",
            "building_materials",
        ] {
            let seeded = dataset.seed_firms.iter().any(|firm| {
                std::iter::once(firm.recipe.as_str())
                    .chain(firm.extra_plants.iter().map(|p| p.recipe.as_str()))
                    .filter_map(|id| dataset.recipe(id))
                    .any(|recipe| recipe.makes(product))
            });
            assert!(seeded, "{product} has no opening producer");
        }
    }
}
