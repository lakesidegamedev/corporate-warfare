#!/usr/bin/env bash
# Verify a signed archive before upload. Fails closed on any mismatch.
set -euo pipefail
archive="${1:?usage: verify-archive.sh path/to/App.xcarchive}"
app="$archive/Products/Applications/CorporateWarfare.app"
expected_bundle="net.lakesidegames.corporatewarfare"
[[ -d "$app" ]] || { echo "missing app bundle at $app" >&2; exit 1; }

bundle=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$app/Info.plist")
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Info.plist")
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$app/Info.plist")
[[ "$bundle" == "$expected_bundle" ]] || { echo "bundle mismatch: $bundle" >&2; exit 1; }
[[ -n "$version" && -n "$build" ]] || { echo "missing version/build" >&2; exit 1; }

lipo -info "$app/CorporateWarfare" | grep -q arm64 || { echo "device binary is not arm64" >&2; exit 1; }
[[ -f "$app/PrivacyInfo.xcprivacy" ]] || { echo "privacy manifest missing" >&2; exit 1; }
[[ -f "$app/AppIcon60x60@2x.png" || -f "$app/AppIcon60x60@3x.png" ]] || { echo "app icon missing" >&2; exit 1; }

codesign --verify --deep --strict "$app"
codesign -dv --verbose=4 "$app" 2>&1 | grep -q "Authority=Apple Distribution" || {
  echo "archive is not signed with an Apple Distribution identity" >&2
  exit 1
}
echo "archive_ok bundle=$bundle version=$version build=$build"
