# Validation status

Johannes accepted the integrated runtime on 2026-10-07. Its final timing campaign
was closed incomplete by explicit decision; this is architectural acceptance,
not a performance pass or a universal speed claim. Historical timing was mixed.
The final incomplete screening included PPML without FE about 19–21% faster and
wide two-FE OLS about 27–36% slower than the fixed development baseline; these are
descriptive observations, not completed confirmation results.

The preceding integrated Mac evidence covered 375 scientific families, 98 holdouts
and 6,375 native commands at 1/2/4/7/8/9/15/16 workers, with zero recorded scientific
failures, exact fixed-thread repeats, exact unaffected results and stable discrete
decisions. Independent oracles, transaction checks and allocation checks passed.
The accepted native SHA-256 was
`20a9d3d9234fa9faed9338e384c6d3987a6c0d2371d2066242f96e1a1ffa6826`.
The compact acceptance receipt is recoverable at
`d83d4af1:bench/history/2026-10-07-m35-architectural-acceptance.json`.

The completed program retired 11,373 net production lines against its fixed
baseline, including dependency additions. Removing historical reports, plans and
builds does not increase that production-code result.

The single-runtime repository cleanup removes the old selector and packaging,
retains independent scientific witnesses and preserves the numerical Rust runtime.
The completed cleanup checks are recorded below. Old raw captures and
builds are deleted with Johannes's explicit authorization; ignored raw files are
not recoverable through Git. Historical tracked source and summaries remain there.

The integrated scientific qualification above is local Mac only. GitHub tests
remain paused. The bounded platform build checks requested on 2026-10-07 are
recorded separately below and do not imply full platform scientific or timing
qualification. Native distribution still requires the separate
[licensing decision](licensing.md).

## Single-runtime cleanup checks

Local validation on 2026-10-07 passed:

- Full `scripts/run_tests.sh`: formatting, Clippy, Rust workspace tests,
  isolated allocation/FFI audits, unchanged complexity ratchet and mock SPI.
- 219 Python tests, including independent recovered graph/Poisson witnesses,
  reference identity, packaging, parser and transaction-runner checks.
- 12 compact benchmark-tool tests and all four smoke models (28 commands),
  with coefficients, covariance, sample, scalars and output comparisons passing.
  This validates the tool; its timings are descriptive only.
- Universal Mac build, binary audit and real-plugin ABI/load/panic containment.
- All 64 licensed Stata integration fixtures: 63 passed in the full run; the
  remaining historical parity fixture passed after the repair described below.
- 73 native replay cases at 1/2/8 workers, passing independent science,
  exact fixed-thread repeats and stable discrete decisions.
- Three staged-installation fixtures, including removed-option rejection,
  missing-plugin failure and rollback, plus OLS and PPML interactive Break.
- Portable shell/Python/strict-C checks and documentation links.

The tested local plugin SHA-256 is
`14dd962c77d52b6e0f2622f10623dacac04457c190e28bcf874fb171c671f5ac`.
No numerical Rust runtime source changed. The sole Rust test edit relocates the
sealed weak-graph reference include; the reference bytes are unchanged.

The old interaction fixture assumed the legacy engine's final-sample names.
It now asserts the original 21 slots/base flags and the declared omission/rank,
then checks a fixed independently specified retained basis using Stata dummy
regression. Coefficients, VCE, residuals, RSS and DoF are checked. The nearly
singular four-way covariance determinant uses a cancellation-scaled 32-machine-
epsilon bound against the explicit determinant and Stata's calculation, while
preserving PSD checks. Native exact-arithmetic tests remain. The original failed
fixture run is not described as passing; only the repaired fixture passes.

Astra independently reviewed the final retirement at high effort and approved it
without blocking findings. The extracted Rust parser, complexity inventory and
sealed reference bytes were independently checked against their prior versions.
Old source-pin campaign guards were retired; retained native allocation tests
are not a claim of exhaustive detection of every possible new allocation.

The working tree previously occupied about 1.20 TiB, primarily generated target
artifacts. The authorized purge removes those builds/captures and old binaries,
retaining current source, tests, Git history and one current Mac plugin. Ignored
raw captures are intentionally discarded, not archived elsewhere.

## Requested Linux and Windows build checks

The first private Windows build/load and Stata smoke run,
`win-20261007T160507Z-9b10570b`, failed with `STATA_DRIVER_FAILED` on exact source
`f80fbc23554e62477c946b046fbe35b4338261b5`. Its transferred archive SHA-256 was
`357a2c9fd2674dbbaac078617bad73efbf985bd907d24cd532a25cbcea19cda4`;
the sanitized receipt SHA-256 was
`5dacca9be37b66ebe28d6e2d24ceb2c5d3886165b56d762b53e3ed655362b160`.
The aggregate receipt does not establish which build or smoke step failed.
It verified source identity; the wrapper confirmed stopped state, deleted
transient transfers and released the lock. No individual Windows gate is claimed
as passing from that run.

Astra's high-effort review identified a missing private-driver prerequisite:
the build invoked the C compiler/linker and dumpbin without initializing the
MSVC developer environment. The build now initializes and validates the x64
environment before Cargo. The same bounded native retest then passed:

- Source: `a8cdd888e6f4aca74e888193b7f2af07810bc594`.
- Run: `win-20261007T162625Z-3fc8a31f`, private `stata-do` controller.
- Archive SHA-256:
  `d77eb8d1f6fc47cd3f8c8c7fdd3425a1661eb97c67ca6786b68f225f04a7608e`.
- Receipt SHA-256:
  `feb66e9e97b04fcc6b4142cf6cc94db1e9b6fc8ddddae568427c3cc12c87132a`.
- Windows x86-64 release build, PE dependency/export/static-CRT audit,
  real-plugin ABI/load/selftest/protocol rejection and panic containment passed.
- The driver verified the generated plugin checksum and passed
  `windows_ci_smoke.do` and `single_runtime.do`: OLS/PPML estimation, the simple
  independent OLS witness, exact repeats, retired-option rejection and rollback.
- Source identity, receipt digest and every controller check were verified.
  The wrapper confirmed stopped state, deleted transient transfers and released
  the lock. No binary was published; the first failed run remains a failure.

At that checkpoint, Linux execution was pending. The subsequently authorized
2026-10-08 checks are described below. These bounded checks do not replace full
platform scientific or performance qualification; GitHub workflows remain paused.

