//! Time of every view call the app makes on a reload, and the size of what it returns.
//!
//! cargo run --release -p cw-ffi --example ffi_bench -- [scenario-id ...] [--weeks N]
use cw_ffi::{NewGameDto, SessionHandle};
use std::time::Instant;

fn timed<T>(name: &str, rows: &mut Vec<(String, f64, usize)>, f: impl Fn() -> (T, usize)) -> T {
    let mut best = f64::MAX;
    let mut out = None;
    let mut count = 0;
    for _ in 0..5 {
        let t = Instant::now();
        let (value, n) = f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
        count = n;
        out = Some(value);
    }
    rows.push((name.to_string(), best, count));
    out.unwrap()
}

fn main() {
    let mut weeks: u32 = 52;
    let mut scenarios: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--weeks" {
            weeks = args.next().and_then(|v| v.parse().ok()).unwrap_or(52);
        } else {
            scenarios.push(arg);
        }
    }
    if scenarios.is_empty() {
        scenarios = ["great-lakes-2000", "texas-1981", "silicon-valley-1995", "ruhr-rhine-1991"].map(String::from).to_vec();
    }
    for scenario in scenarios {
        let handle = SessionHandle::start(NewGameDto {
            seed: 7,
            scenario_id: scenario.clone(),
            guided: true,
            five_year: false,
            difficulty: "standard".into(),
            economy: "historic".into(),
            starting_capital_cents: None,
            price_path: String::new(),
        })
        .unwrap();
        for _ in 0..weeks {
            let r = handle.advance(1);
            if r.pause.is_some() {
                handle.dismiss_pause();
            }
        }
        let firm = handle.overview().primary_firm;
        let mut rows = Vec::new();
        let h = &handle;
        timed("overview", &mut rows, || (h.overview(), 1));
        timed("firms", &mut rows, || { let v = h.firms(); let n = v.len(); (v, n) });
        timed("events", &mut rows, || { let v = h.events(); let n = v.len(); (v, n) });
        timed("region_boards", &mut rows, || { let v = h.region_boards(firm); let n = v.len(); (v, n) });
        timed("finance", &mut rows, || (h.finance(firm.unwrap()), 1));
        timed("product_lines", &mut rows, || { let v = h.product_lines(firm.unwrap()); let n = v.len(); (v, n) });
        timed("facilities", &mut rows, || { let v = h.facilities(firm.unwrap()); let n = v.len(); (v, n) });
        timed("divisions", &mut rows, || { let v = h.divisions(firm.unwrap()); let n = v.len(); (v, n) });
        timed("stock_board", &mut rows, || { let v = h.stock_board(); let n = v.len(); (v, n) });
        timed("commodity_board", &mut rows, || { let v = h.commodity_board(); let n = v.len(); (v, n) });
        timed("map_divisions", &mut rows, || { let v = h.map_divisions(); let n = v.len(); (v, n) });
        timed("history", &mut rows, || { let v = h.history(firm.unwrap()); let n = v.len(); (v, n) });
        timed("ownership", &mut rows, || (h.ownership(firm.unwrap()), 1));
        timed("recent_raids", &mut rows, || { let v = h.recent_raids(); let n = v.len(); (v, n) });
        timed("economy", &mut rows, || (h.economy(), 1));
        timed("research", &mut rows, || (h.research(firm.unwrap()), 1));
        timed("held_stock", &mut rows, || { let v = h.held_stock(firm.unwrap()); let n = v.len(); (v, n) });
        timed("storage_summary", &mut rows, || (h.storage_summary(firm.unwrap()), 1));
        timed("lanes", &mut rows, || { let v = h.lanes(); let n = v.len(); (v, n) });
        timed("lane_contracts", &mut rows, || { let v = h.lane_contracts(); let n = v.len(); (v, n) });
        timed("freight_summary", &mut rows, || (h.freight_summary(), 1));
        timed("structure_view", &mut rows, || (h.structure_view(), 1));
        let products = h.products();
        let regions = h.region_boards(firm);
        let region = regions[0].index;
        timed("products()", &mut rows, || { let v = h.products(); let n = v.len(); (v, n) });
        timed("market x all products", &mut rows, || {
            let v: Vec<_> = products.iter().map(|p| h.market(region, p.index)).collect();
            let n = v.len();
            (v, n)
        });
        timed("export_save", &mut rows, || { let v = h.export_save().unwrap(); let n = v.len(); (v, n) });
        let total: f64 = rows.iter().filter(|r| r.0 != "export_save").map(|r| r.1).sum();
        println!("{scenario} after {weeks} weeks (best of 5):");
        for (name, ms, n) in &rows {
            println!("    {name:<24} {ms:>8.3} ms   {n} items");
        }
        println!("    reload total (all but save) {total:.3} ms");
    }
}
