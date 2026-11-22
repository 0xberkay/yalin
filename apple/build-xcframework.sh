#!/usr/bin/env bash
# Builds apple/YalinFFI.xcframework (static libyalin_ffi.a + C module) for
# iOS devices, the iOS simulator and macOS. Requires macOS with Xcode and rustup.
#
#   apple/build-xcframework.sh            # release build
#   PROFILE=profiling apple/build-xcframework.sh   # keeps symbols for Instruments
#
# Afterwards `swift build` / `swift test` at the repository root, or add the
# repository to an Xcode project as a local package.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$PWD
PROFILE=${PROFILE:-release}
OUT=$ROOT/apple/YalinFFI.xcframework
WORK=$ROOT/target/xcframework

export IPHONEOS_DEPLOYMENT_TARGET=${IPHONEOS_DEPLOYMENT_TARGET:-13.0}
export MACOSX_DEPLOYMENT_TARGET=${MACOSX_DEPLOYMENT_TARGET:-10.15}

TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin)
rustup target add "${TARGETS[@]}"
for target in "${TARGETS[@]}"; do
  cargo build --locked -p yalin-ffi --profile "$PROFILE" --target "$target"
done

lib() { echo "$ROOT/target/$1/$PROFILE/libyalin_ffi.a"; }

rm -rf "$WORK" "$OUT"
mkdir -p "$WORK/ios" "$WORK/ios-sim" "$WORK/macos"
cp "$(lib aarch64-apple-ios)" "$WORK/ios/libyalin_ffi.a"
lipo -create "$(lib aarch64-apple-ios-sim)" "$(lib x86_64-apple-ios)" -output "$WORK/ios-sim/libyalin_ffi.a"
lipo -create "$(lib aarch64-apple-darwin)" "$(lib x86_64-apple-darwin)" -output "$WORK/macos/libyalin_ffi.a"

# The module map sits in a subdirectory so that it cannot collide with other
# XCFrameworks whose headers Xcode copies into the same include directory.
HEADERS=$WORK/headers
mkdir -p "$HEADERS/YalinFFI"
cp "$ROOT/apple/Sources/YalinFFI/yalin.h" "$HEADERS/YalinFFI/"
cat > "$HEADERS/YalinFFI/module.modulemap" <<'MAP'
module YalinFFI {
  header "yalin.h"
  export *
}
MAP

xcodebuild -create-xcframework \
  -library "$WORK/ios/libyalin_ffi.a" -headers "$HEADERS" \
  -library "$WORK/ios-sim/libyalin_ffi.a" -headers "$HEADERS" \
  -library "$WORK/macos/libyalin_ffi.a" -headers "$HEADERS" \
  -output "$OUT"

echo "Built $OUT"
