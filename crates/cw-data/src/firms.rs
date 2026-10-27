//! Seed roster: two named fictional firms per industry plus their archetype.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Archetype {
    CostLeader,
    BrandBuilder,
    Consolidator,
    Technologist,
    Vulture,
}

impl Archetype {
    pub const ALL: [Archetype; 5] = [
        Archetype::CostLeader,
        Archetype::BrandBuilder,
        Archetype::Consolidator,
        Archetype::Technologist,
        Archetype::Vulture,
    ];

    /// Weights on [cost saving, growth, quality/brand, R&D, acquisition].
    pub const fn weights(self) -> [i64; 5] {
        match self {
            Archetype::CostLeader => [5, 3, 1, 2, 1],
            Archetype::BrandBuilder => [1, 3, 5, 2, 1],
            Archetype::Consolidator => [2, 2, 1, 1, 5],
            Archetype::Technologist => [2, 2, 2, 5, 1],
            Archetype::Vulture => [3, 1, 1, 1, 5],
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Archetype::CostLeader => "cost_leader",
            Archetype::BrandBuilder => "brand_builder",
            Archetype::Consolidator => "consolidator",
            Archetype::Technologist => "technologist",
            Archetype::Vulture => "vulture",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedFirm {
    pub id: String,
    pub name: String,
    pub industry: String,
    pub archetype: Archetype,
    pub region: String,
    /// Operating recipe ID, or `bank` / `property_insurer` for the financial industry.
    pub recipe: String,
    /// One to four operating template blocks.
    pub blocks: u8,
    pub public: bool,
    /// Opening cash as a percentage of the standard amount. Custom scenarios change it.
    #[serde(default = "full_percent")]
    pub cash_pct: u16,
    /// Further plants the firm opens with beside its main one, each in its own region. They give
    /// goods that would otherwise be import-only a local producer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_plants: Vec<ExtraPlant>,
    /// For a carrier main plant: the regions its lanes are pinned to, so a pipeline opens on a
    /// named corridor. Empty leaves the carrier to choose its lanes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serves: Vec<String>,
}

/// One additional opening plant of a seed firm.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtraPlant {
    pub recipe: String,
    pub region: String,
    pub blocks: u8,
    /// For a carrier: the regions its lanes are pinned to. Empty leaves the lanes automatic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub serves: Vec<String>,
}

fn full_percent() -> u16 {
    100
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedFirms {
    pub schema: u32,
    #[serde(default)]
    pub classification: String,
    pub firms: Vec<SeedFirm>,
}
