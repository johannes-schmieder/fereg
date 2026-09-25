# Fast retained canonical PPML products: local development evidence

Accepted local development milestone, 2026-09-24; **not qualification**. The
whole-public >=2-FE PPML target of <=0.90 times comparable pinned stock remains
unfinished. [ADR 0176](../../../../docs/adr/0176-fast-canonical-ppml-products.md)
records the design. No frozen 77/16/111 definition changes, SCC retry, stopped
campaign restart, tolerance relaxation or release action.

## Source and artifact boundary

Baseline **ownedrhs2**, plugin SHA-256
`f27b1719718146d638493caa5a6769f90f3e828e89c9a793a553caced27ec3c9`, is the
accepted e9bb61460eb4355513e9fda8639b0fdaac1128f8 computational state. Its prior
source verification remains in the column-owned-RHS report.

Candidate **fastproducts1**, plugin SHA-256
`96156a9c3970c2001d4e04795ef9e82eade204622e3eae3ce315f5b0ec3c1d3e`, is captured
from **e9bb61460eb4355513e9fda8639b0fdaac1128f8** plus:

- `crates/fereg-core/src/ols/canonical_products.rs`
- `crates/fereg-core/src/ols/linear_session.rs`
- `crates/fereg-core/src/ppml/prepared_irls.rs`

All **551 captured inputs match final computational source**. The immutable
identities include every source hash, plugin and Rust archive hashes, build
logs, ABI result and capture recipes. Diagnostic artifacts **lifecycleledger1**
and **lifecycleledger2** instrument the committed baseline only; their timings
are never stock-comparator headline evidence. No production changes follow the
fresh-case declaration.

Apple Silicon local development, release arm64 plugin, Rust 1.85.1, licensed
Stata 19 MP8. Pinned unmodified fixest 0.14.2 uses matched native threads, BLAS1,
`glm.tol=1e-10`, `fixef.tol=1e-11`; archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Default-reference accuracy failures remain separate historical evidence.

## Measurement-driven choice

The two isolated baseline profiles use development seed 202609347, three cases,
two repetitions, separate warmup and complete public-command spans. Unioned
nested intervals distinguish inclusive time from exclusive work; the raw spans,
accounting scripts and ledgers are preserved. All twelve diagnostic public
states pass fresh score/model-space/objective checks. These are attribution
observations, not uncontended stock rankings.

| Baseline diagnostic case | Canonical preparation ms, inclusive | Ordinary joint boundary ms | Actual ordinary numerical recurrence ms |
|---|---:|---:|---:|
| Low connected N21120/K2/T4 | 8.13–8.39 | 4.53–4.60 | about 0.25 |
| Low disconnected N42240/K10/T8 | 29.5–29.6 | 22.32–22.69 | about 5.4–5.6 |
| Fast connected N21120/K10/T4 | 14.4–15.6 | LSMR 10.0–10.3 | LSMR 5.1–5.4 |

The narrow ordinary recurrence cannot explain or close the whole-fit stock gap.
Its geometry, reconstruction and independent checking cost much more. Fast
canonical construction spends approximately 5.0–5.3 ms on the initial retained
projection, 3.84–3.88 ms on strict fused reprojection, 1.37 ms refreshing norms
and 4.08–4.92 ms in rank assembly. These are aggregates within the fit and nested
inside the canonical column, not additional additive costs.

The chosen change generalizes the already certified direct-product engine to
fast storage. Current-weight component factors serve all response/control RHSs;
the fresh original-input residual pass supplies the projection certificate and
one-use Gram. Certified controls are written once into the existing dense bank.
The rank solve avoids rebuilding those moments. Shared tile preparation preserves
streamed-group efficiency. The fused working-response anchor advances with that
physical frame, including across strict rollback. No ordinary solver, checking
tolerance, inference rule or publication condition is replaced.

## Whole-public timings

All ranked calls request robust covariance and saved means, use identical final
samples and comparable work, and pass the independent public-state audits.
Setup/data generation and separate warmup are outside each reported command;
loading, estimation, validation and requested output creation are included.
Baseline/candidate/stock order rotates each repetition. The source-captured
process monitor reports no application failure, timeout, competing fit, build,
profile, archive or monitor failure. No compilation or analysis fits run beside
headline calls. A clear sampled monitor is not proof of zero ambient OS load.

