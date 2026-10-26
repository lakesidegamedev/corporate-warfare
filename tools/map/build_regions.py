#!/usr/bin/env python3
"""Write content/geography/regions.json: economic regions inside the eight playable states.

A region is the economic unit: its own labour pool, wages, household demand, land cost,
markets and freight lanes. The state is the political layer: conditions, union law and tax
policy are the state's and every region in it shares them.

Regions are built from whole counties. Each state has two to five anchor metros; every
county joins the nearest anchor in its own state by great-circle distance between the
county's interior point and the anchor. County populations are the Census 2000 estimates
base and incomes are SAIPE 2000 median household income, both from
tools/map/data/counties-2000.csv. State totals keep the calibration of the 1.1.0 states:
a region's demand is its income-weighted share of the state's demand, its wage moves with
its income relative to the state, and its land cost with its density. Freight lanes are
great-circle distances between anchors times a 1.25 road factor.

Also writes tools/map/data/county-regions.csv, the county to region assignment the map
build dissolves into region shapes.
"""
import csv
import json
import math
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
COUNTIES = ROOT / "tools/map/data/counties-2000.csv"
ASSIGNMENT = ROOT / "tools/map/data/county-regions.csv"
OUT = ROOT / "content/geography/regions.json"

FIPS = {"17": "IL", "18": "IN", "21": "KY", "26": "MI", "36": "NY", "39": "OH", "42": "PA", "54": "WV"}

# code, name, hub city, lat, lon, Census 2000 population, GDP $bn (rounded),
# average annual pay $ (rounded), union membership % (rounded)
STATES = [
    ("IL", "Illinois", "Chicago", 41.878, -87.630, 12_419_293, 470, 39_000, 19.0),
    ("IN", "Indiana", "Indianapolis", 39.768, -86.158, 6_080_485, 190, 30_000, 14.0),
    ("KY", "Kentucky", "Louisville", 38.253, -85.759, 4_041_769, 115, 28_000, 10.5),
    ("MI", "Michigan", "Detroit", 42.331, -83.046, 9_938_444, 335, 38_000, 21.0),
    ("NY", "New York", "New York City", 40.713, -74.006, 18_976_457, 770, 46_500, 25.5),
    ("OH", "Ohio", "Columbus", 39.961, -82.999, 11_353_140, 380, 32_500, 17.5),
    ("PA", "Pennsylvania", "Harrisburg", 40.274, -76.884, 12_281_054, 390, 34_000, 15.5),
    ("WV", "West Virginia", "Charleston", 38.350, -81.633, 1_808_344, 42, 26_000, 14.5),
]

# Authored state condition indices, 0..10000 (10000 best, except crime and cost of living
# where 10000 is the corridor average): power grid reliability, road condition, broadband
# reach (2000 era), workforce skill, crime, cost of living, land cost.
CONDITIONS = {
    "IL": (9600, 6200, 2600, 6200, 11500, 11000, 12000),
    "IN": (9700, 6800, 1800, 5000, 8500, 8800, 8500),
    "KY": (9500, 6000, 1400, 4400, 8000, 8500, 8000),
    "MI": (9500, 5200, 2200, 5600, 11000, 9800, 10000),
    "NY": (9400, 5000, 3200, 6800, 10500, 15000, 16000),
    "OH": (9700, 6400, 2000, 5400, 9500, 9300, 9500),
    "PA": (9600, 5600, 2200, 5700, 9000, 10000, 10000),
    "WV": (9300, 4800, 1000, 4000, 7500, 8000, 7000),
}

