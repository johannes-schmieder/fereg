# ADR 0188: canonical PPML cost attribution and Schur scheduling closeout

Status: scheduling prototypes rejected; local development decision, 2026-09-25.

## Decision

Keep the production coefficient solver and serial explicit-Schur construction
from `ad776bd5`. Do not adopt the full-square worker-scratch or disjoint-output-row
prototypes in this experiment. Preserve their source, science and timing receipts
in the [source-bound report](../../benchmarks/reports/development/ppml-canonical-cost-local-2026-09-25/README.md).
No numerical policy, tolerance, memory admission or automatic route changes.

The first prototype's scratch requirement excludes the representative target:
its reduced matrix has 65,536 elements, exceeding an N=30,720 worker bank. Its
public result cannot adjudicate an active parallel kernel. The successor owns
disjoint rows of the existing matrix, removing that restriction without another
bank or reduction. It preserves each entry's serial group/basis summation order.
Independent dense-product tests and explicit serial comparisons pass, including
matrices larger than N, empty groups, changing numeric blocks and uneven panels.

The active successor reduces diagnostic Schur construction from 4.343 to 3.144 ms
across five current-weight frames, but the public target median remains 78 ms.
Its six-case block has seven paired wins, eight losses and three ties. A small
kernel gain with no established whole-command gain does not justify retaining
another sparse traversal implementation. Do not tune panel sizes, scratch
thresholds or case guards to rescue this experiment. This decision does not
reject coefficient-space solving or mathematical Schur elimination.

## Attribution boundary

The PPML Preparation phase ends before the first canonical weighted-linear
solve. It must not be described as entirely canonical-rank construction.
Nested native spans are inclusive; recursive parallel child sums are worker
activity rather than elapsed wall time. Diagnostic builds remain separate from
the pinned stock comparator and production headline timings.

The initial and strict canonical construction calls total about 21.6 ms on the
two-slope diagnostic and 67.5/85.4 ms on the larger three-FE fast/low diagnostics.
The latter use compact products and do not form the explicit Schur matrix.
Consequently this scheduling experiment cannot address their larger repeated
construction cost. Original-column normalization, means, materialization and
residual projection are distinct next boundaries; the profile does not itself
authorize eliminating any certificate or changing convergence policy.

## Scientific and memory authority

Both prototypes retain current-weight numeric preparation, explicit generation
invalidation, independent original-row projection/finite-score checks, canonical
rank and bread, reconstruction, objective, final Hessian and atomic publication.
All 36 headline baseline/candidate pairs have byte-identical coefficient,
covariance and fitted-mean exports. The final prototype has 123 distinct focused
Rust passes and the previously recorded baseline checkpoint-count assertion
failure. Ten licensed oracle executions pass; the failure is not repaired or
relaxed. All application, independent public-state and comparator audits pass.

There is no modeled storage addition; headline selected, scratch and tracked-peak
deltas are zero. Stable unit capacities and these peaks do not replace a real
warmed allocation audit or physical-cap qualification, neither of which was
expanded after the public trial failed. Both prototypes are removed from
production; their source remains immutable evidence. The accepted source inputs
remain identical to the previous checkpoint.

The broader multi-FE 0.90-times-stock objective remains open. Frozen definitions,
known constrained-fit/CG failures, static allocation-inventory drift, deferred
platforms and stopped campaigns remain unchanged. Current implementation
priorities belong only in STATUS.md.
