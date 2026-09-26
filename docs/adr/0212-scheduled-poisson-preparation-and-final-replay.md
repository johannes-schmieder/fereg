# ADR 0212: schedule Poisson preparation and independent final replay

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

Use the existing resolved team for independent current-input PPML work that was
still serial: preparation of immutable outcome terms, final fitted/null likelihood
row contributions, and the independent fingerprint streams binding terminal
separation to its final sample. Preserve every mathematical check and its place
in the acceptance sequence. This extends the bounded row scheduling in ADR 0179;
it does not change the scalar formulas or independent replay contract of ADR 0195.

The measured million-row costs include about 40 ms for fresh final objective
replay and 62 ms for separation fingerprints across their construction and
validation. Immutable Poisson terms are prepared after initialization, within a
separate 33-ms portion of initial-state work. Initialization itself costs about
57 ms and is unchanged here. Direct final sample/output copies cost under 1 ms.
The evidence therefore selects execution organization before a larger ownership
rewrite. No new solver, persistent cache, FE routing, public option or worker
pool is introduced.

## Preparation and final replay

`PreparedPoissonInputs` continues to borrow one immutable outcome/prior-weight/
offset frame and owns the same two cached outcome terms. Assign independent rows
to its existing team at the existing N>=4096 threshold. The serial branch keeps
its traversal. A parallel failure returns the first failing row; partial private
terms are dropped. Cache lifetime, finite-domain rules and all current-weight
IRLS checks are unchanged.

Final replay remains separate and uncached. Compute the null mean through the
existing scaled/log fallback, then freshly derive each fitted deviance, fitted
likelihood and null-likelihood contribution from original outcomes, prior weights
and final means. A bounded three-double row tile is filled in parallel. Merge all
three compensated sums serially in original row order across tiles. This gives
the same scalar reduction bits without a parallel sum tree. No terms or objective
value from IRLS substitute for the independent replay.

The tile is capped by min(N,4096*T) and by the already admitted publication
validator workspace. Separation, PSD and Wald validators have returned and freed
their scratch before allocation. The complete actual tile capacity must fit that
allowance; no phase bound is increased. A tile smaller than 4096 rows, a small fit
or one worker retains scalar replay. Reservation is fallible and failure remains
atomic. No observation-by-regressor bank is added. The maximum T8 tile is 768 KiB.

## Separation fingerprints

Keep the exact fingerprint algorithm, domains, byte order, inputs and comparisons.
Only independent streams run concurrently: final row IDs and outcomes when minting
the replay token; those two streams plus the complete report when validating it.
Each stream remains serial and fresh. Cheap policy/generation checks retain their
early exit; digest result comparisons and error propagation retain their previous
order. Speculative results confer no authority and are discarded after failure.
No digest or mathematical certificate is reused across an invocation or generation.

Use only an already installed native team, at N>=4096 and more than one worker.
Standalone callers remain serial rather than creating an unrelated global pool.
Concurrency adds bounded stack tasks, not data banks. The established scoped
execution exchange keeps SPI calls and interruption handling on Stata's caller.

## Independent acceptance and tests

Original-input projection, finite solve, rank, score, model-space, objective,
reconstruction and final q*mu Hessian conditions are unchanged. The complete
separation/method/ledger validators remain. Weight semantics, samples, inference,
DoF, output ordering and transactional publication do not change.

Tests compare prepared terms and all final replay results with scalar arithmetic
across thread counts, partial tiles, changed/extreme states, zero and fractional
outcomes, and exact/one-byte-short budgets. Invalid inputs retain first-observation
errors and successful retry. Independent analytic likelihood and weight-replication
oracles remain. Threaded seals match scalar bytes and reject mutated row, outcome,
report and combined bindings with the same error precedence. Public explicit-design
Poisson/Hessian and large-sample independent objective/null checks are required
in addition to reference agreement.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ppml-scheduled-final-work-local-2026-09-25/README.md)
retains all sources, distributions, failed/interrupted attempts and separate
instrumented artifacts. Large development numeric medians improve 6.5–8.7%.
Fresh fast737280 improves 765->713 ms against stock 769; its paired ratios
0.926–0.934 still miss 0.90. Five of seven fresh families with valid clocks meet
0.90 in every pair. Repeated contention leaves the eighth family's clocks
unranked; all completed science passes. Four-slope low remains 123/111 ms.
No constants are tuned on consumed seed 202609442.

Complete clean diagnostic fast pairs reduce final composite validation 88.27->46.28
ms and its included independent likelihood replay 41.00->17.64 ms. The initial-state
portion outside initialization falls 33.35->21.43 ms. Initialization and canonical
solving do not materially change. Fingerprint stream durations overlap and are
not additive to boundary wall time.

141 focused Rust tests, six original licensed fixtures, 262 public states and 12
explicit Poisson/Hessian/sandwich arm checks pass. All 86 completed native pairs
have exact scientific exports; 164 independent null-statistic states pass. The
existing minor-version allocation-fixture failure is reproduced on both artifacts;
separate diagnostic copies with only that assertion/sentinel corrected pass.
Stock discrepancies, 21 incomplete anchored comparisons, six detected overlaps and
a pre-measurement diagnostic stop remain visible. 17 OLS pairs have exact exports;
small losses and the initial factor 222->244 ms block remain, while its unchanged
five-pair repeat ties 210/210. This does not establish universal non-regression.

Tracked peak and scratch deltas are zero; modeled bounds differ by at most 48
bytes. No new allowance, N-by-K bank, physical-cap qualification or static-inventory
repair. The broader multi-FE 0.90 goal needs further work on measured initialization,
prepared-model construction and remaining canonical costs, not another tuning
cycle around this scheduling threshold.
