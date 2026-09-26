# ADR 0222: Packed lighter systematic development campaign

Date: 2026-09-26. Status: accepted for the user-authorized development campaign.

The stopped September 10–11 systematic campaign has useful reference timings
but is expensive and has little completed PPML coverage. Its frozen manifest,
controllers, results and stopped jobs remain untouched. This successor measures
estimator c31bcb621ba196e03a070ff2d808812733970f50 with a separately identified
controller. It does not qualify that estimator or change scientific contracts.

`manifest-light-v2.json` defines 140 configurations and 568 groups: four balanced
warm repetitions and eight cold anchors. Both models retain all eight designs,
three observation sizes and T4/T28; four designs also have T1/T8 at N983040.
Four designs retain K50, four small PPML designs retain zeros, and eight
configurations exercise two/three partitions with absorbed slopes and fast/low
native memory. No original frozen benchmark definition changes. IID OLS and
robust PPML remain; fitted-mean publication is not requested in this campaign.

The pinned stock default comparators remain. PPML additionally labels a stock
fixest arm with glm.tol=1e-10 and fixef.tol=1e-11. This changes options, not the
library. Default-reference discrepancies remain separate. Historical timeout
and startup evidence supports three narrow comparator configuration exclusions;
other seeds are budget exclusions, not newly observed failures. Slope APIs
without an established worker are explicitly unsupported, never emulated by
expanded dummy regressions.

Each group remains sequential and indivisible on a reserved 28-core E5-2680v4
node. Model/size mixing and historical plus pilot elapsed costs pack production
tasks toward 10.5 hours under a 12-hour allocation. Per-arm startup, estimation
and extraction caps determine whether a complete group fits before the deadline;
a smaller group may fill the tail. Atomic group and bundle receipts support a
new pending-only plan. Partial groups require isolated recovery, not cross-node
pairing. No active fit is intentionally carried across tasks. Private pinned
Stata reference copies avoid concurrent Mata compilation in shared packages.

Independent bounded explicit-design OLS and Poisson/Hessian/sandwich checks,
including slope rank and zero-group sample checks, gate the compute-node smoke.
A successful public command still runs all native scientific certificates.
Per-pair eligibility additionally requires exact input/sample identity,
coefficients, full covariance, reported objective, convergence and applicable
rank/DoF checks. A default ppmlhdfe failure does not veto a separately passing
fixest development pair. This is a new diagnostic evidence policy, not a change
to qualification's reference gates. Gtools count-only samples remain diagnostic.
Every reported summary requires all four valid warm repetitions. Distributions,
losses, missing groups, timeouts and scientific failures remain visible.

The setup, smoke, pilot and final evidence require scheduler accounting as well
as application and output checks. Smoke/pilot acceptance binds controller,
runtime, plan and receipts. Build/profiling work stays outside benchmark tasks.
No automatic retry of scientific failure and no solver tuning occur in this
campaign. The smaller OLS grid's historical command-time proxy is approximately
40.6% of the old grid's observed cost; sparse PPML history does not justify an
end-to-end twofold-speed claim before the new pilot.
