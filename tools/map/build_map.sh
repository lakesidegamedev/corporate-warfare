#!/usr/bin/env bash
# Build apple/Resources/corridor-map.json from public sources.
#
# Usage: tools/map/build_map.sh <source-dir>
#
# <source-dir> holds the unzipped downloads:
#   cb_2018_us_state_500k, cb_2018_us_county_500k   https://www2.census.gov/geo/tiger/GENZ2018/shp/
#   tl_2018_us_primaryroads                         https://www2.census.gov/geo/tiger/TIGER2018/PRIMARYROADS/
#   ne_10m_admin_1_states_provinces, ne_10m_lakes,
#   ne_10m_rivers_lake_centerlines,
#   ne_10m_populated_places                         https://www.naturalearthdata.com/ (10m)
# All are public domain. Requires mapshaper (npm i -g mapshaper) and python3.
#
# Region shapes are whole counties dissolved by tools/map/data/county-regions.csv, which
# tools/map/build_regions.py writes; run that first when regions change.
set -euo pipefail

SRC=$(cd "$1" && pwd)
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$SRC/out"
MS=${MAPSHAPER:-mapshaper}
P='+proj=aea +lat_1=38 +lat_2=44 +lat_0=41 +lon_0=-82 +datum=WGS84'
B='bbox=-93.5,35.5,-70.5,47.5'
PLAY='["IL","IN","KY","MI","NY","OH","PA","WV"]'
FIPS='["17","18","21","26","36","39","42","54"]'
mkdir -p "$OUT"
cd "$SRC"

$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) >= 0" \
  -simplify 6% keep-shapes -proj "$P" -o format=geojson "$OUT/play_states.json"
$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) < 0" -clip $B \
  -simplify 6% keep-shapes -proj "$P" -o format=geojson "$OUT/context_states.json"
$MS ne_10m_admin_1_states_provinces/ne_10m_admin_1_states_provinces.shp -filter 'adm0_a3=="CAN"' \
  -clip $B -simplify 50% keep-shapes -proj "$P" -o format=geojson "$OUT/canada.json"
$MS cb_2018_us_county_500k/cb_2018_us_county_500k.shp -filter "$FIPS.indexOf(STATEFP) >= 0" \
  -simplify 4% keep-shapes -proj "$P" -o format=geojson "$OUT/counties.json"
# Regions: dissolve whole counties before simplifying, so neighbouring regions share edges.
$MS cb_2018_us_county_500k/cb_2018_us_county_500k.shp -filter "$FIPS.indexOf(STATEFP) >= 0" \
  -join "$ROOT/tools/map/data/county-regions.csv" keys=GEOID,geoid string-fields=geoid \
  -dissolve region -simplify 5% keep-shapes -proj "$P" -o format=geojson "$OUT/regions.json"
$MS ne_10m_lakes/ne_10m_lakes.shp -clip $B -filter 'scalerank <= 5' -simplify 15% keep-shapes \
  -proj "$P" -o format=geojson "$OUT/lakes.json"
$MS ne_10m_rivers_lake_centerlines/ne_10m_rivers_lake_centerlines.shp -clip $B -filter 'scalerank <= 7' \
  -simplify 20% -proj "$P" -o format=geojson "$OUT/rivers.json"
$MS tl_2018_us_primaryroads/tl_2018_us_primaryroads.shp -filter 'RTTYP=="I"' -clip $B -dissolve FULLNAME \
  -simplify 3% -proj "$P" -o format=geojson "$OUT/interstates.json"
$MS "$OUT/interstates.json" -simplify 25% -o format=geojson "$OUT/interstates_thin.json"
$MS ne_10m_populated_places/ne_10m_populated_places.shp -clip $B \
  -filter 'POP_MAX >= 200000 || FEATURECLA.indexOf("Admin-1 capital") >= 0' \
  -proj "$P" -o format=geojson "$OUT/cities.json"

# Region hubs: the anchor city of each region in content/geography/regions.json.
python3 - "$ROOT/content/geography/regions.json" "$OUT/hubs.geojson" <<'EOF'
import json, sys
regions = json.load(open(sys.argv[1]))["regions"]
features = [{"type": "Feature", "properties": {"code": r["code"]},
             "geometry": {"type": "Point", "coordinates": [r["lon_milli"] / 1000, r["lat_milli"] / 1000]}}
            for r in regions]
json.dump({"type": "FeatureCollection", "features": features}, open(sys.argv[2], "w"))
EOF
$MS "$OUT/hubs.geojson" -proj "$P" -o format=geojson "$OUT/hubs.json"

python3 "$ROOT/tools/map/compact_map.py" "$OUT" "$ROOT/apple/Resources/corridor-map.json"
