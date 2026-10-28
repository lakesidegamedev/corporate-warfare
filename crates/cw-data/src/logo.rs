//! Generated company logos: a 16 by 16 plate with a symbol or a two-letter monogram, chosen from
//! the firm's name alone so every platform draws the same logo.
//!
//! The parts live in `content/logos.json`, written by `tools/logo_gen.py`, whose `compose()` does
//! exactly the steps below. Presentation only: logos are not part of the dataset hash.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const LOGOS_JSON: &str = include_str!("../../../content/logos.json");
const SIZE: usize = 16;
const SYM: usize = 10;

#[derive(Deserialize)]
struct Colours {
    p: String,
    e: String,
    s: String,
}

#[derive(Deserialize)]
struct Parts {
    plates: BTreeMap<String, Vec<String>>,
    plate_order: Vec<String>,
    palettes: Vec<Colours>,
    symbols: BTreeMap<String, Vec<String>>,
    font: BTreeMap<String, Vec<String>>,
    banned_initials: Vec<String>,
}

/// A composed logo: `rows` use '.' for transparent, 'p' plate, 'e' edge and 's' symbol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Logo {
    pub plate: String,
    pub edge: String,
    pub symbol: String,
    pub rows: Vec<String>,
}

fn parts() -> &'static Parts {
    static PARTS: OnceLock<Parts> = OnceLock::new();
    PARTS.get_or_init(|| serde_json::from_str(LOGOS_JSON).expect("content/logos.json is valid"))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn initials(name: &str, banned: &[String]) -> String {
    let words: Vec<String> = name
        .split_whitespace()
        .map(|w| {
            w.to_uppercase()
                .chars()
                .filter(|c| c.is_ascii_uppercase())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let pair: String = match words.len() {
        0 => return "O".to_owned(),
        1 => words[0].chars().take(2).collect(),
        _ => format!(
            "{}{}",
            words[0].chars().next().unwrap_or('O'),
            words[1].chars().next().unwrap_or('O')
        ),
    };
    if pair.chars().count() == 2 && !banned.contains(&pair) {
        pair
    } else {
        pair.chars().take(1).collect()
    }
}

/// The logo of a firm, deterministic in its name.
pub fn compose(name: &str) -> Logo {
    let parts = parts();
    let mut stream = Stream(fnv1a(name.trim().to_lowercase().as_bytes()));
    let plate = &parts.plate_order[(stream.next() % parts.plate_order.len() as u64) as usize];
    let colours = &parts.palettes[(stream.next() % parts.palettes.len() as u64) as usize];
    let monogram = stream.next() % 100 < 22;
    let symbol_names: Vec<&String> = parts.symbols.keys().collect();
    let symbol = symbol_names[(stream.next() % symbol_names.len() as u64) as usize];
    let mut grid: Vec<Vec<char>> = parts.plates[plate]
        .iter()
        .map(|row| row.chars().collect())
        .collect();
    let (bitmap, width): (Vec<String>, usize) = if monogram {
        let glyphs: Vec<&Vec<String>> = initials(name, &parts.banned_initials)
            .chars()
            .filter_map(|c| parts.font.get(&c.to_string()))
            .collect();
        let width = 5 * glyphs.len() + glyphs.len().saturating_sub(1);
        let rows = (0..7)
            .map(|y| {
                let mut row = String::new();
                for (index, glyph) in glyphs.iter().enumerate() {
                    if index > 0 {
                        row.push('.');
                    }
                    row.push_str(&glyph[y]);
                }
                row
            })
            .collect();
        (rows, width)
    } else {
        (parts.symbols[symbol].clone(), SYM)
    };
    let ox = (SIZE - width) / 2;
    let oy = (SIZE - bitmap.len()) / 2;
    for (y, row) in bitmap.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                grid[oy + y][ox + x] = 's';
            }
        }
    }
    Logo {
        plate: colours.p.clone(),
        edge: colours.e.clone(),
        symbol: colours.s.clone(),
        rows: grid.into_iter().map(|r| r.into_iter().collect()).collect(),
    }
}

/// A short digest of a logo's rows, for pinning in tests.
pub fn digest(rows: &[String]) -> String {
    format!("{:016x}", fnv1a(rows.concat().as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned in tools/test_logo_gen.py too: both composers must agree.
    const PINS: [(&str, &str); 4] = [
        ("Maumee Valley Grain", "c542aac79e72fb2f"),
        ("Lakeshore Steel", "72508df24c03d0b3"),
        ("Prairie Wheat Co", "4e5bb91c79184181"),
        ("Zeta Works", "422e5baba54f1269"),
    ];

    #[test]
    fn logos_match_the_python_composer() {
        for (name, pin) in PINS {
            assert_eq!(digest(&compose(name).rows), pin, "{name}");
        }
    }

    #[test]
    fn logo_is_deterministic_and_sixteen_square() {
        let a = compose("Ember Oil");
        assert_eq!(a, compose("  ember oil "));
        assert_eq!(a.rows.len(), 16);
        assert!(a.rows.iter().all(|r| r.chars().count() == 16));
        assert!(a.rows.iter().any(|r| r.contains('s')));
    }
}
