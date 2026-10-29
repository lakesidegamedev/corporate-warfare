//! Scenario timelines: the dated events a scenario's history puts in front of the player.
//!
//! Content lives in `content/scenarios/<id>/timeline.json`. Each event has a date (year and week
//! of the year), a plain-English title and summary, a foreshadow lead time and a list of
//! effects. Effects are drawn only from mechanics the engine already has: a world price shock, a
//! demand regime, a central bank stance shift, a credit tightening, a law change, a systemic
//! event and an import ceiling change. The engine copies the events it plays into the save, so
//! these files are not part of the dataset hash.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::Dataset;
use crate::price_paths::PricePathFile;

/// Years from the scenario's first year that an event date may fall in.
pub const SPAN_YEARS: i32 = 15;
pub const WEEKS_PER_YEAR: u32 = 52;
/// The longest lead time a foreshadow notice may have.
pub const MAX_FORESHADOW_WEEKS: u32 = 52;

/// Magnitude caps, in line with the macro caps in `docs/rules/CONTENT-PACING.md`: a regime is at
/// most the size of a recession or boom (10 percent) and the economy never carries more than
/// one of an effect kind from a single event.
pub const MAX_DEMAND_PERCENT: i32 = 10;
pub const MAX_REGIME_WEEKS: u32 = 52;
pub const MAX_PRICE_SHOCK_PERCENT: i32 = 40;
pub const MAX_SHOCK_WEEKS: u32 = 52;
pub const MAX_STANCE_SHIFT: i32 = 40;
pub const MAX_SPREAD_BPS: i32 = 300;
pub const MAX_CREDIT_WEEKS: u32 = 78;
pub const MAX_CEILING_PERCENT: i32 = 50;

/// The systemic event kinds a timeline may trigger, by the keys the engine records them under.
pub const SYSTEMIC_KINDS: [&str; 5] = [
    "spill",
    "port_strike",
    "refinery_fire",
    "lane_outage",
    "harvest_failure",
];

/// The laws a timeline may change, by id, with the range each accepts. `unions` takes 0 for
/// weak, 1 for standard and 2 for strong; the yes-or-no laws take 0 or 1.
pub const LAWS: [(&str, i64, i64); 9] = [
    ("corporate_tax_bps", 1_000, 4_500),
    ("wage_floor_cents", 10_000, 120_000),
    ("merger_review_bps", 1_500, 6_000),
    ("unions", 0, 2),
    ("common_carrier", 0, 1),
    ("vertical_review_bps", 0, 6_000),
    ("reserve", 0, 1),
    ("privatise", 0, 1),
    ("quota_bps", 0, 9_000),
];

/// A date in the scenario's own calendar: the year and the week of that year (1 to 52).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Date {
    pub year: i32,
    pub week: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    /// The world price family of `good` moves by `percent` at once, and the move fades out
    /// evenly over `weeks`.
    PriceShock {
        good: String,
        percent: i32,
        weeks: u32,
    },
    /// Household demand moves by `percent` for `weeks`, for one industry or for `"all"`.
    DemandRegime {
        sector: String,
        percent: i32,
        weeks: u32,
    },
    /// The central bank chair's stance moves by `shift` points (positive is more hawkish).
    CentralBankStance { shift: i32 },
    /// Lenders add `spread_bps` to every rate for `weeks`.
    CreditTightening { spread_bps: i32, weeks: u32 },
    /// A law takes a new value. See [`LAWS`] for the ids and ranges.
    LawChange { law: String, value: i64 },
    /// One of the systemic events strikes the first place it can.
    SystemicEvent { event: String },
    /// The outside world's gateway capacity for `good` changes by `percent`, for good.
    ImportCeiling { good: String, percent: i32 },
}

