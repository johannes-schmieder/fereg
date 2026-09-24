# ADR 0163: admitted current-weight three-FE Schur checkpoints

Status: accepted for local development, 2026-09-24.

## Decision

Integrate the centered Schur design selected by ADR 0162 into the existing
categorical three-FE PPML checkpoint owner. Eliminate the largest partition,
assemble the remaining weighted covariance matrix once per call, diagonally
scale it and solve each RHS independently with CG. Reconstruct from original
inputs and FE coefficients; require the existing complete original-weight
certificate before acceptance. No weighted state persists across calls.

Let q be the sum of the other two level counts. Admit this optional organization
only when q² <= 6N: a dense matvec then has no more multiply-adds than one
three-indicator forward/adjoint pair. Also require its entire temporary bound to
fit the already admitted checkpoint LSMR allowance. This is a structural work
and storage rule, not a claim that every admitted topology will be faster.
Dense per-group covariance construction can still dominate on unmeasured large
group systems; wider-scale confirmation remains open.

Ordinary joint IRLS, one/two/four-plus FE, absorbed slopes, one-shot OLS and
component-scaled checkpoint calls retain their existing algorithms. A successful
Schur checkpoint reports requested LSMR and used MAP, as the existing two-FE
coefficient accelerator does. It is not reported as a failed LSMR fallback.

## Numerical and transactional boundary

Group-centered covariance assembly recognizes exactly group-constant indicators
by topology. It does not clip negative diagonals or impose an identifying gauge.
Extra null directions are allowed. Cheap recurrence diagnostics stop individual
columns; the cap is min(configured maximum, 2q). Neither those diagnostics nor
the recursive residual grants acceptance.

A finite candidate must pass the unchanged fresh canonical certificate computed
from original weights. Initial rank, finite-solve, model-space, score, objective,
reconstruction, final-Hessian, inference, sample, separation and DoF requirements
remain unchanged. Numerical refusal drops every temporary before the existing
selective LSMR retry/fallback sees untouched input. Rejected iterations, setup,
solve, certificate time and retry warnings remain in the aggregate report.
Allocation and other fatal errors propagate. Existing main-thread PPML interrupt
polls remain at their established estimator boundaries; workers make no SPI calls.

Low-memory canonical report merging now accepts a mixture of certified Schur,
LSMR and zero-work tiles. It preserves certificate maxima and costs, clears a
heterogeneous preconditioner identity and preserves the actual fallback flag.
The private ordinary OLS report merger is unchanged. A successful accelerator
must not manufacture a numerical-fallback event.

## Memory ownership

The plan charges normalized weights, eliminated group masses, two q-by-q matrices,
up to 2N sparse coupling entries, offsets, group scratch, diagonal scales, fixed
worker banks, column output and row-major conversion storage. It sums all these
allocations conservatively, without taking credit for nonoverlapping lifetimes.
Every reservation is fallible; actual vector capacities consume the admitted
allowance. Fixed disjoint worker slices bound concurrent Krylov storage even
under nested Rayon execution. No new worker team or session cache is introduced.

The caller-owned original input and canonical workspace remain separately
charged by the existing checkpoint plan. Optional refusal preserves that plan's
previous affordable route. Forecast selected/scratch bytes therefore need not
increase. This is a bounded substitution within the checkpoint allowance, not a
claim that whole-process memory coverage or physical-cap qualification is closed.

## Evidence and limits

The [source-bound integration report](../../benchmarks/reports/development/ppml-three-fe-schur-integration-local-2026-09-24/README.md)
records the final artifact, independent constructed weighted-projection truth,
Poisson/Hessian/sandwich fixtures, allocation/error boundaries, all public
rotations, comparison exclusions and prior failed integration states.

Fresh three-FE medians improve 21–40%; all twenty target pairs improve. Low dense
and serial cases beat the 0.90 stock target, while unbalanced remains at 0.950.
Sparse three-FE and disconnected two-FE stock comparisons are unranked for
scientific discrepancies also seen against the native baseline. OLS exports are
exact; slope OLS has a one-millisecond median loss. No universal non-regression
or universal stock win follows. The broad performance goal remains unfinished.

The next larger comparable gap is wide low-memory two FE: 118 versus stock
80 ms, requiring another 46 ms to reach 0.90. Prior current-source attribution
identifies repeated canonical materialization, streamed products and joint
reconstruction; address their ownership together. The remaining unbalanced
three-FE gap is 5 ms, with ordinary projection still about 32 ms in the first
fresh run. Do not tune this checkpoint's gates or the rejected GS probe on the
consumed fresh seed.
