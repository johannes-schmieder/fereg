# Single-partition slope recovery checkpoint, 2026-09-21

The final `slope-recovery2` plugin is
`bef34c8c45052866f2bde2b52e0041183013f58e089ba86995f7b246cf3c9c41`.
It uses source `f3184434` plus five recorded production overlays; all five match
the working source. Its ABI and panic-containment check passes. Exact source maps,
pinned reference identities, generated workers and numerical exports are archived.
This is local development evidence, with no remote or installed-package change.

Direct recovery solves independent groups using the established scaled MAP rank
policy, followed by unchanged reconstruction and fresh certification. Its
parallel group certificates reduce in the original order. The final fit residual
may return unchanged only after its own fresh norm/span certificate passes.
See [ADR 0088](../../../../docs/adr/0088-direct-single-partition-slope-recovery.md).

At N=245,760 and seed 104729, three alternating fresh-process pairs give:

| Model/request | K | Workers | Prior median ms¹ | Candidate ms | fixest ms | Median paired ratio |
|---|---:|---:|---:|---:|---:|---:|
| Intercept + one slope, IID | 10 | 1 | 127 | 116 | 102 | 1.196 |
| Intercept + one slope, IID | 10 | 16 | 54 | 51 | 58 | 0.879 |
| Intercept + one slope, IID | 50 | 1 | 397 | 397 | 684 | 0.567 |
| Intercept + one slope, IID | 50 | 16 | 147 | 141 | 184 | 0.761 |
| PW, three slopes only, robust + all outputs | 6 | 16 | 122 | 106 | 113 | 0.938 |
| PW, three slopes only, two-way cluster + all outputs | 6 | 16 | 156 | 138 | 131 | 1.046 |

¹IID pairs use `slope-cache10`; weighted output pairs use the intermediate direct
recovery artifact `slope-recovery1`. The earlier, separately paired recovery1
screen reduced those output medians from cache10's 178/210 ms to 121/154 ms.
Paired ratios are medians of each repetition's ratio, not ratios of medians.
All headline calls and scientific comparisons pass. Narrow serial and clustered
losses remain; two final five-pair blocks and untouched validation seeds are pending.

The intermediate instrumented recovery attributes about 23–25 ms to recovery:
roughly 1.4 ms setup, 2.5–3.1 ms group solves, 0.08 ms transposition and 19 ms
finishing/certification. That evidence motivated parallel fresh certificates.
The diagnostic and production scientific comparisons pass. Native peaks remain
105,632,187/373,033,387 bytes for K10/K50 IID, unchanged from cache10. Final PW
robust/clustered output peaks are 84,790,720/90,699,788 bytes. These observations
do not establish complete whole-action memory coverage.

Validation includes 53 focused slope core tests, four strengthened single-slope
tests, a weighted mock-SPI inference check and four final-artifact licensed
fixtures with terminal sentinels. Coverage includes independent explicit spans,
rank-degenerate and rescaled channels, supplied-weight scaling, bitwise serial/
parallel certificates, zero-step unchanged residuals, inconsistent-target rejection,
partial outputs, exact-budget admission and one-byte-short MAP fallback. PPML
saved-slope, allocation/atomicity and insufficient-DoF behavior remain protected.
Final-artifact AW/FW saved-only and full-output science passes across IID, robust,
FE clustering and two-way clustering. PW full outputs pass at the large timing
size. Native comparisons and independent complete-output audits remain authoritative;
ordinary fixest weights are not used as an FW inference comparator.

The archive contains 113 recorded application calls across nine campaigns:
112 pass and one diagnostic warmup fails. The missing diagnostic ado trace
directory was corrected in the controller and a fresh run passed. A separate
launch stopped before any application call because STATA_MP_PATH was unset;
its plan and traceback are retained. An initial unit-test compile typo and all
subsequent checks are preserved. Science-only timings are diagnostic and may
have overlapped compilation or other science calls.

Large output arrays and raw licensed logs remain local. Array hashes and full
output-audit receipts bind them; portable evidence includes plans, workers,
source/artifact identities, numerical exports, phase records and test outputs.
The broader graph campaign is separate and remains in progress. Allocation
inventories require a new reviewed source checkpoint before their next audit.
