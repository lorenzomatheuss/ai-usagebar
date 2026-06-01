#!/usr/bin/env bash
# build-xcframework.sh — Build TorvenCore.xcframework for consumption by the
# Xcode/SwiftUI app under `apple/Torven/`.
#
# Implemented in Story 1.5 (build-xcframework-pipeline). See ADR-4 for the
# rationale behind the UniFFI + xcframework architecture.
#
# ---------------------------------------------------------------------------
# Flow
# ---------------------------------------------------------------------------
#
#   1. Generate Swift bindings + C header + clang modulemap from the UDL.
#   2. Build torven-core as a static library for each available Apple Darwin
#      target. Story 1.5 ships aarch64-only (see Decision D-2); the script
#      tries x86_64 and skips with a warning if the target sysroot is missing.
#   3. lipo the slices into a single universal static library if multiple
#      arches built; otherwise reuse the single-arch slice as-is.
#   4. Stage the header + modulemap into `crates/torven-core/include/` so the
#      xcframework picks them up.
#   5. Produce `apple/Frameworks/TorvenCore.xcframework`.
#   6. Copy the generated Swift binding into
#      `apple/Torven/Bridge/Generated/torven_core.swift` (gitignored — every
#      dev regenerates on `make build-core`).
#
# ---------------------------------------------------------------------------
# Requirements on the build host
# ---------------------------------------------------------------------------
#
#   * macOS with Xcode CommandLineTools (provides xcodebuild, lipo, xcrun)
#   * cargo + rustc with at least one of:
#       - aarch64-apple-darwin   (host on Apple Silicon)
#       - x86_64-apple-darwin    (host on Intel, or rustup target add'd)
#   * Apple Silicon dev machines built from Homebrew rustc already have
#     aarch64-apple-darwin. x86_64 requires rustup.
#
# Universal-binary release wiring is the concern of Story 1.21 (notarization)
# / Story 1.22 (release pipeline) — Story 1.5 explicitly produces an
# aarch64-only xcframework when x86_64 is unavailable.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# `xcodebuild -create-xcframework` requires the full Xcode.app toolchain
# (CommandLineTools alone is not enough). On dev machines where
# `xcode-select` still points at CommandLineTools, we transparently fall back
# to /Applications/Xcode.app via DEVELOPER_DIR so the user doesn't need to
# `sudo xcode-select -s` just to build the xcframework. CI / release boxes
# can pre-set DEVELOPER_DIR themselves; the override is gated on the env var
# being unset so we don't clobber an explicit choice.
if [[ -z "${DEVELOPER_DIR:-}" ]]; then
  active_dir="$(xcode-select -p 2>/dev/null || echo)"
  if [[ "$active_dir" == "/Library/Developer/CommandLineTools" ]]; then
    if [[ -d "/Applications/Xcode.app/Contents/Developer" ]]; then
      export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
      echo "Note: xcode-select points at CommandLineTools; overriding"
      echo "      DEVELOPER_DIR=$DEVELOPER_DIR for xcframework build."
    else
      echo "ERROR: xcodebuild -create-xcframework requires Xcode.app, but" >&2
      echo "       /Applications/Xcode.app is missing and xcode-select" >&2
      echo "       points at $active_dir. Install Xcode or run" >&2
      echo "       'sudo xcode-select -s /path/to/Xcode.app/Contents/Developer'." >&2
      exit 1
    fi
  fi
fi

CORE_CRATE="torven-core"
LIB_NAME="libtorven_core.a"
UDL="$REPO_ROOT/crates/torven-core/src/torven_core.udl"

BINDINGS_OUT="$REPO_ROOT/target/uniffi-swift"
INCLUDE_DIR="$REPO_ROOT/crates/torven-core/include"
UNIVERSAL_DIR="$REPO_ROOT/target/universal-apple-darwin/release"
SWIFT_DEST_DIR="$REPO_ROOT/apple/Torven/Bridge/Generated"
XCFRAMEWORK_OUT="$REPO_ROOT/apple/Frameworks/TorvenCore.xcframework"

# ---------------------------------------------------------------------------
# Step 1 — Generate Swift bindings + C header + modulemap
# ---------------------------------------------------------------------------
echo "[1/6] Generating Swift bindings from UDL..."
rm -rf "$BINDINGS_OUT"
mkdir -p "$BINDINGS_OUT"
(
  cd "$REPO_ROOT"
  cargo run --features uniffi-cli --bin uniffi-bindgen --quiet -- \
    generate "$UDL" \
    --language swift \
    --out-dir "$BINDINGS_OUT"
)

