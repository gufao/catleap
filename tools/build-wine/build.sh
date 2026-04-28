#!/usr/bin/env bash
# build.sh — produce wine-catleap-<VERSION>.tar.xz by repackaging the
# gcenx-maintained Game Porting Toolkit binary distribution.
#
# Background: Apple's `apple/apple/game-porting-toolkit` Homebrew formula
# requires a 2019-era toolchain (clang-8, ld_classic, MacOSX 10.14 SDK)
# that no longer exists on free CI macOS runners or current Xcodes. The
# `Gcenx/game-porting-toolkit` project actively maintains a build of the
# same upstream sources (Apple's GPTK Wine = CodeWeavers 22.1.1 + Apple's
# patches) that compiles on current toolchains. We download their release
# tarball and repackage just the wine binary tree under our naming.

set -euo pipefail

VERSION="${VERSION:-1.0.0}"
GCENX_VERSION="${GCENX_VERSION:-3.0-3}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="${SCRIPT_DIR}/build"
DIST_DIR="${SCRIPT_DIR}/dist"

GCENX_TARBALL="game-porting-toolkit-${GCENX_VERSION}.tar.xz"
GCENX_URL="https://github.com/Gcenx/game-porting-toolkit/releases/download/Game-Porting-Toolkit-${GCENX_VERSION}/${GCENX_TARBALL}"

mkdir -p "$WORK_DIR" "$DIST_DIR"
cd "$WORK_DIR"

# --- download upstream artefact -----------------------------------------
if [[ ! -f "$GCENX_TARBALL" ]]; then
  echo "==> Downloading $GCENX_TARBALL"
  curl -fL --output "$GCENX_TARBALL" "$GCENX_URL"
fi

echo "==> Extracting"
rm -rf "Game Porting Toolkit.app"
tar -xf "$GCENX_TARBALL"

# --- locate the wine tree -----------------------------------------------
WINE_SRC="Game Porting Toolkit.app/Contents/Resources/wine"
if [[ ! -d "$WINE_SRC/bin" ]]; then
  echo "ERROR: expected $WINE_SRC/bin not found in extracted tarball" >&2
  echo "       gcenx may have changed the .app layout; inspect $GCENX_TARBALL manually." >&2
  exit 1
fi
if [[ ! -x "$WINE_SRC/bin/wine64" ]]; then
  echo "ERROR: $WINE_SRC/bin/wine64 missing or not executable" >&2
  exit 1
fi

# --- repackage with our naming ------------------------------------------
ARTIFACT="${DIST_DIR}/wine-catleap-${VERSION}.tar.xz"
echo "==> Repacking as $ARTIFACT"
rm -f "$ARTIFACT" "${ARTIFACT}.sha256"
( cd "$WINE_SRC" && tar -cJf "$ARTIFACT" bin lib share )
shasum -a 256 "$ARTIFACT" > "${ARTIFACT}.sha256"

SIZE_MB="$(du -m "$ARTIFACT" | cut -f1)"
SHA="$(cut -d' ' -f1 "${ARTIFACT}.sha256")"

echo
echo "Done."
echo "  Artifact: $ARTIFACT"
echo "  Size:     ${SIZE_MB} MB"
echo "  SHA256:   ${SHA}"
echo
echo "Upstream provenance: gcenx ${GCENX_VERSION} (compiled from Apple's"
echo "GPTK Wine sources — CodeWeavers 22.1.1 + Apple's patches)."
