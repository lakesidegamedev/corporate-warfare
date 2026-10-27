use cw_data::{Difficulty, PricePath};
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode};
fn main() {
    let sc = std::env::args().nth(1).unwrap_or("texas-1981".into());
    let mut s = Session::create(ScenarioConfig { seed:1, scenario: ScenarioKind::Guided, mode: ScenarioMode::Sandbox, dataset_hash: String::new(), difficulty: Difficulty::Standard, economy_start: Default::default(), starting_capital_cents: None, prices: PriceSetup{scenario_id: sc, path: Some(PricePath::HistoricalVariance)}}).unwrap();
    let exp: Vec<(cw_core::ids::Handle,String,i64,i64)> = s.world.firms.iter().map(|(h,f)| (h,f.name.clone(), cw_sim::economy::seed::expected_annual_ebitda(&s.world,&s.dataset,h), f.book.net_tangible_assets())).collect();
    for w in 1..=52u32 { let r=s.advance(1); if r.pause.is_some(){ for e in s.world.events.pending.iter_mut(){e.resolved=true;} s.world.pending_pause=None;} 
      if w==26 || w==52 {
        println!("--- week {w}");
        for (h,n,e,nt) in &exp { if let Some(f)=s.world.firms.get(*h) { let a=cw_sim::economy::valuation::annualised_ebitda(&f.book); println!("{:<30} expected={:>12} realised={:>12} ratio={:>6.2} ntangible={:>12} fair={}", n, e, a, a as f64/(*e).max(1) as f64, nt, f.shares.fair_cents);} }
      }
    }
}
