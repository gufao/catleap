#!/usr/bin/env bash
# build.sh — produce wine-catleap-<VERSION>.tar.xz from Apple GPTK sources.
#
# Run on macOS Apple Silicon under Rosetta (`arch -x86_64 zsh`) with Intel
# Homebrew installed at /usr/local. This is intentional — Apple's GPTK Wine
# is x86_64-only and links against deps from Intel Homebrew.

set -euo pipefail

VERSION="${VERSION:-1.0.0}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="${SCRIPT_DIR}/build"
DIST_DIR="${SCRIPT_DIR}/dist"
PREFIX="${WORK_DIR}/prefix"
SOURCE_TARBALL="crossover-sources-22.1.1.tar.gz"
SOURCE_URL="https://media.codeweavers.com/pub/crossover/source/${SOURCE_TARBALL}"
SOURCE_SHA256="cdfe282ce33788bd4f969c8bfb1d3e2de060eb6c296fa1c3cdf4e4690b8b1831"

# --- preconditions --------------------------------------------------------
if [[ "$(uname -m)" != "x86_64" ]]; then
  echo "ERROR: This script must run under arch -x86_64 (Rosetta)." >&2
  echo "Re-run as: arch -x86_64 zsh ${BASH_SOURCE[0]}" >&2
  exit 1
fi

if [[ ! -x /usr/local/bin/brew ]]; then
  echo "ERROR: Intel Homebrew not found at /usr/local/bin/brew." >&2
  echo "Install Intel brew first: arch -x86_64 /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"" >&2
  exit 1
fi

BREW=/usr/local/bin/brew

AVAIL_KB=$(df -k "$SCRIPT_DIR" | awk 'NR==2 {print $4}')
if (( AVAIL_KB < 30000000 )); then
  echo "ERROR: Need ~30 GB free at $SCRIPT_DIR. Available: $((AVAIL_KB / 1024 / 1024)) GB" >&2
  exit 1
fi

# --- dependencies ---------------------------------------------------------
echo "==> Installing build dependencies via Intel Homebrew"
"$BREW" install \
  bison flex pkg-config mingw-w64 \
  gstreamer freetype sdl2 libgphoto2 faudio jpeg libpng \
  mpg123 libtiff libgsm glib gnutls libusb gettext sane-backends zlib

# openssl@1.1 was removed from homebrew-core. The build needs it; install
# from the gcenx tap which still ships it. (We only consume the openssl@1.1
# headers/libs at build time; we are not redistributing gcenx artifacts.)
if ! "$BREW" list openssl@1.1 >/dev/null 2>&1; then
  echo "==> openssl@1.1 missing, tapping gcenx/wine to obtain it"
  if ! "$BREW" tap | grep -q '^gcenx/wine$'; then
    "$BREW" tap gcenx/wine
  fi
  "$BREW" install openssl@1.1
fi

# Apple's GPTK formula has its own compiler. Use it.
"$BREW" tap apple/apple https://github.com/apple/homebrew-apple

# Patch the compiler formula to work with modern CMake. The bundled LLVM
# source uses cmake_minimum_required(VERSION < 3.5), which CMake 4+ rejects.
# Injecting CMAKE_POLICY_VERSION_MINIMUM=3.5 tells CMake to keep the legacy
# behaviour for this build only. Idempotent — skip if already patched.
COMPILER_FORMULA="$("$BREW" --repository)/Library/Taps/apple/homebrew-apple/Formula/game-porting-toolkit-compiler.rb"
if [[ ! -f "$COMPILER_FORMULA" ]]; then
  echo "ERROR: Compiler formula not found at $COMPILER_FORMULA" >&2
  exit 1
fi
if ! grep -q 'CMAKE_POLICY_VERSION_MINIMUM' "$COMPILER_FORMULA"; then
  echo "==> Patching $COMPILER_FORMULA for CMake 4 compatibility"
  /usr/bin/sed -i.bak \
    's|"-DCMAKE_INSTALL_PREFIX=#{prefix}",|"-DCMAKE_INSTALL_PREFIX=#{prefix}",\
                      "-DCMAKE_POLICY_VERSION_MINIMUM=3.5",|' \
    "$COMPILER_FORMULA"
  rm -f "${COMPILER_FORMULA}.bak"
  # If the compiler is already installed without the patch, force reinstall.
  if "$BREW" list game-porting-toolkit-compiler >/dev/null 2>&1; then
    "$BREW" reinstall apple/apple/game-porting-toolkit-compiler
  else
    "$BREW" install apple/apple/game-porting-toolkit-compiler
  fi
else
  "$BREW" install apple/apple/game-porting-toolkit-compiler
fi

GPTK_COMPILER="$("$BREW" --prefix game-porting-toolkit-compiler)"
if [[ ! -x "${GPTK_COMPILER}/bin/clang" ]]; then
  echo "ERROR: GPTK compiler clang not found at ${GPTK_COMPILER}/bin/clang" >&2
  echo "       The game-porting-toolkit-compiler install may be incomplete." >&2
  exit 1
fi

# --- fetch + extract sources ---------------------------------------------
mkdir -p "$WORK_DIR" "$DIST_DIR"
cd "$WORK_DIR"

if [[ ! -f "$SOURCE_TARBALL" ]]; then
  echo "==> Downloading $SOURCE_TARBALL"
  curl -fL --output "$SOURCE_TARBALL" "$SOURCE_URL"
fi

echo "==> Verifying source tarball"
echo "$SOURCE_SHA256  $SOURCE_TARBALL" | shasum -a 256 -c -