## 2026-10-08 handover integration

The approved rule changes distinguish arithmetic-preserving cost evidence from
result-changing qualification, permit independently certified processor-specific
arithmetic, and require deletion of code used only by its own tests. The removed
standalone OLS pipeline was unreachable from the plugin; independent explicit
dummy OLS witnesses and the PPML reference remain. Core production lines fell
from 95,023 to 94,423 and dispatch candidates from 1,265 to 1,251. The omission
check rejects only unavailable components with an available omission flag;
the existing continuous, factor and interaction encodings remain valid.

The final Mac integration through `e3380ede` passed 1,647 native test executions,
formatting/Clippy, isolated allocator/FFI checks, the complexity ratchet and
APPLEMAC/OPUNIX numeric input/output guards under UBSan. The universal build,
binary audit and actual-plugin ABI/panic checks passed. All 73 native replay
cases passed independent science and fixed-thread repeats with zero exact
differences against the prior plugin. The tested Mac plugin SHA-256 is
`66e35883bdb88bd859d20af313375463877d2b281d6a48e21815ec9de442df98`;
the comparator is
`fe58988dfc56b5e545c77d9a5a06a712365b1b6ae29626639c710b41609314b4`.
Its embedded build label predates the source commit; the recorded binary hash
identifies the tested artifact.

Ten mapped licensed-Stata factor/parser/OLS/PPML fixtures passed after correcting
one stale action-protocol assertion and running the Stata-19 reference fixture
on Stata 19. Their two original failures are not passes. After the SPI port,
five mapped transaction/factor fixtures passed again on Stata 19. Astra's
independent high-effort reviews found no lost OLS witness or remaining SDK ABI,
main-thread, bounds/type, read-order or memory-admission blocker.

The native benchmark runner streams large output-column hashes, requires exact
agreement on every repetition and binds inputs/plugins/harness by hash. Its
focused tests and 400-row real-plugin A/B smoke passed; the broader benchmark
Python suite passed 40 tests in the scientific environment. A system Python
without NumPy cannot run three of those tests. The random FE design creates
codes before outcomes rather than replacing effects after data generation.
Exact/Philox PPML bias targets now come from independent input-bound references;
the numerical scope allowlist was not weakened.

The six exact-FMA witnesses are independently derived using rational/integer
rounding, including true extended-precision double-rounding counterexamples.
They pass natively on ARM macOS and under Rosetta for the x86-64 macOS target.
This does not establish an Intel-hardware timing result. Native Windows
arithmetic evidence is recorded below.

### Merged FMA and group-mean evidence

The existing source-bound comparison of `d28db6c`, `10ad656` and `bdbb99c`
is reused for unchanged artifacts: seven interleaved repetitions across twelve
workload/worker combinations per host on M2 Ultra and x86 Broadwell. Scientific
results were exact and every native-time comparison met max(5%, 1 ms).
These are bounded native-call observations, not end-to-end Stata release claims.

SCC job 7968913 added two-repetition Cachegrind comparisons of the parent and
candidate Linux plugins on predeclared 100,000-row, probability-weighted,
10,000-by-1,000 adjacent-mobility cases at one worker. Both repetitions had exact
results and identical group-function instruction counts:

| Regressors | Parent group-function Ir | Candidate group-function Ir | Reduction |
| --- | ---: | ---: | ---: |
| 5 | 193,249,000 | 142,953,000 | 26.0% |
| 20 | 408,279,000 | 187,583,000 | 54.1% |

The totals include the moved kernel bodies and dispatch wrappers. Whole-process
instruction reductions were only about 0.21% and 0.52%; they include harness/I/O.
Instrumented seconds are not native-time qualification. Scheduler accounting
reported `failed=0`, `exit_status=0`, with the application success marker.
Parent/candidate plugin SHA-256 values are respectively
`94bfe19df94b41864c068bae2ffd9d4cbad5fb4d7a65649830ca973b38a6d7f3`
and `07e5e590599abc4e8a8bac2795adfaa9dded473f10d24ef3adea7358ba7289a9`.

### Bounded performance screens

These screens are observations, not additional qualified production optimizations.
The weak/chain/barbell screen used one million rows, five regressors, unit
weights, IID VCE, 1/4/8 workers, 60 seconds per fit, a 20-minute budget and a
deliberately reduced 1,000-iteration limit. Automatic PCG exhausted that limit
on weak and chain graphs; LSMR certified both at every worker count. CMG's main
solve took five iterations but FE recovery fell back to 464/832 CG iterations.
Automatic PCG did well on barbell (three iterations). Initial malformed CMG
requests were corrected to `preconditioner=off` before these observations.
Default routing remains unchanged pending Johannes's required decision.

An exact-bit register-tile prototype on 100,000 rows was slower at K=20/50/96
(median ratios 1.41/2.54/3.20 across five interleaved repetitions). A normalized
weight-cache prototype including its construction was slower on one million
rows at 1/5/50 passes (1.37/1.09/1.07). Neither prototype was integrated. This
does not rule out different layouts or establish an x86 result.

Instruments profiled one-million-row random two- and three-FE designs at
1/2/4/8/16 workers, twenty native fits per trace with a 60-second trace cap.
The three-FE last-fit native times were 1.79/1.07/0.737/0.619/0.617 seconds;
these instrumented, noisy observations are not scaling qualification. At four
workers, back-substitution, sparse multiplication and gather accounted for
9.4%, 6.2% and 5.5% of 44,271 process samples. Inclusive Schwarz/solve shares
overlap and must not be added. At sixteen workers, scheduler/deallocation
samples grew. A standalone CSR-row iterator code-generation probe removed
two per-entry bounds checks while retaining separate multiply/add, but no
full-plugin speed or regression result supports a `within` pin change yet.

### Guarded Linux loading

Clean source `e3380ede` and baseline `a71df28a` built on SCC with native ELF,
glibc-2.28 and actual-loader audits passing. The first job (7969113) stopped
because the system linker could not find UBSan. The focused retest loaded the
installed GCC 12.2 module; both guard configurations and all 73 exact replay
cases passed. Its scheduler accounting (7969139) reported `failed=0`,
`exit_status=0`, 151 seconds and 2.244 GB maximum virtual memory, and the
application success marker was present.

