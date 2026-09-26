# ADR 0226: Continue production from an independently adjudicated pilot

Date: 2026-09-26. Status: accepted development continuation; not qualification.

ADR0225's separate SCC job 7747280 completed with clean accounting and exact
captured inputs. The direct QR/SVD oracle independently agrees with all three
original native results and all three longer-budget pinned references. Sample,
coefficients, full covariance, objective and applicable rank/DoF checks pass at
the unchanged campaign envelopes. The largest native coefficient difference is
2.81e-14. Fresh Poisson final-Hessian, score and nuisance-projection checks pass.
The original 120s reference budgets were insufficient: the unchanged reghdfe
call completed in 734.694s and the strict stock PPML calls in 322.253s/517.104s.
These clocks are useful for resource planning, never cross-job timing ratios.

The original pilot remains rejected under its original gate, and its failed
pairs remain failed/unranked. Repeating already completed native fits and the
nine already comparable pilot groups would add no necessary scientific evidence.
A separate continuation gate may therefore establish readiness for the remaining
production groups using the original receipts plus the independent adjudication.
This is a development-stage readiness decision, not a change to per-pair timing
eligibility, registered scientific accuracy, estimator behavior or qualification.

The new external planner verifies the frozen measurement runtime and accepted
smoke; the predeclared pilot plan and all completed groups; clean task accounting;
original native/reference result identities and numeric outputs; and recomputed
original comparison checks. Only the three recorded missing-reference failures
are eligible. Every one must be covered once by the source-bound adjudication,
whose input hashes, original-output hashes, new reference output hashes, clean
accounting, terminal marker and exact diagnostic source inventory are verified.
Recompute both native/oracle and reference/oracle comparisons. Require finite
score, rank-gap, final-Hessian and projection certificates with the diagnostic's
original bounds. Reject missing, altered, duplicate, foreign or partial evidence.

Write `env/PILOT.ADJUDICATED.json`, never `PILOT.ACCEPTED.json`. Bind its planner,
launch profiles and all consumed evidence. Deploy the planner outside the frozen
controller, which remains byte-identical. The production plan binds the new gate
and planner hashes; estimation still uses the original frozen measurement runner,
original estimator artifact, comparator libraries and options. All 568 registered
groups, samples, tolerances, output work, exclusions and four-repetition/eight-
cold-anchor policy remain. The 12 completed pilot groups remain in the original
results tree; only the 556 pending groups enter the first production plan.

Use the original packing and cost functions. Successful pilot timings and whole-
group elapsed time refine costs as before. The three adjudication reference times
may additionally update the exact configuration's cost estimate, raising future
predeclared command caps through the existing sixfold/headroom/2400s rule. This
changes resource forecasts only. Never rewrite earlier timeout bounds or use
these diagnostic calls to complete a four-repeat timing summary. Original failed
warm-1 pairs still prevent an aggregate if all four passing repetitions are
required; report that missing evidence explicitly.

First-production planning fails if unexpected production results already exist.
Subsequent recovery must audit terminal jobs and use a separate immutable plan
of whole pending groups; do not split or pair partial groups across nodes.
Reserve 28 E5-2680v4 cores, 3G/core, target 10.5h under 12h, concurrency eight.
No build, oracle or profiling work runs alongside headline fits on a node.

Eight focused local tests pass, including the independent oracle controls,
accounting failures/duplicates, missing fresh Hessian, rank/score defects, complete
568-group coverage, exclusion of exactly 12 completed groups, increased reference
budget forecasting, and refusal of already-started production. A read-only SCC
integration check validated all 12 actual pilot groups and all three adjudications
before any gate publication or production submission.
