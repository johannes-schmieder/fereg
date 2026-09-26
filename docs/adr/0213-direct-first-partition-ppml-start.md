# ADR 0213: direct first-partition cold PPML initialization

Status: accepted for local development, 2026-09-25; not qualification.

## Decision

For cold publication PPML with at least two FE partitions, all containing an
intercept, initialize the first partition directly from original outcomes, prior
weights and offsets. Then finish the existing conditional-intercept sweep over
the remaining partitions, followed by the same profiled slope proposals in
partition/group/channel order. This replaces the discarded global intercept and
first-partition correction in ADRs0161/0186. No additional sweep or tuning parameter.

At zero finite coefficients, a global start has eta_i=o_i+c. Its first group
update gives

    eta_i + log(sum_g q*y) - log(sum_g q*exp(eta))
      = o_i + log(sum_g q*y) - log(sum_g q*exp(o)).

The global shift c cancels. Reuse the established one-partition conditional-start
implementation: group-constant offsets cancel analytically through compensated
raw weighted means; otherwise use compensated, shifted log sums. Invalid or
unrepresentable provisional values refuse the entire seed. Subsequent partitions
retain their compensated raw sums and existing finite/bound checks. Slopes retain
their profiled objective safeguard and step-halving budget.

An admitted multi-FE start leaves initial finite coefficients, including the
reported constant coordinate, at zero; absorbed intercepts represent its predictor.
This is an initial gauge, not the reported normalization or a rank decision.
Canonical full-width solving still establishes the finite basis and final constant.
On refusal, compute the former global start lazily and overwrite every predictor
row and its constant coefficient, including earlier provisional updates.

## Scope and authority

Explicit warm starts and their validation remain first. Mu exploration, no FE,
one FE, and any slope-only partition retain their former starting paths. The
existing one-FE arithmetic is extracted as a shared group helper without changing
its formulas. First-group direct evaluation can avoid underflow caused solely by
an unnecessary global offset shift; it must still pass the same predictor/mean
bounds and all subsequent scientific checks.

This changes an initial guess only. Current-weight projection, canonical rank,
finite solve, score, model-space, line search, two consecutive outer convergence
checks, strict acceptance, original-input reconstruction, independent objective,
separation, inference and exact final q*mu Hessian conditions remain unchanged.
No cached geometry or certificate crosses a sample/weight generation. Samples,
weight semantics, VCE, DoF and transactional publication are unchanged.

The initial predictor and coefficient banks already exist. This introduces no
observation/group bank, parallel pool, memory allowance or iterative allocation.
OLS does not call this path. Computation remains in safe Rust with main-thread
SPI and the guarded FFI/publication boundary.

## Evidence and limits

Retain independent separable Poisson and slope-score seed oracles, changing weight
scales, extreme offsets, constant/dependent slopes, explicit starts, refusal after
later partitions and complete rollback. Public Poisson/Hessian/sandwich oracles,
weight/inference/separation/transaction fixtures and excluded OLS controls remain
necessary. Numerical equality to the old recurrence is not required; no tolerance
changes are allowed. A baseline-reproduced strict-checkpoint count assertion is
an existing failure, not repaired by this decision.

The [source-bound report](../../benchmarks/reports/development/ppml-direct-start-local-2026-09-25/README.md)
preserves directstart1, its accepted baseline, independent oracles, full rotating
distributions and isolated diagnostics. Initialization falls 53.58 to 20.90 ms on
the million-row diagnostic. Public million-row fast/low medians improve 3.3/3.1%;
unchanged fresh seed 202609443 confirms 3.5% gains in both modes. Smaller losses
remain visible. No source or guard is tuned on fresh validation.

Stock remains pinned and unmodified. The fresh largest numeric stock mean
comparisons, the protected one-FE mean comparison and one small full-output
comparison fail their registered envelopes and remain unranked. Do not relax
tolerances or infer speed parity from those clocks. The broader 0.90-times-stock
objective remains unfinished; current handover belongs only in STATUS.md.
