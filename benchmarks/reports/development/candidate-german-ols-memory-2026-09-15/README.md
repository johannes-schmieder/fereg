# German OLS memory reproduction

One bounded native-only run on the unchanged German 5% input completes in
**385.325 seconds**, retaining **4,361,733 rows**. It reproduces
the public base-only reporting discrepancy: **3,570,575,392 tracked native bytes**
versus **2,841,380,024 base bytes**. The separately admitted initial CG workspace is
**2,191,388,408 bytes**; their sum, **5,032,768,432**, exceeds the observed peak and fits
the **34,359,738,368-byte** usable budget. This does not demonstrate a cap breach or
establish complete action-memory coverage. The complete-action ratio stays null.

Samples, order, all saved output columns, coefficients, covariance, rank/status,
DoF, fit statistics and scientific certificates are exactly equal to the
preserved native output. The original reference residual-parity failure and
proposed ADR 0073 decision remain unchanged. No reference or full-data fit was
restarted. This single reproduction is not a timing comparison or qualification.

The source and artifact are the already tested isolated accounting snapshot
from [the fixed cost check](../candidate-memory-costs-linux-2026-09-15/README.md),
not a newly frozen candidate. [Validation](validation.json) binds the source
inventory, plugin, input and reference lock and records job **7586503**, successful
scheduler/process/Stata/FIT return codes and the explicit terminal sentinel.
Raw licensed logs and data stay outside Git. Complete runtime and solver matrices
are preserved here; RSS is 4,421,021,696 bytes and scratch is
0 bytes. Public OLS diagnostics do not separately time
discarded attempts; total and solver elapsed times include their costs.

[CG reconciliation](cg-reconciliation.json) applies the checked source formula
to the final retained dimensions. It does not infer a multiplier from measured
allocation. The unchanged public base field excludes that solver increment.
Versioned complete-action coverage, parent lifetime reconciliation, the static
allocation inventory, Mac prerequisites, candidate freeze and package/install
gates remain open. Harness copies normalize execution-root paths; validation
retains hashes of the literal executed files.