impl Effect {
    pub const fn key(&self) -> &'static str {
        match self {
            Effect::PriceShock { .. } => "price_shock",
            Effect::DemandRegime { .. } => "demand_regime",
            Effect::CentralBankStance { .. } => "central_bank_stance",
            Effect::CreditTightening { .. } => "credit_tightening",
            Effect::LawChange { .. } => "law_change",
            Effect::SystemicEvent { .. } => "systemic_event",
            Effect::ImportCeiling { .. } => "import_ceiling",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub date: Date,
    pub title: String,
    pub summary: String,
    /// Weeks before the date that a notice goes out. Zero means no notice.
    pub foreshadow_weeks: u32,
    pub effects: Vec<Effect>,
}

impl TimelineEvent {
    /// The week the event fires, counted the way the engine's clock counts: week 1 of the
    /// scenario's first year is tick 1.
    pub fn tick(&self, start_year: i32) -> u32 {
        ((self.date.year - start_year) as u32) * WEEKS_PER_YEAR + self.date.week
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineFile {
    pub schema: u32,
    #[serde(default)]
    pub note: String,
    pub events: Vec<TimelineEvent>,
}

/// The timeline file of a scenario, if it has one.
pub fn timeline_json(scenario_id: &str) -> Option<&'static str> {
    match scenario_id {
        "great-lakes-2000" => Some(include_str!(
            "../../../content/scenarios/great-lakes-2000/timeline.json"
        )),
        "texas-1981" => Some(include_str!(
            "../../../content/scenarios/texas-1981/timeline.json"
        )),
        "silicon-valley-1995" => Some(include_str!(
            "../../../content/scenarios/silicon-valley-1995/timeline.json"
        )),
        "ruhr-rhine-1991" => Some(include_str!(
            "../../../content/scenarios/ruhr-rhine-1991/timeline.json"
        )),
        _ => None,
    }
}

/// The year a scenario's calendar starts in.
pub fn start_year(scenario_id: &str) -> Option<i32> {
    crate::scenarios()
        .iter()
        .find(|s| s.id == scenario_id)
        .map(|s| s.start_year as i32)
}

impl TimelineFile {
    /// Parse and validate a scenario's timeline. `Ok(None)` means the scenario has none.
    pub fn load(scenario_id: &str, dataset: &Dataset) -> Result<Option<TimelineFile>, Vec<String>> {
        let Some(json) = timeline_json(scenario_id) else {
            return Ok(None);
        };
        let file: TimelineFile = serde_json::from_str(json)
            .map_err(|error| vec![format!("{scenario_id} timeline.json: {error}")])?;
        let errors = file.validate(scenario_id, dataset);
        if errors.is_empty() {
            Ok(Some(file))
        } else {
            Err(errors)
        }
    }

    /// Problems with the content, in plain words. Empty means usable.
    pub fn validate(&self, scenario_id: &str, dataset: &Dataset) -> Vec<String> {
        let mut errors = Vec::new();
        let Some(start) = start_year(scenario_id) else {
            return vec![format!("{scenario_id} is not a known scenario")];
        };
        let paths = PricePathFile::load();
        let scenario_paths = paths.scenarios.get(scenario_id);
        let mut ids = BTreeSet::new();
        if self.events.is_empty() {
            errors.push(format!("{scenario_id} timeline has no events"));
        }
        for event in &self.events {
            let at = format!("{scenario_id}/{}", event.id);
            if event.id.is_empty()
                || !event
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                errors.push(format!("{at}: ids are lowercase words joined by hyphens"));
            }
            if !ids.insert(event.id.as_str()) {
                errors.push(format!("{at}: duplicate id"));
            }
            if event.title.trim().is_empty() || event.summary.trim().is_empty() {
                errors.push(format!("{at}: title and summary are required"));
            }
            if event.title.chars().count() > 60 || event.summary.chars().count() > 260 {
                errors.push(format!(
                    "{at}: title over 60 or summary over 260 characters"
                ));
            }
            if event.title.contains(['\u{2013}', '\u{2014}'])
                || event.summary.contains(['\u{2013}', '\u{2014}'])
            {
                errors.push(format!("{at}: no dashes in player copy"));
            }
            if !(start..start + SPAN_YEARS).contains(&event.date.year)
                || !(1..=WEEKS_PER_YEAR).contains(&event.date.week)
            {
                errors.push(format!(
                    "{at}: date {}/{} is outside the scenario's span ({} to {}, weeks 1 to 52)",
                    event.date.year,
                    event.date.week,
                    start,
                    start + SPAN_YEARS - 1
                ));
            } else if event.tick(start) < 2 {
                errors.push(format!("{at}: the first week cannot hold an event"));
            }
            if event.foreshadow_weeks > MAX_FORESHADOW_WEEKS {
                errors.push(format!(
                    "{at}: foreshadow over {MAX_FORESHADOW_WEEKS} weeks"
                ));
            }
            if event.effects.is_empty() {
                errors.push(format!("{at}: an event needs at least one effect"));
            }
            for effect in &event.effects {
                self.validate_effect(&at, effect, dataset, &paths, scenario_paths, &mut errors);
            }
        }
        errors
    }

