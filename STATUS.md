# fereg 0.5.0 beta status

**Last updated:** 2026-08-25
**Development branch:** `main`; pushed through the verified solver candidate
**Verified candidate source:** `786425f2a00049901e6a84877f486a0058aea471`
**Active milestone:** make default OLS consistently faster than `reghdfe` on macOS
**Release state:** public OLS beta implemented; primary macOS solver gates and exact-source local full profile green; release qualification remains incomplete

`STATUS.md` is the mutable project-progress record. The next product target is
a candidly labeled 0.5.0 beta. ADR-0007 and ADR-0008 remain the governing
scientific, solver, evidence, and safety contracts. Historical receipts remain
immutable even when they describe earlier hidden or fail-closed checkpoints.

## Current accepted state

- Public `fereg` implements the documented OLS surface. The frozen 40-case
  semantic inventory is executable: multiple slope-only terms, string and
  interacted FEs, lag/lead/difference materialization, iweights, continuous
  DoF, saved outputs, prediction, `test`, and `estat summarize` are no longer
  blockers.
- The Stata public path retains one substantive plugin call. Rust owns sample
  construction and computation; SPI access stays on the Stata thread; the
  completion receipt is stored last; and failure/Break tests require rollback
  of `e()`, prior estimates, and requested output variables.
- `memory(fast|low|auto)`, scratch-backed low-memory execution, allocation
  preflight, bounded cleanup, allocation/RSS reporting, `timeit`, thread
  reporting, and deterministic per-action Rayon pools are implemented.
- Public solver controls and action/result wire 1.3 are implemented. Explicit
  MAP, genuine batched weighted CG over the symmetric-Kaczmarz operator,
  IT/SQUAREM diagnostics, and LSMR paths expose diagnostics and require an
  independent final certificate. Categorical CG borrows the prepared topology;
  slope-bearing designs use the shared owned operator. Finite nonpositive CG
  curvature is restarted safely before publication and nonfinite curvature
  fails closed. CG now requires both reghdfe's Hestenes energy stop and the
  independent FE certificate; neither recurrence nor certificate substitutes
  for the other on the comparator-equivalent path.
- The frozen automatic policy is implemented: one FE uses direct one-pass
  demeaning; multiway designs run at most four MAP pilot sweeps and return MAP
  if certified. Otherwise auto selects LSMR only when pilot contraction is at
  least 0.008 and total FE coefficients are at most 65,536; all other cases
  continue with genuine CG. An auto LSMR failure has one visible fallback to
  certified CG. Pilot iterations, contraction, selected solver, iteration
  count, fallback, native time, and effective thread count are reported.
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
stronger ADR-0008 performance targets remain development goals and any case
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

## Active checkpoint: freeze the green macOS solver candidate

The comparator-equivalent MAP/CG milestone and the bounded auto policy are now
scientifically green and faster than reghdfe on the registered development
matrix. The deterministic AKM harness derives generated fixtures from
`tests/manual/simple_AKM.do` without modifying that playground, hashes each
dataset, separates parity preflights from timing, and reports cold/warm
medians, MAD, dependency and plugin identities, scientific outputs, solver
routing, pilot diagnostics, fallback, native time, and gregress distance.

The exact-plugin 100,000-row phase matrix produced 148 certified timing
records and ten passing parity preflights. Only explicit IT failed, on the
high-mobility case at both thread counts; both failures returned `r(459)` and
published no estimate. Auto was faster than reghdfe in every registered case:

| Case | Threads | reghdfe | fereg auto | auto/reghdfe | Used solver |
|---|---:|---:|---:|---:|---:|
| one FE | 1 | 0.044 s | 0.039 s | 0.886 | MAP |
| one FE | 8 | 0.029 s | 0.028 s | 0.966 | MAP |
| IID two FE | 1/8 | 0.269/0.210 s | 0.109/0.071 s | 0.405/0.338 | CG |
| high-mobility AKM | 1/8 | 0.256/0.191 s | 0.112/0.070 s | 0.438/0.366 | CG |
| standard AKM | 1/8 | 0.261/0.164 s | 0.159/0.087 s | 0.609/0.530 | CG |
| low-mobility AKM | 1/8 | 0.459/0.234 s | 0.130/0.080 s | 0.283/0.342 | LSMR |

