#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Select an available iPhone on the runner rather than guessing an installed model.
simulator_id=$(xcrun simctl list devices available -j | python3 -c 'import json,sys; d=json.load(sys.stdin); ids=[x["udid"] for runtime,rows in d["devices"].items() if "iOS" in runtime for x in rows if x.get("isAvailable") and x["name"].startswith("iPhone")]; print(ids[0] if ids else "")')
if [[ -z "$simulator_id" ]]; then
  echo "No available iPhone simulator on this runner." >&2
  exit 1
fi
mkdir -p build
xcodebuild test -project CorporateWarfare.xcodeproj -scheme CorporateWarfare \
  -destination "platform=iOS Simulator,id=$simulator_id" \
  -resultBundlePath build/EngineBridge.xcresult CODE_SIGNING_ALLOWED=NO
