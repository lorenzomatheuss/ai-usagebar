#!/usr/bin/env bash
# build-xcframework.sh — Build TorvenCore.xcframework for consumption by the
# Xcode/SwiftUI app under `apple/Torven/`.
#
# Status: STUB. This script is a placeholder created by Story 1.2
# (setup-uniffi). It will be implemented end-to-end in Story 1.5
# (build-xcframework-pipeline).
#
# ---------------------------------------------------------------------------
# Planned flow (Story 1.5)
# ---------------------------------------------------------------------------
#
#   1. Generate Swift bindings + module map from the UDL:
#
#        cargo run --bin uniffi-bindgen \
#          generate crates/torven-core/src/torven_core.udl \
#          --language swift \
#          --out-dir target/uniffi-swift
#
#      Produces:
#        - torven_core.swift          (Swift API the app imports)
#        - torven_coreFFI.h           (C header for the static lib)
#        - torven_coreFFI.modulemap   (clang module map)
#
#   2. Build the Rust static library for both macOS architectures:
#
#        cargo build --release --target aarch64-apple-darwin -p torven-core
#        cargo build --release --target x86_64-apple-darwin  -p torven-core
#
#   3. Merge the two slices into a single universal static lib:
#
#        mkdir -p target/universal-apple-darwin/release
#        lipo -create \
#          target/aarch64-apple-darwin/release/libtorven_core.a \
#          target/x86_64-apple-darwin/release/libtorven_core.a \
#          -output target/universal-apple-darwin/release/libtorven_core.a
#
#   4. Stage the header + modulemap next to the library so xcodebuild can
#      bundle them into the XCFramework:
#
#        cp target/uniffi-swift/torven_coreFFI.h         crates/torven-core/include/
#        cp target/uniffi-swift/torven_coreFFI.modulemap crates/torven-core/include/module.modulemap
#
#   5. Produce the XCFramework consumed by the Xcode project:
#
#        rm -rf apple/Frameworks/TorvenCore.xcframework
#        xcodebuild -create-xcframework \
#          -library  target/universal-apple-darwin/release/libtorven_core.a \
#          -headers  crates/torven-core/include \
#          -output   apple/Frameworks/TorvenCore.xcframework
#
#   6. Copy the generated `torven_core.swift` into the Xcode project at
#      `apple/Torven/Torven/Generated/torven_core.swift`. (xcodegen picks it
#      up via the existing `apple/project.yml`.)
#
# ---------------------------------------------------------------------------
# Why this is not yet implemented
# ---------------------------------------------------------------------------
#
# Story 1.2 only validates the UniFFI pipeline via the AR-1 spike (running
# `uniffi-bindgen generate` against a /tmp output dir). The full XCFramework
# build requires:
#   - apple-darwin Rust targets installed (`rustup target add ...`)
#   - The actual Xcode integration point (Story 1.5)
#
# Run order during development will be: this script -> open Xcode -> build.

set -euo pipefail

echo "build-xcframework.sh: stub — implement fully in Story 1.5 (build-xcframework-pipeline)"
exit 0
