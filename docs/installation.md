# Installation

`fereg` and `feppml` require Stata 15 or newer. The current binary beta supports
Linux x86-64, Windows x86-64 and macOS. The macOS plugin contains Apple Silicon
and Intel code; licensed Mac testing uses Apple Silicon. See
[validation](VALIDATION.md) for the checks performed on each platform.
The Linux binary requires glibc 2.28 or newer.

## Install the current package from main

Start a fresh Stata session and run:

```stata
net install fereg, from("https://raw.githubusercontent.com/johannes-schmieder/fereg/main/") replace
fereg, selftest
help fereg
help feppml
```

This reads the root `fereg.pkg` from `main`. The built-in `net install` route
passes the public installation checks. During beta.11 qualification, the
community-contributed [`github` installer](https://github.com/haghish/github)
downloaded all 35 correct package files but returned `r(132)` in its subsequent
release-metadata query. The official current query helper has the same code.
Use `net install` while that helper limitation remains unresolved; the
[public verification record](../bench/results/beta11-publication-20261030.json)
retains the failure and the successful direct installation checks.

Both commands and their shared native plugin are installed together. Rust,
Python, BLAS, OpenMP and other Stata packages are not needed to use this binary
package. Stata normally installs user packages in its PLUS directory; `sysdir`
shows that location. Use the repository root URL above: the `stata/` subdirectory
contains the release template, while the root package includes the maintained
native binary.

Test an OLS and a PPML fit:

```stata
sysuse auto, clear
fereg price mpg, absorb(rep78) vce(robust)
feppml price mpg, absorb(rep78) vce(robust)
```

To install beta.11 independently of later changes to `main`, use a fresh Stata
session:

```stata
net install fereg, from("https://github.com/johannes-schmieder/fereg/releases/download/v0.5.0-beta.11/") replace
fereg, selftest
```

For an offline installation, download `fereg-0.5.0.zip` from the
[beta release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.11),
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

## Build from source

Source builds need Git, Python 3.11 or newer, Rust installed through
[rustup](https://rustup.rs/), and a platform C compiler. The repository pins
Rust 1.85.1; run the commands from the repository root. Native outputs go in
`dist/`. This builds the same runtime used by the binary beta.

```bash
git clone --depth 1 --branch main https://github.com/johannes-schmieder/fereg.git
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
| macOS refuses to load the plugin | Use the package installed directly by Stata, or the local source build, then run `fereg, selftest verbose` for the loader error. |
| A build cannot find Python, Cargo or a C compiler | Install the listed build prerequisites and make sure they are on PATH before rerunning from the repository root. |

For a reproducible report, include `fereg, selftest verbose` output, your Stata
version and platform, the command that failed, and a small synthetic example.
Do not include private data or a Stata license file.
