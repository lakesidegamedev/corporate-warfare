//! Freight modes, terminals and sea systems.
//!
//! Content lives in `content/logistics.json` and is part of the dataset hash. A mode is data:
//! its price per lot-kilometre, speed, smallest shipment, whether it needs a terminal at both
//! ends or infrastructure on the lane, and which storage classes it may carry. Scenarios supply
//! the starting terminals and sea systems; the rules are the same everywhere.

use serde::{Deserialize, Serialize};

use crate::balance::StorageClass;

pub const LOGISTICS_JSON: &str = include_str!("../../../content/logistics.json");

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeDef {
    pub id: String,
    pub name: String,
    /// Price of moving one lot one kilometre, in thousandths of a cent.
    pub rate_milli_cents_per_lot_km: i64,
    pub speed_km_week: u32,
    /// How far from its home region a carrier's capacity reaches: it serves lanes with an end
    /// within this distance. Zero means only lanes that start at its home region.
    #[serde(default)]
    pub reach_km: u32,
    /// Smallest shipment, in whole lots, this mode will take.
    pub min_lots: i64,
    /// Rail and sea: both ends of a leg need a terminal or a port.
    pub needs_terminal: bool,
    /// Pipeline: a lane exists only where some firm has built capacity on it.
    pub needs_infrastructure: bool,
    /// Outside carriers take overflow on this mode at a premium, so goods never stop.
    pub outside_backstop: bool,
    /// Truck: the lane's road condition moves the price.
    #[serde(default)]
    pub road_sensitive: bool,
    /// Storage classes the mode may carry. A good outside them cannot use the mode at all.
    pub classes: Vec<StorageClass>,
    /// Products barred from the mode even when their class fits.
    #[serde(default)]
    pub excluded_products: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeaSystem {
    pub id: String,
    pub name: String,
    /// Sea distance as a share of the land distance between two ports of the system.
    pub factor_bps: i64,
    pub ports: Vec<String>,
}

/// A sea leg from a port region to an outside gateway (an ocean or a river mouth).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayLink {
    pub region: String,
    pub gateway: String,
    pub km: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogisticsData {
    pub schema: u32,
    #[serde(default)]
    pub classification: String,
    /// What outside carriers charge on an overflow lot, as basis points of the base rate.
    pub outside_premium_bps: i64,
    /// What an empty lane's owners charge, basis points of the base rate. A lane filling up
    /// rises smoothly from here to the outside price.
    pub empty_lane_bps: i64,
    /// The rate at which a group ships on its own carriers: operating cost, no margin.
    pub own_cost_bps: i64,
    /// A lane contract is priced at the lane's spot price times this.
    pub contract_discount_bps: i64,
    pub modes: Vec<ModeDef>,
    pub public_rail: Vec<String>,
    pub public_ports: Vec<String>,
    #[serde(default)]
    pub sea_systems: Vec<SeaSystem>,
    #[serde(default)]
    pub gateway_links: Vec<GatewayLink>,
}

/// What a scenario supplies on top of the shared freight rules: its public rail yards and ports,
/// its sea systems and its legs to outside gateways. The modes and price levels are universal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogisticsOverlay {
    #[serde(default)]
    pub classification: String,
    pub public_rail: Vec<String>,
    pub public_ports: Vec<String>,
    #[serde(default)]
    pub sea_systems: Vec<SeaSystem>,
    #[serde(default)]
    pub gateway_links: Vec<GatewayLink>,
}

impl LogisticsData {
    pub fn load() -> Result<LogisticsData, String> {
        serde_json::from_str(LOGISTICS_JSON).map_err(|error| format!("logistics.json: {error}"))
    }

    /// The shared rules with one scenario's terminals, sea systems and gateway legs.
    pub fn load_with(overlay_json: &str) -> Result<LogisticsData, String> {
        let overlay: LogisticsOverlay = serde_json::from_str(overlay_json)
            .map_err(|error| format!("scenario logistics.json: {error}"))?;
        let mut data = Self::load()?;
        data.public_rail = overlay.public_rail;
        data.public_ports = overlay.public_ports;
        data.sea_systems = overlay.sea_systems;
        data.gateway_links = overlay.gateway_links;
        Ok(data)
    }

    pub fn mode_index(&self, id: &str) -> Option<u8> {
        self.modes.iter().position(|m| m.id == id).map(|i| i as u8)
    }
}
