# ADR 0199: separate PPML slope availability from categorical encoding

Status: accepted for local development, 2026-09-25. OLS slope timing sensitivity
remains unresolved; this is not qualification or an overall non-regression claim.

## Decision

Retain `slopetopology1`, based on d25c6e06 and the source overlays in the
[source-bound report](../../benchmarks/reports/development/ppml-slope-source-preparation-local-2026-09-25/README.md).
Generalize ADR 0087's source-availability compaction to borrowed PPML preparation
with continuous finite terms and at least one absorbed slope. There is no size,
thread, weight, partition-count or memory-route threshold. Actual slope solving,
weighted geometry, canonical checkpoints and inference are unchanged.

The availability planner marks slope sources as synthetic categorical effects
so the existing numeric classifier checks their required fixed-effect source
role. Previously, PPML also factorized those continuous values, built their
codes/levels/membership and discarded that temporary topology. Separate the
borrowed numeric preparation seam from its categorical completion. Validate the
complete original availability request, loaded data, domains, source uses and
sample first. Then use the existing plan compaction to remove synthetic slope
markers before categorical encoding. Finish with the same topology, singleton,
rebasing and final validation machinery.

The numeric sample retains every slope value. Sources that also define true
partitions remain categorical. Cluster roles remain explicit and still receive
their required encoding. A new PPML sample generation repeats classification;
no prepared result or weighted state crosses the generation boundary unchecked.
Pweight subset normalization, generation memory bounds and candidate/weight
rollback retain their prior owners and error semantics.

## Deliberate boundary

Finite factor recipes already contain categorical-source indices into the full
availability table. Removing categories without rebinding those indices would
be wrong. Such models retain their existing path. Tests cover a slope source
also used as a finite factor, categorical interactions, omitted and vanished
levels, and alternative source orders. This decision does not simply remove
the continuous-term guard or change the earlier owned OLS eligibility.

The shared borrowed OLS constructor invokes the extracted classification helper
and then its unchanged completion. Public OLS does not receive PPML's new
compaction route. No solver recurrence, batching, stopping, certificate,
reconstruction, objective, rank, sample, weight, DoF or VCE policy changes.
All accepted PPML states still require fresh current-weight checks and the exact
final q*mu Hessian. There is no new cache, worker, persistent owner or output.

## Work and memory

The paired diagnostic reduces prepared-model time from 6.351 to 2.092 ms for
N30720/K6/T8 two-partition slopes, and 14.282 to 4.944 ms for N61440/K10/T8
unbalanced three-FE slopes. Categorical encoding drops 4.022 to 0.298 ms and
8.652 to 0.487 ms, respectively. Canonical construction remains substantial.
These instrumented timings are attribution, not stock-comparator headlines.

The change omits temporary encodings and membership; it adds no observation bank
or persistent state. Existing admission estimates remain conservative. Tracked
PPML peaks change by -48 to zero bytes in the measured full fits, scratch is
unchanged, and selected telemetry varies -112 to +48 bytes. Removing temporary
preparation storage does not imply the whole-command peak must fall. Two warmed
slope allocation executions pass with sixteen measured iterations each, zero
events/current growth and stable capacities. Physical caps and the documented
static allocation inventory drift remain unresolved.

## Evidence and retention limits

Eleven distinct Rust tests and six independent licensed scientific fixtures pass.
Tests cover original sample/model/design equivalence, missingness precedence,
source ordering, interactions, multiple and deficient slope channels, frequency
and probability weights, changing generations, memory refusal and rollback.
Independent analytical WLS and explicit-design Poisson/Hessian/sandwich checks
remain separate from baseline and fixest agreement.

All 247 public PPML states, 160 baseline-anchored comparisons and 73 direct stock
comparisons pass. All 87 native PPML pairs have exact coefficient/covariance/mean
exports and unchanged iterations/fallbacks. Fresh seed202609432 has 44 wins,
ten losses and one tie; all 35 slope pairs improve, with 3–18-ms median gains.
Seven of eleven medians meet 0.90 stock, all already met by the baseline in this
block. Threaded slope ratios improve to 0.925 low and 0.911 fast; the objective
remains unfinished. Excluded serial/wide categorical medians lose one ms.

OLS science and exports pass in 25 pairs across two public control blocks and
two diagnostics. The initial slope control loses 108→114 ms, and its single
predeclared repeat loses 107→113 ms. Attribution with the existing production
phase timers shows unchanged preparation/iterations/memory and varying absorption
costs; the traced block reverses the overall timing direction. That diagnostic
does not erase the public losses or prove their cause. Retention rests on the
consistent PPML slope gain and removal of demonstrably unnecessary work; OLS
slope timing remains an explicit development concern. Do not tune OLS kernels
or benchmark-specific guards to compensate, or claim universal non-regression.

The bounded preparation experiment is closed. Continue with larger canonical
ownership and narrow lifecycle costs while carrying the OLS timing control
forward. Historical rejected solver/checkpoint experiments, constrained-fit/CG
failures, older three-FE discrepancies, platform deferrals and frozen definitions
remain unchanged. No push, release or qualification is authorized here.
