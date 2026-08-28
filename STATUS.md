# fereg 0.5.0 beta status

**Last updated:** 2026-08-27
**Development branch:** `main`; deterministic parallel saved-FE publication, 16-thread one-FE execution, and conservative topology-aware CMG auto routing are integrated alongside qualified direct Schwarz-LSMR
**Verified candidate plugin:** `c0f8802201356e4c12bb41b5ad281b79824587358966ac53847524b2e2f3f23e`
**Exploratory combined LSMR/CMG plugin:** `5ccc0cdf8841681d580043f821fa04d1d298c00bdb67d2d12148c96d5a4b8c18`
**Verified optimized-recovery source:** `4e78d015c49f659503aa9f761ca8a73473847387`
**Active milestone:** run the expanded macOS/Linux V2 matrix over 0/2/10/30 controls, one/two/three FEs, and requested native threads through 16 without weakening the certified ADR-0009 fallback
**Release state:** public OLS beta and the primary macOS direct-LSMR route are qualified; cross-platform and final release qualification remain incomplete

`STATUS.md` is the mutable project-progress record. The next product target is
a candidly labeled 0.5.0 beta. ADR-0009 supersedes ADR-0008's capped routing
and RSS rule while preserving its scientific and safety contracts. Historical receipts remain
immutable even when they describe earlier hidden or fail-closed checkpoints.

## Current accepted state

- Public `fereg` implements the documented OLS surface. The frozen 40-case
  semantic inventory is executable: multiple slope-only terms, string and
  interacted FEs, lag/lead/difference materialization, iweights, continuous
  DoF, saved outputs, prediction, `test`, and `estat summarize` are no longer
  blockers.
- The README, user guide, compatibility matrix, and installed help now describe
  the implemented public surface rather than the broader 1.0 target. The help
  contains twelve marked, self-contained examples. Examples 1--11 render and
  execute under local licensed Stata/MP 18. The fereg/reghdfe portion of the
  250,000-row benchmark passes sample, coefficient, and RSS assertions and its
  three repeated timing runs. The Stata 19 `areg, absorb(worker firm)
  dfabsorb` arm follows the official Stata 19 syntax but was not executed on
  this Stata 18 host; it is documentation validation, not qualification
  evidence.
- The Stata public path retains one substantive plugin call. Rust owns sample
  construction and computation; SPI access stays on the Stata thread; the
  completion receipt is stored last; and failure/Break tests require rollback
  of `e()`, prior estimates, and requested output variables.
- `memory(fast|low|auto)`, scratch-backed low-memory execution, allocation
  preflight, bounded cleanup, allocation/RSS reporting, `timeit`, thread
  reporting, and deterministic per-action Rayon pools are implemented.
- Public solver controls and action/result wire 1.4 are implemented. Explicit
  MAP, genuine batched weighted CG over the symmetric-Kaczmarz operator,
  IT/SQUAREM diagnostics, and LSMR paths expose diagnostics and require an
  independent final certificate. Categorical CG borrows the prepared topology;
  slope-bearing designs use the shared owned operator. Finite nonpositive CG
  curvature is restarted safely before publication and nonfinite curvature
  fails closed. CG now requires both reghdfe's Hestenes energy stop and the
  independent FE certificate; neither recurrence nor certificate substitutes
  for the other on the comparator-equivalent path.
- Explicit `solver(cmg)` reuses one graph hierarchy per fit, supports up to 64
  nuisance non-base absorbed-intercept columns, and retains certified CG
  polishing/fallback. ADR-0010 now selects CMG before the MAP pilot for the
  frozen favorable intercept-only topology: no nuisance columns, at least
  eight observations per highest-cardinality level, and a core cardinality
  ratio of at least 10:1. Fast and low modes share the route; setup/memory
  failure returns visibly to the existing auto policy, and solve failure uses
  certified CG thereafter. Graph PCG runs at one-hundredth of the public FE
  tolerance to satisfy the OLS score certificate without a separate post-QR
  correction. The GPL-3.0-only dependency remains a release-distribution
  blocker.
- The qualified automatic policy is implemented: one FE uses direct one-pass
  demeaning; multiway designs run at most four MAP pilot sweeps and return MAP
  if certified. Otherwise a pilot contraction of at least 0.008 selects direct
  Schwarz-LSMR at two decimal orders tighter than the requested tolerance;
  there is no coefficient cap. All other cases continue with genuine CG. An
  auto LSMR failure has one visible fallback from the saved pilot state to
  certified CG. The execution and route-specific solver workspaces must fit
  jointly before selection. Pilot iterations, contraction, selected solver,
  iteration count, fallback, native time, and effective thread count are
  reported.
