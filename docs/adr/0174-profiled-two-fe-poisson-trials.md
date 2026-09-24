# ADR 0174: profiled two-FE Poisson trials and one canonical final checkpoint

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

For publication PPML with exactly two categorical intercept partitions, optionally
optimize one partition analytically at every line-search trial. Keep the existing
certified joint MAP/LSMR directions. Choose the partition with more groups; ties
use the first partition. This choice depends on topology, not case names, seeds,
row counts or observed speed. No new public solver family is introduced.

For prior weights q, outcomes y and a trial predictor t, add to each observation
in group g:

    delta_g = log(sum_g q*y) - log(sum_g q*exp(t)).

This is the conditional Poisson optimum for that partition. The full predictor
remains in the same model space. Finite coefficients retain the existing step
interpolation; the extra shift belongs entirely to the absorbed intercepts.
Both sums use stable log scaling and compensated accumulation. Each halved trial
recomputes its shifts before evaluating the existing objective and mean bounds.
Only private trial buffers change until acceptance. No tolerance, objective slack,
step floor, halving cap, clipping rule or convergence condition changes.

Prepare prior log weights and outcome totals once for the immutable prepared
sample. These are not Hessian weights. Means, q*mu, working responses, linear
geometry and certificates continue to belong to the current iteration. The owner
never survives a sample or design change. Constructor refusal, including a group
with zero weighted outcome, retains the previous route. One FE, slopes, three or
more partitions and mu separation retain their prior mechanisms.

After a profiled trial, discard joint LSMR increment history: the extra FE shift
changed the accepted increment. Preserve the canonical session's separate
response ownership. A prior solution may not certify a new weight frame.

## Verification organization

The strict accepted-point check uses one complete canonical Newton checkpoint
instead of the ordinary joint strict witness followed by a separate canonical
predictor solve. The strict checkpoint still independently absorbs the original
current-weight inputs, determines canonical rank, solves the finite regression,
checks its bread and reconstructed predictor, and compares the objective,
identified coefficients and every predictor row to the accepted point. Full
outer scores and the existing consecutive convergence requirements remain.

If strict canonical WLS confirms the accepted point and unchanged basis, retain
that accepted eta and beta. This leaves the checkpoint's weights exactly equal
to q*mu for the published point. Generalize the existing profiled one-FE final-
product boundary: independently check exact weight identity, generation, certified
current products, q*mu and reconstructed predictor before using its within design
and bread for final inference. The final score is freshly checked. The normalized
constant and saved FE outputs still use their established certified boundaries.

A mismatch refuses reuse and runs the existing full final-Hessian synchronization.
A changed strict basis revokes the optional curve. Failed checkpoints retain the
existing bounded refinement/rebuild and transactional rollback rules. These
checks replace redundant work by an independently certified same-state witness;
neither an analytic group optimum nor a solver recurrence certifies the full fit.
Samples, separation, weight semantics, DoF, VCE and publication atomicity remain.

## Memory and scheduling

The owner retains N log prior weights, G log outcome totals, G trial shifts and
K identified coordinate slots. Admission conservatively charges

    sizeof(owner) + 2*sizeof(usize)*K + 8*(N + 2G)

beside the existing solver and canonical checkpoints. The second K bank covers
the temporary active-column identity during construction. It introduces no N-by-K
copy or new dense system. Allocations are fallible and exact-capacity checked;
no buffer grows during ordinary iterations. The existing real sixteen-iteration
allocation audit includes this owner's capacities. Group shifts and row updates
run in one installed worker-team submission, without worker SPI access.

## Alternatives and evidence

The [source-bound report](../../benchmarks/reports/development/ppml-profiled-two-fe-local-2026-09-24/README.md)
records all development candidates, frozen-frame diagnostics, fresh distributions,
independent scientific checks, excluded controls and memory tradeoffs.

A fully dense centered finite/remaining-FE system improved some frozen frames but
lost heavily on a larger disconnected system and required about 40.6 MB there.
It is not adopted. Reusing full canonical products for every ordinary direction
also failed to improve enough over existing joint directions. Its first isolated
build had an ownership defect found before any fits; the corrected measured
version remains rejected. The curve alone saved an iteration in some cases but
retained duplicate final work. The adopted design combines the curve with the
single canonical final checkpoint and avoids another overlapping direction engine.

Independent tests cover analytic group optima, extreme weight scales, disconnected
components, nonzero residual truth, stale generations, predictor corruption,
canonical fallback, failed trials and transaction rollback. Existing numerical
envelopes remain unchanged. Focused OLS and excluded PPML paths protect prior work.
Local evidence does not close the broad ten-percent-over-stock objective, repair
known constrained-memory failures or allocation-inventory drift, or establish
cross-platform qualification. Frozen validation cases must not become tuning cases.