# Region code, name, anchor city, anchor lat, anchor lon. Codes are STATE-AREA.
REGIONS = [
    ("IL-CHI", "Chicagoland", "Chicago", 41.878, -87.630),
    ("IL-CEN", "Central Illinois", "Peoria", 40.694, -89.589),
    ("IL-WCN", "West Central Illinois", "Springfield", 39.781, -89.650),
    ("IL-SOU", "Southern Illinois", "Carbondale", 37.727, -89.217),
    ("IN-CEN", "Central Indiana", "Indianapolis", 39.768, -86.158),
    ("IN-NE", "Northeast Indiana", "Fort Wayne", 41.079, -85.139),
    ("IN-NW", "Northwest Indiana", "Gary", 41.593, -87.346),
    ("IN-SOU", "Southern Indiana", "Evansville", 37.975, -87.571),
    ("KY-LOU", "Louisville and North Central Kentucky", "Louisville", 38.253, -85.759),
    ("KY-BLU", "Bluegrass", "Lexington", 38.040, -84.503),
    ("KY-EAS", "Eastern Kentucky", "Pikeville", 37.479, -82.519),
    ("KY-WES", "Western Kentucky", "Bowling Green", 36.990, -86.444),
    ("MI-DET", "Metro Detroit", "Detroit", 42.331, -83.046),
    ("MI-WES", "West Michigan", "Grand Rapids", 42.963, -85.668),
    ("MI-MID", "Mid-Michigan", "Lansing", 42.733, -84.556),
    ("MI-NOR", "Northern Michigan and the UP", "Traverse City", 44.763, -85.621),
    ("NY-NYC", "New York Metro", "New York City", 40.713, -74.006),
    ("NY-CAP", "Capital and Hudson Valley", "Albany", 42.653, -73.756),
    ("NY-CEN", "Central New York", "Syracuse", 43.049, -76.147),
    ("NY-WES", "Western New York", "Buffalo", 42.886, -78.878),
    ("OH-NE", "Northeast Ohio", "Cleveland", 41.499, -81.694),
    ("OH-CEN", "Central Ohio", "Columbus", 39.961, -82.999),
    ("OH-SW", "Southwest Ohio", "Cincinnati", 39.103, -84.512),
    ("OH-NW", "Northwest Ohio", "Toledo", 41.654, -83.537),
    ("OH-SE", "Southeast Ohio", "Athens", 39.329, -82.101),
    ("PA-PHL", "Greater Philadelphia", "Philadelphia", 39.953, -75.164),
    ("PA-WES", "Western Pennsylvania", "Pittsburgh", 40.441, -79.996),
    ("PA-CEN", "Central Pennsylvania", "Harrisburg", 40.274, -76.884),
    ("PA-NE", "Northeastern Pennsylvania", "Scranton", 41.409, -75.662),
    ("WV-SOU", "Southern West Virginia", "Charleston", 38.350, -81.633),
    ("WV-NOR", "Northern West Virginia", "Morgantown", 39.630, -79.956),
]

ROAD_FACTOR = 1.25


def great_circle_km(a, b):
    lat1, lon1, lat2, lon2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    h = math.sin((lat2 - lat1) / 2) ** 2 + math.cos(lat1) * math.cos(lat2) * math.sin((lon2 - lon1) / 2) ** 2
    return 6371.0 * 2 * math.asin(math.sqrt(h))


def clamp(value, low, high):
    return max(low, min(high, value))


