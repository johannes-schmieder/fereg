# ADR 0220: independent FE score accumulation by direction

Status: accepted for local development, 2026-09-26. Only the repaired
**directionalscore2** artifact is adopted; directionalscore1 was never committed.

## Decision

For threaded PPML score checks with at least 4096 observations, give each FE
intercept/slope direction exclusive ownership of its existing moment and squared-
basis destination. Traverse that level's immutable membership index in original
observation order. Independent directions run on the already admitted team; their
certificate contributions enter the existing canonical direction-order reduction.
Small and one-thread fits retain the original row scan. No topology- or seed-
specific scheduling rule is introduced.

Read the installed team's resolved size without submitting a task. Invoke the
serial accumulation closure directly. External Rayon submissions can allocate
queue blocks inside the required warmed, allocation-free iteration window;
checking the thread count inside such a submission is too late.

This extends ADR 0133's independent finite-column execution to FE directions.
Every check still reads original outcomes, current means, prior weights and FE
bases. Each direction retains its floating-point addition and multiplication
order. Scale sums, finite columns and final certificate reduction are unchanged.
The full scientific acceptance schedule and tolerances remain intact, including
consecutive convergence, current-weight projection, model-space, rank, finite
solve, objective, reconstruction, separation and exact final-Hessian conditions.
No Newton product, projected control, prior score or weight statistic substitutes
for an independent original-input check.

## Ownership and memory

The two existing FE scalar banks remain private scratch. They are cleared and
all directions, including empty levels, are recomputed for the supplied state.
Membership indices belong to the immutable post-separation FE design; no new
index, generation cache, numeric bank, worker pool or persistent authority is
introduced. Worker-local sums and disjoint mutable slices require no allocation.
The installed workspace team, rather than a foreign caller's Rayon team, governs
execution. Workers receive only Rust-owned values; SPI and publication boundaries
are unchanged. Scale and finite-column errors still precede FE accumulation.

## Evidence and trade-offs

The [source-bound report](../../benchmarks/reports/development/ppml-directional-scores-local-2026-09-26/README.md)
retains complete distributions, failures, identities and matched-input follow-ups.
Paired diagnostic large fast/low FE accumulation falls 26.358/26.838 ms to
12.445/12.061; entire score checks fall 40.364/41.093 to 26.560/26.505.
Factor FE work falls 7.555 to 1.752 ms. Scale and finite-column costs stay
essentially unchanged. Instrumented clocks are separate from stock headline
comparisons; whole diagnostic fast and three-FE slope timings still lose.

Large public pilot medians improve 1.5–2.2%; fresh large numeric, factor and
three-FE slope medians improve 0.4–1.6%. Fresh native pairs have 32 wins, nine
losses and four ties. All 45 fresh stock comparisons pass science. Six of nine
fresh families meet 0.90 stock in every pair; low large, four-slope low and
disconnected low still miss. The large pilot low case remains slower than stock.

Serial PPML medians lose two ms in both initial blocks and one ms (540/541)
on the single fixed-input follow-up, with four paired losses and one win. OLS
small serial and factor follow-ups tie at 14/14 and 209/209 ms; one-FE remains
17/18 ms, with one win, two losses and two ties. Do not claim universal
non-regression. Retain these small measured trade-offs for the broader threaded
savings without added numeric storage. Their cause is not established; do not
add graph-specific guards or tune thresholds, layout or seeds to these losses.

65 distinct Rust tests pass (58 prepared-IRLS and seven threading). The inherited
checkpoint-count assertion still fails 0 versus 1 on baseline and candidate.
Thirteen licensed fixtures pass, including actual T1/T4 categorical and slope
16-iteration zero-allocation audits. The first candidate failed the T1 slope
allocation audit while the unchanged baseline passed. That failure invalidated
its draft acceptance; removing the serial submission restores this gate on the
repaired artifact. No allowance or scientific assertion was relaxed.

238 public-state audits and 15 explicit Poisson/Hessian/sandwich comparisons
pass. All 85 native PPML and 33 OLS export pairs are exact. The new integer
explicit-design score/norm oracle covers disconnected and unbalanced membership,
unused levels, multiple/duplicate/zero slopes, slope-only partitions, changing
means/weights, binary scales 2^-400..2^400, T1/T4/T8 and stable storage. Existing
numeric/generated checks compare every certificate field and FE moment.

Observed pilot/fresh peak heap and scratch deltas are zero; modeled bounds vary
-48..+112 bytes with unchanged allowances. No physical-cap, Linux or qualification
claim is made. Fresh seed202609451 is consumed. Earlier stock mean/output
adjudications and the rejected first candidate remain separate evidence.
The broader multi-FE performance goal remains active.
