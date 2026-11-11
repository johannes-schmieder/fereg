# Validation status

## Release 0.6.0, November 11, 2026

Johannes's instruction of 2026-11-11: merge the fedemean branch into main and
publish it as v0.6.0, a breaking release (the `solver()` grammar and `e()`
renames; `feppml` without `solver(map)`). The packaging commit `06a80511`
changes only version metadata against the qualified `44cd1563`:
`check_complexity.py --compare-production 44cd1563` differs only in
`Cargo.toml` and `Cargo.lock` (the version), the only other runtime change is
`ADO_VERSION`, and the Stata frontend differs only in version lines. All three
plugins were built from `06a80511` and carry `06a805119818` and the build date
2026-11-11.

**Corresponding source.** `cargo vendor --locked --versioned-dirs` gives 19
crates; the vendored CMG 0.3.0 matches its reviewed 126-file inventory
(`368eceba`). The dependency audit (cargo-audit 0.22.1, Rust 1.85.1) reports
0 advisories for `Cargo.lock` `56ef1738…`. `fereg-0.6.0-source.zip` is
`fb270ec4fc9ff05503f58b8755504a70ad5a9f46e7e6a1ce8021f3d280133f78`
(33,595,797 bytes, 2,720 manifest files). Its combined notices cover exactly
the vendored crates, Rayon and the Rust notices; the crates that only `within`
needed are gone.

**macOS.** Built offline from the extracted source archive with its
[Mac build instructions](macos_source_build.md) (fresh Cargo home, nothing
fetched, path remaps): universal plugin
`07c3e9c316b8c20b3795be0935694441a4edf5abd88678cae2b52a2c50be0b18`,
33,655,632 bytes; the binary audit and the actual ABI/selftest/panic harness
pass. This offline build is the release plugin.
- `run_tests.sh --package-root` on the staged candidate (Anaconda Python
  3.12.2) passes: the preflight, 361 Python tests, the package gate (34 files,
  runtime `d37bd747…`), 2,508 Rust results with none failing (fereg-core's
  library 1,557, the plugin's FFI tests 243, the allocator audits in fresh
  processes), both Clippy configurations, rustfmt, the fuzz check, the ratchet
  and the portable, static and mock-SPI checks. The Linux mock-plugin audit
  needs a GNU ELF linker.
- Whole tree on Stata/MP 19 (plugin staged alone): 157 pass, 0 fail, 3 not run
  (`staged_plugin_binary` and `feppml_installation_smoke` need an extracted
  package and pass below; `windows_ci_smoke` needs Windows).
- `staged-install`: 12 of 12 on the macOS-only archive (`88ada13f…`) and 12 of
  12 on the three-platform release archive. `older-stata` passes on Stata/MP 18.

**Linux, SCC `run-20261111r060`** (offline depth-2 snapshot of `06a80511` and
the workspace and fuzz vendor closure; the CMG inventory checked in each tree;
manifests verified from the run root before each job).
- Build 8574134 on scc-gr4 (failed 0, exit 0, 187 s): `build_linux.sh`'s ELF
  audit, actual-plugin harness and batch guard pass; plugin
  `f41fff17b172ee7bcaac2d900bd99ab0870e4884bf616ca556b8b59b830615ff`,
  23,981,728 bytes; Linux-only archive `eb5e6aa7…`.
- Tests 8574135 on scc-gr4 (failed 0, exit 0, 666 s): `run_tests.sh
  --package-root` with SCC Python 3.12.4 passes: 361 Python tests, the package
  gate, 2,523 Rust results with none failing (the golden hypot digest
  included), the ratchet and the Linux mock-plugin audit.
- Stata 8574136 on scc-wa3 (failed 0, exit 0, 1,126 s): whole tree on Stata/MP
  19 157 pass, 0 fail, 3 not run; `staged-install` 12 of 12 on the Linux-only
  archive; `older-stata` on Stata/MP 15.1 passes.

**Windows.** The accepted wrapper (`9ee64b16…`; controller `e88ed030…`,
diagnostic guard `14fe3f65…`) from a clean detached worktree of `06a80511`;
`inspect` PASS (roles-anywhere, stopped).
- `stata-smoke` `win-20261111T153930Z-d366751a`: PASS (archive `e5f17253…`,
  receipt `608165dc…`).
- `stata-do` `win-20261111T154102Z-6623cb14`: PASS in 1,454 s of the 1,800 s
  allowance, final stage `complete` (archive `a1dca4c9…`, receipt
  `1aa668c6…`). Every controller check passes, as do the Rust/MSVC build, the
  exact-arithmetic stage with the golden hypot digest, the C bridge, the PE
  audit, the native loader and the installed-package fixtures of the profile.
  Plugin `c72712af0373b91d81fefd45a93d9559eb68b0109287e7109f6c872ca147ecbf`,
  20,392,448 bytes, an x86-64 PE DLL.
- Both runs held the lock and ended with the instance stopped, the transfer
  objects deleted and the lock released.

**Package.** `fereg-0.6.0.zip` (`stage_release.py`, all three platforms, the
combined notices):
`305cbc229405924b597417a05a80325421e3a7fe6366fff27ed850980e67ad13`, 25,438,868
bytes, 47 files. `dist/` is refreshed from it (runtime `d37bd747…`, the source
archive hash, CMG `368eceba`), and `check_github_install.py` passes for 38
files.

Not covered: native Intel-Mac execution, the `staged-install` profile on a
sealed Windows archive, live Break during Windows loading (waived 2026-11-07)
and timing; the November campaign on `80b6768e` is descriptive.

## Exact elimination route and PPML elimination pre-solve, November 8, 2026

**Integrated locally into main on November 8; published in 0.6.0 (above).**
Johannes commissioned a performance workstream for OLS and PPML with two, three
and more categorical effects (branch `claude-need4speed` on `b5aad4ed`) and, on
November 8, accepted its decisions: a distinct reported solver identity for the
elimination (OLS `elimination`, code 9; PPML keeps the MAP engine identity
because the compiled PPML validator admits only the established engines and
the oldest supported Stata needed to rebuild it does not start on the Mac),
crediting a converged elimination pre-solve as the first of the two PPML passes,
keeping the strict checkpoint unchanged, and the declared rounding-level scope
of the shared fixed-tree reductions (SPEC §1, §13, §18).

**Scientific agreement.** Against 0.5.3 (`b5aad4ed`) on the 1M-row benchmark
panel (fourteen designs with two to five effects, low-mobility and hard graphs),
OLS coefficients agree within 1.2e-14 and SEs within 4.5e-10 relative, PPML
coefficients within 4.9e-14 and SEs within 6.7e-14; objectives within 9e-14;
samples and every discrete decision are identical. The frozen external reference
estimator agrees with 0.5.3 at the same distances (OLS SE 4.3e-10, PPML 7.6e-12),
and the branch's OLS SEs now sit at the reference's distance from 0.5.3 because
the elimination's stopping target (0.1×tol of the within norm) is stricter than
the established routes. Linux (SCC runs f, g, h on scc-yk4, scc-wo4, scc-zj4)
shows the same agreement. Fixed-worker repeats are exact; one, two, four and
eight workers agree scientifically.

**Independent witnesses.** The dense-QR and full-dummy oracles check the public
elimination route and rerun the established geometry-policy assertions unchanged
in `FEREG_BLOCK_SCHUR=0` child processes. A validation lane compared 72
specifications (weights, offsets/exposure, clusters, separation, saved outputs,
low memory, bias correction, four and five effects) against 0.5.3 and the
switch-off arms; its findings (iterate() accounting, a route change for explicit
preconditioners, infallible allocations in new parallel passes, the earlier
elimination stopping scale) were fixed before integration.

**Checks at the integration head.** `scripts/run_tests.sh --source-only`:
2,300 Rust test executions, 349 Python tests, the ten isolated allocator audits,
portable static checks, the complexity ratchet (no new size violation) and the
mock-SPI bridge tests pass. Stata fixtures: the 20 solver-report fixtures and
35 PPML `e(sample)` fixtures pass on the Mac package (one PPML fixture,
`parallel_input_oracle`, needs its solver-trace environment and is unchanged).
Not covered: Windows, native Intel-Mac and Stata 15 execution, the Linux mock
plugin audit (GNU ld) and an installed `github install` test of this runtime.

**Performance (4 threads, robust, 1M rows; medians).** Mac M2 Ultra (ms; 0.5.3 /
branch / external reference): ols2 182/58/60, akm 206/60/63, ols3 298/76/68,
ols4 358/87/91, ols5 425/97/105, ols3hard 447/72/73, ols3akm 487/90/181,
ppml2 1161/389/346, ppmlakm 1406/385/272, ppml3 3139/463/432, ppml4 2897/539/510,
ppml5 3478/616/476, ppml3hard 5629/589/456, ppml3akm 5741/636/1096. Linux
scc-zj4, run h (f8ff19ae; ms): ols2 562/203/176, ols3 886/273/188,
ols5 1137/369/284, ppml2 3110/1261/826, ppml3 9305/1704/886, ppml5 9488/2053/1109.
Later commits (`7059ac84`, `93b5bcba`) improve Mac PPML by a further 2-4%.
The Linux gap to the reference is serial work on single workers, Stata-side
time and memory footprint (RSS about 1.2 GB against 0.2 GB for ppml2), measured
with perf on scc-wo4 and scc-yj2.

## Bounded saved-effect recovery refinement, November 7, 2026

Johannes requested a performance refinement on integrated 0.5.2 (`7f6d87b1`).
The candidate gives strict categorical MAP at most eight sweeps and one quarter
of the shared recovery budget. A certified success uses the same fresh product
checks and component normalization as CG; otherwise the unchanged CG recurrence
starts from the original target with the remaining budget. Earlier results and
the accepted 0.5.2 performance exception remain unchanged.

Thirty recovery-module checks pass. Independent review covers certification,
workspace lifetime and budget accounting. The universal Mac candidate passes
binary/actual-interface checks, three focused Stata fixtures, and an independent
balanced-model public fast-path check with exact repeats and saved/unsaved fits.
Linux source integration passes: 2,308 Rust executions across 127 groups, 349
Python tests plus seven portable checks, complexity and actual-plugin ABI checks
(job 8498484). All four mapped Stata fixtures pass. The original million-row
three-effect saved-output reproducer passes default and tighter settings at
sixteen workers, with exact saved/unsaved fits and repeats (8498535). Maximum
row reconstruction errors are 1.469e-10 and 2.845e-13, respectively.

Performance preflight 8498485 reached the historical easy-case arm and stopped
because its 0.5.1 plugin received a 0.5.2 protocol header. Zero timing calls ran;
the failure, preceding science checks and complete accounting are preserved.
The first wrapper repair (8498879) corrected `ado` but missed the separate
`plugin` version field; it also stopped before any timing. Both failed attempts
and their science captures remain preserved. Source inspection confirms those
two version strings are the only release changes in the replay header. The
corrected adapter changes both and verifies every remaining input byte. Its
four-slot actual-plugin preflight (8503484) passes on the easy saved cell with all
three plugins at one and four workers, including protected exact outputs,
cross-worker science and the candidate's certified eight-sweep MAP path.
Accounting is complete: failure/exit both zero, 11 seconds, 612.277 MB on scc-gd4.
The unchanged 63-call comparison **passes** (8503585, isolated `scc-ul2`,
28 slots, 2,870 seconds, 2.961 GB maximum virtual memory; scheduler failure and
exit both zero). All prescribed calls are present: six repetitions and one
warmup per arm/cell. Recomputed comparisons use the frozen paired-median
bootstrap and max(5%, 1 ms) gate. Times below are individual-arm medians;
ratios are medians of paired ratios, so they need not equal ratios of medians.

| Cell (four native workers) | Current 0.5.2 | Candidate | Paired ratio [95% interval] | Gate |
| --- | ---: | ---: | ---: | --- |
| Two-effect, 1m rows, no save | 5.592 s | 5.611 s | 1.0053 [0.9921, 1.0115] | Pass |
| Three-effect, 1m rows, no save | 15.762 s | 15.775 s | 0.9960 [0.9875, 1.0142] | Pass |
| Easy saved effects, 10k rows | 56.437 ms | 53.134 ms | 0.9433 [0.9318, 0.9520] | Pass |
| Difficult saved effects, 1m rows | 95.118 s | 97.235 s | 1.0208 [1.0096, 1.0262] | Pass |

In the same three-arm easy-case schedule, the historical 0.5.1 median is
52.347 ms. Candidate/legacy is 1.0147 [1.0097, 1.0237], also within the gate.
The refinement removes the earlier large easy-case regression under this
criterion: 5.7% faster than the integrated repair, with a remaining 1.5%
historical gap. It costs 2.1% on the difficult saved case; it is not cost-free.
The original 10.2% regression and both zero-timing protocol failures remain
preserved, with no resampling of adverse measurements.

All 63 native calls retain exact repeats and accepted certificates. Protected
cross-arm fields and discrete decisions pass; worker 1/4 scientific comparisons
pass. Traces verify certified eight-sweep MAP on the easy cell and eight-sweep
exhaustion followed by original-target CG on the difficult cell. The 48 host
censuses and 564 process observations pass isolation checks. Collected compact
files match their remote hashes; frozen source, binaries, tools, inputs and
pins are verified. Row reconstruction and sample checks ran on node scratch;
raw row arrays are intentionally not retained.

This is one physical Linux host, so intervals describe within-host variation.
Johannes accepted the refinement and all reported trade-offs and authorized
commit and push to main. **Published as [v0.5.3](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.3)
and pushed to main.** The outreach campaign stays stopped.
The release-version universal Mac offline build and four mapped Stata fixtures
pass. Windows build, binary audit and the licensed Stata driver pass from
`1974e383`; the instance stopped, transfer objects were deleted and the lock
released. Both release binaries retain the measured numerical code.

Release integration 8506576 stopped on an existing three-effect screen test
(1,585 passes, one failure); its original log and accounting remain retained.
An isolated diagnostic passes (8506725). A deterministic one-worker witness
(8506789) reproduces contamination of the test-only `UNSCREENED` switch when
a cached Rayon worker executes another installed action. The original assertion
detail was not captured. Commit `c788fac0` isolates both exactness comparisons
using the existing subprocess helper, preserving all fixtures, worker counts
and assertions and the production runtime identity. The repaired integration
passes (8506863, scc-gd4, eight slots, 599 seconds, 16.839 GB maximum virtual
memory; scheduler failure and exit zero): 2,310 Rust executions across 129
result groups, 349 Python tests and seven portable checks. Both exactness
witnesses, the native ABI and all five mapped Stata fixtures pass.

Root package admission passes for all 37 files, runtime identity, hashes and
Mata bindings. A fresh isolated local install checks every installed byte,
native selftest, independent OLS coefficients/SEs and PPML group means. Five
Stata fixtures also pass from the exact three-platform native archive. The
source closure has zero advisories and matches the successful offline Mac
build except for test isolation, the fixture terminal marker and source metadata.
The five published assets download with the qualified hashes. Fresh HTTP
installs from the release tag, main and the actual `github install` command each
verify all 37 installed files, native selftest and independent OLS/PPML checks.
The GitHub helper initially stopped before installation because the isolated
PLUS lacked its `g` database directory; creating that directory fixes the test
setup without changing the package. The failed log is retained. The release
list is 258,135 bytes, below the 264,392-character helper limit. Native Intel-Mac
execution remains unqualified.
The completed monitor was deleted before collection. Full compact evidence is
retained in the existing external `ppml-repair-20261107/refinement` archive;
all observations, hashes and checks are indexed in the
[refinement receipt](../bench/results/ppml-recovery-refinement-20261107.json).

## Combined Windows/PPML release 0.5.2, November 7, 2026

**Published as v0.5.2 and merged into main (PR #36).** Johannes requested
integrating both changes and directed continuation after the saved-effects
slowdown was disclosed. This accepts that performance exception; the recorded
gate remains **FAIL** (paired ratio 1.1020). No new timings were collected. See the [combined artifact checks](../bench/results/windows-ppml-20261107.json)
and the original performance table below.

The combined runtime identity is
`43110fefd9b65be5ffa9ad66e81ef0f8d1833f13246bab0aebf405e3677f682e`.
Its Windows build passed the complete private profile at `8b84b515` in
`win-20261107T120349Z-258842f3`: actual installed checks, numeric-input oracles
at two and eight workers, saved-intercept CG recovery at both worker counts,
scientific/transaction checks and cleanup. Loading Break retains Johannes's
waiver; the machine stopped and all transient transfer objects were deleted.

The Mac offline build passed the native audit and 77 independent replay cases
at one, two and four workers, with exact fixed-worker repeats at two and four.
Against the current release, 75 cases remain exact. Two saved-PPML cases change
only the declared saved channels and reconstruction certificate magnitudes;
coefficients, covariance, sample and all discrete decisions stay exact.
The disconnected saved-effects witness reaches CG four times, checks independent
model truth and exact saved/unsaved fits at 1e-8 and 1e-12, and passes from the
assembled package. Public PPML live Break/rollback also passes on Mac.

The default `./scripts/run_tests.sh` passes with the matching three native
plugins: 2,291 Rust executions in 126 groups, 349 Python unit tests plus seven
portable checks, FFI/allocator audits, complexity and installed-package binding.
A fresh local root installation verifies all 37 installed file hashes, native
selftest and independent OLS/PPML group-mean examples. Four checks pass from the
complete native archive. The corresponding-source closure has zero advisories;
all 2,775 compiler-input files match the successful offline Mac build exactly.
The published source/native archives, checksums and build instructions download
with exactly the qualified hashes. Fresh HTTP installations from the release
tag, main and the actual `github install` command each verify all 37 file hashes,
native selftest and independent OLS/PPML examples. The release-list response is
246,965 bytes, below the helper's 264,392-character limit. Native Intel-Mac
execution remains unqualified.

Linux native/ABI checks and six mapped PPML fixtures pass, as do the strict
two-effect million-row fit and the three-effect 1e-8 reconstruction/exact-repeat
checks at sixteen workers. The final 1e-12 saved-effects continuation also
passes, with maximum row error 2.845e-13 and exact saved/unsaved and repeated
outputs (job 8498067, scc-ei3, 186 seconds, 4.364 GB; scheduler failure and exit
status both zero). The two earlier launcher/resource-limit failures remain
recorded; all checks used the same binary and inputs.

## PPML repair development evidence, November 7, 2026

At Johannes's request, outreach array 8481934 and its continuation monitor were
stopped before investigating the native failures. Original measurements remain
frozen. This source candidate repairs two separate boundaries:

- A rejected categorical profiling curve now retires, allowing the existing
  independent joint strict witness to run. Acceptance tolerances are unchanged.
- Saved categorical effects can use coefficient-backed CG recurrences and one
  fresh original-target residual restart within the existing iteration budget.
  Fresh reconstruction and FE certificates, final-Hessian component normalization,
  memory admission and transactional publication remain required. Ordinary CG,
  OLS, slopes, successful direct recovery and fits without saved effects retain
  their established paths.

Independent weak/disconnected three-effect witnesses cover exact repeats,
worker changes, target/weight scaling, a zero target and rejection outside the FE
span. Source-only integration passed on Linux (job 8496488, 782 seconds,
17.353 GB), including 2,305 Rust test executions across 126 groups, Python checks,
Clippy, portable and actual-interface audits. Six mapped Linux Stata fixtures and
two focused Mac Stata fixtures pass. The Mac binary passes universal-binary and
actual-plugin ABI checks. These results do not qualify an installed package for
release or promotion.

The original million-row two-effect strict failure passes at tolerance 1e-12.
The million-row three-effect saved-output case passes both default and tighter
settings at four workers (8496362) and the original sixteen workers (8496732).
At sixteen workers, maximum per-row reconstruction errors are 1.469e-10 and
2.845e-13; both profiles repeat exactly. Saving effects preserves coefficients,
covariance, retained sample, aggregate absorbed prediction and eta exactly
against the corresponding unsaved candidate fit.

The one fixed performance check used the current base revision and candidate in
one isolated Linux allocation (8496519, scc-zj4), with six alternating pairs plus
one warmup per arm/cell. All 42 calls are retained. All science, fixed-worker
repeatability and cross-worker gates passed. The maintained PPML timing runner
`bench/time_exact.py` supplied the paired bootstrap and max(5%, 1 ms) bound;
`bench/qualify.py` is currently OLS-specific.

| Cell, four workers | Incumbent median | Candidate median | Paired median ratio | Bootstrap 95% ratio | Gate |
| --- | ---: | ---: | ---: | --- | --- |
| Two FE, 1m, no saved channels | 5.605 s | 5.500 s | 0.9693 | [0.9019, 1.0024] | Pass |
| Three FE, 1m, no saved channels | 16.267 s | 16.096 s | 0.9866 | [0.9683, 1.0058] | Pass |
| Three FE, 10k, saved channels | 68.612 ms | 75.006 ms | 1.1020 | [1.0259, 1.1570] | Fail |

The easy saved-effect cell fails the nonregression gate. The timing job exits 1
for that measured outcome; scheduler failure is zero and host isolation passes.
Retained phase captures are consistent with extra recovery work (11.698 versus
16.547 ms in the traced four-worker preflight), but these single calls are not
additional timing evidence. A bounded existing MAP recovery attempt before CG
is a proposed refinement, not implemented or accepted here. No adverse timings
were resampled. Johannes subsequently directed continuation after disclosure
of the slowdown, accepting it for the combined 0.5.2 release above. The measured
performance gate remains Fail; the proposed refinement was not implemented.

[The compact record](../bench/results/ppml-recovery-20261107.json) retains source
and dependency identities, binary/input hashes, complete timing observations,
science scope, scheduler outcomes and preserved operational failures. The SCC
run identifier is `20261107-ppml-diagnosis`. This development evidence does not
replace the historical paper benchmarks.

## Windows parallel-input candidate, November 7, 2026

**Standalone qualification with the loading-Break waiver.** The Windows
x86-64 Stata/MP 19 GUI candidate at `ef8fa126` passed the full exact-source
Windows profile in run `win-20261107T111500Z-ecc7882e`. The guarded collector
returned the tested binary; its SHA-256 is
`a9baa4333b31504bf0d0af147e65f4c2838686a2689f205db4096c919676279d`
(12,247,040 bytes). Collection independently verified that hash. The machine
stopped, transient transfer objects were deleted and the shared lock released.
The [qualification record](../bench/results/windows-input-20261107.json)
retains the exact source/archive/receipt identities and all earlier attempts.
The combined release above supersedes this standalone binary.

Johannes explicitly waived Break during Windows loading on November 7. The
waiver permits delayed or unavailable cancellation during loading; it changes
no polling code, scientific criterion, failure rollback or cancellation
requirement outside Windows loading. The failed experimental GUI/Break helpers
were removed from the profile. Live Windows loading Break remains unverified.

The only runtime change is one Windows x86-64 Stata/MP 19 GUI host-policy row;
the loader algorithm and scientific arithmetic are unchanged. Other Windows
hosts retain serial input. The actual Rust/MSVC build, embedded source identity,
exact-FMA/fused-kernel witnesses, C numeric-window guards, PE audit, native
loader and installed-plugin checksum checks passed. All mapped OLS/PPML
scientific and transaction fixtures passed, including the 80-cell missing-value
witness, worker panic/read-failure handling and recovery, and the input oracle
at two and eight workers. Each oracle required 72 cases and 208 runs, at least
72 actual parallel-window runs, exact serial/parallel input digests and the
existing independent scientific tolerances. No Windows performance claim is made.

The full local source-only suite passed (2,289 Rust test executions, 349 Python
tests and seven portable checks), as did the strengthened Apple Silicon live
input-Break fixture using the unchanged installed Mac plugin. That fixture
checks PPML covariance/sample rollback and an injected reader failure before
same-session exact serial/parallel recovery. The focused Windows diagnostic
tests, complexity ratchet, documentation checks and independent candidate review
also passed. Main and the 0.5.1 binaries still use serial Windows input. The
candidate is ready for integration with the concurrent PPML work; a matching
native package, corresponding source and strict root-package/HTTP installation
admission remain required before a combined release.

The ten earlier runs remain failures. COM activation did not complete, and
GUI probes could not find an owned visible window for Break delivery. At
`ef1825eb` the native/scientific checks passed before the deferred GUI-probe
failure; that partial evidence was not an overall pass. At `16ff0ae0` the
normal-GUI child reached its token-bound readiness marker but again exposed no
visible top-level window (`r(85010)`). Break was never delivered, so these
attempts do not establish a loader cancellation defect. Every run stopped the
machine, deleted transient transfers and released the lock. Existing Stata
Automation was registered through `/Register`; that registration remains, with
no identity, DCOM-permission, networking, IAM, Stata executable or license changes.

## CMG 0.2.0 consolidation, November 6, 2026

The published [0.5.1 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.1) uses CMG
`e5613e81fb3b03c78de98658b372646fa802aabb`; all three native plugins were built
from fereg `84e8973768cfcd54aaf82e584ae2d1a8bd06ff3e`. The required source suite,
77-case scientific replay, 74 Mac Stata fixtures, three Break profiles, SCC
Linux qualification and private Windows qualification pass. Original-system
certificates, fixed-worker repeatability and cross-worker scientific checks
retain their registered criteria. Fresh/replacement root installation on Mac
verifies all 37 files and independent OLS/Poisson results; exact staged ZIP
installation passes on Mac and Linux. Actual published tag/main, `github install`,
and downloaded native-ZIP fresh/replacement installations pass on Apple Silicon
Stata/MP 19: all 37 repository-package files (36 in the native ZIP), selftest and
independent OLS/Poisson results match. A modified help file is replaced correctly.
The [publication record](../bench/results/cmg-kernels-20261106-publication.json)
retains installed and downloaded-asset hashes. The release tag remains immutable;
subsequent commits only record publication evidence.

The primary Linux performance panel passes all 12 cells on six independent
physical hosts. The Mac panel passes 10 cells; weak-auto-t4 and weak-cmg-t4 are
inconclusive, with median candidate/reference ratios 0.997787 and 1.007402.
Their allowance-adjusted excess intervals are [-2.175, +1.457] ms and
[-55.088, +20.342] ms. These are uncertainty intervals on excess time, not
ratio intervals. Johannes explicitly accepted these two inconclusive results as
release exceptions on November 6; their measured status remains unchanged.
The [acceptance record](../bench/results/cmg-kernels-20261106-acceptance.json)
binds the original evidence. No samples were repeated or excluded. The common library revision retains fereg's
32-level setting and existing component options.

[Structured evidence](../bench/results/cmg-kernels-20261106.json) and its
[Mac measurements](../bench/results/cmg-kernels-20261106-macos-measurements.json)
and [Linux measurements](../bench/results/cmg-kernels-20261106-linux-measurements.json)
retain all 1,032 warmup/measured calls. The earlier consolidation candidate's
inconclusive results remain unchanged in the adjacent November 6 records.
Native Intel-host Stata is unqualified; Windows retains its documented serial
input/Break limitation. This bounded panel makes no PPML performance claim.

## Live SSC installation, November 6, 2026

A fresh Apple Silicon Mac Stata/MP 19 session successfully ran `ssc describe
fereg` and `ssc install fereg` against the live SSC server. The listing identifies
Johannes F. Schmieder's package with distribution date `20261106`; the installed
ado and native runtime versions are both `0.5.0`. PLUS, PERSONAL, SITE and
OLDPLACE were isolated, and `which fereg` failed before installation. All 30
installed files match the corresponding current release files byte-for-byte,
including the repaired loader. SSC selects the Mac plugin for this platform;
its checksum matches the installed sidecar.

The verbose native self-test and maintained `installation_smoke.do` fixture
passed. A two-group PPML fit matched its independent closed-form coefficients
and conditional means within `1e-8`, with `predict` matching the requested saved
means within `1e-12`. On Stata's auto data, absorbed OLS and PPML both used 69
observations, and their mpg coefficients matched Stata's `regress` and `poisson`
with explicit repair-record dummies within `1e-8`. Reinstallation with `replace`
and a subsequent plain installation both reported the package up to date.
The test completed with Stata return code zero and its terminal success marker.
This verifies live SSC installation on Apple Silicon Mac Stata 19; no new Linux,
Windows, Intel-Mac or Stata 15 execution is claimed.

SHA-256 identities:

- Live SSC manifest: `dd3c0686d2301351232713a1ef995bddf9c1da5fa7f7fffe52b3c550a500962d`.
- Installed Mac plugin: `2c8f6af8e10653a675c693e98560e94648e2f795ce5dfc605cf3f93f617d2200`.
- Installed loader: `766857e25b1e441af058938e9de0574a412ee048cdbb6741c8ff4f72d49b6735`.
- Test script: `bc53795f2179e85a18488993d1fc0f784b70c67fd0345e26c039ed070fe537f7`.
- Successful Stata log: `1e29c410b7b9f0619d979b4fa65021641b979187a7386a76bc605be76676c0b1`.

## Home-directory plugin loader repair, November 4, 2026

The loader now resolves the filename returned by `findfile` before registering
the native plugin. Its private helper stays in `_fereg_load.ado`; the installed
file inventory is unchanged. It uses program-local macros, preserves the
working directory, and defers Break only across the short directory-resolution
block. The final implementation strips the basename without simplifying the
directory: the new regression exposed that `pathgetparent()` prematurely
reduces `~/../`, losing the home-directory meaning before `cd` expands it.
No native code, binary, scientific arithmetic or solver routing changes.

The maintained `plugin_paths.do` fixture fails on the incumbent with `r(601)`
and passes on Mac Stata/MP 19 after the repair. It covers tilde plus parent
traversal, absolute, relative and parent-traversal paths containing spaces,
fresh plugin registration, exact working-directory/Mata-variable preservation,
and independent two-FE OLS/PPML coefficient witnesses. It is included in the
quick, plugin-load and full Stata profiles. The existing 100-reload, OLS
transaction and PPML command fixtures also pass. A fresh local root-package
installation verifies all 37 installed file hashes, deliberately resolves both
the loader and plugin through `~/...`, and passes the path fixture and an
independent dummy-variable PPML fixture.

`scripts/run_tests.sh` passes with Python 3.12, including all 349 Python tests,
Rust/FFI/allocator checks, portable C/SPI checks and the unchanged complexity
ratchet. The initial invocation selected system Python 3.9 and stopped on
missing modules before native checks; rerunning with the documented Python
3.11-or-newer environment resolved that environment error. The released Mac
plugin passes its binary audit and actual ABI/selftest/panic-containment check.
This repair's licensed execution is Apple Silicon Mac Stata 19; no new Linux,
Windows, native Intel-Mac or Stata 15 execution is claimed.

At Johannes's explicit request, main and the existing v0.5.0 tag and release
archives now include the repair. The [publication record](../bench/results/v050-publication-20261103.json)
retains the previous tag and all six previous asset identities, the seven
replacement asset identities, test hashes and the fresh checks. All 3,628 source
manifest entries verify, and all 2,695 native/dependency source files match the
prior source archive. A fresh offline universal Mac build from the new source
archive passes its binary audit and actual ABI tests; its rebuilt plugin is not
distributed. All three released plugin binaries remain exact.

Fresh local binary-archive and SSC-layout installations verify all 36 and 34
installed files, respectively, and pass tilde-path loading and independent
OLS/PPML checks. Four staged fixtures pass, including plugin deletion and prior
estimate preservation. Fresh HTTP installations from main and v0.5.0, and the
actual `github install ..., replace` command, each verify all 37 installed
files and the same path/scientific checks. The first GitHub-helper attempt
stopped before package download because its isolated PLUS/g directory was
missing; creating the helper in the correct isolated directory resolved the
harness error. All seven unauthenticated public asset downloads match their
prepared hashes. The release-list response is 204,332 bytes, below Stata's
264,392-byte limit. Beta releases were not changed.

## Home-directory plugin path investigation, November 4, 2026

Mauricio Caceres Bravo's [issue #35](https://github.com/johannes-schmieder/fereg/issues/35)
is reproduced on Apple Silicon Mac Stata/MP 19 at `083cd5c7`.
With a `~/...` ado-path entry, `findfile` returns that spelling and `confirm
file` succeeds, but `program, plugin using()` and `fereg, selftest` return
`r(601)`. The same released plugin loads through an absolute path. The shared
loader passes `r(fn)` directly to `program, plugin`; the isolated installation
runners use absolute paths and do not exercise this case. Mata's
`pathresolve(pwd(), path)` alone does not expand `~` in this host.

A temporary copy of the loader with Mauricio's proposed resolver passes
selftest and independent no-FE OLS/PPML coefficient checks, including a path
containing spaces. His literal helper overwrites existing Mata variables
named `dir` and `base`. An adjusted temporary helper instead uses
`pathgetparent()` and `pathbasename()` through `st_local()`, encloses the
directory change and restoration in `nobreak`, and restores the original
directory before propagating a captured path-construction error.
It passes tilde, absolute, `./`, `../` and bare-filename checks; invalid-directory
failure returns `r(170)` without changing the working directory. Existing Mata
variables remain exact. Two-way categorical-FE OLS and PPML coefficients agree
with independent Stata dummy-variable regressions at relative tolerances
`1e-10` and `1e-7`, respectively. Probe development corrected matrix-expression
syntax and an already-dropped plugin-handle assumption; these were harness
errors, not additional product failures.

At the investigation checkpoint the production loader, native binaries and
release archives were unchanged. Only Mac Stata 19 was
executed; Stata 15, Linux and Windows path handling remain untested here.
The subsequent frontend repair and maintained regression test are recorded
above. No native numerical change is needed.

SHA-256 identities:

- Released Mac plugin: `2c8f6af8e10653a675c693e98560e94648e2f795ce5dfc605cf3f93f617d2200`.
- Adjusted temporary loader: `99ff4b1d9de4bedb177a167ac6508fa1387509978ed208e9b90078045de2b9ab`.
- Original-suggestion probe log: `b7d8abc357dc75d2a4556947cefa55bf2e1af7a01009436d3ff1a073f5f11e8e`.
- Adjusted-resolver probe log: `f391f3245bd2f7e80086eb086c740edac37ec48f4427aa5b2df7fa348603e61f`.

## PPML display update, November 3, 2026

`feppml` now reports convergence iterations, effective N, residual DoF, Wald
chi-square and p-value, pseudo-R², log pseudolikelihood, deviance and cluster
counts, followed by the absorbed-DoF table. OLS and PPML share an installed
Stata display helper; the OLS summary and absorbed-table implementations are
unchanged. PPML calls it at the existing reporting point after transaction
commit and on replay. No native code, binary, Mata object, scientific result
or timing boundary changes; prior qualification remains applicable.

On Apple Silicon Mac Stata/MP 19, `replay_and_prefixes.do` passes its eight OLS
variants plus seven PPML variants covering two FEs, two-way clustering,
absorbed slopes, fweights, no FEs, intercept-only with missing Wald inference,
and long names. It checks printed values, missing tests, 90% intervals,
prefix behavior, and exact preservation of coefficients, covariance, fit,
DoF and sample on replay. `feppml_command_contract.do` also passes after
updating its stale beta-only version assertion to 0.5.0.

All 34 focused help/package Python tests pass, including the updated helper
inventory. A fresh local net installation verifies all 32 installed file hashes,
loads the helper from its isolated installed directory, and passes selftest,
OLS/PPML calls and an independent dummy-variable Poisson coefficient check.
The root package check passes with 37 files and unchanged native identity,
plugin hashes and Mata objects; `git diff --check` passes. The Mac plugin is
identical to the OLS display record below. No native rebuild or new numerical
or performance campaign was needed; Linux and Windows were not rerun.
The PPML help examples are unchanged and the reviewed help hash is updated.

Passing Stata log SHA-256 identities:

- Isolated installation: `4b0c4ac3149d540eb7d32202de4fbc504b0f47ae0d9c46eeedaed70eb6d3f1d8`.
- `feppml_command_contract.do`: `8c6b29b0e3720f338cd9266269f0be662976cbaf6eeddfd081b617e774041aaa`.
- `replay_and_prefixes.do`: `b6f194b6844a8178a6ca4b34869d31c2972faa82a3cb29a022d3327976f964ae`.

## OLS display update, November 3, 2026

`fereg.ado` now displays a reghdfe-style fit summary, cluster counts and an
absorbed-DoF table from the existing posted results. The solver diagnostic
and coefficient table remain, and prefix replay retains Stata's inference
display. Estimation adapters, native code, binaries and result semantics are
unchanged; prior scientific and performance qualification remains applicable.

The maintained `replay_and_prefixes.do` fixture passes on Apple Silicon Mac
Stata/MP 19 with the released plugin. It checks eight display variants: two
categorical FEs, robust one-FE without a constant, two-way clustering, absorbed
slopes, three FEs, no FEs, intercept-only with missing F, and long FE/cluster
names. Printed statistics and DoF rows match stored results; replay preserves
coefficients, covariance, fit and DoF matrices and sample membership exactly.
The fixture also covers saved-estimate replay, 90% intervals and coefficient
formatting, bootstrap/jackknife, independent OLS/PPML coefficient witnesses,
selftest and saved outputs. Its old beta-only version assertion was updated
for the stable release. During test development, incorrect assumptions about
returned `r(table)` and matrix-expression syntax were corrected; no scientific
tolerance or runtime behavior changed.

The 36-file installation check confirms the unchanged native runtime identity,
plugin hashes and Mata objects; `git diff --check` passes. No native rebuild,
performance campaign or Linux/Windows rerun is required for this display-only
change. Mac plugin SHA-256:
`2c8f6af8e10653a675c693e98560e94648e2f795ce5dfc605cf3f93f617d2200`.
Passing Stata fixture log SHA-256:
`e7a298cb23c42fd91d4c9a17bee6211745e3d6d24a1f39c873ec2630c701d7db`.

## Public documentation review, November 3, 2026

The post-release review reconciles the README, installation and user guides,
installed help, benchmark summaries, protocol version descriptions and dependency
notices with the released 0.5.0 runtime and frozen evidence. The public help's
19 executable example blocks are unchanged; all 18 non-benchmark examples pass
on Mac Stata 19 through the two maintained documentation fixtures. The optional
benchmark example was not rerun. All 16 help-contract tests pass, and the
36-file installation check confirms that the native runtime identity is unchanged.
Relative links and anchors across the 28 maintained documentation pages pass,
as does `git diff --check`. The reviewed PPML help hash is retained in
`scripts/feppml_help_contract.py`. The fixed release tag and archives retain
their original bytes; the release page links the documentation corrections.

## Stable 0.5.0 promotion

On November 3 Johannes authorized the regular v0.5.0 release. It uses the exact
beta.15 native binaries on all three platforms and the same native runtime source.
The only executable frontend changes replace the selftest and posted OLS/PPML
release labels with `0.5.0`; package metadata, help and installation links are
updated. Prior native scientific and platform qualification therefore remains
applicable, including its limitations and accepted performance regressions.

The [stable package verification](../bench/results/v050-publication-20261103.json)
retains the plugin and archive identities, zero-advisory dependency audit, 51
passing focused Python tests, 3,624 verified source-archive entries, and a fresh
offline universal Mac build with binary/ABI checks. The rebuilt artifact is a
source-closure check; the distributed binaries remain exactly beta.15.
The stable archive passes installation and independent PPML fixtures; a fresh
root installation verifies all 36 file hashes, stable selftest/OLS/PPML labels,
and independent OLS/PPML coefficients, covariance and sample checks.

[Version 0.5.0](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0)
is published as the regular Latest release. All five unauthenticated asset
downloads match their retained hashes; beta.15 and its assets remain unchanged.
Fresh Mac Stata 19 installations from the stable tag, `main` and the community
`github install` command pass all 36 installed-byte checks and independent
OLS/PPML checks. The release list is 234,266 bytes (198,376 in Stata's compact
response), below the 264,392-byte limit. The first community-helper attempt
returned `r(132)` after installation; direct and nested query diagnostics then
passed without changes, followed by one successful fresh installation. Its
cause is not established; the initial failure hash is retained in the record.

## Beta.15 publication decision

On November 2, after reviewing the completed no-FE performance evidence,
Johannes instructed: “accept and publish this candidate.” This explicitly accepts
the Linux 16-regressor, one-worker +7.3% performance exception for the frozen
candidate. The original failed guard and all timing observations remain unchanged.
It does not waive scientific checks or claim that fereg consistently beats regress.
The release uses the accepted runtime, with release-label and qualification-driver
updates only. [Beta.15](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.15)
is published, and the corresponding-source, platform native, strict integration
and installed-package checks pass as recorded below.
Windows keeps serial input; native Intel-Mac Stata execution remains untested.

The [beta.15 package verification](../bench/results/beta15-publication-20261102.json)
records the passing strict `scripts/run_tests.sh`, all three qualified plugins,
zero dependency advisories and 3,587 verified corresponding-source manifest
entries. The final source archive builds offline and passes binary/ABI checks.
The native build commit and packaging source differ only in the README download
link; all 431 compared native-source files match the timed candidate exactly.
Both Mac and Linux release plugins pass all 77 replay cases and reproduce the
timed candidate's 30 no-FE scientific exports exactly. Their four mapped Stata
fixtures and 90-call independent scientific checks pass.

Windows run `win-20261102T192458Z-d0678b9d` passes its exact-source native build,
FMA/kernel/ABI checks, installed-plugin checksum, existing OLS/PPML/transaction
fixtures and both new no-FE fixtures. The machine is stopped, its transient
objects deleted and lock released. Linux native job 8401878 exits zero on scc-ei3.
Fresh root installations on Mac Stata 19 and Linux Stata 15/19 verify all 36
installed files' bytes, beta.15 selftest, the no-FE derived row, sample membership and
independent OLS/PPML coefficient and covariance witnesses. Linux installation
job 8402322 exits zero on scc-pi1. The exact binary archive also passes Mac
installation and independent PPML smoke checks.

All five public release assets match their local hashes when downloaded without
authentication. The release-list response is 185,417 bytes, below the 264,392-byte
installation-helper limit. Fresh HTTP installations from `main` and the beta.15
tag pass on Mac and Linux Stata 19; Mac's actual `github install` command also
passes. Each checks all 36 installed files, selftest, sample membership, the
no-FE derived row and independent OLS/PPML coefficients and covariance. Linux
HTTP job 8402509 exits zero on scc-pi1 (21 seconds, 1.829 GB maximum virtual memory).

The temporary installation driver needed two bounded repairs: Stata 15 matrix
expression syntax and an incorrect model-based Poisson covariance reference for
robust-default PPML. The corrected driver uses the independent score sandwich
with the specified N/(N-1) correction and unchanged tolerances. An optional Mac
Stata 15 console attempt cannot locate its local Stata directory and supplies
no qualification evidence; Stata 15 qualification here is Linux. The Windows
wrapper's initial clean-tree check rejected unrelated untracked files before
starting the host; the clean source snapshot passed. The initial `github install`
test setup omitted the helper's standard `PLUS/g` directory and failed before
package installation; restoring that layout makes the unchanged helper pass.
None of these repairs changes fereg or the frozen performance observations.

## Revised no-FE candidate: preparation and loader integration

The [completed revised report](../bench/results/no-fe-preparation-20261102.md)
retains all 1,080 timing observations across Mac and Linux. The candidate
**did not pass the automatic performance gate**: Linux's 16-regressor, one-worker command median
rises from 1,367.0 to 1,466.3 ms (+7.3%), failing the max(5%, 1 ms) guard.
The other 14 Linux guards and all 15 Mac guards pass. Every parallel cell improves
relative to beta.14. At 64 regressors and 16 Rust workers, Linux falls from
3,100.5 to 708.9 ms and Mac from 1,156.7 to 315.6 ms. Regress takes 748.8/297.3 ms
respectively, but its licenses cap it at four/eight processors. At equal four
workers on Linux, the candidate still takes 1,778.3 ms versus 751.7 ms for regress.
There was no timing retry or exclusion. Johannes subsequently accepted this
performance exception for publication, as recorded above.

The final source gate passes, including 1,582 core unit tests (three ignored),
portable paths, allocator/FFI/SPI audits and the reviewed complexity ratchet.
Both native plugins pass ABI checks, all 77 unaffected scientific replay cases
exactly and both actual Stata fixtures. All 630 scientific exports per platform
pass the unchanged independent QR references, fixed-worker exact repetition,
cross-worker scientific agreement and stable discrete decisions. Actual traces
confirm packed-row and summary adoption for every multithreaded target cell;
serial cells retain the existing loader. Maximum coefficient/covariance absolute
errors are 3.253e-14/1.589e-19 and maximum relative RSS error is 4.974e-14.

Linux build 8400668 completed on scc-ei3 (296 s); the sole timing job 8400671
completed on scc-yj3 (670 s, 28-slot omp28 plus num_proc=28). Both exited zero,
and the timing report returned the retained performance-failure decision.
Whole-node reservation and process/census coverage passed. The frozen source
hash is `ad2188f8bfa03bbafe3967bfc1ed90807dcc9f32cff2479012ca84b16c0f528b`;
Mac/Linux plugin hashes are
`ee8b2c3e3e7d1b3748405322da2832b330be41285eee849fbf27ce4efd586e0f` /
`0808d720d54061fbbf6bbcb8914ccb7370daf9bd8a2786a813b883a1c1ad4951`.
One Linux host supplies descriptive evidence, not multi-host qualification.
The load counter now includes packed-row production; stable summary construction
runs after loading, alongside preparation. Native-counter gains therefore include
some relocated work, and full-command time governs the decision. Neither that
counter nor regress's command timer isolates the small-matrix solver.

Johannes authorized one revised candidate after the retained Mac serial
regressions and inconclusive Linux run. The eligible family, numerical scope,
canonical rank authority, tolerances, output exclusions and performance guards
above beta.14 remain unchanged. The revised work first targets exact bulk
high/low centering and packing, with dimensions checked at the bulk boundary,
and actual no-FE packed loading with exact owner/sample binding. No raw-total
subtraction, broad fast-math, benchmark-specific routing or weaker certificate
is permitted. A reduced summary/score scan requires its own mathematical
argument and independent witnesses before implementation.

The supplied assembly audit identifies existing packed NEON/AVX FMA product
loops and repeated scalar `center_value` calls during packing; it makes no
speedup claim. One bounded Mac function-sampling diagnostic uses the frozen
centered binary, the existing million-row input, 64 regressors and one worker:
12 complete fits, a single ten-second sample at one-millisecond cadence, no
repeat or timing qualification. This establishes a cost distribution with
sampling uncertainty before selecting the coherent candidate. Exact packing
witnesses cover selected/permuted columns, all panel tails, extreme scales,
nonfinite refusal, cancellation and arithmetic equality. Loader witnesses cover
full/missing/restricted samples, memory refusal, SPI/Break/rollback and exact
adoption. Actual trace evidence must show any claimed handoff. The final
candidate receives one frozen declaration, scientific preflight and bounded
qualification; prior evidence is never replaced.

The diagnostic completed all 12 fits. Of 5,874 sampled worker stacks inside
native fitting, 1,612 were in moment building; about 568 of those involved
scalar row filling/centering. The three summary scans accounted for approximately
1,910 stacks; `center_value` itself appeared in 1,265 samples across fitting.
These overlapping, correlated stack counts describe one ten-second window;
they are not independent timing observations or a measured speedup. The sample
capture SHA-256 was
`425b1936cbdd69df1632ae58d0f41a2ab52d3b37ae05f0da7af148d330a76480`.

The revision preserves the stable three-scan summary. With multiple workers,
the existing loader now packs eligible no-FE rows and computes that summary
alongside model preparation, followed by exact sample/value binding at adoption.
Single-worker and inadmissible requests retain fit-time packing and summaries.
Bulk panel preparation retains both `two_sum` operations and division order,
uses immutable extrema to guard finite subtraction, and replays the original
scalar tile on a numerical refusal. Continuous/intercept designs compute the
two separately ordered centered/raw FMA residuals during one traversal; factor
designs retain their original path. This removes one raw-design traversal while
retaining the residual-vector validation at its original error boundary and the
independent compensated original-score pass. The dead factor workspace is
released before both residual vectors coexist. Independent review found no
scientific, scope, error-order or admitted-memory blocker in these changes.

The first actual revised-binary preflight caught a missing wide-descriptor
handoff before timing: 64 regressors plus the intercept exceed the existing
64-column stack binding, so those rows and summaries were correctly refused
but rebuilt unnecessarily. The preflight binary
`6df227f63ec7ace649c0debea672036c806b5bbf3a71c8cf653fda50fe69acfe`
(source `ac175755dc79be19b397b964441aded01d2cc8a00e18f273abcb603f64fbf671`)
passed both Stata fixtures and all 30 target scientific exports matched the
previous centered candidate exactly. The held timing job 8399981 was deleted
before execution; build job 8399980 was cancelled as superseded. No timing
qualification ran. The repair reuses the already charged, fallible wide numeric
descriptor only within centered admission, retaining exact source/response bits,
original allocation identity, shape checks and ordinary fallback. Other families
retain their existing handoff scope. Width-boundary and stale-row witnesses pass in the frozen final candidate.

## No-FE row-parallel numerical redesign

On November 2 Johannes authorized arithmetic and storage changes for an
ambitious native solver, including loader–solver integration, subject to the
existing scientific safeguards. The next candidate uses the same Fast,
unweighted IID, global-intercept, Auto/Auto family defined below. The declared
continuous numerical scope includes centers, norms, cross-products, coefficients,
bread, residuals, covariance, fit statistics and numerical certificates. Initial
projection diagnostics and finite factor/fallback traces may change; the approved
derived residual row remains in scope. Sample membership, original factor/base
slots, rank, omission and all other discrete scientific decisions remain fixed.
Unaffected families retain exact science and solver signatures.

The intended pipeline is stable global summaries, explicitly centered/scaled
row-block symmetric products, guarded Cholesky or parallel orthogonal compression,
then independent original-source residual/score checks. Bounded packed panels and
a deterministic reduction tree avoid a full centered matrix and worker-count
limits imposed by the number of regressors. No centered Gram or RSS is obtained
by subtracting large raw totals. Existing canonical classification and QR retain
authority at collinearity/rank boundaries and whenever a new factor would omit a
column. Loader summaries may replace fit-time summaries only after the existing
exact sample/value/column binding succeeds; speculative errors remain deferred.
The plugin owns transport and admission; all numerical mathematics stays in core.

Witnesses must cover independent Householder/high-precision b/V/RSS and original
scores, large offsets with few-ULP variation, extreme scales, exact fits, zero and
constant outcomes, duplicates, scaled/permuted columns, rank/pivot boundaries,
block-local constants, partial leaves and N near K. Fixed-worker repetition,
cross-worker discrete agreement, portable/hardware paths, memory boundaries,
allocation failure, Break and rollback remain required. Actual traces must prove
the Cholesky, orthogonal and compatibility routes execute. The final candidate
gets one frozen science preflight and bounded qualification; the completed first
round below remains separate. No additional scan-only timing campaign or public
activation is authorized by this declaration.

The bounded mathematical review rejected a provisional `gamma(64N)` classifier
margin because it lacked a derived error enclosure. The implementation instead
uses the one-sided original-extrema/LASSQ bound documented in ARCHITECTURE. A
constructed rank-boundary case also showed that full rank from TSQR alone could
bypass a canonical dropped-span failure. That reproducer now requires canonical
fallback: the existing stronger Cholesky guard must clear before any compressed
retry, including a forced QR retry after bread refusal. Neither change alters a
scientific tolerance or canonical omission decision.

Focused tests pass for three-scan summaries, extreme/subnormal values, bounded
allocation and cancellation, row-block products, panel Householder compression,
forced production QR retry, full-fit analytic coefficients/inverse/residuals,
rank-boundary outcomes, cached-owner binding and exact worker repetition.
Independent high-effort review accepted the classifier bound and stronger rank
guard. The universal Mac artifact passes the native ABI audit and all 77 existing
plugin replay cases exactly; those replay cases exercise unaffected output paths.
The actual Stata no-FE fixture separately checks workers 1/4/16, ordinary/factor
and restricted/missing samples, zero/constant/exact outcomes, intercept-only fits,
extreme scaling, duplicates, public/hidden equality and publication rollback.
At 1e-100/1e100 regressor scales, Stata regress drops the tiny regressor; the test
therefore uses the independent orthogonal-design b/V/RSS witness. Fereg retains
the same full-rank model as its established scale-aware path. Traces show the
new Cholesky route and canonical compatibility. The compressed retry currently
has production-kernel test evidence, not an actual-plugin performance claim.

The Mac million-row preflight passes all 90 calls against the existing independent
QR references, stable model degrees of freedom, fixed-worker bit repetition and
cross-worker science. All 15 candidate cells execute centered projection/products,
Cholesky and the derived residual row. Preflight elapsed times are diagnostic
only. The full `scripts/run_tests.sh --source-only` gate passes for the frozen
artifact, including allocator audits, portable checks, SPI guards and complexity.

The [completed numerical-round report](../bench/results/no-fe-centered-20261102.md)
retains all 540 Mac observations and the incomplete Linux baseline arm. Parallel
native work improves substantially, but 16/64-regressor single-worker command
medians regress by 15.7%/11.8%, failing the max(5%, 1 ms) guard. The candidate is
not qualified for promotion. Linux actual-plugin science preflight passes all
90 calls; timing job 8398094 stops after the first baseline arm because the
monitor's initial scheduler census precedes its five-second process wait. The
resulting 7.708-second observation gap exceeds its frozen 7.5-second limit.
No competing compute was observed, but monitoring coverage is invalid; this
round supplies no Linux speed comparison and was not resampled. The monitor startup
deadline is now anchored to its existing sample, with 21 focused monitor tests
passing; the observation limits remain unchanged. No round was rerun.

Every target cell reports zero loaded/reused rows. The measured improvement
therefore does not establish loader-summary handoff or copy savings. The
candidate still spends substantial native time on summary scans and independent
original-data checks; these are the proposed focus of a subsequent candidate,
subject to discussion of the failed/inconclusive results with Johannes.

A final scope review found that `xb()` alone was not represented by the
pointwise-residual tolerance proxy. A separate no-FE eligibility flag now
excludes all optional observation outputs (including `xb()` and `dropreason()`)
from the centered/direct/derived paths and loader summaries. It does not change
the output policy of other FE families. This scope repair is separate from the
frozen timed artifact; the no-output timing cells retain their original candidate
and all observations. No timing is being replaced or repeated. The actual internal xb adapter stays on the canonical route, with exact
b/V/RSS/predictions, at workers 1/4/16. Both Stata fixtures pass. The repaired
plugin exports exactly the same scientific bytes as the frozen candidate for
all 15 no-output cells and both repetitions; centered/Cholesky/derived routes
remain unchanged. This is science validation, not a new performance round.

The scope-repaired universal Mac binary passes its ABI audit and all 77
unaffected replay cases exactly. Native source integration checks pass, including
1,573 core unit tests, plugin/FFI tests, allocator audits, portable checks,
complexity, mock SPI and numeric-window guards. The full script reached a stale
complexity declaration; after documenting the six-line output-policy increase,
the ratchet and remaining gate checks pass. An additional 436-test benchmark
suite has four unrelated errors: three missing `reportlab` imports and one
unchanged input-report test whose patched global no longer overrides the
function's captured default (and whose fixture lacks arms). The affected source
and test match HEAD. Focused no-FE harness and monitor tests pass.

## No-FE certified-residual extension

Johannes approved extending the derived residual shortcut on November 2. The
new candidate follows the completed, separately frozen structural scheduling
candidate below; its timing evidence is not substituted for that first round.
The initial scope is no-FE Fast, unweighted IID OLS, with a global intercept,
Auto solver/preconditioner, no optional observation outputs and no
developer routing override. The derived residual solver row may change to stop
code 5 with zero iterations. Residual-based fit statistics and IID covariance
may change in low bits within existing tolerances. Coefficients, bread, sample,
rank, omission/status and all unaffected models remain exact. The structural
moment is recomputed from the final residual at the existing derived scale;
failed checks or insufficient workspace retain the certified final projection.

Independent witnesses cover offset/scale and constant outcomes, duplicates and
rank-deficient regressors, independent QR/score/bread checks, forced certificate
failure, corrupted residual recovery, fixed-worker repetition and cross-worker
agreement. Explicit solvers, weighted fits, robust/cluster inference, requested
outputs, no-constant and Low fits must retain their prior behavior. Actual-plugin
science and trace checks precede timing; this declaration is not qualification
or public activation.

The next initial-projection optimization uses the same eligible family and
keeps dense storage and projection arithmetic. A proven generated constant RHS
provides a rejection-only initial-certificate screen; tolerances where that
screen cannot reject keep the complete incumbent path and its zero-iteration
decision. Validated raw maxima from the existing adopted-row moment cache avoid
recomputing maxima. Original structural means retain observation-order
compensated sums; their scan also computes the original 512-row-leaf FE norms.
After the same in-place subtraction, residual norms supply exact residual
maxima for a fresh independent structural-moment scan. No cache means become
projection centers. Finite reference norms, FE reference norms and FE within
norms retain their distinct arithmetic. This initial operation has empty
numerical and solver-route scopes, with a new trace identifying its execution.
It retains existing O(K) heap banks and admission, adding only bounded worker
stack arrays. Exact witnesses cover original/centered panels, both norm banks,
certificates, loose-tolerance screening, cache widths and worker counts. This
work does not introduce a new finite solver or observation source.

Before the numerical redesign, this direct/derived checkpoint passed the full
source-only suite, native universal-Mac build/ABI audit, all 77 actual-plugin
replay cases with zero science differences, and `ols_no_fe_direct.do`. The Stata
fixture covers workers 1/4/16, factors and excluded inference/storage/options.
Its private Mac plugin SHA-256 is
`cd6c3aef49a4e7f78e8e1daffd1ca33bd08d48bf7c79aa5a8d4ff358a54de19d`;
the frozen source archive SHA-256 is
`b1d583406070e2204ed3feba001f728a737ca11330f207bbbb8c3b6d57886ed6`.
This checkpoint was not timed or published. These checks do not qualify the
new numerical pipeline.

## Structural-constant scheduling candidate

The [completed first-round report](../bench/results/no-fe-centering-20261102.md)
retains all 1,080 Mac/Linux timing observations and platform/license limits.
Both target panels pass the median regression guard, exact incumbent science,
independent QR and fixed-worker repetition. Linux used isolated host scc-wf2
(job 8396235, exit zero); its Stata license permits four processors. Mac permits
eight. These results do not qualify the subsequent direct candidate above or
replace protected routing qualification.

The no-FE speed work starts from published beta.14. Its first candidate reuses
the complete initial structural certificate's means for subtraction in the
borrowed MAP and CG empty-design branches, computes structural means in bounded
parallel column bands, and subtracts means over disjoint rows. Each column keeps
its original scale, observation-order compensated accumulation and mean formula;
the final independent certificate and zero-iteration decision remain unchanged.
Declared numerical and route-change scopes are empty. Posted science, norms,
certificates and solver signatures must match beta.14 exactly at fixed workers.
Additional cancellation checkpoints are intentional; interrupted poll counts
and elapsed times are not exact comparison targets. The three private 64-double
arrays use at most 1,536 bytes of stack per active mean task and add no heap bank.

The shared scaled-norm helper also schedules independent column bands when it
has no partial bank and runs inside an installed team. This affects borrowed
and owned FE/PPML callers satisfying that resource condition, not only no-FE
OLS. Each column retains its 512-row leaves and ascending leaf merge order;
row-major error replay and serial norm finalization remain authoritative.
Four private 64-double arrays use at most 2,048 bytes per active norm task.
There is no new heap allocation, admission rule or outside-team parallelism.

Witnesses include a frozen scalar arithmetic reference in fereg-testkit,
independent OLS/FE oracles, weighted and extreme-scale panels, narrow and wide
column boundaries, the zero-iteration shortcut, fallible O(K) workspace
admission, fixed-worker repeats, cross-worker agreement, actual-plugin replay
and mapped Break/transaction tests. Qualification must compare one frozen
candidate against current beta.14, retain protected original/Auto coverage and
show this structural path executes in the no-FE target cells. This declaration
does not assert a performance result or authorize public activation.

The frozen target panel uses the prior million-row Gaussian design and QR
reference (seed 739391), RHS widths 1/16/64, workers 1/2/4/8/16, and three arms:
published beta.14, this candidate, and Stata regress. Six mirrored/rotated blocks
put each arm in each order position twice, with one warmup and one measured call
per cell per block. It records full-command time and fereg's separate input,
native and total-runtime diagnostics; regress supplies only full-command time.
The Stata processor ceiling is recorded for every cell. This descriptive target
panel, dispatched by `bench/qualify.py input`, does not replace protected routing
qualification. No exclusions, timing retries or candidate replacement are allowed.
Science-only preflight requires independent QR, exact candidate/incumbent
exported science, fixed-worker repetition and traces of the structural projection.
All timed calls also export science outside the clock. Linux uses a separately
verified whole-node allocation; Mac retains ordinary desktop activity.

## Beta.14 publication decision

On November 2, after reviewing the final wide-input results and the remaining
no-FE regressions, Johannes explicitly authorized pushing and publishing
beta.14. This accepts the measured performance exceptions for the frozen
candidate; it does not change the failed comparison statuses, discard timings,
or authorize a new candidate. Scientific checks remain passing. The release
retains serial input on Windows and the existing untested native Intel-Mac
caveat. Final package and installation evidence is recorded with the release.

The [final package verification](../bench/results/beta14-publication-20261102.json)
records the complete passing `scripts/run_tests.sh`, exact binding of all three
released binaries to the tested candidate, zero dependency advisories and
verification of all 3,559 source-archive manifest entries. A fresh offline build
of the corresponding source passes native audit and ABI checks. The exact binary
archive passes Mac installation and independent PPML smoke fixtures. Fresh local
root-package installations on Mac Stata 19 and Linux Stata 15/19 verify all 36
installed files, beta.14 selftest, and independent restricted-sample OLS/PPML
coefficient, variance and sample witnesses. Linux job 8395498 exits zero on
scc-tb4. Windows's previously validated installed binary is reused byte for byte.
The published source is unchanged in production code from the frozen candidate;
later source differences are test, harness and documentation changes.

Beta.14 is published at [the release tag](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.14),
and `main` carries the same native package. Unauthenticated downloads of both
archives match their retained SHA-256 values. Fresh public installations from
both the tag and `main` pass on Mac and Linux; the actual `github install`
command also passes in an isolated Mac Stata environment. Every route verifies
all 36 installed files, selftest and independent OLS/PPML results. Linux HTTP
installation job 8395583 exits zero on scc-gr4. The release contains only the
five archive/checksum/build-note assets, and the release-list response remains
below the smallest supported Stata macro limit. The first isolated GitHub-helper
check lacked `githubdb`; supplying the helper's own ado dependencies resolved
that harness setup error without changing fereg or its checks.

## Wide-input transport follow-up

Johannes authorized investigation and repair of the remaining 64-input-column
boundary. One candidate replaces the serial row tile's width limit with its
existing maximum element budget (262,144 doubles, at most 4,096 rows), and uses
the same admission for deferred columns and packed RHS construction. Narrow
inputs retain their tile shapes. Wide serial input reads all columns in original
row-major order before optional categorical preparation; it must not introduce
the narrow pipeline's categorical-first ordering across the old boundary.
Parallel accessor groups and phase limits remain unchanged.

The affected storage/admission scope is unrestricted numeric OLS and PPML input
with supported numeric FE recipes above 64 total numeric input columns, including
weights, FE/cluster sources and supported slopes. No-absorb loading is an unchanged
control. A newly admitted packed RHS or categorical capsule consumes its full
accounted memory; budget refusal retains the ordinary loader. The core
moment algorithms and their separate width policies are unchanged. Scientific
arithmetic, samples, rank, omissions, separation, solver signatures and results
must remain exact against the incumbent at fixed workers; intended numerical
and estimator-route change scopes are empty. A science or route discrepancy
blocks timing and requires diagnosis, not relaxed tolerances.

Independent witnesses cover scalar input bits, missing codes and signed zero,
packed RHS assembly, original SDK failure precedence, memory refusal, actual
plugin science/repeatability, and transactions/Break. Boundary timing must show
the widened path executed, compare one frozen candidate with beta.13 and the
preceding review candidate, retain every observation, and use the existing
max(5%, 1 ms) guard. The preceding full-sample failure is not waived or relabeled.
No performance outcome is asserted by this declaration.

The boundary suite freezes 100,003 rows, total input widths 63/64/65/66/130,
no-FE and one-FE models, and public-parallel versus forced-serial transport.
Three arms are beta.13, the preceding geometry review, and this candidate;
six mirrored/rotated blocks with one retained repetition give each arm every
order position. Linux workers are 4/16; focused Mac workers are 4/8. Every
cell has one warmup per block. The existing fourteen-cell production timing is retained as unaffected evidence:
its widest FE case is below the changed width boundary, and its 64-regressor
no-FE failure does not exercise the widened path. New boundary timings cannot
turn that prior failure into a pass. The full actual-plugin scientific and
transaction checks are repeated for this candidate. Preflight includes exact posted science, independent regress/areg,
fixed-worker repeats, cross-worker agreement and nine wide weighted, clustered,
slope, memory-low, missing-value and PPML variants. No external-interference
retry or exclusion is permitted; host-monitor failure makes timing inconclusive.

### Completed wide-input investigation

The frozen candidate removes a real admission discontinuity in the serial FE
loader and its optional prepared storage. The old 64-column limit forced wider
inputs out of the bounded row-tile/packed-RHS path. The replacement chooses
`min(4096, floor(262144 / width))` rows, so a pair of double buffers remains at
most 4 MiB. It accounts separately for dynamic column positions, descriptors,
source IDs and the full prepared columns/RHS. Insufficient memory retains the
ordinary loader. This is a general width/resource rule; it does not select
individual models or benchmark cells. The narrow path and the parallel
accessor's group and phase geometry are unchanged.

Width here means **total numeric input columns**, including the outcome and
numeric FE, cluster and weight sources, rather than the displayed regressor
count. A one-FE example with 62 regressors, outcome and FE source has width 64.
The core's separate 64-column fused-moment policy is unchanged; this work does
not claim to remove every width-dependent computational transition.

The [Linux report](../bench/results/input-width-beta14-linux-20261102.json) and
[Mac report](../bench/results/input-width-beta14-macos-20261102.json) retain all
720 observations per host, with full [Linux](../bench/results/input-width-beta14-linux-20261102.csv)
and [Mac](../bench/results/input-width-beta14-macos-20261102.csv) tables. Linux
passes all 40 cell/worker settings against both references. Mac passes 36 of 40;
all settings exercising the changed FE path pass on both hosts. Both host
observers pass, with no exclusions or resampling. Linux timing ran in one
verified isolated 28-core allocation on scc-zj4; native building/science ran
separately on scc-wo4. These are one Linux timing host and one Mac, not broad
cross-architecture performance estimates.

Representative four-worker, one-FE, forced-serial medians are:

| Host | Total input columns | Beta.13 load | Candidate load | Beta.13 whole call | Candidate whole call |
| --- | ---: | ---: | ---: | ---: | ---: |
| Linux | 63 | 84.81 ms | 83.56 ms | 264.0 ms | 264.5 ms |
| Linux | 64 | 91.55 ms | 91.33 ms | 273.5 ms | 272.5 ms |
| Linux | 65 | 110.20 ms | 88.14 ms | 310.0 ms | 283.5 ms |
| Linux | 66 | 112.42 ms | 89.50 ms | 332.0 ms | 313.0 ms |
| Linux | 130 | 215.17 ms | 165.43 ms | 775.5 ms | 730.0 ms |
| Mac | 65 | 31.83 ms | 30.61 ms | 128.5 ms | 120.5 ms |
| Mac | 130 | 61.34 ms | 53.56 ms | 291.0 ms | 283.5 ms |

The Linux serial loading jump from width 64 to 65 disappears: beta.13 rises
20.4%, while the candidate decreases slightly. At width 65, loading improves
20.0% and the whole call improves 8.5%; at width 130 the improvements are 23.1%
and 5.9%. Mac's loading discontinuity was smaller, but its whole serial FE calls
also benefit. These observations establish a useful repair for the tested path,
not a universal speedup.

Parallel loading can take longer because the newly admitted packed RHS performs
preparation during loading. For example, Linux width 130 at four workers rises
from 56.64 to 65.54 ms in loading, while the whole call changes from 597.5 to
599.0 ms and passes the guard. Whole-call performance also passes against the
preceding review arm (577.5 ms). Prepared storage increases accounted peak
memory: Mac serial width 65 rises from 100.70 to 109.48 MiB and width 130 from
200.90 to 214.88 MiB. The tile bound does not bound the complete estimator's
memory; admission includes these additional allocations.

The Mac failures are unchanged no-FE controls. At four workers, width-65
parallel increases from 170.0 to 180.5 ms against both references (+6.18%),
width-65 serial from 193.5 to 205.0 ms against beta.13 (+5.94%), and width-66
parallel from 166.0 to 176.0 ms against the review arm (+6.02%). At eight
workers, width-66 serial increases from 183.5 to 194.0 ms against beta.13
(+5.72%). That is four settings and five reference comparisons. Unchanged
loader code does not establish the cause or cancel these measured failures.
The earlier million-row, 64-regressor no-FE Linux failure also remains:
5.9270 to 6.2615 seconds (+5.64%). The new grid neither replaces nor resamples it.
Johannes's acceptance of possible `in` regressions does not waive these
unrestricted-sample failures. **The fixed performance gate was not met.**
The candidate remained unpublished until Johannes subsequently authorized
publication with these exceptions, as recorded above. No additional
implementation or timing round followed from this result.

Scientific checks pass with no declared numerical or estimator-route change.
Each Unix platform passes 68 native fixtures, 77 exact beta.13 replays,
input-oracle runs at four/eight workers, failure checks, and live Break in input,
OLS and PPML. Each boundary preflight executes 456 fereg science calls across
three arms and two worker settings, covering all width cells and nine additional
weighted, clustered, slope, low-memory, missing-value and PPML families. Exact
input bits and posted science agree at fixed workers, public/forced-serial
results agree, fixed-worker repeats agree, and cross-worker science passes the
independent one-worker and regress/areg witnesses. Traces prove the widened
reserved-column/packed-RHS path actually ran. Native witnesses also cover signed
zero, missing codes, first SDK error precedence, maximum tile bounds, and
simultaneous descriptor/source-ID memory admission.

The complete source-only integration suite passes, including the complexity
ratchet and C bridge checks. At the end of this investigation, final root-package admission had not been
performed and the root package still contained beta.13 artifacts. Publication
requires matching package artifacts and separate installed-package checks. The
[Windows report](../bench/results/input-width-beta14-windows-20261102.json)
records successful native build/audit and actual locally installed Stata science,
including 72 input-oracle cases/208 calls at default and eight requested workers,
wide-path trace checks, the independent missing-value witness and OLS/PPML
transactions. Windows continues to use serial input; live parallel Break and
Windows performance are not qualified. The machine is stopped, transient objects
deleted and its lock released.

Bounded harness repairs preceded timing: use the existing PPML adapter name,
separate extra-science trace groups, and provide the review arm's matching
complete ado tree including its developer serial adapter. Production ado and
runtime identities did not change. The Mac build also needed an external vendor
path added to binary path remapping before its privacy audit passed. Windows's
first run passed its default-worker oracle but stopped at a stale coverage
assertion (70/202 instead of the new 72/208); after correcting that assertion,
the full run passed. These repairs did not relax scientific checks, alter the
frozen runtime, or discard any timings. Independent high-effort review supports
retaining the bounded repair while leaving overall performance acceptance open.

## Beta.14 input-transport qualification scope

The input-transport candidate preserves all scientific arithmetic, loaded value
bits, missing flags, original row identities, sample/rank/omission/separation
decisions and solver signatures. Its declared changes are transport work:
Stata-resolved `in` bounds, bounded read/fill/copy phases, complete descriptor
admission and a sticky marker SDK error after diagnostic replay. It also
retains the serial reader on Windows pending live Break qualification. There is
no compact candidate storage or estimator routing change.

Independent witnesses are the scalar input oracle (including all Stata numeric
storage types and missing codes), exact incumbent plugin replay, fixed-worker
repeatability and cross-worker scientific agreement, and transaction/Break
fixtures. Qualification must use the actual installed candidate plugins and
traces proving each changed loading path ran. No new timing or release result
is asserted by this scope declaration.

The fixed timing design is the eight existing input cells plus narrow/wide
1% scattered OLS, a short contiguous `in` inside ten million original rows,
restricted weighted multi-FE OLS, restricted string-FE OLS and restricted PPML
with exposure. Each host uses four mirrored arm blocks and three repetitions,
with a warmup per cell/worker/block; no timing is discarded or resampled.
Linux workers are 4/16/28 in one verified whole-node allocation; Mac workers
are 4/8 with ordinary desktop activity retained. The incumbent is beta.13.
The wall-time regression rule is max(5%, 1 ms). No eligible family's science
or estimator route differs, so the numerical-change and derived-route scopes
are both empty. Actual Stata execution is required; the mock-accessor replay
cannot measure transport. `bench/qualify.py input` runs and reports the maintained
licensed-Stata harness, after its per-cell path and exact input-digest preflight.
There is no external estimator environment in this transport comparison;
Stata/toolchain/plugin/input identities must be pinned in the execution record.

The candidate is **not qualified for beta.14 publication**. The completed
[Linux comparison](../bench/results/input-beta14-linux-20261102.json) retains
all 1,008 observations across 42 settings on one isolated 28-core physical host:
19 settings pass and 23 exceed the declared wall-time regression allowance.
Host observation passes without errors; scheduler accounting reports `failed=0`
and `exit_status=1` because the performance report rejects the candidate.
No observations were excluded or resampled. The
[Mac comparison](../bench/results/input-beta14-macos-20261101.json) retains all
672 observations and passes all 28 settings with ordinary desktop noise retained.
The complete [Linux](../bench/results/input-beta14-linux-20261102.csv) and
[Mac](../bench/results/input-beta14-macos-20261101.csv) timing tables are retained.
The Linux result blocks publication despite the passing Mac comparison and
correctness checks. The public release remains beta.13.

Representative Linux whole-call medians are:

| Setting | Workers | Beta.13 | Candidate | Change |
| --- | ---: | ---: | ---: | ---: |
| 64 regressors, full sample | 4 | 5.1245 s | 5.3885 s | +5.15% |
| Ten million rows, four regressors | 4 | 3.9445 s | 4.3435 s | +10.12% |
| Short `in` inside ten million rows | 16 | 1.1250 s | 1.4380 s | +27.82% |
| Weighted multi-FE, 1% scattered sample | 16 | 0.2915 s | 0.4030 s | +38.25% |
| String FE, 1% scattered sample | 16 | 0.2785 s | 0.4035 s | +44.88% |

The short-`in` case at 16 workers has native medians of 0.2692/0.2690 seconds
and loading medians of 0.0606/0.0544 seconds, despite its whole-call slowdown.
These separate medians do not decompose one individual call, but they show why
an exclusively reader-based explanation is premature. Block variability is
retained in the Linux record. A proposed next step is one bounded diagnostic
round separating frontend/host work, loading, phase barriers and subsequent
native work, followed by one frozen revised candidate only after Johannes's
direction. The hard work cap, exact input bits and memory admission remain
requirements. No follow-up timing or candidate substitution is authorized by
this failed result itself.

Johannes authorized investigation of these regressions on November 2. The
bounded diagnostic grid uses the unchanged incumbent and candidate binaries,
cells c1/c3/c5/c11/c12/c13, sixteen Rust workers, Stata's observed default
processor setting versus one processor, and default versus interleaved NUMA
memory placement. Two mirrored blocks retain three calls after one warmup per
cell/variant: 288 timed observations, with no exclusions or retries. The full
28-core CPU affinity remains available to both arms. Every variant first checks
exact coefficient, covariance, sample-count and input-digest agreement; existing
independent native/scalar witnesses remain applicable because neither binary
changes. Numerical and estimator-route scopes remain empty.

Temporary ado copies add timers before, around and after the substantive plugin
call; the public ado files are unchanged. The diagnostic uses the existing
runtime snapshot separately from the complete plugin call and profiles the
untimed science calls. Optional hardware counters are used only if a preflight
capability probe succeeds; unavailable counters are recorded, not grounds to
repeat a timed observation. Both memory policies must be available before any
timing. One whole-node SCC allocation runs the grid through
`bench/qualify.py input diagnose`; its results explain costs and cannot replace
the failed release comparison or establish release acceptance.

The [completed diagnostic](../bench/results/input-beta14-diagnostics-linux-20261102.json)
retains [all 288 calls](../bench/results/input-beta14-diagnostics-linux-20261102.csv).
It ran on the same physical host as the failed qualification, with successful
scheduler accounting, isolation checks and all eight exact-science preflights.
Stata's observed default was four processors, independently of the sixteen Rust
workers. Its instrumentation distinguishes the native runtime snapshot (which
already includes input loading) from the entire plugin call; adding `load_s` to
the old `native_s` field would double-count loading.

The 32-regressor c3 loading penalty is reproducible in all four diagnostic
configurations: incumbent/candidate medians are 117.5/151.4 ms under defaults,
120.1/159.4 ms with one Stata processor, 126.1/165.1 ms with interleaved memory,
and 126.0/166.3 ms with both controls. Loaded bits, coefficient/covariance bits
and sample counts agree exactly. The transport trace changes from three to
25 phases over the same 1,039 blocks. The candidate accounts for worst-case
read/write/replay/RHS work while halving the nominal cap; in this cell a phase
contains only 42 blocks, about 2.6 blocks per worker. Frequent joins and partly
occupied final task waves are plausible overhead; the cached pool is not
recreated for each phase. The current measurements do not separately identify
polling, window validation, dispatch, cache effects and join imbalance.

The earlier 28–45% whole-call penalties for c11/c12/c13 do not recur at that
size. Under the default diagnostic configuration their incumbent/candidate
whole-call medians are respectively 1.2125/1.1530 s, 0.3050/0.3005 s and
0.2635/0.2670 s. c12/c13 retain small loading penalties (about 3–4 ms), while
c11 loading is approximately 60 ms in both arms. c11's complete plugin call is
approximately 0.29 s; about 0.85–0.91 s lies outside it, including frontend
preparation, wrapper work and automatic cleanup. This identifies a large
shared cost, but cannot retrospectively attribute the original extra 0.3 s.
The failed qualification remains failed; differences in instrumentation,
process history, placement and ordinary host variation are not exclusion rules.

Neither control is a general repair. One Stata processor usually increases
frontend preparation and substantially worsens the restricted calls. Memory
interleaving reduces some whole-call costs while the c3 loading gap persists;
it can increase c11 loading. Untimed whole-process counters show substantially
fewer page faults with interleaving in both arms, but include dataset startup
and cannot attribute faults or NUMA locality to a particular reader phase.
No claim of a causal allocator/NUMA fault is established.

The completed diagnostic grid motivates one bounded function-sampling follow-up
for the reproducible c3 loading cost: unchanged binaries, sixteen Rust workers,
default Stata processors and memory placement, two mirrored blocks, one warmup
and eight observed calls per arm/block (32 calls). A 99 Hz user-cycle profile
with DWARF call stacks records each fresh process; two separate exact-science
preflights precede sampling. This is a mechanism profile with sampling overhead,
not another release comparison. No profile or observation is excluded or retried.

The [function profile](../bench/results/input-beta14-profile-linux-20261102.json)
retains [32 observed calls](../bench/results/input-beta14-profile-linux-20261102.csv)
and summaries of 37,928 samples, with zero samples reported lost. Both arms
spend similar aggregate user cycles under the numeric-window accessor (about
11–12% of the profiled process). The candidate exposes a separate `read_block`
symbol whereas the incumbent inlines that work. There is no demonstrated new
CPU hotspot explaining the full wall gap. In particular, the large inclusive
`wait_until_cold` percentage includes estimator work executed underneath the
scheduler; it is not evidence of idle time or spinning. The profiled loading
medians remain 123.2/157.9 ms. These samples cannot measure sleeping/barrier wall
time or directly assign kernel page-fault cost.

One
fixed causal ablation therefore compares the current candidate with a temporary
build differing only in the production work cap: `2^23` versus `2^26`. The larger
cap is chosen once to approximate the incumbent's original amount of fully
accounted work, while preserving the hard bound and prohibiting an occupancy
override. It permits eight times as much accounted work between Break polls;
this is an explicit responsiveness tradeoff, not a claimed free improvement.
No runtime change is applied to the working source or distributed binaries.

The ablation freezes c3/c6/c11, workers 4/16/28, default Stata processors and
memory policy, two mirrored blocks and three observations after one warmup per
setting: 108 observed calls plus 18 exact-science/input-digest preflights. The
same beta.14 ado files serve both arms. Build/ABI checks finish before the
isolated measured interval. All observations and actual transport phase traces
are retained; there is no cap search, resampling or release-acceptance inference.
The harness's `incumbent` label means the rejected 8 Mi-work candidate in this
ablation, not beta.13; `candidate` means the temporary 64 Mi-work build.

The [completed cap ablation](../bench/results/input-beta14-cap-probe-linux-20261102.json)
retains [all 108 observations](../bench/results/input-beta14-cap-probe-linux-20261102.csv),
18 exact-science/path preflights and the source audit confirming that the cap was
the only source change. Native ABI/guard checks, isolation and scheduler
accounting pass. Results are mixed (Mi means 2^20 accounted work units):

| Cell | Workers | Loading, 8 Mi cap | Loading, 64 Mi cap | Loading change | Whole-call change |
| --- | ---: | ---: | ---: | ---: | ---: |
| 32 regressors (c3) | 4 | 366.4 ms | 402.6 ms | +9.9% | -7.6% |
| 32 regressors (c3) | 16 | 157.9 ms | 117.4 ms | -25.6% | -7.6% |
| 32 regressors (c3) | 28 | 131.1 ms | 88.5 ms | -32.5% | +2.7% |
| Ten million rows (c6) | 4 | 624.7 ms | 682.7 ms | +9.3% | +3.8% |
| Ten million rows (c6) | 16 | 285.7 ms | 240.5 ms | -15.8% | +6.9% |
| Ten million rows (c6) | 28 | 254.3 ms | 204.0 ms | -19.8% | +7.1% |
| Short `in` (c11) | 4 | 126.8 ms | 66.9 ms | -47.2% | -19.9% |
| Short `in` (c11) | 16 | 57.0 ms | 33.0 ms | -42.2% | -18.1% |
| Short `in` (c11) | 28 | 69.2 ms | 44.8 ms | -35.3% | -26.1% |

The c3 phase count falls from 25 to four, c6 from 44 to six and c11 from
32 to four, without changing block counts or loaded bits. This supplies causal
evidence that the phase-budget choice contributes substantially to high-worker
loading cost. It does not show that one larger global cap is a general fix.
At four workers the full-input loading medians worsen. At sixteen workers c6's
native compute median rises from 2.5640 to 2.7821 seconds, outweighing its 45 ms
loading gain; the later work is also slower in both mirrored blocks. Memory
first-touch, task placement and cache effects are plausible mechanisms, not
established causes. c6 was not included in the initial NUMA-control grid.
The same host and exact scientific results do not make memory placement fixed.

The responsiveness tradeoff is measurable: c3's longest traced phase at four
workers rises from 19.2 to 168.5 ms (at sixteen: 8.0 to 43.6 ms). These untimed
preflight observations are not worst-case bounds, especially under accessor
errors/replay. The temporary build has no new PPML, Mac, Windows or interactive
Break qualification and is not selected for publication.

The next repair direction is a shared worker-aware phase policy under a strict
absolute work ceiling, together with efficient task grouping and explicit
investigation of input first-touch effects on subsequent computation. It should
preserve original-order phase coverage and serial error precedence; assigning
workers permanent separated row regions would require extra logic before an
observed marker failure could terminate unread earlier rows. Logical lane
stability alone does not guarantee physical CPU or NUMA placement. A fill-only
optimization can help short `in` ranges but cannot address c3, which uses the
full envelope. Compact sample storage remains a separate architectural change.
These are design directions, not demonstrated universal improvements. The
bounded investigation ends with the original candidate still rejected and no
cap change applied to main; further candidate work must address the disclosed
mixed result rather than retune or repeat this experiment until it passes.

Johannes authorized continuation after the mixed diagnostic on November 2,
with a preference for a clean general implementation. The single revised
candidate changes only the shared transport scheduler: a phase budget of
`min(64 Mi, max(8 Mi, resolved_workers * 2 Mi))` accounted work units, rounded
down to whole admitted blocks, and removal of forced one-block Rayon leaves.
The same computed budget governs row admission, block size and phase size.
Original-order contiguous phases, per-block work, failure reduction, storage,
loaded bits and arithmetic remain unchanged. It adds no user knob, host-specific
route, affinity policy or persistent lane ownership. A bounded independent
high-effort design review found no architectural blocker and required checking
both the selected budget and fixed ceiling, including Break and error paths.

This sizing is a fixed engineering choice motivated by the diagnostic, not an
established optimum. Four-worker phases retain the rejected candidate's 8 Mi
budget; sixteen and twenty-eight workers use 32 Mi and 56 Mi respectively.
Rayon grouping does not guarantee NUMA placement. Higher-worker phases can
lengthen Break latency, and the c6 downstream-compute penalty remains an open
qualification risk. Scientific and estimator-route scopes are still empty;
the independent scalar oracle, exact incumbent replay, cross-worker agreement,
fixed-worker repeatability and transaction/active-window Break witnesses apply.
The revised candidate must pass the original fourteen-cell Linux/Mac comparison
against beta.13, with the same workers, mirrored blocks, repetitions, whole-call
threshold and no exclusion/resampling rule. Actual-plugin science/path checks
and verified relative-path environment manifests precede timing. The earlier
failed qualification and mixed diagnostic remain evidence; neither is replaced
by this authorized revision.

The [revised Mac comparison](../bench/results/input-beta14-revised-macos-20261102.json)
passes all 28 settings with [all 672 calls](../bench/results/input-beta14-revised-macos-20261102.csv)
retained, no resampling and successful host observation. Short-`in` whole-call
medians improve from 228 to 205 ms at four workers and 217 to 205 ms at eight.
The full ten-million-row case changes from 927.5 to 929.5 ms at four workers and
759.5 to 764 ms at eight. Exact input preflights pass across both arms and both
worker counts. The universal Mac build/ABI audit, 68 Stata 19 fixtures, 77 exact
incumbent replays, scalar oracles, failure rollback and live console Break checks
pass. The Stata 18 console oracle also passes 70 cases/202 runs (71 parallel).
Full source integration and the complete suite with the actual candidate root
package pass. A fresh Mac installation verifies all 36 files,
self-test and independent restricted OLS/PPML examples. The
[revised Windows qualification](../bench/results/input-beta14-revised-windows-20261102.json)
passes its exact-source installed-plugin, scalar-oracle, missing-value, rollback
and paired serial/parallel checks. Its beta.13 comparison and interactive Break
remain unqualified. The guarded controller collected the verified plugin,
deleted transient transfer objects, released the lock and stopped the instance.
The corresponding source closure matches every compiled source and dependency
file; only generated audit/SBOM/manifests differ, and the fresh dependency audit
reports zero advisories. These passing correctness results do not override the
failed revised Linux performance comparison below.

The [revised Linux comparison](../bench/results/input-beta14-revised-linux-20261102.json)
retains [all 1,008 calls](../bench/results/input-beta14-revised-linux-20261102.csv)
on the isolated 28-core host: 31 settings pass and eleven exceed the unchanged
whole-call allowance. The failures are c1/28, c8/16, c9/4, c11/16, c12/4/16,
c13/4/16/28 and c14/4/16. Exact actual-plugin preflights pass for all fourteen
families at all three worker counts, and all native/scientific/transaction
checks pass. Scheduler accounting records a performance-gate exit, with no
native or host-monitor failure. Fresh Linux 15/19 installations verify all 36
files, self-test and independent restricted OLS/PPML examples.

The revised c3/c6 whole-call results are within the allowance at every worker
count, but loading does not improve uniformly. Short-`in` c11 at sixteen workers
loads in 59.6/36.0 ms (incumbent/revised), while its whole call takes 1.154/1.343 s.
The candidate's whole-call penalty occurs in all four mirrored blocks. The
per-call residual `wall_s - native_s`, then aggregated, has medians of
879.0/1089.7 ms; this includes frontend/wrapper work AND native work after the
runtime snapshot. It is not solely frontend time. Weighted restricted c12/16
has whole-call medians of 294.5/346.5 ms and residuals of 233.1/282.9 ms.
String-FE c13/16 takes 264.0/316.5 ms, including loading of 28.6/39.5 ms;
its loading penalty occurs in all four blocks. Low-memory c8 and PPML c14 also
show native-time penalties. Separate medians are not additive, and these
observations do not establish a single causal mechanism. The revised candidate
remains unqualified; the public release stays beta.13.

One bounded follow-up uses these unchanged binaries and the existing diagnostic
harness to partition frontend setup, the complete plugin call and frontend
publication. It freezes c8/c11/c12/c13/c14 at sixteen Rust workers, default Stata
processors and memory placement, two mirrored blocks, one warmup and three
observations per cell/arm/block: sixty observed calls. Both arms first verify
exact coefficients, covariance, sample counts and input digests for every cell.
The verified environment pins and whole-node host monitoring remain required;
there are no exclusions, retries, hardware profiles or new runtime builds.
Science and estimator-route scopes are empty. This mechanism diagnostic cannot
replace either failed qualification or qualify the release. Instrumentation
and process history may affect the observed costs and must be acknowledged.

The [revised diagnostic](../bench/results/input-beta14-revised-diagnostic-linux-20261102.json)
completed with [all sixty calls](../bench/results/input-beta14-revised-diagnostic-linux-20261102.csv),
ten exact science/input preflight calls, successful isolation and scheduler
accounting. It ran on the same physical host; Stata used four processors and
Rust sixteen. Its incumbent/revised medians are:

| Cell | Whole call | Complete plugin | Input loading |
| --- | ---: | ---: | ---: |
| Low-memory wide OLS (c8) | 3254.5 / 3333 ms | 2854.5 / 2775.5 ms | 104.5 / 103.6 ms |
| Short `in` (c11) | 1490 / 1083 ms | 312.5 / 286 ms | 63.5 / 41.7 ms |
| Restricted weighted OLS (c12) | 383.5 / 310.5 ms | 63.5 / 65 ms | 16.2 / 17.7 ms |
| Restricted string FE (c13) | 303 / 265 ms | 53.5 / 53 ms | 29.7 / 29.6 ms |
| Restricted PPML (c14) | 734 / 704 ms | 387 / 369 ms | Not separately reported |

The restricted-case whole-call direction reverses in this shorter instrumented
run. c11's candidate is faster in both blocks; its per-call remainder outside
the three estimator timers has medians of 714.5/457.5 ms. This remainder includes
outer wrappers, automatic scope cleanup and timer boundaries, and must not be
called purely cleanup. c12 retains a small loading penalty; c13's larger loading
penalty does not recur here. No new frontend algorithm is implicated by the
source comparison: only forwarded `in` bounds and version labels changed there.

For c8, the pooled plugin median is misleading if read as a causal improvement:
the candidate's plugin block medians are slower by 70 and 35 ms, and whole-call
block medians are slower by 78 and 204 ms. Separate pooled medians cannot be
added or used to exonerate native work. Instrumentation, process history and
ordinary host behavior remain possible influences; none has been established
as the cause of the qualification penalties or as a valid exclusion.

The bounded investigation therefore ends with the revised candidate still
unqualified. Independent high-effort review agrees that another phase constant,
affinity policy, allocator change or sample-specific route lacks supporting
evidence. The small shared scheduler implementation remains available for
review, with passing correctness and Mac checks; the failed Linux gate remains
binding. No further sampling or candidate substitution follows automatically.
Johannes must decide any acceptance of the disclosed performance tradeoff or
broader investigation before publication.

Johannes authorized a further bounded investigation on November 2. Static
review identifies automatic estimator scope cleanup as a specific hypothesis:
restricted OLS and PPML retain a private full-length input marker after result
posting consumes the separate sample output. The public wrapper has no matching
observation-scale operation in these cases. Stata documents move semantics for
`_estimates hold` and consumption of `esample()` by `ereturn post`; removing
rollback or assuming duplicate estimate copies is therefore unsupported.

The cleanup diagnostic compares the same revised native binary and ado sources,
with one scratch-only difference: the candidate explicitly drops the still-live
private `touse` marker after transaction commit, before estimator return. Both
arms receive identical call/return, wrapper-syntax and body timers; they record
whether candidate and rollback markers still exist at return. The frozen grid
is c1/c11/c12/c13/c14, sixteen Rust workers, default Stata processors and memory
placement, two mirrored blocks, three observations after one warmup: sixty calls.
Exact coefficients, covariance, sample count, full posted sample signature and
input digests precede timing. No science or route changes, exclusions, retries
or release-acceptance inference are allowed. Existing native correctness evidence
is reused; actual Mac OLS/PPML smoke checks validate the added instrumentation.
A movement of time into explicit `drop` would establish attribution only; a
whole-call gain is required before selecting any production cleanup change.

The [completed cleanup diagnostic](../bench/results/input-beta14-frontend-linux-20261102.json)
retains all sixty calls on isolated scc-wo4 (job 8393677, accounting exit zero,
95 seconds, 2.990 GB maximum virtual memory). The ten science calls agree exactly,
including full sample signatures. Public wrapper preparation/publication and
outer scope each take about 0–2 ms. Almost all previously unpartitioned time
belongs to the hidden estimator call/return. The obsolete rollback marker is
still live after `unhold, not` in every observed call; the candidate marker is
live in restricted calls and already consumed in full-universe OLS.

Explicit candidate-marker deletion takes median 210.5 ms for ten-million-row
c11, 63.5 ms for c12/c13 and 83.5 ms for PPML c14. The hidden-scope remainder
falls substantially, identifying cleanup as a real cost without establishing
Stata's internal deletion algorithm. Whole-call medians (automatic/explicit)
are 586/584, 1453.5/1320, 367/380.5, 374.5/341.5 and 758.5/732.5 ms for
c1/c11/c12/c13/c14. c11's block directions disagree and c12 worsens in both
blocks. Moving a single deletion is therefore not selected as a repair.
Separate timer medians are not additive; the scope remainder includes timer
and entry/exit gaps. The report records pre-timing harness repairs, including
cancellation of job 8393674 before any output because its copied import closure
was incomplete. No timing observations were excluded or repeated.

The next bounded candidate batches deletion of the two obsolete private markers
in OLS and PPML, inside the final `nobreak` block immediately after `unhold, not`.
Only existing private variables are included; a consumed sample output is skipped.
Deletion is best-effort, with automatic scope cleanup as fallback. All sample
validation, output publication and precommit rollback stay in their existing
order. Scientific and routing scopes are empty; coefficients, covariance,
full posted samples and all discrete decisions must remain exact. Independent
review rejected input/output marker aliasing because it would violate native
nonaliasing admission and make PPML containment validation vacuous.

Before timing, actual Mac transaction fixtures and full-sample witnesses must
pass. One sixty-call Linux comparison uses the same five cells, sixteen workers,
two mirrored blocks and three observations after warmup, with the same native
binary in both arms. The only production difference is batched cleanup. It
reuses the verified environment and diagnostic instrumentation, with exact
science and input preflight and whole-node isolation checks. No exclusions,
resampling or release inference are allowed. A useful whole-call improvement
is a prerequisite to carrying this candidate into full release qualification;
the preceding failed Linux release gate remains binding.

The [batched-cleanup comparison](../bench/results/input-beta14-batched-linux-20261102.json)
completed on isolated scc-wo4 (job 8393894, exit zero, 90 seconds, 2.986 GB).
All sixty observations are retained and all ten exact-science preflights pass.
Automatic/batched whole-call medians are 609/552, 1308/1216, 350/300.5,
333.5/258.5 and 745/715.5 ms for c1/c11/c12/c13/c14. Both private markers are
absent before candidate return; the hidden-scope remainder falls to 1–1.5 ms.
c12's first block is 2 ms worse and c13's first block ties; other block
comparisons favor batching. The result supports carrying this small general
change into qualification, with no claim that every observed difference is
caused by cleanup. Source identity confirms all 227 native production modules
and 58 compiled artifact inputs are unchanged. Five mapped actual Mac fixtures
and exact OLS/PPML sample/result witnesses pass; independent review required
`confirm variable, exact` to prevent a missing temporary name matching a user
variable abbreviation, followed by both transaction fixtures passing again.

The final cleanup candidate is frozen for one full original fourteen-cell
comparison against beta.13: Linux workers 4/16/28 and Mac workers 4/8, four
mirrored blocks, three observations after one warmup per setting/block. This
retains 1,008 Linux and 672 Mac calls, with max(5%, 1 ms) whole-call protection,
no exclusions or retries, verified isolation on Linux and ordinary desktop
noise retained on Mac. Scientific and route scopes remain empty for every OLS
and PPML family. Actual-plugin preflight checks every cell/worker's input path,
input digest, coefficients, covariance, N and full e(sample). Existing unchanged
native science/ABI/portable evidence is reused; frontend transaction and installed
package checks are refreshed for the final source. Passing this new candidate's
full gate is required before beta.14 publication; the diagnostic alone does not
supersede either prior failed release comparison.

Publication review found that the experimental Windows host-table addition lacks
its required live Break witness (development workflow, native/frontend boundaries).
The Windows oracle, injected-failure and missing-value results remain useful
experimental evidence, but do not authorize activation. Before publication the
Windows Stata/MP 19 GUI row is removed, retaining the existing serial fallback.
This policy change affects Windows transport only: no OLS/PPML arithmetic,
sample or estimator routing changes, and no Linux/Mac host decisions change.
The frozen Linux/Mac comparisons finish with their unchanged artifacts; their
timing is reused for this platform-only policy correction. Rebuilt artifacts
must pass actual native/scientific/transaction and installed-package checks.
The Windows oracle must additionally prove that no numeric window runs at any
worker count; it must not relabel serial fallback as a parallel success. Existing
Mac/Linux active-window Break evidence remains applicable. Windows activation
stays deferred until the missing live test can be performed.

The cleanup candidate's [Mac qualification](../bench/results/input-beta14-cleanup-macos-20261102.json)
passes all 28 settings and retains [all 672 calls](../bench/results/input-beta14-cleanup-macos-20261102.csv).
Its 56 preflight calls agree exactly on coefficients, covariance, N, full sample
signatures and input digests. All 68 native fixtures and input/OLS/PPML live
console Break checks pass. Ordinary desktop noise is retained. At eight workers,
beta.13/candidate whole-call medians are 216/131 ms for short `in`, 107.5/79.5 ms
for restricted weighted OLS, 103/75 ms for restricted string-FE OLS and 158/132 ms
for restricted PPML. These are measured Mac results; the frozen Linux gate is
still required. Independent review finds no blockers in the final Windows
fallback or exact-name postcommit cleanup and accepts Linux/Mac timing reuse
for the Windows-only policy removal, subject to rebuilt-artifact checks.

The final cleanup [Linux comparison](../bench/results/input-beta14-cleanup-linux-20261102.json)
retains [all 1,008 calls](../bench/results/input-beta14-cleanup-linux-20261102.csv).
It passes 39 of 42 settings and fails the declared release gate in three:

| Setting | Workers | Beta.13 | Cleanup candidate | Change |
| --- | ---: | ---: | ---: | ---: |
| One FE, 32 regressors | 28 | 1.1195 s | 1.2180 s | +8.8% |
| Ten million rows, four regressors | 4 | 4.0885 s | 4.3370 s | +6.1% |
| One FE, 32 regressors, `memory(low)` | 16 | 2.9865 s | 3.2635 s | +9.3% |

The 32-regressor and low-memory penalties appear in all four block medians;
the ten-million-row penalty appears in three. Low-memory loading is nearly
unchanged (98.3/99.4 ms), while native time including loading rises from
2.7042 to 2.9175 seconds. The 32-regressor loading difference is only 3.1 ms;
its later native work and work outside the native runtime also rise. The
large-input case has a 98.3 ms loading increase plus slower subsequent work.
Separate medians are not additive. Memory placement and cache effects remain
possible explanations, not established causes, and these observations do not
justify excluding any block.

Restricted whole-call improvements are 14–27% for short `in`, 21–24% for
weighted multi-FE OLS, 21–32% for string-FE OLS and 6–7% for PPML. All 84
Linux exact-science preflight calls pass, including complete sample signatures.
Isolation passes; job 8393922 on scc-wo4 records `failed=0`, `exit_status=1`,
2,992 seconds and 9.441 GB maximum virtual memory. Exit one is the performance
gate rejecting the three settings, not a native or scientific failure. No
observations were excluded or resampled. Beta.14 remains unpublished pending
a performance-policy decision or a newly declared, narrower candidate.

The final rebuilt [Mac](../bench/results/input-beta14-final-macos-native-20261102.json)
and [Linux](../bench/results/input-beta14-final-linux-native-20261102.json)
artifacts pass all 68 fixtures, scalar input oracles at four/eight workers,
failure/rollback, real input/OLS/PPML console Break and all 77 exact incumbent
replay cases. The Linux native job ran separately on scc-we3 (8394017, exit
zero), outside the measured host. The Mac build came from the offline source
archive; its stored executable permissions were restored after Python ZIP
extraction omitted them, then the actual ABI/load/panic harness passed.

The [experimental Windows cleanup run](../bench/results/input-beta14-windows-experimental-20261102.json)
passed its driver, but lacks the live Break prerequisite for parallel activation.
The subsequent [serial-policy run](../bench/results/input-beta14-windows-harness-failure-20261102.json)
passed the nested scientific oracle, then failed the trace-checker launcher with
code 65000. A one-option PowerShell `if` output became a scalar string, so
splatting passed individual characters to Python. The
[bounded Windows probe](../bench/results/input-beta14-windows-argv-probe-20261102.json)
reproduced that argument error, verified the typed-array correction, checked
both accepted modes and rejected a forbidden numeric window. It deliberately
returned diagnostic code 97001 and confers no release qualification. Both
runs stopped the machine, deleted transient objects and released the lock.
The fix changes only the test launcher. The corrected
[Windows native and installed run](../bench/results/input-beta14-final-windows-native-20261102.json)
passes at exact source `e43d534c`: both oracle runs cover 70 cases/202 estimations
with zero numeric windows, equal digests and scientific agreement. The missing
witness, native/MSVC/ABI checks and OLS/PPML transaction fixtures pass. The
controller records 1,005.314 seconds, transient objects deleted, lock released
and the machine stopped. This resolves the launcher failure and confirms the
serial fallback; it does not waive the failed Linux performance gate.

The assembled [three-platform review package](../bench/results/input-beta14-final-package-review-20261102.json)
passes the complete `scripts/run_tests.sh`, including native runtime binding and
all 36 package files. Fresh local installations on Mac Stata 19 and Linux Stata
15/19 verify every installed byte, beta.14 selftest and independent restricted
OLS/PPML coefficient, variance and sample witnesses. Linux install job 8394188
exits zero. The source archive has zero dependency advisories and 2,794 native,
Stata, build and dependency files identical to the offline-tested archive.
The review manifest explicitly sets `publication_eligible=false`. No HTTP install,
release tag or main-package publication is claimed. The candidate and current
inputs are retained for the performance decision; superseded local diagnostic
captures, archives and source snapshots were removed after recording summaries.

Johannes subsequently made `in` performance secondary and authorized accepting
an `in` regression. The three remaining cleanup failures are full-sample cells,
so this does not waive those failures. One narrower candidate restores the
incumbent's 16 Mi slot-work phases and one-block Rayon splitting, subject to a
128 Mi complete-work ceiling, with no occupancy override or minimum row floor.
For every validated layout, `4*width + 6*marker + 2*rhs_width <= 8*slots`;
the factor eight includes the one-source/two-RHS appended-intercept case.
This preserves ordinary incumbent scheduling geometry while retaining descriptor
admission, sticky errors, bounded original-row work and the `in` read envelope.
It replaces the worker-adaptive scheduling policy rather than adding another
route, knob, affinity policy or model-specific exception. Independent review
finds this a defensible compatibility narrowing, not a demonstrated performance
fix. Later-native penalties in c3/c8 remain unexplained. Longer work between
Break polls is an explicit responsiveness tradeoff.

Scientific arithmetic, storage, loaded bits and estimator routes remain unchanged;
the numerical-change and derived-route scopes are empty. Before timing, the
actual plugins must pass exact input/science preflights for every cell and worker,
including full samples and the existing independent witnesses. The same fixed
four-block, three-repetition, fourteen-cell Linux/Mac design applies. c11 (`in`)
is retained and reported but advisory under Johannes's instruction; the other
thirteen cells retain max(5%, 1 ms) whole-call protection. All observations are
kept, with no interference exclusions or retries. Environment and input pins
must be reverified from the run root. This is one frozen candidate, not a search
over phase constants, and no new timing or release success is asserted here.

The narrower [Mac comparison](../bench/results/input-beta14-geometry-macos-20261102.json)
passes all 28 settings and retains [all 672 observations](../bench/results/input-beta14-geometry-macos-20261102.csv).
The c11 advisory exception is unused: short-`in` whole-call medians improve from
229/217 ms to 130/131 ms at four/eight workers. Full-sample ten-million-row medians
are 923/760 ms for beta.13 and 908.5/737 ms for the candidate. Actual-plugin
preflight confirms exact input/scientific agreement and identical incumbent
phase/block geometry in every frozen cell and worker setting. All 68 native
fixtures, scalar input oracles, failure rollback, live console Break and 77 exact
incumbent replays pass. The initial Mac fixture launcher used the source archive,
which deliberately omits test-only OLS ado adapters; command-not-found failures
were repaired by running only those affected fixtures from the full exact-source
checkout with the same binary. Passing fixtures and the initial failure evidence
were retained. This is not a scientific or timing retry. Linux qualification
remains required; no publication is authorized by the Mac pass alone.

The narrower [Windows run](../bench/results/input-beta14-geometry-windows-20261102.json)
passes with the same native source as Mac/Linux. Its unchanged qualification
driver verifies the build, ABI, installed-plugin checksum, missing-value witness,
OLS/PPML transactions and both 70-case/202-estimation input oracles with zero
numeric windows. Run `win-20261102T074954Z-393b5d4e` reports 1,032.382 seconds,
all transient objects deleted, the lock released and the machine stopped.
Windows remains serial; this result does not qualify parallel input there.

The narrower [Linux comparison](../bench/results/input-beta14-geometry-linux-20261102.json)
retains [all 1,008 observations](../bench/results/input-beta14-geometry-linux-20261102.csv)
and passes 41 of 42 settings (38 of 39 protected settings plus all three advisory
`in` settings). The prior c3/28, c6/4 and c8/16 regressions are within the guard:
whole-call incumbent/candidate medians are 1.134/1.1275, 4.279/4.283 and
3.174/3.001 seconds respectively. One protected full-sample setting fails:
64 regressors at four workers (c2), 5.927/6.2615 seconds, **+5.64%**
(38.15 ms beyond the 5% allowance). It is slower
in every paired block (+5.89%, +7.80%, +4.64%, +4.99%). Loading medians are nearly
unchanged (641.5/646.1 ms); native time including loading is 5.4817/5.7560 seconds.
Per-call native-minus-load medians are 4.8593/5.0607 seconds. These separate
medians are not additive, and no causal explanation for the later penalty is
established. The candidate must not be represented as passing the release guard.

Short-`in` whole-call medians improve by 23–39% on Linux; the advisory exception
is unused. All 84 actual-plugin science preflights pass exactly, including full
sample signatures, and every frozen workload retains incumbent phase/block
geometry. The build/science job 8394306 exits zero on scc-wo4. Timing job 8394307
on the same physical host retains every observation, passes isolation and records
`failed=0`, `exit_status=1`, 2,288 seconds and 5.569 GB maximum virtual memory.
The exit is the single performance rejection. No observations were excluded or
resampled. This completes the declared compatibility narrowing; another candidate
or timing run is not selected automatically. Johannes's `in` waiver does not
cover this full-sample case. The implementation and native artifacts remain a
review candidate pending the performance-policy decision.

The narrower [three-platform root package](../bench/results/input-beta14-geometry-package-review-20261102.json)
passes the full `scripts/run_tests.sh`, including all 36 installation files and
native runtime binding. Fresh installations on Mac Stata 19 and Linux Stata 15/19
verify every installed byte, beta.14 selftest and independent restricted OLS/PPML
coefficient, variance and sample witnesses. Linux installation job 8394495 exits
zero. These checks make the candidate reviewable but do not waive the remaining
full-sample performance failure; no beta.14 release or main-package push occurred.

The final three-platform review archive also passes its exact-archive Mac Stata
19 installation smoke test. Its source archive has zero dependency advisories;
2,794 native, Stata, build, dependency and license files match the offline-built
source archive byte for byte. The package report retains both archive hashes,
packaging source and smoke result. Its review-only manifest remains ineligible
for publication until the full-sample performance decision.

The actual candidate binaries pass 68 licensed Stata 19 fixtures on both Mac
and Linux, all 77 exact incumbent replay cases on each, scalar input oracles at
multiple worker counts, accessor-failure rollback, and live console Break while
an OLS or PPML numeric window is open. The full source suite and the strict suite
with the candidate root package pass. Fresh local installations verify all 36
installed files and independent restricted-sample OLS/PPML examples on Mac 19
and Linux 15/19. Linux 15–18 replay/prefix checks and Linux 17/18 input oracles
also pass.

The [Mac record](../bench/results/input-beta14-macos-20261101.json) includes
passing Stata 18/19 GUI input oracles. Interactive GUI Break remains unqualified:
AppleScript delivery timed out with an open window, and SIGINT terminated the
GUI process. Local Mac 15/17 installations did not reach fixture startup.
Native Intel-host Stata testing remains outstanding. The
[Windows record](../bench/results/input-beta14-windows-20261101.json) retains
the guarded Stata/MP 19 qualification, including installation, independent
missing-value witnesses, input oracles at two/eight workers and failure rollback.
Its serial/parallel comparison passed the frozen rule; beta.13/beta.14 timing
and interactive Windows Break were not measured.

The many-FE, two-FE structural-gate and ownership/deferred-cell improvements are
accepted, as are the many-FE geometry policy with one documented timing exception
and the PPML two-FE CMG reuse and lean protocol released in v0.5.0-beta.7.
The OLS method-reporting correction changes diagnostic labels and
fallback flags only; solver selection, iteration budgets, numerical calculations
and PPML reporting are unchanged.

## Upstream migration candidate

(Historical: fereg no longer depends on `within`, retired in Milestone 3; see
[the within retirement](#within-retirement-milestone-3-development-mac).)
Beta.11 was the published migration from beta.10. The
[compatibility report](within_migration.md) records the frozen dependency bases,
patch evidence, qualification results and accepted performance tradeoffs.
The final source pin is `8d401d82`: the essential migration stack plus counts,
back-substitution, gather and the general Fold running accumulator. Johannes
accepted all four optimizations and their disclosed individual-patch tradeoffs
on 2026-10-30, and authorized combined building and qualification. Scatter slices
remain excluded; no processor-specific dispatch or scientific-semantic change
is included. Combined source, native and preflight checks passed on Mac and Linux.
Johannes conditionally accepted the disclosed eight-/sixteen-FE performance
regressions on 2026-10-30, provided investigation of the two large fitted-value
disagreements supports the accuracy of the estimates. That numerical condition
is now met: independent optimum enclosures and actual beta.10/beta.11 checks pass
for both full designs on Mac and Linux. His acceptance of the disclosed
eight-/sixteen-FE performance tradeoffs therefore applies. This selects the
beta.11 migration candidate without relabeling the adverse performance results.
The final Linux two-FE PPML control completion now passes all four cells,
including the previously timed-out baseline. The original failure remains
retained. Johannes subsequently authorized finishing release qualification and
publishing beta.11 as the current GitHub version; the package checks below
record native and installation qualification. The earlier counts-only `534ea64f` candidate had
55 passed Mac cells, two inconclusive cells and one regression; its five valid Linux hosts
were below the six-host requirement. Those classifications remain unchanged.

The final combined artifacts bind fereg `65500be6` and within `8d401d82`.
[Mac source checks](../bench/results/within-final-source-macos-20261030.json)
and [Linux source checks](../bench/results/within-final-source-linux-20261030.json)
pass the complete source suites and dependency checks. The
[Mac native record](../bench/results/within-final-native-macos-20261030.json)
passes all twelve native stages, including replay, licensed fixtures, actual
Break/rollback, memory checks and sixteen large-design calls. The
[Mac final preflight](../bench/results/within-final-preflight-macos-20261030.json)
passes all 28 actual beta.10/final-candidate calls, fourteen arm comparisons
and seven cross-worker family checks. Six families execute within; the dense
three-FE MAP-completed family remains an exact control. These records preserve
source/artifact identities, scientific results and output hashes; large captures
are disposable after verification. They establish correctness prerequisites,
not combined performance acceptance or publication. The corresponding
[Linux native record](../bench/results/within-final-native-linux-20261030.json)
and [Linux final preflight](../bench/results/within-final-preflight-linux-20261030.json)
pass the same twelve native stages, sixteen large-design calls and final
28-call science grid, with successful terminal SCC accounting. The larger
companion panel and combined performance comparison remain separate gates.

The [final Mac comparison](../bench/results/within-final-macos-20261030.json)
retains all 1,508 calls across 58 settings and six mirrored blocks, with no
retries, exclusions or scientific failures: 53 settings pass, four regress and
one is inconclusive. The sparse three-FE full-call medians improve from 5.268 to
5.026 seconds at one worker and from 1.916 to 1.681 seconds at four; the original
Mac slowdown is recovered. Eight-FE medians increase from 181.2 to 200.7 ms at
one worker and from 82.6 to 92.2 ms at four. Sixteen-FE medians increase from
444.3 to 497.0 ms and from 201.0 to 229.9 ms. Their qualified median block ratios
are respectively +10.54%, +11.63%, +11.78% and +14.36%; these are distinct from
ratios of the displayed call medians. The four-worker 64-regressor AKM setting
is inconclusive. Fixed-worker scientific fingerprints, independent references
and cross-worker checks pass. Ordinary desktop activity remains in the timings;
the Mac is one machine, not a set of independent Linux hosts. These performance
results alone did not accept beta.11; Johannes's subsequent acceptance and
release instruction are recorded above. No further optimization round is included.

The [initial final Linux panel](../bench/results/within-final-linux-initial-20261030.json)
retains 1,981 calls: 580 on each of three completed physical hosts and 241 in
three incomplete attempts on a fourth host. The first process-observation gap
exceeded 7.5 seconds after the observer added a fresh five-second delay to an
initial scheduler census. All 116 scorer comparisons remain insufficient at
three hosts. Eight- and sixteen-FE settings at both worker counts have adverse
host medians on all three completed hosts; these observations remain retained.
The bounded observer-only completion preserves these hosts and all failed
attempts, and permits only the three missing independent units.

That [completion](../bench/results/within-final-linux-completion-20261030.json)
now supplies six distinct isolated physical hosts and all 3,480 eligible calls;
all 241 original failed-attempt calls also remain retained. All array tasks and
the report have successful terminal accounting, and report validation has no
errors. Of 58 unique endpoints, 50 pass, four regress and four are inconclusive.
Eight-FE complete-call medians increase from 421.1 to 462.5 ms at one worker
and 213.2 to 229.9 ms at four; sixteen-FE medians increase from 895.8 to
992.4 ms and 468.9 to 516.7 ms. Their median block ratios are respectively
+9.72%, +8.20%, +10.74% and +9.75%, distinct from ratios of the pooled call
medians. All six host excesses are positive for each of these four endpoints.
Sparse three-FE at one worker is inconclusive (+4.89% median block ratio;
12.731 to 13.362 seconds in full-call medians). The four-worker 16-regressor
AKM, four-FE T1 and small-third T1 endpoints are also inconclusive. The original
near-confounded-third T4 setup regression is recovered: its complete-call
comparison passes at a -2.28% median block ratio. Johannes subsequently accepted
these eight-/sixteen-FE regressions conditionally
on resolving the numerical concerns below. The original classifications and
inconclusive settings remain retained. No further timing or optimization loop
is authorized.

The large companion panel first stopped on a baseline OLS reference convention:
fereg's documented pairwise absorbed DoF is 2,095,755, whereas the strict fixest
reference reports 2,095,753. Johannes approved preserving the pairwise policy.
The [independent input-graph witness](../bench/results/within-companion-reference-conventions-20261030.json)
reproduces fereg's counts and supplies the matching OLS covariance normalization
without changing raw references, production semantics or tolerances. It also
checks PPML's separate convention: sixteen identified slopes, reported finite
rank seventeen including the normalized constant, and one direction removed
from absorbed rank. PPML covariance remains unchanged. Full checks of retained
native arrays and the unfinished companion calls remain required; the graph
witness alone does not qualify those estimates.

The subsequent [retained-array diagnostic](../bench/results/within-companion-fitted-diagnostic-20261030.json)
records a separate unresolved fitted-value disagreement on beta.10. Rechecking
saved captures started no new native calls. After the approved reference
normalization, coefficients, covariance and scalar criteria pass, but 1,317 of
ten million OLS rows and 19 of one million PPML rows exceed the unchanged
`1e-9 + 1e-7 * max(abs(native), abs(reference))` fitted criterion. Maximum
absolute differences are `4.5843e-6` and `5.6730e-7`; RMS differences are
`3.9842e-9` and `1.1754e-9`. Ordered input hashes, sample, reconstruction,
objectives and normalized model scores pass for both fits. Beta.10's maximum
normalized scores are `2.4602e-13` and `2.4550e-11`, compared with reference
scores `4.6608e-9` and `6.7322e-9`. This raises a reference-accuracy concern
but does not establish pointwise correctness. The diagnostic completed in
15.98 seconds with successful terminal SCC accounting and zero fits. Both
fitted gates remain failed. Johannes subsequently approved continuing the seven
unaffected families, retaining both failures. The bounded continuation covers
28 native calls per platform; Mac adds four weighted-OLS memory calls, while
Linux reuses the four already passed weighted-OLS admission/refusal checks.
Both held families remain outside this continuation, and a new failure stops
the subset. No numerical tolerance or production behavior changed.

The subsequent [bridge witness](../bench/results/within-companion-bridge-witness-20261030.json)
establishes reference error on a subset, independently of either estimator.
In the worker/firm incidence graph, a bridge cell has a free intercept even
with the finite regressors and third FE present. At an OLS or positive-outcome
Poisson optimum its fitted sum must equal its outcome sum. A single-row bridge
therefore has the exact optimum `fitted = y`. All 7,506 OLS and 728 PPML
single-row bridges pass for fereg; the reference fails on 104 and 12 rows.
Maximum fereg errors there are `1.1257e-11` and `2.5644e-10`, respectively.
The graph algorithm passes eleven independent edge-removal checks and every
claimed bridge cut among the failing cells is checked against all original
unique mover edges. Job 8358713 completed on `scc-gr4` in sixteen seconds with
successful terminal accounting and no estimation calls. The maintained
[graph oracle](../crates/fereg-oracle/python/bridge_optimum_witness.py) and
[four independent tests](../tests/python/test_bridge_optimum_witness.py) also
pass locally, including weighted dummy OLS with extra regressors and an extra FE.

Conditionally optimizing each bridge-cell intercept removes 399 OLS and eighteen
PPML discrepancies, leaving 918 OLS rows and one PPML row. Multirow cell
corrections hold their within-cell contrasts fixed and are not an enclosure of
the full optimum. They therefore do not close either original fitted gate or
qualify the candidate. No reference, tolerance or runtime behavior has changed.

The [independent optimum enclosure](../bench/results/within-companion-optimum-enclosure-20261030.json)
now covers both complete retained beta.10 fits, including all 1,317 OLS and
nineteen PPML disputed rows. The [derivation](within_numerical_enclosure.md)
constructs explicit score-balancing graph flows and a small stayer contrast
matrix, separately checks model-space feasibility, and converts these into
prediction and identified-coefficient bounds. OLS fitted error is bounded by
`1.93753e-10` and finite coefficient error by `3.60405e-12`. PPML's sharpened
curvature bound places every fitted error below 70.46% of its unchanged tolerance
and finite coefficient error below `6.74899e-10`, versus the smallest coefficient
tolerance `1.27198e-9`. This supports the estimates independently of fixest.
The original comparator failures remain failed comparisons, with proved
reference errors on the exact bridge subset; they are not migration bugs.

The first diagnostic's PPML report failed on NumPy boolean serialization after
retaining its looser OLS bound. Its failed exit and result remain in the record.
The bounded refinement passed on `scc-gr4`, with successful terminal accounting,
67 seconds wall time and 4.179 GB maximum virtual memory. It shortened the
explicit graph-flow paths and tightened score reduction bounds; it ran no new
native estimate or reference fit. Twelve dense analytic checks passed on Linux;
eighteen expanded OLS/Poisson cases plus the scalar curvature refinement pass
on Mac. Standard floating-arithmetic allowances apply; this is not a formally
verified interval-arithmetic implementation. The subsequent completed Mac/Linux
checks below include the center's uncertainty inside the original tolerance
when checking the actual beta.11 artifacts against this enclosed solution.

The [completed Mac enclosure checks](../bench/results/within-enclosed-companions-mac-20261030.json)
pass all eight beta.10/beta.11 calls on the two formerly held families, at one
and four workers, plus all eight cross-arm/cross-worker comparisons. Each call
adds the independently proved center uncertainty to its difference from that
center before applying the unchanged accuracy tolerance. The worst fitted bound
uses 19.3736% of tolerance for OLS and 70.4552% for PPML; the worst coefficient
bound uses 0.2651% and 53.0589%, respectively. Both arms execute within, with
403 OLS and 149 PPML dependency iterations in every call. Scalar/covariance,
model-score, reconstruction and discrete checks also pass. The controller
completed in 524.2 seconds, with zero retries. These are correctness checks,
not a new performance or fixed-worker repeatability comparison. Together with
the earlier unaffected subset, all 36 planned Mac native cells now pass.

The [Mac PPML memory completion](../bench/results/within-enclosed-memory-mac-20261030.json)
passes the four remaining original memory cells: beta.10 and beta.11 both meet
the independent optimum enclosure when admitted with 64 GiB available; both
refuse at 512 MiB available (zero usable bytes), preserving every matrix and
observation column and performing no solver work. It completed in 78.9 seconds,
with zero retries. Combined with the earlier weighted-OLS checks, all eight
planned Mac memory cells pass. This tests early refusal, not late allocation
failure or performance, and does not close the separate Linux coverage gaps.

The [Linux enclosure attempt](../bench/results/within-enclosed-companions-linux-20261030.json)
passes both retained-center preflights and revalidates the original beta.10 OLS
cell, but the first new beta.11 ten-million-row OLS T1 call reaches its fixed
600-second limit without returning estimates. Its last trace reports a certified
FE solve, with all seventeen RHSs converged; that does not establish completion
of estimation or explain the timeout. The sequence stops, leaving six cells
unstarted. Job 8359778 on `scc-pf8` records scheduler `failed=0`, process exit 1,
988 seconds wall time and 11.256 GB maximum virtual memory. This is incomplete
qualification, not an observed numerical mismatch or a performance verdict.
Johannes approved one retry at 1,200 seconds plus the six unstarted checks,
within a further 3,800-second total limit. Both original Linux timeouts and all
original reference comparisons remain retained.

The [approved Linux completion](../bench/results/within-enclosed-continuation-linux-20261030.json)
passes all eight beta.10/beta.11 cells on the two formerly held designs at one
and four workers. It reuses the original baseline OLS T1 result and runs exactly
seven new calls, including the one approved retry; no further resampling occurs.
The previously timed-out candidate OLS T1 call completes in 153.94 seconds on
`scc-b11`. This does not diagnose its earlier timeout on `scc-pf8` or establish
a performance comparison against the reused baseline from a different host.
All eight cross-arm/cross-worker comparisons pass. Maximum fitted bound ratios
are 0.193735 for OLS and 0.704552 for PPML; maximum finite-coefficient ratios are
0.002651 and 0.530589. All calls execute within, with 403 OLS and 149 PPML
dependency iterations. Scalar/covariance, model-score, reconstruction and
discrete checks pass; all 1,317/19 raw reference discrepancies remain recorded.
Job 8360696 has successful terminal accounting, 557 seconds wall time and
14.519 GB maximum virtual memory. The checker/controller takes 555.1 seconds,
within the approved 3,800-second budget. This closes both fitted-value accuracy
questions for the actual candidate on Mac and Linux. At this checkpoint Linux
passed 32 of 36 large native companion cells; the final PPML completion below subsequently
closed the four remaining cells. No performance or new fixed-worker repeatability
claim is added by this science-only completion.

The [Linux memory completion](../bench/results/within-enclosed-memory-linux-20261030.json)
passes all eight original admission/refusal cells: it revalidates five retained
cells and runs only the three unstarted PPML cells, with no native retries.
Both PPML arms pass the independent optimum enclosure with 64 GiB available.
Both refuse with 512 MiB available (zero usable bytes), preserve every matrix
and all 28 observation columns, and enter no solver work. Job 8360885 on
`scc-fi2` completes with successful terminal accounting, 179 seconds wall time
and 2.533 GB maximum virtual memory. The prior memory jobs plus this completion
use 470 seconds of the original 3,600-second budget. This verifies early
refusal and rollback, not late allocation failure or performance.

The large companion coverage is therefore:

| Platform | Native cells passed | Memory cells passed | Remaining limitation |
|---|---:|---:|---|
| Mac | 36/36 | 8/8 | No remaining cell in this panel |
| Linux | 36/36 | 8/8 | Original execution timeouts retained; completed controls pass |

The [final Linux PPML completion](../bench/results/within-ppml-release-linux-20261030.json)
passes both versions at one and four workers and all cross-version/cross-worker
comparisons on the ten-million-row two-FE control. Job 8361190 on `scc-tn3`
completes within its 4,000-second controller budget, with successful terminal
accounting. It retains the original 600-second baseline timeout and does not
establish its cause. No runtime, input or scientific tolerance changed. This
completes the two numerical investigations and the full Mac/Linux companion
panel, supporting the selected beta.11 candidate with Johannes's accepted
eight-/sixteen-FE tradeoffs. Windows and package-installation evidence are separate.

The [Linux continuation](../bench/results/within-unaffected-companions-linux-20261030.json)
passes all eight weighted-OLS Auto/LSMR calls, then stops when the ten-million-row
two-FE PPML beta.10 call reaches its 600-second limit. No candidate PPML or slope
call starts after that timeout. The final trace reports an inner reduced solve
with nineteen iterations; it does not establish the cause of the timeout.
Terminal accounting records `failed=0`, `exit_status=1`, and 12.363 GB maximum
virtual memory. This is incomplete qualification, not a numerical mismatch or a
performance comparison. Johannes approved the sixteen previously unstarted
Linux slope calls within a further 4,800 seconds of the original budget. That
[slope-only continuation](../bench/results/within-slope-companions-linux-20261030.json)
passes all sixteen calls and the cross-arm/cross-worker comparisons on
`scc-ei3`, covering sorted and permuted ten-million-row one-/three-slope designs.
Job 8357906 completed in 856 seconds with `failed=0`, `exit_status=0`, and
5.052 GB maximum virtual memory. Each call executes within and passes its
independent analytic witness. There were no retries or tolerance changes; the
PPML timeout was not retried. At this checkpoint, the earlier eight weighted-OLS
calls brought Linux to 24 of the 28 calls in the approved unaffected subset.
The final PPML completion above subsequently closed the remaining four checks.
The four previously passed Linux weighted-OLS memory checks remain valid.

The [Mac continuation](../bench/results/within-unaffected-companions-mac-20261030.json)
passes all 28 native calls and 28 cross-arm/cross-worker comparisons across the
seven approved families: weighted OLS Auto and explicit LSMR, ten-million-row
two-FE PPML Auto, and ten-million-row one-/three-slope designs with sorted and
permuted rows. Six families execute within; the PPML family is an independent
unaffected route control. The four weighted-OLS memory calls also pass: both
plugins meet the reference when admitted, and both refusals preserve every
matrix and observation column without solver work. The controller completed in
2,405.3 seconds including transfer and staging, with zero native retries. One
preparation-only failure occurred before any native call: the original helper
confused the reference JSON and NPZ hashes. Its narrow correction checks each
file against its own pinned hash, and both the failure and correction remain in
the record. Completed captures and staged inputs were removed after verification.
These checks establish correctness for the approved subset, not performance or
new fixed-worker repeatability evidence. The later optimum enclosures and Mac/Linux
completion above resolve the two numerical concerns on both platforms. The original
Linux two-FE PPML control timeout remains retained; the final PPML completion
above closes those four large-panel cells.

The separate [Fold accumulator comparison](../bench/results/within-fold-performance-20261030.json)
completes all 516 calls with exact science and solver work. The affected Mac
single-worker fit improves 14.89%; solve medians fall from 5.201078 to 4.313869
seconds, with nearly unchanged setup and 390 iterations over 19 RHS solves. All twelve
Mac intervals pass. Linux has five passed primary intervals and one inconclusive
weighted categorical T1 control, including an 11.15% slower host with both
blocks adverse. Actual sparse fits slow by host medians of 2.96% at one worker
and 1.87% at four. Two construction intervals are also inconclusive. All four
adverse primary and four adverse construction blocks remain retained; no host
is replaced. Johannes accepted this tradeoff for inclusion in the final candidate. Fold
arity-two-through-four performance remains unmeasured, and this independent
comparison did not accept beta.11. The combined beta.10 comparison and subsequent
acceptance are recorded above.

The [final bounded diagnosis](../bench/results/within-final-slowdown-diagnosis-20261030.json)
uses retained evidence and independently reproduced disassembly, with no new
timing or profiling. Loss of FMA does not explain the inspected hot loops.
Changed categorical accumulation is the strongest supported Mac explanation;
the general Fold patch restores register accumulation without restoring the old
arithmetic or whole-operator fusion. Counts removes unnecessary compensation
for exact dense counts, and the five completed Linux hosts descriptively recover
the original setup penalty. Hardware-level attribution, the missing sixth host
and adverse optimization controls remain unresolved. The diagnostic effort is
closed; final optimization inclusion and combined verification remain separate.
The separate counts/back-substitution commit `d1721a25` passes independent source
review, 26 focused solver tests and strict library Clippy. The final four-patch stack now proceeds to combined source and native checks
and a separately declared beta.10 comparison.

Johannes approved retiring the general more-than-two-FE pair-upgrade experiment;
its explicit h/k and builder-Break requests must fail clearly. Two-FE h/k and
builder-Break behavior, public Auto escalation, supported OLS/PPML paths and
independent scientific witnesses remain required. Approval of that retirement
does not accept beta.11 or waive the recorded residual-witness failure.

Johannes accepted upstream's whitening correction, supported by the independent
weighted slope-boundary oracle, for continued qualification. The original
fork/candidate rank-parity failure remains recorded. The native checks below
cover declared published results, factor slots/base flags, PPML and replay;
beta.10 performance must still pass. This decision does not authorize production
adoption or acceptance of the documented rejected near-boundary fits. The completed
original `d3407a16` main panels below each contain a regression and inconclusive
comparisons; the new counts-only panels also do not qualify beta.11.

Focused Rust 1.85.1 adapter checks passed 79 main tests and four subprocess
witnesses, including explicit LSMR, controlled routing, selector retirement,
two-FE CMG and PPML. The separate memory/admission set passed 97 distinct tests;
85 memory and four retained-storage tests passed again after the helper
refactor. The core complexity ratchet decreases from 113,243 to 113,029
production lines and from 1,458 to 1,451 dispatch candidates. Essential host
cancellation and the sanctioned iteration-Break witness increase plugin lines
from 15,527 to 15,675 and dispatch candidates from 141 to 143. The combined
runtime is 66 production lines smaller, with all large-size counts unchanged.

The historical extreme-weight fixture initially failed an expected fallback
flag, while Auto and Off both passed the original `1e-10` certificate and
matched its independent analytic projection within `2.23e-16`. The approved
test update preserves the fixture and now requires analytic error below
`1e-14`; counted-failure boundaries and upstream failing-operator witnesses
test fallback budgets independently of roundoff. The original expectation and
diagnostic values remain in the compatibility report. The full source suite
failed at `automatic_structural_map_uses_admitted_shared_operations`, which
expected positive reported solver workspace, after 1,535 core passes and three
ignored tests. Diagnosis showed Auto correctly admitted shared certificate and
coefficient banks while refusing an optional LSMR checkpoint; its separate
solver-workspace charge was legitimately zero. A test-only correction now
checks the actual reusable charges and total selected bytes against the cap,
preserving policy and numerical comparisons. Both this test and the explicit
LSMR checkpoint-admission test passed focused reruns. The original full-suite
failure remains recorded. The rerun passed all 1,536 core tests (three ignored),
then stopped at the PPML separation routing census: retained-CG developer
configurations 1/2 for seed 17 now fail their LSMR retry. Public Auto retains
exactly the incumbent sample and separation decisions in all three diagnosed
configurations and matches the independent 228-row LP witness. Causal isolation
found identical coefficients and one solver iteration, but different residual
reconstruction: upstream's weighted-residual unscaling gives an FE certificate
of `1.0478754743e-8`, just above this call's `1e-8` tolerance; direct unweighted
model subtraction gives `3.6898581565e-11`, matching the incumbent. No census
expectation or scientific tolerance has been changed. The opt-in prepared-model
output option passed its isolated and combined dependency checks; beta.11 now
pins `d3407a16` and selects it explicitly. Both PPML separation routing tests
now pass, including fixed-worker repeatability and cross-worker agreement;
the independent diagnostic remains ignored. The final Mac source-only suite
completed with exit status 0: 273 Python tests (12 skipped), 1,537 core unit tests (three
ignored), workspace scientific/integration and plugin/FFI tests, isolated
allocation audits, strict Clippy, fuzz compilation, portable/C guards and the
complexity ratchet passed. The compatibility report records its exact log
identity and environment. Installed-package admission was explicitly omitted.
Native correctness checks are recorded separately below; source integration
is not production acceptance.

The [native record](../bench/results/within-native-20261029.json) binds fereg
`d96eccfc`, within `d3407a16` and both actual plugins. Mac and SCC builds,
all 67 licensed fixtures per platform, and four OLS/PPML LSMR Break runs per
platform passed. Break traces prove completed solver work, distinct cancellation,
rollback and successful reuse. The 77-case/693-call replay passes science,
fixed-worker repeats and cross-worker checks on both platforms. The original
Mac declaration failed three PPML diagnostic magnitudes; a source-reviewed
declaration correction passes reassessment without native reruns, preserving
that failure. Linux's first replay launch failed on an ABI-only environment
setting; a separate replay-only repair passed without repeating the independent
fixture/Break stages. Both platforms also pass the four-case public slope
contract: eight oracle fits, eight expected atomic rank-boundary rejections and
four frontend refusals each. Earlier failed expectations remain recorded under
Johannes's instruction to preserve existing rejections. Two additional mapped
fixtures pass on each platform, covering four weighted OLS reuse fits and
eight offset PPML fits against Poisson references. Their traces establish
ordinary Within OLS and augmented joint-PPML execution; the Linux timing
harness was compiled but made no performance calls. Package admission,
beta.10 performance and production acceptance remain outstanding.

The qualification science preflight passes all 28 calls and 14 comparisons
on each platform across seven prospective many-FE families, both actual binaries
and one/four workers. Independent oracles and cross-worker scientific/discrete
checks pass. Six families reach Within in both binaries; the dense three-FE
family completes in the unchanged MAP pilot and remains exact. Linux job
8327872 on `scc-pg7` completed with scheduler failure/exit status 0/0 and all
terminal markers. Its 28 collected result/log pairs and path traces were
independently verified; column files remain on SCC. The native record retains
the exact identities and accounting. This is no timing or additional
fixed-worker repeatability claim; the completed main timing panels follow.

The [main qualification record](../bench/results/within-qualification-20261029.json)
retains every call, order, warmup, timing, solver row and scientific fingerprint
in lossless compact payloads, with artifact/input/source hashes and decoding
instructions. Both panels use the unchanged actual beta.10 and beta.11 plugins,
29 families at one/four workers, and the frozen max(5%, 1 ms) allowance.

| Panel | Native calls | Independent units per cell | Passed / inconclusive / regression |
| --- | ---: | --- | --- |
| Mac | 1,508 | Six mirrored blocks on one desktop | 51 / 6 / 1 |
| Linux | 3,480 | Six physical hosts, two blocks each | 55 / 2 / 1 |

Linux array `8328471` used six distinct whole-node allocations; all six jobs
and report job `8338074` ended with scheduler failure/exit 0/0. There were no
native errors, retries or discarded timings on either platform. The maintained
reports passed scientific, fixed-worker and cross-worker checks; six families
actually execute Within, while the dense three-FE MAP control remains exact.
Independent read-only review reproduced all block orders, report classifications
and bounds. These are one-sided `.025` bounds under the declared unit assumptions,
with no simultaneous-coverage claim. The two reference labels alias the same
beta.10 artifact and are counted once in the table.

Mac `campaign-three-sparse-t1` regresses: median paired ratio `1.12666`, with
raw timed medians 5.59002 versus 6.30906 seconds. Both arms record 19 RHS solves,
390 iterations and outer counts 23+14. Linux `near-confounded-third-t4` regresses:
ratio `1.11520`, with excess above the allowance bounded at 1.58–8.60 ms.
Both arms retain five RHS solves, 71 iterations and outer counts 22+10.
Recorded Linux setup medians increase 14.90–16.70 ms across hosts while solve
medians decrease 1.62–4.41 ms. These phase observations do not identify a cause.
All these calls retain LSMR with additive Schwarz and zero fallback.

Linux's other inconclusive cells are `campaign-akm-k16-t4` and
`campaign-three-sparse-t1`. The record lists all six Mac inconclusive cells.
Mac retains the explicitly permitted Claude background activity: 47.95 sampled
CPU seconds across five bound processes; this is neither an isolation claim nor
a reason to discard adverse measurements. Both complete panels remain
`needs_discussion`; no repeat sampling, production adoption or performance
acceptance follows from them.

Johannes subsequently authorized exactly one function-level profile per frozen
plugin for each platform's regressing cell, followed by a justified fix in the
fork only. All four calls completed and passed their independent oracle, exact
per-artifact scientific fingerprint and recorded solver-work checks. The
[qualification record](../bench/results/within-qualification-20261029.json)
retains their diagnostic summaries and source/profile identities. Mac sampling
and disassembly point to categorical accumulation's per-row bounds checks;
Linux sampling and allocation inspection point to unnecessary compensation
arrays for exact unweighted categorical counts. These motivate separate patches
that preserve arithmetic. They do not establish causal effects or recovered
performance: each profile contains one instrumented call, and Linux's user-space
CPU event excludes kernel allocation costs. Linux job `8338538` completed on
`scc-we4` with scheduler failure/exit 0/0. Any revised candidate requires a
separately declared comparison before acceptance; the completed main-panel
verdicts and all adverse observations remain unchanged.

The two independent fork optimizations now have Mac and Linux public science evidence
in the [patch record](../bench/results/within-optimizations-20261029.json).
Dense categorical counts compare `32dde1bd` with `ba409ada`; categorical Fold
slices compare `6f2ead13` with `1f16b437`. Both use Rust 1.85.1 with generic CPU
release builds, preserve arithmetic and leave the frozen beta.11 plugin
unchanged. The [migration account](within_migration.md#independent-fork-optimization-checks)
links the frozen commits on Johannes's fork and defines their narrow production
scopes and retained independent tests. Neither has been pushed upstream or
submitted as an upstream pull request.

| Patch panel, per platform | Native processes | Exact base/patch comparisons | Exact one/four-worker comparisons | Maximum independent error |
| --- | ---: | ---: | ---: | ---: |
| Dense counts, Mac and Linux | 24 each | 12 each | 8 preconditioner apply each | `7.20e-14` |
| Scatter slices, Mac and Linux | 40 each | 20 each | 8 full solves each | `1.231e-9` |

Every process includes a repeated four-RHS group, with no timers or retries.
Both panels retain the `1e-8` independent witness bound. The two-FE harness uses
solver tolerance `1e-11`; the weighted/sloped entrypoint harness uses the already
approved `1e-12`. Counts' full solves are restricted to one worker because its
frozen upstream base has a known four-worker LSMR repeatability failure; the
combined candidate's fixed-worker requirement remains unchanged. Read-only
reviews checked both completed panels, their exact artifact/source bindings
and every declared comparison. Linux public jobs `8339222`/`8339223` completed
on `scc-tb4` with scheduler failure/exit 0/0, using four-slot shared correctness
allocations, generic CPU builds and GCC 12.2.0. They establish no performance or
isolation claim. Successor `8339224` passed the eight count and three scatter
private witnesses on that host with scheduler failure/exit 0/0; this is a
focused-filter result, not a Linux full-suite claim. These checks do not
establish beta.11 performance acceptance;
the original regressions and inconclusive results remain in force.

The [independent performance record](../bench/results/within-optimization-performance-20261029.json)
now retains all 840 calls on six whole-node Linux hosts: 168 untimed preflights
and 672 timed calls, with exact order, scientific fields, block/host aggregates
and all source/artifact/configuration/accounting identities. Array `8340554`
completed with scheduler failure/exit 0/0 on all six tasks; science and isolation
checks pass, with no native errors, retries or discarded timings. Read-only
reconstruction confirms 23 passed and five inconclusive endpoint comparisons,
with no positive lower-bound regression. Both patches remain `review_required`:
counts has an inconclusive sparse construction control; scatter has three
inconclusive cache construction controls and a weighted memory four-worker
solve. The two endpoints are protected separately; six-host median bounds at
one-sided alpha `.025` have no simultaneous-coverage guarantee. Neither patch
is performance-accepted. The [migration account](within_migration.md#independent-performance-and-combined-source-candidate)
reports the improvements and unfavorable bounds without claiming beta.11 recovery.

Mac's two operational launches retain 56 untimed science calls and zero native
performance timings. A denied hardware query stopped the first; competing
unrelated Stata Monte Carlo work stopped the second at admission. No process
was stopped and no adverse measurement was excluded. Mac timing remains blocked.

Historical combined within source [`4beaa8af`](https://github.com/johannes-schmieder/within/commit/4beaa8afcdc56ddee6f29b3cbd5bf151f0f2d5c5)
was integrated at fereg `379bf7c8` and published on Johannes's fork only.
Its [source record](../bench/results/within-optimized-integration-20261029.json)
retains 588 passing top-level tests, two passing isolated subprocess tests and
one ignored fixture-regeneration test on Mac Rust 1.85.1. A missing cancellation
argument first caused test compilation to fail; the repair changes only the
private witness. fereg's updated pin passed 324 Python tests, 1,537 core tests,
189 non-FFI and 219 FFI plugin tests, scientific integration/oracles, strict
Clippy and portable checks. The original script exited 1 at stale complexity
locations; the first follow-up exited 1 for unused historical growth entries.
After approved metadata-only repairs, size/ratchet, mock-SPI and numeric guards
passed. All measured totals and limits are unchanged, both failures remain
recorded, and no broad rerun occurred. No new actual plugin has been built:
the earlier native evidence and performance verdicts still apply to `d3407a16`.
Fereg `65449fd6` restored the pin to `d3407a16`. Its manifest and both lockfiles
match tested fereg `d96eccfc` exactly; the only shipping-source difference is an
explanatory memory comment. The earlier source/native correctness evidence is
reused, with successful locked offline metadata checks for the 59-package
workspace and 64-package fuzz workspace. No plugin rebuild or new fereg native
test was run.
The original beta.11-versus-beta.10 regressions remain distinct from the standalone
scatter weighted-control concern. No recovery or acceptance follows from the
restoration. At that stage, additional strict-reference fits remained unapproved;
the approved extension below now has results. The held back-substitution patch,
upstream submission and
production adoption remain outside this result.

Johannes subsequently approved testing counts-only `534ea64f`; it is now the
active source pin, with no scatter or back-substitution change. Its single Mac
source-only suite passed 324 Python tests, 1,537 core tests (three ignored),
189 non-FFI and 219 FFI plugin tests, independent scientific oracles, allocation
audits, strict Clippy, fuzz, portable/C guards and complexity checks. The
[migration account](within_migration.md#independent-performance-and-combined-source-candidate)
binds the exact pin diff, environment and log hash. Actual-plugin checks now pass
on both platforms: all 67 licensed fixtures, four iteration-Break runs, mapped
memory/reuse/slope checks and the 77-case/693-call replay. Replay matches protected
`d3407a16` exactly with 308 fixed-worker and 231 cross-worker checks. Each platform
also passes 24 targeted calls covering dense unweighted Schwarz counts, actual
generation-2 retained-CG-to-LSMR retries and public Auto controls. The unchanged
28-call preflight against beta.10 passes on each platform. Operational launch,
build and environment failures remain recorded with their bounded repairs.

The [counts-only record](../bench/results/within-counts-qualification-20261029.json)
retains all 4,489 main-panel calls (713 warmups and 3,776 timed calls), full
ordered scientific/work fingerprints, source/artifact/environment hashes and
report decisions. Mac's 1,508 calls produce 55 passed cells, two inconclusive
cells and one regression. Sparse three-FE at one worker has median paired ratio
`1.130373` (+13.0373%). Across all 12 timed calls per arm, complete-call medians
increase from 5.294788 to 5.983382 seconds, summed solver-row solve medians from
4.527583 to 5.218120, and setup medians decrease from 0.151976 to 0.150099.
Both warmup traces retain 390 iterations and 19 RHS solves. This identifies the
affected phase, not its cause. Bound desktop activity remains measurement noise,
with no retry or excluded adverse measurement.

Linux retains 2,981 calls. Five whole-node hosts complete 580 calls each; the
sixth stops after 81 calls on a startup observer gap of 7.843798 seconds against
the frozen 7.5-second limit. No native/scientific failure or competing process
is recorded. The unchanged report excludes that entire host but the compact
record retains every call, including its final unpaired warmup. All five-host
cell medians are within the allowance descriptively; all 58 cells are still
`insufficient_hosts`, with null one-sided `.025` bounds. Report job `8345728`
completes with scheduler failure/exit 0/0. There are no actual retries or
discarded attempts. The confirmed Mac regression and Linux uncertainty block
acceptance; every earlier adverse observation remains retained.

The three strict large references now pass under Johannes's approved bounded
extension (job `8342499`, four workers, no retries): sparse three-FE OLS at ten
million rows, sparse three-FE PPML at one million and AKM PPML at ten million.
Scientific thresholds are unchanged, and earlier timeouts remain failures.
The separate large companion and eight-call memory-admission panels remain
unrun. Their status does not change the completed main-panel decisions.
The gather-zeroing investigation remains separate from this fixed candidate.

The [bounded diagnosis](../bench/results/within-scatter-diagnosis-20261029.json)
finds real weighted caller code changes but no cause for the adverse controls.
Job `8341332` failed a counter-label prerequisite before native work; a narrow
parser repair preserved that failure. Job `8341371` completed exactly two
profiled calls with exact science, zero lost samples and scheduler failure/exit
0/0. This single pair on previously stable `scc-zj4` did not reproduce the
slowdown. Samples cover the whole process, include unresolved parallel-helper
work, and cannot clear the earlier adverse hosts. Scatter remains held; counts
and the original beta.11 regressions retain their prior decisions. The record
also preserves the restored-pin size check's Python-selection failure and
successful interpreter-only repair. No new qualification or acceptance is claimed.

The Linux separation-fallback companion passes its 16 declared actual-plugin
calls on `scc-pi2` (job 8328529, scheduler failure/exit 0/0). The seed-17 weighted
PPML design retains the independent LP's 228 original rows and passes the
dense Poisson oracle in both binaries. All eight injected calls actually take
the successful generation-2 CG-to-LSMR retry, with 147 iterations over ten
RHS solves; public Auto is a control and uses no Within work here. Eight
fixed-worker repeats are exact, and four cross-worker comparisons pass science
and preserve decisions. At each worker count, public Auto and injected CG have
identical estimates and observation outputs. Across binaries, only the declared
IR interior/span certificate magnitudes and their trace digest differ; all
other scientific fields stay exact. The digest remains exact within a binary
across workers and repeats. Because it also aggregates unpublished discrete
internals, the cross-plugin exception does not prove full internal execution
identity. All 16 Linux result/log/column captures and the scheduler record were
independently rechecked without new native calls. The native record retains
their identities. The same declared Mac companion now passes all 16 calls,
including eight actual generation-2 retries, eight fixed-worker comparisons and
four cross-worker comparisons. Independent review confirms the same sample,
oracle and comparison scope. Its first launch stopped at output-directory
validation before native execution; correcting only that path required no
repeated native calls. Neither platform's checks make a performance or adoption
claim.

The separate sparse back-substitution optimization completed its practical Mac
comparison: 312 calls, 72 paired blocks and all 12 descriptive intervals passed
max(5%, 1 ms), with exact scientific signatures. All interrupted attempts and
unfavorable timings remain in the
[compact evidence](../bench/results/within-backsub-20261029.json), including
the explicitly retained Claude background activity. Linux passed all 12
host-median intervals across six isolated physical hosts, but one of 432
blocks exceeded the allowance by 2.528 ms. Johannes reviewed the retained
observation and instructed us to hold the optional upstream PR. This patch is
absent from the combined candidate and does not establish beta.11 performance
acceptance.

## PyFixest parity policies (development, 2026-10-31)

Johannes approved three policies from the October 31 PyFixest diagnostic: the
many-FE geometry recurrence stopping at the request with `within`'s stall rule
for the Jacobi trial, an ordinary PPML frame floor of 0.1 times the tolerance,
and Schwarz preconditioner reuse across PPML frames. Measured, only the third
holds up, and on the benchmarked routes it changes nothing. This is development
evidence, not qualification.

Setup: one whole SCC node (`scc-zj4`, Xeon Gold 6132, job 8381889) on the
beta.11 campaign's frozen inputs with PyFixest 0.60.0 `within` at its defaults
from the campaign environment. Every process ran on the same four cores
(0-3, which span both sockets on this node) with memory bound to node 0. Each
process made one untimed warm-up fit and two timed fits, three rounds with arms
interleaved. Accuracy is the largest relative coefficient error against the
diagnostic run's strict fereg fits (1e-14; 1e-12 for sparse PPML). Fixed-worker
coefficients repeated exactly in every arm. Seconds are medians of six fits:
fereg native (`e(fereg_runtime)` row 20 or `feppml` total) and Stata command,
PyFixest's fit call after `read_stata`. Arm `diag` is the recurrence at the
request with the previous trial rule; `cand` adds the stall rule (OLS) or the
ordinary floor (PPML).

| Cell (1M rows unless noted) | beta.12 native / command | `diag` | `cand` | PyFixest | beta.12 error | PyFixest error |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| OLS three-sparse 100k | 0.418 / 0.474 | r(459) | 0.417 | 0.299 | 3.5e-14 | 2.4e-14 |
| OLS three-sparse | 4.401 / 4.764 | 4.092 | 4.234 | 3.583 | 9.3e-14 | 7.2e-14 |
| OLS three-dense | 0.958 / 1.349 | 0.955 | 0.939 | 1.112 | 8.9e-14 | 5.2e-14 |
| PPML three-sparse, steady fits | 25.59-25.74 / 25.99-26.41 | - | 25.18-25.44 | 19.42 | 5.2e-13 | 8.8e-15 |
| PPML three-dense | 4.596 / 5.150 | - | 4.492 | 3.935 | 1.2e-14 | 5.2e-11 |
| PPML one-FE | 3.068 / 3.695 | - | 3.113 | 3.259 | 2.0e-16 | 2.1e-14 |

Six of twelve sparse PPML fereg fits, the first fit of a process in both arms,
took 41-68 s; the table gives the other six, and every fit is in the private
run record. PyFixest showed no such fits. On native time fereg is already ahead
on dense OLS and one-FE PPML; the Stata command adds 0.06-0.6 s of input
transfer and ado work that PyFixest's timer does not count.

Many-FE recurrence at the request: not adopted. The one-decade margin was not
idle. The Fast derived residual (§7) inherits the recurrence's accuracy, and
the independent dense full-dummy witnesses in
`fereg-oracle/tests/derived_residual.rs` then miss the §20 residual envelope
(2.4e-9 against a bound of 1.2e-9, and 2.0e-9 against 1.4e-9). The finite
score certificate also loses its margin and has no correction path: `diag`
refused the 100k sparse cell with r(459) in all three rounds, where beta.12
publishes. On the 1M cell it was 7% faster.

`within`'s stall rule for the Jacobi trial: not adopted. It needs four
contractions above 0.7, so on the sparse design the trial escalates after six
to nine Jacobi iterations instead of four or five and finishes in 24 to 27
iterations instead of 22 to 23 (21 at the request). It was 3.5% slower than
the previous rule at the same target on SCC and 4-5% slower than beta.12 on the
Mac at the beta.12 target, and it would lengthen the documented near-confounded
exception.

Ordinary PPML frame floor: implemented on the separate branch
`pyfixest-parity-ordinary-floor`, not adopted. The literal floor, applied also
to the frame after the outer diagnostics pass, publishes a point 1.6e-6 (eta)
from the strict solution on the sparse design. Its checkpoint is rejected,
refined to 1e-14 and repeated, for ten IRLS iterations; it was slower (10.1-10.7
against 9.4-10.0 s on the Mac), and four unit fixtures that require `eta`
within 1e-10 of the exact optimum fail. The branch keeps the strict target for
that frame. Its accuracy matches beta.12, and it was 8% faster on the Mac's
sparse fit. On SCC the last floored frame leaves the next strict-target step
moving eta by 2.9e-6 instead of 6.7e-7, so the strict-entry rule defers once
and adds a 595-step frame: nine instead of eight IRLS iterations and 1.6% less
time. Two replay cases also raise their outer iteration count from five to six,
which the affected-scope contract cannot declare. Dense was 2% faster and
one-FE 1.5% slower, within noise.

Schwarz reuse across PPML frames: implemented as specified in §13 for explicit
`solver(lsmr)` with three or more effects. Public Auto rebuilds no Schwarz
factor per frame: its MAP-family joint Schur recurrence preconditions the
retained block with the current diagonal, and its two Schwarz builds are the
initial and strict checkpoint sessions. Between them the per-frame maxima of
the Hessian log-weight change sum to 3.4 on the sparse design, far beyond any
bound that would justify reuse. Explicit LSMR already retained its
factor whenever frames stayed short. On every benchmarked design they did (15-24
steps on sparse, 6-10 on dense), so the drift rule changes no benchmarked fit.
The 77-case replay was bit-identical to beta.12, and the unit witnesses cover
reuse, refresh, invalidation and per-frame certification.

Remaining gaps: sparse three-FE OLS (4.40 against 3.58 s native) and PPML
(25.6 against 19.4 s steady). The sparse PPML frames take 160 to 600 joint
Schur steps each, so the frame preconditioner, not the stopping target,
dominates their cost. A follow-up diagnostic (job 8381971, `scc-wo4`, Xeon
E5-2680 v4, the same protocol on node-0 cores 0, 2, 4 and 6) compared beta.12's
public Auto PPML with explicit `solver(lsmr)`, whose retained core-pair Schwarz
factor needs 15 to 24 steps per frame. On sparse three-FE, Auto took 25.9 s in
three fits and 33.6-34.1 s in three, explicit LSMR 24.0-25.1 s and PyFixest
20.8-21.8 s. On dense three-FE, Auto took 6.1 s, explicit LSMR 10.3 s and
PyFixest 5.6 s. Explicit LSMR is therefore no general replacement. Auto's
slow sparse fits, seen on two node classes, are explained below.
Coefficients repeated exactly in every arm and stayed within 3.3e-12 of the
strict fits.

### Joint-frame slow mode and the coupling stream (development, 2026-11-01)

Diagnosis (jobs 8384403 and 8384428 on `scc-zj4`, Xeon Gold 6132, beta.12,
sparse three-FE PPML, node-0 cores 0, 2, 4 and 6, one warm-up and two timed fits
per process, per-fit `perf` counters and per-second thread samples). The joint
frames' coupling operator ran either at 4.16 ms per step or at 5.3-9.9 ms per
step, constant through every frame of a fit. Ten of the 11 default slow fits
were a process's first timed fit. Slow fits executed the same instructions at
the same clock with 0.52-0.84 times the instructions per cycle,
10-30% more L2 misses served from L3 and unchanged L3 misses. Disabling
transparent huge pages for the process (`prctl`) still gave 2 slow fits of 8,
and large allocations from glibc arenas 1 of 8, against 11 of 24 by default; no
two running threads shared a core. The operator streamed offsets, targets and
values for each of its two coupling pairs, six arrays per worker, and whether
they conflicted in L2 depended on where a fit's banks were allocated.

Change: single-channel compact couplings now read one group-major stream (see
[ARCHITECTURE](ARCHITECTURE.md)). Each group keeps its pair-then-edge
operation order, so results are bitwise unchanged: a unit witness compares the
stream with the per-pair products bit for bit, the 77-case macOS replay is
identical to beta.12, and every timed fit below produced the same coefficient
file in both arms.

A/B (job 8384441, `scc-wo4`, Xeon E5-2680 v4, the node of the route diagnostic,
six processes per arm alternating, the same placement): beta.12 took 25.6-26.2 s
in nine fits and 32.9-39.3 s in three first fits; the stream took 18.4-18.9 s in
all twelve. The operator went from 4.09-4.31 ms per step (8.8 in a slow fit) to
1.61-1.63 ms, with identical step counts in every frame. On the Mac the fit went
from 9.3-10.0 to 7.2-7.3 s and the operator from 1.6-1.9 to 0.84-0.88 ms per
step. PyFixest took 20.8-21.8 s on `scc-wo4` the day before (job 8381971), so
fereg's sparse three-FE PPML is now about 12% ahead of it there; other node
classes were not remeasured. The two checkpoint solves (about 7 s) are unchanged.

### Seeded PPML joint frames (development, 2026-11-01)

Change (`e0e607c8`, SPEC §13): an ordinary PPML Auto joint frame whose sample,
topology and kept generated columns match the last certified joint frame solves
for the correction from that frame's fitted values and adds its coefficients
back; the full response's norm still scales every stopping and acceptance test.
Witnesses: a lockstep seeded/cold unit test with independent full-response
finite scores and bitwise cold results for stale, refused, failed and
invalidated seeds (1 and 3 workers); the 1e13-disparity disconnected-slope
fixture, whose seeded fit passes its first strict check at eta error 1.7e-14
(cold: two strict rejections, 1.8e-11), with the cold path's rejection
assertions kept behind a test-only fault plan; the 77-case macOS replay against
`a6dda3a4` with the declared scope, where 12 PPML cases change numerical leaves
only and no discrete field, at 1, 2 and 4 workers; and
`./scripts/run_tests.sh --source-only`.

Mac, sparse three-FE PPML (1M rows, 4 workers), three alternating processes per
arm with two timed fits each: `a6dda3a4` 7.18-7.33 s native, seeded 6.11-6.53 s.
The seven ordinary frames took 181, 246, 337, 426, 511, 547 and 596 steps
before and 181, 220, 269, 342, 329, 245 and 135 after (2,844 to 1,721), with 8
IRLS iterations in both.

SCC (job 8384682, `scc-zj4`, Xeon Gold 6132; both plugins built in the job from
their commits; node-0 cores 0, 2, 4 and 6 and node-0 memory; six rounds of
base, seeded and PyFixest processes in rotating order, each an untimed warm-up
and two timed fits; no retries or exclusions): base 16.38-16.62 s native
(Stata timer 16.77-16.99 s), seeded 14.22-14.42 s (timer 14.59-14.78 s) and
PyFixest 17.03-17.52 s per fit call, data loading excluded. Frame steps went
from 181/247/336/430/512/547/599 to 181/220/269/343/328/238/130, 8 IRLS
iterations in every fit. Base and seeded each repeated their coefficients
exactly in all six processes and agree within 4.9e-13 relative; the seeded
coefficients are within 9.2e-14 of PyFixest's (base 5.2e-13). The branch head
had no slow fit on this node, where beta.12 had taken 23.4-24.3 s in second
fits and up to 35 s in first fits (job 8384403). On this node fereg's Stata
command (median 14.66 s) is now 15% faster than PyFixest's fit call (median
17.28 s) on sparse three-FE PPML; other node classes and cells were not
remeasured.

### PPML cells remeasured after the parity branch (development, 2026-11-01)

Descriptive remeasurement of the beta.11 PPML cells where PyFixest default was
faster: published beta.12, the branch plugin built from `e0e607c8` and PyFixest
default in one allocation per job, on the beta.11 inputs (recorded hashes), with
campaign placement (socket-ordered physical cores, `numactl` CPU and memory
binding), three rounds of rotating arm order, an untimed warm-up and two timed
fits per process, no retries or exclusions. Medians of the Stata command timer
for fereg and the fit call for PyFixest, 1M rows:

| Cell | Workers | Node | beta.12 | branch | PyFixest | PyFixest / branch |
| --- | ---: | --- | ---: | ---: | ---: | ---: |
| Sparse three-FE | 4 | `scc-wo4` | 26.59 | 17.52 | 21.68 | 1.24 |
| Sparse three-FE | 16 | `scc-zj4` | 16.85 | 9.80 | 9.47 | 0.97 |
| Dense two-FE | 4 | `scc-wo4` | 6.25 | 6.18 | 5.57 | 0.90 |
| Dense two-FE | 16 | `scc-zj4` | 2.72 | 2.73 | 2.40 | 0.88 |
| Dense three-FE | 4 | `scc-wo4` | 6.82 | 6.82 | 5.82 | 0.85 |
| Gravity | 4 | `scc-wo4` | 20.88 | 20.42 | 20.57 | 1.01 |
| One-FE | 4 | `scc-wo4` | 5.38 | 5.30 | 4.28 | 0.81 |

Jobs 8384884 (`scc-wo4`, Xeon E5-2680 v4) and 8385014 (`scc-zj4`, Xeon Gold
6132). The first job's 16-worker fereg processes stopped at `set processors 16`
(Stata-MP is licensed for four) before fitting; 8385014 reran both 16-worker
cells, all three arms, with Stata's own processors capped as in the campaign and
`threads(16)` unchanged. Every arm repeated its coefficients exactly across
processes except PyFixest, which is not bitwise repeatable; the branch agrees
with beta.12 within 4.9e-13 and with PyFixest within 5.4e-11. beta.12's sparse
fits still showed the slow mode (up to 34.0 s at 4 and 18.4 s at 16 workers).

Native time is now close to PyFixest's in every cell: sparse three-FE at 16
workers 9.28-9.94 s against 9.38-9.96 s, dense two-FE 5.40-5.78 against
5.26-5.80 s (4 workers) and 2.29-2.40 against 2.33-2.46 s (16), dense three-FE
5.75-6.26 against 5.47-6.21 s; one-FE remains slower natively (4.57-4.84 against
3.80-4.70 s). The rest of each gap is the PPML Stata frontend outside the native
call: 0.54-0.77 s per fit on `scc-wo4` and about 0.36 s on `scc-zj4`, against
0.17 s on the Mac. A timer-instrumented scratch copy of the unchanged frontend
(job 8385168, `scc-wo4`, one-FE input, four node-0 cores) attributes 0.45-0.56 s
at 1M rows to per-row passes: `marksample` 0.07-0.12 s, generating the sample
marker 0.08-0.12 s, and three post-plugin checks of the returned marker (binary,
within the candidate sample, count) 0.10-0.11, 0.09-0.10 and 0.04-0.05 s;
`ereturn post` takes 0.015 s. The same passes take 0.12 s on the Mac. `feppml`
runs the whole public call with one Stata processor (ADR 0241, measured on 30k
to 123k rows), so Stata cannot spread these passes over cores; the OLS frontend
has neither the serial host nor the three marker checks.

### PPML frontend passes and host scope (development, 2026-11-01)

Marker checks (`73f85bd0`): one `summarize, meanonly` pass and one `count if`
pass replace the three post-plugin passes over the sample marker, with the same
failures, messages and order. Host scope: `feppml` keeps the one-processor scope
of ADR 0241 below one million observations and the caller's processor count
from there (SPEC §18). Arms were scratch copies of the frontend with the branch
plugin; every process used four node-0 cores, Stata at four processors and
`threads(4)`, arm order rotating by round, no retries or exclusions.

Job 8385337 (`scc-zj4`, Xeon Gold 6132, campaign one-FE and dense two-FE PPML,
three rounds of three timed fits per size), medians of the Stata command time:

| Arm | One-FE 1M | Two-FE 1M | One-FE 100k | Two-FE 100k |
| --- | ---: | ---: | ---: | ---: |
| Before the marker change | 3.078 | 3.592 | 0.326 | 0.372 |
| Marker change, serial host (V0) | 2.995 | 3.436 | 0.318 | 0.363 |
| No host switch (V1) | 2.938 | 3.317 | 0.356 | 0.424 |
| Serial through the native call (V2) | 2.887 | 3.361 | 0.340 | 0.420 |
| Serial only around the plugin call (V3) | 2.913 | 3.354 | 0.377 | 0.423 |

The marker change lowered the time outside the native call from 0.38 to 0.29 s
at 1M rows and from 0.040 to 0.033 s at 100k; the other arms lowered it to
0.15-0.21 s at 1M but added 0.03-0.09 s of native time, the interference ADR
0240 described, which dominates at 100k. Coefficients were identical in every
arm and round.

Job 8385376 (`scc-wo4`, Xeon E5-2680 v4, campaign AKM baseline PPML: leading
300k, 600k, 1M and 3M rows of the 3M input in four rounds of two fits, the 10M
input in two rounds of one fit), median Stata command time against V0:

| Rows | V0 | V1 | V2 |
| ---: | ---: | ---: | ---: |
| 300k | 5.27 s | -9.7% | -7.4% |
| 600k | 5.91 s | -8.5% | -8.7% |
| 1M | 7.22 s | -8.0% | -3.9% |
| 3M | 18.55 s | -4.9% | -0.6% |
| 10M | 75.04 s | -4.1% | -1.1% |

V1's frontend overhead was 0.21, 0.65 and 1.53 s at 1M, 3M and 10M against
0.47, 1.32 and 3.21 s for V0; this node showed no native interference. The
measured per-row savings (0.14-0.26 microseconds) against the largest fixed
interference (0.08 s, `scc-zj4`) break even near 600k rows, so the switch is
kept below one million observations. The public-scope fixture now also checks
a one-million-row call (caller's count inside, restoration after an injected
failure and a success); it, the host-processor and transaction fixtures, both
genuine PPML Break modes and the Python suite pass on the Mac.

Strict checkpoint seeding (investigated, not changed): the strict checkpoint
already re-projects the retained design, which holds the regressors projected
at the initial checkpoint's weights, so it starts warm. On sparse three-FE PPML
(job 8384884's traced fit) all 17 columns needed 23-24 LSMR iterations at
1e-12, the response 23, because the weights move far between the checkpoints.
A response seed from the joint frames cannot shorten the batch, and a regressor
seed at near-final weights would need another 17-column projection (the initial
one took 11 iterations to 1e-6), costing about what it saves.

One-FE PPML (job 8385347, `scc-wo4`, campaign one-FE input, four node-0 cores):
fereg's native 4.54 s splits into preparation 0.76 s, FE projection 0.59,
finite solve 1.04, line search 0.41, strict final solve 0.78, inference 0.08
and publication 0.24 (4 IRLS iterations plus the strict solve); PyFixest's fit
(3.92-4.52 s) also makes five weighted solves, with 1.26 s in `demean`. fereg's
projection is faster per solve (0.15 against 0.25 s). A macOS `sample` of
twelve one-FE fits showed the four workers idle 41-49% of the time behind
serial sections on Stata's main thread (input reading, which must stay there,
numeric classification, result construction, inference) and on the driving
worker; inside the parallel work, the per-iteration score certificate and the
compensated norm statistics cost about as much as the Gram and projection
kernels. The levers are the serial sections (numeric classification is a
sequential scan that a chunked parallel scan with first-row reductions could
replace, shared with OLS), the finite solve's kernels and the score
certificate's cost; none changes the iteration structure.

### One-FE PPML exact speedups (development, 2026-11-01)

Three arithmetic-preserving changes; coefficients and covariance are bitwise
unchanged, and each passed the 77-case macOS replay against the branch head
without a scope and `./scripts/run_tests.sh --source-only`:

- `64d4bb30`: numeric sample classification checks availability once per
  2,048-row block with branch-free column folds and applies only the domain
  policy on available blocks. The PPML preflight pass must stay allocation-free
  on Stata's main thread (the early-preflight allocator test counts events), so
  it was made cheaper rather than parallel. Mac: 0.052 to 0.0125 s per fit.
- `a89ab4c1`: the implicit one-FE design refresh stages each row's normalized
  weight once in the projected bank, schedules group means per (column, group)
  and fills the projected response in parallel. Mac: FE projection 0.106 to
  0.098 s, strict final solve 0.221 to 0.212 s.
- `86b6f936`: the one-FE profile binds numeric column slices and forms step
  coefficients once per 4,096-row block, keeping each row's fused sequence, and
  fills Poisson means on the team with first-row error order. Mac, per profile
  step: predictor 4.6 to 1.7 ms, means 5.7 to 1.4 ms. A column-major variant
  was slower and was dropped; fusing the score certificate's columns showed no
  gain and was reverted.

Mac native one-FE PPML (1M rows, 4 workers): 1.078 to about 0.99 s. SCC, four
node-0 cores, Stata at four processors and `threads(4)`, branch plugin against a
candidate built in the job, with PyFixest, rotating arms, no retries: job
8385792 (`scc-zj4`, all three changes, six rounds of two fits) one-FE PPML
2.959 to 2.763 s (-6.6%; native 2.828 to 2.618 s) against PyFixest's 2.921 s,
and one-FE OLS 0.649 to 0.661 s (+1.8%, ranges 0.632-0.672 and 0.641-0.692 s)
against PyFixest's 0.745 s; the Mac shows no OLS phase difference beyond noise.
Job 8385700 (`scc-wo4`, the first two changes, four rounds) measured one-FE
PPML -1.4% (wider spread), one-FE OLS -9.7% and dense two-FE PPML +0.7%. Every
arm repeated its coefficients exactly and the candidate matched the branch.

Two measured costs were left for decision: keeping freed memory in the heap
across fits (`MALLOC_MMAP_MAX_=0`, no trimming; job 8385639, `scc-zj4`) saved
0.10 s of a 2.78 s one-FE fit, mostly first-touch page faults in preparation;
and with Stata's processors kept from one million observations, Stata's OpenMP
workers spin after the frontend's parallel passes (11.6% of busy samples in a
Mac profile), the competition ADR 0240 described.

## Published beta.11 descriptive campaign

The October 30 SCC campaign of the published Linux plugin `af65682b...` (runtime
source `65500be6`) adds clustered-mobility and gravity designs, strict PyFixest,
32 instead of 64 controls, the Stata-comparison cells on an Ice Lake node, and
the Veneto long panel on the leave-one-out and largest connected sets. All 357
native fits outside eighteen superseded cells passed their repeat, cross-worker
and independent smoke checks. The two clustered-mobility cells were rerun once
after the generator stopped leaving a singleton firm-year. The PPML AKM bundle
first ran on a node whose reference fit was 1.8 times slower; its fifteen cells
and reference were rerun once on a normal-speed node. Superseded positions are
retained. Setups passed their harness suites, and accounting, sentinels, host
observations and frozen inventories verified. No other timed fit was repeated or
excluded and no tolerance changed; five roots stopped before any timed fit are
recorded in the
[completion record](../bench/results/scc-20261030-beta11-completion.json).
This is descriptive measurement of a published release,
not new runtime qualification. See the [synthetic](fixest_benchmarks.md) and
[Veneto](veneto_benchmarks.md) reports and the
[accuracy study](../bench/results/scc-20261030-beta11-accuracy.json).

## Published beta.10 descriptive campaign

The October 28 SCC rerun of the published Linux plugin `a7964116...` (runtime
source `3348fe45`, harness `7c5d9f39`) retains the beta.8 inputs, pins, tolerances
and all 1,646 positions: 1,448 synthetic and 99 per Veneto variant. All 286
native fits passed. Synthetic fixed-worker repeatability passed in 56 cells
and cross-worker science in 14 groups; the panels passed 20 exact repeats and
30 worker-reference comparisons. All setup suites passed 421 tests, every
accepted scheduler record and application sentinel passed, and the retained
host observations and frozen inventories verified. Original pre-fit input
discovery failures and the bounded canonical-path repair are recorded separately;
no timed fit was retried or excluded.

Sparse three-FE OLS at ten million rows completed in 53.18 seconds (three
repetitions, 52.94–54.82 seconds), using 12.50 GiB. Comparator failures and
memory tradeoffs remain explicit in the [synthetic report](fixest_benchmarks.md)
and [Veneto report](veneto_benchmarks.md). The slower reference observation on
`scc-xc1` is retained. This campaign describes an already-qualified release;
it is not new runtime qualification and makes no absolute cross-host
beta.8/beta.10 speedup claim. See the
[completion and repair record](../bench/results/scc-20261028-beta10-completion.json)
and its linked evidence hashes.

## Accepted results

| Change | Evidence and scope |
| --- | --- |
| Many-FE Auto | [Final compact result](../bench/results/incremental-many-20261018-compact.json): Linux qualification and practical Mac recovery passed all 126 protected comparisons per platform; public activation and independent science passed. |
| Two-FE structural gates | [Linux](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json) and [Mac](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json): all 84 protected bounds per platform passed. [Activation](../bench/results/two-fe-policy-gates-activation-20261023.json): exact public/hidden science, worker, gate, hierarchy and resource checks passed after the documented transport-accounting amendment. |
| Ownership and deferred cells | [Linux](../bench/results/ownership-qualification-final-linux-20261024.json): 10,440 calls, 58 settings and 12 distinct physical hosts, all 116 protected bounds passed. [Practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json): 870 calls and all 116 paired-median comparisons within max(5%, 1 ms). The Mac check is descriptive, not a confidence-bound claim. |
| Many-FE geometry Auto | [Decision record](../bench/results/many-geometry-20261025.json): Linux qualification passed 80 of 84 protected comparisons and public activation 122 of 126, on 12 distinct physical hosts each. Every miss is the near-confounded third FE, accepted by Johannes as a documented exception; public/hidden agreement was exact. |
| PPML two-FE CMG reuse and lean protocol | [Lean record](../bench/results/ppml-two-fe-lean-20261026.json): every two-FE cell faster than beta.6 at 4 and 28 workers on Linux (0.32-0.84), science within the §20 envelopes, 73/73 replay cases. Merged and released as v0.5.0-beta.7; the [beta.7 SCC comparison](fixest_benchmarks.md#october-26-2026-beta7-rerun-with-pyfixest-and-areg) found all 168 native fits passing and every OLS fit bit-identical to beta.6. |
| Two-FE Auto speed | [Qualification record](../bench/results/two-fe-auto-qualification-20261027.json): Linux 7,560 calls on 31 physical hosts, all 126 protected comparisons within max(5%, 1 ms) after Johannes waived the 12-distinct-host rule (84 cells had 11); practical Mac check 119 of 126 passed, the other 7 inconclusive with paired medians within the bound. [Veneto comparison](../bench/results/two-fe-auto-veneto-20261027.json): 1.24-2.08 times faster than beta.7 without saved FE and 1.06-2.18 with saved FE; faster than PyFixest's `within` in 9 of 10 cells. |
| Two-FE Auto round two | [Round-two record](../bench/results/two-fe-round-two-20261027.json): Linux 10,080 calls on 28 physical hosts, 123 of 126 protected comparisons within max(5%, 1 ms) after the 12-distinct-host waiver (every cell had 11) and 3 inconclusive under 1 ms; Mac check 115 of 126 passed, 11 inconclusive; Veneto candidate faster than beta.8 in all 20 cells and than PyFixest `within` in 9 of 10; PPML exact timing 12 of 12 within bound with identical science. |
| Public integration | [Final activation](../bench/results/ownership-public-activation-20261024.json): 174 actual hidden/public/public-repeat calls per platform, independent witnesses, fixed-worker exactness, cross-worker science, resource/gate/hierarchy checks, Mac/Linux ABI/load/panic checks and the full source suite passed. Six licensed Mac Stata fixtures and actual public OLS Break/rollback passed. |
| Beta.10 integration | [Round record](../bench/results/beta10-2026-10-28.json): Linux 13,920 calls on 35 physical hosts, 173 of 174 protected comparisons within max(5%, 1 ms) (one inconclusive by 0.02 ms; 20 cells one host short of twelve under Johannes's renewed waiver); Mac 162 passed, 11 inconclusive and the accepted sixteen-FE one-worker exception of the within fused LSMR step; Veneto candidate within 4% of beta.9 and faster than PyFixest `within` in 9 of 10 cells; PPML 12 of 12 within bound and bit-identical; sparse three-FE series finishes at ten million rows where beta.9 does not. Publication is a separate authorization. |

Ownership/deferred cells activate only for OLS Auto/Auto with a global intercept
and exactly two categorical-intercept FE terms. Explicit injections and paths
outside the scope retain their contracts. The Break test interrupts at the
native pre-preparation checkpoint; it does not establish interruption inside a
cell constructor. Scientific behavior is specified in [SPEC](SPEC.md).

## Two-FE Auto speed

Public OLS Auto/Auto with exactly two categorical-intercept FEs applies its CMG
hierarchy on the worker team, runs the deferred cell operator in parallel,
builds the hierarchy without CMG's low-effective-degree correction and hands
off early under gate v2; Fast fits without saved outputs certify their within
residual by its derived bound. Specified in [SPEC](SPEC.md) and
[the gate notes](two_fe_auto_gates.md); the investigation behind it is in
[the PyFixest comparison](pyfixest_twoway_investigation.md).

The fixed-candidate qualification used beta.7 public Auto as original and
incumbent and the hidden `two-gates-v2` arm, with the 42 routing cells. Linux
passed every protected comparison; the weak and high-degree-weak cells ran
4-11% faster and the others within noise. Science, gate decisions, resources
and cross-worker agreement passed. The routing cases all request saved outputs,
so they do not exercise the derived residual; its witnesses are the dense
explicit-OLS oracle, the 68 licensed Mac Stata fixtures and the Veneto runs.

On one whole SCC node with the audited Veneto panels (seconds, medians of three):

| Panel, workers | Candidate | beta.7 | PyFixest `within` |
| --- | ---: | ---: | ---: |
| short, 1 | 2.01 | 2.49 | 2.08 |
| short, 4 | 1.03 | 1.85 | 1.08 |
| short, 16 | 0.78 | 1.61 | 0.96 |
| long, 1 | 10.53 | 13.32 | 26.94 |
| long, 4 | 5.28 | 8.54 | 9.84 |
| long, 16 | 3.62 | 7.08 | 6.23 |

Every fixed-worker repeat was exact and every cross-worker, beta.7 and
PyFixest comparison passed the campaign tolerances. The 73-case replay corpus is
exact against beta.7's runtime, and PPML's campaign two-sparse design ran 2%
faster from the hierarchy options with the other PPML designs unchanged.

## Two-FE Auto round two

Round two keeps beta.8's policy and changes three mechanisms, specified in
[SPEC](SPEC.md) and [the gate notes](two_fe_auto_gates.md): the (worker, firm)
cells are built on the worker team, bit for bit as the serial build, and before
the diagonal pilot, so the pilot's Schur products use the cell operator, with
block cancellation polls in the serial kernel; the pinned CMG (`f6a044c9`)
dispatches a level's row-operator matvec to the team when its entries times
lanes reach the parallel threshold, so dense coarse levels with few rows no
longer run serially; and PPML's joint recurrence merges helper products, forms
the finite cross block and the level masses in exact parallel form and reuses a
finished canonical probe. The CMG and PPML changes are arithmetic-preserving;
the cells change moves low bits of the pilot within the certified tolerances
and is bound by the replay scope
[`2026-10-27-two-fe-eager-cells-macos.json`](../tests/replay/scopes/2026-10-27-two-fe-eager-cells-macos.json)
(three two-FE cases; solver codes, iterations, stop reasons and every discrete
decision unchanged). The [round-two record](../bench/results/two-fe-round-two-20261027.json)
holds every number below.

The fixed-candidate Linux qualification (beta.7 public Auto as original, beta.8 as
incumbent, the hidden `two-gates-v2` arm, 42 routing cells, 3 shards by 12
replicas, 10,080 native calls on 28 physical hosts) reported `needs_discussion`
because every cell reached 11 of the required 12 distinct physical hosts;
Johannes waived that rule for this round as for the previous one. Under the
waiver 123 of 126 comparisons satisfy max(5%, 1 ms) and three are inconclusive
by under a millisecond (sixteen-fe-t4 and small-retained-t4 against the
incumbent, small-retained-t4 against the original, medians below zero). No
scientific, gate, resource or cross-worker check failed and no native call was
repeated. Against beta.8 the repeated-cells, disconnected and wide-rhs cells
ran 19-21% faster at one worker and the high-degree and weak cells 10-15%;
weighted-t1 ran 4% faster, so the eager build's cost on that no-aggregation
design needs no exception on Linux.

The practical Mac check (42 cells, blocks of 6, 1,638 native calls, beta.8 as
original and incumbent) passed 115 of 126 comparisons with 11 inconclusive and
no regression or scientific error; weighted-t1, which the first eager-cells
candidate had regressed by 9% because the serial kernel polled once per cell,
closed to a paired ratio of 1.021 after the block-poll change. The licensed
Mac Stata development profile passed its 68 fixtures on a fresh build. On the
synthetic Veneto-shaped AKM panels (profiling builds, no competing process),
native time relative to beta.8 at 1/2/4/8 workers was 0.944/0.931/0.937/0.886 on the short panel
and 0.949/0.887/0.884/0.914 on the long panel. Attribution on the same
panels: eager cells cut the pilot's operator phase (long panel at two workers
0.72 s to 0.41 s) and the entries-gated matvec cut the short panel's recurrence
by 14/32/40% at 2/4/8 workers (its 3,043-row level is newly parallel; the long
panel's planned levels all exceed 4,096 rows and its dispatch is unchanged).

On one whole SCC node with the audited Veneto panels, three rotated
repetitions, every fixed-worker repeat exact and every cross-worker, beta.8 and
PyFixest comparison eligible (seconds, medians):

| Panel, workers | Candidate | beta.8 | PyFixest `within` | vs beta.8 | vs `within` |
| --- | ---: | ---: | ---: | ---: | ---: |
| short, 1 | 1.81 | 2.01 | 2.07 | 0.90 | 0.88 |
| short, 2 | 1.39 | 1.42 | 1.34 | 0.97 | 1.03 |
| short, 4 | 1.03 | 1.06 | 1.08 | 0.97 | 0.95 |
| short, 8 | 0.78 | 0.82 | 0.94 | 0.95 | 0.82 |
| short, 16 | 0.73 | 0.78 | 0.96 | 0.94 | 0.77 |
| long, 1 | 9.64 | 10.53 | 26.95 | 0.92 | 0.36 |
| long, 2 | 6.96 | 7.38 | 15.42 | 0.94 | 0.45 |
| long, 4 | 4.83 | 5.29 | 9.84 | 0.91 | 0.49 |
| long, 8 | 3.54 | 3.99 | 7.04 | 0.89 | 0.50 |
| long, 16 | 3.22 | 3.63 | 6.19 | 0.89 | 0.52 |

With saved fixed effects (PyFixest's saved effects exceed the command limit):

| Panel, workers | Candidate | beta.8 | vs beta.8 |
| --- | ---: | ---: | ---: |
| short, 1 | 2.83 | 3.27 | 0.86 |
| short, 2 | 2.11 | 2.14 | 0.98 |
| short, 4 | 1.49 | 1.51 | 0.99 |
| short, 8 | 1.11 | 1.15 | 0.97 |
| short, 16 | 1.03 | 1.13 | 0.91 |
| long, 1 | 13.19 | 14.61 | 0.90 |
| long, 2 | 9.68 | 10.25 | 0.94 |
| long, 4 | 6.61 | 7.15 | 0.93 |
| long, 8 | 4.78 | 5.35 | 0.89 |
| long, 16 | 4.31 | 4.97 | 0.87 |

PPML exact timing (campaign-DGP cases through `bench/ppml_cases.py` and
`bench/time_exact.py`, one whole node, five alternating repetitions, science
identical between beta.8 and the candidate in every run; candidate over
beta.8 native-time ratio, candidate median in parentheses):

| Design, rows | 4 workers | 28 workers |
| --- | ---: | ---: |
| two-sparse, 1,000,000 | 0.991 (5.66 s) | 0.984 (4.01 s) |
| two-sparse, 4,000,000 | 0.990 (25.42 s) | 0.967 (17.23 s) |
| akm-baseline, 1,000,000 | 0.983 (4.08 s) | 0.990 (2.74 s) |
| akm-baseline, 4,000,000 | 0.985 (18.32 s) | 0.995 (12.15 s) |
| two-dense, 1,000,000 | 0.996 (3.83 s) | 1.005 (2.48 s) |
| two-dense, 4,000,000 | 0.996 (14.76 s) | 0.999 (10.07 s) |

All twelve PPML cells pass the nonregression bound. The team application of
CMG to PPML's joint frames above 350,000 finest edges was built, measured at
0.98 on the long Poisson panel at 8 workers and removed; separation detection
is 42-61% of PPML native time on Veneto-scale Poisson panels and is the
subject of a separate workstream.

## Beta.10 integration qualification

The integration candidate `codex/beta10-integration` (runtime commit
3348fe45, qualification tooling 77a45d18) joins the beta.10 workstreams below
on the beta.9 baseline: exact many-FE Schwarz admission (the ten-million-row
sparse three-FE fix), the shared Jacobi-trial design, RHS-transport and
projected-matrix release, the fused three-FE MAP pilot sweep, the ordered
additive-Schwarz reduction in every mode, the within fused LSMR pin, the
derived residual on the one-FE and many-FE routes, the CMG admission level
cap, the PPML separation re-execution, re-certification and Auto-route
activation, and the wide private Gram bank. The
[round record](../bench/results/beta10-2026-10-28.json) keeps the
identities, declarations, comparisons and failure history.

- **Replay.** 77 of 77 cases pass against the published beta.9 macOS plugin
  with the within fused-LSMR, derived-residual and PPML separation scopes;
  only `ols-derived-fe1` and `ols-derived-fe3` differ outside a scope, in
  their residual row's stop code (2 to 5) and certificate norms, and the
  integration head is their exact replay parent from beta.10 on.
- **Declaration.** The frozen configurations declared scope `two` with a
  `two-gates-v2` hidden arm. The first Mac attempt and the first Linux array
  stopped at their first three-FE cell with the exactness rule, because the
  within fused LSMR pin changes many-FE science in the last bits and the
  derived one-FE residual changes the residual row's stop code on the
  no-output one-FE campaign cell. Both configurations were amended before any
  timing was read (scope `many`; candidate families `two` and `many` with
  `public_policy: two-gates-v2`; hidden `many-geometry-v1`), the tool learned
  to bind a hidden arm only to the family it activates and to validate a
  projection-to-derived residual row ([README](../bench/README.md)), and the
  discarded attempts are listed in the record. With the actual plugins, the
  candidate's published values and observation columns on the no-output
  one-FE cell are bit-identical to beta.8 and beta.9.
- **Linux qualification** ([configuration](../bench/qualify_configs/beta10-linux.json),
  array 8313820, report 8313821): 58 cells, beta.7 original, beta.9 incumbent,
  candidate and hidden arms, 13,920 native calls on 35 distinct physical
  hosts. 173 of 174 protected comparisons are within max(5%, 1 ms); the one
  inconclusive comparison is `small-retained-t4` against beta.7, whose upper
  bound exceeds the allowance by 0.02 ms (median 0.26 ms faster). Twenty
  cells reached 11 of the required 12 distinct hosts (60 comparisons, all
  within the bound), the same one-host shortfall Johannes waived for round
  two; the tool therefore reports `needs_discussion`, and Johannes applied the
  waiver again on 2026-10-28, so the Linux result stands as passed with one
  inconclusive comparison. Against beta.9 the
  median cell ratio is 0.992 and no cell is slower than 1.017; the many-FE
  cells improve most: eight FE 0.77 and 0.84, four FE 0.80 and 0.87, small
  third 0.81 and 0.88, campaign three-sparse 0.82 and 0.84,
  campaign AKM k64 0.86 and 0.88 at one and four workers.
- **Mac practical check** ([configuration](../bench/qualify_configs/beta10-darwin.json),
  beta.8 original, 3,016 calls): 162 of 174 comparisons pass, 11 are
  inconclusive on small or noisy cells (`slopes-t4`, `campaign-two-dense-t4`,
  `disconnected-t4`, `weak-t1/t4`, `sixteen-fe-t1` against beta.9, where the
  medians are within noise or negative) and one is a regression:
  `sixteen-fe-t1` is 6% slower than beta.8 (0.413 s against 0.389 s in every
  block; 0.390 s for beta.9). The same cell is 6% faster than beta.9 on Linux.
  A development attribution with arm64 release builds of the integration
  head selecting within's `StepKernel::Separate` instead of the default fused
  step reproduces beta.9's time (0.389 s) and output bits on that cell, while
  the fused kernel is also the source of the one-worker gains on three, four
  and eight effects (small third 0.110 against 0.120 s, four FE 0.110 against
  0.121 s, eight FE 0.181 against 0.211 s). The fused step therefore costs
  about 6% at sixteen effects and one worker on the M2 Ultra and nothing at
  four workers; no platform-specific kernel fence was added. Johannes
  accepted this one-cell Mac exception on 2026-10-28; a within follow-up for
  many small unsorted intercept terms on arm64 remains open.
- **Veneto panels** (candidate against the beta.9 plugin on one whole 28-core
  node, three rotated repetitions): with saved effects the candidate is within
  2% of beta.9 at every core count (short 0.98-1.00, long 0.99-1.00) with
  exact fixed-worker repeats and cross-worker agreement. Without saved
  effects the first job's fereg arms agree within 1% while its PyFixest arm
  failed the pinned-source check (main now pins the beta.8 campaign's
  PyFixest commit, the reused beta.7 environment carried the older one); the
  rerun against the beta.8 Veneto environment (job 8315525, scc-xf4) has the
  candidate within 4% of beta.9 everywhere (short 0.96-1.01, long 0.98-1.00)
  and faster than PyFixest `within` in 9 of 10 cells (short 0.77-0.92 at 1, 4,
  8 and 16 cores, 1.00 at 2 cores; long 0.36-0.51), with exact fixed-worker
  repeats and cross-worker agreement. Both first jobs ran on scc-uf3, where
  every phase of every arm was about 2.5 times slower than on the other
  E5-2680 v4 nodes with no competing process observed; their within-job
  ratios stand, their absolute levels do not compare with round two.
- **PPML timing** (campaign-DGP PPML cases from `bench/ppml_cases.py`: two-sparse,
  AKM baseline and two-dense at 1M and 4M rows, 16 regressors, robust VCE;
  beta.9 against the candidate, five rotated repetitions, one whole node, job
  8313485 on scc-uf3): all 12 cells pass the max(5%, 1 ms) bound with paired
  median ratios 0.985-1.005 at 4 and 28 workers, and every published result
  is bit-identical to beta.9 (the beta.10 PPML changes act only on separation
  detection, which these designs do not reach). The node ran about 2.4 times
  slower than the round-two node for both arms, so only the ratios compare.
- **Sparse three-FE size series** (the campaign design: 16 controls, iid VCE,
  `threads(4)`, Auto/Auto; one whole 28-core node, scc-wo4, jobs 8316297 and
  8318321; three repetitions per cell in a fresh Stata process, each fit
  bounded at 30 minutes; 100k-7M rows regenerated with the campaign generator
  and the campaign's ten-million-row input reused read-only). The candidate
  takes 0.73, 1.87, 5.02 and 15.7 s at 100k, 300k, 1M and 3M rows against
  beta.9's 0.84, 1.98, 5.49 and 16.9 s (7-13% faster, 10-20% less resident
  memory, same LSMR route and iteration counts, coefficients equal to twelve
  digits with last-bit differences under the within fused-LSMR scope), and
  28.0, 39.4 and 58.9 s with 6.8, 9.2 and 12.6 GiB at 5M, 7M and 10M rows
  (23-24 LSMR iterations). beta.9 exhausted the 30-minute bound at 5M and 7M
  rows on this node, and Johannes skipped its ten-million-row attempt, which
  the beta.8 campaign had already shown not to finish. The campaign's recorded
  timings stand; this series is the candidate's.

## Beta.10 exact engineering

Two arithmetic-preserving changes from the beta.8 campaign analysis were
measured first and kept; the [record](../bench/results/beta10-exact-2026-10-28.json)
holds every number below. Trace-only `FEREG_PHASE_TIMING` laps split the MAP
wrapper, the finalize score certificate, the dense projection's RHS/moment
construction, the PPML one-FE profile and the loader pipeline. Each item was
kept only if its phase reached 5% of native time on a 1M-row campaign design
at 1 or 4 workers, or (for the 64-column cluster) if the control series jumped.

- The borrowed MAP wrapper and the bounded three-FE MAP pilot apply the exact
  one-group initial rejection screen before the complete initial certificate,
  and entry finite scans run as a parallel predicate (MAP wrapper overhead was
  16-18% of one-FE native time).
- Wide raw-moment caches use the widest 64/32/16/8/1-column band that keeps
  every worker busy, each column in its serial row order, and designs wider
  than the 64-column stack binding fill the `[y, X]` batch through a heap
  descriptor charged by the fast memory estimate. At 63 to 64 controls the
  native time had jumped by 0.36 s (one worker) for one control; the jump is gone.

Native seconds on an M-series Mac (medians, step 0 against final): one FE
0.417/0.178 to 0.382/0.161 at 1/4 workers, dense three-FE 1.132/0.417 to
1.047/0.410, AKM with 64 controls 2.633/1.072 to 1.947/0.977, with 96 controls
5.390/1.925 to 3.477/1.561; PPML one FE unchanged within noise. The score
certificate (at most 2.6%), converged two-FE lanes (under 0.4%), CMG
application (at most 2.1%), PPML one-FE serial passes (at most 3.7%) and
loader packing (fully overlapped with the SPI read) fell below the rule and
were not changed. A follow-up removed the last cliff: Gram dimensions wider
than the 64-column stack block accumulate in a private heap bank with the same
partition of Gram rows and the same per-row rank-1 pass, charged by the fast
memory estimate. In paired sessions against the previous head, the finite
solve at 65 controls fell from 0.780 to 0.519 s (one worker) and 0.327 to
0.247 s (four), at 96 controls from 1.300 to 0.988 s and 0.543 to 0.459 s;
native time at 65 controls fell from 2.234 to 1.975 s and 1.108 to 1.011 s,
with identical coefficients and residual sums.

Plugin replay is 74/74 exact against beta.9 after each change, including the new compact
`ols-wide-k67` case (67 regressors), and the universal release build is exact
at 2/4/8 replay threads; Stata `e(b)`, `e(V)` and posted scalars are
bit-identical to beta.9 on all eight 1M-row designs at 1 and 4 workers. The
42-cell Mac protocol (beta.9 as original and incumbent, the hidden
`two-gates-v2` arm, 1,638 native calls) found no scientific, agreement or
resource error; 102 of 126 comparisons passed and 24 were inconclusive with
negative median excess and upper bounds of at most 1.9 ms, so the status is
`needs_discussion`, as in round two.
## Beta.10 round two: dense many-FE route, residency and repeatability

Descriptive Mac evidence on the 1M-row campaign designs; every number is in
the [record](../bench/results/beta10-round2-2026-10-28.json).

- **Fused pilot sweep (kept, exact).** On dense three-FE the bounded MAP pilot
  was 70% of native time at one worker (two symmetric sweeps 0.57 s of
  1.02 s), and group statistics outweighed the subtraction pass about 2.5 to 1
  at four workers. The pilot now raises the next effect's column scales while
  it subtracts the current effect's means. Native time fell from 1.036 to
  0.974 s at one worker and 0.404 to 0.376 s at four, with identical
  `e(b)`/`e(V)`; sparse three-FE (no pilot) is unchanged. Unit tests compare
  fused and unfused sweeps and whole pilot attempts bit for bit at 1/2/4/8
  workers, and replay was 74/74 exact against beta.9.
- **Jacobi trial instead of the pilot (refuted).** Without the pilot, dense
  three-FE certifies after four Jacobi LSMR iterations per RHS but takes
  1.325/0.508/0.325 s against 0.959/0.377/0.260 s at 1/4/8 workers and peaks
  about 450 MB higher. Sparse three-FE has no predicted sweep count, so it
  already starts the trial. No pass-cost rule improves on the geometry policy
  on these cells; none was added.
- **Fast residency (design only).** The Fast increment over the loaded Stata
  process is about 425-459 B/row on AKM with 16 controls, 529-538 on dense
  three-FE and 1,750-2,250 on sparse three-FE. After projection the
  column-major prepared sources are read by the independent raw residual
  `y - Xb`, which gives the reported intercept, and by requested `xb`/residual
  outputs. Removing them needs a different intercept arithmetic (losing the
  witness) or a second SPI read after the solve (a memory and transaction
  design change), so nothing changed. An exact option remains for dense
  three-FE: the pilot keeps the untouched row-major original beside its
  candidate only for the LSMR fallback and could rebuild it from the sources
  instead (about 144 B/row).
- **Implicit level index (skipped).** 149 member-slice call sites and 28
  kernel parameters, including installed function-pointer types, take
  `&[usize]`; a u32 or implicit identity index cannot be introduced
  bit-identically in one bounded pass.
- **Ordered additive reduction (kept, declared scope in SPEC §18).** Every
  additive-Schwarz build now uses the Ordered reduction in default mode too.
  An escalating default-mode fit repeats bit for bit at 1/2/4/8 workers. On
  sparse three-FE the public trial does not escalate at 1M rows, and
  explicit `solver(lsmr) preconditioner(schwarz)` gave identical bits under
  both reductions with native times within -2.3% to +0.7% at 1/4/8 workers.
  Replay against the Auto-reduction build of the same tree is 74/74 exact;
  against beta.9 the only differences are the within fused-LSMR scope (14
  cases, passing under its scope file) and the separation activation's
  `ppml-separation-ir` certificate digest.

## Derived residual on the one-FE and many-FE routes

Fast OLS Auto without residual, xbd, d or saved-FE outputs now certifies its
within residual by the derived bound (SPEC §7, §16) on the direct one-FE route
and the many-FE geometry policy as well as the two-FE owner; see the
[compact record](../bench/results/derived-residual-routes-2026-10-28.json).
It is not yet qualified with `bench/qualify.py`; that is the integration round's
task.

- **Independent witnesses.** Dense explicit-dummy OLS (one FE; three FE through
  the balanced MAP pilot and dense/sparse LSMR; unit and analytic weights)
  matches the derived residual at the prediction tolerance, and explicit level
  moments meet 1e-10 at the derived scale. On 60,000-row designs fixed-worker
  repeats are bit-identical and 1/2/4/8 workers agree with each other and
  with the polished projection. Unit tests cover the forced certificate failure
  (bit-identical coefficients), saved FE, requested outputs, low memory and
  explicit solvers.
- **Replay.** Against the published beta.9 plugin, 73/73 cases pass with zero
  exact differences. Every OLS replay case requests residual, xbd and absorbed
  outputs, so the corpus does not exercise the derived path on any route; the
  [scope](../tests/replay/scopes/2026-10-28-derived-residual-routes-macos.json)
  therefore lists no cases. Three replay cases now request only the sample
  and drop reasons in Fast memory (`ols-derived-fe1`, `-fe2`, `-fe3`: one FE
  iid, two FE analytic-weight robust, three FE robust); their science check
  requires the residual row's stop code 5 with the absorption's solver, zero
  iterations and zero pilot work, beside the dense explicit-dummy
  coefficients and covariance. On the integration head all three take the
  derived path and pass with 4- and 8-worker repeats and cross-worker
  agreement (replay against the same build, 77/77). Against beta.9,
  `ols-derived-fe2` is exact (beta.9 already derived the two-FE owner's
  residual), while beta.9 projects the one- and three-FE residuals (stop
  code 2), so those two cases differ in the residual row's stop code and
  certificate norms (and, for three FE, the within fused LSMR leaves). A
  stop-code change is a route signature that no affected scope may declare,
  so these cases take the current build as their parent from beta.10 on.
- **Mac Stata end-to-end.** Coefficients are bit-identical to beta.9 in every
  cell below. One FE and dense three FE are bit-identical apart from the
  residual solver row, because beta.9 already accepted their residual at the
  initial certificate. Sparse three FE changes r2, rmse, rss, F and the iid
  covariance in the last bits through the residual sum of squares; this
  corrects the earlier claim that iid covariance is unaffected. Used solver,
  preconditioners, fallback and pilot fields of the residual row are unchanged.

Arm64 profiling builds, Stata 19 MP, 1,000,000 rows and 16 controls, medians of
three interleaved runs under the frozen quiet-host rule (three contaminated
beta.9 runs were discarded and repeated once):

| Design, workers | beta.9 native s | Candidate native s | Residual phase ms, beta.9 -> candidate |
| --- | ---: | ---: | ---: |
| one-fe, 1 | 0.422 | 0.414 | 19.4 -> 19.8 |
| one-fe, 4 | 0.182 | 0.185 | 13.6 -> 13.2 |
| three-dense, 1 | 1.161 | 1.149 | 23.1 -> 22.3 |
| three-dense, 4 | 0.437 | 0.441 | 12.3 -> 12.3 |
| three-sparse, 1 | 6.526 | 6.418 | 88.2 -> 26.6 |
| three-sparse, 4 | 1.928 | 1.945 | 44.7 -> 15.9 |

Only the sparse LSMR route saves its second solve, about 1-1.5% of native time
and within the run-to-run spread of the totals; on one FE and the MAP pilot the
derived certificate costs the same as the certificate it replaces.

## Many-FE geometry policy

Public OLS Auto/Auto for at least three categorical-intercept FEs uses the
geometry policy specified in [SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons).
Johannes accepted it on 2026-10-25 with one documented exception, below. The
`many-geometry-v1` arm and `routing_many_geometry_v1` token select the same
policy for comparisons; hidden `routing_incremental_many` keeps the previous one.

Linux evidence used the candidate plugin built at `99f0134d` with the release
toolchain and the beta.5 plugin (`1ac7125b`) as original and incumbent. Each of
12 replicas ran on its own whole generic 28-core node, all arms in the same
allocation; accounting recorded `failed=0` and exit 0 for every replica. Ratios
are candidate/beta.5 median native-call times; bounds are one-sided .025
bounds on the excess over max(5%, 1 ms) across physical hosts.

| Cell, 1 / 4 workers | Qualification | Public activation |
| --- | ---: | ---: |
| Four FEs | 0.68 / 0.64 | 0.68 / 0.64 |
| Eight FEs | 0.60 / 0.61 | 0.60 / 0.62 |
| Sixteen FEs | 0.62 / 0.67 | 0.61 / 0.67 |
| Small third FE | 0.65 / 0.63 | 0.65 / 0.61 |
| Near-confounded third FE | 1.125 / 1.066 | 1.122 / 1.063 |
| All other cells | 1.00-1.05 | 0.98-1.02 |

Qualification (5,040 native calls) passed 80 of 84 protected comparisons.
Public activation (7,560 calls) compared public Auto with the selected hidden
arm and passed 122 of 126, with exact scientific agreement and solver
signatures between public and hidden calls, fixed-worker repeatability and
cross-worker agreement. In both runs the misses were the near-confounded third
FE against both references: a one-worker regression (excess +16.1 and +15.4 ms,
lower bounds +10.7 and +11.2 ms) and an inconclusive four-worker result
(+1.9 and +1.8 ms, bounds spanning zero). This design needs the additive
preconditioner from the start; the Jacobi trial spends a few iterations before
escalating and finishes in 19 rather than 17 LSMR iterations. Johannes accepted
this cell as a documented exception rather than requiring a successor. A
`within` change sharing one prepared design between the trial and additive
solvers would remove roughly half of that overhead.

Two harness and wiring amendments, approved by Johannes before measurement,
made the contracts match the policy: the scientific harness checks the geometry
arm at its requested certificate and admits its MAP pilot, rather than the
unseeded portfolio's private hundredth; and hidden `routing_many_geometry_v1`
keeps public two-FE ownership/deferred-cell activation so public/hidden
agreement is exact on every design. Operational failures are recorded in the
decision record: an earlier run blocked on the old certificate contract before
any timing, one bench unit test lost a timer race and passed on its single
retry, and two activation replicas failed to open the shared placement lock
before any native call and were resubmitted.

Before qualification, `./scripts/run_tests.sh --source-only` passed (268 Python tests, Rust unit and
independent numerical integration tests, Clippy, FFI allocation audits,
portable checks, mock SPI and the complexity ratchet). New witnesses compare the
public policy with dense explicit-dummy QR on a mixing panel that finishes on
the Jacobi trial and a weak AKM panel that escalates, with exact fixed-worker
repeats and exact public/hidden agreement. The plugin replay against main's
plugin was exact for 72 of 73 cases. In `ols-30`, main rejected a three-FE
LSMR solve at the private hundredth of the request and fell back to 28 MAP
sweeps with a warning; the candidate accepts a 12-iteration solve at the
request. Its 502 numerical leaves stay within the
[declared scope](../tests/replay/scopes/2026-10-25-many-fe-geometry-linux.json)
and science passes. The used-solver, fallback and warning fields are the
intended route change. Four-worker repeats and cross-worker science passed for
every case. The suite and replay were rerun after the recurrence margin below
and after merging main.

On Apple Silicon, the source-only suite passed at `7b641ceb` (319 Python tests).
The candidate's universal Mac plugin passed its binary audit and actual
ABI/load/panic checks. Its replay against main's Mac plugin was exact for 72 of
73 cases. `ols-30` takes the same route change as on Linux, with 510 numerical
leaves inside a separate
[macOS scope](../tests/replay/scopes/2026-10-25-many-fe-geometry-macos.json),
because the macOS replay input has its own hash. Science, four-worker repeats
and cross-worker checks passed for every case. Fourteen focused Stata 19/MP
fixtures with three or more absorbed effects passed, including dense-dummy,
reghdfe parity, saved-output, memory-mode and LSMR-reuse checks, and so did
public OLS Break/rollback. Two development fixtures had asserted main's route
and were updated without changing any numerical threshold:
`ols_three_fe_map_oracle.do` now expects the geometry pilot to certify the
final-residual projection with MAP, and `ols_mixed_solver_reports.do` expects
the accepted LSMR solve where main fell back to MAP. Main's plugin still passes
both original fixtures; the candidate agrees with the dense references within
4e-14 in both.

Local diagnostic timing on a shared cloud VM, four workers, alternating
candidate/main replays, medians of five runs (ten for the last two rows),
`NATIVE_SECONDS`. These are descriptive engineering checks, not qualification
evidence; repeated runs varied by several percent.

| Design | Rows | Candidate/main |
| --- | ---: | ---: |
| Three dense FEs, unit weights | 100,000 / 1,000,000 / 4,000,000 | 0.55 / 0.87 / 0.61 |
| Three dense FEs, analytic weights | 100,000 / 1,000,000 / 4,000,000 | 0.96 / 1.01 / 0.83 |
| AKM-like three sparse FEs | 1,000,000 | 0.38 |
| Mixing designs with 3, 4, 8 and 16 FEs | 100,000 | 0.52-0.62 |
| Near-confounded third FE | 100,000 | 1.02 |
| Campaign-shaped three sparse FEs | 1,000,000 | 1.04 (1.03-1.10 across runs) |

Stopping the recurrence a decade below the request instead of a hundredth cut
one to three LSMR iterations from every LSMR-routed solve above (for example
17 to 15 on the AKM-like design and 26 to 23 on the campaign-shaped one). No
case needed the correction. Against the candidate without this margin, ten
alternating runs on a slower replacement VM gave 0.95 on both one-million-row
sparse designs; the 100,000-row ratios (0.97-1.06) were within that host's
noise.

The last two rows need the additive preconditioner from the start; the Linux
near-confounded result above is the corresponding accepted exception. Weighted
dense designs do not reach the unit-weight pilot.

A one-pass shifted projection route from the research plan failed two existing
witnesses (CG energy roundoff and tiled-bank cross-thread identity), so MAP
projections keep the scaled two-pass route. A bit-identical cache-blocked
row-streaming kernel for dense group statistics and certificate norms was
faster on the development Mac but slower on Linux at 4, 8 and 28 workers.
Complete four-million-row dense three-FE fits took 5% longer at four workers
and 21% longer at 28, so it was not adopted
([record](../bench/results/r4-row-streaming-20261026.json)).

## Many-FE Schwarz admission at scale

(Historical: the many-FE LSMR route and this additive-Schwarz admission model
were retired with `within` in Milestone 3; see
[the within retirement](#within-retirement-milestone-3-development-mac).)

The beta.8 campaign's sparse three-FE OLS fit did not finish at ten million
rows. Traced SCC reruns of main (`a023073e`) showed why: at 1M and 3M the
geometry policy's Jacobi trial certified in 23 LSMR iterations, but at 10M the
general categorical route refused the additive LSMR attempt and ran the
original-input MAP fallback (a perf attach saw only borrowed-MAP kernels, 5.0
GiB RSS at about 3.6 cores). The refusal came from the additive-Schwarz memory
bound, which charged `T + R` level-sized banks per concurrent RHS with
`R = levels * (effects - 1) / 10,002`, quadratic in the levels: 11.9 GB at 1M,
95 GB at 3M and about 1 TB at 10M for 17 RHS and four workers, against measured
peaks of 1.96 and 5.91 GiB. Exactly three pair components of this design have a
block of at least 10,000 levels at every size. The repair counts them when the
level bound refuses ([SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons));
the 10M solver estimate becomes 32.4 GB and the fit takes
the unchanged geometry route (24 iterations, no escalation).

On one whole E5-2680 v4 node (three repetitions, baseline and candidate
alternating up to 3M), median native seconds were 0.38, 1.33, 4.51, 14.47,
25.9, 36.8 and 65.0 at 100k, 300k, 1M, 3M, 5M, 7M and 10M rows, with peak
process RSS 0.23 to 17.0 GiB. Baseline and candidate were bitwise identical
through 3M, as was the candidate with the campaign's beta.8 3M capture. At 10M
the coefficients agree with FixedEffectModels.jl at 1e-10 tolerance and with
PyFixest to 2e-13 relative, and the covariance to 1.5e-13 of the
standard-error scale after the absorbed-DoF convention. The campaign's 10M
references were 55 s (PyFixest), 105 s (FixedEffectModels.jl) and 338 s
(fixest); their peak memory, 4.4 to 12.6 GiB, stays below fereg's. Deterministic
mode repeated bit for bit at 10M; default mode differed in the last bits there
because within's non-reproducible adjoint uses an atomic scatter for terms of at
least 100,000 levels (the 100,000-level firm effect), which no earlier size
of this design reached.

Local checks: `./scripts/run_tests.sh --source-only` passed on the M2 Ultra; the
candidate arm64 plugin's replay against the published beta.9 macOS plugin was
exact for 73 of 73 cases; the many-FE corpus cells' level bounds are at most
1.9 GB, so none reaches the refinement. A one-million-row campaign-topology
test admits the additive route under a budget one byte below the level bound
with the unbudgeted fit's bits; with the refinement disabled it stayed in the
MAP fallback past 600 seconds. Record:
[three-fe-scale-2026-10-28](../bench/results/three-fe-scale-2026-10-28.json).

## Many-FE memory, shared trial design and candidate session windows

(Historical: the many-FE LSMR route, its session windows and the exact
yielding-component admission were retired with `within` in Milestone 3; see
[the within retirement](#within-retirement-milestone-3-development-mac).)
Branch `codex/many-fe-windows` (from the sparse three-FE scale fix) adds four
changes for beta.10; the record is
[many-fe-windows-2026-10-28](../bench/results/many-fe-windows-2026-10-28.json).

1. The exact yielding-component admission now also covers explicit
   `solver(lsmr)` (borrowed and owned designs), the automatic owned-design
   LSMR attempt and the explicit PPML LSMR preflight, only after the
   level-count bound refused ([SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons)).
   Witnesses run each route on the sparse three-FE topology under a budget one
   byte below the level bound: the fits are admitted with the unbudgeted
   fit's bits (PPML: agreement within 2e-9 and exact repeats), at one and four
   workers, while a budget below the exact bound and a sloped design are still
   refused.
2. The Jacobi trial and its additive escalation share one prepared pinned
   design (bit-identical; a digest of escalating fits matches the parent
   revision). With the pinned `within` only the design is shareable: it costs
   0.1-0.3 ms on sorted designs and 5-7 ms at one million locality-sorted
   rows, against 45-56 ms for the additive build; the larger saving this
   section's predecessor anticipated needs a `within` change (shared square
   root weights, stable adjoint and solver frame).
3. The many-FE windows transport and release their own RHS columns, and OLS
   Fast releases its projected matrix after the finite solve. Admission
   totals are unchanged, so `memory(auto)` chooses as before. Campaign data,
   one million rows, 16 controls, four workers, Mac process peak (including
   Stata and 0.21 GB of data): three-sparse 1.876 to 1.778 GB, three-dense
   0.726 to 0.724 GB; bits unchanged. The implicit identity `LevelIndex` and
   u32 observation indexes were not made: `members()` returns `&[usize]` to
   about 200 call sites across MAP, OLS, PPML and slope code.
4. Hidden `routing_many_windows_v1` is a candidate policy, not a public
   default: at most one live LSMR session per worker, with the Jacobi
   escalation barrier at each window's end. Without an escalation it is
   bit-identical to public Auto; after one, later windows start on the
   additive solver. Dense explicit-dummy QR witnesses pass at 1, 2, 4 and 8
   workers with exact fixed-worker repeats. Of the routing families,
   small-third, four-, eight- and sixteen-FE are bit-identical to public at
   one and four workers; near-confounded-third is identical at four and at
   one worker takes 18 instead of 19 projection iterations (coefficients
   within 2.7e-14, covariance within 2.2e-11).

Mac timing (M2 Ultra, arm64 profiling build of `acdfb097`, Stata 19 MP batch,
fresh process per fit, three alternating repetitions, native seconds median,
peak RSS GB). Rule frozen before collection: wait until no other Stata,
cargo, rustc or Python process runs; a fit during which one appears is
discarded and repeated once (one public fit was repeated).

| Design | Workers | Public s | Hidden s | Public GB | Hidden GB |
| --- | ---: | ---: | ---: | ---: | ---: |
| three-sparse | 1 | 6.64 | 5.81 | 1.77 | 1.05 |
| three-sparse | 4 | 1.94 | 1.79 | 1.79 | 1.37 |
| three-sparse | 8 | 1.36 | 1.17 | 2.00 | 1.79 |
| three-dense | 1 | 1.17 | 1.21 | 0.72 | 0.72 |
| three-dense | 4 | 0.45 | 0.45 | 0.73 | 0.73 |
| three-dense | 8 | 0.30 | 0.30 | 0.73 | 0.73 |

The sparse design's outcome RHS escalates; with windows the later RHSs start
on the additive solver (19 instead of 22 iterations), and results agree with
public within 2.3e-14 of the coefficient scale. The dense design certifies in
the MAP pilot, so both arms run identical code and results; its differences
are noise.

SCC confirmation at ten million rows (campaign three-sparse input, threads(4),
one whole E5-2680 v4 node, two repetitions per arm in a fixed alternating
order, every fit reported): base is the branch parent `cf5ddfc0`, candidate
`acdfb097` uninjected, hidden `acdfb097` with the windows token.

| Arm | Native s (rep 1, 2) | Peak RSS GiB (rep 1, 2) |
| --- | --- | --- |
| Base public | 73.6, 72.4 | 16.89, 17.03 |
| Candidate public | 75.0, 74.3 | 15.63, 15.48 |
| Candidate hidden | 68.8, 69.1 | 9.65, 9.64 |

Public coefficients agree with the base to 1e-17 of their scale (default mode
repeats differ in the last bits, as in the scale record); the hidden arm
agrees with public within 1.1e-14 (coefficients) and 1.1e-13 (variance, RSS).
Its later windows finish in 19-20 instead of 22-23 iterations. The public
candidate's 2% longer native time over two repetitions is not resolved by
this confirmation; its peak is 1.3 GiB lower.

Local checks: `./scripts/run_tests.sh --source-only` passed (319 Python
tests, 1500 core library tests, plugin and oracle tests, Clippy, allocator
audits, portable checks, complexity ratchet). The candidate arm64 plugin's
replay against the published beta.9 macOS plugin was exact for 73 of 73 cases.
Public activation of the windows policy needs the bounded qualification and
Johannes's decision.
## Pinned within fused LSMR step

(Historical: fereg no longer depends on `within`, retired in Milestone 3; see
[the within retirement](#within-retirement-milestone-3-development-mac).)
Branch `codex/within-fused-lsmr` pins `within` `3b5b9b9a` of
`codex/fused-lsmr-pass` in three reviewed steps
([record](../bench/results/within-fused-lsmr-2026-10-28.json)).

- `17fcf288` shares a session's response with its unit-weight RHS and an
  unseeded recurrence's RHS. A unit-weight seeded session holds one fewer
  observation-length vector and an unseeded one two fewer. Storage only: the
  macOS replay against the published beta.9 plugin was exact for 73 of 73 cases.
- `2ba7239d` runs each modified-LSMR step as one pass over observations and
  sums every design adjoint in fixed row lanes, with no atomics. `A v`, the `u`
  update and its norm are bitwise unchanged; the declared scope is the adjoint's
  low bits. Default-mode `within` adjoints are now repeatable and identical
  across worker counts; before, the atomic scatter for terms with at least
  100,000 levels made repeated ten-million-row fits differ in their last bits.
  Whole solves are repeatable and worker-count independent with Off, Diagonal
  and ordered additive preconditioners. Without `deterministic`, fereg's
  additive Schwarz still uses `within`'s Auto reduction, which may select an
  atomic scatter; on the campaign sparse design below, fused fits nevertheless
  matched bitwise at 1, 4 and 8 workers while beta.9's did not. Fourteen three-FE
  replay inputs change within the
  [declared scope](../tests/replay/scopes/2026-10-28-within-fused-lsmr-macos.json)
  (467 leaves); solver codes, used solvers, fallbacks, stop reasons, iteration
  counters and discrete decisions are unchanged, and no one- or two-FE case
  changes. Scoped replay passed all 73 cases with four- and eight-worker
  repeats and cross-worker science.
- `3b5b9b9a` adds `Solver::with_preconditioner`, which swaps a solver's
  preconditioner on its prepared frame and is bitwise a fresh construction.
  fereg does not use it yet; replay against the `2ba7239d` plugin was exact.

`./scripts/run_tests.sh --source-only` passed at both runtime pins, including
the many-FE witnesses: public geometry trial against dense QR with and without
escalation, the portfolio against strict QR with discrete decisions across
workers, near-confounded escalation, and explicit three-FE LSMR against
explicit dummies. The complexity ratchet is unchanged. Inside `within`, the
separate kernel matches 24 session digests recorded at the previous pin, and
the fused kernel repeats at 1, 2, 4 and 8 workers.

Descriptive Mac timing (Apple M2 Ultra, shared with other agents; frozen
competitor rule, medians of six clean fits): native seconds for 1M rows and 16
controls, fused pin against beta.9.

| Design | 1 worker | 4 workers | 8 workers |
| --- | ---: | ---: | ---: |
| Campaign three sparse FEs | 0.79 | 0.84 | 0.90 |
| Campaign three dense FEs (MAP, no LSMR) | 1.00 | 0.99 | 0.98 |

The `within` step kernel alone, on the same sparse topology with 16
right-hand sides and 23 steps, ran at 0.65-0.79 of the separate kernel with
the diagonal preconditioner and 0.78-0.93 with additive Schwarz. On that
design the lane merge costs 1.93 MB of buffers per live session and about 11%
of a single-RHS step at eight workers (merge plus zeroing). Peak RSS for the
sparse fit at eight workers fell from 2.12 to 2.00 GB. fereg's LSMR admission
is unchanged and remains an upper bound. These timings are not qualification
evidence; `bench/qualify.py` has not been run.

## PPML two-FE CMG reuse and lean protocol

PPML Auto with exactly two intercept FEs reuses one CMG hierarchy across IRLS
frames and canonical products, and publication fits enter the strict
checkpoint once the predictor settles, as specified in
[SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons) and §13.
Johannes accepted the result; it ships in v0.5.0-beta.7. The
[lean record](../bench/results/ppml-two-fe-lean-20261026.json) retains every
timing of the final qualification and the
[CMG record](../bench/results/ppml-two-fe-cmg-20261026.json) the earlier
CMG-reuse round.

On one whole node (`scc-wk1`, Xeon E5-2680 v4), campaign-generator PPML inputs
with 16 regressors ran five rounds per cell with rotated arm order: shipped
beta.6, the CMG-reuse head `7db43e41` and the lean head `277c5d11`. Times are
medians; ratios are medians of paired rounds.

| Design | Rows | Workers | beta.6 s | `7db43e41` s | Lean s | Lean/beta.6 | Lean/`7db43e41` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Two sparse FEs | 1,000,000 | 4 | 17.75 | 11.05 | 5.82 | 0.324 | 0.528 |
| Two sparse FEs | 1,000,000 | 28 | 10.86 | 7.56 | 4.13 | 0.380 | 0.543 |
| Two sparse FEs | 4,000,000 | 4 | 81.30 | 45.28 | 26.37 | 0.327 | 0.582 |
| Two sparse FEs | 4,000,000 | 28 | 48.36 | 32.92 | 18.04 | 0.374 | 0.549 |
| AKM | 1,000,000 | 4 | 5.07 | 5.02 | 4.20 | 0.833 | 0.850 |
| AKM | 1,000,000 | 28 | 3.36 | 3.34 | 2.76 | 0.820 | 0.826 |
| AKM | 4,000,000 | 4 | 22.94 | 22.65 | 18.66 | 0.811 | 0.828 |
| AKM | 4,000,000 | 28 | 14.71 | 14.55 | 12.29 | 0.835 | 0.845 |
| Two dense FEs | 1,000,000 | 4 | 4.70 | 4.54 | 3.88 | 0.825 | 0.852 |
| Two dense FEs | 1,000,000 | 28 | 3.01 | 2.94 | 2.49 | 0.823 | 0.845 |
| Two dense FEs | 4,000,000 | 4 | 20.30 | 19.93 | 14.90 | 0.734 | 0.755 |
| Two dense FEs | 4,000,000 | 28 | 12.42 | 12.17 | 10.15 | 0.819 | 0.831 |

Against beta.6, coefficients and covariance stay within 2e-5 and 7e-10 of the
§20 envelopes (largest on the sparse design, whose published state is one
Newton step earlier), and sample, status, DoF, drop, separation and FE
fields are exact in every cell. Every arm repeats bit for bit across rounds;
4- and 28-worker lean results agree within 1e-8 and 2e-10 of the envelopes
with exact discrete fields. IRLS iteration counts are unchanged except the
1M-row sparse design (8 instead of 9). The sparse design defers its strict
checkpoint once (1M) or twice (4M) and passes the first one. The earlier
CMG-reuse round had left AKM four-worker cells at 1.048-1.062; all cells are
now faster than both baselines.

The 73-case replay corpus passes its independent dense science targets in
every case with no discrete, repeat or thread differences. All OLS cases are
bit-identical to `7db43e41`; seven PPML cases differ by at most 1.2e-14
relative in coefficients and covariance. New witnesses cover the settling
rule, small and shared-CMG implicit components, the implicit assembly choice,
team-independent joint checks and Gram, refreshes and the three-attempt
bound. The full source-only suite passed at `277c5d11`. A parallel CMG
application, a row-tiled certificate Gram and a data-based IRLS start were
measured and rejected (slower, or no fewer iterations).

### PPML two-FE parity work (development, Mac)

PPML exact-timing protocol: campaign-DGP PPML cases are written by
[`bench/ppml_cases.py`](../bench/ppml_cases.py) (maintained generator, policy
seed, 16 regressors, robust VCE, `separation(fe simplex ir)`, tolerance 1e-8,
iterate 10000) and timed with [`bench/time_exact.py`](../bench/time_exact.py),
which guards exact science between the plugins it compares. The beta.7 lean
record's case generator was never committed; this tool reproduces its protocol
for arithmetic-preserving PPML candidates. Result-changing PPML candidates
still need an extension of `bench/qualify.py`, which refuses PPML today.

On an M2 Ultra Mac (development measurement, three repetitions per cell,
medians of repetitions 2-3, arms interleaved, no competing build/test/Stata
process observed), the exact parallel joint sections and the reuse of a
finished canonical probe changed no bit of `e(b)` or `e(V)` in any of 17
cells (campaign-generator AKM, sparse and dense designs at 1M rows with 16
regressors at 1, 4 and 8 workers; synthetic Veneto-shaped Poisson panels of
1.39M and 5.63M rows at 1, 2, 4 and 8 workers), and the 73-case replay was
exact against beta.8. Native time was 0.975-0.986 of beta.8 on the campaign
designs at 4 and 8 workers and 0.994-1.009 on the Veneto panels. Separation
IR is 42-61% of PPML native time on the Veneto panels at 4 and 8 workers;
the joint frames' scalar CMG cycle is 5-8% on the long panel. A team
application of the PPML hierarchy above CMG's 350,000-edge single-RHS
minimum (row-operator plan only) left results bit-identical but changed the
long panel's native time by 0.980-1.013 (1-8 workers) against the same build
without it, short of the 5% acceptance margin; it was not retained.

## CMG admission level cap (development, Mac)

CMG `5c5b5b2b` caps its default hierarchy depth, and with it every pre-build
admission bound, at 32 levels instead of 128 (Johannes's decision; not part of
the beta.9 qualification). On the synthetic AKM panels of 1.39M and 5.63M rows
(`fereg y <controls>, absorb(worker firm)`, 1 and 4 workers) the
[memory record](../bench/results/cmg-admission-level-cap-2026-10-27.json)
shows the admitted serial CMG construction bound falling from 640 MB to
166 MB and from 1,759 MB to 446 MB; the parallel bound and the retained charge
of two-FE OLS Auto fall by the same 474 MB and 1,313 MB. Both hierarchies have
3 levels, and `e(b)`, solver rows, hierarchy traces and every reported peak
(`e(fereg_memory)`, the routing and cell traces) are identical before and after.
Those reported peaks do not include the CMG admission stage: they are the same
when CMG is refused. Under `FEREG_MEMORY_LIMIT_BYTES` the short panel's CMG
install threshold fell by 473-475 MB (predicted 474 MB); at a 1.25 GB limit the
parent refused CMG (Schur PCG, 52 iterations) and the head installed it (17
iterations, the unconstrained result). Replay is exact against the parent's
plugin for all 73 cases without a scope; against beta.8 only the existing
eager-cells scope applies. CMG's full suite and `./scripts/run_tests.sh
--source-only` passed; no installed package was built or checked.
### PPML separation routing (development, Mac; not activated)
### PPML separation routing (development, Mac; activated for beta.10)

The iterated rectifier was 42-66% of PPML native time on the Veneto-shaped
Poisson panels. On both panels the separation chain's IR phase is one graph
proposal followed by a single rectifier projection that certifies no further
separation at iteration 1: a weighted finite-plus-FE projection of 3 (short) or
12 (long) columns with zero-outcome weights near 1e-21, solved by
retained-product CG (60-61 iterations, no preconditioner hierarchy). The
hidden `inject=ppml_separation_auto` arm solves admitted two-intercept detector
projections with the OLS two-FE Auto route instead; the detector's weights,
tolerances, iteration limit and decision rules are unchanged and the public
default is not changed. Measured on an M2 Ultra (three repetitions per cell,
medians of repetitions 2-3, arms interleaved, no competing process; the
[record](../bench/results/ppml-separation-routing-2026-10-27.json) keeps every
cell):

| Panel | Workers | Public native s | Hidden native s | Ratio | Public IR s | Hidden IR s | Ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Short (1.39M) | 1 | 14.03 | 6.25 | 0.446 | 9.02 | 1.24 | 0.137 |
| Short (1.39M) | 4 | 6.43 | 4.29 | 0.666 | 3.23 | 1.04 | 0.321 |
| Short (1.39M) | 8 | 5.01 | 3.68 | 0.735 | 2.11 | 0.79 | 0.374 |
| Long (5.63M) | 1 | 112.08 | 46.17 | 0.412 | 74.09 | 8.70 | 0.117 |
| Long (5.63M) | 4 | 43.48 | 23.91 | 0.550 | 26.50 | 7.10 | 0.268 |
| Long (5.63M) | 8 | 29.64 | 18.92 | 0.638 | 15.60 | 5.00 | 0.320 |

On both panels at 1, 2, 4 and 8 workers, on four campaign-generator
separation designs (dense, AKM strong and weak mobility, probability weights)
and on the replay corpus, `e(b)`, `e(V)`, the separated rows, drop counts and
`e(feppml_separation_sequence)` are identical between the arms, and
`e(feppml_separation)` differs only in the rectifier's interior certificate
magnitude; the separation trace digest in the receipt hashes that magnitude
and therefore also differs on `ppml-separation-ir`. Each arm repeats bit for
bit. The public replay is exact (73 of 73 cases) with the arm compiled in.

The witnesses do not hold on every case. A seeded census
(`crates/fereg-oracle/tests/ppml_separation_routing.rs`: 18 small chain,
sparse-AKM and dense designs, some with induced finite-regressor separation,
three separation configurations, public tolerances) is identical in 32 of 54
cells. Where both arms succeed the separated set never differs, but in 9 cells
the hidden arm certifies the terminal rectifier at iteration 1 where CG needs
2, which changes RSEP and RSEQ iteration counts. In 9 cells retained CG fails
a projection certificate and the public fit errors while the hidden arm
completes; in 2 cells the Auto route exhausts its iteration budget where the
public fit succeeds; 2 cells fail in both arms with different errors. Under
`fe simplex ir` the divergences are 5 iteration-count cells and 3 public
failures. The hidden arm is deterministic at fixed workers and its decisions
agree across 1, 2, 4 and 8 workers wherever it succeeds. In an isolated unit
test, a Break at every host poll inside a rectifier projection returns
`Interrupted` on both arms.

The public failures were a detector defect, now fixed (SPEC §14, the
[record](../bench/results/ppml-separation-routing-2026-10-27.json)'s
`post_fix_census`). Retained CG solves the rectifier projection on the
weighted normal equations, which square the zero-row weight
`tolerance^2 / zeros` (about 1e-18): on six cells the proposed direction left
the model span (independent least-squares span distance 0.14-0.19 on seeds 11
and 17, 2e-8 on seed 5) and failed the deletion certificate, r(430). On three
`fe simplex ir` cells the span check of an exact simplex direction lying in
the FE span failed its finite score certificate, normalized by FE-solver
noise, r(459). A detector step that fails such a certificate now runs once
more with LSMR under the same rules and certificates. All 9 cells complete
and retain exactly the sample of an independent HiGHS linear program
(maximal recession-cone separation alternated with singleton pruning), which
every completing arm of all 54 cells also matches. Every other cell, in both
arms, is unchanged bit for bit (full separation report, `e(b)`, `e(V)` and
error messages). Seed 3 (2 cells) still fails in both arms: its CG direction
has the right rows but span distance 1.8e-10 over the 1e-10 tolerance, and
LSMR ends 15% over its FE certificate. The replay corpus is exact against the
published beta.9 plugin (73 cases). Through Stata, the four campaign
separation designs (4 and 8 workers) and both Veneto-shaped Poisson panels
(1, 2, 4 and 8 workers) give identical `e(b)`, `e(V)`, separation matrices,
drop counts and dropped rows with no re-execution; run through Stata, the
seven fixed census cells that return r(430) or r(459) under beta.9 complete
with the LP sample (seed 3 stays r(430)). Both arms repeat
bit for bit at fixed workers and agree across 1, 2, 4 and 8 workers.

After the fix the census is identical in 35 of 54 cells and differs in 15
iteration-count cells (equal sample and fit), 2 hidden-route failures and the
2 seed-3 cells; under `fe simplex ir` only 5 iteration-count cells differ.
The two hidden failures (seed 9 without simplex) occur in the first rectifier
projection, not through the rectifier's iteration limit: the Auto route's
two-FE PCG accepts the final residual absorption at iteration 0 on its
recurrence proxy, the FE certificate (the same as CG's) rejects it, final
residual absorptions have no PCG correction, and the MAP fail-safe exhausts
its 16,000 iterations; retained CG certifies the same residual in 8
iterations.

The residual was accurate; the certificate's arithmetic was not. The
categorical FE certificate (shared by two-FE PCG, retained CG and MAP) forms
each level mean as the first member's value plus a compensated sum of
weighted differences. When that member is a zero-outcome row with a residual
of order one, the positive rows' residuals (about 1e-18) vanish in their
differences and the mean carries one ulp of the shift, which against the
rectifier's weighted residual norm (9.1e-9) is of the order of the 1e-8
tolerance: exact relative moments at most 2.7e-10, computed 1.06e-8, and MAP
corrections below one ulp leave every bit unchanged for 16,000 sweeps; retained
CG's later iterates certified (8.5e-21). A stop scaled like the certificate
would not remove that floor. Since 2026-10-28 an Auto-routed detector step
that fails a numerical certificate takes no MAP continuation and runs once
more as retained CG under the same budget and rules, then continues exactly
as a retained-CG step. Both seed-9 cells complete with the LP-witness sample
and the seed-3 cells report the retained-CG error in both arms; every other
census arm-cell is bit-identical at 1 and 4 workers (full report with
certificate magnitudes, `e(b)`, `e(V)`, errors). The census is then 39 same
and 15 iteration-count cells.

#### Activation (beta.10 candidate, 2026-10-28)

Johannes authorized activating the Auto route for beta.10 with a declared
scope for the iteration-count class (SPEC §14): the separated set, sample,
`e(b)` and `e(V)` are unchanged on every witness; `RSEP` iteration counts and
certificate magnitudes, `RSEQ` iterations and the receipt's separation trace
digest may change. `inject=ppml_separation_cg` keeps the retained-CG path.
Witnesses, all on the activated arm64 profiling candidate against the
published beta.9 plugin with the same ado (the [record](../bench/results/ppml-separation-routing-2026-10-27.json)'s
`activation`):

- Census (`ppml_separation_routing.rs`, CG arm vs default): all 54 cells
  agree in sample and fit (39 identical, 15 iteration-count cells, no
  failure class); every completing arm retains the LP-witness sample;
  fixed-worker repeats and cross-worker decisions hold at 1, 2, 4 and 8
  workers.
- Stata: the four campaign separation designs (4 and 8 workers) and both
  Poisson panels (1, 2, 4 and 8 workers) give `e(b)`, `e(V)`,
  `e(feppml_separation_sequence)`, drop counts, `e(feppml_fit)` and the
  dropped rows identical to beta.9 at every worker count (1,794, 430, 1,078,
  430, 7,498 and 1,536 dropped rows); `e(feppml_separation)` differs only in
  the rectifier row's interior certificate magnitude and the receipt only in
  the separation trace digest. Each worker count repeats bit for bit; dropped
  rows, drop counts, the separation sequence and the discrete separation
  columns agree across worker counts (coefficients within 2.1e-12 relative).
  No detector re-execution occurred. The CG arm reproduces beta.9 exactly,
  separation matrices and receipt included, on all six designs.
- Replay (`scripts/run_plugin_replay.sh`, 73 cases): 72 exact; only
  `ppml-separation-ir` differs, in `RSEP` zero-based 22 (the rectifier row's
  interior certificate, 2.69e-20 to 2.39e-20) and `RREC` 28-29. The replay
  harness now declares this class (development workflow, plugin replay):
  `RSEP` certificate magnitudes and iteration counts are scope leaves by row
  stride, and the separation trace digest is accepted only beside a declared
  `RSEP` leaf with a separation reason. With the
  [declared scope](../tests/replay/scopes/2026-10-28-ppml-separation-activation-macos.json)
  and the [within fused LSMR scope](../tests/replay/scopes/2026-10-28-within-fused-lsmr-macos.json),
  replay of the integration head (`d64fdc0d`, arm64) against the published
  beta.9 plugin passes 74/74 (15 scoped cases, no science, exact or discrete
  failures outside the declared leaves).
- Break: an isolated unit test cancels at every host poll inside a rectifier
  projection of the activated policy (Auto, no MAP continuation, retained-CG
  re-execution) and of retained CG; every stop returns `Interrupted` and is
  not re-executed.

Timing (frozen rule: before each cell, no other Stata, cargo, rustc or Python process
and no other agent's timing driver for 60 s, a 10-s monitor during the cell, discard and
repeat once on a competitor; one cell, short at 1 worker on the candidate, was discarded
for a rustc process and repeated cleanly). arm64 profiling builds of the activated tree and
of v0.5.0-beta.9, Stata 19/MP on the M2 Ultra, three repetitions per Stata process, medians
of repetitions 2-3, arms alternating first per cell, `FEREG_PHASE_TIMING=1`:

| Panel | Workers | beta.9 native s | Activated native s | Ratio | beta.9 IR s | Activated IR s | Ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Short (1.39M) | 1 | 14.72 | 6.93 | 0.471 | 9.32 | 1.38 | 0.148 |
| Short (1.39M) | 4 | 6.50 | 4.38 | 0.674 | 3.27 | 1.05 | 0.322 |
| Short (1.39M) | 8 | 5.03 | 3.67 | 0.729 | 2.13 | 0.77 | 0.363 |
| Long (5.63M) | 1 | 87.88 | 38.11 | 0.434 | 57.01 | 7.29 | 0.128 |
| Long (5.63M) | 4 | 34.19 | 20.55 | 0.601 | 19.81 | 6.23 | 0.314 |
| Long (5.63M) | 8 | 23.81 | 16.23 | 0.682 | 12.03 | 4.35 | 0.361 |

The rectifier's share of native time falls from 42-65% to 19-30%.

Seed 3 (cells (3,1) and (3,2)) was a documented limitation. With a
60-digit reference projection the rectifier's exact direction flags the 20
LP-witness rows and passes every deletion certificate (span 6e-19). Retained
CG's fitted values are wrong by up to 2.3e-10 on zero-outcome rows only
(4.9e-17 on positive rows; `u - r` formation is exact), which the weighted
FE certificate cannot see at weight 1e-18 but the unit-weight span check
(1e-10) rejects (1.82e-10). LSMR's and Auto's directions are accurate (span
3.3e-17 and 1.7e-12, same 20 rows) but their residuals fail the FE
certificate's first-member-shift floor (computed 1.149e-8 and 4.25e-8, exact
6.3e-23 and 1.2e-18; independent of summation order).

Johannes chose the narrow re-certification (2026-10-28, SPEC §14): when an
Auto detector step's retained-CG and LSMR re-executions also fail, the Auto
step runs once more and its two-FE PCG certificate miss re-certifies the
rejected candidate with each level's mean formed about its largest-weight
member, at the same scales and tolerance
(`prepared_separation/recertify.rs`); the shared FE certificate is unchanged.
Witnesses at the integration head plus this change, against the integration
head (`d64fdc0d`):

- Census (`ppml_separation_routing.rs`): full fingerprints of all 108
  arm-cells (separation report with certificate magnitudes, drop reasons,
  `e(b)` and `e(V)` bits, errors; elapsed seconds excluded) at 1 and 4
  workers are identical except the Auto arm of (3,1) and (3,2), which now
  completes with the LP-witness sample (220 rows) after the last attempt
  instead of the retained-CG error. The retained-CG arm keeps that error, so
  the census is 37 same, 15 ir-iterations and 2 cg-fails; fixed-worker
  repeats and cross-worker decisions hold at 1, 2, 4 and 8 workers.
  Seed 9, whose Auto miss retained CG certifies, is unchanged.
- Unit tests: the re-check removes a constructed first-member floor that the
  shared certificate rejects and matches compensated level sums about zero,
  while a genuine 1e-6 moment fails both; an injected two-FE PCG miss reaches
  the re-check only on a miss, a certified PCG result never consults it and is
  bit-identical; the detector policy arms it only after the Auto, retained-CG
  and LSMR attempts all fail numerically, reports the retained-CG error when
  it fails, propagates Break and allocation failures and never arms it for a
  retained-CG detector.
- Replay: all 77 cases are bit-identical in the exact view between arm64
  builds of the integration head and of this change; against the published
  beta.9 plugin with the two beta.10 scopes, 75 pass and the one- and
  three-FE derived-residual cases differ as described in the derived-residual
  section.

## Parallel numeric input (beta.13)

Transport-only change (SPEC §17): workers call Stata's numeric accessor inside
a validated window; loaded values, missing flags, candidates, fills, error
precedence and all scientific results are unchanged. Independent review found
two blockers and nine should-fix items before implementation: concurrent
`vdata` safety rests on qualification, aliases are excluded up front, the input
team stays separate from the tiled-pipeline gates, the completeness check uses
the loaded columns, refusals after the first phase are errors, the overlapped
factorization is not combined with parallel reading, and a leaked window cannot
brick the plugin. All were adopted.

Native evidence: transport unit tests (exact values/flags/candidates for widths
1–130, all-candidate/runs/scattered/tail/none samples, 2/3/8 workers, block and
phase edges, marker and payload error precedence, Break between phases, worker
panic, read failure, declines before dispatch, repeatability across worker
counts); loader equivalence of every path (tiled with packed RHS and
factorization hint, batched unrestricted/restricted, string peer, weights) at
2/3/8 workers; C window guards (open validation, refusal of every other wrapper
while open, worker reads only inside the window and generation, leaked-window
recovery) under UBSan; `scripts/run_tests.sh --source-only` and the complexity
ratchet with eight declared growth entries.

Stata qualification: Mac rows use `84704edb` (plugin sha256 `aa7eef3f…`), Linux
rows `e2fee7a4` (identical runtime source; `84704edb` adds the oracle's segment
case, so Linux oracles have 62–63 cases). The oracle (`parallel_input_oracle.do`) covers byte/int/long/
float/double with all 27 missing codes, unrestricted/`if`/`in`/`if`+`in`/
scattered/tiny/empty samples, aw/fw/pw, factors, slopes, two FEs with
clustering, a string FE, PPML offset and exposure, reordered and sorted data,
`preserve`, a non-default frame, small storage segments, repeated calls and
alias variables (serial). Every case's input digest is identical across
threads(1), forced-serial and parallel runs; forced-serial and parallel results
are bit-identical at equal workers. `parallel_input_failures.do` injects a
worker panic and a read failure (r(3499), full rollback, no serial retry, exact
follow-up). `estimate_input_break.do` sends a real Control-C through the PTY
harness after the first joined phase (OLS and PPML roll back; exact follow-up).
`missing_witness` stores NaN, ±Inf, `DBL_MAX`, the `.` boundary ±1 ulp and all
missing codes: Stata's `ismissing` equals `value >= SV_missval` for all 80
cells and the checked and unchecked accessors return identical bits.

| Host | Interface | Oracle | Failures | Live Break | Witness |
|---|---|---|---|---|---|
| macOS arm64, Stata 19.0 MP (12 Aug 2026) | console | pass (64 cases, 65 parallel runs) | pass | pass | pass |
| macOS arm64, Stata 19.0 MP | GUI executable (`-e`) | pass (65 parallel runs) | pass | not run¹ | — |
| macOS arm64, Stata 18.0 MP (20 Dec 2023) | console | pass (65 parallel runs) | pass | pass | pass |
| macOS arm64, Stata 18.0 MP | GUI executable (`-e`) | pass (65 parallel runs) | pass | not run¹ | — |
| Linux x86_64 (SCC `scc-tb4`, Xeon E5-2660 v3), Stata 17/18/19 MP | console | pass | pass | pass | pass |
| Linux x86_64 (SCC `scc-gd4`), Stata 17/18/19 MP | GUI `xstata-mp` (Xvfb, `-e`) | pass (64 parallel runs each) | pass | not run | — |
| Windows x86-64 (AWS, 2 vCPUs), Stata 19 MP | GUI executable (`/e`) | pass (2 and 8 workers)² | pass | not run² | pass |

¹ A live GUI Break needs Stata's AppleScript `UtilSetStataBreak`, which requires
a one-time macOS Automation approval; SIGINT terminates the GUI process instead.
Mac Stata 15/17 installations on the test Mac do not start ("cannot find Stata
directory"); they are unqualified and keep the serial reader, as do SE/BE
editions, which were not exercised.

² Windows (branch `windows-parallel-input`, run
`win-20261101T223215Z-7f874c46` at `5405afa9`, receipt `f2f34c5e...`, plugin
`8728826c...`; the instance stopped). The `stata-do` driver builds the
candidate with the Windows Stata/MP 19 GUI row, installs and hash-verifies it,
runs the ten mapped fixtures (which now load through the window) and then:
builds the missing-value witness with MSVC and checks all 80 cells; runs the
input oracle in a second Stata/MP process started with the solver trace (the
controller starts the driver's Stata without it) at the default two workers
and at eight, where the trace checker requires 64 cases, 184 runs, identical
input digests within every case and the window in every parallel run but the
tiny sample; runs the worker-panic and read-failure fixture; and applies the
frozen paired timing rule (`bench/parallel_input_paired.do`: hidden adapters
with and without `inject(serial_input)` alternating in one process, 1M rows,
cells c1-c5 and c7, two workers, five repetitions; a regression is a median
wall time more than 5% and 1 ms above the serial reader). Every step passed.
The controller returns only a sanitized receipt, so the timing medians and
oracle counts stay on the machine; the pass of each rule is the evidence. No
interactive session exists for a live Break; Break between phases rests on the
unit tests and the console Break evidence on macOS and Linux, through the same
Rust/C window code. Two earlier runs (`win-20261101T214908Z-cc726ec3`,
`win-20261101T221240Z-eedd9b90`) stopped at r(9113) because the witness
fixture's `clear all` dropped the driver's result-check program before the
oracle step's result was read; the check is now a do-file. The same 1M-row
paired rule on the Mac at two workers gave wall ratios 0.74-0.97 and input
loading 0.39-0.83 of the serial reader.

Timing: a frozen paired comparison in licensed Stata (the replay timing
harness mocks the accessor, so `bench/qualify.py` cannot measure this change).
Mac M2 Ultra, threads 4/8, eight cells, all 384 timed calls retained
([summary](../bench/results/parallel-input-macos-20261101.md)): no regression;
at eight workers complete OLS input loading falls to 0.10–0.22 of the incumbent
in all-numeric cells (0.65 with a string FE, whose strings stay serial) and
whole-command time to 0.49–0.84; allocation peaks are unchanged. Linux, one
whole SCC node (`scc-wo4`, 28 cores), threads 4/16/28, all 576 timed calls
retained ([summary](../bench/results/parallel-input-linux-20261101.md)): no
regression; at 28 workers all-numeric OLS loading falls to 0.04–0.12 of the
incumbent (0.74 with the string FE) and whole-command time to 0.46–0.78 (PPML
0.88). A 28-worker exactness oracle passed in the same allocation first.

Merge with the PyFixest parity work (`0e98ccbe`). The action protocol bump
changes one request field in every replay case, so two input bindings were
rebound: the 12 cases of the seeded-frame scope and the two bias-reference
inputs. Each new hash was accepted only when restoring `protocol_minor=6` in the
regenerated case reproduced the recorded hash exactly; declared leaves, reasons
and the pinned independent bias targets are unchanged. Replay chain (Mac,
77 cases): the pre-merge branch `686ab424` on protocol 1.7 inputs equals the
beta.12 plugin on the same inputs restored to 1.6 in every case's exact view;
the merged plugin against `686ab424` passes with the seeded-frame scope at 1,
2 and 4 workers (77 scientific passes, no unscoped difference). The source-only
`run_tests.sh`, the parallel-input oracle (64 cases) and failure fixtures, the
PPML public-scope fixture and the candidate-input oracle pass with the merged
Mac plugin.

## fedemean full system and explicit routes (Milestone 2, development, Mac)

OLS Auto with six or more categorical intercept effects, and the
elimination's declines, run fedemean's full system (Jacobi PCG escalating to
additive Schwarz); explicit `solver(lsmr)` and the new explicit
`solver(cg, reduce() precondition())` requests run on fedemean's full system or
elimination (SPEC §1). Witnesses: dense MAP oracles (tolerance 1e-13) for every
full-system plan at one, three, six and seven effects with and without
weights, including coefficient reconstruction; worker-count bit identity (1, 3
and 8 workers), repeated projections and weight changes; admission exactly at
fedemean's bound with one byte less refused; prepared 6/7-FE fits against MAP
at tolerance 1e-13 (Auto, explicit LSMR Schwarz/Jacobi, explicit full CG
Jacobi), fixed-worker bit repeats, cross-worker agreement and a forced
full-system decline reported as a certified fallback; fedemean's escalation
policy (an easy design keeps Jacobi, a slow one escalates and later
projections start on Schwarz) and the whole-component approximate local solve
(symmetric positive definite action). The OLS LSMR iteration Break witness now
observes fedemean (1 and 4 workers, populated and null `e()` restored). The PPML
separation census changes only where the retained-CG arm's LSMR re-execution
moved to fedemean: (3,1) and (3,2) now complete with the Auto arm's bits and
the LP-witness sample.

Matched Mac comparison against the pre-migration head `81f583ad` (1M rows,
`x1-x4`, `vce(robust)`, 4 threads, 2 interleaved processes x 3 fits, medians):

| case | 81f583ad (ms) | d87fe9d4 (ms) | route (d87fe9d4) | max abs db | max rel dse |
|---|---:|---:|---|---:|---:|
| ols6 | 1342 | 332 | full x cg x jacobi | 6.1e-16 | 1.6e-11 |
| ols7 (+hardfirm) | 2210 | 1383 | full x cg x schwarz, 1 escalation | 5.6e-16 | 9.8e-12 |
| ols6, aweights | 1566 | 510 | full x cg x jacobi | 4.2e-16 | 8.8e-12 |
| ols2, solver(lsmr) | 614 | 209 | full x lsmr x jacobi | 2.5e-16 | 2.4e-12 |
| ppml2, solver(lsmr) | 573 | 575 | unchanged | bitwise | bitwise |
| ols2, ols3, ols5, ppml2, ppml3 | 58, 79, 102, 386, 450 | 59, 79, 101, 383, 461 | unchanged | bitwise | bitwise |

Against `tolerance(1e-12)` fits of `81f583ad`, the default (1e-8) fits differ
by rel dse 8.3e-12 (ols6) and 1.4e-11 (ols7), against 9.6e-12 and 1.5e-11
for `81f583ad`'s own defaults; the two arms' 1e-12 fits agree to 7.6e-15 and
2.4e-15. Stata fixtures: no new failures (the four known failures and the
fixtures that need a staged release, launch arguments or Windows excepted);
the Rust, plugin FFI and allocator-audit suites and clippy in both
configurations pass. Limitations: PPML explicit LSMR (its joint augmented
operator and core-pair factor reuse), PPML's strict checkpoint LSMR, slope
designs and the Auto fallbacks keep the pinned LSMR library (all moved off
`within` later in Milestone 3; see below); PPML with six or
more effects keeps MAP; no Linux or SCC timing.

## fedemean with absorbed slopes and saved slope recovery (Milestone 3, development, Mac)

Absorbed slopes run on fedemean's full system (SPEC §1, §5): explicit
`solver(lsmr)` (level channel blocks as `jacobi`, the hybrid Schwarz as
`schwarz`; within keeps explicit `schwarz` with fewer than two intercept
effects and every fit fedemean does not certify, then MAP, reported as the
fallback; the within leg and the hybrid were retired later in Milestone 3, see
[the within retirement](#within-retirement-milestone-3-development-mac)), OLS
Auto on owned slope designs with two or more effects outside
the exact two-effect slope solver (PCG with channel blocks escalating to the
hybrid Schwarz, the generic owner's independent certificate, MAP continuation
as the reported fallback), and saved slope recovery (retained fedemean
coefficients, then fedemean from them or zero, then MAP; fereg's pivoted
representative on rank-deficient levels and the §5 gauge on every path).
Witnesses: `fereg-oracle/tests/slopes.rs` (four slope shapes, intercept and
slope-only, with a constant-slope, a zero-slope and a single-row level, unit
and analytic weights, Auto in both memory modes, explicit LSMR with each
preconditioner and MAP, at 1 and 4 workers against the explicit-design QR;
saved channels from fedemean and from retained coefficients against the MAP
recovery in the §5 gauge, with exact zeros on the singular levels); the
adapter's units (zero-copy view, coefficients reproduce the removed fit,
bitwise repeats at 1/3/4 workers, declines); the within continuation on the
boundary fixture's offset-slope case; Stata slope, saved-output and PPML
saved-slope fixtures.

Matched Mac comparison against `8bcb3022` (the preceding head's package; 1M
rows, `x1-x4`, `vce(robust)`, 4 threads, 2 interleaved processes x 3 fits,
medians; `t = year + mod(_n,5)/5`, a function of year in this panel, and
`u = year + mod(worker*year,7)/7`, identified next to year FE):

| case | 8bcb3022 (ms) | 2441732f (ms) | route (2441732f) | max abs db | max rel dse |
|---|---:|---:|---|---:|---:|
| `worker firm##c.t year` | 641 | 358 | full x cg x jacobi (channel blocks) | 1.4e-16 | 6.2e-11 |
| the same, aweights | 700.5 | 424.5 | full x cg x jacobi | 9.2e-16 | 2.4e-10 |
| `worker firm##c.t`, `solver(lsmr)` | 1413.5 | 635 | full x lsmr x jacobi (8bcb3022: within, schwarz) | 2.4e-16 | 5.7e-12 |
| `fe_w=worker fe_f=firm##c.u fe_y=year` | timeout (> 900 s per process) | 589 | full x cg x jacobi, retained coefficients | – | – |
| `fe_w=worker fe_f=firm##c.u` | 1508 | 500 | two-effect slope solver, fedemean recovery | bitwise | bitwise |
| `worker firm##c.t`, `worker##c.t firm` | 289.5, 246.5 | 291.5, 239 | two-effect slope solver | bitwise | bitwise |
| ols2, ols3, ols5, ppml2 (quiet rerun) | 63, 80, 99, 380 | 61, 79, 98, 381.5 | unchanged | bitwise | bitwise |

Against `tolerance(1e-12)` fits, `worker firm##c.t year` defaults differ by
rel dse 5.9e-11 (2441732f) and 6.3e-11 (8bcb3022); the two arms' 1e-12 fits
agree to 4.4e-14. Saved effects at 100k rows (where 8bcb3022's MAP recovery
finishes: 52.9 s against 57 ms for `fe_w=worker fe_f=firm##c.u fe_y=year`,
169 against 55 ms for the two-effect case): every saved channel's standard
deviation agrees to 1e-11 and the means differ only by the §5 transfer (the
firm intercept's weighted mean moves to the worker channel). The two-effect
slope solver stays Auto's route: with it disabled (measurement build),
fedemean's full system was 3.4% and 4.5% faster on `worker firm##c.t` and
`worker##c.t firm` and 14% faster with saved effects (450 against 524.5 ms),
but its default fits sit 1.5e-10 from the 1e-12 reference in rel dse,
against 3.4e-12 for the incumbent. The first slope run (kept in the run
records) overlapped another lane's compiles; the comparison above is its
quiet rerun, whose only logged interference (one compiler process during the
bitwise intercept cases) prompted the separate quiet rerun of those cases.
Stata fixtures: no new failures (the nine known failures: `mata_correction`,
`native_splitbranch_block`, two PPML allocation audits,
`parallel_input_oracle` without its trace environment, and four installation
fixtures); the slope, saved-output, PPML saved-slope and within slope
boundary fixtures pass. The Rust core, oracle and testkit suites, plugin FFI
(242) and the 11 allocator audits, clippy in both configurations and the
ratchet pass.

Limitations: two-effect slope designs keep the exact two-effect slope solver
(measured faster); slopes whose raw level Gram loses their within-level
variation keep within; the borrowed many-FE within LSMR, PPML slope routes and
the within-shaped LSMR memory estimates are outside this change; no Linux or
SCC timing. (The within leg, the many-FE within LSMR and the within-shaped
estimates were retired later in Milestone 3; see below.)

## Many-FE fallback without within (Milestone 3, development, Mac)

OLS Auto's general categorical route, which three or more categorical FEs
reach after fedemean declines or with fedemean off, no longer runs the pinned
within LSMR: the geometry-qualified MAP pilot where admitted, then MAP with
the requested certificate (SPEC §1). A decline census at `b664fa61` with
temporary counters found no certified within result outside test
constructions. None of the 37 bench designs (1M rows), the 161 Stata
fixtures or `tolerance(1e-14)` reach the route. `iterate(20)` and
`iterate(30)` on ols6, ols7, ols3chain and ols6w decline on fedemean's
iteration ceiling and fail with r(459) before and after, and those and nine
further variant fits repeat bit for bit. In a 680-fit Rust budget sweep
(three to seven FEs, a chain, weights, retained coefficients) the 169
memory-limit declines never admitted the LSMR and run the same MAP bit for
bit. Only the developer fault that starves the full system (6/7 FE, with
and without weights) moves, from within LSMR to MAP: |db| at most 2.4e-14,
relative dSE at most 3.7e-10 and relative RSS at most 2.4e-10 against
`tolerance(1e-12)` MAP fits. With fedemean off, a weak AKM design that MAP
cannot certify at 1e-12 within 512 sweeps now fails clearly where the LSMR
had certified.

Matched Mac comparison against the head package `2441732f` (1M rows, `x1-x4`,
`vce(robust)`, 4 threads, 2 interleaved processes x 3 fits, medians):

| case | 2441732f (ms) | bde79318 (ms) | science |
|---|---:|---:|---|
| ols2, ols3, ols5 | 59.0, 80.5, 101.0 | 59.0, 80.0, 101.5 | bitwise |
| ols6, ols7, ols6 aweights | 368.0, 1382.0, 502.5 | 369.5, 1364.0, 497.5 | bitwise |
| ols2chain, ols3chain | 107.5, 233.0 | 109.0, 236.0 | bitwise |
| ppml2, ppml3 | 382.5, 463.5 | 391.5, 462.5 | bitwise |

Every arm repeated exactly. One unidentified Stata process was sampled during
that run; the one quiet rerun (another lane's build started during its last
cases) was again bitwise, with ratios 0.97 to 1.02.

Tests: fereg-core lib, the workspace without default features (fereg-oracle
included), plugin FFI with the 11 isolated allocator audits, clippy in both
configurations, rustfmt and the ratchet pass; Python 282 (the three
numpy-dependent modules cannot import here). Stata fixtures: 152 of 161 pass
with a plugin-only staging, the nine failures identical with `b664fa61`
(`mata_correction`, `native_splitbranch_block`, the two PPML audits,
`parallel_input_oracle`, and four installation fixtures that need a staged
release, launch arguments or Windows). Limitations: the owned explicit and
Auto LSMR (`fe/lsmr.rs`, `lsmr_batch.rs`), the within-modelled LSMR
admission and the developer tooling still use the pinned library (all retired
in the next section); no Linux or SCC timing.

## within retirement (Milestone 3, development, Mac)

fereg-core no longer depends on the pinned `within` crate. `schwarz-precond`
stays at revision `8d401d82` of the adapted fork (fedemean's modified LSMR,
PPML's explicit joint `[X D]` LSMR and the developer LSMR Break hooks), and
`approx-chol` 0.6.0 stays for fedemean's Schwarz local solves; the CMG pin is
unchanged (SPEC §1).

- Explicit `solver(lsmr)` on owned (slope) designs runs fedemean's full
  system, accepted only by fereg's independent §16 certificate after at most
  one correction at a tenth of the tolerance. A fit fedemean does not certify
  (numerical decline, memory decline, missed certificate) continues with MAP
  from the original input, reported as `e(fe_fallback)=1` with the reason in
  the warnings; there is no within leg. Explicit `precondition(schwarz)` on
  slope designs with fewer than two intercept effects runs fedemean's pair
  subdomains over all channels instead of within's additive Schwarz. A design
  fedemean refuses as unsupported (reachable only through a test fault plan)
  continues with MAP, reported as the fallback; Auto, `absorb_auto` and
  saved-effect recovery are unchanged.
- The slope-boundary fixture's offset case (case 1 of
  `tests/stata/development/ols_offset_slope_boundary.do`; slopes near
  ±2^24 that differ by units, `solver(lsmr, precondition(none))`,
  `tolerance(1e-10)`) failed with r(459) and full rollback between the within
  retirement (`d748f6f3`) and fedemean `f67ea713`, which applies and
  certifies such offset slopes in each level's centered basis. It now
  certifies on fedemean with residual-derived outputs and no fallback, in one
  iteration (Mac fixture run, 2026-11-09): b, SE, rss and
  residuals are bitwise those of the `tolerance(1e-12)` fit at one and four
  workers, and against the fixture's exact oracle |Δb| ≤ 1.5e-15, relative
  Δrss 2.2e-16 and |Δresidual| ≤ 2.1e-15.
- Memory admission (SPEC §19): the within-shaped modified-LSMR estimate and
  the categorical Schwarz model with its exact yielding-pair-component
  refinement are removed. Owned LSMR charges fereg's held buffers and
  certificate scratch, and fedemean admits its own workspace beside them;
  PPML LSMR charges an estimate of fedemean's workspace plus a Schwarz grant
  over its star pairs. Outcomes change only under memory limits between the
  former and the new estimates. On the 161 Stata fixtures the pass set is unchanged (152 pass; the same 9 staging and installation failures as `9fe647ab`), and 13 matched one-million-row Mac cases (the defaults and every explicit route the change does not touch) are bitwise identical to `9fe647ab` on the same routes, so no admission outcome changed there.
- Tooling: the within entry-point, slope-boundary comparison and allocation
  witnesses under `bench/` are removed (Git history keeps them at revision
  `9fe647ab`); [the migration record](within_migration.md) and
  [the optimum enclosures](within_numerical_enclosure.md) are historical.
- Evidence (runtime `d748f6f3`, dependency `e80da565`, Mac, development):
  the source-only integration suite passes (Rust 2,353 tests, including
  fereg-core's 1,542 and the plugin's 242 FFI tests with its 11 allocator
  audits; both Clippy configurations; rustfmt; the locked fuzz check; Python
  279, with the three numpy-dependent modules not run), as do the
  actual-plugin harness and the ratchet (fereg-core 119,999 to 118,260
  production lines). Stata fixtures: 152 of 161 with the same nine failures
  as `9fe647ab`. Against a `9fe647ab` package, ols2, ols3, ols5, ols6, ols7,
  ppml2, ppml3, ppml2lsmr, ppml3lsmr, ols2lsmr, ols2slopelsmr, ols3slope and
  ols2slope at one million rows (four threads, two interleaved pairs of
  three fits) are bitwise identical in b, SE and objective on the same
  routes, with median time ratios 0.96 to 1.01.

## CMG 0.3.0 pin (development, Mac)

fereg-core and fedemean pin the published CMG
[v0.3.0 release](https://github.com/johannes-schmieder/CMG/releases/tag/v0.3.0),
whose annotated tag resolves to `368eceba765db91ff818815698da35889ce42ed0`, in
place of `e5613e81` and the local `[patch]` override that had replaced it in
the root and fuzz manifests since `d3152d88`. The pinned production sources (`src/` outside
`#[cfg(test)]` modules, and the manifest's features and dependencies) equal
the override's `d029341`: the branch-free two-way partitions of the canonical
edge introsort (`b6e75e4`) and team applications of every planned hierarchy
level (`d029341`), both arithmetic-preserving. The other upstream changes are
the R package, documentation, version metadata and two test-only cancellation
witnesses (`856cf74`, `368eceba`). Each lock file changes only in its cmg
entry, which now records version 0.3.0 and the git source.
`licenses/CMG-SOURCE.json` is refreshed from a clean
`cargo vendor --locked --versioned-dirs` under Cargo 1.85.1. The license text
and MIT metadata are unchanged.

- Evidence (Mac, development): the locked dev and release builds, fedemean
  with all features (91 tests), the plugin's 242 FFI tests and its 12
  allocator audits, both Clippy configurations, rustfmt, the locked fuzz
  check and Python 282 (the three numpy-dependent modules cannot be imported
  here) pass. fereg-core's library passes 1,541 of 1,542 tests. The one
  failure, `a_slope_fit_fedemean_does_not_certify_continues_with_map`, is
  already stale at the base `a59598f6` and is replaced by `5340cbfe`.
  Dependencies compile without `cfg(test)`, so its CMG code equals the
  base's.

## Combined validation (10f4b633)

Integration-boundary validation of the fedemean branch head `10f4b633`
(elimination with escalation, the full system with Schwarz, slopes, PPML LSMR
with the joint Schwarz block, certified fallbacks and the CMG v0.3.0 pin)
against the frozen `71fce764` and the tranche-1 head `f370027f`, on
2026-11-09. Development evidence: nothing is pushed or published.

**Checks (Mac).** `scripts/run_tests.sh --source-only` passes: 2,383 Rust
test results with none failing (fereg-core's library 1,554; the plugin's 243
FFI tests and its 12 allocator audits, each audit in a fresh process), both
Clippy configurations, rustfmt, the locked fuzz check, the portable static
checks and the complexity ratchet. Python ran 282 tests: the three
numpy-dependent modules cannot be imported here and were not run, the rest
pass (13 skipped). The actual-plugin ABI harness passes on the candidate
plugin. Stata fixtures with only the plugin staged: 154 of 163 pass; the nine
failures are the known set (`mata_correction`, `native_splitbranch_block`,
the two PPML allocation audits, `parallel_input_oracle` without its trace
environment, and four installation fixtures), with unchanged signatures.

**Mac M2 Ultra** (1M rows, `x1-x4`, `vce(robust)`, 4 threads, medians in ms;
every timing window recorded no interference). The main set ran 2
interleaved processes x 3 fits, the other cases 1 process x 2 fits. Science is
the candidate against `71fce764` unless noted.

| case | 71fce764 | f370027f | 10f4b633 | science |
|---|---:|---:|---:|---|
| ols2, akm, ols3, ols5, ols3hard | 60.5, 59.5, 80.0, 101.5, 76.0 | 60.0, 59.0, 80.0, 101.5, 76.0 | 61.0, 59.0, 80.5, 102.0, 77.0 | bitwise, all arm pairs |
| ppml2, ppml3, ppml5, ppml3hard, ppmlakm | 398.0, 465.0, 619.0, 592.5, 393.5 | 390.5, 468.5, 612.5, 588.5, 387.0 | 390.5, 464.5, 615.5, 591.0, 392.0 | bitwise, all arm pairs |
| ols3, ppml3 at 1 thread | 184.0, 1195.0 | 182.5, 1189.0 | 185.0, 1197.5 | bitwise, all arm pairs |
| ols3, ppml3 at 8 threads | 56.5, 343.0 | 57.5, 339.5 | 57.0, 345.0 | bitwise, all arm pairs |
| ppml2lsmr, ols2cmgschur | 580.5, 409.5 | 580.5, 411.5 | 582.5, 365.5 | bitwise |
| ols2slope, ols2wslope | 299.0, 256.0 | 288.5, 252.5 | 296.0, 247.0 | bitwise |
| ols6 | 1343.0 | 1339.0 | 348.0 | \|db\| 6.1e-16, rel dSE 1.6e-11 |
| ols6w | 1579.5 | 1570.5 | 472.5 | 4.2e-16, 8.8e-12, rss identical |
| ols7 (Schwarz, 1 escalation) | 2233.0 | 2219.5 | 1408.0 | 5.6e-16, 9.8e-12 |
| ols2lsmr | 632.0 | 628.0 | 213.5 | 2.5e-16, 2.4e-12 |
| ppml3lsmr | 2310.5 | 2290.0 | 878.5 | 2.1e-13, 4.8e-14, deviance identical |
| ols3chain | 448.5 | 242.5 | 236.0 | 2.5e-16, 6.1e-12; bitwise to f370027f |
| ols3slope | 668.5 | 666.5 | 372.5 | 1.9e-16, 6.2e-11 |
| ols2slopelsmr | 1447.5 | 1436.5 | 646.0 | 2.5e-16, 5.7e-12 |
| ols2slopefe | 1500.5 | 1508.0 | 512.5 | b, SE, rss bitwise; saved effects differ by the §5 transfer |
| ols2chain, ppml2chain, ppml3chain | not rerun | 110.5, 674.0, 1281.0 | 107.0, 605.0, 1224.0 | bitwise to f370027f |
| ols3slopefe | not rerun | timeout (> 900 s) | 571.0 | – |

Main-set geomeans: 0.999 against `71fce764` and 1.005 against `f370027f`,
with no case above max(1.03x, +2 ms). The `71fce764` chain timings from the
f118c999 validation on this Mac are 23,613 ms (ols2chain) and 27,545 ms
(ppml2chain); ppml3chain and ols3slopefe exceed 900 s there. The chain fits
here equal f118c999's bit for bit, whose distances to `71fce764` were
|db| 1.9e-11, rel dSE 1.6e-7 and rel rss 1.9e-9 (ols2chain, where the
base's Jacobi recurrence stalls) and |db| 3e-16, rel dSE 9.0e-14 with an
identical deviance (ppml2chain).

**Linux** (SCC scc-wo4, Xeon E5-2680 v4, a whole 28-core node, 4 threads;
medians of 9 fits for the main set, 5 for the many-FE, explicit-LSMR and
slope cases, 3 for the chain cases). The plugins were built on scc-b14 from
offline snapshots (git archive plus `cargo vendor --locked`, Cargo offline);
the candidate's vendored CMG matched the 368eceba pin, its manifests, lock
file and the reviewed source inventory. Both jobs ended with failed=0 and
exit 0, all 235 Stata processes completed with the expected embedded commit,
and every arm repeated exactly.

| case | 71fce764 | f370027f | 10f4b633 | cand/base | science |
|---|---:|---:|---:|---:|---|
| ols2, akm, ols3, ols4 | 443, 425, 533, 556 | 451, 428, 555, 599 | 455, 439, 543, 593 | 1.03, 1.03, 1.02, 1.07 | bitwise, all arm pairs |
| ols5, ols3hard | 617, 528 | 570, 504 | 599, 534 | 0.97, 1.01 | bitwise, all arm pairs |
| ppml2, ppmlakm, ppml3, ppml4 | 2167, 2015, 2556, 2772 | 2084, 1940, 2673, 2831 | 1998, 2051, 2506, 2707 | 0.92, 1.02, 0.98, 0.98 | bitwise, all arm pairs |
| ppml5, ppml3hard | 3084, 2549 | 2960, 2521 | 2948, 2511 | 0.96, 0.99 | bitwise, all arm pairs |
| ols6, ols6w | 5036, 6265 | – | 1641, 2313 | 0.33, 0.37 | as on the Mac |
| ols7 | 7645 | – | 6005 | 0.79 | 3.3e-16, 9.8e-12 |
| ols2lsmr, ppml2lsmr | 2822, 2848 | – | 905, 2668 | 0.32, 0.94 | as on the Mac; ppml2lsmr bitwise |
| ols2slope, ols2wslope | 1424, 1445 | – | 1507, 1332 | 1.06, 0.92 | bitwise |
| ols3slope, ols2slopelsmr, ols2slopefe | 3379, 6875, 4826 | – | 1722, 2542, 2351 | 0.51, 0.37, 0.49 | as on the Mac |
| ols2chain, ols3chain | – | 550, 1113 | 534, 1452 | – | bitwise to f370027f |
| ppml2chain, ppml3chain | – | 3108, 6693 | 3000, 5652 | – | bitwise to f370027f |
| ols3slopefe | – | – | 2695 | – | – |

Main-set geomeans: 0.996 against `71fce764` and 1.000 against `f370027f`.
The base's chain timings from the earlier whole-node run on the same host
are 56,369 ms (ols2chain) and 71,513 ms (ppml2chain).

**Timing signals, kept as measured (not resampled):**
- ols3chain on Linux: 1452 against `f370027f`'s 1113 ms (1.30). All three
  candidate fits took 1367-1458 ms, against 1087, 1587 and 1113 ms; the
  results are bitwise identical, and the Mac shows 0.97. Diagnosed below
  ("ols3chain Linux phase diagnosis"): not reproduced, no change.
- ols2slope on Linux: 1.06 against `71fce764` on the same solver and bits
  (1435-1627 against 1329-1484 ms per fit); the Mac shows 0.99.
- ols4 on Linux: 1.07 against `71fce764`, already present at `f370027f`
  (1.08) and in the f118c999 validation (1.07); 0.99 against `f370027f`.
  Against `f370027f`, ols5, ols3hard and ppmlakm (1.05-1.06) run the same
  arithmetic and sit within 0.97-1.02 of `71fce764`.

**Route changes by design.** Only fits whose route changed differ from
`71fce764`: six and seven effects (fedemean's full system), explicit
`solver(lsmr)` on OLS and three-effect PPML, the chain designs (escalation),
slopes outside the two-effect slope solver and explicit slope LSMR. Against
`tolerance(1e-12)` references, earlier lanes reported rel dSE 8.3e-12
(ols6) and 1.4e-11 (ols7), against `71fce764`'s own 9.6e-12 and 1.5e-11, and
5.9e-11 for ols3slope against the base's 6.3e-11. Saved slope effects follow
the documented SPEC §5 normalization (max |Δ| of the saved-effect means and
standard deviations 2.8e-2 on ols2slopefe, with the fit bitwise).

Not covered: Windows, Intel Macs, installed-package admission, the three
numpy-dependent Python modules and release work.

**ols3chain Linux phase diagnosis** (2026-11-09; the same host, scc-wo4,
whole node, bench job failed=0/exit 0). Nine interleaved rounds, each with one
fresh process per arm (warmup, then one timed fit), all with
`FEREG_PHASE_TIMING=1`. The arms are the byte-identical `f370027f` and
`10f4b633` plugins of the validation run, plus diagnostic builds of both
commits whose CMG local solve counts and times its build and applications
(uncommitted). The candidate's diagnostic build also ran with its lane groups
capped at one (`g1`) and two (`g2`). Medians in ms:

| arm, 4 threads | fit | solve | setup | blocks | PCG | CMG apply (work) |
|---|---:|---:|---:|---:|---:|---:|
| f370027f / 10f4b633 | 1402 / 1311 | 1005 / 926 | 15 / 15 | 51 / 52 | 922 / 848 | – |
| diagnostic f370027f | 1347 | 968 | 14 | 53 | 878 | 50.6 (50.5) |
| diagnostic 10f4b633: 4 groups / g2 / g1 | 1311 / 1316 / 1360 | 938 / 949 / 974 | 14 | 51-52 | 867 / 874 / 900 | 32.7 (71.2) / 41.3 (68.5) / 51.5 (47.9) |

Without phase timing the two plugins took 1427 and 1321 ms; at one thread
1797 and 1782 ms (PCG 902 and 899, CMG apply 44.5 and 45.3). Every arm runs
99 iterations with 20 CMG applications. The CMG build takes 4-6 ms on a
9,999-edge graph, below CMG's planned-edge threshold, so the team apply never
runs. All arms repeat exactly and agree bit for bit at each thread count.
The 1.30 is not reproduced: here the candidate is 0.93 against `f370027f`,
and per-fit spreads of 1245-1579 and 1269-1623 ms show process-to-process
variance of the size the validation's three fits per arm saw. The concurrent
lane groups shorten CMG application by 19 ms per fit on this processor (as
on the Mac, 16.8 to 11.0 ms), and nothing else on the path differs. No
policy change.

## Modified LSMR engine in fedemean (development, Mac)

fedemean carries the modified LSMR engine of the MIT-licensed
`schwarz-precond` crate as `fedemean::lsmr` (`04f63332`): the engine at the
former pin `8d401d82` of the adapted within fork, moved with its unit tests
and upstream notice and without the crate's Schwarz preconditioner, its
arithmetic unchanged (the mechanical edits are listed in the module's
provenance note). fedemean's explicit-LSMR projections then run batches of
up to eight columns in lockstep through the engine's `mlsmr_columns`
(`14d829c8`), developed on the fork after that pin and never published. fereg
has no dependency on the within repository: both lockfiles lose
schwarz-precond, thiserror, thiserror-impl, proc-macro2, quote, syn and
unicode-ident (`Cargo.lock` 32 to 25 packages, `fuzz/Cargo.lock` 37 to 30),
and no locked package resolves from that repository. Modified LSMR no longer
needs fedemean's `schwarz` feature, which covers additive Schwarz and
approx-chol.

Declared scope of `14d829c8`: fedemean's full-system LSMR projections with
more than one column (explicit `solver(lsmr)` on OLS, slope LSMR, PPML's FWL
projections). Each column keeps its own recurrence, stopping tests, restarts,
budget and certificate, and the batch's kernels keep `mlsmr`'s per-column
summation order, so every column returns the bits of its one-column solve.
Independent witnesses: the engine's tests compare every result field of
`mlsmr_columns` with `mlsmr` at widths 1-8 (restarts, refused stops,
breakdowns, indefinite metrics, invalid columns, cancellation, pool widths),
and fedemean's tests compare lockstep projections with one-column projections
(Jacobi, identity, Schwarz, weights, slopes with channel blocks and centered
offsets, a memory-narrowed width). The reported workspace is the widest
batch's; under a tighter limit the batch narrows down to one column, and a
projection declines only when one column does not fit (the plugin allocator
audit `fedemean_lsmr_batches_stay_within_their_reported_workspace` binds the
measured peak to it).

**Column order (`ffe95897`).** Lane Q1's fixture run found that
`14d829c8` kept every certified column's bits but not the per-column
route's outcome where columns interact: on the offset-slope boundary
fixture's case 2 a declining batch charged 72 iterations where one column
at a time charged 21, so `iterate(100)` was spent and the fit failed r(430)
instead of its MAP fallback's r(459). A setup decline also returned before
the earlier columns ran, the batch shared one slow flag, and after a
Jacobi pilot the first column to hand off took the round's others with it.
Now a hand-off or decline is resolved only once every earlier column is
solved: a decline reports the first declining column in column order with
the iterations it and its predecessors spent, the columns before it
written and their input state; the first hand-off escalates and later
columns that worked on Jacobi start again on Schwarz. Witnesses
(`crates/fedemean/tests/lsmr_order.rs`, each failing on `14d829c8`): the
fixture's case 2 at two tolerances and five budgets declines exactly as one
column at a time, a mid-batch setup decline charges and writes the columns
before it and hands the same remaining budget on, and a later escalation
gives each column its one-column bits. The fixture passes its four cases
again (case 2 r(459), case 3 r(430), at 1 and 4 workers); fedemean's tests
pass in all four feature combinations, fereg-core's library (1,555), the
plugin's 243 FFI tests and 13 allocator audits, both Clippy configurations,
rustfmt and the ratchet pass. On the gated Mac
(no interference; 2 processes x 3 fits) ols6lsmr, ols2slopelsmr, ols2lsmr,
ppml2lsmr and ppml3lsmr are bitwise identical to `14d829c8`; their timing
ratios of 0.93-1.05 are noise on a busier host (both arms ran 13-23% slower
than the earlier window), as the PPML cases show at 1.02-1.03, since their
joint route never reaches this code.

**Checks (Mac).** `scripts/run_tests.sh --source-only` passes with the three
numpy-dependent Python modules not run: Python ran 282 tests (13 skipped; the
three modules cannot import numpy here, and nothing else fails), and the
script's remaining steps pass: 2,413 Rust test results with none failing
(fedemean's library 125, among them the moved engine's tests and the lockstep
tests; fereg-core's library 1,554; the plugin's 243 FFI tests and its 13
allocator audits, each in a fresh process), both Clippy configurations,
rustfmt, the locked fuzz check, the portable static checks and the complexity
ratchet. fedemean's tests also pass without features, and Clippy is clean in
all four fedemean feature combinations. The new batch audit measured peaks of
0.63-0.96 of the reported workspace over 2 and 3 effects and a slope design,
Jacobi and Schwarz, 5 and 11 columns and 1 and 4 workers, including the runs
one byte below the widest batch and at one column's admission. The
actual-plugin ABI harness passes on the `14d829c8` plugin. The offline source
archive (vendored closure, `package_release.py`) builds the plugin with Cargo
offline.

**Mac M2 Ultra** (1M rows, `x1-x4`, `vce(robust)`, 4 threads, medians in ms;
both timing windows recorded no interference). The first comparison ran one
process x 2 fits per arm, the second 2 interleaved processes x 3 fits. Every
case is bitwise identical (b, SE and objective) in both comparisons.

| case | 10f4b633 | 04f63332 | 04f63332 | 14d829c8 |
|---|---:|---:|---:|---:|
| ols6lsmr | 704.5 | 697.5 | 651.5 | 509.0 |
| ols2slopelsmr | 679.5 | 665.5 | 642.5 | 600.5 |
| ols2lsmr | 216.5 | 214.0 | 211.5 | 204.0 |
| ppml2lsmr, ppml3lsmr | 600.5, 906.0 | 594.5, 906.5 | 580.0, 873.0 | 578.5, 872.0 |
| ppml2chainlsmr (`solver(lsmr)`) | 900.0 | 900.5 | 888.0 | 881.5 |
| ols2, ols3, ppml2, ppml3, ols6 | 60.5, 82.5, 399.5, 472.0, 341.0 | 61.0, 80.5, 393.0, 469.5, 340.0 | 58.5, 79.5, 382.0, 462.5, 359.5 | 58.0, 79.0, 384.0, 459.5, 343.5 |

The move leaves every timing within noise. The lockstep batch takes ols6lsmr
to 0.78 and ols2slopelsmr to 0.93 of the one-column route; ols2lsmr (0.96,
ten iterations per column, where fixed per-column passes dominate) and the
PPML cases are flat: PPML's default explicit route is the joint recurrence,
which solves one system per frame. The default cases run unchanged code
(ols6's processes spread 331-365 ms in the first arm).

Not covered: Linux and SCC timing, Windows, Intel Macs, installed-package
admission, the three numpy-dependent Python modules and release work.

## Validation gaps closed (Mac, 1577e7af)

Addendum-3 cleanup on the fedemean branch, 2026-11-09/10. Development
evidence: nothing is pushed, published or written to `dist/`.

**PPML allocation audits (runtime defect, `97d492ab`).** On the exact
elimination route every warm PPML IRLS frame allocated and freed an 8-byte
copy of the elimination's residual norms (`block_schur::Cache::attempt`
attached it to each result), so the allocation audit's 16-iteration window
saw 48 events at one thread and 32 at four, `e(feppml_allocation_cert_passed)`
was 0, and `feppml_joint_categorical_audit`/`feppml_profile_curve_audit`
failed r(9). The defect was present at the frozen `71fce764` (same counts)
and absent at v0.5.3 (0 events). The PPML owner's cache now keeps the norms
and `absorb_with_selected_norms` reads them by borrow, with the attach path's
checks. Scope: allocation only. 24 PPML specifications (two to four effects,
fast and low memory, robust and clustered, weights and an offset, 1 and 4
threads) are bitwise identical in b, V, ll, deviance and iterations to the
pre-fix plugin; both audits pass at 1 thread/pool 1 and 4 threads/pool 2.

**Python environment (`bf9426fd`).** With an interpreter lacking NumPy, SciPy
and pandas, three modules failed to import and 13 tests skipped. Every Python
step of `scripts/run_tests.sh` now uses `FEREG_PYTHON_BIN`, and a preflight
stops the run before any check without those modules. With the existing
Anaconda Python 3.12.2 (NumPy 1.26.4, SciPy 1.13.1, pandas 2.2.2) all 361
Python tests run and pass, none skipped.

**Fixtures (`5651385b`).** The other seven of the nine whole-tree failures
also failed identically with the `71fce764` plugin. `parallel_input_oracle`
needs the solver trace, `staged_plugin_binary` and `feppml_installation_smoke`
an extracted release package, and `windows_ci_smoke` Windows; they now
declare that input (`* fixture-requires:`), and the runner provides the trace
and its digest check or reports the fixture as not run (a failure when named
explicitly; never a pass). The new `staged-install` profile holds the staged
fixtures, and the `development` profile lists every development fixture.
`mata_correction`, `native_splitbranch_block` and
`feppml_platform_load_capture` were inputs of drivers retired in `788b05ea6`,
needed code never in this repository and are removed.

**Installed-package admission (`1577e7af`).** The universal plugin was built
from a clean worktree of `1577e7af` with `scripts/build_macos.sh` and the
remaps of the [Mac build instructions](macos_source_build.md) (sha256
`6214d881df52c865b8f6ec05ab3c822775e94971e8d7df5cc5d47c08e098f46c`,
33,655,968 bytes, x86_64 and arm64): binary audit and actual ABI/selftest/
panic harness pass. `check_github_install.py` was bound to the repository's
`dist/`, which still holds main's runtime and fails here as designed;
`--package-root` checks a candidate root package staged by
`stage_local_install.py --root-package` (macOS only; it also requires the
candidate's Stata files to equal the source's). `scripts/run_tests.sh
--package-root` with that candidate passes: the preflight, 361 Python tests
(none skipped), the package gate on the candidate (34 files), 2,498 Rust test
results with none failing (fereg-core's library 1,555; the plugin's 243 FFI
tests and its 13 allocator audits, each in a fresh process), both Clippy
configurations, rustfmt, the locked fuzz check, the ratchet and the
portable, static and mock-SPI checks; the Linux mock-plugin audit needs a GNU
ELF linker and is not run on the Mac. A `net install`
of the candidate into isolated PLUS/PERSONAL/OLDPLACE/SITE directories
installs 34 files byte for byte; the plugin resolves from that PLUS, selftest
reports commit `1577e7af5fac`, independent comparisons give 1.2e-15 (OLS
against `regress` with dummies) and 1.1e-11 (PPML against `poisson`), and 11
installation and postestimation fixtures pass against the installed copy.
The macOS-only release archive (`stage_release.py --platforms macos`, sha256
`cb751f1d01ab0bf2da0a0b133db8a50a5b7721b84b6056bdabf9210df5ed87cc`;
dependency audit 0 advisories) passes all 12 `staged-install` fixtures.
Staging repair: the archive manifest now names the OLS result wire 1.7 and
PPML request wire 1.10 the runtime speaks (it still said 1.5 and 1.8). The
PPML Mata objects match their Stata 15 receipt; the compiled poster, which
cannot be rebuilt without Stata 15, still posts `e(feppml_linear_route)` and
`e(feppml_solver_fallback_count)`. This is a known reporting limitation and
does not block admission.

**Whole-tree Stata run** (the candidate plugin staged alone, Stata/MP 19):
160 fixtures, 156 pass, 1 fails and 3 are not run: `staged_plugin_binary` and
`feppml_installation_smoke` (they pass in the staged-install run above) and
`windows_ci_smoke` (the Windows qualification runs it).
`parallel_input_oracle` passes its trace check (72 cases, 208 runs, 73 in the
numeric window). The failure is new, not one of the nine:
`ols_offset_slope_boundary` case 2 (explicit slope `lsmr,
precondition(none)`, `iterate(100)`) now fails r(430) where it expects r(459).
It enters at `14d829c8`, the lockstep multi-column LSMR (`04f63332` passes;
that lane ran no Stata fixtures). The fit's second projection declines; the
column-by-column solve stopped at its first failing column and charged 21
iterations, while the batch charges the 72 iterations a later column spent in
lockstep, so 28 + 72 exhaust the budget and the certified MAP fallback no
longer runs. Cases 1, 3 and 4 pass. Neither the batch's accounting nor the
fixture is changed here: charging a decline only through its first failing
column, or declaring the new outcome as was done for case 3, needs a decision.

Not covered: Linux and Windows builds and qualification, Intel Macs, the
corresponding-source archive (the candidate's notices omit the vendored
license texts), HTTP installation and release work.

## Final Mac qualification (899fcf20)

The final-source Mac check of Addendum 3, 2026-11-10, on `899fcf20` (runtime
change `ffe95897` since `1577e7af`). Development evidence: nothing is pushed,
published or written to `dist/`.

**Candidate.** Built from a clean worktree of `899fcf20` with
`scripts/build_macos.sh` and the remaps of the
[Mac build instructions](macos_source_build.md) (Rust 1.85.1, offline):
universal x86_64/arm64 plugin, sha256
`567b1cb184221806bbb097f6186fd441ee186968ad33d1e788e00dfbcd923927`,
33,655,552 bytes, commit string `899fcf209518`. The binary audit and the
actual ABI/selftest/panic harness pass. It was staged with
`stage_local_install.py --root-package --platform macos`.

**`scripts/run_tests.sh --package-root`** (Anaconda Python 3.12.2 via
`FEREG_PYTHON_BIN`) passes. That covers the environment preflight, 361 Python
tests with none skipped, and the package gate on the candidate (34 files,
runtime bound to `899fcf209518`). It covers 2,502 Rust test results with none
failing: fereg-core's library 1,555, the plugin's 243 FFI tests, and its 13
allocator audits, each in a fresh process. Both Clippy configurations,
rustfmt, the locked fuzz check, the ratchet, and the portable, static and
mock-SPI checks pass too. The Linux mock-plugin audit needs a GNU ELF linker
and is not run on the Mac.

**Whole-tree Stata run** (Stata/MP 19, candidate plugin staged alone): of 160
fixtures, 157 pass, 0 fail and 3 are not run. The three are
`staged_plugin_binary` and `feppml_installation_smoke`, which need an
extracted release package and pass below, and `windows_ci_smoke`, which needs
Windows. `ols_offset_slope_boundary` passes all four cases: case 2 gives
r(459) and case 3 r(430), each at 1 and 4 workers with rollback.
`parallel_input_oracle` passes its trace check (72 cases, 208 runs, 73 in the
numeric window).

**Installed package.** The macOS-only release archive passes all 12
`staged-install` fixtures on Stata/MP 19. It was made with
`stage_release.py --platforms macos` (sha256
`32e95e4546f2d972f4b802464ce5e6d350583cbbc9ed53938e3764e6cd4b1455`), and its
dependency audit found 0 advisories. A `net install` of the staged root into
isolated PLUS/PERSONAL/OLDPLACE/SITE directories installs the package's 34
files byte for byte and nothing else. `fereg.ado`, `feppml.ado` and the
plugin resolve from that PLUS, and selftest reports `899fcf209518`. The
independent comparisons give 9.0e-16 (OLS b and SE against `regress` with
dummies for two effects) and 1.4e-12 (PPML b against `poisson` with dummies).
Eleven installation and postestimation fixtures pass against the installed
copy alone, and the installed bytes are unchanged afterwards. They are the 10
of the `staged-install` profile that need no extracted package, plus
`staged_plugin_binary` pointed at the installed directory. The `older-stata`
profile (`replay_and_prefixes`) passes on Stata/MP 18. Stata 15 and 17 do not
start on this Mac, so Linux covers Stata 15.1.

Not covered here: Linux and Windows qualification (separate lanes), Intel
Macs, the corresponding-source archive, HTTP installation and release work.

## Windows qualification (899fcf20)

The Windows check of Addendum 3, 2026-11-10 (UTC), on the exact clean commit
`899fcf209518cdff0eb521b36f27aef178658980`, sent from a clean detached
worktree through the accepted private-machine wrapper (`windows_ci.sh`,
sha256 `9ee64b16499780cb2a044c466ca2da6d9f5dc40b721bb7224aaae8130bd3b1ef`;
controller `e88ed0308472dc87d8767915dde7532a105891645cdfe7593202d5b92a75a09e`;
diagnostic guard
`14fe3f6515439c55d0732e99fba797280b76dfd9638345794a639a00a44ffd12`; the
accepted pins). Development evidence: nothing is pushed, published or
written to `dist/`. `inspect` reported the restricted
roles-anywhere profile and a stopped instance. Both runs held the shared lock,
ran as SYSTEM on Windows Server 2022 x86-64 with Stata/MP 19, and passed every
controller check (execution identity, source hash and paths, source manifest,
shutdown backstop, Stata, project test). Each finished with the instance
stopped, its transient transfer objects deleted and the lock released.

| Run | Profile | Source archive sha256 | Receipt sha256 | Result |
| --- | --- | --- | --- | --- |
| `win-20261110T041936Z-accd7ed2` | `stata-smoke` | `3c4500aded8c4d7f566c02b573f5409539971a2e0195230b3b46be9106c504a5` | `c902a9d6aedb9a7169cd967d530071e57870b08e901496add5b84b9432d6ae2c` | PASS |
| `win-20261110T042144Z-3aff94ad` | `stata-do` | `2c88562b2361791540359c2c644ab3b3c515c8c7287121bcafd6a01fbdf80f86` | `bcb5fd0fc9dda492e4791bdddd05f56f1c565748d7209a1543422402d7a1a8e2` | PASS |

The two archives hold the same deterministic `git archive` of the commit plus
the wrapper's per-run source manifest, whose zip timestamp differs. Each
receipt binds its own archive hash, which the controller rechecked before
extraction.

**Full profile** (`windows-ci.do` → `scripts/windows_ci_runtime.do`,
1,304 s of the 1,800 s fereg allowance; final stage `complete`; the status
file gives `WINDOWS_CI=PASS` only after the driver's last assertion). The
Rust/MSVC release build (static CRT), the exact-FMA and x86-64 fused-kernel tests on the
FMA/AVX hardware path, the C bridge (`/W4 /WX`) and numeric batch guards, the
plugin link with embedded commit and build date, the PE binary audit and the
actual native loader all pass. The plugin is checksum-verified, staged with the
Windows release file set (`stage_local_install.py --platform windows`) and
`net install`ed into isolated PLUS/PERSONAL; the installed copy has the same
hash. The installed package runs `windows_ci_smoke` (selftest names
`fereg_windows.plugin`, a robust one-effect fit, `observation_stores`),
`single_runtime`, `ols_solver_reporting`, `ols_cmg_raw_residual_oracle`,
`ols_mixed_solver_reports`, `feppml_one_fe_dummy_oracle`,
`within_weighted_ols`, `ols_three_fe_batch_oracle`, `lsmr_batch_oracle`,
`ols_offset_slope_boundary`, `replay_and_prefixes`, the transaction fixtures
`estimate_ols_eclass_transaction` and `estimate_ppml_hidden_transaction`,
`ols_no_fe_direct`, `ols_no_fe_centered` and `feppml_saved_map_pilot`. From
`ols_offset_slope_boundary` on, the source ado directory, byte-identical to
the installed one, is also first on the adopath. The host is Stata/MP 19
GUI; the missing-value witness, `parallel_input_failures` and the nested
input oracle at the default and eight workers pass. The oracle
gives 72 cases, 208 runs, at least 72 in the numeric window, and exact
serial/parallel input digests. PPML saved-intercept recovery passes at both
worker counts, with at least four traced CG recoveries each. The guarded
collector returned the tested plugin, sha256
`67a4eb9b38736e9ae6e8c542a1fcd37ac9660f20a00fed4d5c9c33b47be0e61a`
(20,388,352 bytes), and the download matched the receipt. Locally it is an
x86-64 PE DLL carrying `899fcf209518` and the build date 2026-11-10. The
project's PE audit needs `dumpbin`, so it ran on Windows only.

Not covered: the `staged-install` profile on a sealed Windows release
archive. Its runner starts Stata with Unix `-b do`, `stage_release.py` needs a
git checkout and a dependency-audit receipt, and `staged_plugin_binary`
deletes the loaded plugin, which Windows blocks. The Windows profile runs
three of that profile's fixtures against the installed copy (`single_runtime`,
`observation_stores`, `replay_and_prefixes`). Also not covered: the
`cargo-test` workspace profile (not part of the project's Windows
qualification), live Break during Windows loading (waived 2026-11-07), Windows
performance, and HTTP installation and release work.

## Final Linux qualification (899fcf20)

The final-source Linux check of Addendum 3, 2026-11-09/10, on SCC (run
`run-20261109q`). Development evidence: nothing is pushed, published or
written to `dist/`. The input was an offline snapshot of the exact clean
commit `899fcf209518cdff0eb521b36f27aef178658980`: a shallow git checkout,
so the staging tools see a clean source identity, and the
`cargo vendor --locked --versioned-dirs` closure of the workspace and fuzz
lock files. The emitted source replacement sits in the parent directory's
`.cargo/config.toml`, outside the checkout, so tracked files stay unchanged.
Cargo ran offline with an empty `CARGO_HOME` and fetched nothing. The vendored
CMG matches the reviewed 368eceba inventory (`licenses/CMG-SOURCE.json`,
126 files), and the build compiled it from that pin.

| Job | Host | Work | SGE |
| --- | --- | --- | --- |
| 8547136 | scc-tb4 (E5-2660 v3) | build and staging | failed=0, exit 0 |
| 8547137 | scc-tb4 | `run_tests.sh --package-root` | failed=0, exit 101 |
| 8547138 | scc-va2 (E5-2680 v4) | licensed Stata | failed=0, exit 1 |
| 8547445, 8547443, 8547456 | scc-tb4, scc-tb4, scc-pi2 | diagnosis | failed=0, exit 0 |

Diagnosis attempts 8547397 and 8547398 were stopped by defects in their own
scripts (a SIGPIPE in a version probe and a Stata log-name collision) and
were rerun as 8547445 and 8547443. 8547442 was deleted before it started,
to fix the witness's input order.

**Build.** `scripts/build_linux.sh` (Rust 1.85.1, GCC 12.2): the ELF audit,
the actual-plugin ABI/self-test harness and the numeric batch guard pass. The
plugin has sha256
`5f0467c4ed98259bd439e7180822cdfb75cd6bcae53e1b511485cd61aa6155e4`, is
23,977,576 bytes and carries `899fcf209518`. Staged from a second, pristine
checkout:
- the candidate root package (`stage_local_install.py --root-package
  --platform linux`);
- the Linux-only release archive (`stage_release.py --platforms linux`, sha256
  `486770345cf5f9017e8006582050a9c374c71a7124d211ab4668579154a77bb5`). Its
  dependency audit (cargo-audit 0.22.1, run on the Mac for the same commit and
  `Cargo.lock`) reports 0 advisories.

**`scripts/run_tests.sh --package-root`**, with the SCC Python 3.12.4 module
(NumPy 1.26.4, SciPy 1.14.0, pandas 2.2.2), fails: one Rust test fails.
Everything before the Rust tests passes:
- the preflight and all 361 Python tests, none skipped;
- the package gate on the candidate (34 files, bound to `899fcf209518`);
- the exact-FMA vectors and both exact-rational FE witnesses;
- rustfmt, the locked fuzz check and both Clippy configurations.

The entry point stops at the first failing test binary, so the remaining steps
were also run to completion. This is a diagnostic run, not the strict gate:
- 2,497 Rust results, of which 2,496 pass and 1 fails: 2,235 workspace results
  with 5 ignored (fereg-core's library: 1,554 pass, 1 fails, 3 ignored), the
  plugin's 249 FFI tests, and 13 allocator audits, each in a fresh process;
- the portable and static checks (including the GCC analyzer), the ratchet, the
  mock-SPI bridge, the numeric batch guard and the Linux mock-plugin audit all
  pass.

**Licensed Stata.**
- **Whole tree** (Stata/MP 19, plugin staged alone): of 160 fixtures, 155 pass,
  2 fail and 3 are not run. `staged_plugin_binary` and
  `feppml_installation_smoke` need an extracted package and pass in the next
  run; `windows_ci_smoke` needs Windows. `parallel_input_oracle` passes its
  trace check (72 cases, 208 runs, 73 in the numeric window).
- **`staged-install`** on the Linux archive: 12 of 12 pass.
- **`older-stata`** on Stata/MP 15.1 failed (item 1 below) and passes after the
  repair. SCC provides Stata/MP 15 through 19.

**Failures.**
1. **`replay_and_prefixes` on Stata 15.1: a fixture defect, repaired.** Since
   `04bc478d`, the display check has expected Stata 17's header
   `[90% conf. interval]`, but Stata 15 and 16 print `[90% Conf. Interval]`.
   The check now expects the running release's exact header. The repaired
   fixture passes on Stata/MP 15.1, 16.1, 17, 18 and 19 with the candidate
   plugin.
2. **`frozen_session_uses_standardized_qr_fallback_for_clear_weak_full_rank`
   (fereg-core), Linux only; it passes on the Mac.**
   - One bread entry differs between the retained and the one-shot route by
     2.5e-12 relative (62,545.7295181558 vs 62,545.7295183098), against a
     tolerance of 2e-12.
   - The design is weak by construction (x2 = x1 ± 5e-4). κ(X′X) ≈ 5.5e9, so
     κu ≈ 6e-7, far above the observed difference.
   - Both routes' Givens rotations and QR norms use libm `hypot`, whose results
     differ by platform. On a fixed set of 10^6 pairs, glibc 2.28 and macOS
     libm give different results for 108,640 pairs. They return a value that is
     not correctly rounded in 111,518 and 147,960 cases respectively (an exact
     rational check).
   - **Not changed.** The tolerance is a scientific assertion. Scaling it by
     the condition proxy, as the same helper already does for the condition
     proxy itself, needs Johannes's decision.
3. **`ols_offset_slope_boundary` case 3 gives r(459), not r(430), at 1 and 4
   workers.** Cases 1, 2 and 4 pass.
   - The trace shows explicit LSMR certifying the first projection in 42
     iterations and declining the second after 42. Only 84 of iterate(100)
     are charged, so the MAP fallback runs and fails its check.
     `iterate(1000)` charges the same 84.
   - The Mac and Windows give r(430): LSMR spends the whole budget.
   - fedemean's LSMR recurrence builds its Givens rotations with `f64::hypot`.
   - The 10f4b633 Linux plugin gives the same outcome, so this predates Lane E
     and `ffe95897`.
   - **Open.** This is a platform-dependent discrete outcome. Either it is
     declared, or the recurrence's `hypot` is made platform-independent (an
     arithmetic change); either needs a decision.
4. **`ols_weighted_schur` rank fallback with x17 = 2·x1.**
   - Linux omits x1 (status 4) and keeps x17 (b = 0.49915). `regress`, the Mac
     and the fixture keep x1 and omit x17.
   - The rank (16), `df_r`, x18 (status 3), the route and the fallback flag
     all agree, at 1, 4 and 16 workers.
   - The 10f4b633 and the frozen `71fce764` Linux plugins choose the same
     column, so the Linux behaviour is long-standing.
   - The two columns tie in exact arithmetic, and the pivot rule prefers the
     earlier column only within a few ulps. Rounding in the column norms most
     likely decides the tie on Linux.
   - **Not fixed.** Changing the omission behaviour needs Johannes's decision.

Not covered on Linux: a `net install` of the candidate root package, the
interactive Break fixtures, timing, and HTTP installation and release work.
The regenerable source snapshot and build trees were removed after the jobs;
the packages, logs and summaries remain on SCC.

## Portable hypot and pivot ties (44cd1563)

Johannes's decisions of 2026-11-10 on the three Linux-only items of the
`899fcf20` qualification, implemented in `3dda12e6` (portable hypot),
`ece57da3` (pivot ties) and `44cd1563` (SPEC, help and guide), the
qualified commit. `c999f6e3` (a doc-comment correction) and `19b88fdb` (the
ratchet's line positions) follow; `check_complexity.py --compare-production
44cd1563` passes on them, so the qualification below applies. Development
evidence: nothing is pushed, published or written to `dist/`.

**Declared arithmetic scope, before qualification.**
- *Portable hypot* (SPEC §18). `fedemean::numerics::hypot` replaces the C
  library's `hypot` at seven sites: fedemean's modified-LSMR recurrence (its
  Givens rotations, residual estimate and `‖A‖` estimate) and fereg-core's
  retained QR (streamed Givens rotations, trailing column norms, dropped-span
  certificate) and dropped-constant certificate. `git grep hypot` leaves the C
  library's only in independent test references. Affected: explicit
  `solver(lsmr)` (OLS, slopes, PPML joint and FWL), PPML's LSMR checkpoint
  accelerator and separation re-execution, owned Auto's LSMR attempt and every
  fit whose finite rank the QR decides. The §5 recovery and §16 certificate
  norms do not use `hypot` and are unaffected.
- *Pivot ties* (SPEC §8). Only the choice among QR pivot candidates within
  `max(64, N + K)` ulps of the largest trailing norm; rank, DoF and the other
  omission rules unchanged.

**The function.** Built from IEEE operations only (exact power-of-two
scaling, Dekker's exact squares with Veltkamp splitting, the correctly
rounded square root, one Newton correction); no math library, no FMA.
- An exact integer check of 211,502 random and adversarial pairs (exponent
  gaps 0 to 64 over the whole range, the shortcut boundary, nearly equal
  arguments, results at binade boundaries, subnormal and near-overflow pairs,
  scaled Pythagorean triples) finds every result within one ulp of correct
  rounding and every normal result correctly rounded; 1,609 subnormal results
  are one ulp off from their second rounding.
- Exact triples, IEEE special values and symmetry are tested.
- Golden digest of 10^6 pairs: `0x128efb568e0c0b75`, identical in Mac debug
  and release builds. The Linux run and the Windows exact-arithmetic stage
  (newly added to `build_windows.ps1`) must reproduce it.

**Default routes stay bitwise** (Mac, 1M rows, 4 threads, against an arm64
build of `ffe95897`, whose runtime is that of `842a0632`; one pair, one timed
fit each through the timing gate, no interference; the candidate was the
uncommitted arm64 build of these changes; timings are not evaluated).
Bitwise b, SE and objective: ols2, ols3, ols5, ols6, akm, ppml2, ppml3,
ppmlakm, ols2slope, ols3slope and ols2slopefe (saved effects too).

**Changed at rounding level, as declared** (same runs):

| case | \|Δb\| | rel ΔSE | objective |
| --- | --- | --- | --- |
| ols2lsmr | 5.6e-17 | bitwise | rel Δrss 2.2e-16 |
| ols6lsmr | 2.8e-17 | bitwise | bitwise |
| ppml2lsmr | bitwise | 6.7e-16 | bitwise |
| ppml3lsmr | 6.9e-18 | 5.6e-16 | bitwise |
| ols2slopelsmr | bitwise | bitwise | bitwise |

Default `solver(auto)` fits whose finite rank the QR decides (the
`ols_weighted_schur` design, 16,512 weighted rows, traced
`finite_factor|method=canonical_qr`, at 1 and 16 workers with equal results):
x1–x18 with x17 = 2·x1 |Δb| 1.2e-14, rel ΔSE 2.2e-14, rel Δrss 1.1e-15;
x19 = −3·x1 listed first 3.6e-15, 2.2e-14, 6.7e-16; a weak full-rank pair
x20 = x2 + 2e-3·noise 1.6e-11, 7.1e-11, 2.2e-15 (amplified by its
conditioning). Ranks and every status are unchanged. These sites are kept
because the QR is the kernel of item 1 and has to be platform independent.

**The three items.**
1. **`frozen_session_uses_standardized_qr_fallback_for_clear_weak_full_rank`:
   not resolved; open for Johannes.** With the portable hypot the miss is
   identical on every platform, and it now also fails on the Mac: bread entry
   62,545.7295181251 (one-shot) against 62,545.7295183053 (fresh session),
   rel 2.9e-12 against the fixed 2e-12 (Linux with glibc gave 2.5e-12; the
   macOS C library passed). Cause: both routes feed the same streamed QR, but
   their reference norms of the first column differ by one ulp (one-shot
   147.7836256152893 from its scaled sum of squares; session
   147.78362561528934, the correctly rounded √21,840). On this
   κ(X′X) ≈ 5.5e9 design that moves R₁₁ by 1.4e-12 relative and the bread by
   2.9e-12, against the scientific bound κu ≈ 6e-7. The C library's rounding
   only decided which side of 2e-12 it fell. The hypot change cannot resolve
   it, and no tolerance was changed. Options: (a) scale this witness's bread
   comparison by the condition proxy, as its helper already does for the
   proxy itself; (b) one reference-norm computation for both routes, an
   arithmetic change of every fit's standardization in the last bit.
   **Item 1 resolved: condition-scaled witness tolerance, Johannes
   2026-11-10** (option (a), test-only): the witness compares its routes'
   breads at `max(2e-12, ε·proxy²)` (here 3.0e-7, against the observed
   2.9e-12) and rejects a bread perturbed by ten times that bound; runtime
   production is identical to `44cd1563`.
2. **`ols_offset_slope_boundary` case 3: resolved.** Every platform now gives
   r(459): explicit LSMR certifies the first projection in 30 iterations and
   declines the second after 42, so the MAP fallback runs within iterate(100)
   and fails its check (Mac, at 1 and 4 workers and at iterate(1000); case 2:
   29 + 34, r(459)). The fixture expects r(459) for both cases. Linux traces
   the same 30 + 42 and 29 + 34 iterations at 1 and 4 workers and both
   budgets, and the fixture passes on Mac, Linux and Windows.
3. **`ols_weighted_schur` x1/x17 tie: resolved.** The QR tie band is
   `max(64, N + K)` ulps of the largest standardized column norm (SPEC §8).
   Over 16,512 rows the tied norms were measured up to 80 ulps apart, beyond
   the former 64; the band is 16,529 ulps, at least 64 times below the rank
   threshold. The new kernel test with x17 = c·x1 (three seeds, five
   multipliers, three storage orders) fails on the former rule and passes now;
   the near-tie test keeps the larger norm just outside the band. The fixture
   asserts x1 kept (status 1) and x17 omitted (status 4) against `regress` at
   1 and 16 workers, and x19 = −3·x1 listed first kept by both. It passes on
   Mac, Linux (x17 status 4 at 1, 4 and 16 workers, where Linux formerly
   omitted x1) and Windows, whose profile now runs it.

**Qualification of `44cd1563`.** Item 1 fails on every platform that runs
the Rust tests, so the strict entry point fails on the Mac and Linux at that
test; the remaining steps were run to completion as a diagnostic.
- **Mac.** Universal plugin built from a clean worktree with
  `scripts/build_macos.sh` and the remaps of the
  [Mac build instructions](macos_source_build.md): sha256
  `3ac6da3ab2ba7b9b64cbe28bb2a4cadf46ac3c56d5a828b725e386d0d0f33af3`,
  33,655,648 bytes, `44cd1563ef3b`; binary audit and the actual
  ABI/selftest/panic harness pass; staged with `--root-package`.
  - `run_tests.sh --package-root`: preflight, 361 Python tests (none
    skipped), the package gate (34 files bound to `44cd1563ef3b`), the
    exact-FMA vectors, both exact-rational FE witnesses, rustfmt, the fuzz
    check and both Clippy configurations pass; the Rust tests stop at item 1.
    To completion: 2,509 Rust results, 2,508 pass and 1 fails (item 1):
    workspace 2,239 (fereg-core's library 1,556 pass, 1 fails, 3 ignored),
    the plugin's 262 FFI tests and 13 allocator audits in fresh processes.
    The portable, static and mock-SPI checks, the batch guard and the ratchet
    pass; the Linux mock-plugin audit needs a GNU ELF linker.
  - Whole tree (Stata/MP 19, plugin alone): of 160 fixtures 157 pass, 0
    fail, 3 not run (`staged_plugin_binary` and `feppml_installation_smoke`
    need an extracted package and pass below; `windows_ci_smoke`).
  - `staged-install` on the macOS archive (sha256
    `562d7c3a035c80d7090c45ea3d107cef50b4391b340a72c62278a2b4e23f5845`,
    dependency audit 0 advisories): 12 of 12. `older-stata`
    (`replay_and_prefixes`) passes on Stata/MP 18.
- **Linux, SCC `run-20261110f1`** (offline snapshot: depth-1 checkout plus
  the `cargo vendor` closure, CMG `368eceba` inventory verified, 126 files;
  manifests checked from the run root before each dependent job).
  - Build 8548431 on scc-gr4 (failed 0, exit 0): `build_linux.sh` passes
    (ELF audit, actual-plugin harness, batch guard); plugin
    `16978964a9c46e8f45a2461923e8e1d7599d1033afe9b9ce2014ce1cd89a55a0`;
    Linux archive
    `3259a9b21524520158765d2e5da5b6f5af016759f009be58075a3e82abc972e5`.
  - Tests 8548432 on scc-gd4 (failed 0, exit 101): preflight, 361 Python
    tests (none skipped), the package gate (34 files, `44cd1563ef3b`), rustfmt,
    fuzz and both Clippy configurations pass; the golden hypot digest
    matches. To completion: 2,523 Rust results, 2,521 pass, 2 fail: workspace
    2,247 (2,240 pass, 2 fail, 5 ignored), FFI 268, 13 audits. Item 1 gives
    the Mac's exact values. The second failure,
    `ppml_lsmr_admission_admits_its_exact_estimate_and_refuses_one_byte_less`,
    occurred in both runs of this job: two consecutive budgeted PPML LSMR fits
    on the same prepared design differed by at most 2.1e-15 in eta, where the
    test requires identical bits. It did not recur in seven reruns: the test
    alone three times on scc-gd4 (8548502), fereg-core's library suite twice
    on scc-gr4 (8548547) and once serially on scc-gd4 (8548568), passing
    each time, as did the parent `842a0632` in the same three suite runs. It
    passes on the Mac. The cause, a test-isolation defect, is identified and
    fixed below (`6cf8cb24`, test-only). The
    static and mock-SPI checks, the
    batch guard and the Linux mock-plugin audit pass. The size check stopped
    at the ratchet's "parent revision unavailable": the depth-1 snapshot has
    no parent commit (a harness defect of this run; the ratchet is
    source-only and passes on the Mac).
  - Stata 8548433 on scc-ei1 (failed 0, exit 0): whole tree 157 pass, 0
    fail, 3 not run; `staged-install` 12 of 12 on the Linux archive;
    `older-stata` passes on Stata/MP 15.1.
- **Windows** (accepted wrapper from a clean detached worktree of the exact
  commit; `inspect` PASS, roles-anywhere, stopped; both runs held the lock,
  passed every controller check and ended with the instance stopped, transfer
  objects deleted and the lock released).
  - `stata-smoke` `win-20261110T090714Z-9482349f`: PASS; archive
    `7357031c3c7ed8cbb1a7a496b04e5fa9f73de49d8cbcd34558dd57616a3b2685`,
    receipt `f267d38eb66debe98c59de1dab27645d0efb10ce755d799c82cbc6d28ff2f877`.
  - `stata-do` `win-20261110T090911Z-18906670`: PASS in 1,355 s (allowance
    1,800 s), final stage `complete`, `WINDOWS_CI=PASS`; archive
    `325dae847ebfd605e18f50018761e549f451513066d1cc80cfcde544aef40da6`,
    receipt `1107c28bacc5585598d5ddc347ac5b382f05e9f5a6ed2f152132ce980d158490`.
    Every stage passes, including the exact-arithmetic stage with the golden
    hypot digest, and the profile's fixtures, among them
    `ols_offset_slope_boundary` (case 3 r(459)) and `ols_weighted_schur`.
    Plugin `e2dbc5af81232073cfae32d10ab44e167ddc99e0019d7dfd35a9aeaccb98308e`,
    20,392,448 bytes, `44cd1563ef3b`; not written to `dist/`.
  - Not covered: the `cargo-test` profile, so item 1 is not run on Windows;
    the `staged-install` profile on a sealed Windows archive and loading
    Break, as before.

Not covered: timing (the benchmark campaign follows on this commit), Intel
Macs, HTTP installation and release work.

## PPML LSMR repeatability witness: test isolation (6cf8cb24)

The Linux-only failure of
`ppml_lsmr_admission_admits_its_exact_estimate_and_refuses_one_byte_less` in
SCC tests job 8548432 (two repeated budgeted PPML LSMR fits differed by up to
2.1e-15 in eta) is a test-isolation defect, not runtime nondeterminism.
`6cf8cb24` changes tests only; `check_complexity.py --compare-production
44cd1563` passes, so runtime bits and the qualification above are unchanged.

**Cause.** `block_schur::disable_for_test()` turns fedemean's elimination off
for the whole test process while its guard lives. Explicit PPML LSMR seeds
from the elimination when it is enabled and from the checkpoint LSMR attempt
when it is not (traced `initial_certificate_screen`, `checkpoint_lsmr_attempt`),
so its eta moves at rounding level with the switch (`FEREG_BLOCK_SCHUR=0` gives
different eta bits at 1 worker). The test compared its repeated fits bitwise
without the shared side of the route lock (`stable_route_for_test()`), so a
disabling test that started or ended between them changed one fit. This also
explains the reruns: the failure needs concurrent tests, and the serial and
single-test reruns had none. Production reads `FEREG_BLOCK_SCHUR` once per
process, so a fit's route cannot change between fits there.

**Reproduction** (Mac, diagnostic builds with per-fit eta hashes, not
committed).
- Switching the elimination off for the second repeated fit fails at the same
  assertion, rel 1.4e-15.
- Beside a diagnostic test that toggles the switch at random 2–41 ms
  intervals through the exclusive route lock, the unfixed test failed in 20
  of 20 runs (rel 1.1e-15 to 1.6e-15); holding the shared lock it passed 20
  of 20 with the undisturbed eta bits.
- Alone (`--exact`, one test thread): every fit's eta bits, at 1 and at 4
  workers, were identical in 121 runs before and 300 runs after the fix (the
  two worker counts agree within the test's 2e-9). Beside the 33 disabling
  tests the unfixed test ran wholly in the disabled state in 5 of 5 runs
  (other bits, still repeated), the fixed one wholly in the enabled state in
  20 of 20; whole-library runs (6 before, 5 after) gave the undisturbed bits.

**Audit of the other tests.** A diagnostic test toggled the switch without
the lock (at 2–41 ms or 0.2–3 ms intervals) during 27 full fereg-core library
runs. Every failure apart from tests that already hold the lock (which the
diagnostic bypasses) and item 1 (whose old tolerance was still in the
diagnostic tree) came from eleven tests that compare fits across
switch-dependent routes, or whose low-memory fits report inconsistent solver
routes when a fit straddles a flip. They now hold the shared lock as well:
`ppml_topology_complete_irls_preserves_science_outer_iterations_and_solver_signatures`,
`canonical_final_preserves_accuracy_on_a_small_dynamic_range_model`,
`low_categorical_geometry_refusal_retains_canonical_projection`,
`low_categorical_joint_three_fe_preserves_truth_nested_rank_and_zero_control`,
`low_three_fe_checkpoint_budget_refusal_preserves_joint_owner_and_science`,
`load_hint_preserves_first_plan_certificates_and_domain_precedence`,
`automatic_family_matches_explicit_solver_and_preserves_requested_identity`,
`profiled_final_products_match_canonical_and_reject_stale_authority`,
`bias_exact_full_system_matches_independent_explicit_design`,
`multipartition_ppml_map_keeps_fused_projection_capacities_stable` and
`gates_unaffected_auto_families_are_exact`. The last six audit runs found no
unprotected failure. The audit is probabilistic; a test too short to meet a
flip can still be unprotected.

**Linux, SCC `run-20261110d3`** (job 8549089 on scc-gd4, Xeon Gold 6242,
the host of the original failures; 8 slots; offline snapshot of `f16ade2c`,
whose runtime is `44cd1563`, plus the diagnostics; the fixed tree has every
lock except the capacity test's):
- Unfixed: alone, 200 of 200 runs gave one set of eta bits at 1 and at 4
  workers. The whole library (default test threads) failed this test in 2 of
  12 runs, at the same assertion with rel 1.3e-15 and 2.1e-15 (the original
  failure's size); the second fit's bits in both are ones the switch toggler
  produces (20 of 20 failures beside it, rel 1.3e-15 to 2.1e-15).
- Fixed: alone 200 of 200, the whole library 12 of 12 and beside the
  toggler 20 of 20 runs passed, all with the unfixed tree's undisturbed bits.
- Accounting: failed 0, exit 0, 5,316 s, maxvmem 20.1 GB. The snapshot and
  binaries were deleted after the run; logs and summaries remain on SCC.

**Checks of `6cf8cb24`** (Mac): fereg-core's library 5 full runs, 1,557 pass,
0 fail, 3 ignored each; the plugin's FFI tests (243 pass) and 13 allocator
audits in fresh processes; rustfmt; both Clippy configurations; the ratchet
and production identity against `44cd1563`.

## November revision campaign on SCC (80b6768e)

The paper's expanded revision campaign (the November 6 protocol: paired
default/tight comparison blocks, package-default inference, raw numerical
deviations, GLFixedEffectModels.jl for PPML and reghdfejl for OLS, Veneto
included) ran once on `80b6768e`, whose runtime is production-identical to the
qualified `44cd1563`. The Linux plugin
(`a21b87b28b4e053f53e9f62360ed8902aa2c0bb3003a882c5d9173964f28a9ec`) was built
from that commit in SCC job 8549582 and staged as a root package. The frozen
manifest (`728beacc…`, 779 allocations, 1,297 blocks, 5,188 timed and 992
diagnostic fits) and policy (`0900bd73…`) reproduce the November 6 run byte for
byte.

- The two native failures that stopped the November 6 production (blocks 194
  and 182, a tight PPML fit and two saved-effect diagnostic refits returning
  r(430)) passed as their frozen blocks in job 8549157 and again in production.
- The smoke (job 8549679) qualified with both native profiles 19 of 19, and the
  production array 8549825 completed 779 of 779 tasks with failed=0 and exit 0
  (175.6 node-hours, 47 hosts). Lifting the task concurrency cap from 8 to 779 at
  16:51 EST on November 10, at Johannes's request, changed scheduling only.
- Every native fit passed: 1,297 default and 1,297 tight, plus 260 and 260
  diagnostic refits. Comparator results keep their 10 command timeouts and 130
  capture errors; nothing was resampled.
- Median default fereg time is 0.572 of the paper's beta.15 evidence over the 63
  matching cells (descriptive, other hosts).

Collection exposed two harness defects, now fixed: production sentinels were
matched by substring (`4a96ea8f`), and the export's capture hash rejected a
package's reported log likelihood of −∞ (`10553433`). The
[campaign record](../bench/results/scc-20261110-revision.md) gives the jobs,
within-block ratios, deviations, sample differences and export hashes.

## Package checks

### Beta.13 parallel input and parity packaging

Beta.13 runtime `ee61833e` combines parallel numeric input on qualified hosts
(action protocol 1.7) with the PyFixest parity work: seeded PPML joint frames,
joint-frame slow mode, PPML frontend passes and host scope, and the one-FE PPML
exact speedups. Loaded values are bit-identical to the serial reader, and the OLS
replay results are bit-identical to beta.12. PPML results differ only in the 12 cases
of the declared seeded-frame replay scope, where every scientific check still passes.
The [native record](../bench/results/beta13-native-20261101.json) binds the
three plugins and checks below.

| Platform | Beta.13 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build offline from the vendored `ee61833e` source archive (`c064aa92...`) with an empty Cargo home and path remaps; binary audit and actual ABI/load/panic checks passed. All 77 replay cases pass against the pre-merge branch plugin `686ab424` (`ae336950...`, itself exact against beta.12) at 1, 2 and 4 workers: the 12 seeded-frame cases differ within their declared scope and no other case differs. All 68 fixtures of the full Stata 19/MP profile, the OLS, PPML and parallel-input Break rollback fixtures, the parallel-input oracle (64 cases, 65 parallel runs, identical trace digests) and its failure injections passed; the Stata 15-syntax replay fixture passed on Stata 18. |
| Linux x86-64 | SCC build job 8387492 and check job 8387493 (both on `scc-tb4`, `failed=0`, `exit_status=0`): ELF audit and actual ABI/load/panic checks passed, all 68 fixtures of the full Stata 19/MP profile passed, the parallel-input oracle (64 cases, identical trace digests), failure injections and live Break passed on Stata 19, and the Stata 15-syntax fixture passed on Stata/MP 15.1, 16.1, 17 and 18. |
| Windows x86-64 | Run `win-20261101T205555Z-e008d473` at `ee61833e` passed the Rust 1.85.1/MSVC build, PE archive-path and source-manifest checks, the native loader and the licensed Stata/MP 19 `stata-do` driver with its ten mapped fixtures; the controller's execution-identity, source-hash and shutdown-backstop checks passed and the instance stopped (receipt `de9156db...`, plugin `66e06389...`). Windows is not a qualified parallel-input host and keeps the serial reader. |

The [package record](../bench/results/beta13-release-package-20261101.json)
binds the five release assets. Packaging source `118c2188` differs from the
offline-built `ee61833e` archive only in its records and generated metadata
(3,498 identical files). The source ZIP is
`533c063325c94d9f0ad1cf8c35c0dfb461f518f7ce718ba8503c49b174ac02d1`; the
binary ZIP is `81086e04c88e0197e5d468628d8c9bba0905f5f00f63e1149a5d57a6a3d1a46d`.
The dependency audit reports no advisories. The exact binary archive passes
all twelve staged-package fixtures on Mac and on Linux (job 8387820). A fresh
local `net install` of the root package on Stata 19 passes the selftest and the
independent OLS/PPML comparisons with `regress` and `poisson` (relative
differences `7e-15` and `7e-11`); Mac Stata 15 no longer starts on the test
Mac, and Linux runs the Stata 15.1 checks. The strict default
`./scripts/run_tests.sh` passes with the refreshed plugins.

Beta.13 was published with archives only, from tag commit `87a5a4e7`, to
which `main` was fast-forwarded; the release list is 189,940 characters. Fresh
installations from the beta.13 tag, from `main` and with `github install
johannes-schmieder/fereg` pass on Mac in Stata 19 and on Linux (job 8387946)
in Stata 19 and 15.1: 36 of 36 installed files byte-identical, selftest and
the OLS/PPML checks, with `github list` recording beta.13 as installed and
latest. See the [publication record](../bench/results/beta13-publication-20261101.json).

### Beta.12 interface packaging

Beta.12 changes the Stata interface only; runtime `864960f1` keeps beta.11's
arithmetic, routing, iteration budgets and certificates. It lowers the minimum
release to Stata 15, adds replay (so `estimates replay`, `bootstrap:` and
`jackknife:` work), posts `e(fereg_version)`, states the passed accuracy check
in the OLS header, quiets saved-output placeholders and replaces the failed
score-certificate message with one that names the loose `tolerance()`.
The [native record](../bench/results/beta12-native-20261031.json) binds the
three plugins and checks below.

| Platform | Beta.12 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build offline from the vendored `864960f1` source archive (`b0f6aaf4...`) with an empty Cargo home and path remaps; binary audit and actual ABI/load/panic checks passed. All 77 replay cases are exact against the beta.11 Mac plugin (`dfb50318...`), with the same three repaired reference defects. All 68 fixtures of the full Stata 19/MP profile and both Break rollback fixtures passed; the Stata 15-syntax replay fixture passed on Stata 15.1 and 18. Scratch copies of the 67 earlier full-profile fixtures, with `version 15.0` and Stata 15 matrix syntax in their assertions only, passed 66 of 67 on Stata 15.1; the remaining fixture's own `poisson` oracle rejects negative factor levels in Stata 15, after its `fereg` fits passed. The local Stata 17 license has expired. |
| Linux x86-64 | SCC build job 8370549 and check job 8370550 (both `failed=0`, `exit_status=0`): ELF audit and actual ABI/load/panic checks passed, all 68 fixtures of the full Stata 19/MP profile passed, and the Stata 15-syntax fixture passed on Stata/MP 15.1, 16.1, 17 and 18. |
| Windows x86-64 | Run `win-20261031T095017Z-ec6e9c7d` at `864960f1` passed the Rust 1.85.1/MSVC build, PE archive-path and source-manifest checks, the native loader and the licensed Stata/MP 19 `stata-do` driver, whose ten mapped fixtures now include the Stata 15-syntax replay fixture; the controller's execution-identity, source-hash and shutdown-backstop checks passed and the instance stopped (receipt `4eb0539b...`, plugin `5f97813d...`). |

Estimates are the same across Stata releases: a feature check on Stata 15.1
and 19 agrees to `4e-16` relative difference, and 26 OLS/PPML statistics are
bitwise equal to beta.11.

The [package record](../bench/results/beta12-release-package-20261031.json)
binds all 50 release assets. Packaging source `8bd119ad` differs from the
offline-built `864960f1` archive only in its records and generated metadata
(3,471 identical files). The source ZIP is
`a4717974377419e6bcd7828441176cc08a32ecfb06c8995a9bbe38c811bb034b`; the
binary ZIP is `cf508943de83bc5cb233becde22eb68ace6e4b518817fe0aee5a9100a7e232fd`.
The dependency audit reports no advisories. The exact binary archive passes
all twelve staged-package fixtures (the eleven earlier ones plus the replay
fixture) on Mac and on Linux (job 8370604). Fresh local `net install` sessions
of the root package on Stata 19 and Stata 15.1 verify all 35 installed files,
the selftest and independent OLS/PPML comparisons with `regress` and `poisson`
(relative differences `7e-15` and `7e-11`). The strict default
`./scripts/run_tests.sh` passes with the refreshed plugins.

Beta.12 was published with archives only (binary and source ZIPs, checksums
and Mac build notes). The community `github` helper had failed since beta.11
because it reads the whole release list as one Stata line, truncated at
`c(macrolen)` (645,200 characters on SE/MP, 264,392 on BE); per-file assets had
made that list 906,445 characters. With Johannes's approval the 479 per-file
assets of beta.1 to beta.11 were removed after each was verified byte-identical
to its copy in that release's binary ZIP; the list is now 148,286 characters.
Versioned installations use the release tag, whose `dist/` carries exactly the
released plugins from beta.2 onward. Fresh installations from the beta.12 tag,
from `main` and with `github install johannes-schmieder/fereg` pass on Mac in
Stata 19 and 15.1 (35 of 35 files byte-identical, selftest, OLS/PPML checks);
see the [publication record](../bench/results/beta12-publication-20261031.json)
for these and the Linux installations.

### Beta.11 migration packaging

Beta.11 retains the selected migration runtime `65500be6` and within pin
`8d401d82`, including counts, gather, Fold and sparse back-substitution.
No optimization or scientific tolerance changed during release qualification.
The large companion panel passes all 36 native and eight memory cells on each
of Mac and Linux. The accepted eight-/sixteen-FE regressions and the inconclusive
performance settings above remain part of the release decision.

The [Mac release record](../bench/results/beta11-macos-release-20261030.json)
passes all 77 exact replay cases (693 native calls across one, two, four and
eight workers), all 67 licensed Stata fixtures, native ABI/load/panic checks
and the universal binary audit. Its plugin was built with Rust 1.85.1 offline
from the vendored `6c2231c2` source archive and an empty Cargo home.
The [Windows release record](../bench/results/beta11-windows-release-20261030.json)
passes the MSVC build, numeric and native-loader checks, actual local package
installation and ten mapped Stata fixtures at the same source revision.
Two preceding Windows failures are retained: a diagnostic-only driver localized
the failure to the slope fixture's source-location assertion, and using an
absolute ado path fixed the harness without changing the fixture or runtime.
The qualified Linux plugin retains its original `65500be6` build and unchanged
native, replay, Break/rollback and large-companion evidence.

All production and dependency inputs in packaging source `b58474bc` match the
offline-built source. The archive comparison checks every file: its differences
are documentation, completed validation records and generated metadata only.
The source ZIP is `b8b6f33e3f1ad581658bd6748805a73afd40a2309ca26736551ac5105eadd558`;
the binary ZIP is `5c4e551877efc0f023cf3de81de54cec87334a6f18a707e70daeff9ba60b300e`.
The dependency audit reports no advisories. The strict default
`./scripts/run_tests.sh` passes with all three refreshed native plugins,
including the installed-package binding, complexity, FFI and allocator checks.
The current [native metadata](../dist/fereg-native.json) binds each actual
build commit, plugin checksum and the corresponding-source archive.

The [package qualification record](../bench/results/beta11-release-package-20261030.json)
binds all 50 release assets and the completed checks. The exact binary archive
passes all eleven staged-package fixtures on both Mac and Linux. Fresh local
installations verify all 35 main-package files on Mac and all 34 flat-package
files on Linux, native selftest and independent OLS/PPML examples. Linux job
8362394 on `scc-222` has successful terminal accounting, 15 seconds wall time
and 2.804 GB maximum virtual memory. Its 32-slot request covers the existing
`threads(32)` installation test; no host or processor was selected. The job
waited about 42 minutes before execution. Switching the queued job to the
general `omp` environment and requesting a scheduler reservation changed no
resource quantity, test, input or limit.

Native Intel-host Stata execution remains unqualified; both Mac processor
slices are supplied. Existing source and native releases remain immutable.

The [public verification record](../bench/results/beta11-publication-20261030.json)
confirms all 50 published asset digests and sizes and fresh Stata 19/MP
installations from the versioned beta.11 URL and raw GitHub `main` URL.
All 34/35 installed files match, native selftest passes, and independent OLS
and PPML coefficient checks pass (relative differences `3.17e-14` and
`1.12e-10`). The OLS covariance check also passes; PPML covariance is covered
by the maintained independent-sandwich fixtures.

The actual community `github install` command is a documented limitation:
it downloads all 35 correct files but then returns `r(132)` inside its
release-metadata parser, before the smoke estimates. A standalone query
reproduces that error; the official current helper is identical. A bounded
instrumented offline diagnostic returns a different parser error (`r(133)`)
and does not establish the exact cause. Neither failure is relabeled as a
successful command. Two earlier attempts stopped before installation because
the isolated bootstrap omitted the helper's `PLUS/g` directory; both are
retained, including the first unsuccessful repair command. The tested built-in
`net install` route is now the primary installation instruction. This changes
no runtime, package file, scientific assertion or published asset.

### Beta.10 integration packaging

Beta.10 ships the integration round described in
[Beta.10 integration qualification](#beta10-integration-qualification). The
qualified Linux binary is the `3348fe45` SCC build (job 8313475), Windows was
built at `7674cf06` (run `win-20261028T183336Z-bc5a4e6b`), and universal macOS was built offline from
the corresponding-source archive at `7674cf06`. Runtime production inputs are
unchanged across these revisions (`scripts/check_complexity.py
--compare-production 3348fe45`: 225 runtime modules, 58 artifact inputs). The
[beta.10 package metadata](https://github.com/johannes-schmieder/fereg/blob/v0.5.0-beta.10/dist/fereg-native.json)
records each actual build commit, binary checksum, runtime identity and
corresponding-source hash.

| Platform | Beta.10 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. Replay of the archive-built plugin (`aa453ba7...`) against the qualified Mac plugin (`3e8f88de...`) is exact on all 77 cases. All 67 fixtures of the full Stata 19/MP profile passed (69 of 69 profile steps) on a fresh universal build at `7674cf06` with the path-remap environment. All 11 staged-package fixtures passed from the exact binary archive (`1a4de469...`) in a fresh Stata 19/MP session. |
| Windows x86-64 | Run `win-20261028T183336Z-bc5a4e6b` at `7674cf06` passed the Rust 1.85.1/MSVC build, the project tests, the PE archive-path and source-manifest checks, the native loader and licensed Stata `stata-do` smoke with its authoritative status file, and the execution-identity, source-hash and shutdown-backstop controller checks (receipt `039f3245...`, source archive `f709dca1...`). |
| Linux x86-64 | The qualified binary from build job 8313475 retains its SHA-256 (`a7964116...`). Job 8318613 passed the ELF audit, actual ABI/load/panic checks and all 67 fixtures of the full Stata 19/MP profile (0 failed); job 8318967 passed all 11 staged-package fixtures from the exact binary archive (`1a4de469...`) in Stata 19/MP. |

The dependency audit found no advisories. The strict default
`./scripts/run_tests.sh` passed with the refreshed package. The source archive
SHA-256 is `2ca4fa49566c462bb24d045ea4b878bba5ab80952c3c410eb7924ae2990e365b`;
the binary archive SHA-256 is `1a4de46983daaea92643dd0adba682007fd2dce6b8410219079e9e1ba94dc9cf`. Native Intel-host Stata testing remains
outstanding; both macOS slices are supplied. One tooling mistake preceded the
accepted runs and changed no artifact: the first archive-build attempt looked
for the source ZIP in a directory while `package_release.py` writes the ZIP at
the given output path.

After publication, fresh Stata 19/MP sessions installed beta.10 from the public
[beta.10 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.10)
and from updated `main` into isolated PLUS directories. All 34 package files
installed from the release matched the staged package bytes, and the 35 files
installed from `main` matched them too apart from `dist/fereg-native.json`, the
root package's own manifest, which bound the runtime to `7674cf06`. Native
selftest passed on both routes, and `fereg`/`feppml` on the `auto` data agreed
with Stata's `regress` and `poisson` dummy regressions to rounding (coefficient
relative differences 2e-16 and 6e-14). GitHub's recorded SHA-256 and size
matched every one of the 50 published assets. A first `main` installation
before the notices refresh (`d894f630`) differed only in
`fereg-THIRD-PARTY-NOTICES.txt`, which main's copy had not carried forward.

### Beta.9 two-FE Auto round-two packaging

Beta.9 ships the round-two changes described above. The qualified Linux binary
is the `d6975ec2` SCC build (job 8305425), Windows was built at `8b2f38a4`
(run `win-20261027T235540Z-45d74d45`), and universal macOS was built offline
from the corresponding-source archive at `8b2f38a4`. Runtime production inputs
are unchanged across these revisions (`scripts/check_complexity.py
--compare-production`). The [package metadata](../dist/fereg-native.json)
records each actual build commit, binary checksum, runtime identity and
corresponding-source hash.

| Platform | Beta.9 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. All 67 fixtures of the full Stata 19/MP profile, the 73-case replay against the beta.8 plugin under the eager-cells scope, public OLS/PPML Break/rollback and eleven staged-package fixtures passed (the five remaining installation fixtures need the intentionally excluded development adapters or a Windows host). |
| Windows x86-64 | Run `win-20261027T235540Z-45d74d45` at `8b2f38a4` passed the Rust 1.85.1/MSVC static-CRT build, exact-FMA and fused-kernel tests, C numeric guards, PE audit, native loader checks, checksum, Stata smoke and single-runtime stages; the controller stopped the machine, deleted transient objects and released its lock. The collected plugin's SHA-256 matches the receipt. |
| Linux x86-64 | The qualified binary from build job 8305425 retains its SHA-256. Job 8308581 passed the ELF audit (glibc 2.28 maximum), actual ABI/load/panic checks and all 67 fixtures of the full Stata 19/MP profile; job 8308686 passed the same eleven staged-package fixtures from the exact binary archive (job 8308667 stopped before any fixture because the runner needs an absolute Stata executable path). |

The dependency audit found no advisories. The strict default
`./scripts/run_tests.sh` passed with the refreshed package. The source archive
SHA-256 is `7f1ab939328f94c338ceb126cb404185439f0b1bbe0857a508963c9e7547decd`;
the binary archive SHA-256 is `ae409717f30440191e1ce633695d14079714f8e74f02e6d834f2dc2de5ae7fac`. Native Intel-host Stata testing
remains outstanding; both macOS slices are supplied. Two tooling mistakes
preceded the accepted runs and changed no artifact: the first source-archive
attempt used an exported snapshot without Git metadata and the first staging
attempt lacked the plugin checksum files.

After publication, fresh Stata 19/MP sessions installed beta.9 from the public
[beta.9 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.9)
and from updated `main` into isolated PLUS directories. All 35 installed files
matched the staged package bytes on both routes, the manifest bound the runtime
to `8b2f38a4`, native selftest passed, and `fereg`/`feppml` on the `auto` data
agreed with Stata's `regress` and `poisson` dummy regressions to rounding
(coefficient relative differences below 1e-13). GitHub's recorded SHA-256 and
size matched every one of the 50 published assets.

### Beta.8 two-FE Auto packaging

Beta.8 ships the two-FE Auto changes described above. The qualified Linux
binary is the `18b47675` build, Windows was built at `37f1ab89`, and universal
macOS was built offline from the corresponding-source archive at `6b97e504`.
Runtime production inputs are unchanged across these revisions. The
[package metadata](../dist/fereg-native.json) records each actual build commit,
binary checksum, runtime identity and corresponding-source hash.

The strict default `./scripts/run_tests.sh` passed with the refreshed package:
319 Python tests, Rust unit and independent numerical integration tests,
Clippy, FFI allocation audits, portable checks, mock SPI transactions,
the complexity ratchet and the root-package identity check. An independent
high-effort source review found no blocking defects in the parallel operators,
gate v2, derived-residual certification or publication validation; it did not
replace the executed checks.

| Platform | Beta.8 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive, with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. All 44 mapped Stata 19/MP fixtures, 73 exact replay cases against the qualified Mac candidate, public OLS/PPML Break/rollback, and ten staged-package fixtures passed. |
| Windows x86-64 | Run `win-20261027T153054Z-e4e3b51f` at `37f1ab89` passed the Rust 1.85.1/MSVC static-CRT build, FMA/hardware tests, C numeric guards, PE audit and actual ABI/load/panic checks. A fresh local `net install` verified plugin bytes and passed installation smoke, single-runtime repeatability and four OLS/PPML fixtures. The controller stopped the machine, deleted transient cloud objects and released its lock. |
| Linux x86-64 | The qualified binary from build job 8298656 retains its SHA-256. Job 8299903 passed ELF audit (glibc 2.28 maximum), actual ABI/load/panic checks and 42 Stata fixtures. Two development fixtures required the intentionally excluded `_fereg_estimate_ols_test` adapter; after restoring it only in the test workspace, job 8300025 passed both. No runtime or assertion changed. Job 8300111 passed ten staged-package fixtures and a fresh local `net install`, verifying all 34 versioned files and independent OLS/PPML comparisons. |

A fresh local Mac installation of the root package verified all 35 installed
files, native selftest, exact OLS/PPML repetition and rollback, and independent
full-dummy OLS and Poisson/covariance comparisons. The dependency audit found
no advisories. The initial Mac build without the documented path remaps failed
the personal-path audit; the remapped offline rebuild is the distributed binary.
An initial local source-test invocation selected Python 3.9 and failed on missing
Python modules; the required strict suite passed in the corrected environment.
No performance observations were repeated during release preparation.

The source archive SHA-256 is
`8e89db60ba8b9111b000c68ea81d71e0bea48b5f6421f421a00e205592546c62`;
the binary archive SHA-256 is
`aafc8ce74dbe0df876a07c4375f6e34f0c860fbcd44f76d29bb7ec67a7ebd843`.
Native Intel-host Stata testing remains outstanding; both macOS slices are supplied.

After publication, fresh Stata sessions installed from the public
[beta.8 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.8)
and through `github install johannes-schmieder/fereg` from updated `main`.
All 34 versioned files and all 35 main-package files matched their expected
bytes; native selftest, repeatability/rollback and the independent full-dummy
OLS and Poisson/covariance checks passed on both routes. GitHub's recorded
SHA-256 and size matched every one of the 50 published assets.

### Beta.7 PPML packaging

The beta.7 package shipped the lean PPML two-FE protocol and CMG reuse.
Its [versioned release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.7)
records each binary's build revision, checksum and corresponding-source identity. The Linux plugin is the qualified
`277c5d11` build measured above. Windows was built at `7a33fc20`, and the
universal macOS plugin was built offline from the corresponding-source archive
packaged at `7a33fc20` with a passing dependency audit (no advisories). The
runtime identity is unchanged between `277c5d11` and `7a33fc20`.

The default `./scripts/run_tests.sh` passed at `8506a65a`: 319 Python tests,
Rust unit and independent numerical integration tests, Clippy, FFI allocation
audits, portable checks, mock SPI transactions, the complexity ratchet and the
root-package identity check.

| Platform | Beta.7 checks |
| --- | --- |
| macOS | The offline archive build passed its binary audit and actual ABI/load/panic checks. 40 Stata 19/MP fixtures passed: the 18 beta.6 release fixtures plus 22 PPML route, canonical, joint, separation, inference and parity fixtures. Public PPML and OLS Break/rollback passed, and ten installed-package fixtures passed from the staged archive. |
| Windows x86-64 | Run `win-20261026T181228Z-1bcb165d` at `7a33fc20`: Rust 1.85.1/MSVC static-CRT build, exact-FMA and hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and OLS/PPML fixtures in Stata 19/MP. The machine was stopped afterwards. |
| Linux x86-64 | Qualification job 8287434 built the plugin offline with Rust 1.85.1 and GCC 12.2.0; ELF audit and actual ABI/load/panic checks passed. Job 8289216 repeated the audit (glibc 2.28 maximum) and load checks and passed the same 40 Stata 19/MP fixtures; job 8289358 passed ten installed-package fixtures from the staged archive. |

The root package passed a fresh local Mac `net install`: all 35 installed files
matched, and selftest, OLS/PPML examples, a three-FE OLS comparison and a
two-FE PPML comparison with explicit dummies passed. After publication, a fresh
Stata 19/MP `net install` from the
[beta.7 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.7)
matched all 34 versioned package files and passed the same checks. All 50
release assets matched their local SHA-256 checksums. After the merge,
`github install johannes-schmieder/fereg` from `main` matched all 35 installed
files and passed the same checks in a fresh session.

### Beta.6 many-FE geometry packaging

The beta.6 package shipped the many-FE geometry policy. The Linux and macOS
plugins were the qualified `99f0134d` builds; Windows was built at `07d4da6d`, and the runtime
identity is unchanged between them. The corresponding source was packaged from
`07d4da6d` with a passing dependency audit (no advisories). An offline Cargo
build from that archive produced a universal Mac plugin that passed its binary
audit and ABI/load/panic checks.

The default `./scripts/run_tests.sh` passed at `78dfeca0`: 319 Python tests,
Rust unit and independent numerical integration tests, Clippy, FFI allocation
audits, portable checks, mock SPI transactions, the complexity ratchet and the
root-package identity check.

| Platform | Beta.6 checks |
| --- | --- |
| macOS | Binary audit and actual ABI/load/panic passed. The 14 focused Stata 19/MP fixtures above passed, public OLS Break/rollback passed again at `07d4da6d`, and eight installed-package fixtures passed from the staged archive. |
| Windows x86-64 | Run `win-20261026T003403Z-d67fa2b4` at `07d4da6d`: Rust 1.85.1/MSVC static-CRT build, exact-FMA and hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and four OLS/PPML fixtures in Stata 19/MP. The machine was stopped afterwards. |
| Linux x86-64 | SCC job 8277741 built the plugin offline with Rust 1.85.1 and GCC 12.2.0. ELF audit (glibc 2.28 maximum) and actual ABI/load/panic checks passed. Job 8280233 passed 18 Stata 19/MP fixtures, including the 14 focused many-FE fixtures; job 8280278 passed eight installed-package fixtures from the staged archive. |

The root package passed a fresh local Mac `net install`: all 35 installed files
matched, and selftest, OLS/PPML examples and a three-FE comparison with
explicit dummies passed. After publication, a fresh Stata 19/MP `net install`
from the [beta.6 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.6)
matched all 34 versioned package files and passed the same checks. All 50
release assets matched their local SHA-256 checksums. After the merge,
`github install johannes-schmieder/fereg` from `main` matched all 35 installed
files and passed the same checks in a fresh session.

### Beta.5 platform packaging

The beta.5 packages added Linux and Windows binaries with the method-reporting
correction; their runtime production code was unchanged by the packaging work.
The default `./scripts/run_tests.sh` passed for the reporting correction: 316
Python tests, Rust unit and independent numerical integration tests, Clippy,
FFI allocation audits, portable checks, mock SPI transactions and the complexity
ratchet. Packaging changes reuse that runtime evidence and add focused tooling,
native and installed-package checks. No new speed claim is attached to them.

| Platform | Checks |
| --- | --- |
| macOS | Universal plugin built offline with Rust 1.85.1; binary audit and actual ABI/load/panic containment passed. All 12 mapped Stata 19/MP fixtures passed on Apple Silicon, plus public OLS Break/rollback at the pre-preparation checkpoint. |
| Windows x86-64 | Rust 1.85.1/MSVC static-CRT build; exact-FMA and eight hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and four OLS/PPML fixtures in Stata 19/MP. |
| Linux x86-64 | Offline Rust 1.85.1 build; ELF audit (glibc 2.28 maximum), actual ABI/load/panic and C numeric guards passed. Six Stata 19/MP fixtures passed, followed by a fresh local installation, selftest and four OLS/PPML fixtures. All 30 installed files matched. |

The scientific fixtures cover CG/CMG reporting, zero-step certificates, mixed-block
MAP fallback, independent OLS dummy-variable comparisons and PPML full-dummy
Poisson. The broader Mac set also covers weighted/wide OLS and explicit CMG-Schur.

Linux build job 8269530 produced the plugin and passed its audit/load checks,
then failed because the default compiler's sanitizer library was missing.
Its dependent installation job 8269613 stopped before testing. Job 8269823 reused
the same plugin with GCC 12.2.0: C guards and all Stata checks passed, but the file
checker rejected Stata's lowercase ancillary filenames. Exact-byte verification
with case-normalized names passed in job 8269843 (`failed=0`, `exit_status=0`).
The focused verification reused the existing installed files and test results.
These environment/checker failures required no numerical changes or estimation
retries. The failed jobs remain failed.

The Windows build harness required two corrections: it assumed exactly three
hardware-kernel tests although the suite now has eight, and its installed-file
shell call used Stata compound quotes instead of OS quotes. Native compilation,
exact-FMA/hardware checks and ABI/load/panic containment passed before the latter
failure. Short inert-file probes isolated argument quoting; the corrected probe
verified installation and exact bytes without building or loading native code.
A later cold build exceeded the controller’s 15-minute limit during the C bridge
build. The fereg build/test profile now has a fixed 30-minute allowance; other
profiles and cleanup safeguards are unchanged. Failed build/install attempts
remain failed; these were tooling repairs. Final run
`win-20261024T170546Z-5b24267c` passed at source `5b381a2c` in 884.843 seconds.
Its actual build revision and plugin checksum are recorded in the native
package metadata and release manifest. Transient objects were deleted, the lock released and the
Windows machine stopped.

The assembled three-platform main package passed a fresh local Mac installation,
selftest and four OLS/PPML fixtures; all 35 installed files matched exactly.
After publication, actual `net install` from beta.5 and `github install
johannes-schmieder/fereg` passed in fresh Stata 19/MP sessions. All 34 versioned
package files and 35 main-package files matched, including Linux and Windows
binaries; selftest and the four OLS/PPML fixtures passed in both installations.
All 50 GitHub release assets matched their local SHA-256 checksums and sizes.

## Limits

- Linux performance evidence applies to Linux. The practical Mac ownership/cell
  check reports paired medians, not confidence bounds or a universal speed claim.
- Many-FE designs that need the additive preconditioner from the start can be
  slower than beta.5: the near-confounded third FE is about 12% slower at one
  worker on Linux. This is an accepted exception, not a passed comparison.
- The Mac plugin contains Apple Silicon and Intel slices. Licensed installation
  testing is on Apple Silicon; native Intel-host Stata testing is outstanding.
- PPML CMG reuse mainly helps weakly connected two-FE designs; AKM and dense
  two-FE PPML gain 16-27% from the leaner passes and settled strict entry.
  The PPML joint frames still apply their CMG hierarchy with one scalar
  cycle per step, and separation IR dominates PPML time on Veneto-shaped
  panels; the October 27 parity work measured these on the Mac only. In
  the beta.7 SCC comparison fereg was faster than default fixest in every
  eligible PPML configuration, but PyFixest's `within` and MAP demeaners were
  faster on dense two- and three-FE PPML.
- PyFixest's `within` default was also faster than fereg on dense three-FE OLS.
  After the two-FE Auto speed changes it remains 3% faster on the short Veneto
  panel at two workers; fereg is faster in the other nine Veneto cells measured.
- The two-FE Auto speed qualification accepted 11 instead of 12 distinct
  physical hosts per Linux cell (waived by Johannes); its Mac check is
  descriptive.
- Explicit `solver(cmg)` was about 50 times slower than `solver(cmgschur)` on the
  real Veneto panels, and explicit `solver(lsmr)` returned `r(909)` on the long
  panel in a 64 GB allocation where Auto completed. Auto does not use either
  route there.
- fedemean charges a Schwarz build's stages, transients included, against its
  memory limit before they allocate (fedemean `a59598f6`), and a fedemean
  fit's published solver workspace includes that build's peak: the
  explicit-Schwarz capped-memory test repeats its bits at exactly the
  published peak and declines one byte below it. PPML LSMR projections, the
  strict-checkpoint accelerator and the separation re-execution still limit
  fedemean to fereg's estimate of its base workspace plus a fixed Schwarz
  grant (SPEC §19), because fedemean cannot yet estimate a Schwarz build
  before building it. The limit binds fedemean, so the admission is an upper
  bound on its use, but the grant is small: on the one-million-row bench
  panel (worker and firm, and with year) it is 76 and 152 MB against
  fedemean's measured Schwarz build of 389 and 415 MB beside a base of
  87 MB, the same with unit and varying weights. There `feppml,
  solver(lsmr, precondition(schwarz))` fails with r(909) (already so at
  `e80da565`, before the build was charged), and an automatic escalation of
  those projections would keep Jacobi.
- Package comparisons are descriptive: one seed, three repetitions, and
  same-model SCC nodes that differed by up to 2.4 times in absolute speed.
- GitHub Actions remain disabled; checks run locally or on the authorized SCC
  and private Windows hosts.
