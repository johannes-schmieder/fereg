> Historical rebuild coverage mapping. The incremental [M0–M1 plan](PLAN.md)
> supersedes its architecture, deletion schedule and cross-thread bit policy.
> M0.4 replaces this mapping with current executable coverage. No old pending
> replacement below is a passing gate; scientific source fixtures remain.

# Scientific requirements and replacement coverage

This mapping carries legacy requirements into SPEC and executable new-engine
checks. It is not a progress record; completion and evidence are tracked in
MILESTONES.md. No source requirement, test or receipt listed here may be deleted
until its replacement is checked under plan §0.9. Explicitly superseded public
policies must be identified rather than silently dropped.

## Historical scientific inputs

These are findings at the preserved source e61cb9ee, not new-engine claims.

- [Completed sweep](../../benchmarks/reports/development/current-source-sweep-completion-scc-2026-09-29/README.md):
  568/568 groups, 149 pair accuracy discrepancies and eight existing timeouts.
  Default-stock and strict-stock comparisons remain distinct. Thread-scaling
  observations require controlled confirmation; no missing pair is a pass.
- [Solver diagnostics](../../benchmarks/reports/development/scc-harness-repair-2026-09-29/README.md):
  CG core success did not establish public near-within oracle success; the
  public r(9) assertion remained unlocalized. Schwarz retained a canonical
  rank/reporting failure. Neither candidate was adopted. M0.8/M3 must retain
  independent regressions and distinguish accuracy from reporting assertions.
- [Anchored PPML recovery](../../benchmarks/reports/development/ppml-anchored-recovery-2026-09-28/README.md):
  preserve component anchors and the row reconstruction certificate. Original
  capture/repair identities stay attached to their source, not the new engine.
- [Independent-unit support](../../benchmarks/reports/development/ppml-bias-support-2026-09-28/validation.json):
  retain PPML mean bias correction, both second-order terms, separate corrected
  outputs, support screening and the distinction between probe variation and
  sampling uncertainty. See legacy ADRs 0227/0246 and ppml_bias_correction.md.
- The issue's b41f18e CG/CMG failures require reproduction at their stated
  source/design and adjudication on current code before selecting a repair.
  The old STATUS.md remains frozen; stopped sibling empirical campaigns are
  outside this rebuild.

## Migration boundary

The new public solver set is auto/lsmr/cmg. The legacy fast/low memory split,
extra solver/accelerator options and optional determinism are explicitly
superseded by staged ADRs 0002, 0004, 0005 and 0006. Their scientific assertions
about samples, outputs, certificates and rollback remain applicable; obsolete
route counts and byte constants are not automatically new-engine contracts.

## Contract-to-test mapping

The SPEC section numbers below are the twenty required sections in plan M1.1.
Replacement descriptions are requirements, not assertions of passing coverage.
The source reconciliation below defines the migration; M1.3/M3–M6 must execute
the named replacement families before deletion is allowed. The legacy path inventory preserves every
tracked source/test/harness path and Git blob identity at the rebuild snapshot.

