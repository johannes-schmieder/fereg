# P4/P7 retained dense moments: adopted local checkpoint

Adopt [ADR 0054](../../../../docs/adr/0054-retained-dense-moment-row-standardization.md).
Retained dense finite moments now standardize each selected observation suffix
once per Gram-row partition and reuse those values for its cross-products.
The fixed 64-double local cache applies to scaled designs with at most 64 active
columns when the existing layout assigns more than one column per partition.
The final partition may contain one column. Wider, unscaled, single-column
partitions and scratch sources retain their previous loops. One-shot OLS
prepared-moment sources and all rank/routing policies are unchanged.

Each Gram/RHS entry retains its exact observation order, weight normalization
and fused multiply-add expression. Selected columns use their actual stored
mapping. Original reference scales, current-weight norms, independent score/
inverse/dropped-span certificates, retained QR and strict basis transactions
remain authoritative. This changes repeated arithmetic and memory access,
without transferring scientific state between weight frames.

## Cost attribution and source identity

The [diagnostic profile](profile-summary.json) identifies a dominant Rayon leaf
under retained frozen finite moments. Its 15,076 top-of-stack samples are a
sampling count, not CPU percentage or a complete phase timer. Source inspection
finds repeated checked division of the same right-column value inside each
cross-product row. For K=50, the previous loop performs 1,325 column
standardizations per observation across partitions. The cache reduces that to
122 at four workers or 338 at sixteen workers. All 1,275 Gram and 50 RHS FMAs
remain. These are source-derived operation counts, not hardware counters.

There is no new full data pass, N-by-K copy, heap allocation, solver setup or
retained Gram partial bank. The cache payload is 512 stack bytes per partition,
conservatively bounded by 32 KiB across eligible partitions. This is not a
measured RSS increment or a heap-memory saving. A separate non-inlined helper
keeps the array outside wider/unscaled calls.

Accepted native baseline is `6ad8b201b73dcc1e550827a6b713011288292910`;
main before this experiment is evidence-only
`25034ab3a1a613e1e0a423c682d38aca381c2fee`. [Identities](identities.json)
bind all 152 native inputs, the exact [implementation patch](implementation.patch),
locked dependencies, build and isolated universal plugins. Every baseline input
matches the accepted revision; every candidate input matches the adopted source.
The candidate plugin is
`ad39e2329579cf87295796379e12035805d923a3417b8021af37df0a198322f6`.
The changed crate visibly compiled for both architectures. The installed plugin
was neither the baseline nor replaced.

## Complete-command, phase and memory evidence

The [prospective protocol](protocol.md) requires at least 5% and 20 ms median
paired command savings on a primary, with savings in changed finite work.
The first screen authorized two further seeds for each of the three targets.
All completed observations remain: **16 pairs / 32 commands**, comprising nine
public primary pairs, six controls and one separate private allocation audit.
Fresh local M2 Ultra Stata/MP 19 processes perform a small untimed warm-up;
arm order rotates. Affinity is uncontrolled. Builds, tests and statistical
sampling did not overlap timing. These are local development results.

| Public target | N / K / workers | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 9.540 → 7.440 | −22.0% |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 3.180 → 2.446 | −24.0% |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 2.944 → 2.253 | −24.4% |

Every primary pair improves. Finite-solve phase medians fall
2.5602→1.2917 / 0.8772→0.3813 / 0.8441→0.3952 seconds for complete/FW/PW.
Strict/final-solve medians fall 3.3502→2.4768 / 0.9161→0.6795 /
0.8801→0.6355 seconds. Projection medians stay close at 1.0236→1.0252 /
0.4404→0.4373 / 0.3358→0.3322 seconds. Preparation, response refresh and
inference remain close. The saving is supported by the changed moment consumers;
not every fluctuation in an unchanged phase is attributed to this change.
Overlapping and independently computed phase medians must not be summed;
median paired changes need not equal ratios of command medians.

