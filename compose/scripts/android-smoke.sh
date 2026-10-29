#!/usr/bin/env bash
# Runs inside the CI emulator (android.yml): installs the shrunk release build, launches it, and
# fails on a dead process or a crash signature in logcat; then runs the instrumented test that
# plays a career against the same shrunk build. Logcat lands in $OUT for the artifact.
set -uo pipefail
cd "$(dirname "$0")/.."
PKG=net.lakesidegames.corporatewarfare
OUT="${SMOKE_OUT:-$PWD/../build/smoke}"
mkdir -p "$OUT"
BAD='FATAL EXCEPTION|UnsatisfiedLinkError|ClassNotFoundException|NoSuchMethodError|NoClassDefFoundError'
BINDINGS='(jna|uniffi)'
fail=0

check_log() { # $1 logcat file, $2 pid or empty
  if grep -E "FATAL EXCEPTION" "$1" >/dev/null; then
    echo "::error::crash in $1"; grep -E -A14 "FATAL EXCEPTION" "$1" | head -60; fail=1
  fi
  if [ -n "$2" ]; then
    if awk -v pid="$2" '$3==pid' "$1" | grep -E "$BAD" >/dev/null; then
      echo "::error::linkage error in the app process"; awk -v pid="$2" '$3==pid' "$1" | grep -E "$BAD" | head -20; fail=1
    fi
    if awk -v pid="$2" '$3==pid && ($5=="E" || $5=="F")' "$1" | grep -i -E "$BINDINGS" >/dev/null; then
      echo "::error::JNA or UniFFI error in the app process"; awk -v pid="$2" '$3==pid && ($5=="E" || $5=="F")' "$1" | grep -i -E "$BINDINGS" | head -20; fail=1
    fi
  fi
}

APK=$(ls androidApp/build/outputs/apk/release/*.apk | head -1)
echo "installing $APK"
adb logcat -c
adb install -r "$APK" || exit 1
adb shell am start -W -n "$PKG/.MainActivity" || fail=1
sleep 3
PID=$(adb shell pidof "$PKG" | tr -d '\r' | awk '{print $1}')
[ -n "$PID" ] || { echo "::error::app is not running after launch"; fail=1; }
for i in $(seq 1 30); do
  sleep 2
  NOW=$(adb shell pidof "$PKG" | tr -d '\r' | awk '{print $1}')
  if [ "$NOW" != "$PID" ]; then echo "::error::app process died or restarted (was '$PID', now '$NOW') after $((i * 2 + 3)) s"; fail=1; break; fi
done
adb logcat -d -v threadtime > "$OUT/logcat-launch.txt"
adb exec-out screencap -p > "$OUT/launch.png" || true
check_log "$OUT/logcat-launch.txt" "$PID"

echo "instrumented test against the shrunk build"
adb logcat -c
./gradlew --no-daemon :androidApp:connectedReleaseAndroidTest -Pcw.smoke=true || { echo "::error::instrumented test failed"; fail=1; }
adb logcat -d -v threadtime > "$OUT/logcat-test.txt"
check_log "$OUT/logcat-test.txt" ""
exit $fail
