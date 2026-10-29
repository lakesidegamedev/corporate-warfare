#!/usr/bin/env bash
# Runs inside the CI emulator (android.yml): installs the shrunk release build, launches it, and
# fails on a dead process or a crash signature in logcat; then runs the instrumented test that
# plays a career against the same shrunk build. Logcat lands in $OUT for the artifact.
set -uo pipefail
# Every step is bounded: a hung emulator must fail the job with logs, not run to the job timeout.
stamp() { echo "[$(date +%H:%M:%S)] $*"; }
cd "$(dirname "$0")/.."
PKG=net.lakesidegames.corporatewarfare
OUT="${SMOKE_OUT:-$PWD/build/smoke}"
mkdir -p "$OUT"
exec > >(tee "$OUT/run.txt") 2>&1
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
timeout 30 adb logcat -c
stamp install; timeout 180 adb install -r "$APK" || exit 1
stamp launch; timeout 90 adb shell am start -W -n "$PKG/.MainActivity" || { echo "::error::launch did not complete"; fail=1; }
sleep 3
stamp launched; PID=$(timeout 20 adb shell pidof "$PKG" | tr -d '\r' | awk '{print $1}')
[ -n "$PID" ] || { echo "::error::app is not running after launch"; fail=1; }
for i in $(seq 1 30); do
  sleep 2
  NOW=$(timeout 20 adb shell pidof "$PKG" | tr -d '\r' | awk '{print $1}')
  if [ "$NOW" != "$PID" ]; then echo "::error::app process died or restarted (was '$PID', now '$NOW') after $((i * 2 + 3)) s"; fail=1; break; fi
done
timeout 60 adb logcat -d -v threadtime > "$OUT/logcat-launch.txt"
timeout 30 adb exec-out screencap -p > "$OUT/launch.png" || true
check_log "$OUT/logcat-launch.txt" "$PID"

echo "instrumented test against the shrunk build"
adb logcat -c
stamp "instrumented run"
TESTAPK=$(ls androidApp/build/outputs/apk/androidTest/release/*.apk | head -1)
timeout 180 adb install -r -t "$TESTAPK" || { echo "::error::test APK did not install"; fail=1; }
# Driven with adb, not Gradle: Gradle waits forever when the instrumentation process dies on start.
timeout 600 adb shell am instrument -w -r "$PKG.test/androidx.test.runner.AndroidJUnitRunner" | tee "$OUT/instrument.txt"
R="$OUT/instrument.txt"
if grep -q "INSTRUMENTATION_STATUS_CODE: 0" "$R" && grep -q "INSTRUMENTATION_CODE: -1" "$R" \
  && ! grep -q "INSTRUMENTATION_STATUS_CODE: -[12]" "$R" && ! grep -q "Process crashed" "$R"; then
  echo "instrumented test passed"
else
  echo "::error::instrumented test failed"; fail=1
fi
timeout 60 adb logcat -d -v threadtime > "$OUT/logcat-test.txt"
check_log "$OUT/logcat-test.txt" ""
exit $fail
