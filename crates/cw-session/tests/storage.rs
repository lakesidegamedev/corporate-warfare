//! Storage through the session: a save carries held goods, orders and stock targets; alerts
//! appear only when storage matters; telemetry reports storage use, fees and spoilage.

use cw_core::quantity::MilliLots;
use cw_session::{ScenarioConfig, Session};
use cw_sim::economy::storage::SpecOrder;
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn config() -> ScenarioConfig {
    ScenarioConfig {
        seed: 41,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    }
}

#[test]
fn a_save_carries_held_goods_orders_and_stock_targets() {
    let mut session = Session::create(config()).unwrap();
    let firm = session.world.investor.primary_group.unwrap();
    let steel = session.dataset.product_index["steel"];
    let region = session.world.firm(firm).unwrap().region;
    let plant = session.world.firm(firm).unwrap().facilities[0];
    {
        let entity = session.world.firms.get_mut(firm).unwrap();
        entity
            .stock_lot_mut(steel, region)
            .receive(MilliLots::from_lots(80), cw_core::money::Cents(0));
        entity
            .storage
            .held
            .insert((steel, region), MilliLots::from_lots(60));
        entity.storage.orders.push(SpecOrder {
            product: steel,
            region,
            lots_left: 150,
            limit_bps: 11_000,
            expires: 90,
        });
    }
    session.world.facilities.get_mut(plant).unwrap().stock_plan =
        cw_sim::economy::storage::StockPlan {
            weeks: 9,
            seasonal: false,
        };
    session.advance(4);
    let bytes = session.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    let a = session.world.firm(firm).unwrap();
    let b = loaded.world.firm(firm).unwrap();
    assert_eq!(
        a.storage, b.storage,
        "held goods, orders and the weekly record survive a save"
    );
    assert_eq!(
        session.world.facilities.get(plant).unwrap().stock_plan,
        loaded.world.facilities.get(plant).unwrap().stock_plan
    );
    // Both continue identically.
    session.advance(8);
    loaded.advance(8);
    assert_eq!(session.canonical_hash(), loaded.canonical_hash());
}

#[test]
fn storage_alerts_only_appear_when_storage_matters() {
    let mut session = Session::create(config()).unwrap();
    let firm = session.world.investor.primary_group.unwrap();
    let quiet = session.overview();
    assert!(
        quiet
            .alerts
            .iter()
            .all(|a| !a.key.starts_with("alert.storage") && a.key != "alert.spoilage"),
        "a new company has no storage alerts"
    );
    {
        let entity = session.world.firms.get_mut(firm).unwrap();
        for _ in 0..4 {
            entity
                .storage
                .history
                .push(cw_sim::economy::storage::StorageWeek {
                    fee_cents: 400_000,
                    spoiled_cents: 300_000,
                    ..Default::default()
                });
        }
        // Revenue stays tiny against the costs.
    }
    let loud = session.overview();
    let keys: Vec<&str> = loud.alerts.iter().map(|a| a.key.as_str()).collect();
    assert!(keys.contains(&"alert.storage_fees"), "{keys:?}");
    assert!(keys.contains(&"alert.spoilage"), "{keys:?}");
}

#[test]
fn the_snapshot_reports_storage_use_fees_and_spoilage() {
    let mut session = Session::create(config()).unwrap();
    session.advance(13);
    let snapshot = session.market_snapshot();
    assert_eq!(snapshot.schema, 3);
    assert!(snapshot.storage.capacity_lots > 0, "yards count as space");
    assert!(snapshot.storage.used_lots > 0);
    let player = snapshot.player.as_ref().expect("the player has a company");
    assert!(player.storage.capacity_lots > 0);
    let json = serde_json::to_string(&snapshot).unwrap();
    for key in [
        "outside_fees_13w_cents",
        "holding_13w_cents",
        "spoiled_lots_13w",
        "spoiled_13w_cents",
        "held_lots",
        "open_buy_orders",
    ] {
        assert!(json.contains(key), "{key} reported");
    }
}
