# ADR 0235: repeated categorical PPML proposal products

Status: accepted for bounded development, 2026-09-28. Not qualification.

## Decision

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
and Mac universal ABI/panic containment. Both platforms additionally pass two
strict categorical final/retry/nested-rank/zero-control tests, for 32 distinct
Rust tests each. Linux runs them before
headline timing. Linux build/validation 7763145 has clean accounting and terminal
sentinel. The 81-application isolated screen 7763162 is registered against
baseline `d1156e6d…` and candidate `a7e95e67…`. Receipts are in the
[candidate report](../../benchmarks/reports/development/joint-operator-scc-2026-09-28/).
The [initial screen](../../benchmarks/reports/development/joint-operator-scc-2026-09-28/initial02/README.md)
and [fresh confirmation](../../benchmarks/reports/development/joint-operator-scc-2026-09-28/fresh03/README.md)
complete 192 headline applications and 64/64 native plus 64/64 stock science
pairs. Fresh serial and K24 C/B paired medians are 0.678 and 0.783, with every
pair faster; K12 is 0.958 and weak connectivity 0.934. Small T4, sparse, larger
and three-FE are approximately neutral. The initial small-T4 gain does not
replicate. Accept the small implementation for the repeatable serial/wide gains,
without adding a fitted selector. Outer/inner counts and modeled memory fields
match on every pair. No heap bank is added; physical peaks still vary.

Controls remain mixed: fresh slopes lose 4.9% at both paired and marginal
medians, with two of three losses and range 0.897–1.197; initial slopes instead
improve. Fresh OLS is neutral. Do not claim uniform nonregression, attribute
excluded-route changes to this kernel, or reopen control-specific tuning.

Small dense PPML still takes 1.894 of stock at T4 and 1.327 at T1; OLS takes
1.570. Recorded same-input T4/T1 science passes, but paired timing is 1.005,
range 0.843–1.543, and marginal medians imply a 15.7% T4 loss. Requests were
separately grouped; matched alternating requests are necessary before changing
team policy or attributing the loss to the existing traversal switch. The
universal 1.20 ceiling, 0.90 aspiration and monotonic scaling are not achieved.
ARM performance is unmeasured here, although independent science passes.

Close this bounded kernel experiment. Prioritize the original large-input
comparator discrepancies next; smaller oracles have not adjudicated them.
Future performance work must retain these concrete small-command and scaling
losses, rather than reopening rejected engine swaps or fitting guards.
