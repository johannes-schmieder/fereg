# Sparse FE factor: interim evidence for the next PPML plan

This immutable planning snapshot records completed local development work at
**3e1d9120**. It is not adoption, qualification or the experiment's final archive.
The accepted **higherdim1** artifact has all **565 computational inputs** matching
that checkpoint and plugin SHA-256
`92df56c44f5c40aabc4c297e3d2bbc863d528f0fe3f0ce395c7a7fb6723e50a2`.
The active plan belongs only in STATUS.md.

[evidence.json](evidence.json) preserves timing vectors, audit summaries and
original-receipt hashes. The three identity files preserve complete captured
source maps and plugin identities. Original inputs, implementations, captures,
logs and failed attempts remain in `target/ppml-sparse-fe-factor-20260925` and
the corresponding isolated artifact directories. They must remain available for
the eventual experiment closeout; this compact snapshot does not replace them.

## Current-source gaps

Three rotations use existing development seed **202609347**, robust inference,
fitted means, matched threads and unmodified pinned fixest with
`glm.tol=1e-10` / `fixef.tol=1e-11`. The stock source archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
No fresh validation seed is consumed. All 18 baseline applications and public
states, and nine baseline/stock comparisons, pass. Sampled monitors report no
overlaps or monitor errors.

| Case; N/K/T; mode | Accepted fereg vector, ms | Stock vector, ms | Median ratio | Savings to 0.90 stock |
|---|---|---|---:|---:|
|Disconnected two categorical FE; 61440/6/4; low|109,105,106|101,100,102|1.050|15.1 ms|
|Narrow two categorical FE; 24960/2/4; fast|36,37,37|34,33,35|1.088|6.4 ms|
|Two slope partitions; 30720/6/8; low|72,70,71|81,81,82|0.877|Already below; limited headroom|

These are separate from ADR0203's seed435 holdout and earlier source revisions.
They must not be pooled or substituted for those distributions.

## Sparse kernel and first public integration

The safe-Rust diagnostic factors the grounded full bipartite FE Laplacian,
retaining symbolic ordering/fill while rebuilding numeric factors for changed
weights. Seven actual accepted-command weight/RHS frames (41 columns) pass an
independent explicit-design weighted LSMR oracle. Maximum weighted relative
error is **7.853e-14**; T1/T4 output binaries agree exactly. The large T4 kernel
takes about **3.4 ms per numeric-factor/projection call**, with **7.3 ms symbolic
setup** and **12.52 MB counted owned banks**. Those clocks exclude finite
regression, public finalization and several production consumers. Counted payload
is not a complete memory admission model.

The public prototype **sparsepublic1** has plugin SHA-256
`768f57bb5a8feeb7ee5b543158165cc6d01892fb0b980c700cbb770812f2b186`.
It installs the factor in the categorical projector, disables the coefficient
owner for two categorical effects and disables their fused canonical builder.
Consequently ordinary joint directions become full WLS solves. This organization
loses despite fast factor kernels. All three licensed scientific fixtures pass;
27 public states, eighteen baseline-anchored pairs and nine direct stock pairs
pass. The prototype has incomplete memory admission, capacity accounting and
refusal/fallback handling, and is **not production eligible**.

| Case, same order as above | Baseline vector, ms | Candidate vector, ms | Stock vector, ms | B/C/stock medians |
|---|---|---|---|---|
|Disconnected low|106,107,105|156,155,154|98,99,97|106/155/98|
|Narrow fast|37,37,36|43,43,43|34,34,37|37/43/34|
|Slope exclusion|71,72,74|72,71,76|81,82,78|72/72/81|

All 27 applications complete, with no recorded monitor errors or overlaps.
The large-case deficit to 0.90 stock in this separate block is **17.8 ms** for
the accepted baseline. Reference drift between blocks remains visible.

A separate instrumented artifact, **sparseprofile1**, completes four applications
with four independently passing states and two passing prototype/profile pairs.
Its timings are diagnostic only. Large-case sparse operations use T4 and numeric
factorization takes about 1.5 ms per changed frame. In the uninstrumented pilot's
first large pair, outer iterations remain four, while projection telemetry rises
**24.25 to 57.67 ms** and finite-solve time **7.50 to 13.93 ms**. Thus replacing
joint/fused work with repeated full WLS is a measured regression. The expectation
that retaining those consumers could rescue the factor is still a hypothesis.

No corrected integration, fresh-case validation, production memory admission,
new OLS timing block or final experiment decision exists in this snapshot. No
runtime files are changed at this planning checkpoint. Existing science
discrepancies, OLS losses, static-inventory drift and constrained-fit failures
remain open.
