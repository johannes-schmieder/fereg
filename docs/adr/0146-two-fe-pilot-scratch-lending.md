# ADR 0146: lend idle two-FE group banks to categorical pilot work

Status: accepted for local development, 2026-09-23.

## Decision

The reusable categorical absorber may temporarily lend its admitted two-FE PCG
group banks to initial certification and symmetric MAP pilot work. The largest
partition's `left_work` and `left_weights` provide sufficient group capacity for
every effect and every admitted RHS. A scoped owner swaps them into the existing
parallel projector and restores them on success, error or unwinding.

Lending requires an installed multiworker team, at least 16,384 row-by-RHS
entries, empty ordinary parallel group banks, and an admitted two-FE PCG owner.
The existing work threshold now considers the RHS dimension for this previously
serial case. Ordinary row-based reservations and memory admission are unchanged.
The shared certificate dispatcher likewise considers row-by-RHS work, while
still requiring sufficient actual group storage. It does not allocate storage.

Do not lend numerical PCG state during an active recurrence. Initial checks and
pilot sweeps precede current-weight PCG initialization. On entry without a
continuation, PCG reconstructs group denominators and RHS moments, clears its
solution and scalars, and computes operator scratch afresh. Prior group contents
therefore carry no authority. Ownership is restored before checkpoint Schur,
LSMR, PCG, a return, or a failed solve. No state is accepted across weight frames.

## Scientific and memory boundaries

Use the existing deterministic parallel group projection and certificate
kernels. Per-entry observation arithmetic and ordered certificate merge remain
unchanged. Keep initial checks, pilot count, convergence scheduling, solver
selection, current-weight residual checks, canonical rank/inverse, score,
objective, reconstruction and final-Hessian requirements. Sample, weights, DoF,
separation and transactional publication retain their contracts.

No heap bank, observation copy, admitted capacity or memory formula is added.
The implementation replaces serial execution of eligible work; it adds no
alternative mathematical solver. Refusing optional PCG storage, serial teams,
narrow small work, and already provisioned group storage retain their paths.
Slope, three-FE and one-shot OLS algorithms are unchanged. Workers access only
Rust-owned state; the safe-core and main-thread SPI boundaries are unchanged.

## Evidence and limits

The new weighted ring oracle has two disconnected components, changing component
masses, a zero RHS and enough work to enter the loan below the row reservation
boundary. It verifies exact serial/parallel residuals and certificates against
an independently constructed weighted-orthogonal residual. Injected errors and
panics poison loaned buffers; pointers and capacities must be restored, and
later current-weight solves must still certify.

The [source-bound report](../../benchmarks/reports/development/ppml-two-fe-pilot-scratch-local-2026-09-23/README.md)
records 156 focused Rust tests, public explicit Poisson/Hessian/sandwich checks,
an actual-plugin allocation window, separation/transaction controls, alternating
stock comparisons, predeclared fresh designs and focused OLS controls. The
allocation window covers ordinary joint steps; it is not claimed as a direct
allocator audit of canonical pilot execution. The ownership test directly
covers the changed banks. Physical-memory and cross-platform checks remain
deferred.

The wide development case improves 188 to 167 ms; fresh affected medians improve
about 5–13%. Dense two-FE PPML still loses to stock. Retain control losses and
instrumentation overhead in the report. Close this scratch-lending experiment;
do not tune its threshold to validation cases. Removing redundant canonical
projection work is a separate design requiring independent acceptance evidence.
