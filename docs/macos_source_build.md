# Building the beta.2 Mac source archive

This supplements the abbreviated build command inside the frozen beta.2 archive.
Use the explicit Rust path remaps below: stripping debug maps alone does not
remove paths embedded in Rust diagnostic strings. The supplied archive and
native bundle remain immutable.

The recipe was checked on Apple Silicon macOS with Xcode command-line tools,
Python 3.11 or newer and preinstalled Rust/Cargo 1.85.1, including both Darwin targets.
Install that toolchain and the `x86_64-apple-darwin` and
`aarch64-apple-darwin` targets before the offline build. Keep `rustup`, `clang`,
`strip`, `lipo`, `codesign`, `nm`, `otool` and `python3` available on PATH.
This builds both slices; it is not a native Intel-host Stata qualification.

Exact inputs:

- Source commit: `3f487f214331bc9b87c379fa0a27de6347f9a8f7`.
- [Source archive](https://github.com/johannes-schmieder/fereg/releases/download/v0.5.0-beta.2/fereg-0.5.0-source.zip),
  SHA-256 `ed6b2ecc1a8eb471a77870598963c30d77bdcdcee412fc08034a7426a7f361da`.
- The archive includes its file manifest, exact locked dependency sources,
  local Rayon patch, licenses and relocatable offline Cargo configuration.

Verify the archive hash, extract it preserving executable permissions, then enter
`fereg-0.5.0-source`. Run the following in Bash. The fresh Cargo home prevents
an existing dependency cache from hiding missing archive inputs.

```bash
set -e
FEREG_SOURCE_ROOT="$(pwd -P)"
FEREG_BUILD_WORK="$(mktemp -d)"
mkdir "$FEREG_BUILD_WORK/cargo-home" "$FEREG_BUILD_WORK/cargo-target"
export CARGO_HOME="$FEREG_BUILD_WORK/cargo-home"
export CARGO_TARGET_DIR="$FEREG_BUILD_WORK/cargo-target"
export CARGO_NET_OFFLINE=true CARGO_BUILD_JOBS=4 RUSTUP_TOOLCHAIN=1.85.1
RUSTC="$(rustup which --toolchain 1.85.1 rustc)"
RUSTDOC="$(rustup which --toolchain 1.85.1 rustdoc)"
export RUSTC RUSTDOC
export PATH="$(dirname "$RUSTC"):$PATH"
FEREG_RUSTUP_ROOT="${RUSTUP_HOME:-$HOME/.rustup}"
FEREG_REMAP_SEPARATOR=$'\x1f'
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$FEREG_SOURCE_ROOT=/fereg-source${FEREG_REMAP_SEPARATOR}--remap-path-prefix=$CARGO_TARGET_DIR=/fereg-target${FEREG_REMAP_SEPARATOR}--remap-path-prefix=$CARGO_HOME=/cargo${FEREG_REMAP_SEPARATOR}--remap-path-prefix=$FEREG_RUSTUP_ROOT=/rust-toolchain"
export FEREG_GIT_COMMIT=3f487f214331 FEREG_BUILD_DATE=2026-10-16
bash scripts/build_macos.sh
```

Use a clean compiler environment: do not combine this recipe with inherited
`RUSTFLAGS`, compiler wrappers, profile overrides, `CFLAGS` or `LDFLAGS`.
The build uses release optimization, fat LTO and one codegen unit, removes only
Mach-O debug symbols before signing, and runs the maintained raw-byte privacy,
universal dependency/export and actual ABI/load checks before writing a checksum.
The resulting bundle is `dist/fereg_macos.plugin` in the extracted source tree.
The original shipped bundle is separately bound and validated; rebuilding does
not inherit its licensed Stata or performance evidence.

The checked offline rebuild produced SHA-256
`38fa61a9ce310f30c59f03153b4e3b3677683480cfe7744fbef104397fe4c9d0`.
The supplied tested bundle is
`6eeec78a552eede98af1eb1aeb534a63bf156855aa92e32fb75d7ea925f35442`.
They differ; byte reproducibility is not claimed. Build paths, dependency package
identities, SDK/linker versions and signing can affect the resulting bytes.

For a current Git checkout, use the same remap setup, but replace the two
archive identity constants before building:

```bash
export FEREG_GIT_COMMIT="$(git rev-parse --short=12 HEAD)"
export FEREG_BUILD_DATE="$(date -u +%Y-%m-%d)"
```

Record the checkout and actual build inputs; this does not turn a new build
into the supplied beta.2 artifact.
