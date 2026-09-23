# ADR 0125: reject the first prepared PPML coupling integration

Status: rejected implementation, 2026-09-23.

## Decision

Do not adopt `slcouple1` over ADR 0124. Keep the accepted observation-based
Schur operator in production. Prepared compact `D-C'C` and explicit Schur
products remain viable mathematical candidates, but the tested preparation
and storage organization does not justify its complexity or memory cost.

The candidate passes independent science checks. Three small public gains
(2.8–6.1%) accompany a 3.1% wide-case loss and 4–21 MB additional tracked
peak. On the wide diagnostic, preparation consumes 48.62 ms across eight
weight frames, including 27.22 ms building C and 17.02 ms building D. None
of 540 prepared applications uses the cancellation fallback. A successful
frozen operator comparison therefore does not establish a public speed gain.

## Successor requirements

Separate immutable symbolic topology and observation-to-block mappings from
changing-weight numeric products. Avoid rebuilding sparse patterns and using
a retained dense D construction bank for the compact route. Admit construction
and retained capacities, and let refusal preserve the already affordable solver.
Use existing explicit generation authority for any weighted reuse. Keep original
RHS reconstruction and independent current-weight acceptance authoritative.

The tested 20% operation-count margin is not an adopted routing policy. Do not
tune it to the observed cases, assume uniform superiority, or weaken tolerances.
A successor must measure preparation, products, reconstruction and whole public
runtime, with independent mathematical checks and fresh cases before adoption.
Existing dense response/control fusion already supplies a mathematical warm
start; another history bank needs an additional demonstrated benefit.

This decision changes no estimator policy, accepted production route, certificate
schedule, public interface or memory contract. The full performance objective,
including outer/checkpoint work, one-FE profiling, low-memory solving, categorical
routes and OLS protection, remains open.

## Evidence

The [source-bound development report](../../benchmarks/reports/development/ppml-prepared-coupling-probe-local-2026-09-23/README.md)
contains all successful and failed experiments, distributions, mathematical
checks, source/artifact identities and immutable receipts. Historical accepted
measurements remain attached to their original artifacts.
