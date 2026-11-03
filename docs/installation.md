# Installation

`fereg` and `feppml` require Stata 15 or newer. The 0.5.0 binary release supports
Linux x86-64, Windows x86-64 and macOS. The macOS plugin contains Apple Silicon
and Intel code; licensed Mac testing uses Apple Silicon. See
[validation](VALIDATION.md) for the checks performed on each platform.
The Linux binary requires glibc 2.28 or newer. Windows uses serial data loading;
parallel loading is enabled only on qualified Mac and Linux hosts. The Intel
Mac code is included but has not been tested in a licensed Intel Mac Stata session.

## Install stable version 0.5.0

Start a fresh Stata session and run:

```stata
net install fereg, from("https://raw.githubusercontent.com/johannes-schmieder/fereg/v0.5.0/") replace
fereg, selftest
help fereg
help feppml
```

The release tag pins the complete package, including its native plugins.
Both commands are installed together. Rust, Python, Julia, BLAS, OpenMP and
other Stata packages are not needed to use the binary package. Stata normally
installs user packages in its PLUS directory; `sysdir` shows that location.
Use the repository root URL: the `stata/` subdirectory is a packaging template
and does not contain the native binaries.

Test an OLS and a PPML fit:

```stata
sysuse auto, clear
fereg price mpg, absorb(rep78) vce(robust)
feppml price mpg, absorb(rep78) vce(robust)
```

To install the development package, change `v0.5.0` in the URL to `main`.
The community-contributed [`github` installer](https://github.com/haghish/github)
also installs from `main`:

```stata
net install github, from("https://haghish.github.io/github/")
github install johannes-schmieder/fereg
fereg, selftest
```

Use the tagged `net install` command when you need a fixed release for replication.

For an offline installation, download `fereg-0.5.0.zip` from the
[0.5.0 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0),
extract it, and use the absolute path of the extracted directory containing
`fereg.pkg`:

```stata
net install fereg, from("/absolute/path/to/fereg-0.5.0") replace
fereg, selftest
```

## Update or remove

Exit every Stata session that has loaded fereg before updating. Reopen Stata,
then rerun the direct `net install` command above with `replace`. `clear all`
does not reliably unload a native plugin. On Windows, loaded plugin files also cannot
be overwritten. To remove an installed package, run `ado uninstall fereg`.

## Refreshed 0.5.0 package

At the author's request, the v0.5.0 tag and release archives were refreshed on
2026-11-03 with the current Stata files, help, documentation and MIT licenses.
Reinstall with `replace` to receive the complete OLS/PPML output and the new
250,000-observation comparison in Example 2 of `help fereg`.

The package also includes the corrected PPML Auto/thread descriptions and
numerical acknowledgements. All three native plugins are byte-for-byte
unchanged, with their actual build commits and prior qualification retained.
The [publication record](../bench/results/v050-publication-20261103.json)
preserves the original archive identities. For an exact historical installation,
use its recorded Git commit rather than the refreshed tag.

## Build from source

Source builds need Git, Python 3.11 or newer, Rust installed through
[rustup](https://rustup.rs/), and a platform C compiler. The repository pins
Rust 1.85.1; run the commands from the repository root. Native outputs go in
`dist/`. The commands below check out stable 0.5.0. Use `--branch main` only
when you intend to build the development version.

```bash
git clone --depth 1 --branch v0.5.0 https://github.com/johannes-schmieder/fereg.git
cd fereg
```

Choose the commands for your platform:

| Platform | Additional build tools | Build and stage |
| --- | --- | --- |
| macOS | Xcode Command Line Tools (`xcode-select --install`) | Follow the [Mac source build recipe](macos_source_build.md), then `python3 scripts/stage_local_install.py --platform macos` |
| Linux x86-64 | GCC or Clang with its undefined-behavior sanitizer runtime, GNU binutils and development libc headers | `bash scripts/build_linux.sh` then `python3 scripts/stage_local_install.py --platform linux` |
| Windows x86-64 | Visual Studio 2022 Build Tools with Desktop development with C++, Windows SDK, and Python on PATH | In PowerShell: `powershell -File scripts/build_windows.ps1` then `python scripts/stage_local_install.py --platform windows` |

Mac builds require the explicit path-remap setup in that recipe to pass the
raw-byte privacy audit; its final note gives the identity settings for a current
Git checkout. The versioned archive instructions retain its exact source identity.

Build scripts audit the native binary and run load/ABI checks. The staging
command verifies the plugin checksum and prints a `net install` command with
the absolute local package path. Run that command in a fresh Stata session,
then run `fereg, selftest` and the two example fits above. The default staging
directory is `target/local-install`; to stage another build, choose a fresh
directory with `--output`. Building successfully on one platform does not
establish qualification on another.

The release's `fereg-0.5.0-source.zip` contains the project source, exact Cargo
dependency sources and their license files. Extract it and set the source
identity from its manifest before using the platform build commands above.
On macOS/Linux:

```bash
export FEREG_GIT_COMMIT="$(python3 -c 'import json; print(json.load(open("SOURCE_MANIFEST.json"))["source_sha"][:12])')"
```

In Windows PowerShell:

```powershell
$env:FEREG_GIT_COMMIT = (Get-Content SOURCE_MANIFEST.json -Raw | ConvertFrom-Json).source_sha.Substring(0, 12)
```

Run the build from the extracted source directory. Its `.cargo/config.toml`
uses the included dependencies; `cargo build --offline --locked --release
-p fereg-plugin` checks that Cargo needs no network downloads. Rust toolchains,
system compilers and Stata itself are prerequisites and are not bundled.
The source archive also includes the Mata source used to produce the included
Stata 15 `.mo` objects; `scripts/build_ppml_mata.py --stata /path/to/stata15`
rebuilds them with Stata 15.

## Troubleshooting

| Symptom | Action |
| --- | --- |
| `fereg` is not found | Run `which fereg` and check that installation completed into a directory on `adopath`. |
| `r(601)`, missing plugin | Install the complete package, including the native plugin for your platform. |
| `r(499)`, incompatible files | Exit Stata, install/update the complete package, and restart. Use `which fereg, all` to locate older copies earlier on `adopath`. |
| A Linux loader error names `GLIBC_2.28` | Use a system with glibc 2.28 or newer, or build from source for your supported local environment. |
| `github install` ends with `r(132)` | Restart Stata and use the direct tagged `net install` command above, then run the self-test. The helper can fail while reading GitHub release metadata even after installing the files. |
| macOS refuses to load the plugin | Use the package installed directly by Stata, or the local source build, then run `fereg, selftest verbose` for the loader error. |
| A build cannot find Python, Cargo or a C compiler | Install the listed build prerequisites and make sure they are on PATH before rerunning from the repository root. |

For a reproducible report, include `fereg, selftest verbose` output, your Stata
version and platform, the command that failed, and a small synthetic example.
Do not include private data or a Stata license file.
