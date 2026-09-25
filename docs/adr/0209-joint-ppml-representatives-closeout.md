# ADR 0209: close borrowed canonical controls in joint PPML LSMR

Status: rejected production experiment, 2026-09-25. Local development only.

## Decision

Retain ADR 0208's original-control finite block and existing joint LSMR routing.
Do not adopt the borrowed canonical-control view or its one row-access correction.
Restore all 565 computational inputs to finiteblock1. The
[source-bound report](../../benchmarks/reports/development/ppml-representatives-closeout-local-2026-09-25/README.md)
preserves both implementations, diagnostics, tests, timing distributions and
scientific exclusions. No experimental runtime or dependent test code remains.

## Mathematical and ownership boundary

For retained representatives R = X - D A, a joint predictor R beta + D gamma is
equivalent to X beta + D(gamma - A beta). The finite coefficient axis is unchanged.
The prototypes borrow selected dense canonical columns, keep original controls
for independent current-weight certificates, and preserve the FE core-pair/Schwarz
preconditioner. Their finite Gram factors refresh on every weighted solve.
Prior-weight representatives are coordinates, not current-weight certificates.

The session validates sample/topology and kept generated-column/slot alignment.
Strict attempts reset speculative recurrence state before rewriting physical
representatives, even when accepted fit/basis authority later rolls back. This
implements the physical-bank distinction in ADR 0176; rollback alone does not
restore an old coefficient coordinate system. No new N-by-K bank, solver route,
tolerance, rank policy, check schedule, output contract or public option is added.

The first implementation resolves matrix indices inside arithmetic loops. The
single correction moves row slices and column positions outside those loops
without changing observation/FMA order. Both preserve current-weight FE/finite
acceptance, canonical rank/bread, score, objective, model space, reconstruction,
exact final q*mu Hessian, weight semantics and transactional publication.

## Evidence and rejection

The corrected identical-frame diagnostic reduces hard-factor iterations from
17/14/15 to 6/7/6. Current setup/solve/original-input acceptance call medians fall
from 90/57/86 to 52/46/43 ms. Numeric iteration counts are unchanged and calls
remain slower. Each diagnostic build has 102 passing certified solves. Replay
uses packed captured representatives and a captured reset sequence; it is not
the public baseline's complete outer history or a headline timing comparison.

The first public candidate has no hard-factor median gain and loses 11% numeric
and 17% wide. The correction improves hard-factor median 705 -> 643 ms (8.8%),
stock 709 ms, but all three stock ratios remain above 0.90. Numeric loses
578 -> 619 ms (7.1%); wide loses 225 -> 243 ms (8.0%). There are five paired wins
and 19 losses in its eight-case block. Outer iteration counts are unchanged.
The tradeoff does not justify production integration or another fitted selector.
No fresh validation seed is consumed; 202609440 remains available.

Both candidates pass 36 distinct focused Rust tests and five licensed fixtures,
including independent weighted nonzero-residual truth, Poisson/Hessian/sandwich,
rank, strict retry and rollback checks. Small public screens pass all 12 explicit
oracle arm checks. Across the headline and small screens, 192 public states and
64 baseline/candidate pairs pass. Six sparse stock mean comparisons fail and
remain unranked. The initial borrow-check failure and zero-frame diagnostic
capture/empty replay are retained; an empty replay is not scientific evidence.

Neither prototype adds a modeled heap bank. Corrected tracked peak differences
range from -2.52 MB to +1.95 MB and scratch is unchanged. This does not establish
RSS, warmed-allocation or physical-cap behavior. OLS was not newly benchmarked
after restoring the exact accepted inputs. Existing allocation-inventory drift,
constrained-fit/CG failures, scientific discrepancies and platform gaps remain.

## Consequences

This experiment establishes useful finite/FE decoupling and a costly borrowed
access pattern; it does not reject all FE-equivalent representations. Do not
continue selected-view tuning or revive the unconditional engine swap. A future
use must materially change ownership or traversal and justify complete-command
gains, rather than inheriting this candidate's iteration count or factor timing.

Proceed to canonical weighted-frame construction and its rank/bread/inference
handoff. Compare a justified three-FE direct-product extension with removal of
repeated normalization, materialization and product passes around the current
projector. Measure initial and strict work separately, retain independent
original-input acceptance, and carry the cross-stage large-N savings budget.
Current priorities and progress remain solely in STATUS.md.
