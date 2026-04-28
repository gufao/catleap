# Catleap Wine Build Pipeline

Repackages the [gcenx-maintained](https://github.com/Gcenx/game-porting-toolkit)
Game Porting Toolkit Wine binary into `wine-catleap-<VERSION>.tar.xz`. The
artifact is uploaded as a GitHub Release asset and consumed by Catleap's
first-run installer.

## Why repackage instead of compiling?

Apple's official `apple/apple/game-porting-toolkit` Homebrew formula requires
a 2019-era toolchain (clang-8, `ld_classic`, MacOSX 10.14 SDK) that no longer
exists on free CI macOS runners or current Xcode versions. The gcenx project
keeps a working build of the same upstream sources (Apple's GPTK Wine =
CodeWeavers 22.1.1 + Apple's patches) on current toolchains. We download
their release tarball, extract just the `wine/` subtree from the `.app`
bundle, and repack it under our naming and versioning. End users get a
binary functionally equivalent to building from Apple's source — gcenx is
the build operator instead of us.

## Prerequisites

- Any current macOS (no Xcode, no Homebrew, no Rosetta required)
- `curl`, `tar`, `shasum` (all built-in)

## Usage

```sh
tools/build-wine/build.sh
```

Override the Catleap version or the upstream gcenx version:

```sh
VERSION=1.1.0 GCENX_VERSION=3.0-3 tools/build-wine/build.sh
```

Run takes ~3-5 minutes (most of it downloading ~240 MB from gcenx's GitHub
release). Output:

- `tools/build-wine/dist/wine-catleap-<VERSION>.tar.xz`
- `tools/build-wine/dist/wine-catleap-<VERSION>.tar.xz.sha256`

## Publishing (CI)

The `.github/workflows/build-wine.yml` workflow runs this script on a macOS
runner whenever a `wine-catleap-v*` tag is pushed (or via manual dispatch).
On success it creates a GitHub Release with the `.tar.xz` and `.sha256`
attached, and opens a PR updating the installer constants in
`src-tauri/src/wine/installer.rs`. Merge that PR to ship.

```sh
gh workflow run build-wine.yml -f version=1.0.1
# …or
git tag wine-catleap-v1.0.1 && git push origin wine-catleap-v1.0.1
```

## Bumping the upstream

When gcenx publishes a new GPTK release, set `GCENX_VERSION` accordingly. We
deliberately don't auto-track latest — pinning gives us a stable artifact
that CI can reproduce identically on demand.
