# OLS weight-semantics contract

Weight behavior is a reference-captured scientific contract, not an inference
from Stata's `syntax` declaration or from the native storage type. StFixest
still enables no public OLS estimator, and this document does not claim that a
licensed reference capture has occurred.

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
sample. Native numeric preparation currently preserves the requested kind and
finite values; that storage fact establishes none of the estimator semantics
above.

## Evidence and enablement gate

The capture runner uses only clean, exact-commit `reghdfe` and `ftools`
checkouts staged from `git archive`. It records hex-formatted numerical fields,
case return codes, final-sample markers, and explicitly requested outputs. The
machine stream and licensed Stata log are temporary and cannot be committed.
Only the sanitized, sealed receipt is eligible for review.

Before any weight kind is enabled, its sealed reference result must be paired
with an independent explicit-dummy/WLS oracle and an end-to-end final-sample
identity test. At present the exact `ftools` commit is unresolved in
`references.lock.toml`, so the preflight stops before Stata starts and no OLS
weight behavior is frozen yet.