Licensed Stata 19 compared the same plugins on one million observations,
100,000 worker levels, 1,000 random firm levels, probability weights and robust
VCE. Three interleaved repetitions at K=5/20 and 1/4 native workers gave median
candidate/baseline load-time ratios 0.958/0.919/0.929/0.967 respectively. Every
coefficient/covariance capture was bit-identical. These are bounded load-time
observations, not quiet release or end-to-end performance claims. Candidate and
baseline SHA-256 values are respectively
`45c54141405c249cb9eccc4b8199a5cdcc42509609e46a1308f826764079252d`
and `c8246c028453db2a7502f2f7927194c42797d2953efd0ce2bd5cdf791174bd2c`.

### Native Windows rounding and loading

The first handover Windows run (`win-20261008T191819Z-9805be9a`, source
`bca0c695`) failed with `STATA_DRIVER_FAILED`; its sanitized aggregate cannot
identify the failed step. No individual gate is inferred from that failure.
The build now captures Cargo test stdout without treating ordinary compiler
stderr as a PowerShell 5.1 terminating error, while checking its exit status
and explicit passing-test count. This is a plausible fix, not a proven diagnosis
of the first run. An intervening busy response did not execute a test.

The clean-source retest (`win-20261008T195618Z-1a9d1d41`, source
`e3380edea1b6172fb16ea84253bd255e80b5d5fa`) passed every controller check.
Mandatory gates covered the exact-FMA vectors, both numeric input/output guard
sentinels, the MSVC build and PE audit, real-plugin ABI/load checks and licensed
Stata OLS/PPML smoke. Its source archive SHA-256 is
`e02c58a142902d811a9b07c18d433df53c9cf97e7292c929016375d3a3248915`;
receipt SHA-256 is
`2672136ddd2e56b62d1801cd57f70960e52b4dab63c3e8a10fa3033a2907fe9a`.
The wrapper verified stopped state, deleted transient transfers and released
the lock. This is bounded native qualification, not Windows timing evidence.

### All-FMA and profile-guided build screens

The experimental x86 Linux all-FMA build was a measurement tool, not a portable
distribution candidate. After the bias-reference prerequisite passed, its
73-case replay passed independent science, fixed-worker repeatability and
1/2/4-worker discrete checks. Seventeen cases changed within the declared,
input-bound numerical scope; 7,759 numerical paths were allowed. The scope did
not permit changes to sample, omission, rank, separation or discrete metadata.
SCC job 7969158 completed successfully. The experimental plugin SHA-256 is
`8e9cf25c6d094aaf1c886902f20737b917d6462d2b0d68802ca41fccadfded85`.

Three interleaved repetitions at 1/4 workers screened 50,000-row weighted robust
OLS with K=64/65/129 and a 200,000-row robust PPML case, with a ten-minute budget
and 60 seconds per process. OLS outputs were exact; at K=129 the native-time
ratios were 0.751/0.797. PPML ratios were 0.981/0.957 with numerical differences
and exact within-build repeats. These results neither isolate the speculative
PPML products nor satisfy result-changing quiet-Mac qualification, so those
runtime products remain unchanged. The wide OLS result motivated a separate,
arithmetic-preserving tile-dispatch experiment. Timing job 7969187 completed
with `failed=0`, `exit_status=0` and its application success marker.

The Mac PGO screen used a 30-minute stopping budget. Training combined all
73 replay cases with large two-FE, three-FE and PPML cases at 1/4 workers.
The matching Rust profiling runtime was needed; initial linker/training-driver
failures were repaired before the successful run. The generated profile covered
6,441 functions; cold-function missing-profile warnings remained. Native binary
and ABI audits and the optimized 73-case exact replay passed. Five interleaved
repetitions per workload/worker combination were exact, but the three-FE native
time ratios were 1.071/1.061, exceeding the 5% allowance. Other ratios ranged
from 0.922 to 0.987. PGO was not adopted and no profile-dependent build recipe
was committed. The optimized experimental plugin SHA-256 is
`f62badfa8d3e0a76ce47dbab9b962ede02793c17b6ea278b48f4977a5d1300c4`.

The isolated wide-tile proposal extracted only the existing standardized Gram
update for more than 64 columns into the runtime FMA table. It preserved the
per-entry operation order, validation, stack tiles, memory admission and solver
route; independent scalar witnesses covered cache boundaries and untouched
matrix entries. The Mac full suite, universal build/ABI audit and all 73 exact
replay cases passed, and independent high-effort review found no blocker.
Rosetta could compile the x86 tests but exposed no FMA/AVX hardware path, so
its hardware-kernel tests were skipped and are not counted as hardware evidence.

The predeclared Mac comparison used all twelve frozen standard input hashes
plus K=64/65/129 cases at 1/4 workers, five interleaved repetitions, 60 seconds
per process and a 30-minute total budget. It was stopped after 57 observations:
substantial background FileProvider activity and another project's Rust test
contaminated timing, and the completed three-FE/four-worker comparison exceeded
the allowance. This is inconclusive, not a no-regression pass. The proposal was
removed from production and the qualified Mac artifact restored. The
unstandardized Gram fallback also remains unchanged. The experimental Mac
plugin SHA-256 was
`18f60378139f9eac6dc3dfd344becfa39862a106f4438394505be00b33b5aaba`.

The isolated proposal also passed native Linux build/ABI checks, all three
hardware-kernel tests on an FMA/AVX host, and the 73-case exact replay. Five
interleaved repetitions on the same predeclared K=64/65/129 designs at 1/4
workers produced sixty exact observations. Native-time ratios were
0.990/1.008, 0.811/0.857 and 0.751/0.743 respectively. This makes the wide
kernel a promising follow-up, but supplies no missing Mac nonregression pass.
Its source is `e3380ede` plus the isolated patch with SHA-256
`a42fac1b9eed7e9903f84c7e41002c2d7aa971724887ebbf51e430537649ad18`;
the experimental Linux plugin SHA-256 is
`75fb8c8a4280255717033d3eebc9248eee77184f92e4da415a66b7bbe92eee37`.

