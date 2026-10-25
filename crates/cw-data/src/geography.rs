//! Corridor geography fixture.
//!
//! Status: the official 2004 BEA county-to-economic-area crosswalk could not be recovered.
//! Region codes and names below are the verified official roster entries. County membership
//! is absent rather than invented. Population, wage, demand, land and distance values are
//! labelled engineering calibration for the one-percent game scale.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Region {
    pub code: String,
    pub name: String,
    pub states: Vec<String>,
    pub principal_city: String,
    pub lat_milli: i32,
    pub lon_milli: i32,
    /// Engineering calibration: relative wage level, 10000 = corridor baseline.
    pub wage_index_bps: i32,
    /// Engineering calibration: relative weekly household demand volume.
    pub demand_index_bps: i32,
    /// Engineering calibration: relative land cost.
    pub land_index_bps: i32,
    /// Labelled estimate where no union density observation exists.
    pub opening_organisation_bps: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lane {
    pub a: String,
    pub b: String,
    pub km: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Geography {
    pub schema: u32,
    pub classification: String,
    pub vintage: String,
    pub official_source: String,
    pub attempts: Vec<String>,
    pub membership_status: String,
    pub notes: String,
    pub regions: Vec<Region>,
    pub lanes: Vec<Lane>,
}

impl Geography {
    pub fn geometry_note(&self) -> String {
        format!(
            "{} | vintage: {} | {} | attempts: {}",
            self.classification,
            self.vintage,
            self.membership_status,
            self.attempts.join(" ; ")
        )
    }
}
