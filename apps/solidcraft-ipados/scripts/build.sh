#!/bin/sh
# Unsigned device build only. Never boots, launches, installs or alters signing accounts.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repository_dir=$(CDPATH= cd -- "$script_dir/../../.." && pwd)
cd "$repository_dir"
export CARGO_TARGET_DIR="$repository_dir/target"
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export SDKROOT="$(xcrun --sdk iphoneos --show-sdk-path)"
cargo +1.95.0 build --locked --profile ci --target aarch64-apple-ios -p solidcraft-ipados-bridge
xcodebuild \
  -project apps/solidcraft-ipados/SolidCraft.xcodeproj \
  -scheme SolidCraft \
  -configuration Debug \
  -sdk iphoneos \
  -destination 'generic/platform=iOS' \
  -derivedDataPath "$repository_dir/target/ipados-xcode" \
  CODE_SIGNING_ALLOWED=NO \
  CODE_SIGNING_REQUIRED=NO \
  build