Two one-worker Cachegrind repetitions at K=129 counted approximately
41.98949 billion baseline versus 36.52103 billion candidate whole-process
instructions, a 13.0% reduction; the new tile kernel's instruction count was
identical between repetitions. All four outputs were exact. These totals
include parsing and output, and instrumented seconds are not speed evidence.
The screen used a 25-minute scheduler limit, a ten-minute native-timing budget
and a 1,000-second instruction-count budget with 240 seconds per instrumented
process. Compact observations and source probes were retained; completed raw
datasets, profiles and temporary source/build copies were retired.
SCC job 7969744 on `scc-gr4` finished with `failed=0`, `exit_status=0`,
811 seconds, 2.673 GB maximum virtual memory and the application success marker.

### Automatic PCG continuation investigation

The October 8 follow-up tested an automatic OLS two-FE continuation using the
existing PPML progress criterion: after eight PCG iterations, restart LSMR from
untouched inputs when the maximum active preconditioned residual contraction
exceeds 0.01. The prototype charged the retained PCG owner against continuation
memory, shared the remaining iteration allowance, refreshed the original-frame
certificate and norm handoff, and preserved explicit solver and PPML routes.
The LSMR recurrence used the existing 100-times-tighter target while publication
required the original requested tolerance. Existing callers were unchanged.

Paired-observation witnesses with exact residuals of plus/minus 1/8 on 512-level
chain and weak graphs certified in nine total iterations, while the original
policy exhausted a 100-iteration budget. A barbell retained exact outputs and
its three-iteration solver signature. Independent weighted dummy OLS witnesses
also passed across unit/analytic weights, Fast/Low storage and one/four workers.
Memory admission boundaries, tiny iteration budgets, deterministic repeats,
structural routing and checkpoint tests passed.

However, an 80-level chain needed 79 iterations under the existing PCG policy
and 167/168 under the prototype. With `iterate(128)`, the existing policy
succeeded and matched the independent dummy oracle, while the prototype failed
in all eight weight/storage/worker combinations. At `iterate(100)`, both failed:
the existing policy reserves one quarter of the allowance for MAP, leaving
fewer than 79 PCG iterations. The distinguishing 128-budget result rejects the
proposed routing rule without requiring timing. Slow early PCG contraction alone
does not establish that the selected LSMR preconditioner will improve convergence.
No graph-size fence was added and the prototype was removed.

Independent architectural review also identified a pre-existing retry-accounting
limitation in the LSMR helper: a positive-definiteness breakdown can occur after
iterations have been spent, yet the unpreconditioned retry retains the full
allowance and reports only its own work. The rejected continuation would have
inherited that limitation; this investigation does not fix or qualify it.
Compact experiment logs and the rejected patch are retained under the ignored
`target/optimization-followup/` directory. No performance claim is made for the
fallback and the default solver policy remains unchanged.

### Wide Gram and CSR follow-up qualification

The isolated wide-Gram dispatch was restored on `dc2d575c`. Its runtime source
matches the previously tested Linux proposal; the hardware test now reports an
unavailable FMA/AVX path explicitly. The full Mac entrypoint passed 1,647 native
test executions, formatting, Clippy, allocation/FFI audits, the complexity ratchet
and both numeric guard configurations. The universal build and ABI audit passed,
as did all 73 exact replay cases against the production artifact and five mapped
licensed Stata 19 transaction/public-contract fixtures. The restored candidate
plugin SHA-256 is
`629e0169603213fcc3ac49ed97630dd856caf0bca140a0f03544c3eda6748dcf`;
its production comparator is
`66e35883bdb88bd859d20af313375463877d2b281d6a48e21815ec9de442df98`.

A separate `within` candidate, local commit
`297f32d9c4f83cc7d814ea778cd28c1f976e68d0` based on the existing `803a4fe` pin,
replaces per-entry CSR indexing in back-substitution with checked row slices.
It preserves entry order, separate multiplication/addition, source-vector bounds
checks and parallel partitioning. Independent hand-computed witnesses cover
10,017 rows across four workers, empty/singleton/multiple-entry rows, cancellation,
FMA-sensitive operands, zero kept columns and signed zero. Focused tests and the
full `within`/`schwarz-precond` library suites passed. Disassembly of the actual
ARM plugins confirms a 15-instruction steady-state stored-entry loop becomes
nine instructions, with CSR slice checks outside it. This is code-generation
evidence, not a measured whole-estimation speedup.

The combined local candidate passed the same 1,647-execution fereg entrypoint,
universal build/ABI audit and 73-case replay, with zero exact scientific or solver
signature differences against the wide-only candidate. Its plugin SHA-256 is
`f286ff49d89ffe6146ab1664a71ff02da49484a7ec1f68c1e383b479ce7a1696`.
Nine mapped Stata fixtures passed, including weighted Schwarz, three-FE low-memory
operator reuse, deterministic four-FE OLS/PPML, saved outputs and transactions.
An additional `ols_three_fe_batch_oracle.do` check failed with `r(459)` and an
uncertified-result error on the combined, wide-only and untouched production
binaries. That fixture's default 64-slot LSMR uses diagonal preconditioning;
the modified Schwarz back-substitution is outside this failing route. The common
failure is preserved and is not counted as a pass or repaired by these changes.

A separate bounded large-input science check compared production, wide-only and
combined binaries on all twelve frozen standard cases, six K=64/65/129 cases
at one/four workers and four one-million-row three-FE LSMR cases at 1/4/8/16
workers. All 66 observations had exact scientific outputs and solver signatures.
The run had a five-minute total cap and 60 seconds per process. It ran on the
busy host solely to test results; its durations are not performance evidence.

The pre-commit Windows diagnostic (`win-20261008T213049Z-a29f1256`) failed with
`STATA_DRIVER_FAILED`. Its sanitized aggregate does not identify the failed
stage, so no native gate is inferred to have passed. The transferred source
was dirty, whereas inspection of `build_windows.ps1` confirms that its manifest
binding requires a clean source commit. This attempt therefore cannot supply
formal Windows qualification; a clean-commit rerun was required. The archive
SHA-256 is `7bc0e871b6e3ef19ebf8eb975dcaefefd8581bf060bf3a1d4642c80f1c424d99`
and receipt SHA-256 is
`516ce7a7d67415ed570da70168ce7e3d5969dd027c679ec897e5329e8546b051`.
The wrapper confirmed stopped state, deleted transient objects and released its
lock. GitHub testing was not resumed.

