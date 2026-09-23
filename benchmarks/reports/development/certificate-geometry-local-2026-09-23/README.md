# Scoped generic owned-FE certificate geometry

Local Mac development milestone, 2026-09-23. Baseline `mapinit1` matches all
529 computational inputs at `51ae723d`. Final candidate `certcache3` is that
revision plus the archived `certificate.rs`, `auto.rs`, `map.rs` and
`accelerated_map.rs` overlays; all 529 captured inputs match the resulting
computational source. Candidate plugin SHA-256:
`21bb7a0d0a093e49e22b03e5923f4b13ab7bef2f727b7ebf4c4d30a640552e82`.
Baseline plugin:
`97271f6c7980a31b3601bf32a419469c10f3c721e51a286257d07854af0a63d8`.

## Cost and change

The preceding milestone removed an unused initial MAP certificate. The
remaining automatic path still rebuilt identical weighted QR geometry at each
fresh residual check. [ADR 0116](../../../../docs/adr/0116-scoped-owned-certificate-geometry.md)
retains only those bases within one immutable design/current-weight absorption.
A private context prevents replacing the geometry's design or weight binding.
Residual norms, projected coefficients, raw moments and their reduction order
remain fresh and unchanged, as do the scheduled checks, iterations, contraction,
route, rank, DoF and tolerances. There is no cached score, working response,
IRLS generation or final Hessian.

The automatic MAP pilot and CG continuation share the context. Direct solvers,
categorical paths, the existing single-partition cache and changing-weight
reusable MAP retain their existing behavior. Shared generic callers build a
new context for each current-weight call. The cache is admitted alongside the
existing construction/solve scratch, released before LSMR, and released before
CG when retaining it would crowd out that continuation. Exact-budget tests
verify the original route remains affordable, with identical outputs.

Separate diagnostic artifacts show the same initial/final certificate and
projection counts: two-slope case 9/2 certificates and 36/3 projections;
unbalanced three-FE case 8/2 and 55/5. Initial-batch certificate time falls
160→111 ms and 160→124 ms; final-residual certificate time falls 17.3→7.3 ms
and 15.6→6.6 ms. Candidate geometry construction adds 9.0/9.3 ms total across
the two absorption calls. Thus total certification including construction falls
177→127 ms and 176→140 ms. Projection work stays approximately 109/106 ms
initially. These diagnostics explain the mechanism and do not enter headline
rankings. Fresh span/raw-moment work remains a substantial serial cost.

## Alternating public-command measurements

Three repetitions rotate baseline/candidate/stock order. Seeds 202609271–276
were declared before these blocks; reserved seeds were not used. All data
construction, compilation, profiling, oracles and archiving occur outside
headline fits. Requested threads match; BLAS uses one thread. Timings include
the public command, native loading, preparation, estimation and result
formation; external import and diagnostic export are excluded. No saved
outputs are requested in this screen; licensed saved-slope fixtures protect
that boundary separately. Low memory is a native storage-mode request, not an
equal-budget stock comparison. All times below are medians in milliseconds.

| Case (N, K, threads) | Baseline | Candidate | Stock | Paired ratio min / median / max |
| --- | ---: | ---: | ---: | --- |
| Two slope partitions (61440,10,4) | 314 | 271 | 59 | .859 / .863 / .873 |
| Unbalanced three FE, two slope partitions (61440,10,4) | 362 | 328 | 71 | .892 / .904 / .934 |
| Fresh wide two-partition slopes (92160,28,1) | 1015 | 967 | 843 | .927 / .953 / .977 |
| Fresh unbalanced three FE (92160,18,8) | 1997 | 1876 | 424 | .925 / .939 / .949 |
| Fresh low-memory slopes (30720,12,1) | 373 | 291 | 96 | .772 / .789 / .802 |
| Weak native slopes (19200,7,4) | 80 | 72 | unranked | .900 / .911 / .914 |
| AW robust slope control (30720,10,4) | 300 | 249 | — | .825 / .827 / .833 |
| PW clustered slope control (30720,10,4) | 300 | 247 | — | .809 / .823 / .833 |
| Categorical one-FE control (61440,10,4) | 19 | 19 | — | .950 / 1.000 / 1.053 |
| Categorical two-FE control (61440,10,4) | 80 | 79 | — | .987 / .988 / .988 |
| Public categorical PPML control (61440,6,4) | 223 | 225 | — | .986 / 1.009 / 1.009 |
| Explicit MAP slope control (30720,10,4) | 802 | 807 | — | 1.001 / 1.004 / 1.009 |
| Explicit LSMR slope control (30720,10,4) | 141 | 141 | — | .939 / 1.000 / 1.051 |

