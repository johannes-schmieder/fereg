# Private retained finite-moment accumulators — SCC development evidence

Accepted bounded development improvement, 2026-09-28;
[ADR0243](../../../../docs/adr/0243-private-retained-moment-accumulators.md).

## Change and identity

Narrow scaled retained dense finite moments now accumulate each worker's Gram
and RHS rows privately and copy the completed rows once. This eliminates
per-observation writes to cache lines potentially shared by disjoint rows. The
selected columns, weight normalization, standardization and every ordered FMA
recurrence are unchanged. No reduction, certificate or scientific policy changes.

| Binding | Identity |
|---|---|
| Baseline main | `1383d0bc` (production package from `4ddf7aaa`) |
| Baseline package, 215 bound inputs | `14c753ede241d94812a25b52f06325cb81d5a08d5465efe0a343ec8b9c4bfad9` |
| Candidate package, 215 bound inputs | `a9fb23075ecbe4f2bc135c78e547d3aa301e49f50a281bdae61820e469edcc20` |
| Baseline Linux plugin | `d007491ab5dfb5aa659f6c5639c8ac90c7e2e6c2bcd2e2603216cb91cb812116` |
| Candidate Linux plugin | `f919ea96f5700c1ebbf0df9c2b7215d3b4a2e54f17c2578e3c55e4697efaff83` |
| Candidate Mac universal plugin | `57eb15d816abcbcd739dd8e96b68d1d64be831a743da3ffee9f26a591f9036ad` |

[Baseline](baseline-source.json), [candidate](candidate-source.json),
[selection rationale](candidate-selection.json) and [patch](candidate.patch)
preserve the exact computational inputs. Only `finite_wls_kernel.rs` changes.
The [separate accepted-source profile](../ppml-thread-cost-scc-2026-09-28/README.md)
provides attribution; it never supplies headline clocks. The largest sampled
Rayon helper contains finite-regression arithmetic; this is not proof of team
submission overhead. Cache-line contention is inferred from ownership and source,
not directly counted by hardware coherence events.

## Validation and measurement

[Mac](mac-validation01.json) and [Linux 7763797](linux-validation01.json) each
pass **90 distinct Rust tests and six licensed fixtures**. The existing moment
oracle is extended with weighted K6 and T2, covering T1/2/4/16, extreme scales,
selected columns, exact scalar moments, width boundaries and nonfinite failure.
The six fixtures cover independently constructed weighted Poisson/Hessian and
sandwich results, FW/PW/cluster and saved outputs, fast/low memory, categorical
PPML, multipartition slopes and OLS. Actual-plugin ABI/panic containment passes.
Linux also passes PPML smoke. Both license-fixture receipts, return codes and
terminal sentinels are checked. The legacy `ols::linear_session::tests` filter
matches zero tests and contributes zero to the count; the 90 are 61 IRLS, 26
finite-kernel and three one-FE profile tests. Thirteen rotation-harness tests pass.
No broader Stata profile, Windows test or qualification campaign is claimed.

Whole-command clocks compare public `feppml` with pinned unmodified
`fixest::fepois`, robust inference, and public `fereg` with `fixest::feols`, IID.
No optional observation outputs are requested in these clocks. Saving and
clustering are independently validated, not timed here. Slopes use stock
`glm.tol=1e-10`, `fixef.tol=1e-11`; other cases use unchanged stock defaults.
Canonical warm-up, input audits, thread settings and frozen science gates remain
unchanged. Data generation/import/audit is outside the command; loading inside
the command, public preparation and result publication remain charged. Complete
process-time and RSS distributions are also retained.

| Block | Job | Seed | Cases × repeats × arms | Applications |
|---|---|---:|---:|---:|
| Initial | 7763828 | 2026092817 | 10 × 2 × 3 | 60 |
| Fresh | 7763839 | 2026092829 | 10 × 3 × 3 | 90 |
| Direct scaling | 7763839 | 2026092831 | 8 × 3 × 3 | 72 |

Both timing jobs reserve 28 isolated E5-2680v4 cores and alternate arm order.
No build, profile, archive or competing fit overlaps headline timing. Fresh and
scaling blocks execute sequentially in one allocation. Fresh seeds are consumed
validation inputs, never future tuning seeds. Initial accounting is 192s/1.998G;
fresh/scaling 503s/1.999G. Both failed=0, exit_status=0 and terminal sentinels.
Validation 7763797 reserves 16 slots for the explicit 16-worker oracle and uses
363s/6.359G. All jobs are terminal. No push or remote release.

All **222 applications, 74 native pairs and 74 stock pairs pass**. No timeout,
application failure, NR cell or scientific discrepancy is hidden in these blocks.
Every one of the **50 same-thread native b/V exports is byte-identical**; their
recorded iterations and selected memory agree. Across differing threads, 15/24
b/V pairs are byte-identical and all 24 meet the unchanged scientific envelopes.
The small two-FE and OLS controls are preserved; legacy case names containing
“excluded” are identifiers, not a claim that every PPML call avoids this shared
finite kernel. These results do not adjudicate older campaign discrepancies.

## Distributions and remaining losses

Ratios below are candidate/baseline or candidate/stock; smaller is faster.
Ranges are the observed paired minimum and maximum, not confidence intervals.
All ten cases are the frozen registrations, not a subset of winners.

