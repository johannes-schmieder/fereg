# Current-weight coarse canonical CG: rejected public-performance experiment

Local Mac arm64 development evidence, 2026-09-25. Not qualification.
[ADR 0196](../../../../docs/adr/0196-canonical-coarse-cg-closeout.md) rejects
canonicalcoarse1. Production remains e7731129/finalreplay1. Fewer canonical
iterations produce only 1-ms median improvements on two targets, with mixed
paired results and a narrow-control loss. No source correction, fresh holdout,
OLS timing campaign or threshold tuning follows this decision.

## Source and candidate

| Artifact | Base and overlay | Plugin SHA-256 |
|---|---|---|
|finalreplay1 baseline|f97c9dbe + family.rs/result.rs; accepted at e7731129|84db18f8e91f31eb64f26c854df98609b3722a3d49828edbad040118b3f188b3|
|canonicalcoarse1 candidate|a4792e9d + canonical_products.rs|d1abdf4adbd7ce04f2b2ddb6316ea41c436775cd2c08d149fadaf68788c04ec9|
|canonicalstages1 diagnostic|e7731129 + isolated timing instrumentation|6ee7bf77ab77e5df302a13521e72d509ad12220af0b0b46f7187b902a40770d7|

The candidate's 558 captured inputs matched the experiment at closeout before
its one production file was restored exactly. Baseline has 558 inputs; the
instrumented build has 559. Captured sources, identity manifests, patch, recipes
and actual-plugin ABI/panic-containment receipts are archived. The diagnostic
artifact never supplies a stock headline.

For each current-weight grounded scaled component matrix A, the prototype forms
v from its free-coordinate diagonal roots, normalized by their largest value,
and positive finite c=v'Av. PCG uses P*r=r+v*(v'r)/c. Invalid c disables the
optional correction. The unpreconditioned recurrence residual still determines
when the unchanged original-input residual certificate is attempted. This
certificate/direct Gram, subsequent canonical rank, scores, reconstruction,
objective and exact final q*mu Hessian remain independent acceptance authorities.

One existing Q-double assembly bank holds packed component directions and their
energies. Its prolonged lifetime is conservatively charged beside Krylov banks;
Ap scratch becomes P*r after its last use. No fourth RHS vector, cross-call
weight cache, new observation bank or larger allowance is introduced. Direct
Cholesky is unchanged. Production does not retain this prototype or its tests.

## Completed canonical attribution

This closes the previously pending canonical ledger on development seed347:
two rotations each on three designs, accepted baseline versus isolated stages.
All 12 public states and six native comparisons pass. Inclusive full-call median
spans below are diagnostic; nested scopes cannot be added as independent savings.

| Scope, ms | Low N61440/K6/T4 | Fast N61440/K6/T4 | Narrow fast N24960/K2/T4 |
|---|---:|---:|---:|
|Canonical build|18.106|16.337|3.005|
|RHS solves|7.426|7.308|0.177|
|Matrix assembly|2.073|2.041|0.637|
|Packing/factorization|1.276|1.303|0.052|
|Original-input certificate rows|2.113|1.840|0.356|
|Write controls/dense rows|2.257|0.919|0.169|
|Whole plugin|109.686|88.523|33.278|

Large calls retain group centers with four components of 256 levels, seven RHSs,
and 5,031,520 planned bytes. Work admission selects iterative reduced solving.
These profiles identify a real but bounded opportunity: eliminating all 7.4 ms
of RHS work would still leave most of the latest fresh low-mode 27.4-ms gap.
They do not justify a scratch-layout redesign based on an assumed dominant write.

## Complete public pilot and all distributions

Three alternating baseline/candidate/stock rotations, development seed347,
robust inference and fitted means. Unmodified pinned fixest 0.14.2 uses
`glm.tol=1e-10`, `fixef.tol=1e-11`; archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
OpenMP/Rayon match each declared thread count, with BLAS at one. Generation,
import, startup and warmup precede the clock; public loading, estimation,
certification, inference and output are inside it. No build, profile, archive or
competing fit overlaps headline timing. Every sampled process monitor passes;
this is not a guarantee against all ambient operating-system activity.

