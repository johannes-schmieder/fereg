# P4 combined frame refresh: evaluated and rejected

Reject the standalone prototype against accepted `7739e8e`. It removes duplicate
validation and invalidation when PPML supplies a complete response/weight frame,
and applies the existing exact-weight identity rule to ordinary as well as final
refreshes. All four prespecified command screens miss the gain gate. The patch
is archived; accepted native source is restored without adopting the change.

## Mechanism, identity and validation

The [source audit](source-audit.json) identifies one removed response scan, one
old-weight positivity/normalization scan and duplicate output invalidation per
changed complete frame. New inputs still validate once before mutation. Prior
response storage for categorical warm projection, changed-weight copies, exact
raw-bit/all-scale-field matching and sample/topology generation checks remain.
No observation-sized allocation, numerical tolerance or public solver region is
introduced. Separate-update methods remain test-only comparison seams in the
prototype. These are source-derived pass counts, not hardware traffic measures.

[Identities](built-identities.json) bind 178 inputs and isolated universal plugins:

- Accepted baseline: `2689faf0418896ac66f0cb7fcaa9552b339a1928ab38409b43710dab6f63faeb`.
- Rejected candidate: `ddb302c64a12f482a7797fa249191a94e8e132ed6277a05ce2924561013b9b2c`.

Both target architectures forcibly rebuild core/plugin in the isolated source.
Universal/static and actual arm64 ABI checks pass. Main and installed dist were
not built. The [patch](implementation.patch), [source archive](source-archive.json),
executed script and log hashes preserve the rejected candidate.

Eleven focused core tests and 95 serial plugin tests pass. New fast/low tests
compare combined and separate changed-weight solves with exact coefficients,
residuals and bread, including one-bit weight and all scale-field changes. Invalid
frames leave the prior certified output usable in the prototype. Existing stale-
norm, exact-weight, strict rollback/checkpoint and team checks pass. This is not
a claimed red/green baseline regression; the new tests were run on the candidate.

Ten independent licensed fits per arm pass with explicit terminal sentinels:
three K8 unit/FW/PW Poisson/Hessian/sandwich cases, one generated-factor case and
six K2 unit/FW/PW robust/cluster cases. The narrow fixture supplements the
prospectively added one-control screen. See [validation](validation-summary.json).

## Complete-command screen

The [protocol](protocol.md) requires both 5% and 10 ms command gain, or a material
admitted memory improvement. The [narrow supplement](narrow-screen-protocol.md)
was fixed during the build, before any timing. It uses a distinct one-control,
four-worker request on the original million-row full/warm data, retaining the
original three K50 screens. No data value, seed or tolerance is changed.

All eight commands run sequentially, with rotated arm order and no overlapping
build/test/oracle work. Scientific CSVs, sample hashes and discrete solver fields
are exact. These are single paired screens, not repeated gain estimates.

| Case | N / finite K / workers | Command seconds | Paired change | Weight/response phase seconds |
|---|---|---:|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 7.408 → 7.496 | +1.19% | 0.05651 → 0.05295 |
| FW exposure, missing, robust | 245760 / 50 / 4 | 2.509 → 2.419 | -3.59% | 0.02095 → 0.01973 |
| PW offset, missing, clustered | 245760 / 50 / 16 | 2.219 → 2.214 | -0.23% | 0.02086 → 0.01949 |
| Complete one FE, one control | 983040 / 1 / 4 | 1.427 → 1.458 | +2.17% | 0.05529 → 0.05641 |

The main phase moves only a few milliseconds. Most of the FW command reduction
is in unchanged finite/strict solving; it is not attributed to frame validation.
Outer/inner iterations remain 3/5 for complete/narrow and 4/6 for FW/PW. There
are no step halvings or fallbacks. Every native peak and model is exactly equal
within pairs and admitted: complete 2.522/2.963 GB, FW 633.45/745.43 MB,
PW 645.21/935.51 MB and narrow 1.240/1.372 GB (observed/modeled).
Sampled process RSS is effectively unchanged in complete/narrow, rises slightly
for FW and falls for PW; it is distinct from native live allocation.

[All phases and peaks](summary.csv), [screen summary](screen-summary.json) and
[content-addressed captures](captures.json) retain the complete results. No pair
exceeds the protected cost gate, but no target meets the gain gate either. No
extra seeds, holdout timings, trace or private allocation audit were run after
this rejection. Low-mode coverage here is focused core validation, not a public
performance claim.

## Disposition and next boundary

Restore all 178 accepted native inputs and retain the tested predictor adoption.
The standalone refresh simplification is not adopted merely because its science
passes. P4's broader pass/state items remain open. A next candidate is reuse of
already-computed Poisson means; it must track accepted/trial mean validity through
line-search swaps, strict-candidate scratch reuse, bounds changes and failures.
No such mean-reuse change is implemented or measured in this report.

The installed plugin and unrelated work remain intact. Evidence is local Mac
development only. R1–R5 are excluded from the current user-directed run; no
agents, cluster jobs, upstream changes, external benchmark campaign or push
occurred here.