Unrelated Stata processes and background CPU activity prevented the first Mac
timing attempt. No follow-up timing was started then, and neither candidate had
a new Mac nonregression pass. The twelve frozen standard cases and K=64/65/129
cases were prepared for one warmup and seven interleaved repetitions, a 90-second
per-fit cap and a 30-minute campaign cap; the CSR screen additionally covers three-FE
LSMR at 1/4/8/16 workers. The allowance remains max(5%, 1 ms). At that point,
dependency integration and performance adoption remained unqualified. The local
Cargo override was removed, both dependencies retained their original pin, and
the patch, candidate artifacts and compact evidence were retained under ignored
`target/optimization-followup/`.

### Quiet Mac retry

The October 8 retry used the same frozen artifacts and inputs after the competing
Stata jobs ended. All agents were idle and no builds ran on the qualification
host. Sampled OS background activity was retained with the observations. Each
cell used one warmup and seven alternating baseline/candidate repetitions;
native-call times exclude input parsing and output writing. The decision uses
the upper bound of a paired-median 95% bootstrap interval, with 10,000 resamples
and a fixed seed, against the unchanged max(5%, 1 ms) allowance.

Wide-Gram dispatch passed all twelve standard cells and six K=64/65/129 cells
at one/four workers. All 288 observations were exact, including solver
signatures. Median candidate/baseline native-time ratios ranged from 0.986 to
1.022; the largest upper confidence bound was 1.035. The campaign finished in
500 seconds within its 30-minute limit. This supplies the missing Mac
nonregression evidence; the previously isolated Linux instruction-count and
timing evidence remains the basis for the x86 performance benefit.

The isolated CSR comparison passed all twelve standard cells and the
one-million-row three-FE LSMR cells at one/four/eight workers. The initial
sixteen-worker cell was inconclusive: two early pairs had large timing spikes
affecting both binaries, and its upper confidence bound was 1.193 despite a
median ratio of 0.952. These observations were retained. One bounded follow-up
added fifteen paired repetitions at eight/sixteen workers, with a ten-minute
limit and five-second host-activity samples. No competing estimator or compiler
was observed during that follow-up. All 320 initial/follow-up outputs were exact.

Pooling all 22 measured pairs at each of those worker counts gives:

| Three-FE LSMR workers | Median CSR/wide-only time | Paired-median 95% interval |
| --- | --- | --- |
| 1 | 0.987 | 0.971–1.002 |
| 4 | 0.979 | 0.949–0.988 |
| 8 | 0.991 | 0.984–1.001 |
| 16 | 0.983 | 0.966–1.006 |

The one/four-worker rows retain the original seven pairs. All final bounds pass
the allowance; observed whole-estimation gains are modest. Together with the
actual-plugin instruction reduction and independent scientific witnesses, this
qualifies the arithmetic-preserving change for integration. Both dependency
pins now select `297f32d9c4f83cc7d814ea778cd28c1f976e68d0`, containing only the
reviewed loop and its witnesses relative to the old pin. The source branch was
pushed after qualification; its push does not match the dependency repository's
main/tag workflow triggers, and the commit also carries `[skip ci]`. No GitHub
workflow was requested.

The integrated clean source `c1a9470ad3a1fa35ddf1d2dd153eb2d39ff98ca3`
passed the full Mac entrypoint again: 1,647 native test executions, formatting,
Clippy, allocation/FFI audits, complexity and numeric guards. Its universal
build and actual-plugin ABI check passed. The resulting plugin SHA-256 is
`5eb9302427f8d1be9d005677449098afecc09c53d506dd67f633457624fde4e5`.
All 73 replay cases passed with zero exact scientific or solver-signature
differences against the combined local-path candidate. The final mapped Stata
run again passed nine fixtures and reproduced only the existing diagonal-LSMR
`ols_three_fe_batch_oracle.do` failure described above.

A final bounded comparison of that Git-pinned artifact against production used
all 22 prepared workloads, one warmup and seven alternating measured pairs,
with the same 90-second per-fit and 30-minute total limits. Builds and agents
were idle; five-second host samples retained observed OS indexing/background
activity. All 352 outputs and solver signatures were exact. Every workload
passed the unchanged allowance: median candidate/production ratios ranged from
0.972 to 1.011, with a largest upper 95% bound of 1.044. The run took 702 seconds.
The one-million-row three-FE LSMR median ratios at 1/4/8/16 workers were
0.972/0.997/0.986/1.004. These are bounded native-call observations, not a claim
of a large Mac or end-to-end Stata speedup.

The clean-source Windows retry (`win-20261008T233610Z-3c89924a`) also failed
with `STATA_DRIVER_FAILED`. Its source archive SHA-256 is
`63f57f2ce38a105dc857105ce6d06008440f9b7974882aaa75243d204b3ab676`
and original sanitized receipt SHA-256 is
`0d34cc7dcb4008d6389480e35a8b70d47109e8b6d81dae6dc9aca315d2c9ed27`.
The receipt verifies the clean source manifest, source hash and controller
safeguards, but neither the Stata nor project-test gate passed. It does not
identify the failing native-build or Stata stage, so the earlier dirty-source
limitation cannot explain this second result and no native Windows gate is
inferred to have passed. The instance was stopped, transient objects deleted
and lock released. Windows qualification remains open; further diagnosis needs
stage-specific sanitized evidence from the guarded controller. No controller
boundary was bypassed, no GitHub testing resumed, and no native distribution
was published.

Completed generated datasets, raw replay files, temporary source copies and
intermediate plugins were retired after recording the results. Compact timing
observations, input hashes/headers, harness/source patches, failed evidence,
replay summaries and production/final comparator plugins remain in the ignored
follow-up directory; the current build cache and development plugin are retained.

### Windows diagnostics repair and successful rerun

The 2026-10-08 controller repair replaces the ambiguous wait-for-file failure
with separate process outcomes and source-bound stage diagnostics. The old
controller waited up to fifteen minutes even after Stata had exited; therefore
an old run's duration alone cannot establish a timeout. Earlier raw source/logs
were removed by that controller, and its aggregate receipts cannot recover the
historical cause. The failure did not reproduce with the instrumented driver;
no numerical runtime code changed in this repair.

The clean rerun passed all Windows gates:

