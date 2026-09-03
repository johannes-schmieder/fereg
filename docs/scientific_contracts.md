# Scientific contracts and OLS reference gate

Architecture, source presence, and native unit tests are not estimator
validation. Public `fereg` OLS and `feppml` PPML are implemented for 0.5.0 but
are not source-qualified until the exact candidate passes their release gates.
Windows licensed-Stata testing and formal cross-platform qualification are
currently deferred.

Accepted scientific decisions are recorded under `docs/adr/`. ADR 0006
supersedes the temporary signed residual-DoF/negative-iid-variance
absorbed-slope route: successful OLS inference requires positive residual DoF.

## PPML public contracts

PPML outcome, public weight, offset/exposure, initialization, IRLS,
rank, DoF, robust/cluster VCE, prediction, and saved-effect policies are fixed by
[ADR 0016](adr/0016-ppml-scientific-contract.md). Certified separation and its
sample-ledger/singleton ordering are fixed independently by
[ADR 0017](adr/0017-certified-ppml-separation-ledger.md). These policies do not
inherit OLS correction or reporting rules merely because both estimators use
the shared mechanisms of ADR 0015.

The exact pinned `ppmlhdfe` closure is the compatibility reference, and small
explicit-dummy Poisson and recession-cone calculations are the independent
authority. The prepared PPML transaction is the sole production estimator;
obsolete alternate production paths were removed or made explicitly
test-only. Public and hidden Stata fixtures and the one-call wrapper make
these contracts executable through `feppml`, but source presence and public
availability are not scientific qualification. The scientific, memory,
transaction, platform, and `fereg` non-regression gates in ADRs 0016--0020
remain the release boundary, with Windows and the final aggregate deferred by
explicit qualification decision.

For cross-estimator PPML objective adjudication,
[ADR 0026](adr/0026-ppml-comparator-objective-adjudication.md) makes the exact
pinned `ppmlhdfe` fit's reported `e(ll)` and `e(deviance)` the authoritative
reference objective, against which fereg must be no worse within the registered
comparison tolerance. Its public predicted mean, replayed objective, KKT
score, and model-space projection remain required diagnostics. The predicted
state is an additional objective witness only when independent checks certify
both model-space feasibility and stationarity; a projected state alone is not
a witness. Fereg's own published mean must still replay its reported objective,
lie in its declared model space, and pass every registered score, solver, rank,
sample, and result certificate.

## Core contracts

| Contract | Current accepted rule | Enablement requirement |
|---|---|---|
| Candidate and effective sample | Candidate exclusion precedes availability/domain rules; final compact rows and drop reasons remain aligned to original observations; cluster missingness precedes singleton deletion and cluster counts never define FE singletons | Exact sample and reason parity for every enabled model, weight, cluster, and output path |
| Weights | Unweighted, aweight, fweight, and pweight objective rules are explicit; fweights use exact multiplicity; pweight iid inference is rejected in favor of robust inference; iweights reproduce the pinned `r(101)` rejection | Independent oracle plus pinned Stata parity for every public weight kind/error |
| Singleton pruning | Queue/fixed-point pruning is authoritative after initial sample rules; frequency multiplicity and `keepsingletons` are explicit | Exact retained sample, singleton count, and failure behavior |
| FE absorption | Successful solves require direct weighted projection certificates for every intercept and slope channel; allocation failure or nonconvergence is fatal | Independent explicit-dummy evidence and sealed estimator-level parity |
| Finite regression | Deterministic rank-aware solve; canonical kept/base/omitted/FE-collinear/numerically-rank-deficient statuses; no unpivoted inverse | Coefficient/status/bread certificates and Stata-facing naming parity |
| Residual DoF and iid covariance | Successful OLS inference requires `df_r > 0`; `V_iid = RSS/df_r * bread`; negative iid variance is not publishable | ADR 0006 positive-DoF slope recapture plus fail-closed nonpositive-DoF test |
| Robust and clustered covariance | HC1 and multiway cluster rules, corrections, nesting, and CGM repair are centralized and independently certified | Exact structure/count/DoF parity and registered numerical tolerances |
| Result transport | Rust validates all N/K/K² outputs before stores; private receipt `COMPLETE` is last; partial SPI writes have no logical meaning | Preserve receipt-last atomicity and Stata transaction rollback |
| Public transaction | One substantive native call; no partial `e()` state; unsupported surfaces return explicit errors | Every documented semantic, output, postestimation, memory, performance, and platform gate exact-green |

