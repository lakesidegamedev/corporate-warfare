use cw_data::{Difficulty, PricePath};
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode};
fn main() {
    let name = std::env::args().nth(2).unwrap_or("Gulf Broadcast Network".into());
    let sc = std::env::args().nth(1).unwrap_or("texas-1981".into());
    let mut s = Session::create(ScenarioConfig { seed:1, scenario: ScenarioKind::Guided, mode: ScenarioMode::Sandbox, dataset_hash: String::new(), difficulty: Difficulty::Standard, economy_start: Default::default(), starting_capital_cents: None, prices: PriceSetup{scenario_id: sc, path: Some(PricePath::HistoricalVariance)}}).unwrap();
    for w in 0..=8u32 {
        if w>0 { s.advance(1); }
        let (_, f) = s.world.firms.iter().find(|(_, f)| f.name==name).unwrap();
        println!("w{w} mark={} fair={} ent={} eq={} ntangible={} ebitda13={} cash={} annEb={} issued={}", f.shares.mark_cents, f.shares.fair_cents, f.valuation.enterprise_cents, f.valuation.equity_cents, f.valuation.net_tangible_cents, f.valuation.ebitda_trailing_cents, f.book.cash(), cw_sim::economy::valuation::annualised_ebitda(&f.book), f.shares.eligible_votes());
        for wk in f.book.recent_weeks().iter().take(3) { println!("   rev={} ebitda={}", wk.revenue, wk.ebitda()); }
    }
}
