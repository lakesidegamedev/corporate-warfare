#!/usr/bin/env python3
"""Storage (1.7 preview 2): storage classes and holding costs for goods, a storage space
service and four storage facilities in logistics, balance schema 3.

Every good declares a storage class (dry warehouse, tank farm, silo, cold store) and a weekly
holding cost in basis points of its reference price. Storage facilities are ordinary recipes in
the logistics industry whose `weekly_output_lots` is the lots of space one block holds; they make
no goods. Space, fees and costs are authored game constants. Re-running is a no-op.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "content/initial-balance.json"

CLASS = {
    "tank": ["fuel", "gasoline", "jet_fuel", "crude_oil", "natural_gas", "petrochem_feedstock",
             "ngl", "chemicals"],
    "silo": ["grain", "ore", "coal", "scrap", "pig_iron", "building_materials", "fertilizers"],
    "cold": ["food_raw", "food_retail", "livestock", "pharmaceuticals"],
}
HOLDING_BPS = {"dry": 8, "tank": 6, "silo": 5, "cold": 12}

SPACE = {
    "id": "storage_space",
    "name": "Storage space",
    "reference_price_cents": 100,
    "kind": "service",
}

# id, name, class, lots per block, capex cents, build weeks, life years, crew, inputs
RECIPES = [
    ("warehouse", "Warehouse", "dry", 5000, 40_000_000, 8, 25, [2, 1, 0], {}),
    ("tank_farm", "Tank Farm", "tank", 4000, 60_000_000, 13, 30, [2, 1, 1], {}),
    ("grain_silo", "Grain Silo", "silo", 8000, 35_000_000, 8, 30, [1, 1, 0], {}),
    ("cold_store", "Cold Store", "cold", 5000, 55_000_000, 13, 20, [2, 1, 1], {"electricity": 1}),
]


def main():
    data = json.loads(PATH.read_text())
    data["schema"] = 3
    by_class = {pid: cls for cls, ids in CLASS.items() for pid in ids}
    for product in data["products"]:
        if product["kind"] in ("financial_contract",) or product["id"] == "storage_space":
            continue
        if product["kind"] in ("local_service", "project_service", "transport_service",
                               "subscription", "service", "lease_service"):
            continue
        cls = by_class.get(product["id"], "dry")
        product["storage_class"] = cls
        product["holding_bps_weekly"] = HOLDING_BPS[cls]
    if not any(p["id"] == SPACE["id"] for p in data["products"]):
        data["products"].append(dict(SPACE))
    have = {r["id"] for r in data["recipes"]}
    for rid, name, cls, lots, capex, weeks, life, crew, inputs in RECIPES:
        if rid in have:
            continue
        data["recipes"].append({
            "industry": "logistics",
            "id": rid,
            "name": name,
            "output": "storage_space",
            "weekly_output_lots": lots,
            "storage_class": cls,
            "weekly_inputs_lots": inputs,
            "workers_low_mid_high": crew,
            "capex_cents": capex,
            "build_weeks": weeks,
            "life_years": life,
        })
    PATH.write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    main()
