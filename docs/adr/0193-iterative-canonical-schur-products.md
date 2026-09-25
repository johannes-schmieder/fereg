# ADR 0193: iterative reduced solves for canonical PPML products

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

Extend the existing two-categorical-FE canonical product builder (ADRs
0168–0175). When pair assembly and complete memory admission pass but component
Cholesky work exceeds the existing work allowance, propose the reduced solution
with conjugate gradients. Affordable Cholesky calls keep their current route.
This supersedes only the cubic-factor-work refusal; it does not remove assembly
or memory guards or introduce benchmark-specific thresholds.

Use the existing current-weight Schur matrix after eliminating the larger FE
partition, grounding the maximum-mass level in each connected component, and
diagonal scaling. Assemble each component matrix once per call and share it
across response/control RHSs. Columns run independently on the bounded team;
components run sequentially within each column. A completed column performs no
further recurrence work for other columns.

Each component starts from zero, scales its RHS by its largest absolute entry,
and requires finite positive curvature. Its iteration cap is the smaller of
the configured cap and its free dimension. The recurrence residual target is
0.01 times the existing MAP tolerance, relative to the normalized RHS norm.
This is a proposal diagnostic, not an acceptance certificate. Dense matvecs use
the existing eight-accumulator product from the joint coefficient engine; its
reduction order is independent of thread scheduling.

Unfinished or numerically invalid proposals decline to the existing certified
projector. Invalid inputs and allocation failures retain their existing error
semantics. No solution, weighted matrix or certificate authority survives the
call. Changing PPML weights therefore always rebuild this geometry.

## Scientific authority

The same fused original-input residual pass remains mandatory. It independently
reconstructs the proposed projection, checks current-weight FE moments against
the existing bounds, and forms direct absorbed Gram/RHS/norms. A recurrence pass
alone can never publish products. Certificate refusal retains the old projector.

Cholesky retains its global pivot guard. CG uses positive-curvature checks and
the independent projection certificate instead of claiming a Cholesky pivot
test it did not perform. Neither reduced algorithm decides canonical finite
rank. The existing finite rank/status, inverse, residual, score, reconstruction,
strict-checkpoint, objective, exact final `q*mu` Hessian and inference checks
remain. Samples, weights, DoF, separation, numerical acceptance tolerances and
transactional publication are unchanged. No early-checkpoint schedule from
ADRs 0191/0192 is revived.

The solve report records the maximum component/RHS Krylov count for an iterative
proposal, and one for a direct proposal. Those counts are not MAP sweeps and
must not be compared as equal units of work. The private solver trace identifies
the reduced route; the public solver request remains MAP.

## Ownership and scope

Reserve three extra `D*W` double vectors through the existing fallible budget,
where D is the total free reduced dimension and W includes response and controls.
The complete phase plan conservatively charges this scratch with the component
matrix/factor bank before reclaiming any old projection buffer. Every phase
must fit the unchanged `16*N*pool` allowance. Scratch ends before independent
certification. There is no persistent bank, speculative memory credit or dense
observation-by-regressor cache added by this decision.

This reaches the existing admitted fast/low two-categorical-FE PPML canonical
entries. Ordinary directions, slope and three-plus-FE canonical engines, and
public OLS routing are unchanged. Existing row streaming, tiled reconstruction
and fallback ownership remain in force.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-iterative-canonical-products-local-2026-09-25/README.md)
records a refused cold factorization with 7,269,032 estimated operations against
a 2,682,240 work allowance. The new proposal makes this existing fused builder
usable without paying that cubic setup. A sparse/skyline diagnostic found
little structural saving and was not implemented.

The first CG implementation improved threaded cases but lost 11 ms on the fresh
serial median. It is not adopted. Reusing the existing independent accumulation
chains removes that measured matvec bottleneck; a separate development rotation
and a new frozen fresh block validate the final implementation. No admission or
tolerance tuning follows the fresh cases.

Independent analytic WLS and explicit-design Poisson/Hessian/sandwich oracles
cover nonzero residuals, changed weights, zero columns, scalar extremes, threads,
iteration refusal and exact budget boundaries. The connected-cycle test now
expects a certified iterative solution, while a one-iteration cap still refuses;
no truth assertion was relaxed. Final focused Rust results are 68 distinct passes
and one already known baseline checkpoint-count assertion failure. Four licensed
oracle definitions pass on each of three candidate artifacts.

Fresh disconnected medians improve 14–24% in three threaded cases; the serial
median improves 1 ms with two losing pairs. Two stock comparisons remain
unranked because both baseline and candidate disagree with stock fitted means.
Four of the other six fresh medians meet the 0.90 target. All candidate/baseline
science comparisons pass. OLS exports match exactly, but two of three control
medians lose 1 ms; the slope PPML control also loses 1 ms. Tracked peak changes
range from -2,681,856 to +433,344 bytes in the final fresh block, with unchanged
scratch storage. These are development results, not physical-memory or platform
qualification, and they do not complete the broader performance objective.
