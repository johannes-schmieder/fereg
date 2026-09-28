# Large three-FE OLS: IID degrees-of-freedom mismatch

The four completed T4 OLS repetitions at N3,932,160/K10 have a material
**inference-policy mismatch** with default stock fixest. Fereg's absorbed and
residual degrees of freedom exactly match the original pinned reghdfe outputs
on every input. Default fixest counts more FE parameters, producing larger
IID standard errors. This explains essentially all of the SE difference; it
does not adjudicate the remaining coefficient/RSS differences or grant timing
eligibility.

Current run is `20260928-current-sweep-161b9491`, measurement fingerprint
`fb2ad540…`, estimator package `c09eeab3…`, Linux plugin `59d0341a…`.
Original reghdfe evidence is from `20260926-systematic-light-58284f76`.
[evidence.json](evidence.json) binds all 32 consumed result, complete-group and
coefficient/covariance files. Current native/default stock and original reghdfe
have identical input and retained-sample hashes for each repetition. The old
reference clocks are not new paired timings.

## Source and arithmetic

The [source audit](source-audit.json) verifies the exact fixest 0.14.2 archive
SHA256 `17372a78…`. The frozen worker calls public `feols(..., vcov="iid")` with
default small-sample settings. In the pinned source, `K.exact` defaults to false;
unclustered inference uses the stored parameter count. Its IID covariance is
the inverse within Gram multiplied by RSS/(N−K).

Public fereg uses the accepted pairwise absorbed-DoF policy. Each later FE's
redundancy uses the maximum component count over earlier pairs. This is the
documented reghdfe-compatible policy, not a claim of exact rank for arbitrary
three-plus-FE designs. It is unchanged here.

The read-only [analysis](analyze.py) removes each estimator's own reported
RSS/residual-DoF scale and compares the resulting inverse-Gram shapes. It also
compares SEs after applying that scalar ratio, purely as a diagnostic. It does
not modify an export, reference command, benchmark tolerance or eligibility.

| Repetition | Native/reghdfe residual DoF | Default fixest residual DoF | Original largest SE difference | Difference after matching RSS/DoF scale |
|---|---:|---:|---:|---:|
| 1 | 3,106,158 | 3,104,431 | 7.898e-8 | 5.381e-12 |
| 2 | 3,106,231 | 3,104,431 | 8.231e-8 | 1.010e-12 |
| 3 | 3,106,291 | 3,104,431 | 8.497e-8 | 2.139e-12 |
| 4 | 3,106,166 | 3,104,431 | 7.931e-8 | 4.240e-12 |

Default stock's inferred parameter count exceeds the native/reghdfe count by
1,727–1,860. The predicted native/stock SE ratios are .9997005–.9997220. The
normalized inverse-Gram discrepancy is 7.12e-9–3.79e-8, measured entrywise
relative to the native diagonal geometric mean. The RSS contribution to the
scale is tiny compared with the DoF contribution. Native RSS differs from
original reghdfe by 1.24e-6–6.44e-6, versus 0.00330–0.03003 from default fixest.
Those comparisons alone do not establish the independent optimum.

Analytic two-coordinate checks verify the scale identity, sensitivity to a
changed covariance shape and rejection of nonpositive reference residual DoF.
This analysis reads scalar and 10-by-10 exports only. No fit, compilation,
profiling, new reference run or qualification accompanies it.

## Consequences and unresolved work

Keep all four native/default-stock pairs **NR** under the frozen controller.
Repetition 2 fails only SE comparison. Repetition 1 also fails RSS; repetitions
3 and 4 also fail coefficients and RSS. These remaining failures are separate
from the explained scalar inference-policy difference. This is not blanket
evidence that either estimator is inaccurate.

A future comparable reference profile must explicitly establish matched DoF
work and policy before timing. `ssc(K.exact=TRUE)` adds stock FE recovery, and
its count must be checked against the accepted pairwise policy; it is not an
automatic substitute for that policy in every higher-dimensional design.
Post-hoc covariance rescaling cannot make old default timings perform that
additional work. Original NR and stock settings remain unchanged.

The [isolated frozen-controller partial report](partial-state.json) at 15:31:13 UTC also has **81
successful applications, two command timeouts, 17 scientific comparison
failures, 29 passing pairs and zero complete eligible four-repeat configurations**.
Fifty registered historical budget exclusions remain separate. In particular,
native degree-two OLS N3,932,160/K10/T4/seed8675309 again times out, now at a
720-second command bound; original source timed out at 120 seconds on the same
input. The other timeout is strict stock three-FE PPML at N3,932,160/T28/seed20260819,
at 2,400 seconds. Neither proves a solver stall. Exact timeout receipts remain
in the live run, and the four array tasks continue. No run was restarted.

After the timing sweep and held diagnostics, the large native OLS timeout needs
one bounded phase/solver diagnosis. The final campaign report should distinguish
this genuine runtime failure, policy-driven SE incomparability, and unresolved
numerical differences. The separate fresh weak-PPML SE case remains open.
