# ADR 0016: PPML scientific contract

**Status:** Accepted
**Date:** 2026-08-31
**Decision owner:** Johannes Schmieder

## Context

The source tree contains an early PPML implementation, but it predates the
prepared OLS architecture and is not a public scientific contract. In
particular, it imposes arbitrary fixed linear-predictor bounds, can compute a
final rank and bread from a state different from the accepted predictor,
reuses OLS DoF, and applies incorrect fweight and cluster covariance scaling.
Its unit tests cannot qualify public `feppml` behavior.

The compatibility reference is `ppmlhdfe` at commit
`b85665d8f674e93c1e07446a9a2b29be7f797910`, with its exact pinned `reghdfe`
closure in `references.lock.toml`. Reference parity is necessary but not
sufficient: small models also require an independently implemented
explicit-dummy Poisson oracle.

This ADR fixes the initial `feppml` scientific surface. It does not activate
the public command.

## Decision

### Model, candidate sample, and constant

1. On retained observation `i`, the model is

   ```text
   eta_i = offset_i + X_i beta + d_i
   mu_i  = exp(eta_i)
   ```

   where `d_i` is the sum of absorbed fixed-effect channels. The PPML
   first-order condition uses the pseudo-score `q_i (y_i - mu_i)`.
2. Outcomes must be finite and nonnegative. Fractional non-count outcomes are
   valid. Missing outcomes are sample exclusions; a finite negative outcome
   on an otherwise eligible candidate row is invalid data (`r(459)`). Values
   outside `if`/`in` are not inspected for scientific domain errors.
3. Stable sample construction applies the candidate restriction, then outcome,
   regressor, prior-weight, fixed-effect, cluster, and offset/exposure
   availability and domain rules in the registered ledger order. Every
   exclusion retains its original-row identity and reason.
4. The model always has the structural `_cons` normalization used by the
   pinned comparator. With no absorbed intercept it is an ordinary reported
   coefficient. An absorbed intercept can span the constant, in which case it
   is not an additional identified rank direction.
5. A final sample with no observations or no positive weighted outcome has no
   finite PPML intercept solution and returns insufficient observations
   (`r(2001)`) without a result.

### Public weight semantics

6. The initial public surface supports no weight, `fweight`, and `pweight`,
   matching the pinned comparator. `aweight` and `iweight` are rejected by
   syntax with `r(101)`; their presence in the old placeholder parser did not
   promise support.
7. Define the prior computational weight `q_i` as follows:

   - unweighted: `q_i = 1` and effective N is the physical retained count;
   - fweight: `q_i` is the exact positive integer multiplicity and effective N
     is its checked sum;
   - pweight: raw values are finite and strictly positive, then normalized as
     `q_i = raw_i * N_physical / sum(raw)`; effective N is the physical
     retained count.

   Raw pweight sum and expression remain available as metadata. Common
   pweight rescaling must leave coefficients, objective comparisons, bread,
   covariance, residuals, and fit statistics invariant.
8. Missing weights exclude rows. An fweight that is nonpositive,
   nonintegral, nonfinite, or outside the supported exact multiplicity range
   returns `r(401)`. A nonpositive or nonfinite pweight returns `r(459)`.
   Candidate-excluded invalid values are ignored.
9. Fweights mean literal replication for the objective, effective N, robust
   meat, cluster score, DoF, and all reported weighted sums. An implementation
   shortcut may not replace that contract with squared physical-row weights.

### Offset and exposure

10. `offset()` and `exposure()` are mutually exclusive (`r(198)`). A missing
    offset or exposure excludes the row.
11. A retained offset must be finite. A retained exposure must be finite and
    strictly positive; a nonpositive candidate exposure returns `r(459)`.
    Exposure is transformed once to `log(exposure)` in the Rust computation
    plane.
12. Offset and exposure affect initialization, eta, mean, likelihood,
    residuals, and predictions. They do not enter the homogeneous separation
    cone and therefore cannot by themselves change a certified separation
    direction.

