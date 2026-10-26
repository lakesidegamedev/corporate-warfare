#!/usr/bin/env bash
# Build the Rust engine for the Compose apps and generate its Kotlin bindings.
#
#   compose/scripts/build-engine.sh android   arm64-v8a and x86_64 libraries for the APK
#   compose/scripts/build-engine.sh desktop   the host library for the desktop app
#
# Outputs go to compose/build/engine (never committed): jniLibs/<abi>/libcw_ffi.so for Android,
# desktop/<jna-platform>/ for the desktop, and kotlin/ for the bindings.
set -euo pipefail
cd "$(dirname "$0")/../.."
OUT=compose/build/engine
mkdir -p "$OUT"

android() {
  local ndk="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
  if [[ -z "$ndk" ]]; then
    local sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
    ndk="$(ls -d "$sdk"/ndk/* 2>/dev/null | sort -V | tail -1)"
  fi
  [[ -d "$ndk" ]] || { echo "Android NDK not found; set ANDROID_NDK_HOME" >&2; exit 2; }
  local host
  host="$(ls "$ndk/toolchains/llvm/prebuilt" | head -1)"
  local bin="$ndk/toolchains/llvm/prebuilt/$host/bin"
  # API 26 is the app's minSdk.
  for pair in "aarch64-linux-android:arm64-v8a" "x86_64-linux-android:x86_64"; do
    local triple="${pair%%:*}" abi="${pair##*:}"
    local env_triple="${triple//-/_}"
    local upper
    upper="$(echo "$env_triple" | tr '[:lower:]' '[:upper:]')"
    rustup target add "$triple" >/dev/null
    export "CARGO_TARGET_${upper}_LINKER=$bin/${triple}26-clang"
    export "CC_${env_triple}=$bin/${triple}26-clang"
    export "AR_${env_triple}=$bin/llvm-ar"
    cargo build --locked -p cw-ffi --release --target "$triple"
    mkdir -p "$OUT/jniLibs/$abi"
    cp "target/$triple/release/libcw_ffi.so" "$OUT/jniLibs/$abi/"
  done
  bindings "target/aarch64-linux-android/release/libcw_ffi.so"
}

desktop() {
  cargo build --locked -p cw-ffi --release
  local lib platform
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) lib=libcw_ffi.so; platform=linux-x86-64 ;;
    Linux-aarch64) lib=libcw_ffi.so; platform=linux-aarch64 ;;
    Darwin-arm64) lib=libcw_ffi.dylib; platform=darwin-aarch64 ;;
    Darwin-x86_64) lib=libcw_ffi.dylib; platform=darwin-x86-64 ;;
    MINGW*|MSYS*|CYGWIN*) lib=cw_ffi.dll; platform=win32-x86-64 ;;
    *) echo "unsupported host $(uname -s)-$(uname -m)" >&2; exit 2 ;;
  esac
  # JNA finds a library on the classpath under its platform folder.
  mkdir -p "$OUT/desktop/$platform"
  cp "target/release/$lib" "$OUT/desktop/$platform/"
  bindings "target/release/$lib"
}

bindings() {
  rm -rf "$OUT/kotlin"
  cargo run --locked -p cw-bindgen -- generate "$1" --language kotlin --no-format --out-dir "$OUT/kotlin"
}

case "${1:-android}" in
  android) android ;;
  desktop) desktop ;;
  *) echo "usage: $0 [android|desktop]" >&2; exit 2 ;;
esac
