# ADR 0183: close the shared dense slope-factor experiment

Status: rejected for production, 2026-09-25. The multi-FE speed goal remains active.

## Decision

Do not replace the current slope Schur/CG owner with the tested full reduced-
system factorization. Keep production computation and routing unchanged. The
[source-bound report](../../benchmarks/reports/development/ppml-shared-slope-factors-local-2026-09-25/README.md)
preserves both implementations, independent oracles, public comparisons and
actual canonical-input replays. No scientific or memory contract changes.

The experiment forms the current-weight Schur matrix from the existing local
rank-aware QR bases and group-pair coupling. A blocked Cholesky factor of
`S + sqrt(epsilon)*max(diag(S))*I` serves every RHS. The diagonal addition only
stabilizes a proposal; it is not a ridge objective, rank decision or acceptance
criterion. Refinement uses the unshifted problem. The first implementation
reconstructs original-row residuals after each proposal. The corrected one uses
coefficient moments for the first refinement and reconstructs for acceptance.
Both preserve the original full fresh certificate and certified CG fallback.

The public prototype lazily builds its factor once per coupling generation and
invalidates it before every numeric coupling refresh, including ordinary joint
work. It does not carry numerical factor authority across PPML weights. Its
2,048-coordinate bound limits a diagnostic experiment; it is not an adopted
routing threshold. Its additional banks do not have production memory admission.

## Why it is rejected

On cold, strict, wide three-FE frames the corrected full projection/product call
improves approximately 23–24% at T8. That does not describe the public workload.
Across the six public cells there are 14 losses, two ties and two wins in 18
candidate/baseline pairs. Large fast PPML worsens from 223 to 238 ms; low mode
worsens from 240 to 243 ms. The corresponding stock medians are 231 and 230 ms.

Captures of actual accepted-command inputs localize the mismatch. The initial
canonical stage uses the established scheduled tolerance of 1e-6; strict
canonical acceptance uses 1e-12. The large fast initial call evaluates 87 CG
products across 12 RHSs, versus 170 across 11 at strict construction. Several
scalar refreshes use zero or one product. The original cold experiment required
1e-12 throughout and used different frozen RHS/weight frames.

At the actual targets, the large fast T8 initial replay worsens 20.512→26.881 ms,
while strict improves only 26.684→26.057. Large low initial worsens
20.407→26.178 and strict improves 28.489→25.675. The smaller two-FE T8 initial
and strict blocks both lose. These replays rebuild geometry for both arms;
they are not public clocks or a reproduction of same-generation reuse costs.
Baseline operator counts match every captured block exactly at all tested
thread counts. This supports closing the broad replacement without fitting
new phase/width/topology guards to the favorable cold cases.

The large factor retains 6,610,248 numeric bytes; its factor-plus-panel payload
is 6,842,952 bytes, additional to existing owners. That investment is not
justified by the observed public results. Memory planning, constrained budgets,
allocation audits and qualification were deliberately not advanced after the
early public rejection. This is not a proof against all block or sparse factors.

## Next implementation boundary

Move to ordinary joint-direction construction and its verification passes.
The preceding accepted-source whole-command diagnosis puts that work near
68 ms on the large slope cases and 38 ms on the smaller two-partition case.
Measure preparation, operator applications, original-input reconstruction and
certification on actual scheduled frames before selecting an implementation.

Investigate computing current-weight finite/group sufficient statistics from
original controls together, then performing elimination in coefficient space,
instead of repeatedly normalizing, projecting and scanning each observation
column. Reuse existing joint ownership and keep the observation operator as
the numerical fallback. These would remain direction proposals, with fresh
current-weight FE/finite scores and reconstruction; they must not acquire
canonical rank, inverse or final-Hessian authority. ADR 0167's rejection of
cancellation-sensitive canonical Gram subtraction still applies.

Use an early complete public screen before further integration or a broad
validation block. A promising candidate then needs explicit WLS/Poisson/Hessian
oracles, near-collinear and deficient slopes, changing weights, memory refusal,
and predeclared fresh cases plus categorical PPML and OLS controls. Do not
resume paired-lane, retained-coefficient, metadata or factor-threshold tuning.