### Initialization and finite evaluation

13. With prior weights `q`, initialize the structural intercept
    deterministically and offset-aware:

    ```text
    c0 = log(sum_i q_i y_i) - log(sum_i q_i exp(offset_i))
    eta_i = offset_i + c0
    ```

    Both logarithmic sums are evaluated with scaled or log-sum-exp
    accumulation. The initial finite coefficients and FE coordinates are zero
    subject to the constant normalization.
14. The fixed `[-29.9, 29.9]` eta bound in the stale implementation is
    rejected. Production code does not clip eta or mu to manufacture a finite
    iterate.
15. Deviance and pseudo-log-likelihood are evaluated in stable eta form. For
    positive outcomes, working ratios use `exp(log(y_i) - eta_i)` where that is
    safer than forming `y_i / mu_i`; for zero outcomes, the exact limiting
    formula is used.
16. An accepted state must have finite representable eta, strictly positive
    finite mu, objective, working weights, response, moments, coefficients,
    and certificates. A nonrepresentable full step is backtracked. If no
    representable improving step can pass the score certificate, estimation
    fails instead of clipping or publishing a boundary value.

### IRLS, line search, and convergence

17. At an accepted state, the Fisher/IRLS subproblem uses

    ```text
    h_i = q_i * mu_i
    z_i = eta_i - offset_i + (y_i - mu_i) / mu_i
    ```

    with the exact zero-outcome expression
    `z_i = eta_i - offset_i - 1`.
18. Default outer tolerance is `1e-8`; default maximum iterations is 10,000.
    The registered relative deviance criterion is

    ```text
    abs(D_new - D_old) / max(min(D_new, D_old), 0.1).
    ```

19. A full Fisher step is tried first. Backtracking halves the coefficient and
    fitted-predictor step together until every state is finite and deviance is
    nonincreasing, allowing only a registered roundoff-scale objective slack.
    Exhausting the step budget is fatal.
20. Apparent convergence requires two consecutive accepted iterations that
    satisfy both the deviance criterion and an independently recomputed,
    normalized finite-regressor and FE score certificate.
21. Apparent convergence triggers a fresh strict weighted-linear solve at
    inner tolerance `min(1e-12, 0.1 * outer_tolerance)`. If the strict solve
    changes beta, eta, kept basis, rank, or objective beyond the registered
    publication tolerance, its state becomes the next IRLS state and iteration
    resumes.
22. Publication additionally requires independent Hessian/bread, rank, FE
    projection, score, objective, and
    `eta = offset + X beta + d` reconstruction certificates. Nonconvergence or
    any failed final certificate returns no result.

### Rank and regressor status

23. Canonical rank selection occurs only after the final accepted singleton
    and separation drops and one atomic topology rebase. A separation drop may
    make a regressor collinear; the basis is therefore recalculated once on
    that final sample.
24. The canonical kept-column basis is then frozen for the fit. Strictly
    positive IRLS weights do not change mathematical model rank, so an
    iteration-specific numerical route cannot silently change public
    coefficient status.
25. Base/omitted factor columns, columns spanned by fixed effects, and
    numerically rank-deficient finite columns remain distinct statuses in
    canonical input order.
26. The frozen basis is revalidated against the final Hessian. A final
    Hessian rank loss or a different rank-revealing decision is fatal, not a
    silent last-iteration omission.

### PPML DoF and covariance

27. PPML does not call the OLS DoF or small-sample policy. The shared topology
    mechanism computes absorbed rank and cluster nesting on the final PPML
    sample, after which the PPML adapter applies this ADR.
28. Structural residual DoF is

    ```text
    df = N_effective - finite_within_rank - absorbed_rank.
    ```

    For clustered inference, reported DoF is additionally capped at
    `G_min - 1`. Positive reported DoF and at least two groups in every
    declared cluster dimension are required for publication.
