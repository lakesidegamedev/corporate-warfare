#!/usr/bin/env bash
# Build apple/Resources/world-map.json from public sources.
#
# Usage: tools/map/build_map.sh <source-dir>
#
# <source-dir> holds the unzipped downloads:
#   cb_2018_us_state_500k, cb_2018_us_county_500k   https://www2.census.gov/geo/tiger/GENZ2018/shp/
#   tl_2018_us_primaryroads                         https://www2.census.gov/geo/tiger/TIGER2018/PRIMARYROADS/
#   ne_10m_admin_0_countries.geojson, ne_10m_admin_1_states_provinces, ne_10m_lakes,
#   ne_10m_rivers_lake_centerlines,
#   ne_10m_populated_places, ne_10m_roads           https://www.naturalearthdata.com/ (10m)
# All are public domain. Requires mapshaper (npm i -g mapshaper) and python3.
#
# Geometry stays in longitude and latitude: the apps project it at draw time, as a globe when
# zoomed out and a flat map when zoomed in.
#
# Region shapes are whole counties dissolved by tools/map/data/county-regions.csv, which
# tools/map/build_regions.py writes; run that first when regions change.
set -euo pipefail

SRC=$(cd "$1" && pwd)
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT="$SRC/out"
MS=${MAPSHAPER:-mapshaper}
B='bbox=-93.5,35.5,-70.5,47.5'
NEAR='this.bounds[0] < -70.5 && this.bounds[2] > -93.5 && this.bounds[1] < 47.5 && this.bounds[3] > 35.5'
PLAY='["IL","IN","KY","MI","NY","OH","PA","WV"]'
FIPS='["17","18","21","26","36","39","42","54"]'
mkdir -p "$OUT"
cd "$SRC"

$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) >= 0" \
  -simplify 6% keep-shapes -o format=geojson "$OUT/play_states.json"
# Neighbours are kept whole (not clipped) when they touch the scenario's box, so no box edge shows.
$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) < 0 && $NEAR" \
  -simplify 6% keep-shapes -o format=geojson "$OUT/context_states.json"
$MS ne_10m_admin_1_states_provinces/ne_10m_admin_1_states_provinces.shp -filter "adm0_a3=='CAN' && $NEAR" \
  -simplify 50% keep-shapes -o format=geojson "$OUT/canada.json"
$MS cb_2018_us_county_500k/cb_2018_us_county_500k.shp -filter "$FIPS.indexOf(STATEFP) >= 0" \
  -simplify 4% keep-shapes -o format=geojson "$OUT/counties.json"
# Regions: dissolve whole counties before simplifying, so neighbouring regions share edges.
$MS cb_2018_us_county_500k/cb_2018_us_county_500k.shp -filter "$FIPS.indexOf(STATEFP) >= 0" \
  -join "$ROOT/tools/map/data/county-regions.csv" keys=GEOID,geoid string-fields=geoid \
  -dissolve region -simplify 5% keep-shapes -o format=geojson "$OUT/regions.json"
$MS ne_10m_lakes/ne_10m_lakes.shp -filter 'scalerank <= 5' -simplify 15% keep-shapes \
  -o format=geojson "$OUT/lakes.json"
$MS ne_10m_rivers_lake_centerlines/ne_10m_rivers_lake_centerlines.shp -clip $B -filter 'scalerank <= 7' \
  -simplify 20% -o format=geojson "$OUT/rivers.json"
$MS tl_2018_us_primaryroads/tl_2018_us_primaryroads.shp -filter 'RTTYP=="I"' -clip "$OUT/play_states.json" -dissolve FULLNAME \
  -simplify 3% -o format=geojson "$OUT/interstates.json"
$MS "$OUT/interstates.json" -simplify 25% -o format=geojson "$OUT/interstates_thin.json"
$MS ne_10m_populated_places/ne_10m_populated_places.shp -clip $B \
  -filter 'POP_MAX >= 200000 || FEATURECLA.indexOf("Admin-1 capital") >= 0' \
  -o format=geojson "$OUT/cities.json"

# Every country, for the globe and the context around a scenario.
$MS ne_10m_admin_0_countries.geojson -filter-fields NAME,ADM0_A3 -simplify 4% keep-shapes \
  -filter-slivers -o format=geojson "$OUT/countries.json"

# Every state and province border and the main highways, drawn grey outside the scenario.
$MS ne_10m_admin_1_states_provinces/ne_10m_admin_1_states_provinces.shp -simplify 3% keep-shapes \
  -innerlines -o format=geojson "$OUT/provinces.json"
$MS ne_10m_roads/ne_10m_roads.shp -filter "(type=='Major Highway' || type=='Secondary Highway') && scalerank <= 6" \
  -filter-fields type -simplify 8% -o format=geojson "$OUT/world_roads.json"

# Scenario areas for the menu globe: each scenario's states or country as one outline.
$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) >= 0" -dissolve \
  -simplify 4% keep-shapes -each 'id="great-lakes-2000"' -o format=geojson "$OUT/scenario_great_lakes.json"
$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter '["TX","OK","LA"].indexOf(STUSPS) >= 0' -dissolve \
  -simplify 4% keep-shapes -each 'id="texas-1981"' -o format=geojson "$OUT/scenario_texas.json"
$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter 'STUSPS == "CA"' -dissolve \
  -simplify 4% keep-shapes -each 'id="silicon-valley-1995"' -o format=geojson "$OUT/scenario_california.json"
$MS ne_10m_admin_0_countries.geojson -filter 'ADM0_A3 == "DEU"' -simplify 4% keep-shapes \
  -each 'id="ruhr-rhine-1991"' -o format=geojson "$OUT/scenario_germany.json"

# Region hubs: the anchor city of each region in content/geography/regions.json.
python3 - "$ROOT/content/geography/regions.json" "$OUT/hubs.geojson" <<'EOF'
import json, sys
regions = json.load(open(sys.argv[1]))["regions"]
features = [{"type": "Feature", "properties": {"code": r["code"]},
             "geometry": {"type": "Point", "coordinates": [r["lon_milli"] / 1000, r["lat_milli"] / 1000]}}
            for r in regions]
json.dump({"type": "FeatureCollection", "features": features}, open(sys.argv[2], "w"))
EOF
$MS "$OUT/hubs.geojson" -o format=geojson "$OUT/hubs.json"

python3 "$ROOT/tools/map/compact_map.py" "$OUT" "$ROOT/apple/Resources/world-map.json"