echo "==> Extracting wine/ subdir (clean rebuild)"
rm -rf wine wine64-build wine32-build
tar -xf "$SOURCE_TARBALL" --include='sources/wine/*' --strip-components=1

# --- apply Apple's patch -------------------------------------------------
echo "==> Applying Apple GPTK patch"
APPLE_FORMULA="$("$BREW" --repository)/Library/Taps/apple/homebrew-apple/Formula/game-porting-toolkit.rb"
if [[ ! -f "$APPLE_FORMULA" ]]; then
  echo "ERROR: Apple GPTK formula not found at $APPLE_FORMULA" >&2
  exit 1
fi
# Extract the patch from the formula (everything after __END__)
awk '/^__END__$/{found=1; next} found' "$APPLE_FORMULA" > apple.patch
if [[ ! -s apple.patch ]]; then
  echo "ERROR: apple.patch is empty — Apple formula may no longer use __END__ heredoc." >&2
  echo "       Inspect $APPLE_FORMULA and update the extraction logic." >&2
  exit 1
fi
( cd wine && patch -p1 < ../apple.patch )

# --- configure + build ---------------------------------------------------
COMMON_FLAGS=(
  "--prefix=${PREFIX}"
  "--disable-win16" "--disable-tests"
  "--without-x" "--without-pulse" "--without-dbus" "--without-inotify"
  "--without-alsa" "--without-capi" "--without-oss" "--without-udev"
  "--without-krb5"
)
CC_FLAGS=("CC=${GPTK_COMPILER}/bin/clang" "CXX=${GPTK_COMPILER}/bin/clang++")

CFLAGS_EXTRA="-O3 -Wno-implicit-function-declaration -Wno-format -Wno-deprecated-declarations -Wno-incompatible-pointer-types"
LDFLAGS_EXTRA="-lSystem -L/usr/local/lib -Wl,-rpath,/usr/local/lib -Wl,-rpath,@executable_path/../lib/external"
for dep in zlib freetype sdl2 libgphoto2 faudio jpeg libpng mpg123 libtiff libgsm glib gnutls libusb gettext openssl@1.1 sane-backends; do
  CFLAGS_EXTRA+=" -I$("$BREW" --prefix "$dep")/include"
  LDFLAGS_EXTRA+=" -L$("$BREW" --prefix "$dep")/lib"
done
export CFLAGS="$CFLAGS_EXTRA"
export CXXFLAGS="$CFLAGS_EXTRA"
export LDFLAGS="$LDFLAGS_EXTRA"
# Apple's GPTK compiler is clang-8 (2019) and only accepts 10.x deployment
# targets — anything 11+ produces "invalid version number". Apple's formula
# uses 10.14; we bump to 10.15 because newer Xcode SDKs may have dropped
# 10.14 support but should still tolerate 10.15.
export MACOSX_DEPLOYMENT_TARGET=10.15
export GSTREAMER_CFLAGS="-I$("$BREW" --prefix gstreamer)/include/gstreamer-1.0 -I$("$BREW" --prefix glib)/include/glib-2.0 -I$("$BREW" --prefix glib)/lib/glib-2.0/include"
export GSTREAMER_LIBS="-L$("$BREW" --prefix gstreamer)/lib -lglib-2.0 -lgmodule-2.0 -lgstreamer-1.0 -lgstaudio-1.0 -lgstvideo-1.0 -lgstgl-1.0 -lgobject-2.0"

mkdir -p wine64-build wine32-build

echo "==> Building wine64"
( cd wine64-build && \
    ../wine/configure "${COMMON_FLAGS[@]}" --enable-win64 --with-gnutls --with-freetype --with-gstreamer "${CC_FLAGS[@]}" && \
    make -j"$(sysctl -n hw.ncpu)" )

echo "==> Building wine32on64"
( cd wine32-build && \
    ../wine/configure "${COMMON_FLAGS[@]}" --enable-win32on64 --with-wine64=../wine64-build --without-gstreamer --without-gphoto --without-sane --without-krb5 --disable-winedbg --without-vulkan --disable-vulkan_1 --disable-winevulkan --without-openal --without-unwind --without-usb "${CC_FLAGS[@]}" && \
    make -j"$(sysctl -n hw.ncpu)" )

# --- install -------------------------------------------------------------
echo "==> Installing into prefix"
rm -rf "$PREFIX"
( cd wine64-build && make install )
( cd wine32-build && make install )

# --- post_install: rewrite dylib IDs to @rpath, then ad-hoc codesign ----
echo "==> Rewriting dylib IDs and ad-hoc signing"
for d in "${PREFIX}"/lib/wine/x86_64-unix/*.so "${PREFIX}"/lib/wine/x86_32on64-unix/*.so; do
  [[ -f "$d" ]] || continue
  chmod 0664 "$d"
  install_name_tool -id "@rpath/$(basename "$d")" "$d"
  codesign --force --sign - "$d"
  chmod 0444 "$d"
done

echo "==> Ad-hoc signing all Mach-O binaries in bin/"
for bin in "${PREFIX}"/bin/*; do
  [[ -f "$bin" ]] || continue
  if file "$bin" | grep -q "Mach-O"; then
    codesign --force --sign - "$bin"
  fi
done

# --- package -------------------------------------------------------------
ARTIFACT="${DIST_DIR}/wine-catleap-${VERSION}.tar.xz"
echo "==> Packaging into $ARTIFACT"
( cd "$PREFIX" && tar -cJf "$ARTIFACT" bin lib/wine share/wine )
shasum -a 256 "$ARTIFACT" > "${ARTIFACT}.sha256"

echo
echo "Done. Artifact: $ARTIFACT"
echo "SHA256:        $(cut -d' ' -f1 "${ARTIFACT}.sha256")"
