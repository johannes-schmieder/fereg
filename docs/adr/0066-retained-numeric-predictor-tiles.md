# ADR 0066: retained numeric predictor tiles

**Status:** Accepted for local development
**Date:** 2026-09-14

## Decision

Fresh and strict retained weighted-linear solves may bind numeric/intercept-only
designs through the existing `NumericOlsColumns` guard: at least 4,096 rows and
8–64 generated columns. Accumulate the kept columns into the existing finite
predictor in 128-row tiles using the installed worker team. A fixed 64-entry
stack bank carries actual generated identities and coefficients in their original
coefficient-slot order. Each row retains the original scalar FMA sequence.

Generated recipes, narrower/wider designs and smaller samples use the existing
per-cell path. The guard applies to both resident and low-memory sessions; no
memory-mode exclusion is introduced. Ordinary frozen PPML already uses its
combined response-minus-residual predictor and is unchanged. One-shot OLS does
not use this retained-session reconstruction path.

The absorbed predictor remains `(response - residual) - finite_predictor`.
The independent cancellation-aware reconstruction certificate, reference scales,
rank/status ordering, generation validity, final-Hessian checks, and transaction
boundaries remain authoritative. The new helper adds no observation-sized heap
storage or new thread pool, and workers never call Stata SPI.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-predictor-tiles-local-2026-09-14/README.md)
records 5.7–6.3% median paired complete-command reductions on three repeated
primary targets. Separate exact-science diagnostics locate 82–90 ms savings in
the three fresh/strict predictor accumulations of the weighted targets. Native
peaks are unchanged and admitted; this is not a memory-reduction claim.

Low-FW timing is variable. The original +142/+171/+3 ms costs and two subsequent
large A/B flags remain in the evidence. Six interleaved identical-binary A/A
pairs range from -132 to +185 ms; six new A/B pairs have median -24 ms, and all
nine A/B pairs median +3 ms. These pass the predeclared median cost review but
do not establish zero overhead or a low-FW speedup. No observation was replaced.

This adopts a bounded predictor improvement. It does not close broader PPML
construction/refresh/certificate passes, weighted Schur comparisons, or state
invalidation work in P4, and does not alter automatic solver regions.
