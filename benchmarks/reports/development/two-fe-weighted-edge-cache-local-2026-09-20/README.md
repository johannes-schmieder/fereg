# Weighted Schur edge cache: local development checkpoint

Measured on the local M2 Ultra Mac with licensed Stata/MP 19.0, eight licensed
Stata processors and sixteen requested native workers unless specified. This
is development evidence, not cross-platform or release qualification.

The accepted change reuses existing idle left-RHS and group-statistics banks
for right-neighbor codes and f64 normalized edge-weight sums. A bounded repeat
sample and capacity checks gate construction. Original left RHS means are
restored before reconstruction; the independent observation-space certificate
and charged continuation remain unchanged. No new heap owner is introduced.
PPML/shared reusable calls explicitly opt out. See [ADR 0080](../../../../docs/adr/0080-weighted-schur-edge-scratch.md).

## Measured effect

Three-pair command medians in milliseconds:

| AW design | K | Request | Parent | Candidate | Default fixest |
|---|---:|---|---:|---:|---:|
| Ordered dense | 6 | IID | 326 | 297 | 487 |
| Ordered dense | 6 | All outputs | 488 | 458 | 713 |
| Ordered dense | 50 | IID | 858 | 768 | 1944 |
| Ordered dense | 50 | All outputs | 1039 | 945 | 2145 |
| Shuffled dense | 6 | IID | 372 | 340 | 299 |
| Shuffled dense | 6 | All outputs | 524 | 505 | 551 |
| Degree six | 6 | IID | 86 | 82 | 70 |

The degree-six repeat guard rejects cache construction. Shuffled IID and
narrow degree six still trail fixest. Default-reference output discrepancies
remain recorded; independent native audits and prior tight-reference checks
provide separate accuracy evidence. No reference precision change supplies
these timing targets.

A separate shuffled K=6 profile localizes the operator improvement from
103.19 to 73.70 ms, with initialization increasing 7.58 to 8.90 ms and
reconstruction increasing 3.93 to 6.87 ms because left means are restored.
Initial absorption falls from 187.71 to 166.66 ms. Full-command timing is
noisy and includes other phases; the profile is not a replacement benchmark.

## Controls and scientific checks

The archive contains 103 initial application calls and eighteen confirmation
calls. All succeed. Fifty parent/candidate pairs pass the numerical comparison
and preserve exact status/DoF. Twenty-one pairs retain exact b/V files; changed
edge summation order explains why bitwise parity is not required elsewhere.
No pair raises the native allocation peak. Twenty-two independent output audits
pass weighted group normal equations, finite scores, component normalization,
reconstruction and RSS. AW/FW/PW robust/clustered outputs, constant/dispersed/
group-concentrated weights, a tiny-weight bridge, 4/8-worker, low-memory, unit
and one-FE controls are retained.

Three initial single-pair slowdowns remain visible. Their three-pair
confirmations are:

| Control | Initial ratio | Confirmation parent/candidate ms | Median paired ratio |
|---|---:|---|---:|
| Dispersed weights | 1.1208 | 662 / 576 | 0.87405 |
| Group-concentrated weights | 1.0532 | 1421 / 1430 | 1.00633 |
| Low memory | 1.1504 | 356 / 349 | 0.97753 |

Thirteen focused Schur tests pass, including shared PPML checkpoint fallback.
The new operator/projection test covers fractional weights at common scales
1e-100/1/1e100, connected/disconnected graphs, RHS widths 3/5/8/52, capacity
rejection, and 8/16 workers. Separate MAP and forced-CG-continuation comparisons
pass. Initially the forced continuation used a looser requested tolerance than
ordinary Schur's internal floor; cached and uncached distances were identical
at 2.69635e-9, beyond the test's 2e-9 oracle bound. The test now requests the
same strict 1e-14 tolerance in both routes. The bound and all production
tolerances are unchanged; original failures, localization and final tests remain.

Three final-artifact licensed fixtures pass with process status, explicit
fixture sentinel and terminal sentinel: a new weighted-cache design checked
against areg plus explicit second-FE dummies; singleton/weight/sample behavior;
and publication transaction atomicity. The new fixture's 3,300 left levels,
repeated pairs and N=16,500 provide enough existing scratch to admit this cache.
It checks K=3/6, AW/FW IID, AW/FW/PW robust and one-way clustered inference,
residuals and saved-FE reconstruction. Its version 18.0 directive requests Stata
compatibility behavior; the executable is Stata 19.0.

## Identity and limitations

Candidate `weightededges2` uses base `455150af` plus `implementation.patch`.
Plugin SHA256 is
`337deed4ecdae50f490f84cd8731f1be68beb69b3151bdde7a0ce2a5be4edb9f`.
All 514 captured source entries match; actual-plugin ABI/panic checks pass.
The build embeds the helper's September 19 date; measurements occurred
September 20. The new Stata fixture is separately hashed and was added after
that artifact's source capture. Parent is `narrowedges2`, plugin
`8ec5df3a1686c9f476dcf2a83b63318ee252f789b1e49b75c624d5300d76efb2`.
The first `weightededges` binary was not timed and differs only in test source;
its original failed test evidence is preserved. An initial validation-script
argument rejection ran no applications and is retained as such.

This closes the cache checkpoint, not the broad milestone. Narrow input/RHS
passes, well-connected graph routing, the 111 controls, fresh holdouts and two
independent five-pair scoreboards remain. No installed package or remote state
changed.