All sixteen native allocation peaks are exact and fit their unchanged models.
Primary peaks remain 2522.61/633.49/645.34 MB. Primary sampled process-RSS
medians are 1.9576→1.9583 / 0.5315→0.5265 / 0.5439→0.5647 GiB.
In particular, PW RSS rises; there is no general memory reduction claim.
The archive retains all phase models, observed peaks and scratch estimates.

Single-pair protected command changes are weak two FE +3 ms, low FW −61 ms,
small one FE −3 ms, saved/separated +2 ms, explicit strong two-FE MAP −4 ms
and K=65 +10 ms (1.4%). None exceeds max(10 ms, 5%). The low scratch path's
timing reduction is a control observation, not proof that its unchanged loop
benefits from the cache. Discrete outer/inner work is exact, and all sixteen
pairs retain zero reported fallbacks, step halvings and strict-profile rejections.
QR correctness/failure behavior is separately exercised by focused tests;
no fallback-heavy performance gain is claimed.

The private PW allocation audit falls 7.817→5.429 seconds (−30.5%) and is
excluded from public aggregates. Both arms retain zero allocation events,
zero live/peak growth over sixteen audited iterations, stable linear/PPML
capacities and passing fresh/warm and allocation-free certificates.

## Scientific checks and disclosed failures

All sixteen full b/V exports are byte-identical. Samples, rank/status, DoF,
weight/cluster/separation structure and discrete solver decisions are exact.
Independent runtime certificates and saved-output comparisons pass.
[Comparisons](comparisons.json), [summary](summary.csv) and the
[checksummed numeric archive](archive.json) preserve every completed pair.

[Validation](validation.json) records 62 distinct candidate Rust tests in a
dedicated Cargo target. The new exact scalar-moment oracle covers 1/7/16/63/64/65
columns, 1/4/16 workers, selected/gapped storage, unit/provided weights, scaled/
unscaled designs, extreme finite scales and scratch controls. Selected NaN/Inf
fails closed. Existing retained fresh/frozen/strict, rank-deficient and above-64
QR, stale-weight norm, generation, worker-team and allocation-free rollback
tests pass. Both isolated artifacts pass the independent full-dummy Poisson/
Hessian/sandwich Stata oracle, with explicit rc=0 and terminal sentinels.
Universal static audits and the actual native arm64 ABI/self-test pass;
x86_64 scientific execution and cross-platform qualification were not run.

Three development failures remain disclosed:

- The initial new scratch fixture wrote NaNs in unselected physical cells.
  Scratch correctly rejected them. Only its unselected cells were changed to
  zero; dense unselected NaNs still detect accidental reads. Selected values,
  the scalar oracle and exact comparisons were unchanged.
- The diagnostic sampler captured one callgraph, then failed to read an
  exiting child's command line. Both identical-binary fits and science passed,
  but a second sample is not claimed. Their 10.449/9.641-second commands are
  excluded from performance adoption.
- The first K=65 control duplicated `vce(robust)`, failing its warm-up with
  r(198) before target invocation. The [failure receipt](failed-wide-control.json)
  binds original logs/request/driver. Only that duplicate option was corrected;
  the same shape, seed and arm order were rerun under a distinct prefix.
  The corrected pair passes. No failed result was overwritten or counted as a
  successful command. The first archive path scan also flagged its own pattern
  literals; only those validator literals are exempted from the final scan.

No tolerance, rank decision, certificate or public transaction was weakened.
Broader widths/scratch reuse, remaining predictor/construction/state passes,
P0 accounting and the remaining #27 packages stay open. Rejected raw-reference
and fresh-input certificate prototypes remain absent. Unrelated Veneto work
and historical receipts remain intact; no installed-dist replacement, agents,
SCC, push, external-comparator claim or qualification occurred.

Reproduction uses the existing `benchmarks/local_fixest_gap` helpers and isolated
baseline/candidate trees selected by `FEREG_LOCAL_BENCH_ROOT`. The portable oracle
driver reads `STATA_MP_PATH`. Raw licensed logs, datasets and binaries are excluded.