The six-case development screen uses seed 202609347 and three rotations. Its
medians B/C/stock are 58/51/54, 118/110/76, 45/43/30, 128/108/167, 75/71/121,
and 49/49/54 ms. The last is the unchanged low control. Full observations,
phases and native/stock science comparisons are preserved.

Fresh validation is predeclared seed **202609395**, five rotations. Fast mode
unless stated. **Median [minimum–maximum] ms**; native losses count repetitions
where candidate exceeds the paired baseline.

| Case | Baseline | Candidate | Stock | Candidate/stock medians | Native losses |
|---|---:|---:|---:|---:|---:|
| 0 | 62 [61–63] | 55 [54–57] | 62 [60–63] | 0.887 | 0/5 |
| 1 | 119 [115–121] | 108 [107–108] | 90 [86–93] | 1.200 | 0/5 |
| 2 | 48 [47–48] | 46 [45–47] | 32 [32–34] | 1.438 | 0/5 |
| 3 | 122 [119–126] | 112 [112–113] | 164 [162–167] | 0.683 | 0/5 |
| 4 | 89 [87–93] | 80 [79–82] | 143 [142–144] | 0.559 | 0/5 |
| 5 | 416 [415–418] | 240 [239–243] | 385 [383–391] | 0.623 | 0/5 |
| 6 | 1236 [1230–1249] | 643 [638–646] | 971 [967–979] | 0.662 | 0/5 |
| 7 | 102 [101–104] | 102 [101–108] | 383 [379–388] | 0.266 | 2/5 |
| 8 | 52 [51–53] | 52 [52–54] | 61 [58–62] | 0.852 | 3/5 |
| 9 | 110 [108–111] | 110 [109–112] | 126 [121–127] | 0.873 | 2/5 |
| 10 | 85 [85–86] | 86 [84–87] | 96 [89–97] | 0.896 | 3/5 |
| 11 | 862 [842–924] | 856 [848–900] | 971 [957–981] | 0.882 | 3/5 |

Case definitions, unchanged after declaration:

0. Connected N23040/K10/T4.
1. Four disconnected components N46080/K10/T8.
2. Connected narrow N23040/K2/T4.
3. Unbalanced N32640/K28/T4.
4. Connected serial N24960/K10/T1.
5. Connected wide N34560/K64/T8.
6. Disconnected wide N53760/K102/T8.
7. Weak graph N17280/K8/T4.
8. Low connected N23040/K10/T4.
9. Two intercept-plus-slope partitions N23040/K10/T4.
10. Three FE unbalanced N24960/K10/T4.
11. Low disconnected wide N55680/K102/T8.

The two wide fast cases improve **42.3% and 48.0% versus baseline**, and
**37.7% and 33.8% versus stock**. All five paired native and stock comparisons
support those gains. Case 0 improves 11.3% versus both medians; all pairs beat
stock but only three reach the full 10% margin. Cases 0–6 have no native paired
loss (case 2 ties once). The weak median is unchanged and has two paired losses,
including one 5 ms loss. Low, slope and three-FE controls retain their algorithms;
case 10's median rises 1 ms and has three paired losses. Case 11 has three native
paired losses despite a lower median. Do not attribute unchanged-path timing
variation to the new algorithm.

Ten of twelve selected medians reach the 10% stock target; the narrow case still
costs 1.438 times stock, and small disconnected costs 1.200 times stock. All five
pairs lose to stock in each. This is a selected development screen, not a
package-wide win rate or universal thread/scale claim. Exact paired ratios and
losses are in `final-scientific-review.json`.

## Work and memory

Outer iteration counts do not change. In cases 5/6, reported inner units fall
27→12 and 50→14. These native report units are not equivalent fixest operation
counts. FE projection falls 110.50→38.86 and 258.42→120.40 ms; finite solving
28.83→19.45 and 71.39→49.45 ms; strict solving 128.85→32.71 and 561.93→126.00 ms.
Strict timings include nested work and must not be added to other phase totals.
Preparation, objective work, inference and publication remain visible costs.

Fast mode conservatively adds 16*N*min(poolsize, finite_columns) bytes to the
complete plan, with existing positive-dimension floors, without crediting its
old projector or dense capacity. Numeric factors and reductions are call-local;
one retained dense bank survives. Optional budget refusal retains the certified
prior route. Low mode's allowance is unchanged.

