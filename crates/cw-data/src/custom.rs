//! Custom scenarios: a player's edited copy of a built-in scenario's numbers.
//!
//! A custom scenario never invents content. It names a base scenario and overrides starting
//! capital, difficulty, the opening economy, which rival firms start and how large they are,
//! regional wage and demand levels, and the laws in force at the start. Files are versioned
//! JSON in the per-user data folder.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Dataset;

/// Current file format. Files from a newer version are refused rather than half read.
pub const FILE_VERSION: u32 = 1;

pub const CAPITAL_MIN_CENTS: i64 = 50_000_000;
pub const CAPITAL_MAX_CENTS: i64 = 50_000_000_000;
pub const RIVAL_SIZE_RANGE: (u16, u16) = (25, 300);
pub const RIVAL_CASH_RANGE: (u16, u16) = (25, 400);
pub const REGION_PCT_RANGE: (u16, u16) = (50, 200);
pub const TAX_RANGE_BPS: (i32, i32) = (0, 6_000);
pub const WAGE_FLOOR_RANGE_CENTS: (i64, i64) = (0, 100_000);
pub const MERGER_REVIEW_RANGE_BPS: (i64, i64) = (1_000, 10_000);
pub const NAME_MAX: usize = 40;

/// How world commodity prices move. Stored now; the price paths themselves come later.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricePath {
    #[default]
    Historical,
    HistoricalVariance,
    Random,
}

impl PricePath {
    pub const fn key(self) -> &'static str {
        match self {
            PricePath::Historical => "historical",
            PricePath::HistoricalVariance => "historical_variance",
            PricePath::Random => "random",
        }
    }

    pub fn from_key(key: &str) -> Option<PricePath> {
        match key {
            "historical" => Some(PricePath::Historical),
            "historical_variance" => Some(PricePath::HistoricalVariance),
            "random" => Some(PricePath::Random),
            _ => None,
        }
    }
}

/// One named rival: whether it starts, and its size and opening cash against standard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalSetting {
    pub firm_id: String,
    pub enabled: bool,
    pub size_pct: u16,
    pub cash_pct: u16,
}

/// A region's wage level and household demand against standard.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionAdjust {
    pub code: String,
    pub wage_pct: u16,
    pub demand_pct: u16,
}

/// Laws in force in week one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomLaws {
    pub corporate_tax_bps: i32,
    pub wage_floor_cents: i64,
    pub merger_review_bps: i64,
    /// "weak", "standard" or "strong".
    pub unions: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomScenario {
    pub version: u32,
    /// File-safe identifier, empty before the first save.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub base_scenario_id: String,
    /// "easy", "standard" or "hard".
    pub difficulty: String,
    /// "historic", "calm", "boom" or "downturn".
    pub economy: String,
    #[serde(default)]
    pub starting_capital_cents: Option<i64>,
    #[serde(default)]
    pub rivals: Vec<RivalSetting>,
    #[serde(default)]
    pub regions: Vec<RegionAdjust>,
    #[serde(default)]
    pub laws: Option<CustomLaws>,
    #[serde(default)]
    pub price_path: PricePath,
}

fn in_range<T: PartialOrd + std::fmt::Display + Copy>(
    errors: &mut Vec<String>,
    label: &str,
    value: T,
    range: (T, T),
) {
    if value < range.0 || value > range.1 {
        errors.push(format!(
            "{label} must be between {} and {}, not {value}",
            range.0, range.1
        ));
    }
}

impl CustomScenario {
    /// Every setting at its standard value for a base scenario.
    pub fn standard(dataset: &Dataset, base_scenario_id: &str, name: &str) -> CustomScenario {
        CustomScenario {
            version: FILE_VERSION,
            id: String::new(),
            name: name.to_owned(),
            base_scenario_id: base_scenario_id.to_owned(),
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            rivals: dataset
                .seed_firms
                .iter()
                .map(|firm| RivalSetting {
                    firm_id: firm.id.clone(),
                    enabled: true,
                    size_pct: 100,
                    cash_pct: 100,
                })
                .collect(),
            regions: dataset
                .regions
                .iter()
                .map(|region| RegionAdjust {
                    code: region.code.clone(),
                    wage_pct: 100,
                    demand_pct: 100,
                })
                .collect(),
            laws: None,
            price_path: PricePath::Historical,
        }
    }

