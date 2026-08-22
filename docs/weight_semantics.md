# OLS weight-semantics contract

Weight behavior is a reference-captured scientific contract, not an inference
from Stata's `syntax` declaration or from the native storage type. StFixest
still enables no public OLS estimator. The licensed, sealed ARM64 macOS/Stata
18 OLS reference receipt identified in `STATUS.md` now freezes the probe
outcomes below.

## Frozen reference probes

The frozen `ols_v1` manifest binds the following probes to deterministic CSV
bytes and exact `reghdfe` commands:

| Question | Manifest cases | Required evidence |
|---|---|---|
| Unweighted baseline | `unweighted_no_fe` | sample, `e(b)`, `e(V)`, DoF, fit statistics |
| Analytic weights | `aweight`, `aweight_rescaled` | raw weight declaration, sample, sum of weights, coefficient/VCE and objective behavior under constant rescaling |
| Frequency weights | `fweight`, `fractional_fweight_error` | effective `e(N)`, sum of weights, integer-domain rejection, sample, coefficient/VCE |
| Probability weights | `pweight` | acceptance, robust inference, sample and scaling metadata |
| Importance weights | `iweight_probe` | observed success/error status plus full results if accepted; no support is presumed |
| Domain boundaries | `zero_weight_probe`, `negative_weight_error`, `missing_weight`, `large_weight` | exact return status, final sample, posted weight metadata, and results when defined |
| Sample interactions | `weight_if_in`, `weighted_singletons` | `if`/`in` precedence, singleton fixed point, retained observation identities |
| Candidate precedence | `invalid_weight_outside_candidate` | a negative analytic weight outside `if` is ignored; capture aborts unless the asserted 11-row sample is retained |

`record` cases intentionally discover behavior whose success status has not yet
been adjudicated. Their outcome becomes evidence only in a sealed receipt; it
does not silently enable the same behavior in StFixest. `must_succeed` and
`must_error` cases are harness assertions and abort the capture if violated.

## Native implementation obligations

For every eventually supported weight kind, the authoritative Rust path must
separately specify and test:

- the accepted finite domain, including zero, sign, integrality, and bounds;
- whether invalid values outside `if`/`in` are inspected;
- objective, residualization, score, and covariance scaling;
- the meaning of `e(N)`, weight sums, residual DoF, and cluster corrections;
- factor-level discovery and singleton deletion under weights;
- constant-rescaling invariants, if any;
- alignment after every native sample deletion.

The ado layer may parse the weight expression and pass its source role, but it
must not normalize weights, replace invalid values, or pre-drop a different
sample. The developer-only preparation boundary accepts at most one analytic-,
frequency-, or probability-weight variable. Analytic weights retain positive finite values
bit-for-bit without normalization, drop zero or missing values with reason 4,
and error on a negative candidate-row value. Frequency weights retain finite,
strictly positive integral values representable as `u32` bit-for-bit. Values
with the explicit Stata-missing mask receive reason 4; unmasked NaNs or
infinities and zero, negative, fractional, or out-of-range candidate-row
values fail closed. No weight outside the `if`/`in` candidate marker is read.
The frequency contract is anchored by the sealed successful
`fweight` and fractional-error cases plus the independent row-expansion oracle.
Probability weights retain positive finite values bit-for-bit; explicitly
masked missing values receive reason 4, while zero, negative, or unmasked
nonfinite candidate-row values fail closed. This is deliberately conservative:
the sealed `pweight` case establishes successful positive-weight transport and
records that the reference uses robust VCE, but this preparation action neither
computes VCE nor extrapolates public probability-weight behavior. The sealed
`iweight_probe` returns `101` without results, so importance weights have no
developer-action binding and remain explicitly unsupported.
The guarded action maps invalid-data failures to `459`; matching the public
reference command's `401` error remains a later Stata-facing gate. This
establishes sample preparation only. It deliberately does not convert the
retained physical-row count to effective `N = sum(fweight)`, expand rows,
normalize objective weights, or compute VCE, DoF, or posted results. It does
implement the bounded preparation-time singleton identity rule: unweighted,
analytic-, and probability-weighted models count physical rows, while
frequency-weighted models use exact checked integer multiplicity per FE level.
Singleton counts report physical rows removed, not summed frequency weight.
The sealed `weighted_singletons` analytic-weight case fixes the retained and
deleted identities; the pinned `reghdfe` source supplies the frequency-weight
multiplicity rule. Prepared iid, HC1 robust, and clustered inference are now
defined below; all posted-weight semantics remain later gates.

The separate core-only prepared coefficient checkpoint now supplies a bounded
objective-semantic test without changing those Stata-facing limits. For
analytic, frequency, and probability inputs it minimizes the positive weighted
sum of squared residuals; a common internal maximum-weight normalization is
used only to make the computation scale-safe and leaves coefficients and
scores invariant. A positive weight that underflows to zero under that
normalization fails closed. Frequency weights match literal row expansion for
identified coefficients and residuals on physical rows. Independent tests
cover all three supplied kinds, unit weights, common scaling from `1e-200`
through `1e200`, and no/one/two/repeated intercept-only fixed effects. This
checkpoint also returns inverse cross-product bread on the pinned reference
scale: raw integer cross-product weights and exact summed `N_ref` for frequency
weights; analytic/probability weights rescaled to sum to physical N; and unit
weights otherwise. Common analytic/probability scaling therefore leaves bread
unchanged, whereas multiplying frequency weights by an integer divides bread
by that integer, matching literal row expansion.

