#!/usr/bin/env bash
# Builds the Mac disk image: the app, a shortcut to Applications and the branded background with
# its drag arrow. CI only (macOS runner); the app is unsigned, like the zip it replaces.
#
#   apple/scripts/make-dmg.sh <path to Corporate Warfare.app> <output.dmg>
#
# The pretty window needs Finder scripting, which a headless runner sometimes refuses. When it
# does, the script still writes a plain disk image with the app and the Applications shortcut, so
# a release never ships without a Mac download.
set -euo pipefail
app="${1:?usage: make-dmg.sh <app> <output.dmg>}"
out="${2:?usage: make-dmg.sh <app> <output.dmg>}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The runner Python is managed by Homebrew, so Pillow goes into a throwaway environment.
python3 -m venv "$work/venv"
"$work/venv/bin/pip" install --quiet pillow
"$work/venv/bin/python" "$root/tools/dmg_art.py" "$work/art"
# One file with both resolutions, so the window is sharp on Retina screens.
tiffutil -cathidpicheck "$work/art/dmg-background.png" "$work/art/dmg-background@2x.png" -out "$work/art/background.tiff"

rm -f "$out"
name="Corporate Warfare"
brew list create-dmg >/dev/null 2>&1 || brew install create-dmg
if create-dmg \
    --volname "$name" \
    --background "$work/art/background.tiff" \
    --window-pos 200 120 --window-size 660 400 \
    --icon-size 96 \
    --icon "$(basename "$app")" 170 215 \
    --app-drop-link 490 215 \
    --hide-extension "$(basename "$app")" \
    --no-internet-enable \
    "$out" "$app"; then
  echo "branded disk image written to $out"
else
  echo "create-dmg failed; writing a plain disk image instead" >&2
  rm -f "$out"
  stage="$work/stage"
  mkdir -p "$stage/.background"
  cp -R "$app" "$stage/"
  ln -s /Applications "$stage/Applications"
  cp "$work/art/dmg-background.png" "$stage/.background/background.png"
  hdiutil create -volname "$name" -srcfolder "$stage" -ov -format UDZO "$out"
fi
test -s "$out"
hdiutil verify "$out"
