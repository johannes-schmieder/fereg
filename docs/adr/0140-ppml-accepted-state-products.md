# ADR 0140: immutable PPML family inputs and canonical product ownership

Status: accepted for local development, 2026-09-23.

## Decision

A finalized PPML IRLS run borrows its immutable outcome, prior weights and offset
through `PreparedPoissonInputs`. It prepares `log(y)` (with the existing exact
zero branch) and `log Gamma(y + 1)` once. Objective and working-state evaluations
use these terms with the same per-row arithmetic, compensated reductions and
finite-domain checks as before. Each separation/sample generation constructs its
own owner. Immutable Rust borrows prevent a changed sample, prior weight or
offset from silently inheriting this state. This owner contains no fitted means,
Hessian weights, working responses, FE geometry or iteration acceptance.

Canonical retained weighted-linear fitting constructs the original finite
predictor once and moves that vector from its draft into the installed session.
The independently accumulated raw-residual mean uses `response - predictor`;
the reported-intercept formula still uses independently weighted original-column
means. Numeric binding and the existing generated-term predictor kernel remain.
The coefficient-slot owner is built once and updated after the deferred intercept
normalization. The old temporary raw-residual construction and second original-
column predictor reconstruction are removed from this PPML session boundary.
One-shot OLS keeps its existing raw-residual construction.

Canonical within residuals retain their preconditioned rowwise arithmetic and
FMA order. Dense and implicit designs construct disjoint residual rows in the
installed thread team. Implicit columns are bound once for the scan instead of
rebinding each observation/column recipe. Scratch designs keep sequential reads.
This is shared canonical machinery for all PPML FE families, not a new solver or
a design/seed/width routing rule.

## Scientific authority

No check is removed, rescheduled or weakened. Initial and final canonical rank,
both inverse sides, direct dropped-span checks, fresh residual absorption,
finite scores, outer original-input scores, objective acceptance, two consecutive
convergence passes, reconstruction, separation and final-Hessian conditions
remain. The composite result independently replays its final objective from the
published mean and original outcome/prior weights without cached terms. The null
objective also retains its direct calculation.

Moving the finite predictor changes the raw-residual recurrence from successive
subtractions to subtraction of a separately constructed finite sum. Bitwise
identity of that recurrence is not required. Independent WLS/Poisson/Hessian
oracles and the established numerical envelopes remain authoritative. Cached
family calculations themselves preserve direct-evaluation bits in focused
changing-state and extreme-scale tests. Failed strict transactions retain their
prior predictor owners; speculative construction never overwrites them.

This refines the implementation/ownership choices of ADRs 0015, 0020 and 0139.
It changes no sample, rank policy, weight semantics, DoF, VCE, separation rule,
public interface, Stata/Rust boundary, SPI threading or publication transaction.

## Memory and accounting

The immutable term vector costs **16*N bytes** and is allocated only after
preflight. The PPML iteration envelope charges these two terms in addition to
its eight mutable vectors, across both fast and low routes. Capacity certificates
for those eight mutable vectors retain their original meaning. Terms are released
when the IRLS run returns; publication performs its independent replay directly.
Some cases' selected peak remains dominated by another phase.

The moved predictor replaces the canonical draft's prior N-sized raw-residual
owner. The existing strict overlap retains the previous transactional predictor;
there is no additional N-by-K design. Implicit residual bindings fit the
previously admitted binding envelope. Canonical predictor allocation reuse is
reported false because the draft transfers a newly constructed vector. Ordinary
warmed iteration allocation audits remain unchanged.

This is a time/memory tradeoff, not a claim of universally lower memory. The
source-bound report records increased tracked and modeled peaks and a one-byte
optional-storage refusal that still passes an independent Poisson oracle.
Physical-cap qualification and the allocation inventory's documented source
drift remain open.

## Evidence and limit

The [source-bound report](../../benchmarks/reports/development/ppml-accepted-state-local-2026-09-23/README.md)
records current baseline and candidate diagnostics, work counts, 495 distinct
Rust tests, 14 licensed fixture runs, independent explicit-design oracles,
matched stock rotations, fresh cases, OLS controls, and all timing losses.
The initial unmeasured prototype's generated-kernel omission was caught in source
review and corrected before any public benchmark.

The accepted change reduces canonical construction and repeated family work.
It is a bounded implementation milestone, not completion of the broader goal of
whole-command PPML at most 0.90 times comparable stock fixest.
