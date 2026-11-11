//! Raid alerts, all read from the world and never stored. Under 2.0 a raid is announced weeks
//! before it resolves (`raid_warning_weeks` in content), so the first job is to say plainly that
//! a campaign is coming: who, against which business, what is at stake, the chance today, the
//! weeks left and what defence would cost and do. Then: what a recent raid on the player did and
//! what defence costs; when a raid took the last block of the player's only business and which
//! ways on exist; and, before any raid, that a one block business is exposed.

use cw_data::Dataset;
use cw_sim::economy::events_engine::player_side;
use cw_sim::economy::sectors;
use cw_sim::state::World;

use crate::{Alert, paper};

/// Weeks a raid on the player stays an alert.
const RECENT_WEEKS: u32 = 3;

fn industry(index: u8) -> String {
    cw_sim::state::INDUSTRY_NAMES
        .get(index as usize)
        .copied()
        .unwrap_or("business")
        .to_lowercase()
}

fn pct(bps: i64) -> String {
    format!("{}.{:02}", bps / 100, bps % 100)
}

/// The defence line: what a doubled defence costs per week and what it does to the odds of a
/// raid with `attack_cents` behind it.
fn defence_line(
    world: &World,
    dataset: &Dataset,
    firm: cw_core::ids::Handle,
    region: u8,
    industry_index: u8,
    attack_cents: i64,
) -> Option<String> {
    let now = sectors::raid_exposure(
        world,
        dataset,
        firm,
        region,
        industry_index,
        attack_cents,
        0,
    );
    if now.blocks_at_stake == 0 {
        return None;
    }
    let extra = (now.defence_cents / 13).max(100);
    let better = sectors::raid_exposure(
        world,
        dataset,
        firm,
        region,
        industry_index,
        attack_cents,
        extra,
    );
    let money = |cents: i64| paper::money(world, dataset, firm, cents);
    Some(format!(
        "Defence is a quarter of your marketing spend plus a little of your output. Raising weekly marketing by {} (a defence of {} instead of {}) cuts their chance from {} to {} percent; set it in the company's marketing budget.",
        money(extra),
        money(better.defence_cents),
        money(now.defence_cents),
        pct(now.chance_bps as i64),
        pct(better.chance_bps as i64),
    ))
}

/// One `alert.raid_announced` per campaign announced against the player's companies.
fn announced(world: &World, dataset: &Dataset, alerts: &mut Vec<Alert>) {
    for view in crate::campaigns(world, dataset) {
        if view.role != crate::dto::CampaignRole::AgainstPlayer {
            continue;
        }
        let weeks = view.weeks_left;
        let defence = defence_line(
            world,
            dataset,
            cw_core::ids::Handle(view.defender),
            view.region,
            view.industry,
            view.attack_cents,
        )
        .map(|line| format!(" {line}"))
        .unwrap_or_default();
        alerts.push(Alert {
            key: "alert.raid_announced".to_owned(),
            subject: view.defender_name.clone(),
            severity: 2,
            detail: format!(
                "{} has announced a raid on {}'s {} business in {}, with {} behind it. At stake: {} of your {}. Their chance today is {} percent; it is decided in {} week{}, against the defence you have then.{defence}",
                view.attacker_name,
                view.defender_name,
                view.industry_name,
                view.region_name,
                paper::money(world, dataset, cw_core::ids::Handle(view.defender), view.budget_cents),
                blocks(view.blocks_at_stake as u32),
                blocks(view.division_blocks),
                pct(view.chance_bps as i64),
                weeks,
                if weeks == 1 { "" } else { "s" },
            ),
        });
    }
}

fn blocks(n: u32) -> String {
    format!("{n} block{}", if n == 1 { "" } else { "s" })
}

