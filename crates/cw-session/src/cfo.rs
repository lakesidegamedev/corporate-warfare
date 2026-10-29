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
    /// overproduction, idle_capacity.
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

/// A division makes this many times what it sells or more before the CFO calls it far above.
const IDLE_RATIO_PCT: i64 = 150;

/// Service and carrier divisions run by hand far above what they sell: their output cannot be
/// stored, so everything over what sold is paid for and thrown away each week. Named with the
/// weekly cost of the excess and a button that hands production back to the weekly plan, which
/// runs them at what sells plus a margin. None when production is planned automatically, when
/// no such division stands out, or when the waste is too small to matter against sales.
pub fn idle_capacity(
    world: &World,
    dataset: &Dataset,
    firm: u64,
    revenue: i64,
) -> Option<CfoAdvice> {
    let handle = Handle(firm);
    let policies = &world.firms.get(handle)?.policies;
    if !policies.manual_production {
        return None;
    }
    let mut names = Vec::new();
    let mut waste = 0i64;
    for line in crate::dto::product_lines(world, dataset, firm) {
        let expires = dataset
            .products
            .get(line.product as usize)
            .is_some_and(|p| p.kind.is_service());
        if expires
            && line.made_last_week_lots > 0
            && line.made_last_week_lots * 100 >= line.sold_last_week_lots.max(0) * IDLE_RATIO_PCT
        {
            let excess = line.made_last_week_lots - line.sold_last_week_lots.max(0);
            waste = waste.saturating_add(excess.saturating_mul(line.unit_cost_cents.max(0)));
            names.push(format!(
                "{} (made {}, sold {})",
                line.name, line.made_last_week_lots, line.sold_last_week_lots
            ));
        }
    }
    if names.is_empty()
        || waste < MIN_WEEKLY_CENTS
        || waste.saturating_mul(10_000) < revenue.max(0).saturating_mul(MIN_SHARE_BPS)
    {
        return None;
    }
    Some(CfoAdvice {
        key: "idle_capacity".to_owned(),
        headline: "Some divisions run far above what they sell".to_owned(),
        detail: format!(
            "{} last week. A service or freight capacity that is not sold is lost, but you \
             still pay its wages and inputs. Match output to demand: production follows what \
             sells plus a margin, and you can set it by hand again any time.",
            names.join(", ")
        ),
        weekly_cents: waste,
        action_label: "Match output to demand".to_owned(),
        commands: vec![json(CommandIntent::SetAutoProduction {
            firm: handle,
            enabled: true,
        })],
    })
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
    let revenue = recent.iter().map(|w| w.revenue).sum::<i64>() / recent.len() as i64;
    let policies = &entity.policies;
    // Idle service or carrier capacity is raised even while the company makes money: it is
    // paid for every week and expires unsold.
    let idle = idle_capacity(world, dataset, firm, revenue);
    if net >= 0 {
        return idle.into_iter().collect();
    }
    let mut items: Vec<CfoAdvice> = idle.into_iter().collect();

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
                "{} {} for less than a lot costs to produce, so every sale loses money. \
                 Price {} at your cost plus 5 percent, then adjust from there.",
                name_list(&below_names),
                if below_names.len() == 1 {
                    "sells"
                } else {
                    "sell"
                },
                if below_names.len() == 1 { "it" } else { "them" }
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

/// "Crude oil", "Crude oil and natural gas", "Crude oil, natural gas and steel": names after
/// the first start lower case, as they sit inside a sentence.
fn name_list(names: &[String]) -> String {
    let lower = |n: &String| {
        let mut c = n.chars();
        match c.next() {
            Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
            None => String::new(),
        }
    };
    match names.len() {
        0 => String::new(),
        1 => names[0].clone(),
        n => {
            let rest: Vec<String> = names[1..n - 1].iter().map(lower).collect();
            let mut head = names[0].clone();
            for r in rest {
                head.push_str(", ");
                head.push_str(&r);
            }
            format!("{head} and {}", lower(&names[n - 1]))
        }
    }
}

#[cfg(test)]
mod name_list_tests {
    use super::name_list;

    #[test]
    fn lists_read_as_a_sentence() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(name_list(&v(&["Crude oil"])), "Crude oil");
        assert_eq!(
            name_list(&v(&["Crude oil", "Natural gas"])),
            "Crude oil and natural gas"
        );
        assert_eq!(
            name_list(&v(&["Crude oil", "Natural gas", "Steel"])),
            "Crude oil, natural gas and steel"
        );
    }
}
