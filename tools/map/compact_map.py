#!/usr/bin/env python3
"""Compact projected GeoJSON layers into apple/Resources/corridor-map.json.

Input: the mapshaper outputs described in tools/map/README.md (Albers conic, metres). Output: a
10000-unit-wide view box with integer coordinates, flat [x0, y0, x1, y1, ...] rings and lines,
and per-layer points thinned to a minimum spacing. Sources are US Census cartographic boundary
and TIGER files and Natural Earth, all public domain.
"""
import json
import sys
from pathlib import Path

PLAY = ["IL", "IN", "KY", "MI", "NY", "OH", "PA", "WV"]
FIPS = {"17": "IL", "18": "IN", "21": "KY", "26": "MI", "36": "NY", "39": "OH", "42": "PA", "54": "WV"}
WIDTH = 10_000


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
              ["play_states", "context_states", "canada", "counties", "lakes", "rivers",
               "interstates_thin", "cities", "hubs"]}
    xs, ys = [], []
    for feature in layers["play_states"] + layers["context_states"] + layers["canada"]:
        for polygon in geometries(feature):
            for ring in polygon:
                for x, y in ring:
                    xs.append(x)
                    ys.append(y)
    # Frame on the playable states with a margin, so context land fills the edges.
    play_x, play_y = [], []
    for feature in layers["play_states"]:
        for polygon in geometries(feature):
            for ring in polygon:
                for x, y in ring:
                    play_x.append(x)
                    play_y.append(y)
    margin = 0.06 * (max(play_x) - min(play_x))
    left, right = min(play_x) - margin, max(play_x) + margin
    bottom, top = min(play_y) - margin, max(play_y) + margin
    scale = WIDTH / (right - left)
    height = round((top - bottom) * scale)

    def point(x, y):
        return round((x - left) * scale), round((top - y) * scale)

    def flat(sequence, spacing, closed):
        out = []
        last = None
        for x, y in sequence:
            px, py = point(x, y)
            if last is not None and abs(px - last[0]) + abs(py - last[1]) < spacing:
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
        if not (0 <= px <= WIDTH and 0 <= py <= height):
            continue
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
        "schema": 1,
        "attribution": "Boundaries and roads: US Census Bureau cartographic boundary and TIGER/Line files (2018). Lakes, rivers, places and provinces: Natural Earth. All public domain.",
        "projection": "Albers equal-area conic, standard parallels 38N and 44N, centre 41N 82W",
        "viewbox": [WIDTH, height],
        "states": states,
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
    print(f"wrote {output}: {size // 1024} KB, {len(states)} states/provinces, {len(counties)} counties, "
          f"{len(lakes)} lakes, {len(rivers)} rivers, {len(interstates)} interstates, {len(cities)} cities")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
