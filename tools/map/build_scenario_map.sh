#!/usr/bin/env bash
# Add one scenario's map layers to apple/Resources/world-map.json without rebuilding the rest.
#
# Usage: tools/map/build_scenario_map.sh <source-dir> <scenario-id>
#
# Supported scenarios: texas-1981 (Texas, Oklahoma, Louisiana), silicon-valley-1995 (California)
# and ruhr-rhine-1991 (Germany, the Netherlands, Belgium, Luxembourg, Alsace and Lorraine).
# <source-dir> holds the unzipped downloads named in build_map.sh. The US scenarios' regions are
# whole counties dissolved by tools/map/data/county-regions-<scenario-id>.csv, which
# tools/map/build_regions.py writes; run that first when regions change.
#
# Ruhr and Rhine 1991 additionally needs, in <source-dir>: Eurostat GISCO NUTS 2016 10M GeoJSON
# NUTS_RG_10M_2016_4326_LEVL_3.geojson (https://gisco-services.ec.europa.eu/distribution/v2/nuts/,
# (C) European Union), plus Natural Earth ne_10m_rivers_lake_centerlines, ne_10m_populated_places and
# ne_10m_roads. Its regions are whole NUTS 3 areas dissolved by
# tools/map/data/nuts3-regions-ruhr-rhine-1991.csv (build_regions.py writes it); its play "states"
# are the five countries, each the union of their regions. Tool used: mapshaper 0.7.x.
# Layers land in <source-dir>/out-<scenario-id> and
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
  ruhr-rhine-1991)
    EUROPE=1
    B='bbox=2.5,47.2,15.2,55.1'
    NUTS=NUTS_RG_10M_2016_4326_LEVL_3.geojson
    ASSIGNMENT="$ROOT/tools/map/data/nuts3-regions-ruhr-rhine-1991.csv"
    REGIONS_JSON="$ROOT/content/scenarios/ruhr-rhine-1991/regions.json"
    ;;
  *)
    echo "no map recipe for scenario $SCENARIO" >&2
    exit 2
    ;;
esac
OUT="$SRC/out-$SCENARIO"
mkdir -p "$OUT"
cd "$SRC"

if [ -n "${EUROPE:-}" ]; then
  JOIN="-join $ASSIGNMENT keys=NUTS_ID,nuts_id string-fields=nuts_id -filter !!region"
  $MS $NUTS $JOIN -dissolve region -simplify 40% keep-shapes -o format=geojson "$OUT/regions.json"
  $MS $NUTS $JOIN -each 'STUSPS=CNTR_CODE, NAME=NAME_ENGL' -dissolve STUSPS copy-fields=NAME \
    -simplify 40% keep-shapes -o format=geojson "$OUT/play_states.json"
  # The countries layer already draws the neighbours, so no context states are added.
  echo '{"type":"FeatureCollection","features":[]}' > "$OUT/context_states.json"
  $MS $NUTS $JOIN -each 'GEOID=NUTS_ID, STATEFP=CNTR_CODE, NAME=NAME_LATN' -filter-fields GEOID,STATEFP,NAME \
    -simplify 40% keep-shapes -o format=geojson "$OUT/counties.json"
  $MS ne_10m_rivers_lake_centerlines/ne_10m_rivers_lake_centerlines.shp -clip $B -filter 'scalerank <= 8' \
    -simplify 20% -o format=geojson "$OUT/rivers.json"
  $MS ne_10m_roads/ne_10m_roads.shp -clip "$OUT/play_states.json" -filter "type=='Major Highway'" \
    -each 'FULLNAME=name || ""' -simplify 25% -o format=geojson "$OUT/interstates_thin.json"
  $MS ne_10m_populated_places/ne_10m_populated_places.shp -clip $B \
    -filter 'POP_MAX >= 150000 || FEATURECLA.indexOf("Admin-1 capital") >= 0' \
    -o format=geojson "$OUT/cities.json"
else
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
fi

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