- Fast-mode saved fixed effects no longer rerun observation-space MAP. LSMR and
  CMG retain their coefficient solutions, CG propagates compact coefficient
  states through its recurrence, Irons--Tuck mirrors accepted extrapolations in
  coefficient space, auto combines its four-sweep MAP pilot with the selected
  continuation, and intercept-only MAP now records coefficients during its
  original batched `y`/`X` sweeps. MAP therefore has no recovery solve in the
  normal fast saved-output path. Routes without a retained product use bounded
  reconstruction-aware CG, then a four-sweep forward-Kaczmarz/steepest-descent
  pilot plus LSMR on the remainder; slopes use strict LSMR. Every route still
  applies connected-component normalization plus independent reconstruction
  and FE-moment certificates at the unchanged publication tolerance.
- ADR-0011 makes retained-coefficient publication part of the bounded native
  thread team. Saved-only calls omit both the persistent `xb` vector and the
  combined absorbed vector; intercept-only coefficient products are scattered
  observation-major; and their reconstruction and FE moments use fixed,
  deterministic parallel chunks. The one-FE path evaluates the certificate
  directly from the target and level coefficients without allocating an
  observation-length recovery residual. Rust tests require bit-identical
  scientific results at 1/2/4/8/16 threads. The V2 Stata prototype passed both
  K=0/one-FE and K=30/worker-firm-year corners at requested 16 threads while
  recording the local Stata license cap of eight separately. No CMG upstream
  change was needed; ADR-0010 continues to govern any routing expansion.
- Warm local Stata/MP timing checkpoints for saved output are shown below.
  Unless specifically noted after the table, they are alternating-order
  medians. The 300,000-row runs use nine pairs; the 30,000-row CG/IT
  runs use nine pairs, MAP five, and SQUAREM three. Absolute overhead is small
  for retained paths; MAP remains the candid exception even with its best
  tested fallback.

  | Solver | Rows | No saved FE | Saved FE | Delta |
  |---|---:|---:|---:|---:|
  | auto | 300,000 | 0.321 s | 0.377 s | +0.056 s |
  | LSMR | 300,000 | 0.354 s | 0.365 s | +0.011 s |
  | CMG | 300,000 | 0.287 s | 0.331 s | +0.044 s |
  | CG | 30,000 | 0.093 s | 0.100 s | +0.007 s |
  | Irons--Tuck | 30,000 | 0.295 s | 0.310 s | +0.015 s |
  | MAP | 30,000 | 0.858 s | 0.887 s | +0.029 s |
  | MAP | 300,000 | 3.626 s | 3.727 s | +0.101 s |
  | SQUAREM | 30,000 | 2.553 s | 2.950 s | +0.397 s |

  The new MAP rows are alternating-order warm medians (15 pairs at 30,000 rows
  and nine pairs at 300,000 rows); the earlier solver rows retain their
  registered medians. At the original 30,000-row reproducer scale, saved MAP
  output now adds 3.4 percent, down from the earlier 44.9 and 5.6 percent
  checkpoints. At 300,000 rows it adds 2.8 percent, down from 21.2 percent; the
  paired-difference median is 0.098 seconds (2.7 percent of the no-save median).
  These timings include coefficient accumulation, strict normalization and
  certification, and writing two observation-scale Stata variables. The MAP
  path stores coefficients in the projection kernel's native level-major
  order, fuses their update with parallel group means, transposes once after
  certification, reuses exact connectedness from the DoF result, and avoids a
  second observation-scale buffer for saved-only publication. The original
  `tests/manual/simple_AKM.do` reproducer's saved AUTO
  and CMG runs were 0.129 s and 0.066 s. The expanded
  saved-output parity fixture passes MAP, CG, IT, SQUAREM, LSMR, and CMG with
  matching coefficients/RSS and explicit FE-channel reconstruction identities.
  The complete local entry point passes repository checks, 269 Python tests,
  strict formatting/Clippy, 318 core tests, 53 FFI plugin tests, integration
  oracles, mock SPI, and portable static checks.
- `feppml` remains installed but is an explicit `r(498)` stub. PPML behavior
  and performance are outside the 0.5.0 beta.
- The reference closure remains pinned to reghdfe 6.14.1 (`29b2b203...`),
  ftools 2.50.0 (`d6e24d66...`), and require 1.3.1 (`f4760b86...`).
