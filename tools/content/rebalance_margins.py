#!/usr/bin/env python3
"""Raise thin strategies to a real margin by trimming their crews.

Owner direction (2026-10-26): a typical plant should earn about 20 percent of revenue at
reference prices after inputs, wages, maintenance, land and depreciation. Strategies below 20
percent move to a target of 20 percent plus half their distance from their sector's mean
margin, held between 17 and 24 percent, so a sector's strategies keep their order. The lever is
crew size: output and inputs, and so the market's volumes, are unchanged, and the workers
released return to regional background employment. Strategies at or above 20 percent are left
alone. The content file records that the floor was applied, so a rerun is a no-op.
"""
import json
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = ROOT / "content/initial-balance.json"
FLOOR = 0.20
LOW, HIGH = 0.17, 0.24


def main():
    data = json.loads(PATH.read_text())
    if data.get("margin_floor") == FLOOR:
        print("margin floor already applied")
        return
    prices = {p["id"]: p["reference_price_cents"] for p in data["products"]}
    wages = data["baseline_weekly_wage_cents"]
    maint_bps = data["maintenance_annual_bps"]
    land = data["fixture_land_rent_cents_weekly"]

    def parts(recipe):
        revenue = recipe["weekly_output_lots"] * prices[recipe["output"]]
        inputs = sum(q * prices.get(k, 0) for k, q in recipe["weekly_inputs_lots"].items())
        payroll = sum(a * b for a, b in zip(recipe["workers_low_mid_high"], wages))
        upkeep = recipe["capex_cents"] * maint_bps // 10_000 // 52 + land
        depreciation = recipe["capex_cents"] // (recipe["life_years"] * 52)
        return revenue, inputs, payroll, upkeep, depreciation

    def margin(recipe):
        revenue, inputs, payroll, upkeep, depreciation = parts(recipe)
        return (revenue - inputs - payroll - upkeep - depreciation) / revenue

    sector_mean = {}
    for recipe in data["recipes"]:
        sector_mean.setdefault(recipe["industry"], []).append(margin(recipe))
    sector_mean = {k: statistics.mean(v) for k, v in sector_mean.items()}

    for recipe in data["recipes"]:
        before = margin(recipe)
        if before >= FLOOR:
            continue
        target = min(max(FLOOR + (before - sector_mean[recipe["industry"]]) * 0.5, LOW), HIGH)
        revenue, inputs, payroll, upkeep, depreciation = parts(recipe)
        factor = (revenue * (1 - target) - inputs - upkeep - depreciation) / payroll
        if factor >= 1:
            continue
        crew = recipe["workers_low_mid_high"]
        # Small tiers round to whole workers; the largest tier then absorbs the remainder exactly,
        # so a plant with one or two specialists still lands on its target.
        largest = max(range(3), key=lambda tier: crew[tier] * wages[tier])
        budget = revenue * (1 - target) - inputs - upkeep - depreciation
        trimmed = [0 if w == 0 else max(1, round(w * factor)) for w in crew]
        others = sum(trimmed[t] * wages[t] for t in range(3) if t != largest)
        trimmed[largest] = max(1, round((budget - others) / wages[largest]))
        recipe["workers_low_mid_high"] = trimmed
        print(f"{recipe['id']:22} {before:6.1%} -> {margin(recipe):6.1%}  crew x{factor:.2f}")
    data["margin_floor"] = FLOOR
    PATH.write_text(json.dumps(data, indent=2) + "\n")


if __name__ == "__main__":
    main()