- Source: `81c51e7e8f8ac67f0adc0ad53daa1692136d1200`.
- Run: `win-20261009T004206Z-3ec0d7f7`, private `stata-do` profile.
- Source archive SHA-256:
  `c69818fd57189ea451b8816afe4b4e517327e136d1de969aa90d93b88d125027`.
- Receipt SHA-256:
  `8fc3057b16132331ae75bacbb5eef2b166e005cdc1c3e0d1b8effb9c211f4ee2`.
- Plugin SHA-256:
  `08995120c8c4e96963ae32acb55f90a45b8fb0b660ebb0902610b173df50500a`.
- Release build, exact platform FMA witnesses, three fused-kernel tests,
  numeric input/output guards, binary audit and actual-plugin loader passed.
- Checksum verification, `windows_ci_smoke.do` and `single_runtime.do` passed.
- The controller reported `passed`, final stage `complete`, exit code zero and
  765,820 ms elapsed. This is an operational duration, not a performance result.
- Source/controller identities and every controller gate passed. The wrapper
  verified stopped state, transient-object deletion and lock release.

The fixed collector passed twelve local tests. The maintenance installer passed
thirteen isolated payload/rollback/cleanup tests. Real Windows PowerShell 5.1
tests passed stage ordering, original-error preservation, atomic status writing,
changed-plugin rejection, partial markers, early exit and timeout process-tree
cleanup before installation. Shared-backend acceptance passed the out-of-scope
permission denial, clean smoke, deliberate failure and concurrent-request
rejection, with cleanup verified. Existing FEVC receipt/artifact contracts and
the fifteen-minute Stata limit were preserved; GitHub workflows remain paused.

The accepted shared controller SHA-256 is
`f4b2fa90f4fe11a3b022407f0014207854bc4318163d00e8d2fa59e03ccd066d`;
its fixed diagnostic collector SHA-256 is
`37095cbf9ecbc91e796e3ff0e24a0d0ca7db96e3ae7e9af238f98d390a77ac3d`.
The maintained patch is against controller SHA-256
`f8db7b3321b44d893d19919dc11e1a298b41ced53b8a6b5a4b24ca60a494c3c2`.
These are bounded Windows build/load and mapped-test results, not full Windows
scientific or performance qualification or native distribution authorization.

## 2026-10-09 cleanup program, phase 1

Phase 1 retired test-only and dead code without changing published science
(commits f5eace5c through c4d91623). Independent public-API witnesses replaced
the non-independent in-crate dense PPML and separation comparators before they
were deleted: seeded dense PPML differential tests against
`fereg_oracle::explicit_ppml`, bounded separation primers through the public
fit, and an exact-rational recession-cone check of the same primers.

Evidence per commit used the authorized tiers: production-token identity where
no runtime token changed, otherwise a pinned-identity Mac build and the 73-case
replay against the program baseline (`9debf9bf`, SHA-256
`a66e48d37903f6f249acfc8d4ebcf0e5768309b8cec96b1cc5f83e034f2e174c`) with zero
exact differences. The C-wrapper deletion produced a byte-identical plugin.

Boundary wall-time check (`bench/time_exact.py`, Mac M-series, seven
alternating pairs after one warm-up, exact science and solver signatures in
every run) of candidate
`49ab9e29b4a162d8556f469df301edead43c111f63870f60392302b818250f55` against the
baseline on all 18 frozen workloads: every bootstrap upper bound is within its
max(5%, 1 ms) allowance. Median ratios range from 0.999 to 1.015; the largest
upper bound is 1.030 (no-FE K=5, four workers, allowance 1.212). A first run
stopped at the first one-million-row input because the timing harness used the
replay corpus's 10-million-cell bound; the harness build now raises it.

## 2026-10-09 cleanup program, phases 2 and 3

Phases 2 and 3 (commits 09fc5e4c through 261536ac) shared one reservation
trait, fixed frontend contract defects, made OLS interruptible with Break,
enabled checked batched stores on Linux and Windows, and removed redundant
work from hot paths. One declared ulp-scope change (296cbc23, PPML score
certificate normalization) passed its scoped replay; the replay baseline was
rebuilt from it (SHA-256
`9009210ad15c2142eaaf24cb66f4dc8280b0ecd6d88719b9a624e544bce91f24`).

Every later runtime commit is arithmetic-preserving: the 73-case replay
against that baseline has zero exact differences, and commits that changed a
parallel loop also replayed at two and four workers. Paths the replay does
not reach were checked directly: a bitwise A/B of clipped multiway clustered
covariances (fereg-testkit) for the eigensystem reuse, and an exact native
A/B on all 18 frozen standard and wide (64/65/129-column) workloads at one
and four workers for the FE kernel changes. PPML input loading through the
action team also passed the `ppml-scientific` Stata profile on 261536ac.

Boundary wall-time check (`bench/time_exact.py`, Mac M-series, seven
alternating pairs after one warm-up, exact science in every run) of candidate
`c0f9aff986349965365b6d96586b420ccf76d3007e50633bc5cbb31287f914a8` against
the 296cbc23 baseline on all 18 frozen workloads: every bootstrap upper bound
is within its max(5%, 1 ms) allowance. The largest upper bound is 1.108
(no-FE K=5, four workers, allowance 1.193). Observed median ratios below one
include 0.91 for two-FE K=20 at four workers, 0.93 and 0.92 for the
64-column workload at one and four workers, 0.94 and 0.91 for 65 columns,
and 0.95 for the probability-weighted two-FE case at four workers. These are
bounded observations on one host, not release performance claims.