def main():
    counties = list(csv.DictReader(COUNTIES.open()))
    anchors = defaultdict(list)
    for region in REGIONS:
        anchors[region[0][:2]].append(region)

    members = defaultdict(list)
    with ASSIGNMENT.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["geoid", "region"])
        for county in counties:
            state = FIPS[county["statefp"]]
            point = (float(county["lat"]), float(county["lon"]))
            region = min(anchors[state], key=lambda r: (great_circle_km(point, (r[3], r[4])), r[0]))
            members[region[0]].append(county)
            writer.writerow([county["geoid"], region[0]])

    def totals(rows):
        population = sum(int(c["population_2000"]) for c in rows)
        income = sum(int(c["population_2000"]) * int(c["median_household_income_2000"]) for c in rows)
        land = sum(int(c["land_km2"]) for c in rows)
        return population, income, land

    base_gdp = 380  # Ohio = 10000
    base_pay = 34_000
    state_rows = {code: [c for c in counties if FIPS[c["statefp"]] == code] for code, *_ in STATES}
    regions = []
    for code, name, city, lat, lon, population, gdp, pay, union in STATES:
        grid, road, broadband, skill, crime, living, land_index = CONDITIONS[code]
        state_pop, state_income, state_land = totals(state_rows[code])
        state_mhi = state_income / state_pop
        state_density = state_pop / state_land
        state_wage = pay * 10_000 / base_pay
        state_demand = gdp * 10_000 / base_gdp
        for rcode, rname, rcity, rlat, rlon in anchors[code]:
            rows = members[rcode]
            if not rows:
                raise SystemExit(f"{rcode} has no counties")
            rpop, rincome, rland = totals(rows)
            mhi_ratio = (rincome / rpop) / state_mhi
            density_ratio = (rpop / rland) / state_density
            regions.append({
                "code": rcode,
                "name": rname,
                "states": [code],
                "principal_city": f"{rcity}, {code}",
                "lat_milli": round(rlat * 1000),
                "lon_milli": round(rlon * 1000),
                "population_2000": rpop,
                "wage_index_bps": round(state_wage * mhi_ratio ** 0.6),
                "demand_index_bps": max(1, round(state_demand * rincome / state_income)),
                "land_index_bps": round(land_index * clamp(density_ratio ** 0.35, 0.55, 1.8)),
                "opening_organisation_bps": round(union * 100),
                "conditions": {
                    "power_grid_bps": grid,
                    "road_bps": road,
                    # Broadband in 2000 followed density; the rest is state policy and stock.
                    "broadband_bps": round(clamp(broadband * clamp(density_ratio ** 0.25, 0.6, 1.5), 0, 10_000)),
                    "skill_bps": round(clamp(skill * clamp(mhi_ratio ** 0.5, 0.8, 1.2), 0, 10_000)),
                    "crime_bps": crime,
                    "cost_of_living_bps": round(living * clamp(mhi_ratio ** 0.5, 0.85, 1.2)),
                },
                "counties": len(rows),
            })

    states = [
        {"code": code, "name": name, "capital_hub": f"{city}, {code}", "population_2000": population}
        for code, name, city, _lat, _lon, population, *_ in STATES
    ]
    lanes = []
    for i, a in enumerate(REGIONS):
        for b in REGIONS[i + 1:]:
            km = great_circle_km((a[3], a[4]), (b[3], b[4])) * ROAD_FACTOR
            lanes.append({"a": a[0], "b": b[0], "km": max(1, round(km))})
    document = {
        "schema": 3,
        "classification": "economic regions built from whole counties inside the eight playable states; Census 2000 county populations and SAIPE 2000 incomes, rounded public circa-2000 state GDP, pay and union figures as relative indices, authored condition indices",
        "vintage": "nominal January 2000 start",
        "official_source": "US Census Bureau 2000 county population estimates base and SAIPE 2000 median household income; BEA, BLS and union membership figures rounded for indices",
        "attempts": [],
        "membership_status": "each region is a set of whole counties joined to the nearest anchor metro in the same state; tools/map/data/county-regions.csv lists the assignment",
        "notes": "Generated by tools/map/build_regions.py. Condition indices and calibrated demand are game estimates, not observations.",
        "states": states,
        "regions": regions,
        "lanes": lanes,
    }
    OUT.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {OUT.relative_to(ROOT)}: {len(regions)} regions, {len(lanes)} lanes")
    for r in regions:
        print(f"  {r['code']:7} {r['name']:38} counties {r['counties']:3} pop {r['population_2000']:>10,} "
              f"wage {r['wage_index_bps']:>6} demand {r['demand_index_bps']:>6} land {r['land_index_bps']:>6} "
              f"bb {r['conditions']['broadband_bps']:>5} skill {r['conditions']['skill_bps']:>5}")


if __name__ == "__main__":
    main()