# Expected artifacts:
#   torven_core.swift          — Swift API the app imports
#   torven_coreFFI.h           — C header for the static lib
#   torven_coreFFI.modulemap   — clang module map (renamed to module.modulemap)
for f in torven_core.swift torven_coreFFI.h torven_coreFFI.modulemap; do
  if [[ ! -f "$BINDINGS_OUT/$f" ]]; then
    echo "ERROR: expected uniffi-bindgen artifact missing: $BINDINGS_OUT/$f" >&2
    exit 1
  fi
done

# ---------------------------------------------------------------------------
# Step 2 — Build per-arch static libraries
# ---------------------------------------------------------------------------
# Determine which Apple Darwin targets are installed in the rustc sysroot.
# Skip-with-warning rather than fail when a target is missing — that's what
# lets the Apple-Silicon-only dev box (Homebrew rustc) keep working.

SYSROOT_LIB="$(rustc --print sysroot)/lib/rustlib"
declare -a AVAILABLE_TARGETS=()
declare -a SKIPPED_TARGETS=()

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  if [[ -d "$SYSROOT_LIB/$target" ]]; then
    AVAILABLE_TARGETS+=("$target")
  else
    SKIPPED_TARGETS+=("$target")
  fi
done

if [[ ${#AVAILABLE_TARGETS[@]} -eq 0 ]]; then
  echo "ERROR: no Apple Darwin Rust targets installed." >&2
  echo "  Try: rustup target add aarch64-apple-darwin" >&2
  exit 1
fi

echo "[2/6] Building $CORE_CRATE for: ${AVAILABLE_TARGETS[*]}"
if [[ ${#SKIPPED_TARGETS[@]} -gt 0 ]]; then
  echo "      Skipped (target sysroot missing): ${SKIPPED_TARGETS[*]}"
  echo "      Universal binary will not be produced. See Story 1.5 Decision D-2."
fi

declare -a SLICE_PATHS=()
for target in "${AVAILABLE_TARGETS[@]}"; do
  (
    cd "$REPO_ROOT"
    cargo build --release --target "$target" -p "$CORE_CRATE"
  )
  SLICE_PATHS+=("$REPO_ROOT/target/$target/release/$LIB_NAME")
done

# ---------------------------------------------------------------------------
# Step 3 — Merge slices (or reuse the single slice as-is)
# ---------------------------------------------------------------------------
mkdir -p "$UNIVERSAL_DIR"
UNIVERSAL_LIB="$UNIVERSAL_DIR/$LIB_NAME"

if [[ ${#SLICE_PATHS[@]} -gt 1 ]]; then
  echo "[3/6] Creating universal binary via lipo..."
  lipo -create "${SLICE_PATHS[@]}" -output "$UNIVERSAL_LIB"
else
  echo "[3/6] Single-arch build — copying slice to universal/ as-is..."
  cp "${SLICE_PATHS[0]}" "$UNIVERSAL_LIB"
fi

# ---------------------------------------------------------------------------
# Step 4 — Stage headers + modulemap into include/
# ---------------------------------------------------------------------------
echo "[4/6] Staging headers into $INCLUDE_DIR..."
rm -rf "$INCLUDE_DIR"
mkdir -p "$INCLUDE_DIR"
cp "$BINDINGS_OUT/torven_coreFFI.h" "$INCLUDE_DIR/torven_coreFFI.h"
# The xcframework / clang expects the module map to be named exactly
# `module.modulemap` inside the headers directory of each slice.
cp "$BINDINGS_OUT/torven_coreFFI.modulemap" "$INCLUDE_DIR/module.modulemap"

# ---------------------------------------------------------------------------
# Step 5 — Build the xcframework
# ---------------------------------------------------------------------------
echo "[5/6] Building $XCFRAMEWORK_OUT..."
mkdir -p "$(dirname "$XCFRAMEWORK_OUT")"
rm -rf "$XCFRAMEWORK_OUT"
xcodebuild -create-xcframework \
  -library "$UNIVERSAL_LIB" \
  -headers "$INCLUDE_DIR" \
  -output "$XCFRAMEWORK_OUT"

# ---------------------------------------------------------------------------
# Step 6 — Copy generated Swift binding into the Xcode source tree
# ---------------------------------------------------------------------------
echo "[6/6] Copying torven_core.swift into Xcode source tree..."
mkdir -p "$SWIFT_DEST_DIR"
cp "$BINDINGS_OUT/torven_core.swift" "$SWIFT_DEST_DIR/torven_core.swift"

echo ""
echo "  TorvenCore.xcframework built at: $XCFRAMEWORK_OUT"
echo "  Swift bindings at: $SWIFT_DEST_DIR/torven_core.swift"
echo ""
echo "  Next: cd apple && xcodegen generate && xcodebuild -scheme Torven build"
