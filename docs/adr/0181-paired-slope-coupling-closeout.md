# ADR 0181: close the paired slope coupling experiment

Status: rejected for production; development decision, 2026-09-25.

## Decision

Keep production computational checkpoint `711802bf`. Do not integrate the
paired-RHS coefficient solver developed from `21280286`. Its large cold-frame
kernel gains do not transfer sufficiently to complete public PPML calls to
justify another persistent solver bank and recurrence implementation. Preserve
all prototypes and their successful and adverse evidence in the
[source-bound report](../../benchmarks/reports/development/ppml-paired-coupling-local-2026-09-25/README.md).
The multi-FE <=0.90-times-comparable-stock objective remains active.

The experiment shares immutable current-weight coupling loads across two or four
RHS lanes. Each column retains its own CG recurrence, restart, curvature fallback,
reconstruction and stopping decision. A stopped lane is zeroed; measured operator
work includes its continued evaluation and unused tail lanes. Neither recurrence
convergence nor another column's acceptance grants scientific authority.
Every returned column still requires the existing fresh reconstruction and full
current-weight original-input FE certificate. No tolerance, rank, weight, sample,
inference or final-Hessian condition was weakened.

## Implementation and evidence

On eight frozen weighted-linear frames, two-lane pairing improves 22/24 median
complete projection/product configurations. Its main three-FE/K10/T8 calls fall
from about 34 to 22 ms. Both lane widths pass all 72 independent explicit-design
configurations in total; all 144 candidate export files agree bitwise with the
scalar exports. All 240 B/C pairs have identical total and maximum column
iterations, reconstructed-check counts and useful operator work. About 11% of
candidate evaluated lanes are inactive or padded. These frames all use compact
coupling; the two-FE label does not establish dense-Schur coverage.

Three public implementations were then built and measured. The first generalized
the scalar operator over lane count; matching diagnostic builds showed an
8–10% ordinary joint-iteration slowdown on large slope cases. The successor
restored the original scalar kernel and used paired arrays. Fresh seed 202609409
exposed a 4.5% serial regression. The final bounded correction restored the
scalar dense kernel's four-row accumulator structure inside the paired kernel.
This is a computational-layout correction, not a tolerance or case-name guard.
No fourth kernel correction or fresh-case threshold tuning was attempted.

The final public version allocates pair banks once under a separate optional
allowance after the existing solver, coupling, joint and checkpoint workspaces.
It retains the scalar path when RHS count does not exceed worker count. No
additional observation-by-regressor bank is introduced; existing observation
buffers remain available to low-memory joint work. For R admitted RHSs, T workers,
P remaining coefficient coordinates and C eliminated channels, its payload and
worker descriptors cost `min(T,ceil(R/2)) * (16*(4*P+C)+sizeof(BatchWorker))`
when R>T, plus the charged owner descriptor. Budget refusal preserves scalar
solving. The banks carry no weight-generation or acceptance authority.

Fresh seed 202609410 confirms only 2.5–3% median public gains on large three-FE
cases. The multiple-slope case loses all five B/C pairs (196 versus 193 ms), and
the weak case has a small median loss. The serial correction passes on the new
seed, but the resulting public benefit is still small relative to complexity.
There are 15 losses and 5 ties among 45 final fresh pairs. Do not interpret the
strong native microstructure result as a 35% public PPML improvement.

The final artifact passes 99 distinct focused Rust tests, four licensed scientific
fixture executions and two real 16-iteration allocation-audit executions. The
latter certify ordinary warmed iterations, stable capacities and cold/warm
agreement; they are not a separate allocator measurement of every canonical pair
call. Independent native WLS and licensed explicit-design Poisson/Hessian/
sandwich oracles accompany stock agreement. All 426 public states across the
headline and diagnostic blocks pass their declared checks. This is development
evidence, not qualification.

## Consequences and next boundary

Restore all accepted runtime inputs; retain no unused production batch engine or
parallel test-only benchmark tree in the repository. Historical evidence and
frozen benchmark definitions remain unchanged. Seeds 202609409 and 202609410 are
consumed validation seeds. A separately declared four-FE boundary control at seed
202609411 was not run after the implementation was rejected.

Move next to the narrow categorical whole-command path. The current accepted
fresh N27360/K2/T4 case is 47 ms against stock 34 ms, requiring 16.4 ms (35%) total
savings for the objective. Attribute Stata preparation/publication and native
setup on the same command before choosing the intervention. For large slopes,
current accepted low-mode time is 322 versus 288 ms: pairing recovered only 8 ms
of the 62.8 ms required. A future slope intervention must remove a larger part
of canonical construction or surrounding repeated work; do not tune these pair
widths, retained-coefficient layouts or thresholds on the frozen validation cases.

Existing allocation-inventory drift, constrained-fit/nonretaining-CG failures,
comparator limitations and deferred platforms remain open. No push, release,
SCC retry, stopped campaign or qualification was undertaken.
