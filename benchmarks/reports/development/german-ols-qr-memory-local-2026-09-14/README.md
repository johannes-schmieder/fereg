# German OLS: long QR reductions and macOS memory discovery

Local development evidence on macOS 26.6.2, Apple Silicon, 192 GiB RAM,
Stata/MP 19. This is not qualification or an isolated full-run speed comparison.

## Decision and source identity

Adopt the two focused repairs against local main
`0af4321e0003b97f9fcd0e1a81cb526ac0d37c1f`. The parent already contains the
[shared OLS singleton fix](../singleton-index-local-2026-09-14/README.md) and
[production PPML singleton fix](../ppml-singleton-index-local-2026-09-14/README.md).
The combined isolated universal plugin is
`bc1e8909de8a30bc99694be4b04e131f6eb5eed2849c3ee6f5d3040a1408b0ce`.
Its build label is `0af4321-qr-mac-memory`, dated 2026-09-14. The installed
plugin remains `4187efdf`; it was not rebuilt. No push or cluster work occurred.

[source.patch](source.patch), [built-identities.json](built-identities.json) and
[patch-replay.json](patch-replay.json) bind all 177 compiled inputs and the new
licensed fixture to the candidate. Raw licensed logs, data, estimates, rejected
binaries and stack samples remain local; [external-evidence-hashes.json](external-evidence-hashes.json)
identifies them without distributing them.

## Repairs and unchanged boundaries

QR segments of at least 65,536 entries use a fixed maximum scale and compensated
squared-value sum for reflector norms, and compensated serial reflector dot
products. Existing parallel dot reductions and short checkpoint arithmetic remain.
Low-memory QR now rejects an unrepresentable transformed leading value immediately.
Bread errors include rank, both inverse errors and the unchanged tolerance.
Raw pre-absorption reference scales, rank thresholds, weights, covariance,
convergence/publication certificates and memory capacities are unchanged.
There is no new observation-sized state.

macOS availability now reads free and inactive pages through `HOST_VM_INFO64`.
The former `vm.page_inactive_count` sysctl is absent on this OS and had silently
contributed zero. Only the documented original field prefix is required, so older
kernel structure revisions remain usable. The host send right is released after
each call. Mach includes speculative pages in its free count; they are not added
twice. On API failure the conservative sysctl fallback remains. Checked arithmetic,
the planner's existing 20%/minimum headroom and explicit test-cap behavior remain.
Linux and Windows detection are unchanged.

## Focused verification

- Two power-of-two oracles fail under the old long reductions: a norm loses
  approximately 1.819e-12 of tail energy, and an orthogonal reflector's leading
  value misses its exact result by 7.276e-12. The candidate passes slice/matrix
  layouts and scales 1, 1e150 and 1e-150. An independent overflow example checks
  a nonfinite transformed head with finite tails.
- 71 OLS fit tests and 302 PPML core tests pass. These core runs precede rustfmt; formatting replay exactly matches
  the final QR source and the production algorithms are unchanged. The final
  combined source passes all 97 plugin tests without warnings, including
  memory-cap controls and live macOS statistics checks.
- The final universal binary passes its static audit and actual ARM64 ABI/self-test.
  Eight large weighted OLS native fits pass a built-in dummy-regression oracle
  in both memory modes, covering samples, ordering, DoF, slope covariance and
  residuals. Fourteen prepared PPML fits pass exact singleton/separation masks,
  reasons, weights, rank/status and covariance, with dummy Poisson checks.
  No x86 runtime or Windows scientific result is claimed.
- An independent C probe reports 136,663,531,520 available bytes versus the old
  query's 80,077,963,264: 3,453,709 inactive pages of 16,384 bytes were omitted.
  Under the restricted invocation, the old free-page sysctl also fails, while
  Mach succeeds. This can make historical memory admission and routes differ.
  The earlier full run's exact available-memory value was not published.

The QR-only candidate passes German subsets retaining 132,691 and 338,054 rows
in both memory modes. Its largest slope difference versus saved pinned reghdfe
results is 1.103e-11. The combined candidate repeats the 132,691-row check with
exact low/fast masks and the same numerical differences. These are single
checks, not paired performance estimates. See [german-subset-summary.json](german-subset-summary.json).

## Full German result

The original all-field import and specification are retained: 2001–2009,
quadratic/cubic age profiles by education, year indicators, worker/establishment
FEs, CMG requested, eight native threads, low memory, poolsize 10, and default
recursive singleton removal. No artificial memory-limit override, external
pruning or sample trimming is used. The comparator is the owner's completed
reghdfe result with poolsize 5, compact and default MAP/CG at tolerance 1e-8.

