# Packaging

Johannes authorized public source/native beta publication on 2026-10-13 and
confirmed the required distribution permissions. See
[licensing](licensing.md), [validation](VALIDATION.md) and
[development checks](development_workflow.md). GitHub tests remain paused.

Build the platform plugin with `scripts/build_macos.sh`, `build_linux.sh` or
`build_windows.ps1`. Each build runs its native audit/load checks and writes a
checksum. Run scientific, transaction and installed-package checks for the actual
artifact before distributing a development package. Retain explicit platform gaps.

Use uppercase `F` package directives for the license texts, source link, JSON
binding and SHA-256 files. Stata otherwise treats those suffixes as ancillary
files and omits them from `net install`; see the
[Stata net manual](https://www.stata.com/manuals/rnet.pdf). Installation checks
must verify their presence and bytes in the isolated PLUS directory too.

`scripts/stage_release.py` stages a deterministic archive with one plugin per
included platform, production Stata files, checksums, source identity, dependency
audit (receipt written by `scripts/run_dependency_audit.py`), SBOM and licenses.
Default production staging requires all three platforms. `--platforms macos`
creates an explicitly macOS-only beta and filters `fereg.pkg` to that platform.
The manifest names included and missing platforms. It makes no qualification
claim for omitted platforms or for an untested processor/host.
`--review-platforms macos` creates a visibly incomplete review archive; passing its
local checks does not confer publication eligibility. `run_staged_stata_tests.py
--review-only` admits only such explicitly requested review packages.

Source archives use `scripts/package_release.py`. For a native publication, run
`cargo vendor --locked --versioned-dirs` into the task directory and retain its
emitted Cargo configuration. Pass those paths with `--vendor-directory` and
`--vendor-configuration`. Also pass the pinned toolchain's `share/doc/rust`
directory using `--rust-license-directory` to retain its notices. The source
archive includes the complete dependency
closure, its license texts and a relocatable offline Cargo configuration.
Extract its `THIRD_PARTY_LICENSES.txt` and pass that file to native staging with
`--third-party-licenses`; the installed package carries the combined notices,
GPLv3 text and corresponding-source URL. Test a build from the source archive
with Cargo offline before publishing it alongside the exact binary package.

For the `v0.5.0-beta.1` release, upload the binary/source ZIPs and checksums, plus
every flat file from the binary package as individual GitHub release assets.
This supplies the versioned `net install` URL in the
[installation guide](installation.md). Run a fresh Stata installation from that
public URL into an isolated PLUS directory, verify the installed hashes, then
run selftest and OLS/PPML checks. A local package test alone is not an HTTP
installation test.

Use one `target/tmp/<task>/` directory for clean source snapshots, generated
packages, builds and logs. Packaging tools require output outside their source
snapshot, so place snapshot and outputs beside one another within that task
directory. Retain compact identities/results, publish the source closure, and
remove temporary artifacts at completion. Never force-push or rewrite history.
