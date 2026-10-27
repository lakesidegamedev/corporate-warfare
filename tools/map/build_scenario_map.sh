#!/usr/bin/env bash
# Add one scenario's map layers to apple/Resources/world-map.json without rebuilding the rest.
#
# Usage: tools/map/build_scenario_map.sh <source-dir> <scenario-id>
#
# Supported scenarios: texas-1981 (Texas, Oklahoma, Louisiana) and silicon-valley-1995 (California). <source-dir> holds the unzipped
# downloads named in build_map.sh. The scenario's regions are whole counties dissolved by
# tools/map/data/county-regions-<scenario-id>.csv, which tools/map/build_regions.py writes; run
# that first when regions change. Layers land in <source-dir>/out-<scenario-id> and
# compact_map.py merges them into the shipped world map, replacing any earlier copy of the
# scenario so the step can be repeated. Great Lakes layers are not touched.
set -euo pipefail

SRC=$(cd "$1" && pwd)
SCENARIO=$2
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
MS=${MAPSHAPER:-mapshaper}

case "$SCENARIO" in
  texas-1981)
    PLAY='["TX","OK","LA"]'
    FIPS='["48","40","22"]'
    B='bbox=-107.2,25.7,-88.6,37.2'
    NEAR='this.bounds[0] < -88.6 && this.bounds[2] > -107.2 && this.bounds[1] < 37.2 && this.bounds[3] > 25.7'
    ASSIGNMENT="$ROOT/tools/map/data/county-regions-texas-1981.csv"
    REGIONS_JSON="$ROOT/content/scenarios/texas-1981/regions.json"
    ;;
  silicon-valley-1995)
    PLAY='["CA"]'
    FIPS='["06"]'
    B='bbox=-124.6,32.4,-113.9,42.1'
    NEAR='this.bounds[0] < -113.9 && this.bounds[2] > -124.6 && this.bounds[1] < 42.1 && this.bounds[3] > 32.4'
    ASSIGNMENT="$ROOT/tools/map/data/county-regions-silicon-valley-1995.csv"
    REGIONS_JSON="$ROOT/content/scenarios/silicon-valley-1995/regions.json"
    ;;
  *)
    echo "no map recipe for scenario $SCENARIO" >&2
    exit 2
    ;;
esac
OUT="$SRC/out-$SCENARIO"
mkdir -p "$OUT"
cd "$SRC"

$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) >= 0" \
  -simplify 6% keep-shapes -o format=geojson "$OUT/play_states.json"
# Neighbours are kept whole when they touch the scenario's box, so no box edge shows.
$MS cb_2018_us_state_500k/cb_2018_us_state_500k.shp -filter "$PLAY.indexOf(STUSPS) < 0 && $NEAR" \
  -simplify 6% keep-shapes -o format=geojson "$OUT/context_states.json"
$MS cb_2018_us_county_500k/cb_2018_us_county_500k.shp -filter "$FIPS.indexOf(STATEFP) >= 0" \
  -simplify 4% keep-shapes -o format=geojson "$OUT/counties.json"
$MS cb_2018_us_county_500k/cb_2018_us_county_500k.shp -filter "$FIPS.indexOf(STATEFP) >= 0" \
  -join "$ASSIGNMENT" keys=GEOID,geoid string-fields=geoid \
  -dissolve region -simplify 5% keep-shapes -o format=geojson "$OUT/regions.json"
$MS ne_10m_rivers_lake_centerlines/ne_10m_rivers_lake_centerlines.shp -clip $B -filter 'scalerank <= 7' \
  -simplify 20% -o format=geojson "$OUT/rivers.json"
$MS tl_2018_us_primaryroads/tl_2018_us_primaryroads.shp -filter 'RTTYP=="I"' -clip "$OUT/play_states.json" -dissolve FULLNAME \
  -simplify 3% -o format=geojson "$OUT/interstates.json"
$MS "$OUT/interstates.json" -simplify 25% -o format=geojson "$OUT/interstates_thin.json"
$MS ne_10m_populated_places/ne_10m_populated_places.shp -clip $B \
  -filter 'POP_MAX >= 150000 || FEATURECLA.indexOf("Admin-1 capital") >= 0' \
  -o format=geojson "$OUT/cities.json"

# Region hubs: the anchor city of each region.
python3 - "$REGIONS_JSON" "$OUT/hubs.geojson" <<'EOF'
import json, sys
regions = json.load(open(sys.argv[1]))["regions"]
features = [{"type": "Feature", "properties": {"code": r["code"]},
             "geometry": {"type": "Point", "coordinates": [r["lon_milli"] / 1000, r["lat_milli"] / 1000]}}
            for r in regions]
json.dump({"type": "FeatureCollection", "features": features}, open(sys.argv[2], "w"))
EOF
$MS "$OUT/hubs.geojson" -o format=geojson "$OUT/hubs.json"

python3 "$ROOT/tools/map/compact_map.py" --merge "$SCENARIO" "$OUT" "$ROOT/apple/Resources/world-map.json"
