# Current-frame PPML diagnosis — 2026-09-24

This bounded attribution screen informs the next multi-FE implementation plan.
It makes **no new performance-improvement or qualification claim**. Production
computational source remains `711802bfdd37947cf42be732e68b172c21d67ba1`.

## Source and measurement boundary

The uninstrumented `familyrows2` artifact has plugin SHA-256
`4a326b8d3b210084620478dfd857ffdca672db5aacfc7acbd82fbc3194b98af8`.
All 554 captured source inputs match the production checkpoint. Diagnostic
`currentframeprofile1` captures that checkpoint plus isolated span instrumentation:
555 inputs, plugin SHA-256
`18daa0e5b32aae849f60159ccfb773add70c007f861e44d8c3b33b9a4280ad01`.
Its actual instrumented sources, builder, identity and parsing recipe are archived.
The diagnostic build passed the actual-plugin ABI/panic harness; it is never
used for headline comparisons with stock.

The stock reference is unmodified fixest 0.14.2, source archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The four cases use existing development seed 202609347, numeric unweighted
controls, robust inference, fitted means, matched threads, glm.tol=1e-10 and
fixef.tol=1e-11. This is not fresh holdout evidence. Data generation/loading into
the host precedes each command clock; native command loading, estimation and
publication remain inside it. No compilation, profiling, audit, archive or
competing fit accompanied headline runs. All 32 application/isolation receipts
pass; the eight instrumented calls ran separately before the 24 headline calls.

## Current uninstrumented timings

Three rotations; milliseconds are median [minimum, maximum]. Slopes mean one
intercept-plus-slope channel on each of two partitions of the three-FE design.

| Case | Fereg ms | Stock ms | Ratio | Saving required for 0.90 stock |
|---|---:|---:|---:|---:|
| Two FE N21120/K2/T4, fast |40 [39,40]|28 [28,30]|1.429|14.8 ms / 37.0%|
| Disconnected two FE N42240/K10/T8, fast |91 [89,99]|78 [77,79]|1.167|20.8 ms / 22.9%|
| Unbalanced three FE slopes N61440/K10/T8, fast |226 [226,228]|232 [227,242]|0.974|17.2 ms / 7.6%|
| Same slopes, low |243 [241,246]|231 [226,231]|1.052|35.1 ms / 14.4%|

All ratios miss the target in this screen. The prior source-bound fresh block
in the [scoped-family report](../ppml-scoped-family-local-2026-09-24/README.md)
has different cases and distributions; neither block replaces the other.

## Diagnostic attribution

Values below are median union wall times across two instrumented calls. Only
spans inside the second, timed plugin invocation are selected. Overlapping
worker intervals are merged per function. These are **inclusive, nested**
function times and must not be added together or subtracted from stock timings.
Instrumentation changes execution cost; the numbers prioritize investigations.

| Function or region | Fast slopes ms | Low slopes ms |
|---|---:|---:|
| Whole plugin |219.49|234.59|
| Initial plus strict retained canonical solve, two calls |77.56|92.37|
| Initial retained design construction |27.27|31.98|
| Low retained-control reprojection |not separately entered|28.36|
| Full retained rank solve, two calls |17.04|18.31|
| Current-weight slope geometry preparation, six calls |15.11|15.06|
| Full MAP certificates, ten calls |9.62|10.75|
| Ordinary joint checking, four calls |10.30|10.20|
| Original-column norm/mean refresh, one call |4.03|4.02|

The current original-column refresh cannot by itself close a 35.1 ms low-mode
gap: eliminating it entirely would recover only about 4 ms in this diagnostic.
Initial/strict canonical work is a substantially larger intervention boundary.
Its subregions include projection, materialization, finite/rank work and checks;
this screen does not claim all of that work is removable. Geometry calls also
span changing frames, so their count is not evidence of invalid cache reuse or
unnecessary rebuilding within one unchanged frame.

Source inspection confirms that coefficient-space slope Schur solves, independent
column stopping, joint finite/FE directions, bounded low-memory access,
generation-scoped geometry, canonical-first finalization and two-categorical-FE
fused canonical products already exist. Proposed work must build on these.
Canonical mean reuse also already exists. Generalizing only the categorical
column-parallel statistics guard would be a small scheduling experiment, not
the principal redesign justified by this screen.

## Science, preservation and limits

All **32 public states** pass independent original-design score, model-space and
objective replay. All **12 fereg/stock pairs** pass the established sample,
coefficient, covariance, objective and mean comparisons. No new small explicit
Poisson/Hessian oracle, Rust numerical test or licensed regression fixture was
run for this instrumentation-only milestone. The preceding accepted checkpoint's
validation remains separately source-bound. No discrepancy or failed timed fit
occurred in this screen.

The verified archive preserves **1,991 logical receipts / 835 payloads /
6,812,930 bytes**, SHA-256
`931678fb12c1783d1034f365c8328a38e77d6b5aa56217daab200c25cd180653`.
It includes declarations, all source identities and captured inputs, diagnostic
spans, timing/isolation receipts, numerical exports, audits and recipes.
Data/binary hashes are retained without raw data, binaries or licensed logs.
Use `restore_receipts.py` to verify and restore the sanitized logical receipts;
`verification.json` binds the checked sources, counts and restore result.

The next implementation sequence is maintained only in `STATUS.md`. This screen
does not repair static allocation-inventory drift, constrained-fit/nonretaining-CG
failures, older comparator discrepancies or deferred platform/memory confirmation.