| Quantity | Result |
|---|---:|
| Input rows | 91,045,290 |
| Singleton drops | 2,169,026 |
| Retained rows, both estimators | **88,876,264** |
| Finite rank | 18 |
| Absorbed DoF, both estimators | 14,702,025 |
| Residual DoF, both estimators | 74,174,221 |
| Maximum absolute difference, 18 slopes | **2.56893e-11** |
| Maximum absolute difference, 18-by-18 covariance | **5.67810e-20** |
| Relative RSS difference | **7.28563e-13** |
| fereg command | **1,708.530 s (28.48 min)** |
| Saved reghdfe command | **6,699.551 s (111.66 min)** |
| Native preparation, including singleton pruning | **28.717325 s** |
| Native input / native work | 7.864469 / 1,681.195676 s |
| Process wall time, including original script | 1,844.90 s |
| Process maximum RSS | 78,482,391,040 bytes |
| Native allocation peak | 44,552,096,322 bytes |
| Scratch file | 13,509,192,128 bytes |
| Swaps | 0 |

All common published coefficient/covariance coordinates, including the constant,
have the same maxima as the slope comparison. Native rank, score, bread, IID and
determinant checks pass. The two finite inverse errors are 9.99e-17 and 1.02e-16,
versus the unchanged 5e-10 tolerance. Successful publication, the full retained-count
assertion, saved estimates, comparison assertions and terminal sentinel all exist.
A fresh full reference fit and a full reference sample-mask comparison were not run;
small exact-mask oracles and the prior full retained-count audit are separate evidence.

The command-time ratio is 3.92 in this single comparison. It is not a universal
speedup, and cannot be attributed to these two repairs alone. The previous failed
full fit used CMG; this successful fit uses certified CG fallback. The independent
power-of-two tests isolate the QR bug, while these full runs do not isolate QR
performance or establish its sole causality. The run includes one recorded
one-second stack sample. [full-success-summary.json](full-success-summary.json)
contains exact counters and certificate values.

## Fallback and memory limits that remain

Initial absorption reports three CG fallbacks, 197 as its reported maximum
iteration count, 2.119433 s setup and 1,136.137758 s solve time. Final residual
absorption reports one fallback, 188 iterations, 0.317834 s setup and 132.160195 s
solve time. These iteration fields are not a sum of physical iterations. The
remaining native work includes QR, reconstruction, moments, scores and inference;
no isolated QR phase time is claimed.

The actual planner sees 124,508,012,544 available bytes and keeps 24,901,602,508
bytes of headroom. The selected execution estimate is 36,419,167,464 bytes; the
observed native peak is 22.33% higher than that base estimate, but within the joint
execution/solver budget. The public base estimate is not a complete action bound.

A source-bound probe of pinned CMG's existing conservative model gives
728,795,368,672 bytes of build peak plus 2,393,335,520 bytes of caller graph vectors
for these dimensions. This cannot fit the joint plan and explains the source's
fallback decision; the public result confirms CG but does not publish its textual
reason. Tightening that overly broad bound without weakening admission is a next
P0 target. No CMG sizing or hierarchy changes are adopted here.

## Preserved rejected and failed attempts

The first QR-only full attempt clears preparation in 28.242879 s, then returns
r(909) because a 23,495,238,160-byte CG fallback cannot fit the old restricted
availability estimate. The failed command takes 179.396 s; process wall time is
313.99 s, maximum RSS 51,572,899,840 bytes, with no swaps or saved estimates.
See [full-retry1-summary.json](full-retry1-summary.json). It does not test the QR
repair because the numerical stage was never reached.

Unconditional short-vector compensation moved a protected one-shot/retained
near-collinear comparison just outside its existing 2e-12 envelope. A retained
Givens trial worsened the comparison and was restored exactly. No fixture tolerance
was relaxed. A first Mach draft required the entire newest libc layout; the live
probe caught the older kernel revision and led to the checked-prefix handling.

Two early test invocations produced zero matching tests: one selected main's
manifest with an external target; another reused a stale cross-snapshot binary.
Neither is validation. Accepted checks use a dedicated target and explicit
candidate manifest. A final saved-result export initially targeted a directory
before creating it; no process ran, and the corrected read-only export completed.

German PPML still needs the outcome choice between imputed wage levels
`exp(logrwage2)` and the separate censored `rwage` variable. No German PPML fit or
reference success is claimed. The universal singleton repair is already adopted,
but the larger empirical goal and broader #27 work remain open.