    /// Every problem with the scenario, in plain words. Empty means it can be played.
    pub fn validate(&self, dataset: &Dataset) -> Vec<String> {
        let mut errors = Vec::new();
        if self.version == 0 || self.version > FILE_VERSION {
            errors.push(format!(
                "This scenario file is version {}, but this game reads up to version {FILE_VERSION}",
                self.version
            ));
        }
        let name = self.name.trim();
        if name.is_empty() {
            errors.push("Give the scenario a name".to_owned());
        } else if name.chars().count() > NAME_MAX {
            errors.push(format!("The name can be at most {NAME_MAX} characters"));
        }
        if !crate::scenarios()
            .iter()
            .any(|scenario| scenario.id == self.base_scenario_id && scenario.available)
        {
            errors.push(format!(
                "The base scenario \"{}\" is not a playable scenario",
                self.base_scenario_id
            ));
        }
        if !matches!(self.difficulty.as_str(), "easy" | "standard" | "hard") {
            errors.push(format!(
                "Difficulty must be easy, standard or hard, not \"{}\"",
                self.difficulty
            ));
        }
        if !matches!(
            self.economy.as_str(),
            "historic" | "calm" | "boom" | "downturn"
        ) {
            errors.push(format!(
                "The starting economy must be historic, calm, boom or downturn, not \"{}\"",
                self.economy
            ));
        }
        if let Some(capital) = self.starting_capital_cents {
            in_range(
                &mut errors,
                "Starting capital (cents)",
                capital,
                (CAPITAL_MIN_CENTS, CAPITAL_MAX_CENTS),
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut enabled = 0;
        for rival in &self.rivals {
            if !dataset
                .seed_firms
                .iter()
                .any(|firm| firm.id == rival.firm_id)
            {
                errors.push(format!("Unknown rival \"{}\"", rival.firm_id));
                continue;
            }
            if !seen.insert(rival.firm_id.as_str()) {
                errors.push(format!("Rival \"{}\" is listed twice", rival.firm_id));
            }
            if rival.enabled {
                enabled += 1;
            }
            in_range(
                &mut errors,
                &format!("Size of {}", rival.firm_id),
                rival.size_pct,
                RIVAL_SIZE_RANGE,
            );
            in_range(
                &mut errors,
                &format!("Opening cash of {}", rival.firm_id),
                rival.cash_pct,
                RIVAL_CASH_RANGE,
            );
        }
        // Rivals not listed start at standard, so only an explicit list can empty the board.
        let unlisted = dataset
            .seed_firms
            .iter()
            .filter(|firm| !self.rivals.iter().any(|r| r.firm_id == firm.id))
            .count();
        if enabled + unlisted < 3 {
            errors.push("At least three rival firms must start".to_owned());
        }
        let mut seen = std::collections::BTreeSet::new();
        for adjust in &self.regions {
            if !dataset
                .regions
                .iter()
                .any(|region| region.code == adjust.code)
            {
                errors.push(format!("Unknown region \"{}\"", adjust.code));
                continue;
            }
            if !seen.insert(adjust.code.as_str()) {
                errors.push(format!("Region \"{}\" is listed twice", adjust.code));
            }
            in_range(
                &mut errors,
                &format!("Wages in {}", adjust.code),
                adjust.wage_pct,
                REGION_PCT_RANGE,
            );
            in_range(
                &mut errors,
                &format!("Demand in {}", adjust.code),
                adjust.demand_pct,
                REGION_PCT_RANGE,
            );
        }
        if let Some(laws) = &self.laws {
            in_range(
                &mut errors,
                "Corporate tax (basis points)",
                laws.corporate_tax_bps,
                TAX_RANGE_BPS,
            );
            in_range(
                &mut errors,
                "Wage floor (cents a week)",
                laws.wage_floor_cents,
                WAGE_FLOOR_RANGE_CENTS,
            );
            in_range(
                &mut errors,
                "Merger review threshold (basis points)",
                laws.merger_review_bps,
                MERGER_REVIEW_RANGE_BPS,
            );
            if !matches!(laws.unions.as_str(), "weak" | "standard" | "strong") {
                errors.push(format!(
                    "Union law must be weak, standard or strong, not \"{}\"",
                    laws.unions
                ));
            }
        }
        errors
    }

    /// Apply rivals and regional levels to a loaded dataset. Validate first.
    pub fn apply_to(&self, dataset: &mut Dataset) {
        for rival in &self.rivals {
            if !rival.enabled {
                dataset.seed_firms.retain(|firm| firm.id != rival.firm_id);
            } else if let Some(firm) = dataset
                .seed_firms
                .iter_mut()
                .find(|firm| firm.id == rival.firm_id)
            {
                let blocks = (firm.blocks as u32 * rival.size_pct as u32 + 50) / 100;
                firm.blocks = blocks.clamp(1, 255) as u8;
                firm.cash_pct = rival.cash_pct;
            }
        }
        for adjust in &self.regions {
            if let Some(region) = dataset
                .regions
                .iter_mut()
                .find(|region| region.code == adjust.code)
            {
                region.wage_index_bps =
                    (region.wage_index_bps as i64 * adjust.wage_pct as i64 / 100) as i32;
                region.demand_index_bps =
                    (region.demand_index_bps as i64 * adjust.demand_pct as i64 / 100) as i32;
            }
        }
    }
}

/// A file-safe identifier from a display name.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    out.chars()
        .take(48)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn path_for(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// Save to `dir`, validating first. A blank id takes the name's slug, made unique. Returns the
/// stored scenario so the caller learns its id.
pub fn save(
    dir: &Path,
    dataset: &Dataset,
    scenario: &CustomScenario,
) -> Result<CustomScenario, Vec<String>> {
    let errors = scenario.validate(dataset);
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut stored = scenario.clone();
    stored.version = FILE_VERSION;
    stored.name = stored.name.trim().to_owned();
    if stored.id.is_empty() {
        let base = match slugify(&stored.name) {
            slug if slug.is_empty() => "scenario".to_owned(),
            slug => slug,
        };
        let mut candidate = base.clone();
        let mut n = 2;
        while path_for(dir, &candidate).exists() {
            candidate = format!("{base}-{n}");
            n += 1;
        }
        stored.id = candidate;
    } else if !valid_id(&stored.id) {
        return Err(vec!["The scenario id is not valid".to_owned()]);
    }
    fs::create_dir_all(dir).map_err(|e| vec![format!("Could not create the folder: {e}")])?;
    let json = serde_json::to_string_pretty(&stored)
        .map_err(|e| vec![format!("Could not encode the scenario: {e}")])?;
    let target = path_for(dir, &stored.id);
    let temp = dir.join(format!("{}.json.tmp", stored.id));
    fs::write(&temp, json).map_err(|e| vec![format!("Could not write the file: {e}")])?;
    fs::rename(&temp, &target).map_err(|e| vec![format!("Could not save the file: {e}")])?;
    Ok(stored)
}

pub fn load(dir: &Path, id: &str) -> Result<CustomScenario, String> {
    if !valid_id(id) {
        return Err("The scenario id is not valid".to_owned());
    }
    let text = fs::read_to_string(path_for(dir, id))
        .map_err(|_| "That custom scenario could not be found".to_owned())?;
    let scenario: CustomScenario = serde_json::from_str(&text)
        .map_err(|e| format!("That custom scenario file is damaged: {e}"))?;
    if scenario.version == 0 || scenario.version > FILE_VERSION {
        return Err(format!(
            "That scenario was made by a newer version of the game (file version {})",
            scenario.version
        ));
    }
    Ok(CustomScenario {
        id: id.to_owned(),
        ..scenario
    })
}

/// Every readable custom scenario in `dir`, by name. Damaged files are skipped.
pub fn list(dir: &Path) -> Vec<CustomScenario> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<CustomScenario> = entries
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension()?.to_str()? != "json" {
                return None;
            }
            load(dir, path.file_stem()?.to_str()?).ok()
        })
        .collect();
    found.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    found
}

