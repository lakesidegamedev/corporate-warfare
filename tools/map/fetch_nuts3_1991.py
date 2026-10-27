#!/usr/bin/env python3
"""Write tools/map/data/nuts3-ruhr-rhine-1991.csv: the NUTS 3 areas of the Ruhr and Rhine scenario.

Source: Eurostat GISCO NUTS 2016 boundaries, 10M resolution, EPSG:4326 GeoJSON
(https://gisco-services.ec.europa.eu/distribution/v2/nuts/). (C) European Union, reuse allowed
with attribution; the apps' attribution line credits it. The 2016 vintage is used because its
German Kreise and Dutch COROP areas are the closest published match to the 1991 map the scenario
plays on. All 1991 figures built on top of these areas are game estimates; see build_regions.py.

Writes every NUTS 3 area of Germany, the Netherlands, Belgium and Luxembourg, plus Alsace and
Lorraine in France (NUTS 2016 FRF1 and FRF3). The GeoJSON itself is not committed: the map build
reads it from the download directory.

Usage: tools/map/fetch_nuts3_1991.py [download-dir]    (default /tmp/ruhr-map-src)
"""
import csv
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "tools/map/data/nuts3-ruhr-rhine-1991.csv"
URL = "https://gisco-services.ec.europa.eu/distribution/v2/nuts/geojson/NUTS_RG_10M_2016_4326_LEVL_3.geojson"
NAME = "NUTS_RG_10M_2016_4326_LEVL_3.geojson"


def main():
    directory = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/ruhr-map-src")
    directory.mkdir(parents=True, exist_ok=True)
    source = directory / NAME
    if not source.exists():
        urllib.request.urlretrieve(URL, source)
    features = json.load(open(source))["features"]
    rows = []
    for feature in features:
        p = feature["properties"]
        country, nuts = p["CNTR_CODE"], p["NUTS_ID"]
        if country in ("DE", "NL", "BE", "LU") or nuts.startswith(("FRF1", "FRF3")):
            rows.append((nuts, p["NAME_LATN"], country))
    rows.sort()
    with OUT.open("w", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["nuts_id", "name", "country"])
        writer.writerows(rows)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(rows)} NUTS 3 areas")


if __name__ == "__main__":
    main()
