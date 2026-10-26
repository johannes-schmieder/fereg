# Validation status

The many-FE, two-FE structural-gate and ownership/deferred-cell improvements are
accepted, as is the many-FE geometry policy with one documented timing exception.
The OLS method-reporting correction changes diagnostic labels and
fallback flags only; solver selection, iteration budgets, numerical calculations
and PPML reporting are unchanged.

## Accepted results

| Change | Evidence and scope |
| --- | --- |
| Many-FE Auto | [Final compact result](../bench/results/incremental-many-20261018-compact.json): Linux qualification and practical Mac recovery passed all 126 protected comparisons per platform; public activation and independent science passed. |
| Two-FE structural gates | [Linux](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json) and [Mac](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json): all 84 protected bounds per platform passed. [Activation](../bench/results/two-fe-policy-gates-activation-20261023.json): exact public/hidden science, worker, gate, hierarchy and resource checks passed after the documented transport-accounting amendment. |
| Ownership and deferred cells | [Linux](../bench/results/ownership-qualification-final-linux-20261024.json): 10,440 calls, 58 settings and 12 distinct physical hosts, all 116 protected bounds passed. [Practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json): 870 calls and all 116 paired-median comparisons within max(5%, 1 ms). The Mac check is descriptive, not a confidence-bound claim. |
| Many-FE geometry Auto | [Decision record](../bench/results/many-geometry-20261025.json): Linux qualification passed 80 of 84 protected comparisons and public activation 122 of 126, on 12 distinct physical hosts each. Every miss is the near-confounded third FE, accepted by Johannes as a documented exception; public/hidden agreement was exact. |
| Public integration | [Final activation](../bench/results/ownership-public-activation-20261024.json): 174 actual hidden/public/public-repeat calls per platform, independent witnesses, fixed-worker exactness, cross-worker science, resource/gate/hierarchy checks, Mac/Linux ABI/load/panic checks and the full source suite passed. Six licensed Mac Stata fixtures and actual public OLS Break/rollback passed. |

Ownership/deferred cells activate only for OLS Auto/Auto with a global intercept
and exactly two categorical-intercept FE terms. Explicit injections and paths
outside the scope retain their contracts. The Break test interrupts at the
native pre-preparation checkpoint; it does not establish interruption inside a
cell constructor. Scientific behavior is specified in [SPEC](SPEC.md).

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

## PPML two-FE CMG reuse and lean protocol

PPML Auto with exactly two intercept FEs reuses one CMG hierarchy across IRLS
frames and canonical products, and publication fits enter the strict
checkpoint once the predictor settles, as specified in
[SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons) and §13. This
branch result awaits Johannes's decision. The
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

## Package checks

The beta.6 package ships the many-FE geometry policy.
[Package metadata](../dist/fereg-native.json) records each binary's build revision,
checksum and corresponding-source identity. The Linux and macOS plugins are the
qualified `99f0134d` builds; Windows was built at `07d4da6d`, and the runtime
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
- PPML CMG reuse helps weakly connected two-FE designs only. Dense two-FE PPML
  gains only from the exact passes and still trails fixest at four million rows.
  On weak graphs the first strict checkpoint is commonly rejected and IRLS
  resumes for two frames; that protocol cost is unchanged.
- GitHub Actions remain disabled; checks run locally or on the authorized SCC
  and private Windows hosts.
