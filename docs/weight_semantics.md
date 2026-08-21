# Weight-semantics contract

The native numeric preparation layer now preserves the actual requested kind
(`fweight`, `aweight`, `pweight`, or `iweight`) and compacts raw finite values
bit-for-bit. It has no unweighted pseudo-kind and allocates no pseudo-column for
an unweighted model. It does not itself decide a weight domain, normalize,
rescale, or apply weights and therefore makes no estimator claim for any weight
type.

## Control-plane and data-plane split

Stata may parse the weight expression and identify its source variable. Rust
numeric preparation is authoritative for explicit missing masks and nonfinite
values. A required caller-supplied policy decides whether each otherwise
available value is retained or dropped. Later fully specified native stages
must remain authoritative for:

- missing, nonfinite, zero, and negative values;
- the effective sample induced by weight rules;
- integer requirements for frequency weights;
- objective-function scaling;
- finite-sample and degrees-of-freedom adjustments;
- score and covariance scaling;
- alignment of weights after singleton or separation deletion.

The ado layer must not silently replace invalid values, pre-drop observations,
normalize weights, or send a different sample to the native estimator. A row
outside the candidate `if`/`in` sample does not invoke a weight policy; the
reference behavior for invalid weights outside that sample remains a parity
fixture rather than an assumption in preparation code.

## Required reference capture before enablement

For each weight type accepted by the pinned reference commands, licensed-Stata
fixtures must record:

1. accepted and rejected values, including missing, zero, negative, fractional,
   and very large values;
2. `e(N)`, coefficient, objective, residual, VCE, and prediction behavior;
3. whether multiplying all weights by a constant changes any posted result;
4. interaction with `if`/`in`, factor-level discovery, singleton deletion,
   clusters, and PPML separation;
5. exact sample identity and return codes on invalid input.

No weight type is enabled merely because Stata's `syntax` command accepts it.
Each type requires a written mathematical interpretation, an independent small
oracle, differential tests against its pinned reference, and a final-sample
identity test.

Still deferred to that capture and estimator-specific policy are exact accepted
domains (including zero, negative, fractional, and very large values), the
frequency-weight integer and upper-bound rule, constant-rescaling invariance,
objective and score scaling, VCE/DoF treatment, and alignment after singleton
or separation deletion.
