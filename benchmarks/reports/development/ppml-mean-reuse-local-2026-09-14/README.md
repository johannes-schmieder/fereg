# P4 accepted/trial mean reuse: evaluated and rejected

Reject the standalone prototype against evidence HEAD `ebf038f` and accepted
native `7739e8e`. Reusing already computed Poisson means saves work, but the
measured changed work is too small to substantiate the prospectively required
2% and 10 ms complete-command gain. The candidate and diagnostics are archived;
all 178 accepted source inputs are restored. No mean cache is adopted.

## Mechanism and scientific boundary

The prototype attaches bounds-validity tags to the existing accepted and trial
eta/mean pairs. Initialization has no valid mean. Successful line-search means
can be reused after acceptance; validity moves with the paired buffer swaps.
Different bounds or failed mean construction force the original domain and
exponential checks. Trial validity clears before partial line-search writes or
strict-candidate scratch reuse. Changed outcomes, prior weights and offsets
still rebuild working weights, responses and scores. No weighted design, rank,
topology generation, convergence certificate or public solver policy is cached.

Each valid reuse avoids one N-value eta read, one N-value mean write and N
exponentials/domain checks. It does not remove the working-state or independent
certificate passes. These are source-derived logical costs, not hardware memory
traffic. The actual arm64 workspace size is 336→416 bytes: two inline 40-byte
`Option<PoissonBounds>` fields, with no new vector or heap allocation.

## Source identity and validation

[Identities](built-identities.json) bind 178 inputs and isolated universal plugins:

- Accepted baseline: `2689faf0418896ac66f0cb7fcaa9552b339a1928ab38409b43710dab6f63faeb`.
- Rejected candidate: `8a85e77cb0bd915cac543c54767e581652c980e4d046b958901efa3b5c9cbe9b`.

The [patch](implementation.patch), [source archive](source-archive.json), script
hashes and log hashes preserve the tested candidate. Core/plugin were forcibly
compiled for both architectures. Universal/static and actual arm64 ABI checks
pass. Main and installed dist were not built or substituted into comparisons.

All 58 focused core tests and 95 serial plugin tests pass. Three new tests cover
changed working inputs with unchanged eta; repeated bounds failures and recovery;
partial eta, coefficient and mean writes; acceptance swaps; strict-candidate
scratch reuse; and fast/low capacity invariance. Fresh exponential/working-state
comparisons preserve exact values. Existing backtracking, strict rollback,
checkpoint movement, weight identity and invalid-warm fallback tests remain.
These candidate tests are not presented as baseline red/green regressions.

Ten independent licensed fits per arm pass with process status, explicit r(0)
and terminal fixture sentinels: three K8 unit/FW/PW Poisson/Hessian/sandwich fits,
one generated-factor fit and six K2 unit/FW/PW robust/cluster fits. See
[validation](validation-summary.json). No scientific tolerances or seeds changed.

## Untraced complete-command screen

The [protocol](protocol.md) fixes four original full/warm requests and requires
both 2% and 10 ms command gain with commensurate changed-phase savings. Rotated
arm order, fresh local M2 Ultra Stata/MP processes and unchanged inputs are used.
Builds, tests, oracle calculations and archive compression do not overlap timing.
These four single pairs are screens, not repeated gain estimates.

| Case | N / finite K / workers | Command seconds | Change | Ordinary weight/response refresh seconds |
|---|---|---:|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 7.600 → 7.251 | -4.59% | 0.05679 → 0.05068 |
| FW exposure, missing, robust | 245760 / 50 / 4 | 2.458 → 2.375 | -3.38% | 0.02140 → 0.01921 |
| PW offset, missing, clustered | 245760 / 50 / 16 | 2.198 → 2.177 | -0.96% | 0.02071 → 0.01896 |
| Complete one FE, one control | 983040 / 1 / 4 | 1.446 → 1.432 | -0.97% | 0.05673 → 0.05192 |

Complete/FW pass the time-only screen. However, ordinary refresh saves only
6.104/2.189 ms. Unchanged finite work falls 217.735/51.680 ms; strict/final work,
which also contains mean refresh, falls 111.749/15.110 ms. PW and narrow miss
the time gate. No untraced pair exceeds both 5% and 10 ms added cost.

All scientific exports, sample hashes and discrete solver fields are exact.
Outer/inner counts stay 3/5 for complete/narrow and 4/6 for FW/PW, with zero
step halvings, strict profile rejections or fallbacks. All native peaks and
models are exact and admitted: 2.522/2.963 GB complete, 633.45/745.43 MB FW,
645.21/935.51 MB PW and 1.240/1.372 GB narrow (observed/model).
Sampled process RSS changes 1.9618→1.9594 / 0.5258→0.5300 / 0.5433→0.5408 /
0.7705→0.8202 GiB. In particular, narrow RSS rises about 51 MiB despite exact
native peaks; this is not a memory-saving claim.

## Direct attribution and disposition

The [diagnostic protocol](diagnostic-protocol.md) was recorded after the screen
and before three separate instrumented pairs. Both diagnostic artifacts have
their own [identities](diagnostic-built.json); the only added clock surrounds
mean construction/reuse. Every initial, ordinary, strict and final refresh is
captured. Warm-up calls are retained and separated by compact row count.

| Full fit | Refresh calls / reused calls | Mean construction total, ms | Saving, ms |
|---|---:|---:|---:|
| Complete | 5 / 4 | 14.863 → 3.139 | 11.724 |
| FW, 245015 retained rows | 6 / 5 | 4.349 → 0.735 | 3.613 |
| Narrow | 5 / 4 | 14.740 → 3.157 | 11.583 |

The entire baseline mean-construction scope is only 0.20%, 0.18% and 1.02% of
the respective untraced baseline commands. Its measured reduction cannot
explain a 2% command gain. Instrumented command times are retained but excluded
from performance aggregates. All diagnostic science and solver decisions match
their corresponding original artifact and the opposite arm exactly.

Reject the standalone cache rather than attributing unchanged finite-solve
variation to it. No further seeds, public holdout timing or private allocation
audit followed this finding. Low-memory/error-path coverage is focused core
validation, not a public performance claim. A read-only phase display initially
failed on an unavailable value; [collector notes](collector-notes.json) preserve
the error and correction without altering or rerunning measurements.

[All phases and memory](summary.csv), [screen results](screen-summary.json),
[direct counts](diagnostic-summary.json) and [14 captured commands](captures.json)
retain the evidence. P4's broader pass/state items remain open. Next compare the
existing weighted Schur machinery with actual PPML joint LSMR/MAP/CMG costs;
avoid another small row-pass experiment until its measured share warrants it.
Installed dist, unrelated work and historical receipts are preserved. No push,
agents, SCC jobs, upstream changes or R1–R5 work occurred. This is local
development evidence, not release, cross-platform or external-comparator evidence.
