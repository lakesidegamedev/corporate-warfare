//! World commodity price paths: the outside world's prices for oil, steel, grain and the rest.
//!
//! Content lives in `content/price-paths.json`. Each scenario lists anchor points per price
//! family as `[quarters since the start year began, index]`, where 100 is the price level on day
//! one. The engine reads them when a career starts and keeps its own copy in the save, so this
//! file is not part of the dataset hash. Values are relative indices, not market history.

use std::collections::BTreeMap;

use serde::Deserialize;

pub const PRICE_PATHS_JSON: &str = include_str!("../../../content/price-paths.json");

/// How a price family is described to the player and weighted in the world demand signal.
#[derive(Clone, Debug, Deserialize)]
pub struct FamilyInfo {
    pub label: String,
    /// Share of the world demand signal that feeds the business cycle.
    pub weight: u32,
    /// How strongly the business cycle tilts this family, in percent.
    pub cyclical_pct: u32,
    /// A family only some scenarios have: it joins a game only when the scenario authors a path
    /// for it, so scenarios that predate it play exactly as they did.
    #[serde(default)]
    pub listed_only: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ScenarioPaths {
    #[serde(default)]
    pub note: String,
    /// Anchor points per family: `[quarter, index]`.
    pub families: BTreeMap<String, Vec<[i32; 2]>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PricePathFile {
    pub schema: u32,
    #[serde(default)]
    pub note: String,
    pub families: BTreeMap<String, FamilyInfo>,
    /// Product id to family. Ids that are not in the current content are ignored.
    pub products: BTreeMap<String, String>,
    pub scenarios: BTreeMap<String, ScenarioPaths>,
}

impl PricePathFile {
    pub fn load() -> PricePathFile {
        serde_json::from_str(PRICE_PATHS_JSON).expect("content/price-paths.json is valid")
    }

    /// Problems with the content, in plain words. Empty means usable.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for (product, family) in &self.products {
            if !self.families.contains_key(family) {
                errors.push(format!("product {product} names unknown family {family}"));
            }
        }
        for (scenario, paths) in &self.scenarios {
            for (family, anchors) in &paths.families {
                if !self.families.contains_key(family) {
                    errors.push(format!("{scenario} lists unknown family {family}"));
                }
                if anchors.is_empty() {
                    errors.push(format!("{scenario}/{family} has no anchors"));
                }
                if anchors.windows(2).any(|pair| pair[0][0] >= pair[1][0]) {
                    errors.push(format!("{scenario}/{family} anchors must rise in time"));
                }
                if anchors.first().map(|a| a[0]) != Some(0) {
                    errors.push(format!("{scenario}/{family} must start at quarter 0"));
                }
                if anchors.first().map(|a| a[1]) != Some(100) {
                    errors.push(format!("{scenario}/{family} must start at index 100"));
                }
                if anchors.iter().any(|a| !(10..=1_000).contains(&a[1])) {
                    errors.push(format!("{scenario}/{family} index outside 10 to 1000"));
                }
            }
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_price_paths_are_valid() {
        let file = PricePathFile::load();
        assert_eq!(file.validate(), Vec::<String>::new());
        assert!(file.scenarios.contains_key("great-lakes-2000"));
        assert!(file.scenarios.contains_key("texas-1981"));
        assert_eq!(file.products["fuel"], "oil");
    }

    #[test]
    fn texas_oil_falls_to_a_third_by_the_1986_collapse() {
        let file = PricePathFile::load();
        let oil = &file.scenarios["texas-1981"].families["oil"];
        let low = oil.iter().map(|a| a[1]).min().unwrap();
        assert!((30..=45).contains(&low), "oil trough index {low}");
    }

    #[test]
    fn silicon_valley_chips_peak_in_1995_spike_in_2000_and_bust_after() {
        let file = PricePathFile::load();
        assert!(file.families["electronics"].listed_only);
        assert_eq!(file.products["components"], "electronics");
        let chips = &file.scenarios["silicon-valley-1995"].families["electronics"];
        let at = |quarter: i32| {
            chips
                .iter()
                .find(|a| a[0] == quarter)
                .map(|a| a[1])
                .unwrap()
        };
        // Quarter 20 is the start of 2000: a spike off the 1998 trough, then the 2001 bust.
        assert!(at(20) > at(16), "2000 spike");
        assert!(at(26) < at(20) * 3 / 4, "2001 bust");
        // Oil and grain stay modest next to the chip swings.
        let oil = &file.scenarios["silicon-valley-1995"].families["oil"];
        assert!(oil.iter().all(|a| (60..=250).contains(&a[1])));
    }
}
