#!/usr/bin/env python3
"""Deeper product chains (1.7 preview 2): new products, multi-output recipes, schema 2.

Oil, gas, iron and food each become a chain with real intermediate goods. Plants may make
several products in a fixed yield split (`by_products`, lots per block per week at full capacity).
Yields, prices and plant costs are authored game constants, anchored to rough circa 1980 to 2000
US proportions and not to measurements:

  crude to products: a refinery's slate is about 45 percent gasoline, 25 percent distillate
  (diesel and heating fuel, here "fuel"), 9 percent jet fuel and the rest petrochemical
  feedstock (naphtha); crack spread about 15 to 25 percent over crude.
  gas: a gas plant strips natural gas liquids (ethane to butane) from wet gas; NGL is priced per
  lot above dry gas because a lot of liquids carries more energy and value than a lot of gas.
  iron: ore and coal make pig iron in a blast furnace with slag sold as cement feed; a basic
  oxygen furnace turns pig iron and scrap into steel; an electric arc furnace melts scrap alone.
  Scrap is a by-product of steel works, component stamping and vehicle assembly.
  food: mixed farms grow grain and raise livestock together; processors turn both into packaged
  food. Perishables spoil each week they sit unsold.

The yield and crew of each new recipe are solved so a plant earns about 20 percent of revenue
at reference prices after inputs, wages, maintenance, land and depreciation, as the margin
rebalance of 1.3.1 asked (tools/content/rebalance_margins.py). Re-running is a no-op.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "content/initial-balance.json"
FIRMS = ROOT / "content/firms.json"
TARGET = 0.20

NEW_PRODUCTS = [
    # id, name, reference price (cents per lot), spoilage bps per week
    ("crude_oil", "Crude oil", 15000, 0),
    ("gasoline", "Gasoline", 31000, 0),
    ("jet_fuel", "Jet fuel", 33000, 0),
    ("petrochem_feedstock", "Petrochemical feedstock", 26000, 0),
    ("ngl", "Natural gas liquids", 50000, 0),
    ("pig_iron", "Pig iron", 32000, 0),
    ("scrap", "Steel scrap", 18000, 0),
    ("grain", "Grain", 6000, 150),
    ("livestock", "Livestock", 12000, 300),
]
# Perishables on the existing goods spoil too.
SPOILAGE = {"food_raw": 400, "food_retail": 400}

# industry, id, name, output, lots, by-products, inputs, crew guess, capex, build weeks, life
RECIPES = [
    ("chemical_industries", "refinery", "Oil Refining", "fuel", 26,
     {"gasoline": 36, "jet_fuel": 8, "petrochem_feedstock": 12},
     {"crude_oil": 80, "electricity": 2}, [3, 3, 1], 110_000_000, 52, 30),
    ("chemical_industries", "gas_processing", "Gas Processing", "natural_gas", 36,
     {"ngl": 15}, {"natural_gas": 45}, [1, 1, 1], 32_000_000, 26, 25),
    ("manufacturing", "blast_furnace", "Blast Furnace", "pig_iron", 48,
     {"building_materials": 6}, {"ore": 30, "coal": 10, "electricity": 1},
     [2, 2, 1], 60_000_000, 39, 25),
    ("manufacturing", "steel_mill", "Basic Oxygen Steel", "steel", 58, {},
     {"pig_iron": 40, "scrap": 10, "electricity": 1}, [2, 2, 1], 80_000_000, 39, 25),
    ("manufacturing", "electric_arc", "Electric Arc Steel", "steel", 50, {},
     {"scrap": 52, "electricity": 2}, [2, 2, 1], 50_000_000, 26, 20),
    ("manufacturing", "food_processing", "Food Processing", "food_raw", 175, {},
     {"grain": 120, "livestock": 20, "electricity": 1}, [6, 3, 1], 40_000_000, 13, 20),
    ("agriculture", "mixed_farm", "Mixed Grain and Livestock", "grain", 190,
     {"livestock": 30}, {"fuel": 4, "electricity": 2}, [8, 2, 1], 45_000_000, 13, 20),
]

# Existing recipes that change: output swaps, input swaps and new by-products.
OIL_WELLS = dict(output="crude_oil", weekly_output_lots=74,
                 by_products=[{"product": "natural_gas", "weekly_lots": 5}])
INPUT_SWAPS = {
    # plastics from feedstock and NGL instead of fuel and gas
    "plastics_plant": {"electricity": 2, "petrochem_feedstock": 6, "ngl": 2},
    # chemicals from feedstock as well as fuel
    "chemical_plant": {"fuel": 8, "petrochem_feedstock": 13, "ore": 2, "electricity": 2},
}
EXTRA_INPUTS = {
    "aerospace": {"jet_fuel": 4},
    "full_service": {"gasoline": 4},
    "auto_dealer": {"gasoline": 2},
}
SCRAP_BY_PRODUCT = {"metal_works": 4, "component_works": 3, "vehicle_assembly": 2}

# Opening plants that give import-only goods a local producer.
# A firm runs one strategy per sector in a region (a division), so a firm's plants of one sector
# sit in different regions.
EXTRA_PLANTS = {
    "scioto_chemical": [("refinery", "OH-NW", 2)],
    "black_shale": [("gas_wells", "WV-NOR", 4), ("oil_wells", "OH-SE", 3),
                    ("gas_processing", "WV-NOR", 2), ("rare_earth_mine", "KY-EAS", 2)],
    "ironwood_ore": [("timber_forestry", "MI-MID", 3)],
    "allegheny_polymers": [("pharma_plant", "PA-PHL", 2)],
    "cuyahoga_metal": [("blast_furnace", "PA-WES", 2), ("steel_mill", "PA-CEN", 2),
                       ("electric_arc", "IN-NW", 1), ("building_products", "OH-CEN", 3)],
    "maumee_valley_grain": [("mixed_farm", "IN-CEN", 4), ("food_processing", "OH-NW", 2)],
}


def parts(recipe, prices, wages, maint_bps, land):
    revenue = sum(lots * prices[p] for p, lots in
                  [(recipe["output"], recipe["weekly_output_lots"])]
                  + [(b["product"], b["weekly_lots"]) for b in recipe.get("by_products", [])])
    inputs = sum(q * prices[k] for k, q in recipe["weekly_inputs_lots"].items())
    payroll = sum(a * b for a, b in zip(recipe["workers_low_mid_high"], wages))
    upkeep = recipe["capex_cents"] * maint_bps // 10_000 // 52 + land
    depreciation = recipe["capex_cents"] // (recipe["life_years"] * 52)
    return revenue, inputs, payroll, upkeep, depreciation


def margin(recipe, *args):
    revenue, inputs, payroll, upkeep, depreciation = parts(recipe, *args)
    return (revenue - inputs - payroll - upkeep - depreciation) / revenue


def solve_crew(recipe, *args):
    """Trim or grow the crew toward the target margin, keeping the tier mix."""
    wages = args[1]
    revenue, inputs, payroll, upkeep, depreciation = parts(recipe, *args)
    budget = revenue * (1 - TARGET) - inputs - upkeep - depreciation
    crew = recipe["workers_low_mid_high"]
    if budget <= 0:
        return
    factor = budget / payroll
    new = [0 if w == 0 else max(1, round(w * factor)) for w in crew]
    largest = max(range(3), key=lambda t: crew[t] * wages[t])
    others = sum(new[t] * wages[t] for t in range(3) if t != largest)
    new[largest] = max(1, round((budget - others) / wages[largest]))
    recipe["workers_low_mid_high"] = new


def main():
    data = json.loads(PATH.read_text())
    if any(p["id"] == "crude_oil" for p in data["products"]):
        print("chains already applied")
        return
    data["schema"] = 2
    for pid, name, price, spoil in NEW_PRODUCTS:
        product = {"id": pid, "name": name, "reference_price_cents": price, "kind": "goods"}
        if spoil:
            product["spoilage_bps_weekly"] = spoil
        data["products"].append(product)
    for product in data["products"]:
        if product["id"] in SPOILAGE:
            product["spoilage_bps_weekly"] = SPOILAGE[product["id"]]
    by_id = {r["id"]: r for r in data["recipes"]}
    by_id["oil_wells"].update(OIL_WELLS)
    for rid, inputs in INPUT_SWAPS.items():
        by_id[rid]["weekly_inputs_lots"] = inputs
    for rid, extra in EXTRA_INPUTS.items():
        by_id[rid]["weekly_inputs_lots"].update(extra)
    for rid, lots in SCRAP_BY_PRODUCT.items():
        by_id[rid].setdefault("by_products", []).append({"product": "scrap", "weekly_lots": lots})
    prices = {p["id"]: p["reference_price_cents"] for p in data["products"]}
    args = (prices, data["baseline_weekly_wage_cents"], data["maintenance_annual_bps"],
            data["fixture_land_rent_cents_weekly"])
    for industry, rid, name, output, lots, by, inputs, crew, capex, build, life in RECIPES:
        recipe = {
            "industry": industry, "id": rid, "name": name, "output": output,
            "weekly_output_lots": lots,
            "weekly_inputs_lots": dict(sorted(inputs.items())),
            "workers_low_mid_high": crew, "capex_cents": capex,
            "build_weeks": build, "life_years": life,
        }
        if by:
            recipe["by_products"] = [{"product": k, "weekly_lots": v} for k, v in by.items()]
        solve_crew(recipe, *args)
        data["recipes"].append(recipe)
        by_id[rid] = recipe
    for rid in ["oil_wells", "plastics_plant", "chemical_plant"] + [r[1] for r in RECIPES]:
        print(f"{rid:16} margin {margin(by_id[rid], *args):6.1%}  crew {by_id[rid]['workers_low_mid_high']}")
    PATH.write_text(json.dumps(data, indent=2) + "\n")

    firms = json.loads(FIRMS.read_text())
    for firm in firms["firms"]:
        if firm["id"] in EXTRA_PLANTS:
            firm["extra_plants"] = [
                {"recipe": r, "region": g, "blocks": b} for r, g, b in EXTRA_PLANTS[firm["id"]]
            ]
    FIRMS.write_text(json.dumps(firms, indent=2) + "\n")


if __name__ == "__main__":
    main()
