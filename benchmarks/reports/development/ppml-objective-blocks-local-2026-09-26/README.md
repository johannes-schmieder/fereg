# Fixed compensated PPML objective blocks — local development, 2026-09-26

**Accept objectiveblocks1.** This shared categorical/slope IRLS improvement saves
roughly 0.4–2.1% on large public pilot medians and reduces observed peak allocation
by up to 524,032 bytes. It does not establish the broader 10%-faster-than-stock
goal. Small-case losses, the OLS follow-up and stock discrepancies remain below.
The decision and numerical recurrence are specified in [ADR 0219](../../../../docs/adr/0219-compensated-ppml-objective-blocks.md).

## Exact source and method

Baseline **residualrows2**, plugin
`ef3ee205cab739352ccc1024500e26e9c7c874c609001c58cf0bc7038c32f519`, matches all
567 computational inputs at **826bcdfd4db199cba5b772f9a7e611136008fae0**.
Candidate is that revision plus the archived two-file patch to ppml/family.rs
and ppml/memory.rs, plugin
`fe596205eef0faf28680d6682a778ad3f871e9530d57fd9b361ebf36cce84c04`.
The final production inputs match all 567 captured candidate files.

Isolated diagnostic **stateprofile1/stateprofile2** plugins:
`46d6eee03f383780cc218298de8e1d5c7c16c718237b46efef3ef23424afc9fb` /
`8b242fdd6413f3d335edc7a9134e9690ee76a5903e79182d43ddb25603b5e17c`.
Their clocks never enter headline comparisons. Each build uses a separate Cargo
target and passes the actual ABI panic-containment self-test.

Stock is the pinned unmodified fixest library, source archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with glm.tol=1e-10, fixef.tol=1e-11, matched threads, inference and requested
outputs. BLAS is single-threaded. Every PPML timing uses public feppml, robust or
specified clustered inference and fitted means; science cases also cover full
outputs. Three pilot rotations use development seed202609347; five fresh
rotations use predeclared seed202609449. Native baseline/candidate/stock order
rotates. Complete ordered distributions are in [distributions.md](distributions.md).
All fits are uncontended under the recorded process monitor. Compilation,
audits, profiling and archiving do not overlap headline fits.

## Attribution and adopted organization

Finer baseline spans show only a few milliseconds of avoidable accepted-mean
recomputation. No accepted-point cache was implemented. Objective evaluation
costs about 31 ms per large fit and materializes row contributions before serial
compensated reduction. Fixed 4096-row blocks now form their own compensated
sum/correction pairs, with a deterministic ordered merge. The existing team and
private mutable workspace remain; no per-iteration bank allocation or solver
cache is added. Exceptional nonfinite accumulation replays the original scalar
evaluation. Fresh independent final likelihood/null replay remains unchanged.

Paired diagnostic inclusive milliseconds (nested spans are not additive):

| Case | Objective B/C | Canonical preparation B/C | Whole plugin B/C |
|---|---:|---:|---:|
| Million fast | 32.093 / 22.322 | 229.375 / 227.630 | 872.349 / 855.506 |
| Million low | 31.866 / 25.387 | 250.287 / 251.097 | 917.960 / 916.824 |
| Two slopes low | 1.828 / 1.458 | 23.911 / 23.315 | 71.175 / 69.451 |
| Three FE, two slopes | 2.770 / 2.217 | 65.853 / 63.949 | 152.848 / 152.233 |
| Factor controls | 6.497 / 5.267 | 146.756 / 153.206 | 495.420 / 512.222 |
| Four slopes low | 2.303 / 2.209 | 47.369 / 46.086 | 118.452 / 116.971 |

The factor diagnostic regression is preserved; faster objective work is not
proof of a whole-command gain. Frozen uninstrumented factor pilot/fresh medians
are 570/569 and 625/623. No block-size, layout or seed tuning followed.

## Public distributions and remaining gaps

Pilot B/C/stock medians in ms: million low **938/934/865**, million fast
**864/846/868**, two slopes low **70/72/122**, three-FE two slopes
**150/152/231**, factors **570/569/702**, serial **547/550/951**.
There are **9 paired wins, 7 losses, 2 ties**. Four of six families meet 0.90
stock in every pair; million fast and low do not. The latter's candidate/stock
range is **1.067–1.085**. Their median gaps to the target are **64.8 ms fast**
and **155.5 ms low**.

Fresh B/C/stock medians: large fast **672/666/782**, low **724/715/772**,
T4 fast **462/457/667**, factors **625/623/3253**, three-FE two slopes
**192/193/300**, four slopes low **119/119/115**, serial **557/557/964**,
weak **134/134/789**, disconnected low **94/93/99**.
There are **22 native paired wins, 16 losses, 7 ties**. The first two stock
comparisons are scientifically unranked. Of seven comparable families, five meet
0.90 in every pair. Four-slope low (**1.000–1.132**) and disconnected low
(**0.913–0.980**) miss; median target deficits are **15.5 ms** and **3.9 ms**.
No universal non-regression or universal stock win is claimed.

