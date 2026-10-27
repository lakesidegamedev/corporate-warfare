#!/usr/bin/env python3
"""Owned logistics (1.7 preview 3): tag the carrier recipes with a freight mode and add pipeline,
shipping, rail terminal and port recipes.

A carrier is an ordinary recipe in the logistics industry that makes `freight`; its
`weekly_output_lots` per block is the lots it can move each week on the lanes it serves. A
terminal or port is a carrier with `terminal: true`: it also opens its region to rail or sea for
every firm. Constants are authored game calibration. Re-running is a no-op.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "content/initial-balance.json"

TAGS = {"carrier": "truck", "freight_rail": "rail", "full_service": "truck"}

# A lot of capacity is one lot of goods moved over a lane in a week. At a few dollars a lot the
# old figures (87 lots a block at $120) would never pay a crew, so capacity is rescaled to the
# lots a block can move and freight is priced per lot moved. Authored game calibration.
FREIGHT_PRICE_CENTS = 800
LOTS = {"carrier": 1500, "freight_rail": 3200, "full_service": 1800}
ECOMMERCE_FREIGHT_LOTS = 180

# id, name, mode, terminal, lots per block, capex cents, build weeks, life years, crew, inputs
RECIPES = [
    ("pipeline", "Pipeline", "pipeline", False, 3600, 90_000_000, 26, 40, [2, 1, 1], {"electricity": 2}),
    ("shipping", "Lake & Sea Shipping", "sea", False, 4800, 80_000_000, 26, 30, [6, 3, 1], {"fuel": 10}),
    ("rail_terminal", "Rail Terminal", "rail", True, 1100, 30_000_000, 13, 30, [2, 1, 0], {"electricity": 1}),
    ("port", "Port", "sea", True, 3000, 50_000_000, 26, 40, [3, 2, 1], {"electricity": 1, "fuel": 2}),
]


def main():
    data = json.loads(PATH.read_text())
    for recipe in data["recipes"]:
        if recipe["id"] in TAGS:
            recipe["transport_mode"] = TAGS[recipe["id"]]
            recipe["weekly_output_lots"] = LOTS[recipe["id"]]
        if recipe["id"] == "ecommerce":
            recipe["weekly_inputs_lots"]["freight"] = ECOMMERCE_FREIGHT_LOTS
    for product in data["products"]:
        if product["id"] == "freight":
            product["name"] = "Freight capacity"
            product["reference_price_cents"] = FREIGHT_PRICE_CENTS
    have = {r["id"] for r in data["recipes"]}
    for rid, name, mode, terminal, lots, capex, weeks, life, crew, inputs in RECIPES:
        if rid in have:
            continue
        recipe = {
            "industry": "logistics",
            "id": rid,
            "name": name,
            "output": "freight",
            "weekly_output_lots": lots,
            "transport_mode": mode,
        }
        if terminal:
            recipe["terminal"] = True
        recipe.update({
            "weekly_inputs_lots": inputs,
            "workers_low_mid_high": crew,
            "capex_cents": capex,
            "build_weeks": weeks,
            "life_years": life,
        })
        data["recipes"].append(recipe)
    PATH.write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    main()
