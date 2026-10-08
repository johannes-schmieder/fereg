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