| Case | Baseline ms | Candidate ms | Stock ms | B/C/S medians | C/stock | B/C wins/losses/ties |
|---|---|---|---|---|---:|---|
|Disconnected low N61440/K6/T4|112,113,113|112,111,114|94,102,101|113/112/101|1.109|1/1/1|
|Same design fast|97,99,99|99,97,98|100,102,101|99/98/101|0.970|2/1/0|
|Narrow fast N24960/K2/T4|39,39,41|40,40,40|33,30,34|39/40/33|1.212|1/2/0|
|Two slope partitions fast N30720/K6/T8|77,76,84|76,75,76|77,79,80|77/76/79|0.962|3/0/0|

Seven candidate/baseline wins, four losses and one tie in total. The two iterative
targets together have three wins, two losses and one tie. No median meets the
0.90 target in this pilot; this does not replace the prior fresh block, which has
other passing families. The excluded slope improvement has no changed iteration
work and is not attributed to the new preconditioner. Narrow reports five fewer
inner iterations despite its higher public median, so it is a low-cost comparison
control rather than proof that the entire fit uses only unchanged Cholesky.

One separate closing native trace on both large modes records initial maximum
iterations 15 to 11 and strict maxima 23 to 19. The aggregate inner count falls
by eight; outer and fallback counts are unchanged. Four traced public states and
two native comparisons pass. No instrumented timing is used to rank stock.
These iteration counts are maxima across component/RHS solves, not total matvec
counts or units interchangeable with MAP sweeps.

## Science, memory and limits

Thirteen distinct Rust tests pass: the existing twelve canonical-product tests
and a new analytic grounded weighted-graph recovery test. Coverage includes
nonzero-residual WLS, changed weights, rank/session rollback, T1/T4, extreme RHS
scales, zero RHS, invalid coarse energy, iteration refusal, exact workspace
success and one-byte-below refusal. No existing truth assertion is relaxed.
Six licensed fixtures pass with explicit END_FIXTURE sentinels and zero Stata
return codes: iterative canonical Poisson/Hessian, multipartition slopes,
three-partition slopes, scientific contract, weights/inference and separation
outputs. They use independent explicit-design/Hessian/sandwich checks.

Across pilot, native trace and prior ledger, all 52 public states, 32 baseline-
anchored comparisons and 12 direct stock comparisons pass. There are no
scientific discrepancies, application failures or timeouts in these blocks.
Tracked peak and scratch deltas are zero; selected telemetry changes by zero or
-48 bytes. The conservative extra Q-double charge covers a longer lifetime,
without establishing a physical-memory reduction. No new warmed allocation or
physical-cap campaign is claimed. Rejected source does not enter public OLS;
no new OLS performance claim follows this milestone.

The broad multi-FE goal stays active. Proceed to shared preparation,
reconstruction and checking costs; keep this correction, prior matrix-cache
experiments and early-checkpoint tuning closed. Known checkpoint-count test,
allocation inventory drift, constrained-fit/nonretaining-CG failures, three-FE
scientific discrepancy and deferred platforms are not repaired here.

## Immutable receipts

The content-addressed archive contains 3312 logical receipts / 991 payloads,
9230616 bytes, SHA-256 `8cd94593a80e2e326d41131e676cbfddeae2710d4087a323932372c3772f7d7a`. Restore with
`python3 restore_receipts.py <new-directory>`. All logical hashes were restored
and checked. Data and plugins are excluded with recorded hashes; raw licensed
logs are excluded, with protocol sentinels and structured fixture summaries kept.
Source/build recipes, full distributions, science audits and the rejected patch
are included. No fresh validation seed is consumed; seed431 remains unused.
