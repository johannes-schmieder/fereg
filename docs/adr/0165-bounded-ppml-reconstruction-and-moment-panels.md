# ADR 0165: bounded PPML reconstruction and scratch moment partitions

Status: accepted for local development, 2026-09-24.

## Decision

Joint weighted-linear reconstruction borrows directly accessible original
columns and updates disjoint 1,024-row panels on the existing worker team.
Each residual retains its selected-column FMA order, normalization and initial
FE contribution. After eliminated-effect projection, independent fresh finite
scores scan original columns concurrently, each retaining its observation order.
Incomplete direct-column access leaves the existing dense/recipe path intact
before modifying any residual. No normalized N-by-K bank is introduced.

Retained scratch moments can partition observations rather than Gram rows when
the existing tile workspace supports it. Each partition reads and standardizes
its rows once, forms one complete upper-triangle Gram/RHS, then participates in
the existing compensated merge in partition order. Scratch remains immutable.
This applies to eligible retained weighted-linear scratch consumers, including
categorical and slope PPML. Dense/implicit moment sources and one-shot OLS keep
their existing computation.

For D selected columns, S stored columns and B already admitted tile elements,
one partition uses D²+D+S elements. P is the minimum of B divided by that stride,
the existing partition count and N. Require P>=2. Also require

```
ceil(N/P) * (D*(D+1)/2 + D)
    < N * (L*(2*D-L+1)/2 + L),
```

where L is the existing number of Gram rows assigned to the first worker.
The inequality compares conservative multiply-add critical paths, not measured
wall time. Storage refusal or insufficient work advantage retains the existing
Gram-row method. There is no benchmark identity, favorable seed, new width cap
or extra memory allowance in admission.

## Arithmetic and scientific authority

This supersedes ADRs 0020/0138's retained scratch accumulation organization only
on the admitted row-partition route. The ban on an independently allocated
`partitions * K²` bank remains: partial Grams occupy the existing charged tile
bank. Dense and declined scratch calculations retain their scalar order.
The new scratch sum has a fixed tree for a fixed resolved team and dimensions;
its bits need not equal the former full-observation recurrence or another team.

Each row uses the same normalized weight, selected identity, scale and FMA
products. Consecutive observations form each partial, and compensated merging
reduces their sums in a deterministic order. Tests compare with explicit scalar
products using the ordinary forward-error bound for both accumulation trees,
with Cauchy bounds on absolute products. This replaces bit equality only for
the reordered kernel tests; no runtime or scientific comparison tolerance changes.
Original scratch preservation, I/O bounds, wide refusal and dense exact-order
tests remain.

Reconstruction-only public results are bit-identical to the baseline in the
development block. Combined results pass independent projection/rank kernel
checks, constructed weighted Poisson truth, explicit Poisson/Hessian/sandwich
fixtures and public original-design score/model-space/objective audits.
Fresh coefficient differences are at most 1.102e-16, V 1.001e-17 and means
2.132e-14 in the measured block. These observations do not replace runtime
certification.

Current-weight FE projection, model-space, rank/omission policy, finite solve,
inverse, original-input scores, both outer convergence passes, objectives,
reconstruction and final-Hessian synchronization are unchanged. No certificate
is removed or rescheduled. Weighted geometry, working responses, norms and
products gain no authority across frames. Sample, weight semantics, DoF,
separation, fatal errors, rollback and transactional publication are unchanged.

## Memory and execution

There is no additional explicit heap bank, scratch file, worker team or route
admission charge. Disjoint partial slices fit B; one immutable scratch reader
per active partition uses the established 8-KiB encoded page, replacing the
former worker's same-sized shared-read temporary. Reconstruction borrows columns
and uses scalar loop state. Workers operate only on Rust-owned data; SPI and
interruption boundaries remain on the main thread. The safe core remains safe.

All 71 final-candidate development/fresh pairs have identical modeled selected
bytes, tracked peak deltas, scratch bytes, outer iterations and reported inner
iterations. Explicit warmed allocation fixtures pass. This is not a physical-cap
or cross-platform qualification claim; static allocation-inventory drift remains.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-joint-panels-local-2026-09-24/README.md)
preserves both candidates, 102 distinct focused passing Rust tests, six final
licensed invocations, all timings, controls and initial failures. A broader joint
test selector reveals an identical existing checkpoint-count mismatch on both
the accepted baseline and candidate; it is recorded rather than silently fixed.

Fresh wide low PPML improves 146 to 121 ms (17.1%), unbalanced low 104 to 95
(8.7%), and low slopes 98 to 92 (6.1%). Every pair in those families improves.
The gains justify adoption with unchanged storage and a small implementation.
Excluded narrow fast PPML loses 53 to 54 ms and fast unbalanced three FE loses
90 to 92; the latter confirms a small development loss. The cause is not
established, and no universal non-regression claim follows. OLS exports are
exact; categorical medians improve and slope medians tie.

The <=0.90-times-stock goal remains unfinished. Current wide and unbalanced low
ratios remain 1.360 and 1.508. Their residual costs are substantially larger than
the remaining few milliseconds in the slope/fast-three-FE anchors. Do not tune
this partition rule on consumed seed202609377; continue with the current-weight
projection lifecycle, measuring setup, repeated row passes and recurrence work
before changing another solver or certificate boundary.
