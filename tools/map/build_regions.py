#!/usr/bin/env python3
"""Write the economic regions of each scenario that has its own geography.

Great Lakes 2000 writes content/geography/regions.json (the original dataset). Texas Oil Patch
1981 writes content/scenarios/texas-1981/regions.json from 1980 census county populations; see
the TEXAS section below. Silicon Valley 1995 writes content/scenarios/silicon-valley-1995/
regions.json from July 1995 county estimates; see the CALIFORNIA section. Ruhr and Rhine 1991
writes content/scenarios/ruhr-rhine-1991/regions.json from Eurostat NUTS 3 areas; see the RUHR AND
RHINE section. Run with no arguments to rebuild all, or name a scenario id.

Great Lakes 2000: economic regions inside the eight playable states.

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
import sys
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

# Authored circa-2000 state specializations (primary, secondary), A House Divided style: plants of
# a specialized sector in the state use fewer inputs.
SPECIALIZATION = {
    "IL": ("agriculture", "logistics"),
    "IN": ("manufacturing", "agriculture"),
    "KY": ("manufacturing", "extraction"),
    "MI": ("manufacturing", "chemical_industries"),
    "NY": ("financial", "media"),
    "OH": ("manufacturing", "healthcare"),
    "PA": ("energy", "healthcare"),
    "WV": ("extraction", "chemical_industries"),
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


def build_great_lakes():
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
        {"code": code, "name": name, "capital_hub": f"{city}, {code}", "population_2000": population,
         "specialization": list(SPECIALIZATION[code])}
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


# ---------------------------------------------------------------------------------------------
# TEXAS: Texas Oil Patch, 1981 (Texas, Oklahoma and Louisiana)
#
# The same method as the Great Lakes build, on 1980 data. County populations are the April 1980
# census counts. County incomes are NOT 1980 observations: the Census has no county income series
# for 1980 in a form we can pin, so SAIPE 2000 median household income is used only as a relative
# index of which counties were richer than their state, then adjusted by hand for the oil boom
# (BOOM below). State product, pay and union figures are rounded public circa-1981 values used as
# relative indices. Condition indices and specialisations are authored game estimates.
# Regions are whole counties joined to the nearest anchor in the same state.
# ---------------------------------------------------------------------------------------------

TX_COUNTIES = ROOT / "tools/map/data/counties-1980-texas-1981.csv"
TX_ASSIGNMENT = ROOT / "tools/map/data/county-regions-texas-1981.csv"
TX_OUT = ROOT / "content/scenarios/texas-1981/regions.json"
TX_FIPS = {"48": "TX", "40": "OK", "22": "LA"}

# code, name, hub city, GDP $bn (rounded, 1981), average annual pay $ (rounded, 1981),
# union membership % (rounded)
TX_STATES = [
    ("TX", "Texas", "Houston", 190, 16_000, 10.0),
    ("OK", "Oklahoma", "Oklahoma City", 50, 15_100, 11.0),
    ("LA", "Louisiana", "New Orleans", 65, 15_600, 13.0),
]
TX_HUBS = {"TX": (29.760, -95.370), "OK": (35.468, -97.516), "LA": (29.951, -90.072)}

# power grid, road condition, broadband reach (a 1981 value: almost none), workforce skill,
# crime, cost of living, land cost
TX_CONDITIONS = {
    "TX": (9300, 6400, 600, 4800, 10500, 9000, 8000),
    "OK": (9400, 6000, 400, 4600, 8500, 8200, 6500),
    "LA": (9000, 5000, 400, 4000, 11500, 8800, 7000),
}

TX_SPECIALIZATION = {
    "TX": ("extraction", "chemical_industries"),
    "OK": ("extraction", "agriculture"),
    "LA": ("chemical_industries", "logistics"),
}

# Region code, name, anchor city, anchor lat, anchor lon.
TX_REGIONS = [
    ("TX-HOU", "Houston and the Gulf Coast", "Houston", 29.760, -95.370),
    ("TX-BPA", "Beaumont and Port Arthur", "Beaumont", 30.080, -94.126),
    ("TX-DFW", "Dallas and Fort Worth", "Dallas", 32.777, -96.797),
    ("TX-SAT", "San Antonio", "San Antonio", 29.424, -98.494),
    ("TX-AUS", "Austin and Central Texas", "Austin", 30.267, -97.743),
    ("TX-PMB", "Permian Basin", "Midland", 31.997, -102.078),
    ("TX-CRP", "Corpus Christi and the South Texas Coast", "Corpus Christi", 27.801, -97.396),
    ("TX-AMA", "Texas Panhandle", "Amarillo", 35.222, -101.831),
    ("TX-ELP", "El Paso and the Trans-Pecos", "El Paso", 31.761, -106.485),
    ("TX-TYL", "East Texas", "Tyler", 32.351, -95.301),
    ("TX-RGV", "Rio Grande Valley", "McAllen", 26.203, -98.230),
    ("TX-LUB", "Lubbock and the South Plains", "Lubbock", 33.578, -101.855),
    ("TX-ABI", "Abilene and West Central Texas", "Abilene", 32.449, -99.733),
    ("OK-OKC", "Oklahoma City", "Oklahoma City", 35.468, -97.516),
    ("OK-TUL", "Tulsa and Eastern Oklahoma", "Tulsa", 36.154, -95.993),
    ("OK-CSH", "Cushing and North Central Oklahoma", "Cushing", 35.985, -96.767),
    ("OK-WES", "Western Oklahoma and the Anadarko Basin", "Woodward", 36.430, -99.390),
    ("LA-NOL", "New Orleans", "New Orleans", 29.951, -90.072),
    ("LA-BTR", "Baton Rouge", "Baton Rouge", 30.451, -91.187),
    ("LA-LCH", "Lake Charles", "Lake Charles", 30.226, -93.217),
    ("LA-SHV", "Shreveport and North Louisiana", "Shreveport", 32.525, -93.750),
    ("LA-LAF", "Lafayette and Acadiana", "Lafayette", 30.224, -92.020),
]

# 1981 oil boom adjustment to a region's wage and demand against what its 2000 income says, in
# percent. Authored game estimates: the Permian, Houston and Acadiana paid oilfield wages.
TX_BOOM = {
    "TX-PMB": 22, "TX-HOU": 12, "TX-BPA": 6, "TX-CRP": 4, "TX-AMA": 4, "TX-ABI": 3,
    "OK-OKC": 6, "OK-TUL": 6, "OK-CSH": 4, "OK-WES": 8,
    "LA-LAF": 12, "LA-LCH": 5, "LA-NOL": 3,
}


def build_texas():
    counties = list(csv.DictReader(TX_COUNTIES.open()))
    anchors = defaultdict(list)
    for region in TX_REGIONS:
        anchors[region[0][:2]].append(region)
    members = defaultdict(list)
    TX_ASSIGNMENT.parent.mkdir(parents=True, exist_ok=True)
    with TX_ASSIGNMENT.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["geoid", "region"])
        for county in counties:
            state = TX_FIPS[county["statefp"]]
            point = (float(county["lat"]), float(county["lon"]))
            region = min(anchors[state], key=lambda r: (great_circle_km(point, (r[3], r[4])), r[0]))
            members[region[0]].append(county)
            writer.writerow([county["geoid"], region[0]])

    def totals(rows):
        population = sum(int(c["population_1980"]) for c in rows)
        income = sum(int(c["population_1980"]) * int(c["median_household_income_2000"]) for c in rows)
        land = sum(int(c["land_km2"]) for c in rows)
        return population, income, land

    base_gdp = 190  # Texas = 10000
    base_pay = 15_500
    state_rows = {code: [c for c in counties if TX_FIPS[c["statefp"]] == code] for code, *_ in TX_STATES}
    regions = []
    states = []
    for code, name, city, gdp, pay, union in TX_STATES:
        grid, road, broadband, skill, crime, living, land_index = TX_CONDITIONS[code]
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
            boom = 1 + TX_BOOM.get(rcode, 0) / 100
            regions.append({
                "code": rcode,
                "name": rname,
                "states": [code],
                "principal_city": f"{rcity}, {code}",
                "lat_milli": round(rlat * 1000),
                "lon_milli": round(rlon * 1000),
                "population_1980": rpop,
                "wage_index_bps": round(state_wage * mhi_ratio ** 0.6 * boom),
                "demand_index_bps": max(1, round(state_demand * rincome / state_income * boom)),
                "land_index_bps": round(land_index * clamp(density_ratio ** 0.35, 0.55, 1.8)),
                "opening_organisation_bps": round(union * 100),
                "conditions": {
                    "power_grid_bps": grid,
                    "road_bps": road,
                    "broadband_bps": round(clamp(broadband * clamp(density_ratio ** 0.25, 0.6, 1.5), 0, 10_000)),
                    "skill_bps": round(clamp(skill * clamp(mhi_ratio ** 0.5, 0.8, 1.2), 0, 10_000)),
                    "crime_bps": crime,
                    "cost_of_living_bps": round(living * clamp(mhi_ratio ** 0.5, 0.85, 1.2)),
                },
                "counties": len(rows),
            })
        states.append({
            "code": code, "name": name, "capital_hub": f"{city}, {code}",
            "population_1980": state_pop, "specialization": list(TX_SPECIALIZATION[code]),
        })
    lanes = []
    for i, a in enumerate(TX_REGIONS):
        for b in TX_REGIONS[i + 1:]:
            km = great_circle_km((a[3], a[4]), (b[3], b[4])) * ROAD_FACTOR
            lanes.append({"a": a[0], "b": b[0], "km": max(1, round(km))})
    document = {
        "schema": 3,
        "classification": "economic regions built from whole counties in Texas, Oklahoma and Louisiana; 1980 census county populations, SAIPE 2000 county income used only as a relative index, rounded public circa-1981 state product, pay and union figures as relative indices, authored condition indices and oil boom adjustments",
        "vintage": "nominal January 1981 start",
        "official_source": "US Census Bureau 1980 census county populations (intercensal estimates 1980-1989 base) and SAIPE 2000 median household income; BEA, BLS and union membership figures rounded for indices",
        "attempts": [],
        "membership_status": "each region is a set of whole counties joined to the nearest anchor metro in the same state; tools/map/data/county-regions-texas-1981.csv lists the assignment",
        "notes": "Generated by tools/map/build_regions.py. Condition indices, oil boom adjustments and calibrated demand are game estimates, not observations. Freight lanes are anchor-to-anchor great-circle distances times a 1.25 road factor.",
        "states": states,
        "regions": regions,
        "lanes": lanes,
    }
    TX_OUT.parent.mkdir(parents=True, exist_ok=True)
    TX_OUT.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {TX_OUT.relative_to(ROOT)}: {len(regions)} regions, {len(lanes)} lanes")
    for r in regions:
        print(f"  {r['code']:7} {r['name']:42} counties {r['counties']:3} pop {r['population_1980']:>10,} "
              f"wage {r['wage_index_bps']:>6} demand {r['demand_index_bps']:>6} land {r['land_index_bps']:>6}")


# ---------------------------------------------------------------------------------------------
# CALIFORNIA: Silicon Valley, 1995
#
# The same method on July 1995 county population estimates (Census CO-99-8, the revised 1990s
# series). As in the Texas build, SAIPE 2000 county income is used only as a relative index of
# which counties were richer than the state, then adjusted by hand (CA_ADJUST) because the 2000
# figures carry the dot-com peak and the 1995 Bay Area was less far ahead. State product, pay and
# union figures are rounded public circa-1995 values used as relative indices. Condition indices
# and specialisations are authored game estimates. California is one state, so its regions are
# whole counties joined to the nearest anchor, except for the few counties whose Census interior
# point sits far from where its people live (CA_OVERRIDE).
# ---------------------------------------------------------------------------------------------

CA_COUNTIES = ROOT / "tools/map/data/counties-1995-silicon-valley-1995.csv"
CA_ASSIGNMENT = ROOT / "tools/map/data/county-regions-silicon-valley-1995.csv"
CA_OUT = ROOT / "content/scenarios/silicon-valley-1995/regions.json"
CA_FIPS = {"06": "CA"}

# code, name, hub city, GDP $bn (rounded, 1995), average annual pay $ (rounded, 1995),
# union membership % (rounded)
CA_STATES = [("CA", "California", "Sacramento", 930, 31_600, 18.0)]

# power grid (1990s Californian reliability was good), road condition, broadband reach (a 1995
# value: dial-up and early ISDN), workforce skill, crime, cost of living, land cost
CA_CONDITIONS = {"CA": (9100, 5200, 900, 6200, 11500, 12500, 10000)}

CA_SPECIALIZATION = {"CA": ("technology", "agriculture")}

CA_REGIONS = [
    ("CA-SJC", "San Jose and Silicon Valley", "San Jose", 37.339, -121.894),
    ("CA-SFO", "San Francisco", "San Francisco", 37.775, -122.419),
    ("CA-OAK", "Oakland and the East Bay", "Oakland", 37.805, -122.272),
    ("CA-SMT", "San Mateo and the Peninsula", "San Mateo", 37.563, -122.325),
    ("CA-MAR", "Marin and the North Bay", "San Rafael", 37.974, -122.531),
    ("CA-SAC", "Sacramento", "Sacramento", 38.582, -121.494),
    ("CA-STM", "Stockton and Modesto", "Modesto", 37.639, -120.997),
    ("CA-FRS", "Fresno and the Central San Joaquin", "Fresno", 36.737, -119.787),
    ("CA-BAK", "Bakersfield and the Southern San Joaquin", "Bakersfield", 35.373, -119.019),
    ("CA-LAX", "Los Angeles", "Los Angeles", 34.052, -118.244),
    ("CA-ORA", "Orange County", "Santa Ana", 33.746, -117.868),
    ("CA-INL", "Inland Empire", "Riverside", 33.953, -117.396),
    ("CA-SDG", "San Diego", "San Diego", 32.716, -117.161),
    ("CA-CCS", "Monterey and the Central Coast", "Salinas", 36.678, -121.655),
    ("CA-SBA", "Santa Barbara, Ventura and San Luis Obispo", "Santa Barbara", 34.421, -119.698),
    ("CA-RDD", "Redding and the North State", "Redding", 40.587, -122.392),
]

# County GEOID to region where the Census interior point would put it with the wrong anchor.
CA_OVERRIDE = {
    "06075": "CA-SFO",  # San Francisco: the interior point sits on the Farallon Islands
    "06081": "CA-SMT",  # San Mateo
    "06085": "CA-SJC",  # Santa Clara: the interior point is in the hills south of San Jose
    "06001": "CA-OAK",  # Alameda: the interior point is at Livermore, a commute from both
    "06013": "CA-OAK",  # Contra Costa
    "06041": "CA-MAR",  # Marin
    "06095": "CA-MAR",  # Solano joins the North Bay
    "06087": "CA-SJC",  # Santa Cruz
    "06025": "CA-SDG",  # Imperial
    "06037": "CA-LAX",  # Los Angeles: the interior point is in the San Gabriel Mountains
    "06059": "CA-ORA",  # Orange
    "06073": "CA-SDG",  # San Diego
    "06065": "CA-INL",  # Riverside
    "06071": "CA-INL",  # San Bernardino
    "06111": "CA-SBA",  # Ventura
}

# Adjustment to a region's wage and demand against what its 2000 income says, in percent.
# Authored game estimates: the 2000 figures carry the dot-com peak, and the 1995 Los Angeles
# basin was still shedding Cold War aerospace jobs.
CA_ADJUST = {
    "CA-SJC": -6, "CA-SFO": -6, "CA-SMT": -5, "CA-OAK": -2, "CA-MAR": -3,
    "CA-LAX": -3, "CA-ORA": -2,
}


def build_california():
    counties = list(csv.DictReader(CA_COUNTIES.open()))
    members = defaultdict(list)
    CA_ASSIGNMENT.parent.mkdir(parents=True, exist_ok=True)
    with CA_ASSIGNMENT.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["geoid", "region"])
        for county in counties:
            point = (float(county["lat"]), float(county["lon"]))
            region = CA_OVERRIDE.get(county["geoid"]) or min(
                (r for r in CA_REGIONS), key=lambda r: (great_circle_km(point, (r[3], r[4])), r[0]))[0]
            members[region].append(county)
            writer.writerow([county["geoid"], region])

    def totals(rows):
        population = sum(int(c["population_1995"]) for c in rows)
        income = sum(int(c["population_1995"]) * int(c["median_household_income_2000"]) for c in rows)
        land = sum(int(c["land_km2"]) for c in rows)
        return population, income, land

    base_gdp = 400  # demand units: the same per-person scale as Great Lakes 2000 and Texas
    base_pay = 29_800  # roughly the 1995 national average, so California opens a little above 10000
    regions = []
    states = []
    for code, name, city, gdp, pay, union in CA_STATES:
        grid, road, broadband, skill, crime, living, land_index = CA_CONDITIONS[code]
        state_pop, state_income, state_land = totals(counties)
        state_mhi = state_income / state_pop
        state_density = state_pop / state_land
        state_wage = pay * 10_000 / base_pay
        state_demand = gdp * 10_000 / base_gdp
        for rcode, rname, rcity, rlat, rlon in CA_REGIONS:
            rows = members[rcode]
            if not rows:
                raise SystemExit(f"{rcode} has no counties")
            rpop, rincome, rland = totals(rows)
            mhi_ratio = (rincome / rpop) / state_mhi
            density_ratio = (rpop / rland) / state_density
            adjust = 1 + CA_ADJUST.get(rcode, 0) / 100
            regions.append({
                "code": rcode,
                "name": rname,
                "states": [code],
                "principal_city": f"{rcity}, {code}",
                "lat_milli": round(rlat * 1000),
                "lon_milli": round(rlon * 1000),
                "population_1995": rpop,
                "wage_index_bps": round(state_wage * mhi_ratio ** 0.6 * adjust),
                "demand_index_bps": max(1, round(state_demand * rincome / state_income * adjust)),
                "land_index_bps": round(land_index * clamp(density_ratio ** 0.35, 0.55, 1.8)),
                "opening_organisation_bps": round(union * 100),
                "conditions": {
                    "power_grid_bps": grid,
                    "road_bps": road,
                    "broadband_bps": round(clamp(broadband * clamp(density_ratio ** 0.25, 0.6, 2.5), 0, 10_000)),
                    "skill_bps": round(clamp(skill * clamp(mhi_ratio ** 0.5, 0.8, 1.25), 0, 10_000)),
                    "crime_bps": crime,
                    "cost_of_living_bps": round(living * clamp(mhi_ratio ** 0.5, 0.85, 1.25)),
                },
                "counties": len(rows),
            })
        states.append({
            "code": code, "name": name, "capital_hub": f"{city}, {code}",
            "population_1995": state_pop, "specialization": list(CA_SPECIALIZATION[code]),
        })
    lanes = []
    for i, a in enumerate(CA_REGIONS):
        for b in CA_REGIONS[i + 1:]:
            km = great_circle_km((a[3], a[4]), (b[3], b[4])) * ROAD_FACTOR
            lanes.append({"a": a[0], "b": b[0], "km": max(1, round(km))})
    document = {
        "schema": 3,
        "classification": "economic regions built from whole California counties; July 1995 county population estimates, SAIPE 2000 county income used only as a relative index, rounded public circa-1995 state product, pay and union figures as relative indices, authored condition indices and dot-com adjustments",
        "vintage": "nominal January 1995 start",
        "official_source": "US Census Bureau county population estimates July 1, 1995 (CO-99-8 annual time series, revised March 2000) and SAIPE 2000 median household income; BEA, BLS and union membership figures rounded for indices",
        "attempts": [],
        "membership_status": "each region is a set of whole counties joined to the nearest anchor metro, with a short list of overrides for counties whose Census interior point is far from their people; tools/map/data/county-regions-silicon-valley-1995.csv lists the assignment",
        "notes": "Generated by tools/map/build_regions.py. Condition indices, 1995 adjustments and calibrated demand are game estimates, not observations. Freight lanes are anchor-to-anchor great-circle distances times a 1.25 road factor.",
        "states": states,
        "regions": regions,
        "lanes": lanes,
    }
    CA_OUT.parent.mkdir(parents=True, exist_ok=True)
    CA_OUT.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {CA_OUT.relative_to(ROOT)}: {len(regions)} regions, {len(lanes)} lanes")
    for r in regions:
        print(f"  {r['code']:7} {r['name']:44} counties {r['counties']:3} pop {r['population_1995']:>10,} "
              f"wage {r['wage_index_bps']:>6} demand {r['demand_index_bps']:>6} land {r['land_index_bps']:>6} "
              f"bb {r['conditions']['broadband_bps']:>5} skill {r['conditions']['skill_bps']:>5}")


# ---------------------------------------------------------------------------------------------

TX_COUNTIES = ROOT / "tools/map/data/counties-1980-texas-1981.csv"
TX_ASSIGNMENT = ROOT / "tools/map/data/county-regions-texas-1981.csv"
TX_OUT = ROOT / "content/scenarios/texas-1981/regions.json"
TX_FIPS = {"48": "TX", "40": "OK", "22": "LA"}

# code, name, hub city, GDP $bn (rounded, 1981), average annual pay $ (rounded, 1981),
# union membership % (rounded)
TX_STATES = [
    ("TX", "Texas", "Houston", 190, 16_000, 10.0),
    ("OK", "Oklahoma", "Oklahoma City", 50, 15_100, 11.0),
    ("LA", "Louisiana", "New Orleans", 65, 15_600, 13.0),
]
TX_HUBS = {"TX": (29.760, -95.370), "OK": (35.468, -97.516), "LA": (29.951, -90.072)}

# power grid, road condition, broadband reach (a 1981 value: almost none), workforce skill,
# crime, cost of living, land cost
TX_CONDITIONS = {
    "TX": (9300, 6400, 600, 4800, 10500, 9000, 8000),
    "OK": (9400, 6000, 400, 4600, 8500, 8200, 6500),
    "LA": (9000, 5000, 400, 4000, 11500, 8800, 7000),
}

TX_SPECIALIZATION = {
    "TX": ("extraction", "chemical_industries"),
    "OK": ("extraction", "agriculture"),
    "LA": ("chemical_industries", "logistics"),
}

# Region code, name, anchor city, anchor lat, anchor lon.
TX_REGIONS = [
    ("TX-HOU", "Houston and the Gulf Coast", "Houston", 29.760, -95.370),
    ("TX-BPA", "Beaumont and Port Arthur", "Beaumont", 30.080, -94.126),
    ("TX-DFW", "Dallas and Fort Worth", "Dallas", 32.777, -96.797),
    ("TX-SAT", "San Antonio", "San Antonio", 29.424, -98.494),
    ("TX-AUS", "Austin and Central Texas", "Austin", 30.267, -97.743),
    ("TX-PMB", "Permian Basin", "Midland", 31.997, -102.078),
    ("TX-CRP", "Corpus Christi and the South Texas Coast", "Corpus Christi", 27.801, -97.396),
    ("TX-AMA", "Texas Panhandle", "Amarillo", 35.222, -101.831),
    ("TX-ELP", "El Paso and the Trans-Pecos", "El Paso", 31.761, -106.485),
    ("TX-TYL", "East Texas", "Tyler", 32.351, -95.301),
    ("TX-RGV", "Rio Grande Valley", "McAllen", 26.203, -98.230),
    ("TX-LUB", "Lubbock and the South Plains", "Lubbock", 33.578, -101.855),
    ("TX-ABI", "Abilene and West Central Texas", "Abilene", 32.449, -99.733),
    ("OK-OKC", "Oklahoma City", "Oklahoma City", 35.468, -97.516),
    ("OK-TUL", "Tulsa and Eastern Oklahoma", "Tulsa", 36.154, -95.993),
    ("OK-CSH", "Cushing and North Central Oklahoma", "Cushing", 35.985, -96.767),
    ("OK-WES", "Western Oklahoma and the Anadarko Basin", "Woodward", 36.430, -99.390),
    ("LA-NOL", "New Orleans", "New Orleans", 29.951, -90.072),
    ("LA-BTR", "Baton Rouge", "Baton Rouge", 30.451, -91.187),
    ("LA-LCH", "Lake Charles", "Lake Charles", 30.226, -93.217),
    ("LA-SHV", "Shreveport and North Louisiana", "Shreveport", 32.525, -93.750),
    ("LA-LAF", "Lafayette and Acadiana", "Lafayette", 30.224, -92.020),
]

# 1981 oil boom adjustment to a region's wage and demand against what its 2000 income says, in
# percent. Authored game estimates: the Permian, Houston and Acadiana paid oilfield wages.
TX_BOOM = {
    "TX-PMB": 22, "TX-HOU": 12, "TX-BPA": 6, "TX-CRP": 4, "TX-AMA": 4, "TX-ABI": 3,
    "OK-OKC": 6, "OK-TUL": 6, "OK-CSH": 4, "OK-WES": 8,
    "LA-LAF": 12, "LA-LCH": 5, "LA-NOL": 3,
}


def build_texas():
    counties = list(csv.DictReader(TX_COUNTIES.open()))
    anchors = defaultdict(list)
    for region in TX_REGIONS:
        anchors[region[0][:2]].append(region)
    members = defaultdict(list)
    TX_ASSIGNMENT.parent.mkdir(parents=True, exist_ok=True)
    with TX_ASSIGNMENT.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["geoid", "region"])
        for county in counties:
            state = TX_FIPS[county["statefp"]]
            point = (float(county["lat"]), float(county["lon"]))
            region = min(anchors[state], key=lambda r: (great_circle_km(point, (r[3], r[4])), r[0]))
            members[region[0]].append(county)
            writer.writerow([county["geoid"], region[0]])

    def totals(rows):
        population = sum(int(c["population_1980"]) for c in rows)
        income = sum(int(c["population_1980"]) * int(c["median_household_income_2000"]) for c in rows)
        land = sum(int(c["land_km2"]) for c in rows)
        return population, income, land

    base_gdp = 190  # Texas = 10000
    base_pay = 15_500
    state_rows = {code: [c for c in counties if TX_FIPS[c["statefp"]] == code] for code, *_ in TX_STATES}
    regions = []
    states = []
    for code, name, city, gdp, pay, union in TX_STATES:
        grid, road, broadband, skill, crime, living, land_index = TX_CONDITIONS[code]
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
            boom = 1 + TX_BOOM.get(rcode, 0) / 100
            regions.append({
                "code": rcode,
                "name": rname,
                "states": [code],
                "principal_city": f"{rcity}, {code}",
                "lat_milli": round(rlat * 1000),
                "lon_milli": round(rlon * 1000),
                "population_1980": rpop,
                "wage_index_bps": round(state_wage * mhi_ratio ** 0.6 * boom),
                "demand_index_bps": max(1, round(state_demand * rincome / state_income * boom)),
                "land_index_bps": round(land_index * clamp(density_ratio ** 0.35, 0.55, 1.8)),
                "opening_organisation_bps": round(union * 100),
                "conditions": {
                    "power_grid_bps": grid,
                    "road_bps": road,
                    "broadband_bps": round(clamp(broadband * clamp(density_ratio ** 0.25, 0.6, 1.5), 0, 10_000)),
                    "skill_bps": round(clamp(skill * clamp(mhi_ratio ** 0.5, 0.8, 1.2), 0, 10_000)),
                    "crime_bps": crime,
                    "cost_of_living_bps": round(living * clamp(mhi_ratio ** 0.5, 0.85, 1.2)),
                },
                "counties": len(rows),
            })
        states.append({
            "code": code, "name": name, "capital_hub": f"{city}, {code}",
            "population_1980": state_pop, "specialization": list(TX_SPECIALIZATION[code]),
        })
    lanes = []
    for i, a in enumerate(TX_REGIONS):
        for b in TX_REGIONS[i + 1:]:
            km = great_circle_km((a[3], a[4]), (b[3], b[4])) * ROAD_FACTOR
            lanes.append({"a": a[0], "b": b[0], "km": max(1, round(km))})
    document = {
        "schema": 3,
        "classification": "economic regions built from whole counties in Texas, Oklahoma and Louisiana; 1980 census county populations, SAIPE 2000 county income used only as a relative index, rounded public circa-1981 state product, pay and union figures as relative indices, authored condition indices and oil boom adjustments",
        "vintage": "nominal January 1981 start",
        "official_source": "US Census Bureau 1980 census county populations (intercensal estimates 1980-1989 base) and SAIPE 2000 median household income; BEA, BLS and union membership figures rounded for indices",
        "attempts": [],
        "membership_status": "each region is a set of whole counties joined to the nearest anchor metro in the same state; tools/map/data/county-regions-texas-1981.csv lists the assignment",
        "notes": "Generated by tools/map/build_regions.py. Condition indices, oil boom adjustments and calibrated demand are game estimates, not observations. Freight lanes are anchor-to-anchor great-circle distances times a 1.25 road factor.",
        "states": states,
        "regions": regions,
        "lanes": lanes,
    }
    TX_OUT.parent.mkdir(parents=True, exist_ok=True)
    TX_OUT.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {TX_OUT.relative_to(ROOT)}: {len(regions)} regions, {len(lanes)} lanes")
    for r in regions:
        print(f"  {r['code']:7} {r['name']:42} counties {r['counties']:3} pop {r['population_1980']:>10,} "
              f"wage {r['wage_index_bps']:>6} demand {r['demand_index_bps']:>6} land {r['land_index_bps']:>6}")


# RUHR AND RHINE: Ruhr and Rhine, 1991 (Germany, the Netherlands, Belgium, Luxembourg, France)
#
# Regions are unions of whole NUTS 3 areas (Eurostat GISCO NUTS 2016, listed in
# tools/map/data/nuts3-ruhr-rhine-1991.csv by tools/map/fetch_nuts3_1991.py), assigned by NUTS code
# prefix below. There is no 1991 NUTS 3 population series, so every population, income index,
# wage index, demand index, land index, union density and condition index is an AUTHORED GAME
# ESTIMATE from rounded public knowledge of circa-1991 Europe, not an observation. The political
# layer ("state") is the country, so a market never crosses a border; region codes are
# COUNTRY-AREA. East Germany (the new states and Berlin) carries lower wages, lower income and
# lower demand: the old GDR industry was collapsing under the Treuhand privatisation.
# ---------------------------------------------------------------------------------------------

RR_NUTS = ROOT / "tools/map/data/nuts3-ruhr-rhine-1991.csv"
RR_ASSIGNMENT = ROOT / "tools/map/data/nuts3-regions-ruhr-rhine-1991.csv"
RR_OUT = ROOT / "content/scenarios/ruhr-rhine-1991/regions.json"

# code, name, hub city, specialization
RR_STATES = [
    ("DE", "Germany", "Frankfurt", ("manufacturing", "chemical_industries")),
    ("NL", "Netherlands", "Rotterdam", ("logistics", "chemical_industries")),
    ("BE", "Belgium", "Antwerp", ("logistics", "manufacturing")),
    ("LU", "Luxembourg", "Luxembourg", ("financial", "manufacturing")),
    ("FR", "France (Alsace and Lorraine)", "Strasbourg", ("manufacturing", "agriculture")),
]

# power grid, road condition, broadband reach (a 1991 value: almost none), workforce skill,
# crime, cost of living, by country; East Germany has its own row.
RR_CONDITIONS = {
    "DE": (9800, 8800, 700, 7200, 6000, 10200),
    "DE-OST": (8300, 6200, 200, 6400, 7200, 7400),
    "NL": (9900, 9000, 800, 7000, 8000, 11000),
    "BE": (9700, 8200, 600, 6800, 8500, 10200),
    "LU": (9900, 9000, 800, 7400, 6500, 11500),
    "FR": (9700, 8200, 500, 6500, 8000, 9500),
}

# NUTS 3 code prefixes, first match wins.
RR_RULES = [
    (("DEA1", "DEA31", "DEA32", "DEA36", "DEA51", "DEA52", "DEA53", "DEA54", "DEA55", "DEA56", "DEA5C"), "DE-RUH"),
    (("DEA",), "DE-RHW"),
    (("DE7",), "DE-RHM"),
    (("DEB",), "DE-RLP"),
    (("DEC",), "DE-SAA"),
    (("DE6", "DEF"), "DE-HAM"),
    (("DE9", "DE5"), "DE-HAN"),
    (("DE1",), "DE-STU"),
    (("DE2",), "DE-MUC"),
    (("DE3", "DE4", "DE8", "DED", "DEE", "DEG"), "DE-OST"),
    (("NL33", "NL34"), "NL-ROT"),
    (("NL41", "NL42"), "NL-LIM"),
    (("NL",), "NL-AMS"),
    (("BE1", "BE2"), "BE-ANT"),
    (("BE3",), "BE-LIE"),
    (("LU",), "LU-LUX"),
    (("FRF1",), "FR-ALS"),
    (("FRF3",), "FR-LOR"),
]

# code, name, anchor city, anchor lat, anchor lon, population 1991 (thousands, estimate), income
# against West Germany (1.0), demand adjustment against income (1.0), weekly wage index, land
# index, union density % (rounded)
RR_REGIONS = [
    ("DE-RUH", "Ruhr and Lower Rhine", "Essen", 51.455, 7.012, 9000, 0.95, 1.00, 10300, 14000, 38),
    ("DE-RHW", "Cologne, Bonn and Rhine-Westphalia", "Cologne", 50.938, 6.960, 8700, 1.00, 1.00, 10500, 12000, 33),
    ("DE-RHM", "Rhine-Main and Hesse", "Frankfurt", 50.110, 8.682, 5800, 1.25, 1.00, 11800, 18000, 30),
    ("DE-RLP", "Palatinate and Rhineland-Palatinate", "Ludwigshafen", 49.477, 8.445, 3800, 0.95, 1.00, 9900, 9500, 30),
    ("DE-SAA", "Saarland", "Saarbrucken", 49.240, 6.997, 1070, 0.90, 1.00, 9700, 9500, 38),
    ("DE-HAM", "Hamburg and the North Coast", "Hamburg", 53.551, 9.994, 4250, 1.20, 1.00, 11300, 13000, 30),
    ("DE-HAN", "Hanover and Lower Saxony", "Hanover", 52.375, 9.732, 8100, 1.00, 1.00, 10300, 9000, 34),
    ("DE-STU", "Stuttgart and Baden-Wurttemberg", "Stuttgart", 48.776, 9.183, 9900, 1.15, 1.00, 11600, 13000, 32),
    ("DE-MUC", "Munich and Bavaria", "Munich", 48.137, 11.575, 11600, 1.20, 1.00, 11800, 17000, 28),
    ("DE-OST", "Berlin and the New States", "Berlin", 52.520, 13.405, 18000, 0.45, 0.90, 5400, 6500, 42),
    ("NL-ROT", "Rotterdam and Zuid-Holland", "Rotterdam", 51.924, 4.478, 3550, 1.00, 1.00, 10600, 16000, 25),
    ("NL-AMS", "Amsterdam, the North and East", "Amsterdam", 52.370, 4.895, 8150, 1.10, 1.00, 10900, 18000, 25),
    ("NL-LIM", "Brabant and Limburg", "Eindhoven", 51.441, 5.478, 3200, 0.95, 1.00, 9900, 11000, 24),
    ("BE-ANT", "Antwerp, Flanders and Brussels", "Antwerp", 51.220, 4.400, 6750, 1.00, 1.00, 10500, 15000, 53),
    ("BE-LIE", "Wallonia and Liege", "Liege", 50.633, 5.568, 3250, 0.80, 1.00, 9000, 8500, 58),
    ("LU-LUX", "Luxembourg", "Luxembourg", 49.612, 6.130, 380, 1.50, 1.00, 11800, 14000, 50),
    ("FR-LOR", "Lorraine", "Metz", 49.119, 6.176, 2310, 0.80, 1.00, 8900, 7000, 10),
    ("FR-ALS", "Alsace", "Strasbourg", 48.573, 7.752, 1620, 0.95, 1.00, 9500, 10000, 12),
]


def build_ruhr():
    nuts = list(csv.DictReader(RR_NUTS.open()))
    assignment = {}
    with RR_ASSIGNMENT.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["nuts_id", "region"])
        for row in nuts:
            for prefixes, region in RR_RULES:
                if row["nuts_id"].startswith(prefixes):
                    assignment[row["nuts_id"]] = region
                    writer.writerow([row["nuts_id"], region])
                    break
            else:
                raise SystemExit(f"{row['nuts_id']} {row['name']} matches no region rule")
    counts = defaultdict(int)
    for region in assignment.values():
        counts[region] += 1
    regions = []
    for code, name, city, lat, lon, pop, income, adj, wage, land, union in RR_REGIONS:
        if not counts[code]:
            raise SystemExit(f"{code} has no NUTS 3 areas")
        country = code[:2]
        grid, road, broadband, skill, crime, living = RR_CONDITIONS["DE-OST" if code == "DE-OST" else country]
        factor = clamp(income ** 0.5, 0.8, 1.2)
        regions.append({
            "code": code,
            "name": name,
            "states": [country],
            "principal_city": f"{city}, {country}",
            "country": country,
            "lat_milli": round(lat * 1000),
            "lon_milli": round(lon * 1000),
            "population_1990": pop * 1000,
            "wage_index_bps": wage,
            "demand_index_bps": max(1, round(pop * 0.95 * income * adj)),
            "land_index_bps": land,
            "opening_organisation_bps": union * 100,
            "conditions": {
                "power_grid_bps": grid,
                "road_bps": road,
                "broadband_bps": round(clamp(broadband * factor, 0, 10_000)),
                "skill_bps": round(clamp(skill * factor, 0, 10_000)),
                "crime_bps": crime,
                "cost_of_living_bps": round(living * factor),
            },
            "nuts3_areas": counts[code],
        })
    states = []
    for code, name, city, specialization in RR_STATES:
        states.append({
            "code": code, "name": name, "capital_hub": f"{city}, {code}",
            "population_1990": sum(r["population_1990"] for r in regions if r["country"] == code),
            "specialization": list(specialization),
        })
    lanes = []
    for i, a in enumerate(RR_REGIONS):
        for b in RR_REGIONS[i + 1:]:
            km = great_circle_km((a[3], a[4]), (b[3], b[4])) * ROAD_FACTOR
            lanes.append({"a": a[0], "b": b[0], "km": max(1, round(km))})
    document = {
        "schema": 3,
        "classification": "economic regions built from whole NUTS 3 areas in Germany, the Netherlands, Belgium, Luxembourg and (Alsace and Lorraine) France; every population, income, wage, demand, land, union and condition figure is an authored game estimate for early 1991, not an observation",
        "vintage": "nominal January 1991 start",
        "official_source": "Eurostat GISCO NUTS 2016 boundaries (area geometry and codes only, (C) European Union); no statistical series was used, all 1991 values are rounded game estimates informed by public circa-1991 knowledge",
        "attempts": [],
        "membership_status": "each region is a set of whole NUTS 3 areas assigned by code prefix; tools/map/data/nuts3-regions-ruhr-rhine-1991.csv lists the assignment. The political layer is the country, so no market or state crosses a border",
        "notes": "Generated by tools/map/build_regions.py. East Germany (DE-OST, Berlin and the new states) carries a low wage index and low demand after the collapse of GDR industry and Treuhand privatisation. Luxembourg is one region. Freight lanes are anchor-to-anchor great-circle distances times a 1.25 road factor.",
        "states": states,
        "regions": regions,
        "lanes": lanes,
    }
    RR_OUT.parent.mkdir(parents=True, exist_ok=True)
    RR_OUT.write_text(json.dumps(document, indent=2) + "\n")
    print(f"wrote {RR_OUT.relative_to(ROOT)}: {len(regions)} regions, {len(lanes)} lanes")
    for r in regions:
        print(f"  {r['code']:7} {r['name']:38} nuts3 {r['nuts3_areas']:3} pop {r['population_1990']:>10,} "
              f"wage {r['wage_index_bps']:>6} demand {r['demand_index_bps']:>6} land {r['land_index_bps']:>6}")


BUILDERS = {"great-lakes-2000": build_great_lakes, "texas-1981": build_texas, "silicon-valley-1995": build_california, "ruhr-rhine-1991": build_ruhr}


def main():
    wanted = sys.argv[1:] or list(BUILDERS)
    for scenario in wanted:
        BUILDERS[scenario]()


if __name__ == "__main__":
    main()