The separate same-call `CertifiedPreparedOlsUnadjusted` checkpoint is the
consumer allowed to turn that bread into conventional homoskedastic covariance.
Its reference objective and count policies are:

| Kind | Reference weight | Effective N | Raw sum metadata | IID policy |
|---|---|---|---|---|
| none | 1 | physical retained rows | absent | supported |
| analytic | `raw * physical_N / sum(raw)` | physical retained rows | supplied positive-weight sum | supported |
| frequency | raw positive integer multiplicity | exact checked frequency sum | exact frequency sum | supported |
| probability | same prepared objective normalization as analytic | physical retained rows | supplied positive-weight sum | rejected: pinned reference requires robust inference |

For supported kinds, residual DoF is effective N minus finite kept rank and
absorbed rank; the normalized absorbed `_cons` is not an additional rank.
With no explicit absorb terms, the pinned reference is represented by a
virtual one-level structural constant for both default and `noconstant` fits.
It contributes `df_a=1`, centers Y and X using the reference weights, and is
reported as a normalized `_cons` coordinate only when requested. Thus paired
default/`noconstant` fits have identical finite coefficients, residuals,
statistics, DoF, and finite covariance blocks.
`RSS`, `TSS`, within TSS, RMSE, R-squared statistics, conditional F, likelihoods,
and iid covariance all use that reference scale. Conditional F is the Wald
quadratic over kept nonconstant coordinates, not an SSR subtraction. The analytic and rescaled-
analytic sealed cases therefore have the same RSS and covariance within
tolerance while retaining distinct raw `sumweights`; frequency inference is
identical to literal row expansion. Independent tests also cover common
analytic scales from `1e-150` through `1e150` and exact effective frequency N.

The separate same-call `CertifiedPreparedOlsRobust` checkpoint consumes the
same reference-weight and effective-N report. Its HC1 policy is:

| Kind | Effective N | Per-row score-meat multiplier `lambda_i` | Robust scaling rule |
|---|---|---|---|
| none | physical retained rows | `1` | unweighted HC1 |
| analytic | physical retained rows | `(raw_i * physical_N / sum(raw))^2` | invariant to a common positive scale |
| frequency | exact checked frequency sum | raw integer multiplicity | identical to literal row expansion |
| probability | physical retained rows | `(raw_i * physical_N / sum(raw))^2` | invariant to a common positive scale |

For every kind, `c_HC1=N_effective/df_r` and
`V=c_HC1 * bread * sum_i(lambda_i e_i^2 z_i z_i') * bread`. The robust
checkpoint therefore crosses the previously gated positive-pweight scientific
core boundary without changing any public or posted weight behavior.
Independent explicit-dummy WLS and literal-expansion oracles cover these
formulas; the sealed receipt anchors the full probability-weight covariance.

The separate same-call `CertifiedPreparedOlsClustered` checkpoint uses the
reference objective weight itself in the observation score:

```text
score_i = omega_i * e_i * z_i
```

Its count and scaling policies are:

| Kind | `omega_i` | `N_effective` | Cluster scaling result |
|---|---|---|---|
| none | `1` | physical retained rows | ordinary unweighted group scores |
| analytic | `raw_i * physical_N / sum(raw)` | physical retained rows | invariant to a common positive scale |
| frequency | raw positive integer multiplicity | exact checked frequency sum | identical to literal row expansion |
| probability | `raw_i * physical_N / sum(raw)` | physical retained rows | invariant to a common positive scale |

For every nonempty declared-cluster subset, the uncorrected meat is the sum of
outer products of aggregated group scores. Inclusion--exclusion combines those
meats before applying the one common correction

```text
((N_effective - 1) / correction_denominator_dof)
    * G_min / (G_min - 1).
```

The correction denominator, fit-statistic DoF, and reported
`min(N_effective-df_a-df_m, G_min-1)` are intentionally distinct when cluster
nesting applies; here `df_a` is the post-nesting absorbed rank. Independent
aweight/pweight scale-invariance and frequency-expansion oracles cover the full
covariance, statistics, and certificates.

All three inference checkpoints remain core-only. They do not post `e(N)` or
`e(sumweights)`, define display metadata or public probability-weight defaults,
or become callable through the plugin or `stfeols`.

## Evidence and enablement gate

The capture runner uses only clean, exact-commit `reghdfe`, `ftools`, and
`require` checkouts staged from `git archive`, with every required source file
SHA-256 checked. It records hex-formatted numerical fields,
case return codes, final-sample markers, and explicitly requested outputs. The
machine stream and licensed Stata log are temporary and cannot be committed.
Only the sanitized, sealed receipt is eligible for review.

Before any weight kind is enabled, its sealed reference result must be paired
with an independent explicit-dummy/WLS oracle and an end-to-end final-sample
identity test. The full dependency closure and licensed OLS receipt are frozen.
Only the analytic-, frequency-, and probability-weight preparation rules
implemented in the guarded developer action have crossed an executable
boundary; no weight kind is enabled in `stfeols`.
