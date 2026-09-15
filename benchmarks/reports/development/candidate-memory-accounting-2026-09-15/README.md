# Candidate memory accounting: development implementation, adoption pending

The source-bound prototype extends the archived compact solver-memory carrier
against the current native source. It remains uncommitted in the working tree;
[the patch](implementation.patch), [new carrier file](workspace_peak.rs), and
[identity](identity.json) preserve the exact tested snapshot. No complete-action
memory or performance claim is made and no installed artifact is replaced.

The implementation counts simultaneous storage by addition and sequential
attempts by their maximum. Unknown subscopes propagate. Reporting uses a compact
8-byte carrier and does not add observation-sized state or numerical-loop
instrumentation. Models and admission retain checked u128 arithmetic.

Implemented boundaries include:

- Borrowed MAP/CG capacities, coefficient conversion, independent certificates,
  CMG construction and lazy retained pools, wide Schur probes, and discarded
  attempts carried into the selected fit.
- Retained LSMR construction, preconditioner state, bounded lazy work pools,
  transport/result/certificate overlap, actual installed workers, and retained
  MAP-pilot/reference-norm parents. If the retained operator makes a pilot
  unaffordable, it is released before the cold pilot; certified later routes
  remain available. Numeric fallback retains the attempted model.
- Specialized slope MAP validation and workspace admission, including exact
  and one-byte-short checks before materialization and capacity rechecks.
- Generic owned solver construction and retained design/unit-weight overlap.
  These now produce accounting with or without a supplied budget; a supplied
  budget still controls rejection. An inference test exposed the former
  unbudgeted coverage omission.
- Inference score reconstruction carries its separate solver and failed-CMG
  construction peaks into the fit without changing scientific solver reports.
  Packed/strided/retained-scratch reuse avoids claiming an unused projection.
- Intercept saved-FE recovery carries retained-product, CG, SD/LSMR and MAP
  attempted envelopes into its result. Existing route-specific admission is
  reused; rejected unallocated routes do not contribute an attempt. Slope
  recovery remains explicitly unmodeled at this reporting boundary.

The initial combined snapshot passed 877 core tests but failed one of 95 plugin
tests. The [original result](initial-full-regression.json) remains failed; the
independently reproduced PPML automatic-thread allocation problem was repaired
and committed separately in `3f7ea55`, with its own baseline/fix evidence.
The full regression snapshot includes that repair and passes 878 core, 95 plugin
and 3 top-level library tests. Its terminal sentinel and scheduler acceptance are
recorded in [tests.json](tests.json) and [scheduler.json](scheduler.json). A later
CMG no-budget reporting follow-up passes 35 CMG and 35 PPML IRLS tests against
its [separate exact snapshot](cmg-no-budget/identity.json). These checks do not
bind to subsequent uncommitted memory changes. Focused inference test failures are
preserved: the first inadvertently reused a score cache, and the corrected
uncached test exposed the real missing unbudgeted generic accounting. A warning
from borrowing a mutable constant was removed without changing behavior.

This work does **not** close P0 or freeze a review candidate. Remaining work is
complete preparation/finite/inference/output/publication lifetime reconciliation,
slope recovery and PPML parent/frame coverage, versioned public action diagnostics
and their protocol/package metadata, original three-FE/German allocation
reproduction, the fixed alternating command comparisons, and exact-candidate
licensed platform/install checks. Public base-memory fields remain unchanged;
an incomplete model must not acquire a defined action-memory ratio.
