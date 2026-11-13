#!/usr/bin/env bash
# Runs inside the CI emulator (play-screenshots.yml): sizes the screen, installs the debug build, runs
# the screenshot test and pulls the pictures. Usage: play-screenshots.sh <name> <WxH> <density>
set -uo pipefail
cd "$(dirname "$0")/.."
NAME=$1; SIZE=$2; DENSITY=$3
PKG=net.lakesidegames.corporatewarfare
OUT="$PWD/build/play-screenshots/$NAME"
mkdir -p "$OUT"
exec > >(tee "$OUT/run.txt") 2>&1
timeout 30 adb shell wm size "$SIZE"
timeout 30 adb shell wm density "$DENSITY"
timeout 30 adb shell wm size; timeout 30 adb shell wm density
timeout 180 adb install -r androidApp/build/outputs/apk/debug/androidApp-debug.apk || exit 1
timeout 180 adb install -r -t androidApp/build/outputs/apk/androidTest/debug/*.apk || exit 1
timeout 900 adb shell am instrument -w -r -e screenshots true -e class "$PKG.StoreScreenshotsTest" "$PKG.test/androidx.test.runner.AndroidJUnitRunner" | tee "$OUT/instrument.txt"
timeout 60 adb logcat -d -v threadtime > "$OUT/logcat.txt"
timeout 120 adb pull "/sdcard/Android/data/$PKG/files/shots/." "$OUT/" || { echo "::error::no pictures to pull"; exit 1; }
grep -q "INSTRUMENTATION_CODE: -1" "$OUT/instrument.txt" || { echo "::error::screenshot test did not pass"; exit 1; }
