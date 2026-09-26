# ADR 0221: idle workers in canonical slope products

Status: accepted for local development, 2026-09-26. Adopt only
**canonicalhelpers2**; canonicalhelpers1 was rejected and never committed.

## Decision

When the admitted coefficient-space slope workspace solves one RHS and owns
additional column workers, borrow their existing product and eliminated-group
scratch for compact Schur products. Reuse the helper implementation already used
by ordinary joint finite/FE directions. Independent eliminated-group blocks form
partial products, which are merged in worker order before the existing curvature
guard. Dense coupling and unusable helper configurations retain direct products.

Choose the shared or direct product at compile time within the same column
recurrence. Multiple RHS calls retain the existing contiguous column scheduling;
each column retains independent stopping. Single-worker calls retain the direct
kernel. There is no new size threshold, topology rule, solver family or cache.

This extends the computational organization of the existing Schur solver, rather
than changing its mathematics or acceptance policy. Current-weight rank-aware QR,
coupling preparation, explicit sample/topology/weight generations and invalidation
remain unchanged. Cheap moments schedule fresh reconstruction; original-input
projection and model-space certificates still decide acceptance. Finite solve,
rank, score, objective, final Hessian, separation, inference, DoF and transactional
publication retain their requirements and tolerances. A parallel partial sum can
change low bits; bitwise equality to the serial product is not an acceptance gate.

## Ownership and memory

Helpers are borrowed only when their column work is idle. The owner keeps the
coefficient recurrence and stopping state; helpers supply existing scratch only.
There is no new numeric bank, allocation, worker pool, memory allowance or
cross-call authority. Workers use Rust-owned data. The existing bounded low-memory
workspace determines how many helpers are available; refusal and fallback remain
within its original admission policy.

## Evidence and rejected alternative

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-helpers-local-2026-09-26/README.md)
preserves both candidates, baseline diagnostics, public rotations and fresh cases.
The target low-memory four-slope fit admitted seven workers, but its two response
calls used one each; their compact products cost about nine ms. Multiple controls
already used six workers. This supported sharing the response products without
repacking columns or changing geometry.

The first candidate routed all calls through the helper-capable product. It
improved the target but lost on wide and serial controls; paired traces found
10–14% more operator time without more iterations or certificates. Compile-time
direct/shared selection removed that operator penalty in the revised candidate.
No threshold, layout or seed was tuned to hide those losses.

Final public target medians improve 112 to 106 ms; two fresh four-slope low-memory
cases improve 167 to 158 and 208 to 197 ms, with all nine native pairs improving.
The pilot fast four-slope case loses 101 to 102 ms. The fresh disconnected slope
case loses 57 to 60 ms; one predeclared fixed-input follow-up ties 57/57, with one
paired loss and four ties. Both blocks remain evidence. Other controls have mixed
individual outcomes; this is not a universal non-regression claim.

Paired diagnostic target column solving falls 23.149 to 19.402 ms and canonical
construction 45.494 to 41.956 ms. Total instrumented wide/serial fits still lose
185.097/187.385 and 239.498/240.330 ms. Their operator sums no longer show the
first candidate's penalty. Iterations and fresh check counts are unchanged.
Nested spans and sums across concurrent columns are not additive wall times.

Nineteen focused Rust tests pass. The analytic weighted projection oracle now
forces compact coupling across T1/T4/T8, single/multiple RHS, changed/extreme
weights, disconnected groups and duplicate/zero/slope-only channels. Thirteen
licensed fixtures pass, including independent Poisson/Hessian checks, transaction
checks and actual T1/T4 categorical/slope 16-iteration zero-allocation audits.
All 147 public-state audits, 12 explicit Poisson/Hessian/sandwich comparisons,
96 anchored comparisons and 45 direct stock comparisons pass. Thirty-eight of
51 native PPML pairs are fully exact; all pass the scientific envelopes. Twelve
OLS coefficient/covariance pairs are exact. Observed peak and scratch deltas are
zero; modeled bounds differ by at most 48 bytes with unchanged allowances.

Five of six fresh families and four of six pilot families meet 0.90 stock in
every pair. The smaller four-slope target and disconnected cases still miss.
This is bounded Mac development evidence, not a frozen-suite, constrained-memory
or cross-platform qualification. Existing allocation-inventory drift, checkpoint
and wire-minor issues and constrained-fit/CG failures remain open.
