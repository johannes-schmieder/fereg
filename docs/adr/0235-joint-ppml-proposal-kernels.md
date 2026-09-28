# ADR 0235: repeated categorical PPML proposal products

Status: candidate, 2026-09-28.

## Decision being tested

Retain ordinary categorical joint LSMR, its finite block and FE preconditioners,
canonical initial/strict/final ownership and all acceptance checks. Change only
repeated forward and adjoint products in the speculative augmented operator:

- Borrow each contiguous finite row once for the forward application.
- Accumulate narrow parallel-column adjoints in eight fixed lanes, then merge
  in fixed order and handle the remaining rows. Keep the existing wide serial
  traversal and its routing rule.
- Use inline multiply/add on portable targets without hardware FMA. ARM and
  compile-time FMA targets keep fused products.

The independent unweighted reconstruction and original-input finite/FE
certificate retain their previous arithmetic. Current-weight preparation,
finite Gram/factor, Schwarz construction, recurrence stopping/continuations,
warm-start lifetimes, rank, score/objective, reconstruction and exact final
Hessian remain unchanged. No acceptance tolerance, sample, weight semantics,
DoF, output, separation or publication policy changes.

This candidate introduces no heap bank, cache lifetime, descriptor, solver
route, public option or additional thread team. The eight accumulators are
stack-local. Fixed-configuration reproducibility is required; old recurrence
bits and identical iteration histories are not scientific requirements.

## Evidence motivating the experiment

The separate source-bound [DWARF profile](../../benchmarks/reports/development/canonical-portable-scc-2026-09-28/cpu04/README.md)
identifies ordinary joint LSMR operator callers after the accepted canonical
product improvement. Source inspection shows single-chain column reductions,
strided control reads and portable per-entry FMA calls. The profile does not
quantify the savings this change can deliver. Public timings must establish
whether preparation, convergence and checking preserve any kernel gain.

ADRs 0155, 0157 and 0207 already reject broad coefficient-engine replacements;
ADR 0209 rejects a borrowed canonical-control representation. This candidate
changes neither solver ownership nor control coordinates, and does not revive
those experiments. It is a bounded kernel experiment, not completion of the
broader performance-repair goal.

## Validation and stop rule

Compare forward/adjoint products against separately materialized weighted dense
designs across zero/narrow/wide control counts, ragged row counts, changing
weights and one/four workers. Retain existing adjoint, independent certificate,
strict/fallback and extreme-weight tests. Exercise the changed portable target
on SCC; Mac alone cannot validate portable arithmetic. Use licensed categorical
Poisson/Hessian/sandwich, canonical, slope and OLS oracle controls.

Measure the whole public command in candidate/baseline/pinned-stock rotations
on the existing dense small case and serial, wider and three-FE cases, retaining
low/slope/OLS controls. Keep sample or numerical discrepancies NR. If gains are
promising, register fresh cases before running them. No builds, profiles or
competing fits may overlap headline timings. Reject broadly inconsistent gains
or scientific failures without changing tolerances or adding fitted guards.

## Science checkpoint

Candidate snapshot `4c3d7c44…` passes 30 categorical LSMR Rust tests and four
licensed oracle/control fixtures on both Mac and Linux, plus PPML Linux smoke
and Mac universal ABI/panic containment. Mac additionally passes two strict
categorical final/retry/nested-rank/zero-control tests; Linux runs those before
headline timing. Linux build/validation 7763145 has clean accounting and terminal
sentinel. The 81-application isolated screen 7763162 is registered against
baseline `d1156e6d…` and candidate `a7e95e67…`. Receipts are in the
[candidate report](../../benchmarks/reports/development/joint-operator-scc-2026-09-28/).
Performance adoption remains pending.
