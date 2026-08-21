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
sample. The developer-only preparation boundary now accepts one
analytic-weight variable. It retains positive finite values bit-for-bit without
normalization, drops zero or missing values with reason 4, errors on a negative
candidate-row weight, and never reads a weight outside the `if`/`in` candidate
marker. This establishes sample preparation only; objective normalization,
effective `N`, VCE, DoF, singleton behavior, and posting remain later gates.

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
Only the analytic-weight preparation rules implemented in the guarded
developer action have crossed an executable boundary; no weight kind is
enabled in `stfeols`.
