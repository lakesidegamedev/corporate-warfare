//! The CFO: when a company is losing money, name the few things that cost it most and give
//! each a button that fixes it.
//!
//! The advice reads the firm's recent weekly closes and its current policies. It speaks only
//! when the last four closed weeks add up to a loss, and only about things that matter against
//! the firm's sales. Each item carries ready-made commands, so every platform's button is the
//! same: submit the commands.

use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::commands::CommandIntent;
use cw_sim::state::World;

/// Most items the CFO shows at once.
pub const MAX_ADVICE: usize = 3;
/// Weeks of results the CFO reads.
const WINDOW: usize = 4;
/// An item must cost at least this share of weekly sales, in basis points, to be worth raising.
const MIN_SHARE_BPS: i64 = 200;
/// ... and at least this much a week, in cents.
const MIN_WEEKLY_CENTS: i64 = 50_000;

/// One thing costing the company money, with the fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CfoAdvice {
    /// Stable id of the kind of problem: research, marketing, training, quality, below_cost,
    /// overproduction.
    pub key: String,
    pub headline: String,
    pub detail: String,
    /// What it costs the company each week, in the firm's currency cents.
    pub weekly_cents: i64,
    pub action_label: String,
    /// Command intents as JSON, submitted in order when the player presses the button.
    pub commands: Vec<String>,
}

fn json(intent: CommandIntent) -> String {
    serde_json::to_string(&intent).unwrap_or_default()
}

/// The CFO's top reasons profit is down, biggest first. Empty when the company is not losing
/// money or nothing stands out.
pub fn advice(world: &World, dataset: &Dataset, firm: u64) -> Vec<CfoAdvice> {
    let handle = Handle(firm);
    let Some(entity) = world.firms.get(handle) else {
        return Vec::new();
    };
    let weeks = entity.book.recent_weeks();
    if weeks.len() < 2 {
        return Vec::new();
    }
    let recent = &weeks[weeks.len().saturating_sub(WINDOW)..];
    let net: i64 = recent.iter().map(|w| w.net_income).sum();
    if net >= 0 {
        return Vec::new();
    }
    let revenue = recent.iter().map(|w| w.revenue).sum::<i64>() / recent.len() as i64;
    let policies = &entity.policies;
    let mut items: Vec<CfoAdvice> = Vec::new();

    if policies.research_budget_cents > 0 {
        let level = entity.tech_level;
        items.push(CfoAdvice {
            key: "research".to_owned(),
            headline: "Research is costing more than it earns".to_owned(),
            detail: format!(
                "Each technology level trims input and wage bills by about 2 percent and takes \
                 weeks of spending to reach. You are at level {level}. Pause it until the company \
                 makes money, then fund it from profit."
            ),
            weekly_cents: policies.research_budget_cents,
            action_label: "Pause research".to_owned(),
            commands: vec![json(CommandIntent::SetResearchBudget {
                firm: handle,
                amount_cents: 0,
            })],
        });
    }
    if policies.marketing_budget_cents > 0 {
        items.push(CfoAdvice {
            key: "marketing".to_owned(),
            headline: "Marketing is a standing weekly bill".to_owned(),
            detail: "Advertising builds brand slowly and pays out over months. While the \
                     company is losing money, cut it back to nothing and restart later."
                .to_owned(),
            weekly_cents: policies.marketing_budget_cents,
            action_label: "Stop marketing".to_owned(),
            commands: vec![json(CommandIntent::SetMarketingBudget {
                firm: handle,
                amount_cents: 0,
            })],
        });
    }
    if policies.training_budget_cents > 0 {
        items.push(CfoAdvice {
            key: "training".to_owned(),
            headline: "Training is a standing weekly bill".to_owned(),
            detail: "Training moves a few workers up a skill tier each week. It helps output \
                     over years, not weeks. Pause it while you are in the red."
                .to_owned(),
            weekly_cents: policies.training_budget_cents,
            action_label: "Stop training".to_owned(),
            commands: vec![json(CommandIntent::SetTrainingBudget {
                firm: handle,
                amount_cents: 0,
            })],
        });
    }
    let quality: Vec<(u16, i64)> = policies
        .quality_budgets_cents
        .iter()
        .filter(|(_, amount)| **amount > 0)
        .map(|(product, amount)| (*product, *amount))
        .collect();
    if !quality.is_empty() {
        items.push(CfoAdvice {
            key: "quality".to_owned(),
            headline: "Quality spending is running every week".to_owned(),
            detail: "Better quality lifts what buyers pay, slowly. If sales are not covering \
                     your costs yet, stop the spending and bring it back when they do."
                .to_owned(),
            weekly_cents: quality.iter().map(|(_, amount)| amount).sum(),
            action_label: "Stop quality spending".to_owned(),
            commands: quality
                .iter()
                .map(|(product, _)| {
                    json(CommandIntent::SetQualityBudget {
                        firm: handle,
                        product: *product,
                        amount_cents: 0,
                    })
                })
                .collect(),
        });
    }

    let lines = crate::dto::product_lines(world, dataset, firm);
    let mut below_cost = 0i64;
    let mut below_names: Vec<String> = Vec::new();
    let mut below_commands: Vec<String> = Vec::new();
    for line in &lines {
        if line.unit_cost_cents > 0
            && line.sold_last_week_lots > 0
            && line.price_cents < line.unit_cost_cents
        {
            below_cost += (line.unit_cost_cents - line.price_cents) * line.sold_last_week_lots;
            below_names.push(line.name.clone());
            below_commands.push(json(CommandIntent::SetPricePosture {
                firm: handle,
                product: line.product,
                mode: 2,
                bps: 500,
            }));
        }
    }
    if below_cost > 0 {
        items.push(CfoAdvice {
            key: "below_cost".to_owned(),
            headline: "You are selling below what it costs to make".to_owned(),
            detail: format!(
                "{} sells for less than a lot costs to produce, so every sale loses money. \
                 Price it at your cost plus 5 percent, then adjust from there.",
                below_names.join(", ")
            ),
            weekly_cents: below_cost,
            action_label: "Price above cost".to_owned(),
            commands: below_commands,
        });
    }

    if policies.manual_production {
        let waste: i64 = lines
            .iter()
            .filter(|line| line.made_last_week_lots > line.sold_last_week_lots)
            .map(|line| {
                (line.made_last_week_lots - line.sold_last_week_lots) * line.unit_cost_cents
            })
            .sum();
        if waste > 0 {
            items.push(CfoAdvice {
                key: "overproduction".to_owned(),
                headline: "You make more than you sell".to_owned(),
                detail: "Production is set by hand, so the plants keep running while stock piles \
                         up unsold. Let the plants plan production from what actually sells."
                    .to_owned(),
                weekly_cents: waste,
                action_label: "Plan production automatically".to_owned(),
                commands: vec![json(CommandIntent::SetAutoProduction {
                    firm: handle,
                    enabled: true,
                })],
            });
        }
    }

    items.retain(|item| {
        item.weekly_cents >= MIN_WEEKLY_CENTS
            && item.weekly_cents * 10_000 >= revenue.max(0) * MIN_SHARE_BPS
    });
    items.sort_by(|a, b| b.weekly_cents.cmp(&a.weekly_cents).then(a.key.cmp(&b.key)));
    items.truncate(MAX_ADVICE);
    items
}