| Requirement | SPEC | Existing executable evidence | Required replacement |
|---|---|---|---|
| Syntax, factor/TS terms and public option rejection | 1, 6 | tests/stata/parser/; model_wire_roundtrip/adversarial core tests | ModelSpec parser round trips, adversarial wires, Stata parser parity |
| Candidate rows, missingness, cluster availability and sample order | 2 | numeric_sample_oracle; categorical/prepared preparation testkit tests | Exact legacy/new row, mask and exclusion-reason property comparisons |
| AW/FW/PW objective scales; IW refusal | 3 | numeric_sample_oracle; weighted OLS testkit and Stata parity fixtures | Independent weighted dummy OLS/Poisson and frequency-replication checks |
| Singleton fixed point and separation interplay | 4, 14 | core fe/singleton.rs; ppml_separation tests; Stata PPML parity | Exact deletion ledger, multiplicity and post-separation fixed point |
| Intercept/slope/interacted FE spans and local rank | 5, 8 | intercept_slope_oracle; borrowed_map_oracle; fe_certificate_oracle | Forced-backend dense oracle with slopes, rank deficiency and rotations |
| OLS finite solve, collinearity and reported constant | 7, 8 | ols_oracle; prepared_fit_oracle; categorical_fit_oracle | Rank/status/bread and identified-coefficient dummy-design properties |
| Residual and absorbed DoF; nesting | 9 | core ols/dof.rs and topology_dof.rs; Stata parity fixtures | Exact legacy/new DoF and independent rank controls |
| IID, robust and multiway cluster covariance; PSD policy | 10 | prepared_robust/clustered_oracle; categorical_iid/robust/clustered_oracle; Python PPML cluster oracle | Independent sandwich checks, cluster subsets, finite-sample scaling, PSD controls |
| Fit statistics, stripes and e() layout | 11 | tests/stata/parity_reghdfe/ and parity_ppmlhdfe/; result wire tests | Native result validation and complete Stata result comparisons |
| Prediction, residuals, xbd, normalized saved FEs | 12 | tests/stata/postestimation/; tests/stata/bias/final_recovery_precision.do | Independent row reconstruction and anchors for every supported backend |
| PPML objective, offsets/exposure, starts and convergence | 13 | ppml_explicit_dummy_oracle.py; core PPML tests | Poisson oracle, score/outer acceptance, original-input reconstruction |
| FE/simplex/IR/Mu separation | 14 | ppml_recession_cone_oracle.py; core ppml_separation/ | Independent cone certificates, sample mutation order and bounded errors |
| Mean bias correction and independent-unit support | 15 | tests/stata/bias/; test_ppml_bias_oracle.py; bias_support.rs tests | Exact/randomized full-information correction, both increments, support refusals, atomic outputs |
| Projection membership and normal equations | 16, 20 | fe_certificate_oracle; weak_graph_decimal_oracle | Explicit coefficient reconstruction plus independently scaled span/score checks |
| Weak/ill-conditioned graph accuracy and CMG defect | 16, 20 | 100-digit weak_graph_decimal_oracle_v1 data; prepared_fit_oracle CMG regression | Every supported backend versus high-precision truth; retain direct CMG regression |
| Iteration budget, explicit requests and visible fallback | 1, 16 | FE solver tests; PPML solver-report fixtures | Forced auto/LSMR/CMG dispatch, exhaustion, unsupported errors and diagnostic identity |
| Failure atomicity, panic and Break | 17 | tests/mock_spi/; tests/licensed_stata/; transaction fixtures | Main-thread SPI, receipt-last, injected-store/panic/Break rollback |
| Same-machine cross-thread determinism | 18 | legacy threading tests; new plan ADR 0005 | Exact outputs across thread counts for all backends and dependency adapters |
| Whole-call sizing and fallible allocation | 19 | constrained-memory core tests; plugin allocator audits | Checked arithmetic, early r(909), allocation-failure rollback under one mode |
| Reference pins and independent tolerances | 20 | references.lock.toml; tests/python reference/weak-graph oracles | Frozen reference identities plus independent tests at unchanged scientific tolerances |
| Packaging and installed public behavior | 1, 11, 17 | tests/stata/installation/ and documentation/; mock plugin loader | Clean install, help/examples, native ABI and source/notices packaging |

## Source reconciliation

The independent library is `crates/fereg-oracle`, with no production dependency
on legacy code. Its `ols`, `ppml` and `weak_graph` integration tests exercise the
dense oracles against hand calculations, invariances and the preserved 100-digit
fixtures. `designs` checks seeded graph/sample construction; `legacy` compares
rows, fitted values, residuals, identified coefficients and inference on the
legacy default routes, including direct CMG/LSMR and unsupported-CMG checks.
The temporary legacy dependency is dev-only. Run its longer 2,000-design suite
and the diagnostic CG counterexample with `cargo test -p fereg-oracle --profile
ci -- --include-ignored`. The Poisson oracle exposes the raw multiway sandwich;
PSD repair remains a separate production-inference requirement. These tests
provide comparison infrastructure; the new-engine replacements and cutover
gates in the table still require their own checks.