Assessed and left unchanged, with reasons recorded in the commits: PPML-15
(cascade allocations beside O(N) scans), PLUG-17's memory-limit sampling
(fresh availability is intended), PLUG-18 (final publication barrier and an
O(K^2) check beside an O(K^2) symmetry check), and the residual loops of
OLS-7 (zero-indicator skipping can change a zero's sign). Deferred: PPML-10
(likely result-changing; needs its own scope and witnesses), FE-11
(structured recovery classification must reproduce today's matched set),
OLS-4 (needs a new admitted scratch layout), FE-16 (u32 level indices touch
every FE consumer) and the PLUG-12 runtime missing value (next wire bump).

## 2026-10-09 cleanup program, phase 4 and follow-ups

Phase 4 (commits 5dcaaf81, 366df30f, e97d355e) made developer fault plans
part of shipped control flow: the borrowed MAP absorber, checkpoint LSMR
batch, three-FE Schur budget, linear session, PPML IRLS loop, fused CMG,
prepared separation and simplex now run the same fault checks in shipped
and test builds, with every plan unset in shipped code and only the test
setters gated. Pure observation counters stay test-only. An independent
review found no shipped behaviour change, no shipped writer and no new
allocation; its two follow-ups are applied. These are the first
thread-locals compiled into shipped code (const-initialized `Cell`s without
destructors); Linux and Windows load checks were not run in this phase.

Every commit replayed the 73 cases against the 296cbc23 baseline with zero
exact differences. Boundary wall-time check (`bench/time_exact.py`, Mac
M-series, seven alternating pairs, exact science in every run) of candidate
`fdd48d5687698bcd9580184407827552f4d07d0feabcff1b2e41d80cd9933b41` against that baseline on all 18 frozen workloads: every bootstrap
upper bound is within its max(5%, 1 ms) allowance; the largest is 1.160
(no-FE K=5, four workers, allowance 1.204).

## 2026-10-09 integration checkpoint

All checks below tested clean main commit
`8fb67bb58cc473999e20f3abe9b1c37016d62139`; no source code changed.
GitHub workflows remained paused. This checkpoint makes no timing claim.
Raw commands, logs and receipts are retained under
`target/integration-checkpoint-2026-10-09/` (ignored).

- **Pass — Mac gate:** `./scripts/run_tests.sh`, using the Homebrew rustup
  PATH. The Linux-only mock audit was skipped on Mac as designed.
- **Pass — committed-source Stata full:** 68 tests, zero failures, Stata/MP 19.
  Full includes every syntax, differential and ppml-hidden fixture and both
  hidden Break fixtures; separate runs of those profiles were not needed.
- **Pass — separate licensed Break:** OLS, PPML and PPML `--public-scope`,
  using `scripts/run_stata_break_test.py` and the full-profile plugin.
- **Fail — full release staging:** `python3 scripts/stage_release.py` with
  the full-profile plugin directory failed with `missing or empty native
  artifact: .../fereg_linux.plugin`; that input directory contained only Mac.
  No full release zip was produced. **Pass — Mac review subset:** the supported
  `--review-platforms macos` archive passed the dependency audit, clean
  `net install`, installed-copy OLS/PPML residual/prediction assertions, and
  installation of `_fereg_indepvars.ado`. Review zip SHA-256:
  `6da8b22b9aa7b76c1990ce1613e40902a0e8ac7db5d63290b9097c747dd9a8ca`.
- **Pass — Linux:** `scripts/build_linux.sh`, its binary/loader/numeric-guard
  checks, `cargo test -p fereg-plugin fused_kernels`, and core unit tests
  (1,144 passed, three ignored). The FFI plugin suite passed in its documented
  serial configuration (168 passed, ten ignored). Licensed Stata 19 OLS/PPML
  output-store smokes passed at 96 and 8,192 rows; the latter crosses OLS's
  4,096-row batching threshold. SCC jobs 7983044, 7983253 and 7983326 each
  have accounting `failed=0`, `exit_status=0` and application result records.
- **Fail — initial Linux parallel FFI invocation:**
  `cargo test --locked -p fereg-plugin --lib --profile ci -- --test-threads=4`
  returned 101: 163 passed, five allocator-accounting assertions failed,
  ten ignored (including `construction 74966 > 67138`). This invocation
  conflicted with the repository's required serial FFI configuration; its
  failure is retained separately from the subsequent prescribed serial pass.
  Failed test executable SHA-256:
  `c8345ced0559a0dec0e9fe1d82b3a9e062997034eca76d5b0dd60e74c0b6d127`.
- **Pass — bounded Windows committed driver:** accepted shared controller
  `stata-do` run `win-20261009T153654Z-b98c20e7` passed the release build,
  exact-FMA/fused-kernel tests, numeric guards, PE audit, native loader,
  checksum and licensed OLS/PPML estimates. Outcome `passed`, last stage
  `complete`; source identity and every controller gate passed. The wrapper
  confirmed stopped state, transient-object deletion and lock release.
  Controller files were unchanged. Source archive SHA-256:
  `622d300306f381483cf688771d76788a9ee496b849d36da10484334af1284a11`;
  receipt SHA-256:
  `06195ea585a8fd6804e2d5729b6f183110511a562b7e5a43e5efdf9e7e8732d4`.
- **Not run — Windows saved-output smoke:** the committed controller driver
  does not request residuals/predictions. Adding that coverage would require
  changing its source snapshot; this task prohibited source changes.

Documentation links, whitespace and production identity passed (168 runtime
modules, 58 artifact inputs unchanged). The supplemental identity check first
failed under system Python (`No module named tomllib`); the supported
Homebrew Python invocation passed. Both logs are retained.

Tested plugin SHA-256 values:

| Platform | SHA-256 |
| --- | --- |
| Mac universal | `c43633626c498dc716e528c092fb7574645d496977124bb3d06f348b2ba47d9b` |
| Linux x86-64 | `919330739e440db8ae4a6c6b02c454076188ff63e4160ca067a027276108686d` |
| Windows x86-64 | `961afc7cc593f7520039dba0200d922dd3359e3b55a77a3662259b3c0e5ca751` |

```text
STATA_CI_RECEIPT status=success profile=full tested_sha=8fb67bb58cc473999e20f3abe9b1c37016d62139 stata_rc=0 failure_kind=None
```

**Platform claims:** Linux batched observation stores and loading of shipped
thread-local Cells are **verified** by these bounded checks. Windows loading
of shipped thread-local Cells is **verified**; licensed-Stata batched output
stores are **still unverified**. These results do not establish full Linux or
Windows scientific or performance qualification.

### Saved-output follow-up

The Linux parallel-test failure above was an invocation error: FFI allocation
counters observe the whole process, so simultaneous tests contaminate exact
allocation deltas. The prescribed serial suite passed all 168 active tests.
This serializes test cases, not estimator workers. No Linux runtime repair
was indicated; the original failed result remains recorded above.

Test-only commit `9c08204b74396cb811f61523b3b869836a980b9d` adds the portable
`observation_stores.do` fixture to the Windows smoke and full Stata profile.
Its complete 8,192-row sample exercises OLS batched residual/saved-effect
stores, compares OLS residuals and predictions with explicit dummy regression,
and checks PPML residuals, means, linear predictors and postestimation output.
The Mac gate and focused licensed fixture passed. Production identity against
`8fb67bb5` passed for all 168 runtime modules and 58 artifact inputs.

Windows `stata-do` run `win-20261009T161002Z-0ccd6f09` passed on that exact
commit, including the new fixture, native build, exact-FMA/fused-kernel tests,
numeric guards, PE audit, loader and single-runtime checks. The receipt digest,
source identity and every controller gate were verified; the controller and
diagnostic collector hashes were unchanged. The wrapper confirmed stopped
state, transient-object deletion and lock release.

- Plugin SHA-256:
  `ac92ad817cbbef9adf207f989494210cb32f87edf8b36fab787131a8b00ee359`.
- Source archive SHA-256:
  `6f1088302fa603c3075a2f396eed1b527c9a8266e23949779acfdf7395d6e4c0`.
- Receipt SHA-256:
  `b87b88e37b7afb63094e47c879015c56ce80c0e4974739b1a2c28eed8053fead`.

Windows licensed-Stata batched output stores are now **verified** by this
bounded smoke. Together with the preceding Linux evidence, both platforms
have verified batched stores and loading of shipped thread-local Cells.
Full platform scientific/performance qualification and full release packaging
are unchanged by this test-only follow-up.

## 2026-10-09 measurement-driven performance work

A profile of the frozen workloads plus a new 200,000-row PPML case with 30%
zero outcomes (`replay_large --zero-fraction 0.3 --separation "fe simplex ir"`)
directed four changes (commits 02936956 through 7dd56c01):

- IR no-separation results are certified from stored evidence (finite,
  unit-scaled recurrence; exact reproduction of the sign certificate; zero
  weighted residual moments on every kept regressor and FE channel) instead
  of a full detector re-run, with the re-run as fallback (Johannes's
  decision, SPEC §14). Published outputs are unchanged; an independent review
  found no way to change a result and its follow-ups are applied. The
  rectifier workload runs at 0.78 (one worker) and 0.88 (four) of its prior
  time; `ppml-scientific` passes on 297fc495.
- FE certificate group means use one compensated pass over values shifted by
  each group's first member (declared roundoff-level change, scope file
  `tests/replay/scopes/2026-10-09-certificate-one-pass-group-means.json`:
  FE-certificate norm cells only; all coefficients, covariances, residuals,
  iteration counts and discrete decisions exact at one, two and four
  workers). Projections and CG keep the scaled two-pass kernel. Quiet Mac
  timing passed on all 18 frozen workloads; the replay baseline is now
  d4b68d26 (SHA-256 prefix 4c1a432f).
- Arithmetic-preserving: contiguous gathering in the strided score-Gram
  certificate (129-column robust case 0.83/0.88) and four-row tiles for the
  dense covariance meat (0.92/0.93).

The hardware (FMA/AVX) instantiation of the new group-sum kernel has not
been run on an FMA host; Rosetta reports no FMA. A branchless Neumaier
update measured within noise and was not kept. Remaining measured
candidates: streaming the certificate's group sums by row (bit-identical in
principle, needs admitted per-level workspace), in-place row compaction for
samples with drops, and the three-FE LSMR path inside the pinned `within`
dependency. PPML time is spread across many kernels with no line above 3%.

