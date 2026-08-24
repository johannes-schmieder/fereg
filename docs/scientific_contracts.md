# Scientific contracts and OLS reference gate

Architecture, source presence, and native unit tests are not estimator
validation. Public `fereg` and `feppml` remain fail-closed.

Accepted scientific decisions are recorded under `docs/adr/`. ADR 0006
supersedes the temporary signed residual-DoF/negative-iid-variance
absorbed-slope route: successful OLS inference requires positive residual DoF.

## Core contracts

| Contract | Current accepted rule | Enablement requirement |
|---|---|---|
| Candidate and effective sample | Candidate exclusion precedes availability/domain rules; final compact rows and drop reasons remain aligned to original observations; cluster missingness precedes singleton deletion and cluster counts never define FE singletons | Exact sample and reason parity for every enabled model, weight, cluster, and output path |
| Weights | Unweighted, aweight, fweight, and pweight objective rules are explicit; fweights use exact multiplicity; pweight iid inference is rejected in favor of robust inference; iweight remains unsupported | Independent oracle plus pinned Stata parity for every public weight kind |
| Singleton pruning | Queue/fixed-point pruning is authoritative after initial sample rules; frequency multiplicity and `keepsingletons` are explicit | Exact retained sample, singleton count, and failure behavior |
| FE absorption | Successful solves require direct weighted projection certificates for every intercept and slope channel; allocation failure or nonconvergence is fatal | Independent explicit-dummy evidence and sealed estimator-level parity |
| Finite regression | Deterministic rank-aware solve; canonical kept/base/omitted/FE-collinear/numerically-rank-deficient statuses; no unpivoted inverse | Coefficient/status/bread certificates and Stata-facing naming parity |
| Residual DoF and iid covariance | Successful OLS inference requires `df_r > 0`; `V_iid = RSS/df_r * bread`; negative iid variance is not publishable | ADR 0006 positive-DoF slope recapture plus fail-closed nonpositive-DoF test |
| Robust and clustered covariance | HC1 and multiway cluster rules, corrections, nesting, and CGM repair are centralized and independently certified | Exact structure/count/DoF parity and registered numerical tolerances |
| Result transport | Rust validates all N/K/K² outputs before stores; private receipt `COMPLETE` is last; partial SPI writes have no logical meaning | Preserve receipt-last atomicity and Stata transaction rollback |
| Public activation | One substantive native call; no partial `e()` state; unsupported surfaces return explicit errors | Every documented semantic, output, postestimation, memory, performance, and platform gate exact-green |

## Absorbed-slope policy

The guarded intercept-plus-one-slope preparation and certified MAP engineering
may be retained. The active semantic case, however, must use the deterministic
48-row positive-DoF fixture and must be recaptured against the exact pinned
reference. The original 24-row over-absorbed case is a required
insufficient-observations failure test.

The signed result type and the 32-executable/8-blocked receipt produced by the
superseded route are historical evidence only. Until option A is exact-green,
the active accepted candidate remains 31 executable / 9 blocked at
`f61fee923db2bd3544d457dfe50de8117d27adda`.

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

Exact equality remains mandatory for success/error status, final sample,
coefficient omission/status, requested VCE/DoF policy, cluster counts, and
singleton accounting. Ill-conditioned or nonunique-FE cases are adjudicated on
fitted values, residuals, identified combinations, objective values, and
certificates rather than arbitrary FE normalizations.

## Performance gate

Performance is considered only after scientific parity. Primary cases require
fresh-process measurement, one cold plus at least five warm repetitions at the
registered thread counts, geometric-mean speedup of at least 1.5x, no primary
case slower than the reference, and no worse primary-case peak RSS.

## Current boundary

The hidden transport and licensed eclass transaction are developer-only.
Prediction, recovered/saved fixed effects, the remaining model/data surfaces,
public postestimation, low-memory completion, performance, packaging, and
cross-platform release validation remain open. See `STATUS.md` for the exact
current blocker inventory.
