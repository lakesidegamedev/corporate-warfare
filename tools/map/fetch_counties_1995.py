#!/usr/bin/env python3
"""Write tools/map/data/counties-1995-silicon-valley-1995.csv: California counties.

Sources, all US Census Bureau and public domain:
  * July 1, 1995 county population estimates: "County Population Estimates and Demographic
    Components of Population Change: Annual Time Series, July 1, 1990 to July 1, 1999" (CO-99-8,
    file 99c8_06.txt, Block 1). The columns run from July 1999 back to July 1990 then the April
    1990 base, so July 1995 is the fifth number. This is the revised estimate series released in
    March 2000.
  * Interior points and land area: 2020 Gazetteer counties file (California county boundaries
    that matter here did not change between 1995 and 2020).
  * Median household income: SAIPE 2000 (est00all.dat). The Census does not publish a 1995 county
    income series in this format, so the 2000 values are used only as a relative index of which
    counties were richer than the state; build_regions.py documents this.

Usage: tools/map/fetch_counties_1995.py
"""
import csv
import io
import re
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tools/map/data/counties-1995-silicon-valley-1995.csv"
STATES = {"06": "CA"}

POP = "https://www2.census.gov/programs-surveys/popest/tables/1990-2000/counties/totals/99c8_06.txt"
GAZ = "https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2020_Gazetteer/2020_Gaz_counties_national.zip"
SAIPE = "https://www2.census.gov/programs-surveys/saipe/datasets/2000/2000-state-and-county/est00all.dat"


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def main():
    population = {}
    for line in fetch(POP).decode("latin-1").splitlines():
        match = re.match(r"^1\s+(06\d{3})\s+((?:[\d,]+\s+){11})", line)
        if match and not match.group(1).endswith("000"):
            numbers = [int(n.replace(",", "")) for n in match.group(2).split()]
            population[match.group(1)] = numbers[4]  # July 1995
    income = {}
    for line in fetch(SAIPE).decode("latin-1").splitlines():
        if line[:2] in STATES and line[3:6].strip() != "0":
            fields = line.split()
            income[line[:2] + fields[1].zfill(3)] = int(fields[21])
    archive = zipfile.ZipFile(io.BytesIO(fetch(GAZ)))
    rows = []
    text = archive.read(archive.namelist()[0]).decode("latin-1")
    for row in csv.DictReader(io.StringIO(text), delimiter="\t"):
        row = {k.strip(): v.strip() for k, v in row.items()}
        geoid = row["GEOID"]
        if geoid[:2] not in STATES:
            continue
        rows.append({
            "geoid": geoid,
            "statefp": geoid[:2],
            "name": re.sub(r" (County|Parish)$", "", row["NAME"]),
            "lon": row["INTPTLONG"],
            "lat": row["INTPTLAT"],
            "land_km2": round(float(row["ALAND_SQMI"]) * 2.58999),
            "population_1995": population[geoid],
            "median_household_income_2000": income[geoid],
        })
    rows.sort(key=lambda r: r["geoid"])
    with OUT.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(rows)} counties, {sum(r['population_1995'] for r in rows):,} people in 1995")


if __name__ == "__main__":
    main()
