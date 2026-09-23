# ADR 0138: retained scratch standardization and current-call norms

Status: accepted for local development, 2026-09-23.

## Decision

Extend the retained finite-moment organization of ADRs 0054 and 0057 to scratch
storage, using the private complete-row tiles already admitted by ADR 0070.
For each observation and existing Gram-row partition, resolve and standardize
the selected suffix once. Compact it forward inside that private tile. The
validated selection is strictly increasing, so its source position is never
less than its destination; earlier writes cannot overwrite a later source.
Reuse these standardized values for every assigned Gram row and response moment.
The scratch artifact is immutable throughout this calculation.

Every coordinate preserves observation order, weight normalization, column
scale and fused multiply-add arithmetic. There is no additional tile, partial
bank, allocation, width cutoff or solver route. The existing low-memory bound
is unchanged. Dense and one-shot prepared OLS moment sources retain their paths.
This mechanism applies to retained scratch PPML regardless of FE family.

Canonical retained projection also uses the existing categorical projector's
selected-norm accessor when building or refreshing a control block. Norms may
be borrowed only from the fresh certificate returned by that absorption call,
for exactly one categorical intercept FE and an exactly matching weight divisor.
The selected axis is the current block's column axis. Multiple effects,
different normalization and other projectors keep the explicit norm calculation.
No norm survives an absorption call or authorizes a later weight generation.

## Scientific boundary

The projector still executes its independent acceptance certificate. This
removes a duplicate norm scan, not a projection check or an iteration. Raw
norms and means, FE-collinearity decisions, guarded pivoted Cholesky/retained QR,
finite residual and inverse checks, strict basis transactions, original-input
Poisson scores, objectives, final Hessian and reconstruction remain authoritative.
There is no tolerance, sample, weight, rank, DoF, separation or publication change.
Memory refusal, interruption and errors retain their existing fail-closed behavior.

Independent scalar tests require exact scratch Gram/RHS agreement at widths
1, 7, 16, 63, 64, 65, 128, 129 and 257, with unit/provided weights, scaled/unscaled
columns, noncontiguous selections and T1/T4/T16. They reread the original scratch
artifact to prove it was not mutated. Existing bounded-I/O tests include stored
width 1025. Norm tests exercise changed frames, widths and scales, invalid
selections, mismatched normalization and failed projections. Independent
full-dummy Poisson/Hessian/sandwich fixtures remain separate from stock agreement.

## Evidence and limit

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-tiles-local-2026-09-23/README.md)
records all pairs, fresh cases, exact excluded-path scientific controls,
allocation audits, diagnostic operation counts and remaining losses. Low K32
and fresh K65 whole-command medians improve by about 10% and 11%; narrow cases
show smaller or no median gains. Measured memory peaks are unchanged. All
measured one-FE cases still lose to comparable stock fixest. This supporting
change does not complete the whole-command 0.90-times-stock objective; a larger
change to canonical product construction remains necessary.
