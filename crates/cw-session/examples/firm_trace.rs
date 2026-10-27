//! Diagnostic: quarterly P&L, price, stock and staffing for every firm in one industry.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let industry: String = args.get(1).cloned().unwrap_or("agriculture".into());
    let seed: u64 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
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
    for week in 0..=260u32 {
        if week % 13 == 0 {
            let world = &session.world;
            println!(
                "w{week} outside gateway cash=${}",
                world.outside.gateway_book.cash() / 100
            );
            for (handle, firm) in world.firms.iter() {
                if cw_sim::state::INDUSTRY_NAMES[firm.industry as usize] != industry {
                    continue;
                }
                let weeks = firm.book.recent_weeks();
                let sum = |f: fn(&cw_core::ledger::WeekClose) -> i64| {
                    weeks.iter().map(f).sum::<i64>() / 100
                };
                let facilities: Vec<String> = firm
                    .facilities
                    .iter()
                    .filter_map(|h| world.facilities.get(*h))
                    .map(|f| {
                        format!(
                            "{}r{}b{} staff{} out{} short{} tgt{}",
                            session.dataset.recipes[f.recipe as usize].id,
                            f.region,
                            f.blocks,
                            f.last_staff_ratio_bps,
                            f.last_output.lots(),
                            f.last_inputs_short,
                            f.target_bps
                        )
                    })
                    .collect();
                let stock: Vec<String> = firm
                    .inventory
                    .iter()
                    .filter(|(_, l)| l.qty.0 != 0)
                    .map(|((p, r), l)| format!("p{p}r{r}:{}", l.qty.lots()))
                    .collect();
                let prices: Vec<String> = firm
                    .policies
                    .prices
                    .iter()
                    .map(|(p, c)| format!("p{p}=${}", c / 100))
                    .collect();
                let sold: i64 = firm.sales_history.iter().flat_map(|w| w.values()).sum();
                let contracts: Vec<String> = world
                    .contracts
                    .iter()
                    .filter(|(_, c)| c.seller == handle)
                    .map(|(_, c)| {
                        format!(
                            "to{}p{}q{}${}{:?}rem{}{}",
                            c.buyer.0 & 0xffff,
                            c.product,
                            c.quantity_weekly.lots(),
                            c.price_cents / 100,
                            c.status,
                            c.remaining_weeks,
                            if c.auto_renew { "R" } else { "" }
                        )
                    })
                    .collect();
                println!("   contracts {:?}", contracts);
                println!(
                    "w{week} h{} {:?} {:?} cash=${} rev=${} cogs=${} wages=${} opex=${} dep=${} ni=${} sold13={} prices={:?} stock={:?} fac={:?}",
                    handle.0 & 0xffff,
                    firm.archetype,
                    firm.status,
                    firm.book.cash() / 100,
                    sum(|w| w.revenue),
                    sum(|w| w.cost_of_goods),
                    sum(|w| w.wages),
                    sum(|w| w.operating_expense),
                    sum(|w| w.depreciation),
                    sum(|w| w.net_income),
                    sold,
                    prices,
                    stock,
                    facilities
                );
            }
        }
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
    }
}
