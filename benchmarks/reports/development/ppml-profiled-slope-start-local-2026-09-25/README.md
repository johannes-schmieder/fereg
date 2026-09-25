# Profiled slope-coordinate Poisson starts — local development, 2026-09-25

Adopt **slopeinit2** under [ADR 0186](../../../../docs/adr/0186-profiled-slope-poisson-start.md).
One complete conditional-intercept sweep followed by safeguarded slope-coordinate
Newton proposals avoids one or two full outer iterations in the fresh slope
cases. Their medians improve **2.5–11.4%**, with **38 wins and two ties in 40 pairs**.
Eight of ten fresh PPML medians clear the 0.90-times-comparable-stock target.
The threaded two-slope and narrow categorical cases still lose to stock. A small
OLS control retains a measured 2-ms loss on confirmation; science remains exact.
This is development evidence, not qualification or completion of the broader goal.

## Exact sources and comparator

Baseline **jointteam1** is the compact-operator implementation accepted at
`cda48a0e`, plugin SHA-256
`89b247490669492e928b5cb9fc9f33f3b0b85fef073733624b084f35a84d970b`.
Its 554 captured inputs plus separately bound compact licensed fixture are
explained in the [preceding report](../ppml-cooperative-slope-products-local-2026-09-25/README.md).
Both seed candidates use `cda48a0e` plus one captured
`crates/fereg-core/src/ppml/prepared_irls.rs` overlay. All **555 captured inputs**
of adopted slopeinit2 match the accepted checkpoint.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
|slopeinit1|Intercept sweep only; rejected as the solution|2644ed1f10c98e94216b5378ab59498ba9049194c3bc167713882606d272b976|
|slopeinit2|Intercepts then profiled slope proposals; adopted|2cfb165a645db02ef6fed1a6c787bfbdbee89893757e3d556eedc8514280c79b|

Both builds pass the actual-plugin ABI and injected-panic containment checks.
Stock remains pinned, unmodified fixest **0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable Poisson uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application
threads, robust inference and fitted means; BLAS is serial. Native low mode is not a matched physical-memory cap. No instrumented
reference supplies a headline time. Default-reference discrepancies remain
separate historical evidence.

Public warm commands run in fresh processes after matched warm-up, rotating
baseline/candidate/stock. Startup, generation, import and exports are outside
both command clocks; inference and requested means are inside. All headline
calls pass the captured isolation monitor. There are no overlapping builds,
profiles, archives, audits or competing fits. Diagnostic calls are separate.

## Computational comparison and candidate selection

The current source excluded every slope design from the categorical initial
intercept sweep. Those intercept directions still belong to an intercept-plus-
slope model with slope coefficients initially zero. The first candidate extends
that exact conditional sweep. It passes independent science, but leaves the
principal two-slope case at seven outer iterations and 92 ms; large three-FE
medians slightly worsen. Its one smaller three-FE gain does not justify treating
that preparatory extension as completion.

The second candidate profiles a group's intercept for one Newton proposal along
each scaled slope coordinate. The objective change includes the full intercept
correction when preceding partitions moved the group mass. Compensated scalar
moments, a shifted exponential sum, finite-bound checks and bounded step halving
make a finite model-space initial guess. A declined coordinate keeps its prior
guess; invalid seed arithmetic restores the entire global start. All initial
canonical rank construction and fresh ordinary/strict/final acceptance remain.
There is no extra heap bank, factor, cached solution, thread team or public knob.

Three draft unit-test failures are preserved with their exact source snapshots:
interleaved intercept/slope updates did not retain a prescribed separable
intercept optimum; separated stages then missed the intercept correction on
constant slopes, including rounded tiny variance. The adopted organization
finishes all intercepts first and explicitly handles exact constant directions.
All corresponding final tests pass. No tolerance or benchmark threshold was tuned.

The pinned R source's default feglm initialization delegates to its family
initializer; explicit coefficients have a separate rough FE fit. This is a
source inspection, not an instrumented attribution of reference time. Our
model-space seed is a separately derived candidate. See the captured source
hash/line receipt for that inspection.

## Public distributions

Development seed **202609347**, three rotations per candidate. Fresh seed
**202609420**, five rotations, and OLS seed **202609421** were declared before
generation. The original fresh declarations for candidate 1 are preserved as
unused; candidate 2 inherits the same cases/seeds before any fresh outcome exists.
Values are milliseconds, `median [min,max]`. Losses/ties are paired against the
native baseline, not stock counts. All measured states are scientifically rankable.

### Intercept-only rejected screen

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|61440/10/8|196 [195,200]|199 [199,201]|231 [226,232]|1.015|0.861|2/0|
|Same, low|61440/10/8|214 [212,228]|216 [215,217]|233 [231,233]|1.009|0.927|2/0|
|Two slope partitions T8|30720/6/8|92 [91,93]|92 [92,95]|80 [79,80]|1.000|1.150|1/1|
|Smaller three-FE slopes T4|30720/6/4|101 [100,102]|95 [93,95]|117 [114,118]|0.941|0.812|0/0|
|Two slope partitions T1|30720/6/1|150 [146,160]|149 [146,152]|317 [314,319]|0.993|0.470|1/0|
|Narrow categorical|21120/2/4|37 [36,38]|35 [35,36]|31 [31,31]|0.946|1.129|0/0|

