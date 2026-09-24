# ADR 0160: partition robust PPML inference by matrix work

Status: accepted for local development, 2026-09-24.

## Decision

Partition the existing robust score reduction by both observations and finite
rank. A block targets at most 2^20 products across the two independently
accumulated upper triangles, with at most 16,384 rows. Retain the existing
64-block and 32-MiB partial-bank limits. Use the same partition for every
requested thread count. Clustered PPML and all OLS reductions are unchanged.

The existing estimator-neutral reducer still constructs separate production
and certificate meats, performs their separate compensated merges in opposite
orders, and forms and checks both sandwiches. PSD, independent publication
inspection, Wald, current-weight score and final-Hessian certificates are
unchanged. This changes a deterministic reduction tree, not its scientific
acceptance or tolerances. Prior- versus frequency-weight semantics remain intact.

## Why and memory boundary

The old 16,384-row policy supplies one task at N16,320/K64 and two at
N30,720/K32, despite four/eight requested threads. The pinned stock comparator
partitions upper-triangle entries across threads. Independent diagnostic clocks
attribute about 15.7 and 5.7 ms respectively to our two meats. Work partitioning
reduces those spans to about 5.1 and 1.7 ms. Covariance products and eigensystem
checks retain their cost; the latter must not be removed to claim this gain.

For admitted width K, one partial occupies 16*K*(K+1) bytes. Admission adds
min(requested partial bytes, max(32 MiB, one partial)) to the prior robust
inference envelope without taking credit for its existing square allowance.
This is conservative across every smaller final numerical rank: both the
uncapped block count and stride increase with rank, while capped allocation
cannot exceed the larger of the byte budget and one partial. Checked arithmetic
refuses overflow. Runtime allocation remains fallible. The reducer's existing
single-partial exception to its byte cap is explicitly covered.

The diagnostic K32 bank grows 33,792→523,776 bytes; K64 grows
66,560→4,259,840. Observed whole-fit tracked peaks are unchanged in the public
screen because earlier phases dominate. The widest selected memory forecast
increases by 2,658,799 bytes. This is not physical-memory qualification or a
repair of the documented static allocation inventory drift.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-work-balanced-inference-local-2026-09-24/README.md)
preserves every public timing, losing pair, source map, diagnostic and oracle.
All 104 focused Rust tests and five licensed fixtures pass. The new independent
scalar covariance oracle checks T1/T4 identity; the explicit-dummy Poisson,
Hessian and sandwich fixture covers unit/FW/PW and robust/clustered inference.
All 120 public applications, 108 public PPML states, 36 native PPML pairs,
36 direct stock comparisons and six OLS pairs pass. Coefficients and fitted
means remain exact between native variants; covariance has tiny rounding
changes inside the unchanged envelopes. OLS scientific exports remain exact.

Development wide medians improve 93→89 and 152→143 ms. Fresh two FE
N20,160/K20/T4 improves 104→100 ms, still above stock's 88 ms. Fresh slopes
N20,160/K16/T4 are 112→109 versus stock 155 ms, with mixed paired changes.
Narrow and low-memory controls include small noisy losses; there is no universal
speedup claim. The modest code and measured kernel improvement justify
adoption without further threshold tuning. Seed 202609373 is consumed/frozen.

The user has narrowed further optimization to at least two FE partitions.
Retain this shared improvement and stop one-FE-specific development. Existing
scientific/platform deferrals and frozen benchmark definitions are unchanged.
