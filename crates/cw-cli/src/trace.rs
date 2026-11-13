//! `cw-cli market-trace`: one career, every good, every week, as CSV. For diagnosing why a
//! good's price moved or did not: the price beside the stock, output, buyers' shortfall, imports
//! and exports that drive it.

use std::fmt::Write as _;
use std::fs;

use cw_core::ids::Handle;
use cw_sim::economy::pricing;

use crate::policy::PolicyRunner;

fn flag(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|a| a == name)
        .and_then(|i| arguments.get(i + 1).cloned())
}

/// One CSV row per good and week.
fn rows(session: &cw_session::Session, out: &mut String) {
    let world = &session.world;
    let dataset = &session.dataset;
    let week = world.tick.0;
    for code in 0..dataset.products.len() as u16 {
        let product = &dataset.products[code as usize];
        if product.reference_price_cents <= 0 {
            continue;
        }
        let reference = pricing::price_reference(world, dataset, code);
        let price = pricing::corridor_price(world, dataset, code);
        let import = world
            .gateways
            .iter()
            .filter(|g| g.product == code && g.sell_price_cents > 0)
            .map(|g| g.sell_price_cents)
            .min()
            .unwrap_or(0);
        let export = world
            .gateways
            .iter()
            .filter(|g| g.product == code)
            .map(|g| g.buy_price_cents)
            .max()
            .unwrap_or(0);
        let imported: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == code)
            .map(|g| g.last_import_lots)
            .sum();
        let exported: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == code)
            .map(|g| g.last_export_lots)
            .sum();
        let import_room: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == code)
            .map(|g| g.capacity_lots)
            .sum();
        let made: i64 = world
            .facilities
            .iter()
            .map(|(_, plant)| plant.last_made(dataset, &product.id).lots())
            .sum();
        let (mut makers, mut sold, mut stock, mut wanted_sum, mut wanted_n) =
            (0, 0i64, 0i64, 0i64, 0i64);
        let mut capacity = 0i64;
        for (handle, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
            let plants: Vec<_> = firm
                .facilities
                .iter()
                .filter_map(|h| world.facilities.get(*h))
                .filter(|p| {
                    p.is_operating() && dataset.recipes[p.recipe as usize].makes(&product.id)
                })
                .collect();
            if plants.is_empty() {
                continue;
            }
            makers += 1;
            capacity += plants
                .iter()
                .map(|p| dataset.recipes[p.recipe as usize].lots_of(&product.id) * p.blocks as i64)
                .sum::<i64>();
            sold += pricing::weekly_sold(world, handle, code);
            stock += pricing::free_stock(world, dataset, handle, code);
            if let Some(w) = pricing::wanted_bps(world, dataset, Handle(handle.0), code) {
                wanted_sum += w;
                wanted_n += 1;
            }
        }
        let (short, need) = pricing::buyer_shortfall(world, dataset, code);
        let (unmet, hh) = pricing::household_unmet(world, code);
        let all: Vec<u8> = (0..world.regions.len() as u8).collect();
        let (biz_unmet, biz_need) = pricing::business_unmet_in(world, dataset, code, &all);
        let wanted = if wanted_n > 0 {
            wanted_sum / wanted_n
        } else {
            0
        };
        let _ = writeln!(
            out,
            "{week},{},{:?},{reference},{price},{import},{export},{makers},{capacity},{made},{sold},{stock},{short},{need},{unmet},{hh},{imported},{exported},{import_room},{wanted},{biz_unmet},{biz_need},\"{}\"",
            product.id,
            product.kind,
            pricing::price_reason(world, dataset, code)
        );
    }
}

pub fn run(arguments: &[String]) {
    let seed: u64 = flag(arguments, "--seed")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|s| s.parse().ok())
        .unwrap_or(156);
    let scenario = flag(arguments, "--scenario-id").unwrap_or_else(|| "great-lakes-2000".into());
    let policy = crate::policy_flag(arguments);
    crate::arc::load_markets_override(arguments);
    let path = flag(arguments, "--csv").unwrap_or_else(|| "market-trace.csv".into());
    let config = crate::arc::arc_config(seed, &scenario, cw_data::Difficulty::Standard);
    let mut session = match cw_session::Session::create(config) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("session failed: {error:?}");
            std::process::exit(1);
        }
    };
    if let Err(error) = crate::arc::apply_markets_override(&mut session) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    let mut out = String::from(
        "week,good,kind,ref,price,import,export,makers,capacity,made,sold,stock,short,need,hh_unmet,hh_wanted,imported,exported,gateway_room,wanted_bps,biz_unmet,biz_need,reason\n",
    );
    let mut runner = PolicyRunner::new(policy, false);
    crate::drive_with(&mut session, weeks, &mut runner, &mut |s, after| {
        if after {
            rows(s, &mut out);
        }
    });
    fs::write(&path, out).expect("write csv");
    eprintln!("wrote {path}");
}