### slopeinit2

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|61440/10/8|196 [196,197]|188 [188,189]|230 [226,233]|0.959|0.817|0/0|
|Same, low|61440/10/8|214 [212,216]|205 [203,207]|229 [229,231]|0.958|0.895|0/0|
|Two slope partitions T8|30720/6/8|93 [91,94]|89 [88,90]|77 [74,80]|0.957|1.156|0/0|
|Smaller three-FE slopes T4|30720/6/4|102 [101,102]|89 [89,90]|117 [116,118]|0.873|0.761|0/0|
|Two slope partitions T1|30720/6/1|150 [147,152]|139 [138,139]|319 [318,320]|0.927|0.436|0/0|
|Narrow categorical|21120/2/4|36 [35,36]|36 [35,36]|30 [29,31]|1.000|1.200|1/1|

### fresh

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes, fast|69120/10/8|236 [234,240]|209 [207,209]|268 [267,272]|0.886|0.780|0/0|
|Same, low|69120/10/8|255 [253,258]|232 [230,234]|268 [267,270]|0.910|0.866|0/0|
|Two slope partitions T8|36480/6/8|105 [104,106]|102 [101,104]|83 [83,90]|0.971|1.229|0/0|
|Smaller three-FE slopes T4|34560/6/4|110 [110,114]|98 [97,99]|131 [129,131]|0.891|0.748|0/0|
|Two slope partitions T1|36480/6/1|172 [170,175]|161 [160,165]|325 [322,330]|0.936|0.495|0/0|
|Narrow categorical|24000/2/4|39 [39,40]|39 [39,40]|32 [31,32]|1.000|1.219|0/5|
|Weak slope barbell|14400/4/4|40 [39,41]|39 [38,40]|724 [723,728]|0.975|0.054|0/2|
|Disconnected slopes, low|24960/8/4|107 [106,110]|102 [101,105]|147 [146,148]|0.953|0.694|0/0|
|Two slopes per partition|28800/10/8|150 [148,152]|141 [140,142]|192 [190,193]|0.940|0.734|0/0|
|Four categorical FE, low|30000/8/4|169 [167,170]|170 [167,175]|489 [487,490]|1.006|0.348|3/0|

### ols

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Small serial two-FE|3840/2/1|53 [52,55]|56 [47,57]|—|1.057|—|2/0|
|Three-FE|30720/8/4|52 [50,55]|51 [51,52]|—|0.981|—|1/0|
|OLS slopes|28800/6/4|89 [87,90]|88 [87,90]|—|0.989|—|1/0|

### ols-confirm

| Case | N/K/T | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses/ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Small serial two-FE|3840/2/1|52 [50,54]|54 [50,57]|—|1.038|—|4/1|

Candidate 1 has 11 wins, six losses and one tie in 18 pairs. Candidate 2 has
16 wins, one loss and one tie; all 15 slope pairs improve. Fresh has 40 wins,
three losses and seven ties in 50 pairs; all three losses are the unchanged
four-FE categorical control. Its median loses 1 ms. The narrow categorical
control ties in every pair. No favorable guard is fitted to these controls.

Fresh slope outer iterations change **6→4, 6→4, 7→6, 6→4, 7→6, 6→5, 7→6,
8→7** (in table order, omitting categorical controls). Recorded mixed inner
units change **85→50, 86→58, 79→59, 75→45, 79→59, 104→80, 111→92, 128→98**.
These units mix maximum column absorption counts and ordinary joint Krylov steps;
they are not fixest iteration counts. All public line-search halving totals
remain zero. Seed trial-halving work is part of initialization, not an IRLS
halving count.

OLS has five wins and four losses in nine pairs, with exact scientific exports.
The small serial control's initial candidate range 47–57 ms warrants one bounded
same-input seven-pair confirmation. Binary full/warm hashes match. It retains
**52/54 ms**, with two wins, four losses and one tie; all seven scientific exports
are exact. This is a visible small performance loss, not a universal
non-regression claim. The other OLS control medians improve by 1 ms.

## Phase attribution and remaining bottlenecks

Existing production phase clocks give the following fresh median costs. These
are phase observations from each same-source block, not an additive decomposition
of median whole-command time.

| Case | Preparation B/C ms | FE projection B/C ms | Strict final B/C ms |
|---|---:|---:|---:|
|Large fast slopes|32.90 / 38.98|77.49 / 56.11|46.88 / 44.34|
|Large low slopes|32.96 / 38.98|84.12 / 64.10|56.78 / 57.96|
|Two slopes T8|13.87 / 17.47|34.19 / 30.27|12.82 / 13.09|
|Smaller three FE|14.72 / 17.67|34.41 / 24.23|18.54 / 18.17|
|Multiple slopes|16.67 / 21.11|65.07 / 54.09|21.92 / 22.07|