OLS medians B/C: **66/65, 77/77, 16/15, 211/209, 159/162, 18/17**.
The only slower median prompted one predeclared identical-input five-pair repeat:
B **159,164,162,163,161**, C **158,160,169,163,158**, medians **162/160**.
Full and warm numeric input hashes match. The initial loss does not reproduce;
one 7-ms follow-up loss remains. Both blocks are retained separately.

## Science, memory and failures

**135 distinct focused Rust tests pass**: family25, memory53, prepared IRLS57.
The existing refined-checkpoint count assertion fails 0 versus 1 on both the
candidate and an isolated exact-826bcdfd baseline. It is not repaired or removed.
Initial prototype family results precede the final source and are diagnostic
only; the final source-bound tests supersede them.

**Eleven licensed fixtures** have successful rc=0 terminal sentinels, including
independent categorical/three-partition/higher-dimensional oracles, weights,
inference, atomic transactions and real T4 categorical/slope 16-iteration audits
with zero allocation events and growth. **237 public-state audits** and
**12 explicit Poisson/Hessian/sandwich comparisons** pass. All **79 native PPML
pairs** and **23 OLS pairs** have exact scientific exports. New analytical
Poisson tests cover partial blocks, thread/bank independence, extreme weight
scales, overflow, first-error ordering, retry and address-space sizing.

Anchored comparisons pass **135/146**; direct candidate/stock **56/67**.
The older small three-FE full-output residual discrepancy remains. Both fresh
large designs have six fitted-mean coordinates beyond the existing comparison
envelope in every repetition: representative maximum absolute error
**5.51e-7**, tolerance ratio **1.303**. Coefficients, covariance, objective and
each independently checked public state pass. Both baseline and candidate show
the discrepancy; candidate results are exact to baseline. Its source is not
adjudicated here and no tolerance or stock stopping rule is changed. Those stock
comparisons remain unranked. There are no incomplete comparisons, application
failures, timeouts, interrupted fits or detected overlaps.

At large T8, objective storage falls **512 KiB -> 256 bytes**; T4
**256 KiB -> 128 bytes**. Whole-fit observed and selected modeled peaks fall by
**0..524,032 bytes**. Scratch peaks are unchanged; separate separation phase
model bounds vary **-48..+48 bytes**. Independent final-replay storage and all
other allowances are unchanged. No physical-cap qualification is claimed.

A preparation script first requested a nonexistent OLS declaration filename and
was corrected before any fit. The OLS control declaration's purpose prose was
inadvertently inherited from the previous experiment; its cells, source,
variants and repetitions were correct. The original plus an explicit adjudication
are preserved, rather than rewriting the receipt.

Static inventory drift, wire-minor/checkpoint-count failures, weighted
constrained-fit/nonretaining-CG failures, SCC deferral and stopped campaigns
remain open. Frozen 77/16/111 definitions and pins are unchanged. No push,
release or qualification campaign occurred.

## Next subsystem

Stop tuning family reduction blocks and accepted-mean caching. The next coherent
boundary is the **independent outer score certificate**, about **40 ms** on the
million-row diagnostic fits. Its current code separately traverses original
rows for two scale sums, each finite column, and each FE partition. Finite
columns already run concurrently; FE channel accumulation and scale sums remain
serial. This is measured total attribution, not a claim all 40 ms are removable.

First split these costs, then test fresh original-row moment batching and
parallel ownership of disjoint FE directions using immutable membership indices
and existing workspace. Consider fused original-input score/norm work with
bounded admitted partials if membership gathers lose locality. Preserve each
current-state score, its mathematical normalization, checking frequency and
independence from Newton/projected products. Compare numerical envelopes with
independent oracles; do not demand an obsolete recurrence's bits. This shared
categorical/slope boundary is preferable to another tiny canonical handoff or
case-specific guard. The four-slope low canonical work (~46 ms here) remains a
separate substantial target; its solver does not become faster through this
objective change.

## Receipts

Work root: `target/ppml-working-state-20260926`. The content-addressed archive
retains complete source snapshots, patch, declarations, controller/monitor code,
all attempts, audit results, tests and separate diagnostic spans. Generated DTA
and binary row arrays are hashed and omitted; raw licensed logs are excluded.
Use `restore_receipts.py` with a new directory. Archive identity, restored hash
verification and reviewed path literals are recorded in the adjacent verification
files. These receipts bind this candidate; older timing claims do not transfer.

The archive contains **10,616 restored and hash/privacy-verified logical receipts**.
SHA-256: `f5c9d9bc12376775673e61a5ccd921df8278c63b3d710556d24137c62e861aab`.