## Absorbed-slope policy

The guarded intercept-plus-one-slope preparation and certified MAP engineering
may be retained. The active semantic case, however, must use the deterministic
48-row positive-DoF fixture and must be recaptured against the exact pinned
reference. The original 24-row over-absorbed case is a required
insufficient-observations failure test.

The signed result type and its receipts are historical evidence only. The old
32/40 option-A receipt remains immutable at
`benchmarks/reports/captured/darwin-arm64/stata18/ols/20260824T161810Z-9f2494d`;
it is not evidence for the active public candidate. `STATUS.md` records the
current executable milestone and qualification gaps.

## Pinned absorbed-intercept and bread contract

Pinned `reghdfe` defines the reported normalized absorbed intercept by:

```text
b_cons = mean_w(y) - mean_w(X) b
side   = -B mean_w(X)
corner = 1 / N_ref + mean_w(X)' B mean_w(X)
```

`B` is the inverse within-X cross-product on canonical kept finite coordinates.
The reported `_cons` is a normalization coordinate, not another finite or
absorbed rank direction. Cross-product and effective-N scales follow the
registered unweighted/aweight/fweight/pweight rules.

## Reference and differential evidence

The frozen OLS manifest is the sole semantic command/input inventory. Reference
capture must:

- use the exact pinned `reghdfe`, `ftools`, and `require` commits and locked
  tracked inputs in an isolated ado environment;
- bind every fixture, generator, command, requested scalar/matrix/output, and
  case order;
- record canonical Stata hexadecimal values and complete matrix dimensions;
- require explicit terminal sentinels and Stata-level return codes;
- exclude raw logs, local paths, usernames, license data, and unrelated files;
- seal the canonical JSON payload and commit a read-only sanitized receipt.

A reference receipt proves only reference behavior. fereg parity requires a
separate source-bound differential plus independent mathematical/oracle
evidence.

## Numerical adjudication

For well-conditioned cases, use the registered tolerances:

- coefficients: `rtol=1e-7`, `atol=1e-10`;
- standard errors/VCE: `rtol=1e-6`, `atol=1e-10`;
- residuals and fitted values: `rtol=1e-7`, `atol=1e-9`.

For bounded LSMR fixtures, the authoritative target is the sealed 100-digit
explicit-design oracle, with one grounded FE coordinate per connected
component. It seals residuals, fitted values, weighted objectives, and
checksums for weak chains, barbell and disconnected graphs, AKM mobility,
unequal weights, multiple RHSs, and absorbed slopes. An independent
observation-space projection audit complements the runtime rank-aware FE
certificate in tests and diagnostics.

For large automatic LSMR routes, qualification additionally compares the
returned residual to a solve at 100 times tighter internal tolerance. Reghdfe
residual distance is reported but is not a hard gate when oracle/stability and
all runtime FE and OLS score certificates pass. Reghdfe remains hard for exact
sample/status/singleton/DoF behavior and the registered coefficient, VCE, RSS,
and fit-statistic envelopes.

Exact equality remains mandatory for success/error status, final sample,
coefficient omission/status, requested VCE/DoF policy, cluster counts, and
singleton accounting. Ill-conditioned or nonunique-FE cases are adjudicated on
fitted values, residuals, identified combinations, objective values, and
certificates rather than arbitrary FE normalizations.

## Performance gate

Performance is considered only after scientific parity and on approved cases
with at least 10 million observations. Fresh-process paired same-node jobs must
record one cold plus at least five warm repetitions at registered thread
counts. The candidate must have geometric-mean runtime no more than 40 percent
of `reghdfe`, at least 80 percent of cases at least 2x faster, single-thread
speed at least 1.5x, and no unexplained case more than 25 percent slower.

Measured native allocation must be within 25 percent of its route-specific
model, repeated calls must not leak, constrained-memory planning must select a
fitting certified route or fail before large allocation, and the execution-mode
case must show low succeeding where fast cannot. Process-RSS ratios remain
diagnostic rather than release thresholds.

## Current boundary

The OLS and PPML surfaces, public postestimation, saved outputs, and low-memory
execution are implemented. Exact-SHA full scientific qualification,
the governing performance/memory gates, deterministic packaging, Windows
licensed-Stata testing, and cross-platform release validation remain open. See
`STATUS.md` for the current blocker inventory.