The selected one-million-row acceptance milestone has 24 successful timed
runs and eight passing preflights. Auto is faster in every case. Ratios are
0.951/0.930 for one FE, 0.472/0.333 for IID, 0.651/0.481 for standard AKM,
and 0.717/0.532 for low mobility at one/eight threads. The 1m low-mobility
coefficient count exceeds the frozen LSMR cap, so auto correctly uses CG.

The selected ten-million-row freeze has 12 successful timed arms, four passing
preflights, one cold and five warm runs per arm, and no failures:

| Case | Threads | reghdfe median | fereg auto median | auto/reghdfe | gregress CG |
|---|---:|---:|---:|---:|---:|
| one FE | 8 | 1.968 s | 1.773 s | 0.901 | 0.752 s |
| standard AKM | 1 | 28.986 s | 20.061 s | 0.692 | 7.286 s |
| standard AKM | 8 | 17.682 s | 7.521 s | 0.425 | 7.007 s |
| low-mobility AKM | 8 | 37.011 s | 13.254 s | 0.358 | 30.341 s |

The initial 10m low-mobility candidate matched coefficients, VCE, DoF, sample,
RSS, and fit but missed the frozen residual gate. The cause was algorithmic:
CG returned when the FE certificate passed before reghdfe's Hestenes energy
criterion. Requiring both reduced the maximum residual gap from 1.405e-6 to
2.844e-7, below the roughly 5.33e-7 frozen global gate, and improved rather
than harmed the measured auto runtime margin against reghdfe. Tightening
tolerance alone and removing the four-sweep pilot did not solve the weak-graph
path dependence.

## Current blockers

1. The committed candidate still needs the remaining release-qualification stack:
   clean install, packaging/staging reproducibility, release archive hashes,
   ADR-0008 RSS/leak/constrained-memory evidence, and the final immutable
   release receipts. The exact-source local full profile is green but is not by
   itself a release receipt.
2. Explicit raw-start `solver(cg)` fails closed on the 1m low-mobility
   diagnostic because its refined residual fails the independent OLS score
   certificate. Auto's four-sweep pilot passes and is the beta acceptance path;
   the explicit escape hatch must remain documented as diagnostic until that
   raw-start score interaction is repaired.
3. Explicit IT remains quarantined after the two registered high-mobility
   `r(459)` failures. It may not influence auto or publish uncertified output.
4. The current 10m freeze is deliberately selected rather than the full
   eight-case freeze manifest. The remaining 10m cases and stronger ADR-0008
   aggregate performance thresholds are still release-qualification work.
5. Rosetta, SCC/Linux, hosted Linux/Windows, and licensed Windows evidence is
   not refreshed for this candidate. No tag or publication is permitted yet.

## Development plan

1. Freeze the current auto thresholds, Hestenes-plus-certificate rule,
   deterministic reduction order, and candidate anchors; make no further
   solver tuning without restarting quick/matrix/milestone evidence.
2. Add a regression fixture for the 10m-style weak-graph stop using a bounded
   synthetic analogue, including the requirement that recurrence and FE/score
   certificates all pass. Keep the full generated 10m case in qualification.
3. Diagnose explicit raw-start CG's low-mobility score failure separately from
   auto. Any repair must preserve the four-sweep auto results and cannot weaken
   FE or score certificates.
4. Run the remaining ADR-0008 RSS, leak, constrained-memory, and approved 10m
   cases, then complete deterministic package/clean-install/binary checks.
5. Materialize immutable exact-source licensed receipts only after the complete
   candidate passes. Then commit and push the whole verified stack together.
6. Resume PPML development only after the OLS beta checkpoint is complete.
