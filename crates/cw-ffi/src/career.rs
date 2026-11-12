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

/// One ambition for the new-game screen: its name, one plain sentence and what each medal takes,
/// worked out from the content table for the chosen start and length (no number is typed into an app).
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct AmbitionGuideDto {
    /// "richest", "biggest", "dominant" or "survivor".
    pub key: String,
    pub name: String,
    pub sentence: String,
    pub bronze: String,
    pub silver: String,
    pub gold: String,
}

fn times_words(bps: i64) -> String {
    let tenths = bps / 1_000;
    if tenths % 10 == 0 {
        format!("{} times", tenths / 10)
    } else {
        format!("{}.{} times", tenths / 10, tenths % 10)
    }
}

fn place_words(n: u32) -> String {
    if n == 1 {
        "first place".to_owned()
    } else {
        format!("the top {n}")
    }
}

fn top_share_words(bps: i64) -> String {
    match bps {
        5_000 => "the top half".to_owned(),
        2_500 => "the top quarter".to_owned(),
        b => format!("the top {} percent", b / 100),
    }
}

/// The four ambitions with their medals for a start ("founder" or "helm") and a career length in
/// years (0 takes the default).
pub fn ambition_guide(start: &str, years: u32) -> Vec<AmbitionGuideDto> {
    let t = cw_data::careers::career_table();
    let years = if years == 0 { t.default_years } else { years };
    let m = t.richest.for_years(years);
    let places = if start == "helm" {
        &t.biggest.helm
    } else {
        &t.biggest.founder
    };
    let bronze_places = if places.bronze == 0 {
        format!(
            "Finish in {} of the league by value",
            top_share_words(places.bronze_top_bps)
        )
    } else {
        format!(
            "Finish in {} of the league by value",
            place_words(places.bronze)
        )
    };
    let d = &t.dominant;
    let dominant = |bps: i64| {
        format!(
            "Hold {} percent of one industry's output for {} quarters in a row",
            bps / 100,
            d.quarters
        )
    };
    let start_word = if start == "helm" {
        "the net worth you have just after buying control"
    } else {
        "the fortune you start with"
    };
    vec![
        AmbitionGuideDto {
            key: "richest".to_owned(),
            name: "Richest".to_owned(),
            sentence: "Grow your own fortune as far as you can.".to_owned(),
            bronze: format!("End with {} {}", times_words(m.bronze_bps), start_word),
            silver: format!("End with {} {}", times_words(m.silver_bps), start_word),
            gold: format!("End with {} {}", times_words(m.gold_bps), start_word),
        },
        AmbitionGuideDto {
            key: "biggest".to_owned(),
            name: "Biggest".to_owned(),
            sentence: "Run the largest company in the league.".to_owned(),
            bronze: bronze_places,
            silver: format!(
                "Finish in {} of the league by value",
                place_words(places.silver)
            ),
            gold: format!(
                "Finish in {} of the league by value",
                place_words(places.gold)
            ),
        },
        AmbitionGuideDto {
            key: "dominant".to_owned(),
            name: "Dominant".to_owned(),
            sentence:
                "Own a market: lead one industry year after year. Gold ends the career early."
                    .to_owned(),
            bronze: dominant(d.bronze_bps),
            silver: dominant(d.silver_bps),
            gold: dominant(d.gold_bps),
        },
        AmbitionGuideDto {
            key: "survivor".to_owned(),
            name: "Survivor".to_owned(),
            sentence: "Come through the booms and busts with a company still standing.".to_owned(),
            bronze: "End with a solvent company you control".to_owned(),
            silver: format!(
                "Also rank in {} of the league",
                top_share_words(t.survivor.silver_top_bps)
            ),
            gold: format!(
                "Also rank in {} of the league",
                top_share_words(t.survivor.gold_top_bps)
            ),
        },
    ]
}

/// The career lengths the content offers, in years, and the default.
pub(crate) fn career_lengths() -> (Vec<u32>, u32) {
    let t = cw_data::careers::career_table();
    (t.lengths_years.clone(), t.default_years)
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CareerLengthsDto {
    pub years: Vec<u32>,
    pub default_years: u32,
}

#[cfg(test)]
mod guide_tests {
    use super::*;

    #[test]
    fn the_guide_reads_the_content() {
        let g = ambition_guide("founder", 10);
        assert_eq!(g.len(), 4);
        assert_eq!(g[0].bronze, "End with 10 times the fortune you start with");
        assert_eq!(g[0].gold, "End with 35 times the fortune you start with");
        assert_eq!(g[1].bronze, "Finish in the top half of the league by value");
        assert_eq!(g[1].gold, "Finish in the top 3 of the league by value");
        let h = ambition_guide("helm", 5);
        assert_eq!(h[1].gold, "Finish in first place of the league by value");
        assert!(h[0].silver.contains("4.5 times"));
        assert!(g[2].bronze.contains("20 percent"));
        assert_eq!(career_lengths().0, vec![5, 10, 20]);
    }
}
