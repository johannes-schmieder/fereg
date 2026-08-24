# Native data plane

The data plane is the Stata-independent boundary that turns a validated request
and candidate marker into one authoritative compact sample and the source
representations required by the native estimator.

Public estimator dispatch remains disabled.

## Preparation order

For each original row, candidate exclusion is applied first. On candidate rows,
the current numeric preparation orders outcome, ordinary-regressor, weight,
fixed-effect, cluster, and adjustment availability/domain rules according to the
registered drop-reason contract. A fatal domain error is not converted into a
drop.

After initial compaction:

1. supported FE sources are factorized;
2. the singleton fixed point is computed, with frequency multiplicity and
   `keepsingletons` handled explicitly;
3. all aligned arrays are compacted once more if rows were removed;
4. categorical sources are re-factorized on the final sample;
5. ordinary factor/interaction recipes and FE/cluster topology are rebuilt
   against final codes.

The original-row map and full-axis reason mask remain authoritative.

## Numeric sources and weights

Each numeric source carries raw `f64` values and a separate Stata-missing mask.
Retained numeric bits are not normalized or rewritten merely for storage.

- Unweighted input is structural; no vector of ones is allocated.
- Analytic and probability weights must satisfy their registered positive
  finite domains.
- Frequency weights must be positive finite integers representable in the
  supported multiplicity type.
- Importance weights remain unsupported.
- Candidate-excluded observations are not read for scientific domain checks.

Weight normalization needed by a numerical routine is local to that routine and
must preserve the registered estimator semantics.

## Categorical recipes and topology

Numeric factor sources are collision-safely factorized into compact codes.
All-categorical ordinary interactions are stored as small component recipes and
are evaluated from shared codes; no observation-by-arity tuple table is
retained. Final-sample rebasing removes vanished levels from the logical graph
while preserving canonical omission/status semantics.

Prepared FE topology owns compact source identity and membership once per unique
final source. Repeated terms reuse those objects. Intercept-only borrowed MAP
views copy neither observation-scale codes nor membership.

The guarded intercept-plus-one-slope path borrows a continuous source on the
same final row axis and may be retained as engineering under ADR 0006. It does
not authorize signed residual DoF or public slope inference. The active slope
case must be rebound to the positive-DoF fixture and resealed before it becomes
an accepted executable semantic case.

Strings, general mixed interactions, multiple/slope-only FE terms, saved FE
outputs, time-series materialization, and public syntax remain outside the
accepted data-plane surface.

## Prepared Y/X and solver boundary

The current fast-style prepared builder validates the sample, source recipes,
coefficient mapping, weights, and dimensions; checks `1+K` and `N*(1+K)`; then
fallibly reserves one row-major batch with Y first and generated finite columns
in canonical order. Syntax-omitted slots are not materialized.

Strict MAP consumes the batch. All solver/certificate workspaces are reserved
before mutation; failure cannot expose a partially residualized prepared object.
A successful result requires direct final weighted projection certificates.

The completed low-memory estimator must replace full materialization where
necessary with block generation, streaming cross-products/scores, and bounded
scratch storage while preserving identical scientific results.

## Hidden integration boundary

The developer-only hidden OLS action loads each bound source once, constructs
one authoritative sample, selects one accepted certified inference composition,
and writes only private versioned shadows. Rust validates every observation-,
coefficient-, and covariance-scale output before stores. Receipt completion is
last; the outer ado helper provides prior-estimate rollback and the final Stata
transaction.

This hidden path is evidence infrastructure, not public estimator availability.

## Allocation and failure policy

All potentially large vectors, tables, strings, and matrix dimensions use
checked arithmetic and fallible reservation. Production loops must not assume
infallible growth. Allocation, overflow, invalid identity, nonfinite data,
interruption, nonconvergence, or failed certificates return an explicit error
and no logically complete result.

Detailed memory bounds are in `memory_model.md`; protocol and transaction
details are in `plugin_protocol.md`.