29. The public default is robust covariance. `vce(unadjusted)` is mapped to
    robust for pinned compatibility. Model-based IID covariance is not in the
    initial public surface. Clustered covariance supports one through five
    dimensions after each dimension and subset path is qualified.
30. On canonical kept within-design rows `x_tilde_i`, the bread is the inverse
    of

    ```text
    H = sum_i q_i mu_i x_tilde_i x_tilde_i'.
    ```

31. Robust meat follows the literal observation experiment:

    - unweighted and pweight rows contribute the outer product of
      `q_i (y_i-mu_i) x_tilde_i`;
    - an fweighted physical row contributes `f_i` times the outer product of
      `(y_i-mu_i) x_tilde_i`, matching `f_i` literal copies rather than a
      squared fweight.

    The robust asymptotic correction is `N_effective/(N_effective-1)`.
32. A cluster group score is

    ```text
    S_g = sum_(i in g) q_i (y_i - mu_i) x_tilde_i.
    ```

    Multiway subset meats use deterministic collision-safe grouping and CGM
    inclusion--exclusion. One common correction `G_min/(G_min-1)` is applied;
    the stale subset-specific and OLS `(N-1)/df` corrections are rejected.
33. The shared deterministic CGM positive-semidefinite repair is applied, when
    required by the registered policy, only after complete inclusion--
    exclusion. Production and independent covariance certificates remain
    distinct.
34. Public tests are z and Wald chi-squared, not OLS t and F tests. Coefficient
    stripes, omitted statuses, covariance dimensions, rank, DoF, cluster
    counts, and correction choice are exact-parity fields.

### Objective and returned statistics

35. The reported deviance is

    ```text
    D = 2 sum_i q_i [mu_i - y_i + y_i log(y_i/mu_i)],
    ```

    with the zero-outcome contribution `2 q_i mu_i`. The pseudo-log-likelihood
    uses `y_i eta_i - mu_i - log Gamma(y_i+1)` on the same final weighted
    sample.
36. For pinned compatibility, the null likelihood is the constant-only
    pseudo-likelihood at

    ```text
    mu_0 = sum_i q_i y_i / sum_i q_i,
    ll_0 = sum_i q_i [y_i log(mu_0) - mu_0 - log Gamma(y_i+1)].
    ```

    It deliberately does not absorb fixed effects or apply the fitted model's
    offset. Report pseudo R-squared as `1 - ll/ll_0`. The independent oracle
    and reference manifest cover offsets, fweights, and fractional outcomes so
    this comparator convention cannot drift into an undocumented alternative.
37. The initial public result records coefficients and covariance with
    canonical stripes; retained and effective N; rank and PPML DoF; deviance,
    `ll`, `ll_0`, pseudo R-squared, and Wald chi-squared; convergence and
    IRLS/inner-iteration counts; singleton and separated counts; FE/cluster
    metadata; weight, offset/exposure, separation, route, memory, timing, and
    certificate diagnostics. OLS RSS, TSS, R-squared, RMSE, t, and F fields are
    not borrowed merely because the shared linear engine computes intermediate
    weighted residuals.

### Predictions and residuals

