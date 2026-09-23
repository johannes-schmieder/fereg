# ADR 0118: Parallel owned-FE projection groups

Status: accepted for local development, 2026-09-23.

## Decision

Automatic owned-slope MAP/CG absorption may compute independent group
coefficients on the installed worker team, followed by disjoint row subtraction.
Eligibility requires at least 16,384 rows, nonempty RHSs and more than one
installed worker. No team is created. Direct solvers, serial/small inputs and
the separate changing-weight reusable MAP retain their serial workspaces.
Categorical borrowed paths and specialized single-partition slope paths remain
separate. These are structural scheduling conditions, not benchmark guards.

Each group retains its original observation/channel/RHS accumulation order,
raw weighted Gram, pivoted elimination and 1e-12 solve threshold. Every fitted
entry retains its channel FMA order and one subtraction. Groups have disjoint
members, so delaying all row writes until their coefficients are ready does not
change any group input. Contiguous level chunks retain first-error order;
workers never touch Stata SPI. Rank, norms, fresh certificates, recurrence,
contraction, route selection and publication rules are unchanged.

Retain the serial workspace and optionally admit a bank of at most `L*C*R`
doubles plus `T*C*C` Gram doubles and `T` worker descriptors, where L is the
maximum effect level count, C the maximum channel count, R the actual RHS
count and T the installed team size. Checked dimensions and actual capacities
must fit the admission. Reservations are fallible; size/allocation refusal
retains the original serial path. The bank is reused across effects/sweeps
within one solver invocation. Coefficients are recomputed for every projection;
no weighted geometry, residual, response or iteration result is cached here.

Admission comes after the existing geometry and parallel-certificate budgets.
Optional projection storage cannot evict either or crowd out an affordable
baseline route. MAP releases it before LSMR/CG. CG selects its affordable
geometry/certificate context, then independently admits its projection bank.
Reported peaks retain the maximum of earlier admitted attempts. Existing
runtime/stack headroom covers the unchanged 256-byte row tile per worker.
This component bound is not a tighter whole-action memory guarantee.

Nothing crosses a sample, weight frame or generation. Generic automatic calls
can benefit only through their own fresh workspace; the separate reusable PPML
MAP lifecycle is unchanged. All residual, score and final-Hessian requirements
remain mandatory.

## Evidence and closeout

The [local report](../../benchmarks/reports/development/parallel-projection-local-2026-09-23/README.md)
records 15.3–36.0% paired median gains across seven eligible threaded slope
cases, including unbalanced three FE, low memory and AW/PW. Additional tracked
heap is 0.116–1.966 MB. All 39 native pairs have exact scientific exports and
non-clock solver fields. Independent weighted projection formulas, rank/units/
RHS/thread boundaries, admission tests and licensed fixtures pass. The serial
wide control has a 1.7% paired median loss; other small losses remain visible.
Stock still wins all five comparable timing rows.

Close this scheduling experiment without threshold or chunk tuning. Return to
broader public PPML coverage, beginning with a small current-source screen of
higher-dimensional/slope and saved-output costs. Its changing-weight lifecycle
requires separate profiling and admission; do not extend this workspace or the
OLS geometry cache across iterations on the strength of these OLS receipts.
