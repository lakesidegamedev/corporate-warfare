#!/usr/bin/env bash
# Verify the exported IPA before upload. Fails closed on any mismatch.
# The exported .app must carry the Apple Distribution identity: distribution
# signing happens at export time, not at archive time.
set -euo pipefail
ipa="${1:?usage: verify-ipa.sh path/to/App.ipa}"
[[ -f "$ipa" ]] || { echo "missing ipa at $ipa" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
ditto -x -k "$ipa" "$work/unzipped"
app=$(find "$work/unzipped/Payload" -maxdepth 1 -name '*.app' -type d | head -1)
[[ -n "$app" && -d "$app" ]] || { echo "no .app bundle in IPA Payload" >&2; exit 1; }
expected_bundle="net.lakesidegames.corporatewarfare"
bundle=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Info.plist")
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Info.plist")
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$app/Info.plist")
[[ "$bundle" == "$expected_bundle" ]] || { echo "bundle mismatch: $bundle" >&2; exit 1; }
[[ -n "$version" && -n "$build" ]] || { echo "missing version/build" >&2; exit 1; }
lipo -info "$app/CorporateWarfare" | grep -q arm64 || { echo "device binary is not arm64" >&2; exit 1; }
[[ -f "$app/PrivacyInfo.xcprivacy" ]] || { echo "privacy manifest missing" >&2; exit 1; }
[[ -f "$app/Assets.car" ]] || { echo "asset catalog missing" >&2; exit 1; }
xcrun assetutil --info "$app/Assets.car" | grep -q 'AppIcon' || {
  echo "compiled app icon missing from Assets.car" >&2
  exit 1
}
# App Store validation rejects loose libraries in Frameworks/ (only Swift
# runtime libraries and .framework bundles may live there).
if [[ -d "$app/Frameworks" ]] && find "$app/Frameworks" -maxdepth 1 -name '*.a' | grep -q .; then
  echo "loose static library found in Frameworks/" >&2
  exit 1
fi
codesign --verify --deep --strict "$app"
codesign -dv --verbose=4 "$app" 2>&1 | grep -q "Authority=Apple Distribution" || {
  echo "exported app is not signed with an Apple Distribution identity" >&2
  exit 1
}
echo "ipa_ok bundle=$bundle version=$version build=$build"
