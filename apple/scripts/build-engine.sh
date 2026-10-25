#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
if [[ "$(uname -s)" != Darwin ]]; then
  echo "Apple engine packaging requires macOS and Xcode." >&2
  exit 2
fi
export IPHONEOS_DEPLOYMENT_TARGET=17.0
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo build --locked -p cw-ffi --release --target aarch64-apple-ios
cargo build --locked -p cw-ffi --release --target aarch64-apple-ios-sim
mkdir -p apple/Generated
cargo run --locked -p cw-bindgen -- generate target/aarch64-apple-ios/release/libcw_ffi.a --language swift --out-dir apple/Generated
mkdir -p apple/Generated/Headers
cp apple/Generated/cw_ffiFFI.h apple/Generated/Headers/
cp apple/Generated/cw_ffiFFI.modulemap apple/Generated/Headers/module.modulemap
# Regenerate the derived bundle so removed architectures cannot survive from a prior build.
rm -rf apple/Generated/CwEngine.xcframework
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libcw_ffi.a -headers apple/Generated/Headers \
  -library target/aarch64-apple-ios-sim/release/libcw_ffi.a -headers apple/Generated/Headers \
  -output apple/Generated/CwEngine.xcframework
