//! Currencies and countries for scenarios that span more than one money.
//!
//! A scenario with a `currencies.json` next to its regions lists its countries, the money each
//! uses, and the central bank behaviour the exchange rate model reads. Every region names its
//! country. Ledger units are re-based so one unit of every currency buys the same on day one;
//! `face_milli` only records how many nominal units of the currency one unit of the numeraire
//! bought in the real world, for quoting rates.
//!
//! All numbers here are authored game estimates, not market history.

use serde::{Deserialize, Serialize};

/// A peg to another currency inside a fixed band, like the 1991 exchange rate mechanism.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PegDef {
    /// The currency this one is pegged to.
    pub anchor: String,
    /// Half width of the band, in basis points either side of parity.
    pub band_bps: i32,
    /// Half width after a band crisis widens the band.
    #[serde(default = "default_widened_band")]
    pub widened_band_bps: i32,
    /// Weekly chance of a band crisis once the strain on the peg is high, in hundredths of a
    /// basis point.
    #[serde(default)]
    pub crisis_hazard_ppm: i32,
    /// Weeks since the start during which a historical price path raises the chance of a crisis
    /// (1992 to 1993 for the real mechanism). Empty when the peg has no scripted window.
    #[serde(default)]
    pub crisis_window_weeks: Option<[u32; 2]>,
}

fn default_widened_band() -> i32 {
    1_500
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrencyDef {
    pub id: String,
    pub name: String,
    pub symbol: String,
    /// The country whose central bank issues it.
    pub country: String,
    /// Nominal units of this currency per unit of the numeraire in the real world, times 1000.
    #[serde(default = "default_face")]
    pub face_milli: i64,
    /// Standing gap of the central bank rate over the anchor country's, in basis points.
    #[serde(default)]
    pub policy_spread_bps: i32,
    /// Standing gap of inflation over the anchor country's, in basis points a year.
    #[serde(default)]
    pub inflation_spread_bps: i32,
    /// Largest weekly random move of the market rate, in basis points.
    #[serde(default = "default_volatility")]
    pub volatility_bps: i32,
    #[serde(default)]
    pub peg: Option<PegDef>,
    /// Name of the central bank, for display.
    #[serde(default)]
    pub central_bank: String,
}

fn default_face() -> i64 {
    1_000
}

fn default_volatility() -> i32 {
    60
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountryDef {
    pub id: String,
    pub name: String,
    /// The currency it uses.
    pub currency: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurrencyFile {
    #[serde(default)]
    pub classification: String,
    /// The currency of the anchor country: its central bank is the macro model's, and every
    /// other rate is quoted against it.
    pub numeraire: String,
    pub countries: Vec<CountryDef>,
    pub currencies: Vec<CurrencyDef>,
}

/// The validated currency set of one scenario, in a fixed order with the numeraire first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrencySet {
    pub classification: String,
    pub countries: Vec<CountryDef>,
    pub currencies: Vec<CurrencyDef>,
}

impl CurrencySet {
    pub fn parse(json: &str) -> Result<CurrencySet, String> {
        let file: CurrencyFile =
            serde_json::from_str(json).map_err(|error| format!("currencies.json: {error}"))?;
        let mut currencies = file.currencies;
        let Some(position) = currencies.iter().position(|c| c.id == file.numeraire) else {
            return Err(format!(
                "numeraire {} is not a listed currency",
                file.numeraire
            ));
        };
        let first = currencies.remove(position);
        currencies.sort_by(|a, b| a.id.cmp(&b.id));
        currencies.insert(0, first);
        let mut countries = file.countries;
        countries.sort_by(|a, b| a.id.cmp(&b.id));
        let set = CurrencySet {
            classification: file.classification,
            countries,
            currencies,
        };
        let errors = set.validate();
        if errors.is_empty() {
            Ok(set)
        } else {
            Err(errors.join("; "))
        }
    }

    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.currencies.len() < 2 {
            errors.push("a currency set needs at least two currencies".to_owned());
        }
        for (i, c) in self.currencies.iter().enumerate() {
            if self.currencies[..i].iter().any(|o| o.id == c.id) {
                errors.push(format!("duplicate currency {}", c.id));
            }
            if !self.countries.iter().any(|country| country.id == c.country) {
                errors.push(format!(
                    "currency {} names unknown country {}",
                    c.id, c.country
                ));
            }
            if c.face_milli <= 0 {
                errors.push(format!("currency {} has no face value", c.id));
            }
            if !(0..=500).contains(&c.volatility_bps) {
                errors.push(format!("currency {} volatility out of range", c.id));
            }
            if let Some(peg) = &c.peg {
                if !self
                    .currencies
                    .iter()
                    .any(|o| o.id == peg.anchor && o.id != c.id)
                {
                    errors.push(format!("currency {} pegs to unknown {}", c.id, peg.anchor));
                }
                if peg.band_bps <= 0 || peg.widened_band_bps < peg.band_bps {
                    errors.push(format!("currency {} peg band is invalid", c.id));
                }
            }
        }
        for country in &self.countries {
            if !self.currencies.iter().any(|c| c.id == country.currency) {
                errors.push(format!("country {} uses unknown currency", country.id));
            }
        }
        errors
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.currencies.iter().position(|c| c.id == id)
    }

    pub fn currency_of_country(&self, country: &str) -> Option<usize> {
        let def = self.countries.iter().find(|c| c.id == country)?;
        self.index_of(&def.currency)
    }
}