Fresh fast modeled peaks increase by 1,105,920–8,601,600 bytes. Tracked peaks fall
by 43,522,505 bytes in case 5 and 120,566,983 in case 6, to 101,622,932 and 218,406,528
bytes respectively. Case 7's tracked peak rises 138,240 bytes while its median is
unchanged; case 2's is unchanged. The other changed cases' tracked peaks fall.
All excluded paths' modeled and tracked peaks are unchanged. These are allocator
and planner measures, not RSS, physical-limit or static-inventory qualification.

## Independent validation and visible failures

- **39 distinct focused Rust tests** pass. Weighted WLS truth, nonzero residuals,
  direct Gram identity, disconnected components, extreme scales, changed weights,
  retained/streamed/tiled dense identity, misaligned output rejection and exact
  budget boundaries protect the builder. Session tests cover rank deficiency,
  admitted and refused fast storage, interruption after rewriting, strict retry,
  dropped candidate rollback and subsequent response-only projection. Existing
  current-weight final-Hessian, model-space, strict failure and protected engines
  remain covered.
- **Ten accepted licensed fixture runs** pass: six science fixtures and four
  real sixteen-iteration allocation audits (low/fast, T1/T4). Independent
  explicit Poisson/Hessian/sandwich/mean checks include unit, FW multiplicity,
  PW cluster, offsets, generated terms and saved outputs. Slopes and three-FE
  OLS have independent controls.
- **246 public states, 164 baseline pairs and 82 direct candidate/stock pairs**
  pass across development, fresh and bounded science screens. Six fits pass
  independent explicit Poisson, Hessian, robust sandwich and fitted-mean oracles.
  The two diagnostic screens add twelve independently checked public states.
- **24 OLS applications /12 output/science pairs** pass. Baseline/candidate
  medians are 17/15,28/28,101/101 and61/60 ms for one FE, two FE, multi-partition
  slopes and three FE. No OLS algorithm changes or broad performance claim.

The first expanded Rust test attempted an unsupported NonConvergence injection;
it stopped in the test seam. It was corrected to the supported Interrupted
injection before building fastproducts1; the failed log remains. Two copied
licensed fixtures initially returned the old filename's completion sentinel.
Their computations returned 0, but the harness correctly refused acceptance.
The names were corrected and both fixtures rerun successfully; initial receipts
remain. No successful process alone is treated as fixture acceptance.

The original disconnected N6720/K28 oracle has 1232 FE columns, exceeding the
unchanged 1200-column explicit-oracle bound. Its original six-fit screen's public-state
and reference comparisons pass, but no independent explicit claim is made for
that case. Seed **202609397** was predeclared for smaller N5760/K10 and
N6000/K28 cases, which supply the six explicit-oracle fits. The resource decision
is preserved; no bound or scientific tolerance was loosened. Seeds 395/396/397
are now consumed and frozen.

Known canonical LSMR reporting assertion, static allocation inventory drift,
weighted constrained-fit and nonretaining-CG span failures, deferred Linux/SCC
and physical-memory qualification remain open. This milestone does not repair
or requalify them.

## Remaining work

Prioritize narrow and small disconnected two-FE whole-fit costs. The measured
narrow recurrence is about 0.25 ms, so further recurrence tuning cannot close a
roughly 14 ms public gap. Canonical setup/acceptance, ordinary boundary statistics,
reconstruction, family/objective scans and loading/publication remain candidates
for coherent pass and ownership redesign. Current-weight original-input score,
rank, model-space and final-Hessian witnesses must retain independent authority.
The disconnected case has more removable canonical and ordinary-boundary work;
its larger recurrence is only part of the loss. Do not reopen the rejected
residual extrapolation probe or tune gates to these frozen seeds.

## Receipts

The content-addressed archive and restoration script preserve original source
hashes, sanitized captured source, build recipes, diagnostics, raw per-call
records, full timing distributions, declarations, independent audits, fixture
protocols, failures and review. Data and binary payloads are omitted with hashes;
licensed logs are represented by safe protocol excerpts and hashes. Raw local
work root: `target/ppml-lifecycle-next-20260924`.

Verified archive: **10,298 logical receipts /2,524 payloads /24,312,633 bytes**.
SHA-256 `6dc62d6e6d166b5468c2ce62e01c5149ab1757ed8fbb3ecc9913e9a31d5ea771`.
Restoration verifies every logical payload and the container before writing.
All agent-owned processes are terminal.
