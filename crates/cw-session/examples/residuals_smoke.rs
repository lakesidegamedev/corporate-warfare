//! Residuals smoke: per seed, food stock, fuel import share, refinery count, player net worth
//! and rival dissolutions at the horizon. Seeds run on parallel threads.
//!
//! cargo run --release -p cw-session --example residuals_smoke -- [seeds] [weeks]
struct Row {
    food_stock: i64,
    grain_stock: i64,
    fuel_import: i64,
    fuel_made: i64,
    refineries: i64,
    rival_refineries: i64,
    net_worth: i64,
    dissolved: i64,
}

fn run(seed: u64, weeks: u32) -> Row {
    let config = cw_session::ScenarioConfig {
        seed,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    };
    let mut session = cw_session::Session::create(config).unwrap();
    let (mut imp, mut made, mut wk) = (0i64, 0i64, 0i64);
    for week in 1..=weeks {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
        // Average the last 26 weeks of fuel flows.
        if week + 26 > weeks {
            let fuel = session.dataset.product_index["fuel"];
            imp += session
                .world
                .gateways
                .iter()
                .filter(|g| g.product == fuel)
                .map(|g| g.last_import_lots)
                .sum::<i64>();
            wk += 1;
        }
    }
    let world = &session.world;
    let dataset = &session.dataset;
    let stock = |id: &str| -> i64 {
        let p = dataset.product_index[id];
        world
            .firms
            .iter()
            .flat_map(|(_, f)| f.inventory.iter())
            .filter(|((q, _), _)| *q == p)
            .map(|(_, l)| l.qty.lots())
            .sum()
    };
    let (mut refs, mut rival_refs) = (0i64, 0i64);
    for (_, f) in world.facilities.iter() {
        let r = &dataset.recipes[f.recipe as usize];
        if r.id == "refinery" && f.status == cw_sim::state::FacilityStatus::Operating {
            refs += 1;
            made += r.weekly_output_lots * f.blocks as i64;
            if world
                .firms
                .get(f.firm)
                .map(|o| o.archetype.is_some())
                .unwrap_or(false)
            {
                rival_refs += 1;
            }
        }
    }
    Row {
        food_stock: stock("food_raw"),
        grain_stock: stock("grain"),
        fuel_import: imp / wk.max(1),
        fuel_made: made,
        refineries: refs,
        rival_refineries: rival_refs,
        net_worth: cw_sim::economy::score::personal_net_worth(world),
        dissolved: world
            .firms
            .iter()
            .filter(|(_, f)| f.archetype.is_some())
            .filter(|(_, f)| matches!(f.status, cw_sim::state::FirmStatus::Dissolved))
            .count() as i64,
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(24);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(156);
    let mut handles = Vec::new();
    for chunk in 0..6u64 {
        handles.push(std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
            (1..=seeds).filter(|s| s % 6 == chunk).map(|s| (s, run(s, weeks))).collect::<Vec<_>>()
        }).unwrap());
    }
    let mut rows: Vec<(u64, Row)> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    rows.sort_by_key(|r| r.0);
    for (s, r) in &rows {
        println!(
            "seed {s}: food {} grain {} fuel_import/wk {} fuel_made/wk {} refineries {} (rival {}) networth ${} dissolved {}",
            r.food_stock, r.grain_stock, r.fuel_import, r.fuel_made, r.refineries,
            r.rival_refineries, r.net_worth / 100, r.dissolved
        );
    }
    let n = rows.len() as i64;
    let mean = |f: &dyn Fn(&Row) -> i64| rows.iter().map(|r| f(&r.1)).sum::<i64>() / n;
    let mut nw: Vec<i64> = rows.iter().map(|r| r.1.net_worth).collect();
    nw.sort();
    let imp = mean(&|r| r.fuel_import);
    let made = mean(&|r| r.fuel_made);
    println!(
        "SUMMARY {seeds} seeds x {weeks} weeks: food_raw stock {} grain {} | fuel import/wk {imp} made/wk {made} import share {}% | refineries {} (rival {}) | networth median ${} mean ${} | rival dissolutions {}",
        mean(&|r| r.food_stock),
        mean(&|r| r.grain_stock),
        imp * 100 / (imp + made).max(1),
        mean(&|r| r.refineries),
        mean(&|r| r.rival_refineries),
        nw[nw.len() / 2] / 100,
        nw.iter().sum::<i64>() / n / 100,
        rows.iter().map(|r| r.1.dissolved).sum::<i64>()
    );
}
