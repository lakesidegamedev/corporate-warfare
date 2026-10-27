//! Rival memory evidence: raids, hostile bids and ventures create grudges and alliances through
//! the rules that resolve them, and time fades them. The memory is part of the saved world.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{deals, rivalry, seed, structure};
use cw_sim::state::{DealKind, HolderId, ScenarioKind, ScenarioMode, World};

fn world(seed_value: u64) -> (World, Dataset, Handle, Handle) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Guided,
        ScenarioMode::Sandbox,
    );
    let rivals: Vec<Handle> = world
        .firms
        .handles_sorted()
        .into_iter()
        .filter(|h| world.firms.get(*h).is_some_and(|f| f.archetype.is_some()))
        .collect();
    let (a, b) = (rivals[0], rivals[1]);
    (world, dataset, a, b)
}

fn fund(world: &mut World, firm: Handle, cents: i64) {
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test funding")
            .credit(Account::ShareCapital, cents, "test funding"),
    );
}

#[test]
fn a_raid_leaves_the_defender_with_a_grudge_a_failed_one_with_less() {
    let (mut w, _, attacker, defender) = world(11);
    let mut record = cw_sim::state::RaidRecord {
        tick: w.tick.0,
        attacker,
        defender,
        region: 0,
        industry: 0,
        budget_cents: 1_000_000,
        chance_bps: 5_000,
        success: true,
        blocks: 2,
        price_cents: 0,
    };
    structure::note_raid(&mut w, &record, 2_000);
    let taken = rivalry::grudge(&w, defender, attacker);
    assert_eq!(taken, 3_000 + 600 * 2);
    assert_eq!(rivalry::grudge(&w, attacker, defender), 0);
    let (mut w2, _, a2, d2) = world(11);
    record.success = false;
    record.attacker = a2;
    record.defender = d2;
    structure::note_raid(&mut w2, &record, 2_000);
    assert_eq!(rivalry::grudge(&w2, d2, a2), 1_500);
    assert!(rivalry::grudge(&w2, d2, a2) < taken);
}

#[test]
fn a_raid_on_the_players_company_is_remembered_against_the_investor() {
    let (mut w, _, rival, _) = world(12);
    let player = w.investor.primary_group.expect("player company");
    let record = cw_sim::state::RaidRecord {
        tick: w.tick.0,
        attacker: player,
        defender: rival,
        region: 0,
        industry: 0,
        budget_cents: 1,
        chance_bps: 5_000,
        success: true,
        blocks: 1,
        price_cents: 0,
    };
    structure::note_raid(&mut w, &record, 1_000);
    assert!(rivalry::grudge_against(&w, rival, HolderId::Investor) > 0);
    // The rival now targets the player: every rival with this grudge sees it.
    assert!(rivalry::grudge(&w, rival, player) > 0);
}

#[test]
fn a_hostile_bid_makes_an_enemy_and_a_venture_a_friend() {
    let (mut w, dataset, buyer, target) = world(13);
    fund(&mut w, buyer, 80_000_000_000);
    fund(&mut w, target, 4_000_000_000);
    let mark = w.firms.get(target).unwrap().shares.mark_cents.max(100);
    deals::start(&mut w, buyer, target, DealKind::SharePurchase, mark, 1_000).expect("bid opens");
    assert!(w.deals.iter().any(|(_, d)| d.hostile));
    assert_eq!(rivalry::grudge(&w, target, buyer), 2_500);
    assert_eq!(w.rivalry.stats.grudges_formed, 1);

    // The same two firms then share a venture: the grudge is cooled and a tie formed.
    let recipe = dataset
        .recipes
        .iter()
        .position(|r| {
            r.storage_class.is_none() && r.transport_mode.is_none() && r.capex_cents < 800_000_000
        })
        .unwrap() as u16;
    structure::form_jv(
        &mut w,
        &dataset,
        buyer,
        target,
        0,
        recipe,
        1,
        5_000,
        "Partners".into(),
    )
    .expect("venture forms");
    assert_eq!(rivalry::alliance(&w, buyer, target), 5_000);
    assert_eq!(rivalry::grudge(&w, target, buyer), 1_250, "friends forgive");
    assert_eq!(w.rivalry.stats.alliances_formed, 1);

    // While the venture runs the tie is held up; once gone it fades away.
    for _ in 0..40 {
        rivalry::weekly(&mut w);
    }
    assert!(rivalry::alliance(&w, buyer, target) >= 4_000);
    w.structure.jvs.clear();
    for _ in 0..900 {
        rivalry::weekly(&mut w);
    }
    assert_eq!(rivalry::alliance(&w, buyer, target), 0);
}

#[test]
fn memory_survives_a_whole_world_save_and_load() {
    let (mut w, _, a, b) = world(14);
    rivalry::note_grudge(&mut w, a, b, 4_000, rivalry::Cause::Raid);
    rivalry::note_alliance(&mut w, b, a, 3_000, rivalry::Cause::Deal);
    rivalry::record_move(&mut w, a, "raider", "raid", None, true);
    let bytes = postcard::to_allocvec(&w).unwrap();
    let back: World = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(back.rivalry, w.rivalry);
    assert_eq!(back.rivalry.stats.moves_by_pair["raider/raid"], 1);
}
