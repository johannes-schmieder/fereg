# ADR 0219: fixed compensated blocks for PPML objectives

Status: accepted for local development, 2026-09-26. Not qualification.

## Decision

Replace ADR 0179's IRLS objective row-term tile and serial row fold with fixed
4096-observation blocks on the existing installed team. Each block evaluates
unchanged Poisson row terms from the current eta/mean and immutable prepared
outcome terms, maintaining separate compensated sums for deviance and likelihood.
Merge each sum and its correction separately, in block order. Block boundaries
and merge order do not depend on thread count, scheduling or bank capacity.
Small and one-thread evaluations retain the scalar implementation.

This explicitly supersedes the serial-fold implementation requirement in
ADR 0179 for intermediate IRLS objectives. It does not change a tolerance or
acceptance condition. Bitwise agreement with a different reduction recurrence
is not a mathematical requirement; independent scientific envelopes govern it.
Repeated scheduled evaluation is deterministic for the same inputs. Independent
final objective/null replay under ADR 0212 remains uncached and uses the original
serial compensated fold of freshly formed original-input terms.

## Validity and failure behavior

The partial bank is private scratch for one evaluation. It carries no accepted
point, weight-frame, rank, projection or Hessian authority. Every call overwrites
its used entries. Original-input FE/finite projection, score, model-space,
objective, rank, reconstruction, consecutive convergence and final q*mu Hessian
checks retain their frequency, inputs and tolerances. Samples, separation,
weights, DoF, inference and transactional publication are unchanged.

Workers return the first invalid observation in their fixed block; ordered
failure selection retains the original first-row error. Nonfinite partials or
final accumulation fall back to the original serial evaluation, which still
visits every row before judging total overflow. A speculative partial failure
cannot publish a result or hide a later input-domain error.

## Ownership and memory

Reserve four doubles per simultaneously processed block: two sum/correction
pairs. The bank has 4*min(T,ceil(N/4096)) doubles for N>=4096 and T>1, otherwise
zero. Row-derived sizing bounds multiplication even at address-space limits.
Existing fallible reservation, admission and actual-capacity telemetry charge
it beside all eight mutable and two immutable family vectors on both routes.
There is no per-iteration heap allocation, extra pool or observation-by-regressor
bank. At large T8 the objective bank falls from 512 KiB to 256 bytes; at T4 from
256 KiB to 128 bytes. Independent final replay storage is unchanged.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-objective-blocks-local-2026-09-26/README.md)
retains every rotation, loss, discrepancy and source/artifact identity. Paired
diagnostics reduce large objective work 32.093->22.322 ms fast and
31.866->25.387 ms low. Accepted-mean recomputation accounts for only a few
avoidable milliseconds, so no accepted-point cache was implemented.

Large public medians improve 0.4–2.1% in the pilot and 0.9–1.2% on fresh native
comparisons. This modest saving and up to 524,032 fewer observed peak bytes justify
the small shared change. Small controls remain mixed. The initial 159/162-ms
OLS loss does not repeat: identical-input follow-up is 162/160 with three wins,
one loss and one tie. No source or scheduling constant was tuned between blocks.

135 focused Rust tests, eleven licensed fixtures, 237 public-state audits and
12 explicit Poisson/Hessian/sandwich comparisons pass. Real T4 categorical and
slope 16-iteration audits retain zero allocation events and growth. All 79 native
PPML and 23 OLS export pairs are exact. The inherited checkpoint-count assertion
fails identically on the exact baseline and candidate; it remains unresolved.

Fresh seed202609449 is consumed. Both large fresh designs exceed the unchanged
stock fitted-mean comparison envelope and are unranked, despite passing individual
state checks and native comparisons. The older small saved-output discrepancy
also remains. Comparable stock ratios meet 0.90 in every pair for four of six
pilot families and five of seven comparable fresh families. Four-slope low and
disconnected low still miss; large pilot low remains slower than stock. This
milestone does not complete the broader multi-FE performance goal. No new
qualification, static-inventory repair, SCC run, push or release is claimed.
