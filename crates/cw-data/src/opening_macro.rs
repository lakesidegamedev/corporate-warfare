//! The macro conditions a scenario opens with: the inflation and interest rates of its period.
//!
//! Content lives in `content/scenarios/<id>/macro.json`. Every figure is a calibrated game
//! estimate drawn from the real period, not a measured series. The engine reads them once, when
//! it builds the opening world: the business cycle starts at these rates and expectations, the
//! central bank's rule runs around this neutral rate, background wages open at this growth, and
//! opening loans are priced off this policy rate. Every mechanic runs the same way in every
//! scenario; only the data differs.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningMacro {
    /// States plainly that the figures are game estimates of the period.
    pub classification: String,
    /// Annual inflation on the first day, basis points.
    pub inflation_bps: i32,
    /// What households and lenders expect inflation to average, basis points.
    pub expected_inflation_bps: i32,
    /// The central bank's policy rate on the first day.
    pub policy_rate_bps: i32,
    /// The ten-year government bond yield the period saw. The engine derives its own yield from
    /// the rate path; this is the calibration target a test holds the opening yield to.
    pub ten_year_bps: i32,
    /// The real policy rate that neither pushes nor pulls on the economy.
    pub neutral_real_bps: i32,
    /// The inflation the central bank steers toward.
    pub target_inflation_bps: i32,
    /// Extra spread over the grade spread that lenders charged on the first day.
    pub credit_spread_bps: i32,
    /// Annual growth of wages on the first day; it fades into inflation over two years.
    pub wage_growth_bps: i32,
}

impl OpeningMacro {
    pub fn parse(json: &str) -> Result<OpeningMacro, String> {
        let parsed: OpeningMacro =
            serde_json::from_str(json).map_err(|error| format!("macro.json: {error}"))?;
        let mut errors = Vec::new();
        if !parsed
            .classification
            .to_ascii_lowercase()
            .contains("estimate")
        {
            errors.push("macro.json classification must say the figures are estimates");
        }
        if !(-300..=2_000).contains(&parsed.inflation_bps) {
            errors.push("macro.json inflation is outside -3 to 20 percent");
        }
        if !(25..=2_000).contains(&parsed.policy_rate_bps) {
            errors.push("macro.json policy rate is outside 0.25 to 20 percent");
        }
        if !(0..=2_000).contains(&parsed.expected_inflation_bps)
            || !(0..=500).contains(&parsed.target_inflation_bps)
            || !(0..=600).contains(&parsed.neutral_real_bps)
            || !(0..=500).contains(&parsed.credit_spread_bps)
            || !(-500..=2_000).contains(&parsed.wage_growth_bps)
            || !(0..=3_000).contains(&parsed.ten_year_bps)
        {
            errors.push("macro.json has a figure outside its sane range");
        }
        if errors.is_empty() {
            Ok(parsed)
        } else {
            Err(errors.join("; "))
        }
    }
}

/// The weeks over which opening wage growth fades into inflation.
pub const WAGE_FADE_WEEKS: u32 = 104;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SCENARIO_CONTENT, scenario_content};

    #[test]
    fn every_scenario_has_labelled_estimates_and_no_dashes() {
        for content in SCENARIO_CONTENT.iter() {
            let m = OpeningMacro::parse(content.macro_json).unwrap();
            assert!(m.classification.contains("estimates"), "{}", content.id);
            assert!(
                !content.macro_json.contains('\u{2013}')
                    && !content.macro_json.contains('\u{2014}'),
                "{} has a dash",
                content.id
            );
        }
    }

    #[test]
    fn a_figure_out_of_range_or_an_unlabelled_file_is_refused() {
        let json = scenario_content("texas-1981").macro_json;
        assert!(
            OpeningMacro::parse(
                &json.replace("\"policy_rate_bps\": 1650", "\"policy_rate_bps\": 9000")
            )
            .is_err()
        );
        assert!(OpeningMacro::parse(&json.replace("estimates", "figures")).is_err());
    }

    #[test]
    fn the_period_orders_the_scenarios_as_history_did() {
        let get = |id: &str| OpeningMacro::parse(scenario_content(id).macro_json).unwrap();
        let (tx, sv, rr, gl) = (
            get("texas-1981"),
            get("silicon-valley-1995"),
            get("ruhr-rhine-1991"),
            get("great-lakes-2000"),
        );
        assert!(tx.inflation_bps > 2 * rr.inflation_bps);
        assert!(tx.policy_rate_bps > 2 * sv.policy_rate_bps);
        assert!(
            rr.ten_year_bps > gl.ten_year_bps,
            "the Bund yielded more than the Treasury in 2000"
        );
        assert!(
            tx.ten_year_bps < tx.policy_rate_bps,
            "the 1981 curve was inverted"
        );
    }
}
