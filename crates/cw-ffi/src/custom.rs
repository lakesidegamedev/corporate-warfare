//! Scenario editor surface: edit a built-in scenario's numbers and keep the result as a
//! custom scenario in the per-user data folder.

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use cw_data::Dataset;
use cw_data::custom::{self, CustomLaws, CustomScenario, PricePath, RegionAdjust, RivalSetting};
use cw_session::Session;

use crate::{FfiError, SessionHandle};

fn dataset() -> Result<&'static Dataset, FfiError> {
    static DATASET: OnceLock<Result<Dataset, String>> = OnceLock::new();
    DATASET
        .get_or_init(|| Dataset::load().map_err(|errors| errors.join("; ")))
        .as_ref()
        .map_err(|detail| FfiError::Internal {
            detail: detail.clone(),
        })
}

fn invalid(errors: Vec<String>) -> FfiError {
    FfiError::Domain {
        key: "custom_scenario_invalid".to_owned(),
        detail: errors.join("\n"),
    }
}

#[derive(Clone, uniffi::Record)]
pub struct RivalDto {
    pub firm_id: String,
    pub name: String,
    pub industry: String,
    pub region: String,
    pub enabled: bool,
    pub size_pct: u16,
    pub cash_pct: u16,
}

#[derive(Clone, uniffi::Record)]
pub struct RegionAdjustDto {
    pub code: String,
    pub name: String,
    pub wage_pct: u16,
    pub demand_pct: u16,
}

#[derive(Clone, uniffi::Record)]
pub struct CustomLawsDto {
    pub corporate_tax_bps: i32,
    pub wage_floor_cents: i64,
    pub merger_review_bps: i64,
    /// "weak", "standard" or "strong".
    pub unions: String,
}

#[derive(Clone, uniffi::Record)]
pub struct CustomScenarioDto {
    /// Empty until the first save.
    pub id: String,
    pub name: String,
    pub base_scenario_id: String,
    pub difficulty: String,
    pub economy: String,
    /// None means the difficulty's own capital.
    pub starting_capital_cents: Option<i64>,
    pub rivals: Vec<RivalDto>,
    pub regions: Vec<RegionAdjustDto>,
    /// None means the standard laws.
    pub laws: Option<CustomLawsDto>,
    /// "historical", "historical_variance" or "random".
    pub price_path: String,
}

/// Slider limits, so every platform draws the same bounds the engine enforces.
#[derive(Clone, uniffi::Record)]
pub struct CustomRangesDto {
    pub capital_min_cents: i64,
    pub capital_max_cents: i64,
    pub rival_size_min: u16,
    pub rival_size_max: u16,
    pub rival_cash_min: u16,
    pub rival_cash_max: u16,
    pub region_pct_min: u16,
    pub region_pct_max: u16,
    pub tax_min_bps: i32,
    pub tax_max_bps: i32,
    pub wage_floor_min_cents: i64,
    pub wage_floor_max_cents: i64,
    pub merger_min_bps: i64,
    pub merger_max_bps: i64,
    pub name_max: u32,
    pub min_rivals: u32,
}

/// The live summary line numbers and any problems.
#[derive(Clone, uniffi::Record)]
pub struct CustomSummaryDto {
    pub starting_capital_cents: i64,
    pub rivals_started: u32,
    pub rivals_total: u32,
    pub average_wage_pct: u32,
    pub average_demand_pct: u32,
    pub problems: Vec<String>,
}

fn to_dto(data: &Dataset, scenario: &CustomScenario) -> CustomScenarioDto {
    // Always list every rival and region so the editor shows the full set.
    let standard = CustomScenario::standard(data, &scenario.base_scenario_id, &scenario.name);
    let rivals = standard
        .rivals
        .iter()
        .map(|base| {
            let set = scenario
                .rivals
                .iter()
                .find(|r| r.firm_id == base.firm_id)
                .unwrap_or(base);
            let firm = data.seed_firms.iter().find(|f| f.id == base.firm_id);
            RivalDto {
                firm_id: base.firm_id.clone(),
                name: firm.map(|f| f.name.clone()).unwrap_or_default(),
                industry: firm.map(|f| f.industry.clone()).unwrap_or_default(),
                region: firm.map(|f| f.region.clone()).unwrap_or_default(),
                enabled: set.enabled,
                size_pct: set.size_pct,
                cash_pct: set.cash_pct,
            }
        })
        .collect();
    let regions = standard
        .regions
        .iter()
        .map(|base| {
            let set = scenario
                .regions
                .iter()
                .find(|r| r.code == base.code)
                .unwrap_or(base);
            RegionAdjustDto {
                code: base.code.clone(),
                name: data
                    .regions
                    .iter()
                    .find(|r| r.code == base.code)
                    .map(|r| r.name.clone())
                    .unwrap_or_default(),
                wage_pct: set.wage_pct,
                demand_pct: set.demand_pct,
            }
        })
        .collect();
    CustomScenarioDto {
        id: scenario.id.clone(),
        name: scenario.name.clone(),
        base_scenario_id: scenario.base_scenario_id.clone(),
        difficulty: scenario.difficulty.clone(),
        economy: scenario.economy.clone(),
        starting_capital_cents: scenario.starting_capital_cents,
        rivals,
        regions,
        laws: scenario.laws.as_ref().map(|l| CustomLawsDto {
            corporate_tax_bps: l.corporate_tax_bps,
            wage_floor_cents: l.wage_floor_cents,
            merger_review_bps: l.merger_review_bps,
            unions: l.unions.clone(),
        }),
        price_path: scenario.price_path.key().to_owned(),
    }
}