## 2026-10-09 Windows FMA kernels and plugin module pin

Bounded Windows `stata-do` runs (Windows Server 2022, x86-64 with FMA/AVX,
Stata/MP 19) through the owner-controlled controller. Each run stopped the
instance, deleted transient objects and released the lock.

- `win-20261009T194113Z-25d68772`, commit `a5df8fdc` (source archive SHA-256
  `4f40b488a09009b2e5250d75753f2f2fb7801303971fbef032695c0e44de8cf9`,
  receipt `ae74b6447af0af26ceb4ed177dc5eddba31442842f77e41cc5aba000d02914ec`):
  **fail** at `native_loader`, native return code -1073741819 (0xC0000005).
  `exact_fma` and `fused_kernels` passed first. From this commit a skipped
  hardware path fails that stage, so the FMA instantiations, including the
  one-pass certificate `group_sums` kernel from d4b68d26, matched the
  portable bits on hardware.
- `win-20261009T195813Z-b1b85f9d`, the same commit (archive
  `dce94296763b061754cc6e25ca039b218e92b14a64207b5be96f93a2d610984a`,
  receipt `74a19c1239b9fcdcc2f081570e97d0243eb8566a4565242dff757fd3670a3987`):
  **pass**, all stages and licensed Stata. The crash was intermittent; the
  harness code paths were unchanged since the last passing run.
- Cause: the loader harness calls `FreeLibrary` milliseconds after doctor
  starts the cached 32-worker Rayon team on a 2-vCPU host, and idle Rayon
  workers spin through 33 yield rounds before sleeping, so a worker could
  still run unmapped plugin code. d5eb37a7 pins the module on the first
  action and makes the loader test assert that the module stays mapped
  after release.
- `win-20261009T201439Z-a3950702`, commit `d5eb37a7` (archive
  `8b3df75d00402686469a45f0b92061a7a5b245edfece3fec16d0b0694a156325`,
  receipt `16a0a14d9e4a3ff38ba9a665b11facf9a0e82c8af01c09c1048ca08d5ed977de`,
  plugin `c3783aec8de5851866f7590806a8899a18dbb457f2508ac133b082399ed2de13`):
  **pass**, including the mapped-after-release check and licensed Stata.

One passing run after the fix bounds, but cannot exclude, a residual race of
the same kind; the mapped-after-release assertion is deterministic. Mac:
`scripts/run_tests.sh`, Windows-target clippy for both gates, the 73-case exact
replay (zero differences) and the `quick` Stata profile pass on d5eb37a7. Linux
was not run for these commits.

## 2026-10-09 target cleanup

`scripts/clean_target.sh --apply` reduced `target/` from 35 GB to 8.7 GB,
leaving Cargo's build cache and the replay baseline (d4b68d26, SHA-256 prefix
4c1a432f). The ignored captures that earlier sections describe as retained
under `target/`, including `target/integration-checkpoint-2026-10-09/`, were
deleted; the hashes and summaries recorded in this file are the remaining
record.
