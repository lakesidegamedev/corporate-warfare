#!/usr/bin/env python3
"""Market health (1.7.1): every listed good has a producer in every scenario that uses it.

Maintenance was an input of seven plants that no recipe made, so every maintenance bill went to
the outside sector and the product sat at its placeholder price. This adds the producer recipe,
`plant_services` (a construction-industry strategy that sells maintenance), and opens each
scenario with the plants its roster lacked for goods that other plants consume: research,
maintenance, fertilizers, vehicles, ore, rare earths, timber, pharmaceuticals, building
materials, assembled goods and Ruhr crude. A firm runs one strategy per sector in a region, and
its plants share its money, so each added plant sits in a region of the firm's own currency.
Re-running is a no-op.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from add_chains import solve_crew  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
BALANCE = ROOT / "content/initial-balance.json"

RECIPE = {
    "industry": "construction",
    "id": "plant_services",
    "name": "Plant Maintenance Services",
    "output": "maintenance",
    "weekly_output_lots": 14,
    "weekly_inputs_lots": {"components": 2, "electricity": 1},
    "workers_low_mid_high": [6, 6, 2],
    "capex_cents": 28_000_000,
    "build_weeks": 8,
    "life_years": 15,
}

# firm id -> [(recipe, region, blocks)] per roster file.
PLANTS = {
    "content/firms.json": {
        "cedar_point_software": [("it_services", "OH-CEN", 2)],
        "scioto_chemical": [("fertilizer_plant", "IL-CEN", 3)],
        "miami_components": [("assembly", "IN-NE", 3)],
        "stonebridge": [("plant_services", "IL-CHI", 2)],
    },
    "content/scenarios/texas-1981/firms.json": {
        "anadarko_gas": [("mine", "TX-ABI", 3), ("rare_earth_mine", "TX-LUB", 1)],
        "longhorn_tubular": [("building_products", "TX-AUS", 3)],
        "sabine_petrochem": [("pharma_plant", "TX-SAT", 2)],
        "bluebonnet_assembly": [("vehicle_assembly", "TX-SAT", 3)],
        "cactus_software": [("it_services", "TX-DFW", 2)],
        "heartland_modular": [("plant_services", "OK-TUL", 2)],
    },
    "content/scenarios/silicon-valley-1995/firms.json": {
        "trinity_timber": [("rare_earth_mine", "CA-INL", 2)],
        "fontana_steel": [("plant_services", "CA-SAC", 2)],
    },
    "content/scenarios/ruhr-rhine-1991/firms.json": {
        "rheinische_braunkohle": [("oil_wells", "DE-HAN", 3), ("rare_earth_mine", "DE-OST", 2)],
        "emscher_kohle": [("mine", "DE-RLP", 3)],
        "altmark_agrar": [("timber_forestry", "DE-RLP", 3)],
        "leuna_buna_chemie": [("fertilizer_plant", "DE-RHW", 3)],
        "ruhrstahl": [("plant_services", "DE-RUH", 2)],
        "isar_software": [("it_services", "DE-STU", 2)],
    },
}


# Coastal Generation sold almost nothing from Orange County, where it had no industrial demand,
# and was dissolved every game once its plant staffed up; Los Angeles has the demand.
MOVES = {"content/scenarios/silicon-valley-1995/firms.json": {"coastal_generation": "CA-LAX"}}


def main():
    data = json.loads(BALANCE.read_text())
    if not any(r["id"] == RECIPE["id"] for r in data["recipes"]):
        prices = {p["id"]: p["reference_price_cents"] for p in data["products"]}
        recipe = json.loads(json.dumps(RECIPE))
        solve_crew(recipe, prices, data["baseline_weekly_wage_cents"],
                   data["maintenance_annual_bps"], data["fixture_land_rent_cents_weekly"])
        data["recipes"].append(recipe)
        BALANCE.write_text(json.dumps(data, indent=2) + "\n")
    for path, firms in PLANTS.items():
        file = ROOT / path
        roster = json.loads(file.read_text())
        known = {f["id"]: f for f in roster["firms"]}
        for firm_id, region in MOVES.get(path, {}).items():
            known[firm_id]["region"] = region
        for firm_id, plants in firms.items():
            extra = known[firm_id].setdefault("extra_plants", [])
            for recipe, region, blocks in plants:
                plant = {"recipe": recipe, "region": region, "blocks": blocks}
                if plant not in extra:
                    extra.append(plant)
        file.write_text(json.dumps(roster, indent=2) + "\n")


if __name__ == "__main__":
    main()