pub fn raid_alerts(world: &World, dataset: &Dataset, alerts: &mut Vec<Alert>) {
    announced(world, dataset, alerts);
    let now = world.tick.0;
    for raid in world.raids.iter() {
        if !player_side(world, raid.defender) || player_side(world, raid.attacker) {
            continue;
        }
        let (Some(raider), Some(firm)) = (
            paper::firm_name(world, raid.attacker),
            paper::firm_name(world, raid.defender),
        ) else {
            continue;
        };
        let region = paper::region_name(world, raid.region);
        let kind = industry(raid.industry);
        let chance = pct(raid.chance_bps as i64);
        let empty = world
            .firm(raid.defender)
            .is_some_and(|f| f.operative() && f.facilities.is_empty());
        if raid.success && empty {
            alerts.push(Alert {
                key: "alert.raid_last_block".to_owned(),
                subject: firm.clone(),
                severity: 3,
                detail: format!(
                    "{raider} took the last block of {firm}'s only business ({kind} in {region}). {firm} now has no plants, so it earns nothing while its costs and debts continue. Ways on: build a new plant for {firm} with its cash, or found a new company with your personal cash."
                ),
            });
        }
        if raid.tick + RECENT_WEEKS < now {
            continue;
        }
        let attack = raid.budget_cents
            + world
                .firms
                .get(raid.attacker)
                .map(|f| f.policies.marketing_budget_cents.max(0) * 13)
                .unwrap_or(0);
        let defence = defence_line(
            world,
            dataset,
            raid.defender,
            raid.region,
            raid.industry,
            attack,
        )
        .map(|line| format!(" {line}"))
        .unwrap_or_default();
        let detail = if raid.success {
            format!(
                "{raider} raided {firm}'s {kind} business in {region} and won: {} block{} taken, {} paid to you (half of book value). Their chance was {chance} percent.{defence}",
                raid.blocks,
                if raid.blocks == 1 { "" } else { "s" },
                paper::money(world, dataset, raid.defender, raid.price_cents),
            )
        } else {
            format!(
                "{raider} raided {firm}'s {kind} business in {region} and failed. Their chance was {chance} percent.{defence}"
            )
        };
        alerts.push(Alert {
            key: "alert.raid".to_owned(),
            subject: firm,
            severity: if raid.success { 3 } else { 1 },
            detail,
        });
    }
    exposed_one_block(world, dataset, alerts);
}

/// A company whose whole business is one block, past the opening grace, can be lost whole to one
/// raid. Say so while there is still time to buy defence.
fn exposed_one_block(world: &World, dataset: &Dataset, alerts: &mut Vec<Alert>) {
    let Some(primary) = world
        .investor
        .primary_group
        .filter(|h| world.firms.contains(*h))
    else {
        return;
    };
    let divisions = sectors::divisions_of(world, dataset, primary);
    let [only] = divisions.as_slice() else {
        return;
    };
    if only.blocks != 1
        || world.tick.0
            < sectors::raid_open_tick(world, dataset, primary, only.region, only.industry)
    {
        return;
    }
    let Some(firm) = paper::firm_name(world, primary) else {
        return;
    };
    let base = sectors::raid_exposure(world, dataset, primary, only.region, only.industry, 0, 0);
    let attack = base.defence_cents * 2;
    let quote = sectors::raid_exposure(
        world,
        dataset,
        primary,
        only.region,
        only.industry,
        attack,
        0,
    );
    let region = paper::region_name(world, only.region);
    let tail = defence_line(world, dataset, primary, only.region, only.industry, attack)
        .map(|line| format!(" {line}"))
        .unwrap_or_default();
    alerts.push(Alert {
        key: "alert.raid_exposed".to_owned(),
        subject: firm.clone(),
        severity: 1,
        detail: format!(
            "{firm} is one {} business of a single block in {region}. A rival that spends {} on a raid has about a {} percent chance of taking all of it, and then {firm} would have nothing left to earn from.{tail}",
            industry(only.industry),
            paper::money(world, dataset, primary, attack),
            pct(quote.chance_bps as i64),
        ),
    });
}
