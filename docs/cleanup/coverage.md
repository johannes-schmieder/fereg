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
M1.1 must reconcile each source document; M1.3/M3–M6 must execute the named
families before deletion is allowed. The legacy path inventory preserves every
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
- Copied benchmark sources retain their bytes for M1.4 porting; their old
  controllers are not new campaign launchers. The original 100-digit fixtures
  and scientific receipts remain untouched.
