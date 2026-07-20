#!/usr/bin/env bash
#
# Build script for Clarity Mobile Android artifacts.
#
# 1. Generates UniFFI Kotlin bindings from the UDL file.
# 2. Cross-compiles `clarity-mobile-core` to Android ABIs via cargo-ndk.
#
# Usage:
#   bash mobile/android/rust/build-android.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ANDROID_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
APP_DIR="$ANDROID_DIR/app"
JAVA_SRC_DIR="$APP_DIR/src/main/java"
JNI_LIBS_DIR="$APP_DIR/src/main/jniLibs"

# Rust workspace root is three levels above this script:
# mobile/android/rust -> mobile/android -> mobile -> clarity/
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

cd "$WORKSPACE_ROOT"

# Android targets to compile. arm64-v8a is required for physical devices;
# x86_64 is added so the same APK runs on the Android emulator without
# relying on ARM translation (which can load a stale x86_64 library).
TARGETS="arm64-v8a x86_64"
# TARGETS="arm64-v8a armeabi-v7a x86_64 x86"

echo "==> Generating UniFFI Kotlin bindings..."
mkdir -p "$JAVA_SRC_DIR"
cargo run --quiet -p clarity-mobile-core --bin uniffi-bindgen -- \
    generate \
    --language kotlin \
    --out-dir "$JAVA_SRC_DIR" \
    crates/clarity-mobile-core/src/clarity_mobile_core.udl

echo "==> Building Rust shared libraries for Android..."
for target in $TARGETS; do
    echo "    Building for $target..."
    cargo ndk -t "$target" -o "$JNI_LIBS_DIR" build --quiet -p clarity-mobile-core --features android-logs
    # UniFFI/JNA expects libuniffi_<namespace>.so, but cargo builds
    # lib<crate-name>.so. Rename so the Kotlin loader can find it.
    mv "$JNI_LIBS_DIR/$target/libclarity_mobile_core.so" \
       "$JNI_LIBS_DIR/$target/libuniffi_clarity_mobile_core.so"
done

echo "==> Done. Artifacts:"
echo "    Kotlin bindings: $JAVA_SRC_DIR/uniffi/clarity_mobile_core/"
echo "    Native libraries: $JNI_LIBS_DIR/"
