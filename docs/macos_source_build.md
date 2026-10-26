# Building the Mac source archive

The corresponding-source archive includes the exact locked dependencies, their licenses,
the local Rayon patch and a relocatable offline Cargo configuration. Its
`SOURCE_MANIFEST.json` identifies the source commit. The binary package has its
own manifest: a local rebuild does not inherit the supplied binary's test results.

Use macOS with Xcode command-line tools, Python 3.11 or newer, and Rust/Cargo
1.85.1 with both `x86_64-apple-darwin` and `aarch64-apple-darwin` targets already
installed. Keep `rustup`, `clang`, `strip`, `lipo`, `codesign`, `nm`, `otool` and
`python3` on PATH. Both slices are built; this does not establish native Intel-host
Stata qualification.

Verify the published archive checksum, extract it preserving executable permissions
and enter `fereg-0.5.0-source`. Run the following in Bash. The fresh Cargo home
makes missing dependency sources visible; Rust path remaps prevent local paths
from entering diagnostic strings in the distributed binary.

```bash
set -e
FEREG_SOURCE_ROOT="$(pwd -P)"
FEREG_BUILD_WORK="$(mktemp -d)"
mkdir "$FEREG_BUILD_WORK/cargo-home" "$FEREG_BUILD_WORK/cargo-target"
export CARGO_HOME="$FEREG_BUILD_WORK/cargo-home"
export CARGO_TARGET_DIR="$FEREG_BUILD_WORK/cargo-target"
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=4 RUSTUP_TOOLCHAIN=1.85.1
export RUSTC="$(rustup which --toolchain 1.85.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.85.1 rustdoc)"
export PATH="$(dirname "$RUSTC"):$PATH"
FEREG_RUSTUP_ROOT="${RUSTUP_HOME:-$HOME/.rustup}"
FEREG_REMAP_SEPARATOR=$'\x1f'
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$FEREG_SOURCE_ROOT=/fereg-source${FEREG_REMAP_SEPARATOR}--remap-path-prefix=$CARGO_TARGET_DIR=/fereg-target${FEREG_REMAP_SEPARATOR}--remap-path-prefix=$CARGO_HOME=/cargo${FEREG_REMAP_SEPARATOR}--remap-path-prefix=$FEREG_RUSTUP_ROOT=/rust-toolchain"
export FEREG_GIT_COMMIT="$(python3 -c 'import json; print(json.load(open("SOURCE_MANIFEST.json"))["source_sha"][:12])')"
export FEREG_BUILD_DATE=2026-10-25
bash scripts/build_macos.sh
```

Use a clean compiler environment: do not combine this recipe with inherited
`RUSTFLAGS`, compiler wrappers, profile overrides, `CFLAGS` or `LDFLAGS`.
The build uses release optimization, fat LTO and one codegen unit, strips only
Mach-O debug symbols, signs the universal bundle and runs the maintained binary
privacy/dependency/export audit and actual ABI/load/panic checks. The result is
`dist/fereg_macos.plugin`. Byte-for-byte reproducibility across SDKs, paths and
signing environments is not claimed.

For a current Git checkout, use the same remap setup but identify the source with
`git rev-parse --short=12 HEAD` and use the current UTC build date. Delete the
build work directory when finished.