pub fn delete(dir: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("The scenario id is not valid".to_owned());
    }
    fs::remove_file(path_for(dir, id))
        .map_err(|_| "That custom scenario could not be found".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dataset() -> Dataset {
        Dataset::load().expect("dataset")
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cw-custom-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn standard_scenario_is_valid_and_changes_nothing() {
        let data = dataset();
        let custom = CustomScenario::standard(&data, "great-lakes-2000", "Mine");
        assert!(custom.validate(&data).is_empty());
        let mut applied = dataset();
        custom.apply_to(&mut applied);
        assert_eq!(applied.compute_hash(), data.compute_hash());
        assert_eq!(applied.seed_firms.len(), data.seed_firms.len());
    }

    #[test]
    fn bad_values_each_get_a_clear_message() {
        let data = dataset();
        let mut custom = CustomScenario::standard(&data, "texas-1981", "");
        custom.difficulty = "brutal".into();
        custom.starting_capital_cents = Some(5);
        custom.rivals[0].size_pct = 900;
        custom.regions[0].wage_pct = 10;
        custom.regions.push(RegionAdjust {
            code: "NOPE".into(),
            wage_pct: 100,
            demand_pct: 100,
        });
        custom.laws = Some(CustomLaws {
            corporate_tax_bps: 9_000,
            wage_floor_cents: 34_000,
            merger_review_bps: 3_500,
            unions: "mighty".into(),
        });
        let errors = custom.validate(&data).join("\n");
        for needle in [
            "Give the scenario a name",
            "not a playable scenario",
            "Difficulty must be",
            "Starting capital",
            "Size of",
            "Wages in",
            "Unknown region",
            "Corporate tax",
            "Union law",
        ] {
            assert!(errors.contains(needle), "missing {needle}: {errors}");
        }
    }

    #[test]
    fn too_few_rivals_is_refused() {
        let data = dataset();
        let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Quiet");
        for rival in custom.rivals.iter_mut().skip(2) {
            rival.enabled = false;
        }
        assert!(
            custom
                .validate(&data)
                .iter()
                .any(|e| e.contains("At least three"))
        );
    }

    #[test]
    fn rivals_and_regions_are_applied() {
        let data = dataset();
        let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Edit");
        let first = custom.rivals[0].firm_id.clone();
        custom.rivals[0].enabled = false;
        custom.rivals[1].size_pct = 200;
        custom.rivals[1].cash_pct = 50;
        custom.regions[0].wage_pct = 150;
        let mut edited = dataset();
        custom.apply_to(&mut edited);
        assert!(!edited.seed_firms.iter().any(|f| f.id == first));
        let second = &edited
            .seed_firms
            .iter()
            .find(|f| f.id == custom.rivals[1].firm_id)
            .unwrap();
        let original = data.seed_firms.iter().find(|f| f.id == second.id).unwrap();
        assert_eq!(second.blocks as u32, original.blocks as u32 * 2);
        assert_eq!(second.cash_pct, 50);
        assert_eq!(
            edited.regions[0].wage_index_bps as i64,
            data.regions[0].wage_index_bps as i64 * 3 / 2
        );
    }

    #[test]
    fn save_load_list_delete_round_trip() {
        let data = dataset();
        let dir = temp_dir("rt");
        let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Hard Times!");
        custom.price_path = PricePath::HistoricalVariance;
        let first = save(&dir, &data, &custom).unwrap();
        assert_eq!(first.id, "hard-times");
        let second = save(&dir, &data, &custom).unwrap();
        assert_eq!(second.id, "hard-times-2");
        assert_eq!(load(&dir, "hard-times").unwrap(), first);
        assert_eq!(list(&dir).len(), 2);
        fs::write(dir.join("junk.json"), "{nope").unwrap();
        assert_eq!(list(&dir).len(), 2);
        delete(&dir, "hard-times").unwrap();
        assert!(load(&dir, "hard-times").is_err());
        assert!(delete(&dir, "../etc").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn newer_file_versions_are_refused() {
        let data = dataset();
        let dir = temp_dir("ver");
        let mut custom = CustomScenario::standard(&data, "great-lakes-2000", "Future");
        custom.version = 99;
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("future.json"),
            serde_json::to_string(&custom).unwrap(),
        )
        .unwrap();
        assert!(load(&dir, "future").unwrap_err().contains("newer version"));
        assert!(save(&dir, &data, &custom).is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