- The normal Stata PLUS tree has the exact local comparator closure: reghdfe
  6.14.1, ftools 2.50.0 (compiled), and require 1.3.1. gtools/gregress 1.11.8
  is pinned as a diagnostic speed target with ado and macOS plugin hashes.
- Committed candidate `786425f2a00049901e6a84877f486a0058aea471`
  passes 293 core tests, 52 plugin
  tests, all Rust integration/doc tests, 258 Python tests, strict Clippy and
  formatting, repository checks, the universal macOS build/audit/actual-plugin
  ABI loader, and the licensed 32-fixture Stata surface after rebasing only the
  deterministic candidate anchors affected by the corrected CG stop. The
  exact-source local full-profile plugin SHA-256 is
  `d7c1182e782049b541fed2f34caf720ecd82d245e4e3a1b8fb67045d260dfa85`.
  The local full receipt records `status=success`, `stata_rc=0`, and the exact
  candidate SHA. This is exact-source local evidence; the final immutable
  release receipt and remaining platform/resource qualification are pending.
- The integrated ADR-0009 implementation adds a sealed 100-digit weak-graph oracle,
  an independent observation-space projection audit, direct/forced-refinement
  comparison arms, route-specific memory planning, and borrowed Schwarz-LSMR
  topology reuse. Its final universal macOS plugin passed the fresh-seed 1m
  holdout, untouched 10m freeze, all 32 licensed Stata fixtures, 269 Python
  tests, strict Rust/FFI/all-target suites, mock SPI, binary audit/loader, and
  deterministic packaging checks. The evidence summary is
  `benchmarks/expected/direct_lsmr_qualification_v1.json`.
- The ADR-0010 100,000-row crossover and guardrail calibration rejected short,
  nuisance-heavy, and nearly balanced graphs and froze only the robust region.
  The permanent manifest/analyzer enforce a five-percent per-selected-cell
  slowdown ceiling and a faster selected aggregate. The upstream CMG pin now
  exposes conservative pre-build and exact retained-memory reports consumed by
  fereg's joint memory planner.

## 0.5.0 beta exit gate

The beta is not ready until default `solver(auto)` is consistently faster than
the pinned `reghdfe` on representative standard OLS problems on the primary
macOS Apple-Silicon/Stata 18 platform. A comparison counts only after matching
the estimation sample, singleton policy, coefficients and omissions, VCE, DoF,
requested outputs, tolerance, and effective thread count.

For the beta matrix, use warm medians with the registered repetition policy;
include dense and sparse two-way FEs, a worker-firm mobility design, absorbed
slopes, clustered inference, moderate regressor counts, and one- and
eight-thread runs where meaningful. Default `solver(auto)` must be faster than
`reghdfe` in every registered standard case, not merely faster on average. The
stronger ADR-0009 performance targets remain development goals and any case
that does not meet them must be reported candidly. The selected route and any
fallback must be visible in diagnostics and backed by the same final scientific
certificates.

The exact beta source SHA must also pass the complete local repository checks,
licensed macOS quick/full profiles, native Apple-Silicon clean-install and
binary audits, repeated-call/rollback checks, and deterministic beta archive
staging. Rosetta, SCC/Linux, hosted Linux/Windows, and licensed Windows evidence
remain important qualification work, but they do not replace the primary
macOS speed and correctness gate.

## Completed, evidence-backed baseline

- Four sealed MAP cases at committed source
  `df53822b3263d7425a9a2a42b16b58f7b306dc91` cover dense and sparse two-FE
  designs at one and eight threads. Speedups over the pinned reference are
  2.41x, 2.52x, 2.14x, and 2.40x. Their exact receipts are preserved under
  `build_evidence/local/benchmarks/map-baseline-df53822/`.

- At committed source `a0cc6ecb4aecc6e33bb668c9857ce920f8a5e5f9`, the isolated
  licensed Stata/MP 18 Apple-Silicon quick lane passed 30/30. Its tested plugin
  SHA-256 was
  `10902ebea72314511c7ac03f830486c94fe13a5579da9e0cb260c3fe21547340`.
- The same implementation milestone passed repository checks, 232 Python
  tests, strict C, mock SPI, `cargo fmt`, Clippy with warnings denied, all-target
  Rust tests, and FFI tests. Full workspace counts were 278 core and 50 plugin
  plus integration tests.