fn from_dto(dto: &CustomScenarioDto) -> Result<CustomScenario, FfiError> {
    let price_path = PricePath::from_key(&dto.price_path).ok_or_else(|| {
        invalid(vec![format!(
            "The world price path must be historical, historical_variance or random, not \"{}\"",
            dto.price_path
        )])
    })?;
    Ok(CustomScenario {
        version: custom::FILE_VERSION,
        id: dto.id.clone(),
        name: dto.name.clone(),
        base_scenario_id: dto.base_scenario_id.clone(),
        difficulty: dto.difficulty.clone(),
        economy: dto.economy.clone(),
        starting_capital_cents: dto.starting_capital_cents,
        rivals: dto
            .rivals
            .iter()
            .map(|r| RivalSetting {
                firm_id: r.firm_id.clone(),
                enabled: r.enabled,
                size_pct: r.size_pct,
                cash_pct: r.cash_pct,
            })
            .collect(),
        regions: dto
            .regions
            .iter()
            .map(|r| RegionAdjust {
                code: r.code.clone(),
                wage_pct: r.wage_pct,
                demand_pct: r.demand_pct,
            })
            .collect(),
        laws: dto.laws.as_ref().map(|l| CustomLaws {
            corporate_tax_bps: l.corporate_tax_bps,
            wage_floor_cents: l.wage_floor_cents,
            merger_review_bps: l.merger_review_bps,
            unions: l.unions.clone(),
        }),
        price_path,
    })
}

/// Slider limits the engine enforces.
#[uniffi::export]
pub fn custom_scenario_ranges() -> CustomRangesDto {
    CustomRangesDto {
        capital_min_cents: custom::CAPITAL_MIN_CENTS,
        capital_max_cents: custom::CAPITAL_MAX_CENTS,
        rival_size_min: custom::RIVAL_SIZE_RANGE.0,
        rival_size_max: custom::RIVAL_SIZE_RANGE.1,
        rival_cash_min: custom::RIVAL_CASH_RANGE.0,
        rival_cash_max: custom::RIVAL_CASH_RANGE.1,
        region_pct_min: custom::REGION_PCT_RANGE.0,
        region_pct_max: custom::REGION_PCT_RANGE.1,
        tax_min_bps: custom::TAX_RANGE_BPS.0,
        tax_max_bps: custom::TAX_RANGE_BPS.1,
        wage_floor_min_cents: custom::WAGE_FLOOR_RANGE_CENTS.0,
        wage_floor_max_cents: custom::WAGE_FLOOR_RANGE_CENTS.1,
        merger_min_bps: custom::MERGER_REVIEW_RANGE_BPS.0,
        merger_max_bps: custom::MERGER_REVIEW_RANGE_BPS.1,
        name_max: custom::NAME_MAX as u32,
        min_rivals: 3,
    }
}

/// A standard-valued copy of a built-in scenario, ready to edit. The laws are filled with the
/// standard laws so the editor has numbers to show.
#[uniffi::export]
pub fn custom_scenario_default(
    base_scenario_id: String,
    name: String,
) -> Result<CustomScenarioDto, FfiError> {
    let data = dataset()?;
    let mut scenario = CustomScenario::standard(data, &base_scenario_id, &name);
    let laws = cw_sim::state::Laws::default();
    scenario.laws = Some(CustomLaws {
        corporate_tax_bps: laws.corporate_tax_bps,
        wage_floor_cents: laws.wage_floor_cents,
        merger_review_bps: laws.merger_review_bps,
        unions: laws.unions.key().to_owned(),
    });
    let problems = scenario.validate(data);
    if problems.iter().any(|p| p.contains("base scenario")) {
        return Err(invalid(problems));
    }
    Ok(to_dto(data, &scenario))
}

