//! Week-by-week asks, free stock, sales and imports for one good.
//! cargo run --release -p cw-session --example price_trace -- scenario seed good weeks
#[path = "../tests/common/market_health.rs"]
mod market_health;
use cw_sim::economy::pricing;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (scenario, seed, good, weeks) = (
        &a[0],
        a[1].parse().unwrap(),
        &a[2],
        a[3].parse::<u32>().unwrap(),
    );
    let mut s = cw_session::Session::create(market_health::config(seed, scenario)).unwrap();
    let code = s.dataset.product_index[good.as_str()];
    for week in 1..=weeks {
        let r = s.advance(1);
        if r.pause.is_some() {
            for e in s.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            s.world.pending_pause = None;
            s.dismiss_pause();
        }
        let w = &s.world;
        let d = &s.dataset;
        let mut cells = Vec::new();
        for (h, f) in w.firms.iter() {
            let op = f.operative();
            if !pricing::outputs(w, d, h).contains(&code) {
                continue;
            }
            let ask = f.policies.prices.get(&code).copied().unwrap_or(-1);
            cells.push(format!(
                "[{:?}{} ask {} sold {} free {}]",
                f.status,
                if op { "" } else { " X" },
                ask,
                pricing::weekly_sold(w, h, code),
                pricing::free_stock(w, d, h, code)
            ));
        }
        let imp: i64 = w
            .gateways
            .iter()
            .filter(|g| g.product == code)
            .map(|g| g.last_import_lots)
            .sum();
        let exp: i64 = w
            .gateways
            .iter()
            .filter(|g| g.product == code)
            .map(|g| g.last_export_lots)
            .sum();
        let sell = w
            .gateways
            .iter()
            .filter(|g| g.product == code && g.sell_price_cents > 0)
            .map(|g| g.sell_price_cents)
            .min()
            .unwrap_or(0);
        let snap = s.market_snapshot();
        let line = snap.goods.iter().find(|g| &g.id == good).unwrap();
        println!(
            "w{week:3} corr {:6} imp {imp:5} exp {exp:5} impPx {sell:6} made {:5} sold {:5} stock {:6} {}",
            pricing::corridor_price(w, d, code),
            line.made_lots,
            line.sold_lots,
            line.stock_lots,
            cells.join(" ")
        );
    }
}
