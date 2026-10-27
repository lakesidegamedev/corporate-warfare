#!/usr/bin/env python3
"""Write tools/map/data/counties-1980-texas-1981.csv: Texas, Oklahoma and Louisiana counties.

Sources, all US Census Bureau and public domain:
  * 1980 census county populations: "Intercensal Estimates of the Resident Population of States
    and Counties 1980-1989" (e8089co.txt), the first column is the April 1980 census count.
  * Interior points and land area: 2020 Gazetteer counties file (county boundaries in these three
    states did not change between 1980 and 2020).
  * Median household income: SAIPE 2000 (est00all.dat). The Census does not publish county income
    estimates for 1980 in this format, so the 2000 values are used only as a relative index of
    which counties were richer than their state; build_regions.py documents this.

Usage: tools/map/fetch_counties_1980.py [download-dir]
"""
import csv
import io
import re
import sys
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tools/map/data/counties-1980-texas-1981.csv"
STATES = {"48": "TX", "40": "OK", "22": "LA"}

POP = "https://www2.census.gov/programs-surveys/popest/tables/1980-1990/counties/totals/e8089co.txt"
GAZ = "https://www2.census.gov/geo/docs/maps-data/data/gazetteer/2020_Gazetteer/2020_Gaz_counties_national.zip"
SAIPE = "https://www2.census.gov/programs-surveys/saipe/datasets/2000/2000-state-and-county/est00all.dat"


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def main():
    population = {}
    lines = fetch(POP).decode("latin-1").splitlines()
    for index, line in enumerate(lines):
        # A long parish name wraps: the numbers sit on the next line.
        if re.match(r"^\d{5} [A-Za-z. ]+$", line) and index + 1 < len(lines):
            line = line.rstrip() + " " + lines[index + 1].strip()
        match = re.match(r"^(\d{5})\s+(.+?)\s+(\d+)\s+\d+\s+\d+\s+\d+\s+\d+\s*$", line)
        if match and match.group(1)[:2] in STATES and not match.group(1).endswith("000"):
            # The file repeats every county for 1985-89; the first block is the 1980 census.
            population.setdefault(match.group(1), int(match.group(3)))
    income = {}
    # Fixed-width: median household income estimate is the 22nd whitespace field.
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
            "population_1980": population[geoid],
            "median_household_income_2000": income[geoid],
        })
    rows.sort(key=lambda r: r["geoid"])
    with OUT.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(rows)} counties, {sum(r['population_1980'] for r in rows):,} people in 1980")


if __name__ == "__main__":
    main()
