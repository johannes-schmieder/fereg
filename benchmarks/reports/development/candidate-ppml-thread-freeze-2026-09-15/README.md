# PPML automatic worker count: focused Linux repair

The clean `9ecf47d` baseline records **80 allocations in sixteen warm PPML
iterations** and fails the existing zero-allocation assertion. The minimal
candidate records **zero** and passes all 22 PPML plugin tests on the same
Linux compute node and bounded CPU affinity. It also passes 35 focused core
IRLS tests, including reconstruction, final-Hessian synchronization, failed
strict-candidate rollback, weighted specialized routes and interruption.
[Validation](validation.json) links the source, test-artifact and scheduler
identities. This is a correctness/allocation repair, not a command-time gain or
release qualification.

The driver preserves the caller's requested thread count in telemetry, then
binds its local execution configuration to the effective count used by the
admitted fit. Ordinary, fallback and strict linear frames therefore reuse that
count instead of repeating operating-system discovery. Linux automatic CPU
resolution can allocate depending on the calling thread's affinity and quota
environment. The diagnostic localizes five events per warm linear solve;
disabled trace-variable lookups allocate nothing. A standalone main-thread CPU
query also allocated nothing on another host, so no universal per-query cost is
claimed. The same-host baseline/fix test, rather than that isolated query,
establishes the repair.

The existing zero-event assertion is unchanged. Two added assertions preserve
public `threads=0` and the effective count. The change creates no observation
state, alters no numerical thresholds or input classification, and does not
replace the installed plugin. [The patch](implementation.patch) contains only
the driver binding and those assertions; broader memory changes are excluded
from this final test snapshot.

The [test inventory](tests.json) preserves hashes of the original failed logs
and passing checks. The first clean snapshot omitted a compile-time CSV fixture:
its 22 plugin checks passed but its later core compile failed. One narrow
snapshot repair supplied that committed fixture; all 35 previously unrun core
checks then passed with an explicit terminal sentinel. Scheduler failure for
the earlier attempt remains recorded. The earlier baseline, diagnostic and
broader-prototype tests are supplementary, not substituted for the minimal
source check. No large empirical estimation was run.

Whole-action coverage, public diagnostics, fixed-case command comparisons and
exact frozen candidate platform/install validation remain open. Mac builds
still require the owner's Xcode license acceptance; the German OLS review
adjudication is a separate pending scientific decision.
