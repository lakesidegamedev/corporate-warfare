#!/usr/bin/env bash
# Wraps the release app image (the same folder the .deb is made from) in an AppImage: one file
# that runs on most Linux desktops with no install.
#
#   compose/shared/packaging/linux/build-appimage.sh <app image folder> <output.AppImage>
#
# The app image folder is what `:shared:createReleaseDistributable` writes, for example
# compose/shared/build/compose/binaries/main-release/app/<name>. CI runs this on the
# Linux runner after the .deb is built.
set -euo pipefail
image="${1:?usage: build-appimage.sh <app image folder> <output.AppImage>}"
out="${2:?usage: build-appimage.sh <app image folder> <output.AppImage>}"
here="$(cd "$(dirname "$0")" && pwd)"
icon="$here/../../icons/corporate-warfare.png"
echo "app image: $image"; ls -la "$image" "$image/bin" || true
# The launcher is the one file in bin/, named after the app ("Corporate Warfare").
launcher="$(ls "$image/bin" | head -1)"
test -n "$launcher" && test -x "$image/bin/$launcher" || { echo "no launcher in $image/bin" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
appdir="$work/CorporateWarfare.AppDir"
mkdir -p "$appdir/usr"
cp -a "$image/." "$appdir/usr/"
cp "$here/corporate-warfare.desktop" "$appdir/corporate-warfare.desktop"
cp "$icon" "$appdir/corporate-warfare.png"
cp "$icon" "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<RUN
#!/bin/sh
here="\$(dirname "\$(readlink -f "\$0")")"
exec "\$here/usr/bin/$launcher" "\$@"
RUN
chmod +x "$appdir/AppRun"

tool="$work/appimagetool"
curl -fsSL -o "$tool" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod +x "$tool"
rm -f "$out"
# Runners have no FUSE, so the tool unpacks itself and runs from there.
ARCH=x86_64 "$tool" --appimage-extract-and-run --no-appstream "$appdir" "$out"
chmod +x "$out"
test -s "$out"
