#!/usr/bin/env python3
"""Author the AHD-style sector catalogue into content/initial-balance.json.

Each sector type (industry) gets several operating strategies for a January 2000 start, each a
recipe with its own inputs and output, after A House Divided's sector strategies. The existing
recipes stay as each sector's standard strategy with unchanged numbers. New goods are priced
cost-plus from their producing strategy at full capacity and baseline wages:
price = (inputs + payroll + maintenance + land) / (1 - target margin) / output, rounded.
Prints every strategy's weekly economics so the trade-offs can be reviewed.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "content/initial-balance.json"

# One plant (a block) is a fifth of the 1.1.0 unit, as in A House Divided where a corporation
# runs many small plants: output, inputs, workers, capex and land rent all divide by this, and
# seeded firms hold five times the blocks, so the economy keeps its capacity. The content file
# records the scale it is at, so rerunning the script never divides twice.
PLANT_SCALE = 5


def scaled(quantity, factor):
    """Divide a nonzero integer quantity, keeping at least one unit."""
    return 0 if quantity == 0 else max(1, round(quantity / factor))

NAMES = {
    "food_raw": "Food (farm output)", "ore": "Iron ore", "fuel": "Oil & fuel",
    "electricity": "Energy", "chemicals": "Industrial chemicals", "steel": "Steel",
    "components": "Electronics & components", "goods": "Consumer goods",
    "construction": "Construction services", "freight": "Freight",
    "food_retail": "Groceries", "goods_retail": "Retail goods", "care": "Healthcare services",
    "software": "Software", "advertising": "Advertising", "telecom": "Network services",
    "lease": "Real estate services", "defense": "Defense systems",
    "research": "Consulting & R&D", "maintenance": "Maintenance services",
    "training": "Training", "water": "Water",
}

# id, name, kind, producing recipe, target EBITDA margin
NEW_GOODS = [
    ("coal", "Coal", "goods", "coal_mine", 0.24),
    ("natural_gas", "Natural gas", "goods", "gas_wells", 0.30),
    ("timber", "Timber", "goods", "timber_forestry", 0.20),
    ("rare_earth", "Rare earth minerals", "goods", "rare_earth_mine", 0.32),
    ("plastics", "Plastics", "goods", "plastics_plant", 0.22),
    ("fertilizers", "Fertilizers", "goods", "fertilizer_plant", 0.22),
    ("pharmaceuticals", "Pharmaceuticals", "goods", "pharma_plant", 0.34),
    ("building_materials", "Building materials", "goods", "building_products", 0.20),
    ("vehicles", "Vehicles", "goods", "vehicle_assembly", 0.22),
]

# Display names for the existing recipes, which stay each sector's standard strategy.
STANDARD = {
    "farm": "Traditional", "mine": "Iron & Metals Mining", "generator": "Oil-fired Generation",
    "chemical_plant": "Industrial Chemicals", "metal_works": "Heavy Metals",
    "component_works": "Electronics Manufacturing", "assembly": "Consumer Goods Assembly",
    "contractor": "General Contracting", "carrier": "Trucking", "grocer": "Grocery",
    "goods_store": "General Merchandise", "clinic": "Outpatient & Preventive",
    "software_studio": "Software Focus", "media_studio": "Legacy Broadcast",
    "network": "Wireline Network", "rental_property": "Commercial Property",
    "defense_plant": "Standard Defense",
}

M = 1_000_000  # cents in $10,000
# industry, id, name, output, lots, inputs, workers [low, mid, high], capex $m, build weeks, life years
NEW_RECIPES = [
    ("agriculture", "farm_industrial", "Industrial", "food_raw", 900, {"fuel": 30, "electricity": 12, "fertilizers": 24}, [62, 12, 3], 3.0, 13, 20),
    ("agriculture", "farm_precision", "Precision Agriculture", "food_raw", 780, {"fuel": 18, "electricity": 14, "fertilizers": 12, "software": 8}, [40, 16, 5], 3.4, 13, 20),
    ("agriculture", "farm_sustainable", "Sustainable", "food_raw", 470, {"fuel": 8, "electricity": 8}, [52, 9, 2], 1.6, 13, 20),
    ("extraction", "coal_mine", "Coal Mining", "coal", 600, {"fuel": 18, "electricity": 10}, [70, 18, 3], 2.6, 26, 25),
    ("extraction", "gas_wells", "Oil & Gas", "natural_gas", 300, {"fuel": 10, "electricity": 6, "steel": 10}, [18, 14, 4], 4.2, 26, 20),
    ("extraction", "oil_wells", "Oil Production", "fuel", 280, {"electricity": 8, "steel": 8}, [20, 14, 4], 4.6, 26, 20),
    ("extraction", "rare_earth_mine", "Rare Earth Minerals Mining", "rare_earth", 60, {"fuel": 12, "electricity": 12, "chemicals": 6}, [30, 16, 4], 3.8, 39, 25),
    ("extraction", "timber_forestry", "Timber & Forestry", "timber", 500, {"fuel": 14}, [45, 8, 2], 1.4, 13, 30),
    ("energy", "generator_coal", "Coal-fired Generation", "electricity", 110, {"coal": 55}, [16, 12, 3], 5.4, 39, 30),
    ("energy", "generator_gas", "Gas Turbines", "electricity", 100, {"natural_gas": 30}, [8, 8, 3], 4.0, 26, 25),
    ("energy", "generator_nuclear", "Nuclear", "electricity", 240, {"chemicals": 6, "maintenance": 10}, [20, 30, 15], 24.0, 104, 40),
    ("energy", "generator_renewable", "Renewables Focus", "electricity", 45, {"maintenance": 3}, [3, 4, 2], 4.4, 26, 25),
    ("chemical_industries", "fertilizer_plant", "Fertilizer Production", "fertilizers", 300, {"natural_gas": 40, "electricity": 10}, [24, 16, 4], 3.2, 26, 25),
    ("chemical_industries", "pharma_plant", "Pharmaceuticals", "pharmaceuticals", 60, {"chemicals": 20, "electricity": 8, "research": 10}, [10, 30, 22], 5.0, 39, 25),
    ("chemical_industries", "plastics_plant", "Plastics & Polymers", "plastics", 240, {"fuel": 30, "natural_gas": 20, "electricity": 10}, [22, 16, 4], 3.6, 26, 25),
    ("manufacturing", "vehicle_assembly", "Vehicle Assembly", "vehicles", 40, {"steel": 60, "components": 25, "plastics": 30, "electricity": 12}, [70, 40, 10], 6.0, 39, 25),
    ("manufacturing", "building_products", "Building Products", "building_materials", 400, {"timber": 120, "ore": 40, "fuel": 10, "electricity": 8}, [40, 12, 3], 2.0, 20, 25),
    ("construction", "infrastructure", "Infrastructure Buildout", "construction", 26, {"steel": 40, "building_materials": 60, "fuel": 20}, [90, 60, 12], 1.6, 8, 20),
    ("construction", "modular", "Modular Construction", "construction", 16, {"building_materials": 70, "timber": 40, "electricity": 6}, [40, 40, 8], 1.2, 8, 20),
    ("logistics", "freight_rail", "Freight Rail", "freight", 1100, {"fuel": 40, "electricity": 6, "steel": 6}, [50, 20, 5], 6.0, 26, 30),
    ("logistics", "full_service", "Full-Service Logistics", "freight", 420, {"fuel": 14, "electricity": 6, "software": 8}, [40, 18, 5], 2.2, 8, 20),
    ("retail", "auto_dealer", "Auto & Department Store", "goods_retail", 120, {"goods": 70, "vehicles": 2, "electricity": 8}, [55, 25, 4], 2.0, 8, 25),
    ("retail", "ecommerce", "E-Commerce", "goods_retail", 90, {"goods": 70, "freight": 60, "software": 10, "electricity": 4}, [25, 18, 6], 1.0, 8, 15),
    ("healthcare", "hospital", "Hospital Networks", "care", 260, {"pharmaceuticals": 10, "electricity": 12, "maintenance": 10}, [40, 90, 45], 12.0, 52, 40),
    ("healthcare", "long_term_care", "Long-term Care", "care", 160, {"pharmaceuticals": 3, "electricity": 8, "maintenance": 4}, [60, 40, 8], 4.0, 26, 30),
    ("technology", "hardware_maker", "Hardware Focus", "components", 90, {"rare_earth": 8, "plastics": 10, "steel": 6, "electricity": 10}, [30, 25, 10], 3.0, 20, 15),
    ("technology", "it_services", "IT Services", "research", 60, {"software": 10, "electricity": 4}, [0, 18, 22], 0.6, 8, 15),
    ("media", "digital_media", "Digital-First", "advertising", 120, {"software": 12, "telecom": 80, "electricity": 4}, [3, 12, 14], 1.0, 8, 15),
    ("media", "publishing", "Print & Publishing", "advertising", 110, {"timber": 30, "electricity": 6}, [20, 14, 6], 1.6, 13, 20),
    ("telecommunications", "wireless", "Wireless Network", "telecom", 1300, {"electricity": 14, "components": 30}, [8, 22, 14], 7.0, 39, 20),
    ("telecommunications", "broadband", "Broadband & Internet", "telecom", 1100, {"electricity": 12, "components": 22, "software": 10}, [8, 24, 16], 6.0, 39, 20),
    ("real_estate", "residential", "Residential Development", "lease", 110, {"building_materials": 6, "maintenance": 4, "electricity": 4}, [10, 8, 2], 9.0, 52, 50),
    ("real_estate", "industrial_parks", "Industrial Parks", "lease", 140, {"maintenance": 5, "electricity": 6}, [12, 8, 2], 11.0, 52, 50),
    ("defense", "heavy_armor", "Heavy Armor", "defense", 22, {"steel": 120, "vehicles": 2, "components": 20, "electricity": 12}, [60, 45, 15], 6.0, 39, 25),
    ("defense", "aerospace", "Aerospace Systems", "defense", 18, {"components": 60, "rare_earth": 10, "chemicals": 20, "electricity": 12}, [30, 50, 30], 8.0, 52, 30),
    ("defense", "munitions", "Munitions", "defense", 24, {"chemicals": 60, "steel": 40, "electricity": 10}, [50, 35, 10], 4.0, 26, 25),
]


def main():
    data = json.loads(PATH.read_text())
    factor = PLANT_SCALE / data.get("plant_scale", 1)
    if factor != 1:
        # Rounding small input and crew counts would move each strategy's margin, so after
        # dividing them the output is re-solved to keep the strategy's calibrated margin.
        prices = {p["id"]: p["reference_price_cents"] for p in data["products"]}
        old_land = data["fixture_land_rent_cents_weekly"]
        new_land = round(old_land / factor)

        def unit_cost(recipe, land):
            inputs = sum(q * prices[k] for k, q in recipe["weekly_inputs_lots"].items())
            payroll = sum(a * b for a, b in zip(recipe["workers_low_mid_high"], data["baseline_weekly_wage_cents"]))
            return inputs + payroll + recipe["capex_cents"] * data["maintenance_annual_bps"] // 10_000 // 52 + land

        for recipe in data["recipes"]:
            price = prices[recipe["output"]]
            revenue = recipe["weekly_output_lots"] * price
            margin = (revenue - unit_cost(recipe, old_land)) / revenue
            recipe["weekly_inputs_lots"] = {k: scaled(v, factor) for k, v in recipe["weekly_inputs_lots"].items()}
            recipe["workers_low_mid_high"] = [scaled(w, factor) for w in recipe["workers_low_mid_high"]]
            recipe["capex_cents"] = max(1_000_000, round(recipe["capex_cents"] / factor / 1_000_000) * 1_000_000)
            recipe["weekly_output_lots"] = max(1, round(unit_cost(recipe, new_land) / (1 - margin) / price))
        data["fixture_land_rent_cents_weekly"] = new_land
        data["plant_scale"] = PLANT_SCALE
    wages = data["baseline_weekly_wage_cents"]
    maint_bps = data["maintenance_annual_bps"]
    land = data["fixture_land_rent_cents_weekly"]
    products = {p["id"]: p for p in data["products"]}
    for pid, name in NAMES.items():
        products[pid]["name"] = name
    recipes = {r["id"]: r for r in data["recipes"]}
    for rid, name in STANDARD.items():
        recipes[rid]["name"] = name
    for row in NEW_RECIPES:
        industry, rid, name, output, lots, inputs, workers, capex, weeks, life = row
        recipes[rid] = {
            "industry": industry, "id": rid, "name": name, "output": output,
            "weekly_output_lots": scaled(lots, PLANT_SCALE),
            "weekly_inputs_lots": dict(sorted((k, scaled(v, PLANT_SCALE)) for k, v in inputs.items())),
            "workers_low_mid_high": [scaled(w, PLANT_SCALE) for w in workers],
            "capex_cents": int(round(capex * 100 * M / PLANT_SCALE)),
            "build_weeks": weeks, "life_years": life,
        }

    def cost(recipe):
        inputs = sum(q * products[k]["reference_price_cents"] for k, q in recipe["weekly_inputs_lots"].items())
        payroll = sum(a * b for a, b in zip(recipe["workers_low_mid_high"], wages))
        maintenance = recipe["capex_cents"] * maint_bps // 10_000 // 52
        return inputs + payroll + maintenance + land

    # Margin and payback bias per alternative strategy, against its sector's standard: a strategy
    # is a trade-off of inputs, labour mix, size and build time, never a strictly better button.
    BIAS = {
        "farm_industrial": (0.02, 1.0), "farm_precision": (0.03, 1.15), "farm_sustainable": (-0.02, 0.85),
        "oil_wells": (0.0, 1.1), "generator_coal": (0.02, 1.1), "generator_gas": (-0.02, 0.8),
        "generator_nuclear": (0.06, 1.6), "generator_renewable": (-0.04, 1.2),
        "vehicle_assembly": (0.0, 1.0), "infrastructure": (0.02, 1.2), "modular": (-0.02, 0.9),
        "freight_rail": (0.02, 1.4), "full_service": (0.01, 0.9), "auto_dealer": (0.01, 1.1),
        "ecommerce": (-0.02, 0.8), "hospital": (0.02, 1.3), "long_term_care": (-0.02, 0.9), "hardware_maker": (-0.02, 1.1),
        "it_services": (0.01, 0.8), "digital_media": (-0.03, 0.8), "publishing": (0.0, 1.0),
        "wireless": (0.02, 1.2), "broadband": (0.01, 1.1), "residential": (-0.02, 1.0),
        "industrial_parks": (0.02, 1.1), "heavy_armor": (0.0, 1.1), "aerospace": (0.04, 1.3),
        "munitions": (-0.02, 0.9),
    }
    pricing = {rid for _, _, _, rid, _ in NEW_GOODS}

    def sector_target(industry):
        standard = [recipes[r] for r in STANDARD if recipes[r]["industry"] == industry]
        best = None
        for recipe in standard:
            revenue = recipe["weekly_output_lots"] * products[recipe["output"]]["reference_price_cents"]
            ebitda = revenue - cost(recipe)
            margin = ebitda / revenue
            payback = recipe["capex_cents"] / max(1, ebitda) / 52
            if best is None or margin > best[0]:
                best = (margin, payback)
        if best is None:
            return 0.22, 3.0
        return min(max(best[0], 0.10), 0.60), min(max(best[1], 1.2), 5.0)

    def solve(rid):
        recipe = recipes[rid]
        margin, payback = sector_target(recipe["industry"])
        bias_margin, bias_payback = BIAS.get(rid, (0.0, 1.0))
        margin, payback = margin + bias_margin, payback * bias_payback
        if rid not in pricing:
            price = products[recipe["output"]]["reference_price_cents"]
            recipe["weekly_output_lots"] = max(1, round(cost(recipe) / (1 - margin) / price))
        revenue = recipe["weekly_output_lots"] * products[recipe["output"]]["reference_price_cents"]
        ebitda = revenue - cost(recipe)
        recipe["capex_cents"] = int(round(max(ebitda, 1) * 52 * payback / 1_000_000) * 1_000_000)

    # Price new goods in dependency order (a good's producer may consume an earlier new good).
    pending = list(NEW_GOODS)
    while pending:
        progressed = False
        for entry in list(pending):
            pid, name, kind, rid, margin = entry
            recipe = recipes[rid]
            if any(k not in products for k in recipe["weekly_inputs_lots"]):
                continue
            unit = cost(recipe) / (1 - margin) / recipe["weekly_output_lots"]
            step = 100 if unit < 10_000 else 1_000
            price = int(max(step, round(unit / step) * step))
            products[pid] = {"id": pid, "name": name, "reference_price_cents": price, "kind": kind}
            pending.remove(entry)
            progressed = True
        if not progressed:
            raise SystemExit(f"cannot price {pending}")

    for row in NEW_RECIPES:
        solve(row[1])

    order = [p["id"] for p in data["products"]]
    goods_tail = [pid for pid, *_ in NEW_GOODS]
    financial = [pid for pid in order if products[pid]["kind"] == "financial_contract"]
    # Rerunning on its own output must not duplicate the new goods.
    ordered = [pid for pid in order if pid not in financial and pid not in goods_tail] + goods_tail + financial
    data["products"] = [products[pid] for pid in ordered]
    industries = ["agriculture", "extraction", "energy", "chemical_industries", "manufacturing",
                  "construction", "logistics", "retail", "healthcare", "technology", "media",
                  "telecommunications", "real_estate", "defense"]
    data["recipes"] = sorted(recipes.values(), key=lambda r: (industries.index(r["industry"]), r["id"] not in STANDARD, r["id"]))
    PATH.write_text(json.dumps(data, indent=2) + "\n")

    print(f"{len(data['products'])} products, {len(data['recipes'])} strategies")
    for pid, *_ in NEW_GOODS:
        print(f"  price {pid:20} ${products[pid]['reference_price_cents'] / 100:,.0f}")
    for recipe in data["recipes"]:
        revenue = recipe["weekly_output_lots"] * products[recipe["output"]]["reference_price_cents"]
        ebitda = revenue - cost(recipe)
        dep = recipe["capex_cents"] // (recipe["life_years"] * 52)
        payback = recipe["capex_cents"] / max(1, ebitda) / 52
        print(f"  {recipe['industry']:20} {recipe['name']:30} rev ${revenue / 100:>9,.0f} ebitda {100 * ebitda / revenue:5.1f}%  "
              f"ebit ${(ebitda - dep) / 100:>8,.0f}  payback {payback:5.1f}y")


if __name__ == "__main__":
    main()
