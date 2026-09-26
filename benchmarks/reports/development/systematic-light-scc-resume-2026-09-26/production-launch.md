# Packed OLS/PPML production launch

Production array **7747518**, tasks **1–16**, was accepted on 2026-09-26 at
17:19:33 UTC and was queued at capture. Each task reserves 28 E5-2680v4 cores,
3G/core, with a 12-hour limit and array concurrency eight. The immutable plan
contains 556 remaining groups (270 OLS, 286 PPML); the 12 completed pilot
groups remain in place, retaining all 568 registered groups. Forecasts are
9.87–9.95 hours per task, 158.59 node-hours total. Queueing and runtime error
can increase elapsed completion time; deadline checkpoints preserve pending
whole groups for an audited continuation.

Estimator/controller 58284f76 and the frozen measurement fingerprint are
unchanged. Planner 78b2eb0b implements ADR0226 outside the frozen runtime.
The separate PILOT.ADJUDICATED receipt binds accepted smoke, complete pilot,
and clean exact-input independent adjudication 7747280. Original pilot
rejection, timeouts, reference discrepancies and failed pairs remain intact.
Diagnostic reference clocks informed future budgets only, never timing ratios.
No known ppmlhdfe initialization failure was retried in adjudication; production
uses the original frozen exclusions and registered comparison definitions.

Eight focused tests and actual-SCC read-only gate integration passed before
publication. The plan has 556 unique pending groups and preserves the original
four warm repetitions and eight cold anchors. Existing 120s pilot timeouts may
leave some pairs without four valid repetitions; they cannot be silently filled
from diagnostic runs. No production result or performance claim is available at
launch. Accounting, application sentinels and scientific outputs remain pending.

The existing campaign heartbeat remains active for collection, audit, valid
four-repeat distributions and pending-only continuation if needed. STATUS.md
is the sole mutable resume point. Local commits are not pushed.