| Case | Initial C/B median | Fresh C/B median [range] | Fresh C/stock median [range] |
|---|---:|---:|---:|
| one-small-t1 | 0.891 | 0.923 [0.890, 0.989] | 2.769 [2.268, 2.989] |
| one-small-t2 | 1.029 | 0.893 [0.876, 0.908] | 1.627 [1.435, 1.701] |
| one-small-t4 | 0.861 | 0.874 [0.861, 0.942] | 1.855 [1.835, 2.237] |
| one-low-t4 | 1.137 | 0.965 [0.960, 0.993] | 3.000 [2.314, 3.481] |
| one-wide-t4 | 0.894 | 0.842 [0.779, 0.843] | 0.913 [0.909, 0.964] |
| control-profiled-small | 1.018 | 1.010 [0.937, 1.092] | 1.533 [1.362, 1.640] |
| control-three-fe-core-pair | 0.935 | 0.881 [0.860, 0.960] | 0.306 [0.299, 0.371] |
| control-excluded-slope | 0.868 | 0.948 [0.915, 1.051] | 0.350 [0.335, 0.408] |
| control-excluded-ols | 0.993 | 1.011 [0.957, 1.071] | 1.625 [1.575, 1.640] |
| one-larger-t8 | 0.738 | 0.774 [0.729, 0.890] | 0.927 [0.914, 0.932] |

The principal gains repeat: T4, wide, larger, three-FE and slopes. T2 improves
on fresh data after an initial 2.9% loss; low improves after an initial 13.7%
loss. Small two-FE loses 1.8% initially and 1.0% fresh; fresh worst is 9.2%.
The OLS control moves from .993 to 1.011, with a fresh worst of 1.071. These
losses remain evidence. No case-specific guard or thread cap is added.

Fresh native finite-solve medians fall T2 **52.8→35.3ms**, T4 **41.5→26.0ms**,
wide **159.6→113.3ms**, larger **204.6→103.4ms**, three-FE **156.8→98.4ms** and
slopes **49.5→28.6ms**. Strict-final clocks also decline on those paths. They can
nest with finite clocks and must not be summed. Inference does not consistently
improve: T2 is 8.9→10.1ms and T4 5.1→6.3ms. This is phase attribution, not a
claim that each difference is exclusively caused by cache coherence.

| Same candidate, lower→higher native request | Median high/low | Observed range |
|---|---:|---:|
| one_fe-t1-to-t2 | 0.850 | 0.626–0.964 |
| one_fe-t2-to-t4 | 0.814 | 0.810–0.837 |
| one_fe-t1-to-t4 | 0.788 | 0.616–0.815 |
| two_fe_dense-t1-to-t2 | 0.955 | 0.892–1.043 |
| two_fe_dense-t2-to-t4 | 0.833 | 0.778–0.904 |
| two_fe_dense-t1-to-t4 | 0.793 | 0.697–0.831 |
| one-wide-t4-t1-to-t4 | 0.655 | 0.635–0.691 |
| control-three-fe-core-pair-t8-to-t16 | 0.946 | 0.925–0.960 |

All seven actual core-count transitions improve in their median; 20/21 actual
scaling pairs improve. Two-FE T1→T2 has one 4.3% loss. The final three-FE row is
requested8→16, **effective8→8**, and supplies three capped-request controls, not
native16 scaling. One-FE T1→T2 now improves all three pairs (15% median) on its
predeclared fresh input. That closes this bounded case, not a universal guarantee.

## Memory, decision and next boundary

The kernel adds **33,280 bytes of transient stack per active partition**, for
33 KiB including the prior standardized-row cache. At most 32 partitions qualify:
1,081,344 bytes total payload within existing reserved worker stacks. There is
no new heap allocation, retained partial bank, N-sized state or admission route.
Wider, unscaled, scratch and implicit sources, and layouts assigning one left
column per partition retain their loops. A one-column tail in a wider partition
layout already uses the changed helper.
Heap instrumentation excludes this stack payload.

Modeled same-thread memory stays identical. Observed three-FE native heap peaks
increase in all five pairs by 0.11–9.83 MB (decimal); the largest increase is 9.4%.
That route already overlaps independently allocated work, so unchanged buffer
sizes do not guarantee unchanged simultaneous peaks. This attribution to changed
lifetime overlap is plausible, not independently proved here. Other same-thread
native peaks agree exactly. Fresh process RSS paired medians range .978–1.043;
individual ratios range .960–1.125. Neither selected bytes nor RSS sampling is
physical-cap qualification; the documented static inventory drift remains open.

Retain the coherent storage change for its reliable gains across sizes and FE
designs. Strong three-FE and slope results coexist with substantial remaining
one-FE/low and small two-FE losses. Universal <=1.20 stock, the <=.90 aspiration
and every-pair monotonic scaling remain unmet.

The next bounded implementation target is **implicit low-memory finite moments
and narrow one-column partitions**: their source still writes shared output rows
for every observation, and fresh low finite work is 58.8ms versus 26.0ms for the
fast T4 case. Extend private write ownership only with independent oracles and
explicit stack/budget accounting. Separately, dense sandwich inference remains
slower at T2 than T1 and owns adjacent mutable partial records. It merits an
ownership/reduction experiment rather than another outer team scope. Do not
resume rejected annotation/general-OLS-scope tuning. Older weak-SE discrepancies,
large timeouts, comparator/sample failures and constrained-fit issues remain open.

## Receipts

[Initial](initial03/receipt.json), [fresh](fresh04/receipt.json) and
[scaling](scaling05/receipt.json) bind application outputs and accounting. Their
summary.json files include every duration, ratio, RSS and eligibility outcome;
phases.json and native-equality.json retain detailed native checks. Collection
verifies remote hashes and independently recomputes identical summaries locally.
Raw licensed logs and generated synthetic data remain on SCC at the submission
paths. The preceding profile's failed controller attempt is preserved separately;
this candidate has no failed application or numerical acceptance.
