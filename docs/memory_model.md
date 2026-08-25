# 0.9.0 memory model

## Modes and preflight

`fereg` accepts `memory(fast|low|auto)`. `estimate_phase2_memory()` computes
checked fast/low estimates from observations, finite regressors, absorbed and
cluster dimensions, and the effective thread pool. `plan_ols_memory()` compares
those estimates with available memory:

- fast is selected only when its estimate fits;
- low is selected only when its estimate fits;
- auto prefers fast and falls back to low;
- if neither fits, estimation returns the allocation error mapped to `r(909)`
  before large working allocations.

The `FEREG_MEMORY_LIMIT_BYTES` test seam supplies a deterministic artificial
limit for constrained-memory qualification; it is not a public Stata option.

## Fast and low execution

Fast execution holds the prepared outcome/regressor batch and in-memory score
workspaces. For large multi-thread absorbed models, its preflight also reserves
the worst-case level-by-RHS group-mean table used by deterministic parallel MAP
projection. Low execution includes the corresponding outcome-plus-pool-width
bound, generates columns blockwise, and stores the prepared QR/score columns in
private bounded scratch files. Scratch files:

- use unique process/sequence names in the platform temporary directory;
- validate every dimension, offset, read, write, and finite value;
- keep bounded fallible I/O buffers rather than a second resident design;
- are removed by `Drop` on success, ordinary error, panic unwinding, or Break
  propagation.

Fast and low modes share the same rank policy, certificates, inference,
omission status, result wire, and Stata transaction. Registered tests require
scientific identity, including deterministic results across thread counts.

## Allocation discipline

Observation-scale vectors and matrices use checked arithmetic, fallible
reservation, and explicit capacity validation before mutation. Preparation,
factorization, topology, MAP, QR, score, VCE, result-container, and scratch
paths have allocation/store-failure injection tests. An allocation failure
cannot produce a complete receipt or partially committed public result.

Unit weights remain structural. Categorical and FE recipes reuse compact source
codes; interaction keys and cluster subset scores are streamed rather than
retained as dense observation-by-arity or group-by-parameter tables.

## Instrumentation and acceptance

The public result wire records requested/selected memory mode, estimated fast
and low bytes, selected-model bytes, scratch peak bytes, allocator current and
peak bytes, allocation delta/ratio, process RSS start/current/peak/delta where
the OS exposes them, and action timings. `timeit` displays the registered
summary without changing computation.

Instrumentation is evidence, not a self-certifying claim. Exact-SHA
qualification must show:

- RSS no worse than reghdfe in at least 80 percent of approved cases;
- no unexplained RSS ratio above 1.25;
- measured native allocation within 25 percent of its model;
- no repeated-call leak; and
- a constrained case where low succeeds and fast cannot.
