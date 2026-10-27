"""Compact GeoJSON layers into apple/Resources/world-map.json.

A second mode, `compact_map.py --merge <scenario-id> <dir> <world-map.json>`, adds one scenario's
layers (from tools/map/build_scenario_map.sh) to an existing file and leaves everything else alone.

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
# The states each scenario plays in. The apps show the regions and framing of the scenario of the
# career they are in, found from the region codes the engine reports.
SCENARIOS = {
    "great-lakes-2000": PLAY,
    "texas-1981": ["LA", "OK", "TX"],
    "silicon-valley-1995": ["CA"],
}
SCENARIO_FIPS = {"texas-1981": {"48": "TX", "40": "OK", "22": "LA"}, "silicon-valley-1995": {"06": "CA"}}
SCALE = 10_000
# Spacing in the old 10000-unit corridor box was about 190 m a unit; one unit is about 18 here.
UNIT = 18


def load(directory, name):
    document = json.load(open(Path(directory) / f"{name}.json"))
    if "features" in document:
        return document["features"]
    # -innerlines writes bare geometries.
    return [{"type": "Feature", "properties": {}, "geometry": g} for g in document.get("geometries", [])]


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


def main(source, output):
    layers = {name: load(source, name) for name in
              ["countries", "provinces", "world_roads", "play_states", "context_states", "canada", "counties", "regions", "lakes",
               "rivers", "interstates_thin", "cities", "hubs"]}
    names = {r["code"]: r["name"] for r in
             json.load(open(Path(__file__).resolve().parents[2] / "content/geography/regions.json"))["regions"]}

    countries = []
    for feature in layers["countries"]:
        p = feature["properties"]
        r = rings(feature, 0)
        if r:
            countries.append({"code": p.get("ADM0_A3") or "", "name": p.get("NAME") or "", "rings": r})

    scenario_areas = []
    for name in ["scenario_great_lakes", "scenario_texas", "scenario_california", "scenario_germany"]:
        for feature in load(source, name):
            r = rings(feature, 2)
            if r:
                scenario_areas.append({"id": feature["properties"]["id"], "rings": r})
    provinces = [line for f in layers["provinces"] for line in lines(f, 4)]
    roads = [line for f in layers["world_roads"] for line in lines(f, 6)]

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
        "provinces": provinces,
        "scenario_areas": scenario_areas,
        "roads": roads,
        "states": states,
        "regions": regions,
        "counties": counties,
        "lakes": lakes,
        "rivers": rivers,
        "interstates": interstates,
        "cities": cities,
        "hubs": hubs,
        "scenarios": [{"id": key, "states": SCENARIOS[key]} for key in sorted(SCENARIOS)],
    }
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    Path(output).write_text(json.dumps(document, separators=(",", ":")))
    size = Path(output).stat().st_size
    print(f"wrote {output}: {size // 1024} KB, {len(countries)} countries, {len(provinces)} province borders, {len(roads)} roads, {len(states)} states/provinces, {len(regions)} regions, {len(counties)} counties, "
          f"{len(lakes)} lakes, {len(rivers)} rivers, {len(interstates)} interstates, {len(cities)} cities")


def merge(scenario, source, output):
    """Add one scenario's layers to an existing world map, replacing an earlier copy of them."""
    play = SCENARIOS[scenario]
    fips = SCENARIO_FIPS[scenario]
    document = json.load(open(output))
    layers = {name: load(source, name) for name in
              ["play_states", "context_states", "counties", "regions", "rivers", "interstates_thin", "cities", "hubs"]}
    names = {r["code"]: r["name"] for r in
             json.load(open(Path(__file__).resolve().parents[2] / f"content/scenarios/{scenario}/regions.json"))["regions"]}

    # Shapes tagged with this scenario are ours from an earlier run; states and regions are
    # recognised by code, and counties by state.
    def ours(entry):
        return entry.get("scenario") == scenario

    states = [s for s in document["states"] if s["code"] not in play and not ours(s)]
    for feature in layers["play_states"]:
        p = feature["properties"]
        states.append({"code": p["STUSPS"], "name": p["NAME"], "play": True, "rings": rings(feature, 2)})
    have = {s["code"] for s in states}
    for feature in layers["context_states"]:
        p = feature["properties"]
        r = rings(feature, 4)
        if r and p["STUSPS"] not in have:
            states.append({"code": p["STUSPS"], "name": p["NAME"], "play": False, "rings": r, "scenario": scenario})
    document["states"] = states

    regions = [r for r in document["regions"] if r["state"] not in play]
    for feature in layers["regions"]:
        code = feature["properties"]["region"]
        r = rings(feature, 2)
        if r:
            regions.append({"code": code, "state": code[:2], "name": names[code], "rings": r})
    regions.sort(key=lambda region: region["code"])
    ours_codes = sorted(code for code in names)
    if ours_codes != sorted(r["code"] for r in regions if r["state"] in play):
        raise SystemExit("region shapes do not match the scenario's regions.json")
    document["regions"] = regions

    counties = [c for c in document["counties"] if c["state"] not in play]
    for feature in layers["counties"]:
        p = feature["properties"]
        r = rings(feature, 3)
        if r:
            counties.append({"fips": p["GEOID"], "name": p["NAME"], "state": fips[p["STATEFP"]], "rings": r})
    document["counties"] = counties

    rivers = [r for r in document["rivers"] if not ours(r)]
    for feature in layers["rivers"]:
        found = lines(feature, 6)
        if found:
            rivers.append({"name": feature["properties"].get("name") or "", "lines": found, "scenario": scenario})
    document["rivers"] = rivers

    interstates = [r for r in document["interstates"] if not ours(r)]
    for feature in layers["interstates_thin"]:
        found = lines(feature, 10)
        if found:
            interstates.append({"name": feature["properties"].get("FULLNAME") or "", "lines": found, "scenario": scenario})
    document["interstates"] = interstates

    cities = [c for c in document["cities"] if not ours(c)]
    seen = {(c["name"], c["x"], c["y"]) for c in cities}
    for feature in layers["cities"]:
        p = feature["properties"]
        x, y = feature["geometry"]["coordinates"]
        px, py = point(x, y)
        name = " ".join(p["NAME"].split())
        if (name, px, py) in seen:
            continue
        cities.append({
            "name": name,
            "region": p.get("ADM1NAME") or "",
            "country": p.get("ADM0_A3"),
            "x": px,
            "y": py,
            "population": int(p.get("POP_MAX") or 0),
            "capital": "capital" in (p.get("FEATURECLA") or "").lower(),
            "scenario": scenario,
        })
    cities.sort(key=lambda city: -city["population"])
    document["cities"] = cities

    for feature in layers["hubs"]:
        x, y = feature["geometry"]["coordinates"]
        document["hubs"][feature["properties"]["code"]] = list(point(x, y))

    # Which states each scenario plays in, so an app shows only the active scenario's regions.
    scenarios = {s["id"]: s for s in document.get("scenarios", [])}
    scenarios.setdefault("great-lakes-2000", {"id": "great-lakes-2000", "states": SCENARIOS["great-lakes-2000"]})
    scenarios[scenario] = {"id": scenario, "states": play}
    document["scenarios"] = [scenarios[key] for key in sorted(scenarios)]
    Path(output).write_text(json.dumps(document, separators=(",", ":")))
    print(f"merged {scenario} into {output}: {Path(output).stat().st_size // 1024} KB, "
          f"{len([r for r in regions if r['state'] in play])} regions, "
          f"{len([c for c in counties if c['state'] in play])} counties")


if __name__ == "__main__":
    if sys.argv[1] == "--merge":
        merge(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        main(sys.argv[1], sys.argv[2])