    fn validate_effect(
        &self,
        at: &str,
        effect: &Effect,
        dataset: &Dataset,
        paths: &PricePathFile,
        scenario_paths: Option<&crate::price_paths::ScenarioPaths>,
        errors: &mut Vec<String>,
    ) {
        let known_good = |good: &str| dataset.product_index.contains_key(good);
        match effect {
            Effect::PriceShock {
                good,
                percent,
                weeks,
            } => {
                if !known_good(good) {
                    errors.push(format!("{at}: price shock names unknown good {good}"));
                } else if let Some(family) = paths.products.get(good) {
                    let listed_only = paths.families.get(family).is_some_and(|f| f.listed_only);
                    if listed_only
                        && !scenario_paths.is_some_and(|s| s.families.contains_key(family))
                    {
                        errors.push(format!("{at}: {good} has no world price in this scenario"));
                    }
                } else {
                    errors.push(format!("{at}: {good} has no world price family"));
                }
                if *percent == 0 || percent.abs() > MAX_PRICE_SHOCK_PERCENT {
                    errors.push(format!(
                        "{at}: price shock must be 1 to {MAX_PRICE_SHOCK_PERCENT} percent"
                    ));
                }
                if !(1..=MAX_SHOCK_WEEKS).contains(weeks) {
                    errors.push(format!(
                        "{at}: price shock weeks must be 1 to {MAX_SHOCK_WEEKS}"
                    ));
                }
            }
            Effect::DemandRegime {
                sector,
                percent,
                weeks,
            } => {
                if sector != "all" && !crate::INDUSTRIES.contains(&sector.as_str()) {
                    errors.push(format!("{at}: demand regime names unknown sector {sector}"));
                }
                if *percent == 0 || percent.abs() > MAX_DEMAND_PERCENT {
                    errors.push(format!(
                        "{at}: demand regime must be 1 to {MAX_DEMAND_PERCENT} percent"
                    ));
                }
                if !(1..=MAX_REGIME_WEEKS).contains(weeks) {
                    errors.push(format!(
                        "{at}: demand regime weeks must be 1 to {MAX_REGIME_WEEKS}"
                    ));
                }
            }
            Effect::CentralBankStance { shift } => {
                if *shift == 0 || shift.abs() > MAX_STANCE_SHIFT {
                    errors.push(format!(
                        "{at}: stance shift must be 1 to {MAX_STANCE_SHIFT} points"
                    ));
                }
            }
            Effect::CreditTightening { spread_bps, weeks } => {
                if !(1..=MAX_SPREAD_BPS).contains(spread_bps) {
                    errors.push(format!(
                        "{at}: credit spread must be 1 to {MAX_SPREAD_BPS} basis points"
                    ));
                }
                if !(1..=MAX_CREDIT_WEEKS).contains(weeks) {
                    errors.push(format!(
                        "{at}: credit tightening weeks must be 1 to {MAX_CREDIT_WEEKS}"
                    ));
                }
            }
            Effect::LawChange { law, value } => match LAWS.iter().find(|(id, _, _)| id == law) {
                None => errors.push(format!("{at}: unknown law {law}")),
                Some((_, low, high)) if !(*low..=*high).contains(value) => {
                    errors.push(format!("{at}: {law} must be {low} to {high}"));
                }
                Some(_) => {}
            },
            Effect::SystemicEvent { event } => {
                if !SYSTEMIC_KINDS.contains(&event.as_str()) {
                    errors.push(format!("{at}: unknown systemic event {event}"));
                }
            }
            Effect::ImportCeiling { good, percent } => {
                if !known_good(good) {
                    errors.push(format!("{at}: import ceiling names unknown good {good}"));
                }
                if *percent == 0 || percent.abs() > MAX_CEILING_PERCENT {
                    errors.push(format!(
                        "{at}: import ceiling must be 1 to {MAX_CEILING_PERCENT} percent"
                    ));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dataset() -> Dataset {
        Dataset::load().unwrap()
    }

    fn one(effect: Effect) -> TimelineFile {
        TimelineFile {
            schema: 1,
            note: String::new(),
            events: vec![TimelineEvent {
                id: "test-event".into(),
                date: Date {
                    year: 2002,
                    week: 10,
                },
                title: "Test".into(),
                summary: "A test.".into(),
                foreshadow_weeks: 4,
                effects: vec![effect],
            }],
        }
    }

    #[test]
    fn every_shipped_timeline_is_valid_and_sized() {
        for scenario in crate::scenarios() {
            let ds = Dataset::load_scenario(&scenario.id).unwrap();
            let file = TimelineFile::load(&scenario.id, &ds)
                .unwrap_or_else(|e| panic!("{}: {e:?}", scenario.id))
                .unwrap_or_else(|| panic!("{} has no timeline", scenario.id));
            assert!(
                (8..=12).contains(&file.events.len()),
                "{}: {} events",
                scenario.id,
                file.events.len()
            );
        }
    }

    #[test]
    fn unknown_good_law_and_kind_are_rejected() {
        let ds = dataset();
        let bad_good = one(Effect::PriceShock {
            good: "unobtainium".into(),
            percent: 10,
            weeks: 8,
        });
        assert!(
            bad_good
                .validate("great-lakes-2000", &ds)
                .iter()
                .any(|e| e.contains("unknown good"))
        );
        let bad_law = one(Effect::LawChange {
            law: "free_beer".into(),
            value: 1,
        });
        assert!(
            bad_law
                .validate("great-lakes-2000", &ds)
                .iter()
                .any(|e| e.contains("unknown law"))
        );
        let bad_kind = one(Effect::SystemicEvent {
            event: "meteor".into(),
        });
        assert!(
            bad_kind
                .validate("great-lakes-2000", &ds)
                .iter()
                .any(|e| e.contains("unknown systemic event"))
        );
        let bad_ceiling = one(Effect::ImportCeiling {
            good: "unobtainium".into(),
            percent: -10,
        });
        assert!(!bad_ceiling.validate("great-lakes-2000", &ds).is_empty());
    }

    #[test]
    fn dates_outside_the_span_are_rejected() {
        let ds = dataset();
        let mut file = one(Effect::CentralBankStance { shift: 10 });
        for (year, week) in [(1999, 10), (2015, 10), (2002, 0), (2002, 53)] {
            file.events[0].date = Date { year, week };
            assert!(
                file.validate("great-lakes-2000", &ds)
                    .iter()
                    .any(|e| e.contains("outside the scenario's span")),
                "{year}/{week}"
            );
        }
        file.events[0].date = Date {
            year: 2000,
            week: 1,
        };
        assert!(!file.validate("great-lakes-2000", &ds).is_empty());
        file.events[0].date = Date {
            year: 2002,
            week: 10,
        };
        assert_eq!(file.validate("great-lakes-2000", &ds), Vec::<String>::new());
    }

    #[test]
    fn magnitudes_stay_inside_the_pacing_caps() {
        let ds = dataset();
        let too_big = [
            Effect::DemandRegime {
                sector: "all".into(),
                percent: -11,
                weeks: 26,
            },
            Effect::PriceShock {
                good: "fuel".into(),
                percent: 41,
                weeks: 10,
            },
            Effect::CreditTightening {
                spread_bps: 301,
                weeks: 10,
            },
            Effect::CentralBankStance { shift: 41 },
            Effect::ImportCeiling {
                good: "steel".into(),
                percent: -51,
            },
        ];
        for effect in too_big {
            assert!(
                !one(effect.clone())
                    .validate("great-lakes-2000", &ds)
                    .is_empty(),
                "{effect:?}"
            );
        }
    }

    #[test]
    fn the_player_copy_has_no_dashes() {
        for scenario in crate::scenarios() {
            let json = timeline_json(&scenario.id).unwrap();
            assert!(!json.contains(['\u{2013}', '\u{2014}']), "{}", scenario.id);
        }
    }
}
