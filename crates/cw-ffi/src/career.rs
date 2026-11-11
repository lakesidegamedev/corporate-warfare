//! Careers for the apps (2.0): the helm offers and career options a new-game screen needs, the
//! standing a main screen shows each week and the summary an end screen shows. The engine keeps
//! the state (`cw_sim::economy::career`); the session turns it into plain words
//! (`Session::helm_offers`, `career_standing`, `career_summary`); this module carries those
//! records across the boundary unchanged.

use cw_session::dto;

/// The career choices of a new game. Empty strings and zero take the defaults: a founder start,
/// no ambition (a sandbox career with no goal) and the content's default length.
#[derive(uniffi::Record, Clone, Debug, Default, PartialEq, Eq)]
pub struct CareerOptionsDto {
    /// "founder" or "helm". Empty is "founder".
    #[uniffi(default = "")]
    pub start: String,
    /// For "helm": the `firm` of one of `helm_offers`.
    #[uniffi(default = None)]
    pub helm_firm: Option<u64>,
    /// "richest", "biggest", "dominant" or "survivor"; empty plays with no goal.
    #[uniffi(default = "")]
    pub ambition: String,
    /// 5, 10 or 20; 0 takes the default (10, or 5 in a five-year game).
    #[uniffi(default = 0)]
    pub years: u32,
}

impl CareerOptionsDto {
    pub(crate) fn setup(&self) -> Result<cw_sim::economy::career::CareerSetup, String> {
        use cw_sim::economy::career::{Ambition, CareerSetup, Start};
        let start = match self.start.as_str() {
            "" | "founder" => Start::Founder,
            "helm" => Start::Helm,
            other => return Err(format!("unknown career start {other}")),
        };
        let ambition = match self.ambition.as_str() {
            "" => None,
            key => Some(Ambition::from_key(key).ok_or_else(|| format!("unknown ambition {key}"))?),
        };
        Ok(CareerSetup {
            start,
            helm_firm: self.helm_firm,
            ambition,
            years: self.years,
        })
    }
}

/// One established company the helm start offers, with its figures and the price of control.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct HelmOfferDto {
    pub firm: u64,
    pub name: String,
    pub industry: String,
    pub region: String,
    /// League rank by company value at the start (1 is the largest).
    pub rank: u32,
    pub value_cents: i64,
    pub revenue_year_cents: i64,
    pub plants: u32,
    pub workers: i64,
    pub shares_needed: i64,
    pub mark_cents: i64,
    pub price_cents: i64,
    /// The scenario's standard fortune and the fortune this start gives; they differ when the
    /// standard fortune could not pay for control (the screen must say so).
    pub standard_fortune_cents: i64,
    pub fortune_cents: i64,
}

impl From<dto::HelmOfferDto> for HelmOfferDto {
    fn from(o: dto::HelmOfferDto) -> Self {
        HelmOfferDto {
            firm: o.firm,
            name: o.name,
            industry: o.industry,
            region: o.region,
            rank: o.rank,
            value_cents: o.value_cents,
            revenue_year_cents: o.revenue_year_cents,
            plants: o.plants,
            workers: o.workers,
            shares_needed: o.shares_needed,
            mark_cents: o.mark_cents,
            price_cents: o.price_cents,
            standard_fortune_cents: o.standard_fortune_cents,
            fortune_cents: o.fortune_cents,
        }
    }
}

/// Where the career stands this week.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CareerStandingDto {
    /// "richest", "biggest", "dominant", "survivor", or none for a career with no goal.
    pub ambition: Option<String>,
    /// "none", "bronze", "silver" or "gold".
    pub medal: String,
    /// The company's league place and the league's size; 0 when the player runs none.
    pub rank: u32,
    pub league: u32,
    /// Net worth over the starting fortune in basis points (10000 = no growth).
    pub growth_bps: i64,
    /// What the standing rests on, one plain sentence.
    pub detail: String,
    /// What the next medal needs, one plain sentence; empty at gold.
    pub next: String,
    pub weeks_left: u32,
    pub ended: bool,
    /// The board's warning is in force, its words, and whether it can remove the player.
    pub board_warned: bool,
    pub board_warning: String,
    pub board_can_remove: bool,
}

impl From<dto::CareerStandingDto> for CareerStandingDto {
    fn from(s: dto::CareerStandingDto) -> Self {
        CareerStandingDto {
            ambition: s.ambition,
            medal: s.medal,
            rank: s.rank,
            league: s.league,
            growth_bps: s.growth_bps,
            detail: s.detail,
            next: s.next,
            weeks_left: s.weeks_left,
            ended: s.ended,
            board_warned: s.board_warned,
            board_warning: s.board_warning,
            board_can_remove: s.board_can_remove,
        }
    }
}

/// One year of the career.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CareerYearDto {
    pub year: u32,
    pub rank: u32,
    pub league: u32,
    pub company_value_cents: i64,
    pub net_worth_cents: i64,
    /// Place among the investors the engine can value (information only).
    pub wealth_rank: u32,
    pub medal: String,
}

/// One event that mattered.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CareerEventDto {
    pub week: u32,
    pub key: String,
    pub text: String,
}

/// The career summary for the end screen (and a "so far" view while it runs).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CareerSummaryDto {
    pub start: String,
    pub helm_company: Option<String>,
    pub helm_fortune_scaled: bool,
    pub ambition: Option<String>,
    pub years: u32,
    pub end_week: u32,
    pub standing: String,
    pub ended: bool,
    /// "completed", "won", "no_company", "removed" or "taken_over"; empty while running.
    pub ending: String,
    pub lost: bool,
    pub medal: String,
    /// Why it ended as it did, one plain sentence; empty while running.
    pub sentence: String,
    pub board_warned: bool,
    pub start_worth_cents: i64,
    pub net_worth_cents: i64,
    pub growth_bps: i64,
    pub years_log: Vec<CareerYearDto>,
    pub events: Vec<CareerEventDto>,
}

impl From<dto::CareerSummaryDto> for CareerSummaryDto {
    fn from(s: dto::CareerSummaryDto) -> Self {
        CareerSummaryDto {
            start: s.start,
            helm_company: s.helm_company,
            helm_fortune_scaled: s.helm_fortune_scaled,
            ambition: s.ambition,
            years: s.years,
            end_week: s.end_week,
            standing: s.standing,
            ended: s.ended,
            ending: s.ending,
            lost: s.lost,
            medal: s.medal,
            sentence: s.sentence,
            board_warned: s.board_warned,
            start_worth_cents: s.start_worth_cents,
            net_worth_cents: s.net_worth_cents,
            growth_bps: s.growth_bps,
            years_log: s
                .years_log
                .into_iter()
                .map(|y| CareerYearDto {
                    year: y.year,
                    rank: y.rank,
                    league: y.league,
                    company_value_cents: y.company_value_cents,
                    net_worth_cents: y.net_worth_cents,
                    wealth_rank: y.wealth_rank,
                    medal: y.medal,
                })
                .collect(),
            events: s
                .events
                .into_iter()
                .map(|e| CareerEventDto {
                    week: e.week,
                    key: e.key,
                    text: e.text,
                })
                .collect(),
        }
    }
}
