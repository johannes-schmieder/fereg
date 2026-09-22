# Score-cache and normalized-norm pass fusion: rejected local experiment

Reject `scn1` after two unchanged five-pair confirmation blocks. Its small PW
screen gains do not persist consistently, while FW is slower in both blocks.
Restore the complete accepted `dtk1` computational source and the prior
diagnostic instrumentation. No weight/VCE-specific guard is fitted.

## Experiment

Successful serial cached-slope inference already copies the absorbed matrix
into its packed score cache, then scans it again for OLS's max-normalized
weight norms. The draft computes those norms freshly during the copy/mean
loop. It preserves each norm's scalar recurrence and complete row order,
including outcome and the absorbed-intercept coordinate omitted from the
score cache. Mean accumulation keeps the same fixed blocks and compensated
merge. Two bounded stack arrays replace the separate norm scratch scan;
there is no new heap owner.

The existing cache byte limit precedes norm work. A rejected cache retains the
separate scan. The empty packed-cache case computes fresh norms independently.
The guard requires a successful cached-slope route, one actual worker, no
already-cached norms and a bounded width. Wider, threaded, strided, unit cached-
norm, explicit/shared and low-memory routes retain their prior eligibility.
Raw absorption norms are never rescaled or substituted for normalized norms.

The instrumentation adapter recognizes both the prior and draft row-loop
shapes. Its exact experiment version is preserved in `sources.zip`, even
though the working adapter is restored after rejection.

## Whole-command evidence

Apple M2 Ultra, Stata MP 19, N=245,760, fresh worker processes, fixed thread
teams and alternating pairs. The sixteen-cell three-pair screen has 141 calls.
Two unchanged seven-cell five-pair native blocks add 140 calls. All timing
roots report no detected overlap; builds, tests, audits and phase diagnostics
run separately from headline fits. The full screen/confirmation table is in
[timings.md](timings.md).

| Serial case | Screen paired ratio | Confirmation 1 | Confirmation 2 |
|---|---:|---:|---:|
| AW K10 robust, full outputs | 1.0043 | 0.9866 | 1.0088 |
| AW K10 cluster2, full outputs | 0.9964 | 1.0070 | 0.9857 |
| PW K6 robust, full outputs | 0.9721 | 1.0144 | 0.9860 |
| PW K6 cluster2, full outputs | 1.0041 | 1.0000 | 0.9959 |
| PW K6 robust, saved effects | 0.9741 | 0.9896 | 1.0266 |
| Unit K10 robust, full outputs | 1.0000 | 0.9801 | 1.0000 |
| FW K10 robust, full outputs | 1.0175 | 1.0263 | 1.0087 |

Ratios are medians of per-repeat candidate/prior ratios, not ratios of marginal
medians. AW and PW alternate between small improvements and losses. FW's
paired medians are slower in both confirmation blocks. This supports rejecting
the added complexity; it does not establish a statistically significant
regression for every unfavorable cell. Threaded and wide controls remain
small/mixed. Ordinary fixest is excluded from FW inference comparisons.

Nine separate diagnostic calls compare prior diagnostic, ordinary candidate
under phase tracing and candidate diagnostic artifacts. All numerical exports
and saved arrays match exactly. The total phase includes cache construction
and normalized norms:

| Case | Prior diagnostic ms | Ordinary candidate ms | Candidate diagnostic ms |
|---|---:|---:|---:|
| AW K10 | 14.523 | 13.438 | 14.486 |
| PW K6 | 8.431 | 8.199 | 7.897 |
| AW K50, unchanged strided route | 28.330 | 27.897 | 26.980 |

These are single traces, not headline comparisons. The draft's packed-cache
row timer now includes norm arithmetic, so comparing that subphase directly
with the old copy/mean-only timer would be misleading. Actual-N K50 uses the
strided cache; its warm-up packed trace is excluded. Removing a traversal
barely changes AW's combined phase and saves only about 0.5 ms for PW in the
diagnostic pair. The arithmetic cost remains; fewer passes alone do not
justify adoption.

## Scientific checks and identity

All 281 timed calls pass. All 118 timed native pairs have exact six numerical
exports, samples, requested saved arrays and 28 non-timing solver fields,
with equal tracked allocation peaks. Including nine diagnostic calls, the
archive contains 290 successful application records.

Seven focused release Rust fixtures pass in five invocations: fresh norms
versus independent columnwise scalar evaluation; exact cached values, means
and reservation records; outcome/omitted-intercept coordinates; empty caches,
sample/block/width boundaries; all four weight kinds and extreme values;
budget rejection, wrong dimensions/team, allocation failure and nonfinite
inputs; existing binding, packed/strided inference, mutation and eligibility
guards. Both new plugins pass actual ABI/self-test harnesses.

| Artifact | Plugin SHA-256 |
|---|---|
| Prior dtk1 | `77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151` |
| Rejected scn1 | `f1567fd907312b754ffd6173f89cd66d1aaaa8a42c8ec9546041c30e86de28c0` |
| Prior diagnostic dtk1d | `12161eee6e6cc37c8920183f63fe2be34812e590f26cbb5652b27a1f0401b769` |
| Rejected diagnostic scn1d | `7ebab9d0024cb4af3407cb327eba98d5e730557ed163e910aa60bcd96234eb89` |

The candidate is based on `28f3f47d` plus the six pending graph/window files;
its new production/test changes are confined to `ols/prepared_fit.rs`.
Complete 523-file source trees reconstruct from the four identities and
captured overlays. `scn1-rejection1.json` binds exact restoration to `dtk1`,
including the separately restored instrumentation hash. The six pre-existing
drafts are unchanged.

`evidence.zip` preserves portable measurements, worker sources and numerical
exports, with explicit hashes for large local-only saved arrays. `sources.zip`
preserves the rejected implementation, tests, instrumentation and controllers.
`checks.zip` preserves tests/builds, audits, rejection and filtered traces.
The verifier passes 7,923 stored entries, 272 omitted arrays and four complete
source trees; the separate privacy receipt records its scan. Raw licensed logs
and binaries are excluded.

This is local development evidence. Serial losses, scientific reference gaps,
graph/window adoption, Linux confirmation and final integration remain open.
No earlier final-block claim transfers to another source or artifact.
