"""Compact GeoJSON layers into apple/Resources/world-map.json.

Input: the mapshaper outputs written by tools/map/build_map.sh, in longitude and latitude. Output:
integer coordinates in ten-thousandths of a degree, flat [lon0, lat0, lon1, lat1, ...] rings and
lines, with points thinned to a minimum spacing. The apps project at draw time. Sources are US
Census cartographic boundary and TIGER files and Natural Earth, all public domain.
"""
import json
import math
import sys
from pathlib import Path

PLAY = ["IL", "IN", "KY", "MI", "NY", "OH", "PA", "WV"]
FIPS = {"17": "IL", "18": "IN", "21": "KY", "26": "MI", "36": "NY", "39": "OH", "42": "PA", "54": "WV"}
SCALE = 10_000
# Spacing in the old 10000-unit corridor box was about 190 m a unit; one unit is about 18 here.
UNIT = 18


def load(directory, name):
    return json.load(open(Path(directory) / f"{name}.json"))["features"]


def geometries(feature):
    geometry = feature.get("geometry") or {}
    kind = geometry.get("type")
    coords = geometry.get("coordinates") or []
    if kind == "Polygon":
        return [coords]
    if kind == "MultiPolygon":
        return coords
    if kind == "LineString":
        return [[coords]]
    if kind == "MultiLineString":
        return [[line] for line in coords]
    return []


def main(source, output):
    layers = {name: load(source, name) for name in
              ["countries", "play_states", "context_states", "canada", "counties", "regions", "lakes",
               "rivers", "interstates_thin", "cities", "hubs"]}
    names = {r["code"]: r["name"] for r in
             json.load(open(Path(__file__).resolve().parents[2] / "content/geography/regions.json"))["regions"]}

    def point(lon, lat):
        return round(lon * SCALE), round(lat * SCALE)

    def flat(sequence, spacing, closed):
        out = []
        last = None
        limit = spacing * UNIT
        for lon, lat in sequence:
            px, py = point(lon, lat)
            if last is not None:
                east = abs(px - last[0]) * math.cos(math.radians(lat))
                if east + abs(py - last[1]) < limit:
                    continue
            out += [px, py]
            last = (px, py)
        minimum = 6 if closed else 4
        return out if len(out) >= minimum else None

    def rings(feature, spacing=3):
        result = []
        for polygon in geometries(feature):
            for ring in polygon:
                compact = flat(ring, spacing, True)
                if compact:
                    result.append(compact)
        return result

    def lines(feature, spacing):
        result = []
        for group in geometries(feature):
            for line in group:
                compact = flat(line, spacing, False)
                if compact:
                    result.append(compact)
        return result

    countries = []
    for feature in layers["countries"]:
        p = feature["properties"]
        r = rings(feature, 0)
        if r:
            countries.append({"code": p.get("ADM0_A3") or "", "name": p.get("NAME") or "", "rings": r})

    states = []
    for feature in layers["play_states"]:
        p = feature["properties"]
        states.append({"code": p["STUSPS"], "name": p["NAME"], "play": True, "rings": rings(feature, 2)})
    for feature in layers["context_states"]:
        p = feature["properties"]
        r = rings(feature, 4)
        if r:
            states.append({"code": p["STUSPS"], "name": p["NAME"], "play": False, "rings": r})
    for feature in layers["canada"]:
        p = feature["properties"]
        r = rings(feature, 4)
        if r:
            states.append({"code": "CA-" + (p.get("postal") or p.get("name", "")[:2]).upper(),
                           "name": p.get("name"), "play": False, "rings": r})
    counties = []
    for feature in layers["counties"]:
        p = feature["properties"]
        r = rings(feature, 3)
        if r:
            counties.append({"fips": p["GEOID"], "name": p["NAME"], "state": FIPS[p["STATEFP"]], "rings": r})
    regions = []
    for feature in layers["regions"]:
        code = feature["properties"]["region"]
        r = rings(feature, 2)
        if r:
            regions.append({"code": code, "state": code[:2], "name": names[code], "rings": r})
    regions.sort(key=lambda region: region["code"])
    if sorted(names) != [region["code"] for region in regions]:
        raise SystemExit("region shapes do not match content/geography/regions.json")
    lakes = [{"name": f["properties"].get("name") or "", "rings": rings(f, 3)} for f in layers["lakes"]]
    lakes = [lake for lake in lakes if lake["rings"]]
    rivers = [{"name": f["properties"].get("name") or "", "lines": lines(f, 6)} for f in layers["rivers"]]
    rivers = [river for river in rivers if river["lines"]]
    interstates = [{"name": f["properties"].get("FULLNAME") or "", "lines": lines(f, 10)}
                   for f in layers["interstates_thin"]]
    interstates = [road for road in interstates if road["lines"]]
    cities = []
    for feature in layers["cities"]:
        p = feature["properties"]
        x, y = feature["geometry"]["coordinates"]
        px, py = point(x, y)
        region = p.get("ADM1NAME") or ""
        cities.append({
            "name": " ".join(p["NAME"].split()),
            "region": region,
            "country": p.get("ADM0_A3"),
            "x": px,
            "y": py,
            "population": int(p.get("POP_MAX") or 0),
            "capital": "capital" in (p.get("FEATURECLA") or "").lower(),
        })
    cities.sort(key=lambda city: -city["population"])
    hubs = {}
    for feature in layers["hubs"]:
        x, y = feature["geometry"]["coordinates"]
        hubs[feature["properties"]["code"]] = list(point(x, y))
    document = {
        "schema": 3,
        "attribution": "Boundaries and roads: US Census Bureau cartographic boundary and TIGER/Line files (2018). Countries, lakes, rivers, places and provinces: Natural Earth. All public domain.",
        "units": "ten-thousandths of a degree, longitude then latitude",
        "countries": countries,
        "states": states,
        "regions": regions,
        "counties": counties,
        "lakes": lakes,
        "rivers": rivers,
        "interstates": interstates,
        "cities": cities,
        "hubs": hubs,
    }
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    Path(output).write_text(json.dumps(document, separators=(",", ":")))
    size = Path(output).stat().st_size
    print(f"wrote {output}: {size // 1024} KB, {len(countries)} countries, {len(states)} states/provinces, {len(regions)} regions, {len(counties)} counties, "
          f"{len(lakes)} lakes, {len(rivers)} rivers, {len(interstates)} interstates, {len(cities)} cities")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
