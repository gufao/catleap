# Catleap Wine Build Pipeline

Builds `wine-catleap-<VERSION>.tar.xz` from Apple's official GPTK Wine
sources (CodeWeavers 22.1.1 + Apple patches). The artifact is uploaded
as a GitHub Release asset and consumed by Catleap's first-run installer.

## Prerequisites

- macOS Apple Silicon, with Rosetta 2 installed (`softwareupdate --install-rosetta`)
- Intel Homebrew at `/usr/local/bin/brew`. Install:
  ```sh
  arch -x86_64 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  ```
- ~30 GB free disk for the build tree

## Usage

```sh
arch -x86_64 zsh tools/build-wine/build.sh
```

Override the version:

```sh
VERSION=1.1.0 arch -x86_64 zsh tools/build-wine/build.sh
```

The build takes 30–90 minutes depending on hardware. Output:

- `tools/build-wine/dist/wine-catleap-<VERSION>.tar.xz`
- `tools/build-wine/dist/wine-catleap-<VERSION>.tar.xz.sha256`

## Publishing

1. Verify the artifact runs locally by extracting into a test data dir
   and pointing Catleap at it (or copy into `~/Library/Application Support/Catleap/wine/`).
2. Create a GitHub Release on the Catleap repo named `wine-catleap-<VERSION>`.
3. Upload the `.tar.xz` and `.sha256` as release assets.
4. Update `WINE_RELEASE_URL`, `WINE_EXPECTED_SHA256`, and `WINE_EXPECTED_VERSION`
   constants in `src-tauri/src/wine/installer.rs` to point at the new release.
5. Bump `Settings.wine_version` schema if needed and ship a Catleap release.

## openssl@1.1

Apple's GPTK formula depends on `openssl@1.1`, which has been removed
from `homebrew-core`. The script obtains it from the `gcenx/wine` tap
(which still maintains it). We only consume openssl@1.1 at build time;
no gcenx artifacts ship to end users.

## Troubleshooting

- **"openssl@1.1 not found"**: the `gcenx/wine` tap may have changed its
  formulae layout. Inspect `brew search openssl` and adjust the script.
- **Patch fails to apply**: Apple's tap may have updated the patch.
  `brew tap apple/apple https://github.com/apple/homebrew-apple` and
  re-run.
- **Codesign errors**: ensure no antivirus is interfering. Ad-hoc
  signatures (`codesign --sign -`) are sufficient for local launch.