The seed spends more preparation time to avoid full weighted solves; final
certification remains real work and can increase. The remaining two-slope
candidate also spends 9.84 ms in finite solving, 5.30 in objective/line-search,
2.13 in weight refresh and 6.16 in publication. These phases include more than
the original small dense FE product, which the preceding report places near
1.4 ms across the fit. More dense-product micro-tuning cannot plausibly recover
the **27.3 ms** needed for this case's 0.90-stock target. Narrow categorical
needs **10.2 ms**; its FE projection is only 6.34 ms and strict final 3.66 ms,
so full public-command setup/publication deserves a separate bounded profile.

Separate `--trace` diagnostic calls preserve baseline and candidate score paths.
For the full development two-slope fit, baseline scores over seven iterations
are approximately `.0752, .00657, .000551, .0000457, 4.42e-7, 4.26e-11, 8.47e-14`.
The candidate's six scores are `.00275, .000242, 1.18e-5, 3.02e-8, 1.66e-12,
1.53e-14`. Both retain two consecutive independent convergence passes. Observing
nearly all stages of the inner schedule initially suggested an accuracy-schedule
bottleneck, but the trace shows substantial nonlinear error before convergence.
Schedule causation is unproven; do not loosen or retune it from those counts.

The first diagnostic launch supplied an inherited environment switch, which the
measurement harness correctly clears. Those four no-trace calls and their audits
are preserved as a nongating attempt. The corrected four-call `--trace` block
captures the intended protocol. Neither block supplies headline timings.

## Scientific, memory and receipt boundary

The final prepared-IRLS Rust run has **53 passes and one known failure**. The
unchanged baseline independently reproduces the same dynamic-range assertion:
zero strict rejections versus an expected one. This pre-existing assertion is
not repaired or hidden. All three new tests pass, covering an analytic profiled
Newton step across nine slope/weight scale combinations, independent separable
Poisson truth with duplicate/zero slopes, offsets, exact group scores, explicit
warm starts, Mu and slope-only exclusions, bounds/overflow refusal and complete
restoration. Existing rank/strict/Hessian/rollback tests otherwise pass.

All **14 licensed fixture executions** pass: five independent science/control
fixtures on each built candidate and fast/low T1/T4 compact allocation audits on
the adopted candidate. Explicit-dummy Poisson and independently reconstructed
Hessian/sandwich oracles cover two/three partitions, unit/FW/PW, offsets, robust
and clustered inference. One-FE and OLS fixtures protect exclusions. All four
real warmed allocation windows have zero events/current growth, stable
capacities and fresh/warm agreement.

Across both candidates and diagnostic attempts, **266 PPML public states**,
**176 baseline comparisons**, **86 direct stock comparisons** and **16 OLS pairs**
pass. There are 258 headline PPML calls, eight diagnostic calls and 32 headline
OLS calls. Independent sample, model-space, score and objective-replay checks
complement the explicit-design fixtures. There are no application failures,
timeouts, scientific discrepancies or isolation errors in the public screens.
The failed draft unit tests and reproduced baseline assertion remain visible.

All **68 accepted-candidate headline PPML pairs** have exact tracked allocation
peaks and scratch reports; selected-memory telemetry differs by at most 48 bytes.
The rejected candidate's 18 pairs also have exact peaks/scratch. There is no new
admission term or payload. No RSS or physical-cap equivalence claim follows.

The content-addressed archive has **10,140 logical receipts / 2,397 payloads /
25,296,608 bytes**, SHA-256
`ceec6ecc2342a71f92a0915f8fe9e5a5d4f2140019c3920f4aae1345fe1e8d47`.
Its sidecar, `archive-verification.json` and `restore_receipts.py` bind the
sanitized declarations, source snapshots, build/test failures and successes,
controllers, scientific exports/audits, full timing distributions, phase summaries
and filtered fixture/trace protocols. Omitted raw data/plugins/licensed logs
retain hashes. Restored logical hashes and privacy exclusions are verified.
Work root: `target/ppml-slope-start-20260925`.

Retain the two-stage seed and retire the standalone intercept-only candidate.
Next investigate a coherent ordinary nonlinear coefficient correction that
reduces the observed Poisson score error before another complete weighted solve,
with explicit model-space/objective authority and independent canonical final
acceptance. Separately profile the narrow public-command overhead. Do not start
another seed sweep-count, dense-factor, paired-lane or threshold tuning cycle.
Seeds 420/421 are now consumed validation. Frozen 77/16/111 definitions, static
allocation drift, constrained-fit/CG failures and deferred platform work remain
unchanged. No qualification, push, release, SCC retry or stopped campaign occurred.