Eight automatic slope rows improve 4.7–21.1% by paired medians. Stock still
wins all five comparable rows. Losses remain visible: direct MAP loses 1–7 ms
per pair, PPML includes +2 ms pairs, one-FE includes +1 ms, and explicit LSMR
includes +7 ms. No additional guard or favorable repeat selection follows.
AW/PW retain their deliberate absorbed finite-column omissions; ordinary fixest
weights are not used as a Stata inference oracle. No PPML speedup is claimed.

## Science, memory and failures

All 93 final headline calls pass, with no timeout, detected overlap or monitor
error. All 39 native pairs and 52 repeat checks pass unchanged envelopes.
Scientific exports and every non-clock solver field are exact, including
routing, iterations, preconditioners, fallback, contraction and certificates.
All 15 direct stock headline comparisons pass.

Three independent N2400 explicit-design pilots cover strong two-partition
slopes, unbalanced three FE and the positive-DoF weak barbell. Both native
artifacts pass all three coefficient/covariance/sample/RSS checks. Stock passes
the first two. In the weak pilot its covariance fails two coordinates, maximum
absolute error 1.38683e-10, 1.1101 times the existing tolerance; native covariance
error is 3.24e-15. The failed stock comparison is preserved and weak timings
remain native-only. This does not erase the earlier weak-reference precision
failure, adjudicate the disconnected comparison, or restart the stopped
rank-deficient weak campaign.

41 distinct Rust tests pass: 14 routing/admission, seven certificate, three MAP,
one accelerator and 16 independent FE-certificate/intercept-slope oracle tests.
They cover exact arithmetic across residual updates, weights, rank deficiency,
empty groups/RHSs and extreme units; private immutable bindings; cache capacity
bounds; exact-budget/one-byte-short behavior; release before CG/LSMR; and fresh
checks on scheduled/final iterations. No optimized Rust unit-test run is
claimed. The release plugin passes actual ABI/panic containment. Five harness
tests and four licensed fixtures pass with explicit terminal sentinels: multiple
slope-only OLS, intercept/slope OLS, PPML saved slopes and OLS transactionality.

Tracked heap increases on the eight automatic slope rows by 2.72, 3.02, 4.08,
4.54, 1.36, .67, 1.36 and 1.36 MB respectively, consistently across pairs.
Other controls have identical tracked peaks. The bound charges up to
`8*N*sum(channels)` plus nested vector descriptors; existing maximum-level QR
scratch remains charged. Numeric columns move into the cache without another
copy. Both fast and low modes may use this optional storage. Insufficient
memory keeps the previous uncached route; this is not a tighter whole-action
memory claim. Static allocation inventory still exits 1 for documented source
drift and is not regenerated.

The preliminary `certcache1` screen also completed 18 calls with exact native
science; its two paired median gains were 15.8% and 8.8%. Finalization made the
context's bindings private, added the exact-budget CG release regression, and
scoped a test-only import. `certcache2` was built but never timed. The original
cases were repeated on final `certcache3`, and only final results enter the
table. All four native diagnostic calls and nine pilot calls complete; the
stock weak scientific failure is distinct from application success. A mistaken
Rust test-target invocation and a Python bytecode-cache permission error are
retained as development setup failures, followed by correct test invocation and
successful diagnostic builds. No numerical algorithm or benchmark guard was
tuned after the initial screen.

## Evidence and next boundary

`receipts.zip` contains immutable controllers, declared cells/seeds, source and
artifact identities, sanitized protocol/phase records, summaries, independent
oracles, exact solver audit, build/test receipts, fixtures and the initial
experiment. Data/binaries and raw licensed logs are excluded. Source bindings
verify 529 baseline and 529 final candidate inputs; per-entry hashes and the
archive checksum are separately verified.

Close geometry-cache tuning. The next bounded target is deterministic parallel
group evaluation for fresh owned-FE certificates, with ordered final reductions
and explicit scratch admission. Remaining projection and certification cost is
large enough to justify that investigation; extending caches across changing
weights or chasing a small control loss is not. Static allocation drift,
weighted constrained fits, nonretaining CG span instability, factor-base
provenance and SCC/Linux confirmation remain open. Frozen 77/16/111 definitions
are unchanged. This is local development evidence, not qualification, release
or completion of the broader speed objective.

Archive verification: 3,864 hashed entries, 5,404,958 bytes, SHA-256
`7e7dd504d58294761a5f835bfa8a8f6570953301b95c1acc1f08389e3ae6118d`.