38. Public predictions are defined on `e(sample)` and missing elsewhere:

    | Request | Value |
    |---|---|
    | `xb` | offset plus finite predictor, excluding absorbed contribution |
    | `d` or `absorbed` | total absorbed contribution |
    | `eta` or `xbd` | `offset + X beta + d` |
    | default or `mu` | `exp(eta)` |
    | `response` | `y - mu` |
    | `score` | raw public residual `y - mu` |
    | `pearson` | `(y - mu) / sqrt(mu)` |
    | `working` | `(y - mu) / mu` |
    | `deviance` | `2[mu-y+y log(y/mu)]`, with zero case `2mu` |
    | `anscombe` | `1.5(y^(2/3)-mu^(2/3))/mu^(1/6)` |

    `deviance` deliberately preserves the pinned per-row contribution rather
    than a signed square-root residual. Unsupported prediction types fail
    closed. Prediction availability also obeys the requested-output-only
    memory contract in ADR 0018; a fit may not retain an unrequested hidden
    O(N) `d`, `eta`, or `mu` vector merely to make a later prediction work.

    - `xb` is always available. It is reconstructed from the fitted finite
      coefficients, the current finite covariates, and the current
      `offset()` or `exposure()` variable.
    - Without absorbed effects, `d`/`absorbed` is identically zero and `eta`,
      `mu`, and every residual statistic are reconstructed from the current
      `xb`. Thus a fit with no requested observation-level outputs supports
      the complete prediction surface.
    - With absorbed effects, `d`/`absorbed` requires a compatible retained
      `d()` output. `eta`, `mu`, and every residual statistic require a
      compatible retained `d()`, `eta()`, or `mu()` output. If none was
      requested and retained at estimation time, prediction returns `r(498)`
      before creating the target variable; `xb` remains available.
    - A retained `d()` is authoritative when more than one compatible output
      exists: `eta` is reconstructed as the current `xb` plus retained `d`,
      even if retained `eta()` or `mu()` snapshots are also present. Only when
      `d()` is absent may retained `eta()`, then retained `mu()`, serve as a
      snapshot fallback. Those fallbacks deliberately do not promise to track
      post-estimation edits to finite covariates or adjustment variables.
    - `d` and `absorbed` are one request, as are `eta` and `xbd`; specifying
      both members of either alias pair is invalid syntax (`r(198)`) and
      creates no target variable.

39. The public score residual is unweighted. The internal sandwich score
    remains prior-weighted according to items 31--32.

### Saved fixed effects

40. Saved fixed effects are recovered only from the final accepted strict
    state using final Hessian weights `q_i mu_i`. Normalization is
    deterministic by connected component through the shared recovery
    mechanism parameterized by those weights.
41. The total and individual saved channels must independently certify

    ```text
    eta_i = offset_i + X_i beta + sum_j FE_ij
    ```

    and the individual channels must sum to the published total absorbed
    predictor on every retained row.
42. Requesting saved effects cannot change the final sample, beta, eta, mu,
    rank, objective, covariance, or iteration path. Individual FE values are
    normalized coordinates; fitted values and identified combinations are the
    scientific authority in nonunique cases.

## Failure atomicity and evidence

Syntax/domain failure, allocation failure, interruption, nonconvergence,
unrepresentable arithmetic, rank instability, or any failed certificate
leaves no public `e()` result or requested output. Exact equality is required
for status, sample identity and drop reasons, coefficient status/order, DoF,
VCE policy, and cluster/separation counts. Registered numerical tolerances
apply to coefficients, covariance, eta, mu, objectives, residuals, and saved
effects only after all discrete fields agree.

The initial return-code map is:

| Failure | Stata code |
|---|---:|
| General syntax or mutually exclusive options | 198 |
| Unsupported aweight or iweight syntax | 101 |
| Invalid frequency-weight domain | 401 |
| Invalid outcome, pweight, offset, exposure, or other data domain | 459 |
| Empty/zero-outcome final sample, nonpositive DoF, or insufficient clusters | 2001 |
| IRLS/step/separation nonconvergence or failed numerical publication certificate | 430 |
| Unsupported public feature | 498 |
| Protocol mismatch | 499 |
| Allocation or memory preflight | 909 |
| User interruption | 1 |
| Internal invariant failure or caught panic | 3499 |

A detector that cannot allocate uses `r(909)`; a detector that allocates but
cannot converge or certify uses `r(430)`. A malformed internal result is not
misreported as user data.

Qualification requires the exact pinned comparator closure plus independent
explicit-dummy Poisson calculations for no FE, one and multiway FE, absorbed
slopes, collinearity, fractional outcomes, zeros, offset/exposure, both public
weights, robust and multiway cluster covariance, saved effects, and separated
samples.