/// Live summary and problems for what is on screen.
#[uniffi::export]
pub fn custom_scenario_summary(scenario: CustomScenarioDto) -> Result<CustomSummaryDto, FfiError> {
    let data = dataset()?;
    let model = from_dto(&scenario)?;
    let mean = |values: Vec<u32>| -> u32 {
        if values.is_empty() {
            100
        } else {
            values.iter().sum::<u32>() / values.len() as u32
        }
    };
    let capital = model.starting_capital_cents.unwrap_or_else(|| {
        cw_data::Difficulty::from_key(&model.difficulty).starting_capital_cents()
    });
    Ok(CustomSummaryDto {
        starting_capital_cents: capital,
        rivals_started: scenario.rivals.iter().filter(|r| r.enabled).count() as u32,
        rivals_total: scenario.rivals.len() as u32,
        average_wage_pct: mean(scenario.regions.iter().map(|r| r.wage_pct as u32).collect()),
        average_demand_pct: mean(
            scenario
                .regions
                .iter()
                .map(|r| r.demand_pct as u32)
                .collect(),
        ),
        problems: model.validate(data),
    })
}

/// Save to the folder, returning the stored scenario with its id.
#[uniffi::export]
pub fn custom_scenario_save(
    dir: String,
    scenario: CustomScenarioDto,
) -> Result<CustomScenarioDto, FfiError> {
    let data = dataset()?;
    let model = from_dto(&scenario)?;
    let stored = custom::save(Path::new(&dir), data, &model).map_err(invalid)?;
    Ok(to_dto(data, &stored))
}

/// Every saved custom scenario, by name.
#[uniffi::export]
pub fn custom_scenario_list(dir: String) -> Vec<CustomScenarioDto> {
    let Ok(data) = dataset() else {
        return Vec::new();
    };
    custom::list(Path::new(&dir))
        .iter()
        .map(|scenario| to_dto(data, scenario))
        .collect()
}

#[uniffi::export]
pub fn custom_scenario_delete(dir: String, id: String) -> Result<(), FfiError> {
    custom::delete(Path::new(&dir), &id).map_err(|detail| invalid(vec![detail]))
}

#[uniffi::export]
impl SessionHandle {
    /// A new career from a custom scenario. The scenario's own difficulty, economy and capital
    /// apply.
    #[uniffi::constructor]
    pub fn start_custom(
        seed: u64,
        guided: bool,
        five_year: bool,
        scenario: CustomScenarioDto,
    ) -> Result<Arc<SessionHandle>, FfiError> {
        let model = from_dto(&scenario)?;
        let config = cw_session::ScenarioConfig {
            seed,
            scenario: if guided {
                cw_sim::state::ScenarioKind::Guided
            } else {
                cw_sim::state::ScenarioKind::Founder
            },
            mode: if five_year {
                cw_sim::state::ScenarioMode::FiveYear
            } else {
                cw_sim::state::ScenarioMode::Sandbox
            },
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
        };
        let session = Session::create_custom(config, &model)
            .map_err(|detail| invalid(detail.split("; ").map(str::to_owned).collect()))?;
        Ok(Arc::new(SessionHandle {
            inner: Mutex::new(session),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_save_list_start_round_trip() {
        let dir = std::env::temp_dir().join(format!("cw-ffi-custom-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let dir_text = dir.to_string_lossy().to_string();
        let mut dto =
            custom_scenario_default("great-lakes-2000".into(), "Lean Lakes".into()).unwrap();
        assert!(!dto.rivals.is_empty() && !dto.regions.is_empty());
        dto.starting_capital_cents = Some(250_000_000);
        dto.rivals[0].enabled = false;
        dto.price_path = "random".into();
        let summary = custom_scenario_summary(dto.clone()).unwrap();
        assert!(summary.problems.is_empty(), "{:?}", summary.problems);
        assert_eq!(summary.rivals_started + 1, summary.rivals_total);
        let saved = custom_scenario_save(dir_text.clone(), dto).unwrap();
        assert_eq!(saved.id, "lean-lakes");
        let listed = custom_scenario_list(dir_text.clone());
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].price_path, "random");
        let handle = SessionHandle::start_custom(5, false, false, listed[0].clone()).unwrap();
        assert_eq!(handle.overview().investor_cash_cents, 250_000_000);
        custom_scenario_delete(dir_text.clone(), "lean-lakes".into()).unwrap();
        assert!(custom_scenario_list(dir_text).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_input_gives_plain_errors() {
        let mut dto = custom_scenario_default("great-lakes-2000".into(), "X".into()).unwrap();
        dto.regions[0].demand_pct = 999;
        let err = custom_scenario_save("/nonexistent-cw".into(), dto)
            .err()
            .unwrap();
        assert!(err.to_string().contains("Demand in"), "{err}");
        assert!(custom_scenario_default("texas-1981".into(), "T".into()).is_err());
    }
}