[SPEC](../SPEC.md) consolidates the following sources. "Superseded" below
means their stated target policy is replaced by the approved plan, not that
the source or its scientific tests can already be deleted. All sources remain
reachable in the working tree and the inventory. Numerical optimizations in
legacy documents are implementation history; their accepted sample, rank,
certificate, inference and output invariants survive in the listed sections.

| Source | Retained destination | Explicitly superseded or reconciled content |
|---|---|---|
| scientific_contracts.md | SPEC §§2–20; every independent-oracle and accuracy row above | Historical activation/qualification statements remain source-bound; old per-route memory/performance gate becomes PLAN §7, single memory mode and declared suite. Multi-FE starts become global plus analytic one-FE only under M6.1. |
| weight_semantics.md | §§2–4, 7, 9–11 | No scientific change; FW u32 domain, literal replication and AW/PW reference scales retained. |
| ols_core.md | §§5, 7–12, 16–19 | Fast/low duplication and size/thread route guards are replaced; automatic 1e-14 derived-output solve is superseded by PLAN §3.5. Positive DoF and score/inverse/covariance checks remain. |
| architecture.md (relocated to cleanup/legacy-architecture.md) | SPEC §§2–20; ARCHITECTURE.md | Preserved prose and repaired relative links avoid a case-only filename collision on macOS. Legacy source-reading checks follow the preserved copy; the new engine replaces old routes without changing scientific invariants. |
| data_plane.md | §§2–6, 12, 17–19 | Old scheduling/width thresholds and duplicate representations are not target routing rules; main-thread SPI, one row map, no ado design expansion and failure atomicity remain. |
| factor_protocol.md | §§2, 5–6, 17 | Its old action/result minor numbers are descriptive history; exact current table in plugin_protocol.md and source constants governs the migration seam. |
| plugin_protocol.md | §§11, 15–17, 19 | Current versions remain until seam changes; M7 replaces result layout with v2 and removes legacy memory/allocation/route rows. Scientific fields, COMPLETE-last and request framing survive. |
| compatibility_matrix.md | §§1–20 | Old auto envelopes, fast-only PPML explicit solvers, optional determinism, fixed route rows and release-status claims are superseded. Mathematical backend support and named limitations remain. Windows receives build/mock checks only. |
| user_guide.md | §§1–15, 18–20 | Old tuning options and route descriptions remain legacy-only until M7; retained syntax, examples, predictions and optional mean correction survive. |
| ppml_bias_correction.md | §15 | Both increments, support screen, anchors, Philox identity, exact/randomized results and diagnostics retained. Old route-dependent reproducibility becomes cross-worker identity. Cached-factor/memory-mode thresholds are implementation history, not new routing policy. |
| upstream_map.md | THIRD_PARTY_NOTICES.md | Provenance folded without deleting the source; within and CMG runtime roles distinguished from behavioral references. G-LICENSE remains separate. |
| ADR 0003 | §§1, 16–17 | Historical all-estimators-fail-closed staging was already superseded by activation; unsupported features still fail closed. |
| ADR 0004 | §17 | Main-thread-only SPI unchanged. |
| ADR 0005 | §§2, 6, 17 | Stata metadata ownership and narrow TS materialization retained; no broad dummy expansion. |
| ADR 0006 | §§7–10, 16, 20 | Positive DoF, no negative IID variance, 48-row success/24-row failure retained; obsolete activation checkpoint is historical. |
| ADR 0016 | §§1–3, 8–15, 20 | PPML sample, family, inference and output semantics retained; public activation already occurred; M6.1 explicitly simplifies starts only. |
| ADR 0017 | §§4, 14, 17, 19 | Certified proposals, generation ordering, mandatory IR and explicit simplex-cap behavior retained under single top-level memory planning. |
| ADR 0019 | §§11, 17, 20 | Additive PPML action and atomic transaction retained during seam; old stub/activation and receipt-campaign procedure superseded by PLAN cutover gates. No scientific OLS non-regression exemption. |
| ADR 0026 | §20 | Reported comparator objective and feasibility/stationarity requirements unchanged. |
| ADR 0055 | §17 | Inline/local crossover, fixed macro name, cap, exact NUL/UTF-8/truncation checks unchanged. |
| ADR 0182 | §17 | Metadata-only framing, byte identity and temporary cleanup retained; historical timing claims remain source-bound. |
| ADR 0201 | §17 | Bounded independent result validation, oldest-compiler object identity, missing-helper rollback retained if compiled helpers remain; M7 may regenerate or remove them. |
| ADR 0227 | §15 | Full-system correction, units, both increments, separate outputs and numerical uncertainty retained. |
| ADR 0246 | §15 | Two distinct units in every level of both FEs, support summaries and interpretation retained. |
| Other scientific ADR requirements referenced by these sources | Relevant SPEC sections and mapping rows above | Only implementation-specific routes, caches, layout thresholds and historical acceptance procedures are superseded. Original ADRs are inventoried; M7 must review every proposed deletion against executable replacement coverage. |
| stata/fereg.sthlp; fereg_postestimation.sthlp | §§1–12, 16–20 | Removed options listed explicitly in §1; installed diagnostics, prediction prerequisites, test/summarize and runnable examples retained. |
| stata/feppml.sthlp | §§1–6, 8–20 | PPML options, solver support, residual aliases/formulas, all drop codes, statistical results and correction controls retained; fast/low and legacy solver rows removed only at M7. |
| Other stata/*.sthlp | §§1, 6, 17 and packaging row | Diagnostic/parser/example-runner help remains supported only in its documented role; hidden developer interfaces do not become public estimation options. |

The fallback budget in SPEC §1 instantiates PLAN §5's required finite budget:
at most one transfer to LSMR, no recursive/reset-budget attempt, and explicit
fatal classes. It is a target engine requirement, not a claim that the legacy
fallback code has that policy. M3 tests must enforce accounting independently.
The requested-tolerance output change does not waive tighter PPML accepted
Hessian checks or independent numerical tolerances. M7 must also verify final
wire-v2 field mapping and installed help before removing the corresponding
legacy transport fixtures. No replacement passes are implied by this table.

## Source inventory and commands

- [legacy-inputs.tsv](legacy-inputs.tsv) binds paths and content hashes to
  e61cb9ee. It includes docs/ADRs, native unit/integration tests, Stata/Python
  fixtures, build/CI/packaging scripts and original benchmark harnesses.
- Run the preserved comprehensive entry point with
  `./scripts/run_legacy_tests.sh` (PowerShell mirror available). Its historical
  layout/allocation-source checks may report drift; do not confuse that with
  estimator accuracy or weaken scientific assertions to satisfy path checks.
- Run individual Python families with `python3 -m unittest discover -s
  tests/python -p 'test_<family>.py' -v`; native integration oracles with
  `cargo test -p fereg-testkit --test <name>`; Stata profiles with
  `./ci/run_stata_ci.sh <profile>` as listed in ci/stata_profiles.json.
- The lean `./scripts/run_tests.sh` runs Rust and mock-SPI checks. Larger
  property/legacy and licensed checks are additional, not replaced by linting.
- The legacy crates now live under `legacy/`, with their source and tests
  unchanged. Current path readers follow that relocation; source-bound Git
  pins, historical manifests and `legacy-inputs.tsv` retain their original
  paths and hashes. Run old campaign controllers at their recorded revisions
  when reproducing those historical artifacts.
- Copied benchmark sources retain their bytes for M1.4 porting; their old
  controllers are not new campaign launchers. The original 100-digit fixtures
  and scientific receipts remain untouched.