- Licensed absorbed-slope checks passed IID, robust, aweight, cluster,
  `dof(none)`, and `dof(firstpair)`. The pure all-slope-only model now matches
  the pinned comparator's structural no-constant result.
- A constrained-memory local fixture passed with a 539,500,000-byte limit:
  fast mode returned `r(909)`, low mode succeeded, and auto selected low.
- The earlier accepted exact quick/full and captured hidden receipts remain
  useful historical evidence, but they do not qualify the current public
  source.

## Active checkpoint: direct Schwarz-LSMR qualified for auto

Calibration evaluated requested and 100-times-tighter direct LSMR,
off/diagonal/Schwarz preconditioners, forced restarted CG correction at 4/8/16
iterations, and the prior coarse-LSMR/ordinary-CG hybrid. The frozen survivor
is direct Schwarz-LSMR at `max(1e-14, tolerance * 1e-2)`, without CG refinement.
The selection, tolerance, preconditioner, pilot threshold, and diagnostics were
frozen before fresh-seed holdouts.

The full 100,000-row matrix produced 148 timing records and ten green
scientific preflights. Only explicit IT failed on the high-mobility case at one
and eight threads; both returned `r(459)` and published no estimate.

The final-hash one-million-row holdout and untouched ten-million-row freeze
passed every hard scientific, stability, runtime, native-allocation, and leak
gate:

| Case | Threads | 1m candidate/old CG | 10m candidate/old CG | Auto route |
|---|---:|---:|---:|---:|
| standard AKM | 1 | 0.986 | 0.973 | CG |
| standard AKM | 8 | 1.033 | 0.996 | CG |
| low-mobility AKM | 1 | 0.262 | 0.231 | direct Schwarz-LSMR |
| low-mobility AKM | 8 | 0.344 | 0.493 | direct Schwarz-LSMR |

At 10m, routed allocation-model errors were 0.4 percent and 20.3 percent at
one/eight threads. Repeated-call leak gates passed. The reghdfe residual
distance was 1.68e-6/1.73e-6 and is report-only under ADR 0009; the independent
100-times-tighter stability distance was 1.19e-7 against a 5.33e-7 envelope,
and all FE/OLS score certificates and comparator RSS/fit contracts passed.

The joint memory planner selects CG when direct LSMR does not fit and CG does,
and fails with `r(909)` before large allocations when neither route fits. The
old process-RSS ratio remains diagnostic; it is deliberately not a promotion
blocker when modeled allocation, cleanup, and science pass.

## Current blockers

1. The integrated ADR-0009/ADR-0010 route still needs a clean exact-commit
   rebuild, CI, fresh 1m/10m CMG holdouts, the broader release matrix, and
   immutable release receipts.
2. Explicit raw-start `solver(cg)` fails closed on the 1m low-mobility
   diagnostic because its refined residual fails the independent OLS score
   certificate. Auto's four-sweep pilot passes and is the beta acceptance path;
   the explicit escape hatch must remain documented as diagnostic until that
   raw-start score interaction is repaired.
3. Explicit IT remains quarantined after the two registered high-mobility
   `r(459)` failures. It may not influence auto or publish uncertified output.
4. The direct-LSMR qualification covers the registered standard and
   low-mobility 1m/10m cases. The broader slopes, clustered inference, and
   moderate-K release matrix still must be rerun on the eventual integration
   commit even though the existing full Rust/Stata surface is green.
5. Rosetta, SCC/Linux, hosted Linux/Windows, and licensed Windows evidence is
   not refreshed for this candidate. No tag or publication is permitted yet.

## Development plan

1. Preserve the frozen ADR-0009 fallback and ADR-0010 topology tree. Run the
   permanent CMG matrix and analyzer on fresh 1m holdouts and selected 10m
   freeze cases before expanding the route or making release claims.
2. Preserve the sealed Decimal fixtures, observation-space audit, large-case
   tighter-solve stability check, and full generated 10m case in qualification.
3. Diagnose explicit raw-start CG's low-mobility score failure separately from
   auto. Any repair must preserve the four-sweep auto results and cannot weaken
   FE or score certificates.
4. Run the broader release matrix on the integration commit, then complete
   clean-install, cross-platform, and final archive checks. Treat process RSS
   as diagnostic and route-specific modeled allocation/leak/preflight as hard.
5. Materialize immutable exact-commit licensed receipts only after that stack
   passes. Then push the complete verified checkpoint together.
6. Resume PPML development only after the OLS beta checkpoint is complete.
