# Bounded current-source performance work

This harness creates fresh development screens; it never resumes a stopped
campaign or edits frozen manifests. Use a new output directory on every run.
Build isolated artifacts with `benchmarks/one_fe_optimization/build.py` and
retain their identity JSON. `run.py` verifies the artifact source/plugin hashes
and the stock fixest archive pin before fitting. The caller supplies the pinned
R library and licensed Stata executable through `R_LIBS_USER` and
`STATA_MP_PATH`; it must not substitute an instrumented reference library.

`run.py ROOT --artifacts ARTIFACTS --variants BASE CANDIDATE fixest
--fixest-archive ARCHIVE --cells CELLS --seed SEED --repeats 3` creates all data
before timing, captures workers/generator, rotates variants, checks imported
double values, records application sentinels, and samples competing processes.
Do not compile, profile, archive, run oracles or launch other fits during a
headline screen. A `complete.json` means all attempts were collected, not that
science passed. Failed, nonconverged or overlapping calls remain visible.

The stock worker supports numeric regressors, unweighted OLS IID and unweighted
PPML robust inference, without saved outputs. PPML covariance is HC0 times
N/(N−1). Other options are refused when `fixest` is selected. Native-only
controls can use the existing Stata worker's weights, VCE, absorb and fit options.
No comparison of ordinary fixest weights to Stata FW inference is provided.
Additive topology controls are implemented in `designs.py`; the systematic
generator and frozen definitions stay unchanged.

The additive `--surface` worker supports unweighted OLS numeric columns, factor
main effects and categorical interactions, with IID or robust inference. It
exports every finite coefficient and covariance entry. `outputs: "all"` adds
one-FE normalized saved effects, residuals, xb and xbd; fit and prediction
costs are recorded separately and their sum is the matched command timer.
The `product` probe records the public continuous-product rejection, not a
supported performance case. `surface_audit.py ROOT OUTPUT --independent` adds
bounded explicit-dummy inference and normalized-output checks; omit the flag
for large paired comparisons. Original failed attempts must remain visible.
`memory: "low"` requests native scratch-backed OLS; the default `"fast"`
keeps the public OLS default. Stock fixest uses its ordinary memory strategy,
so these timings do not establish performance under an equal memory budget.

`terms: "time_series"` replaces each numeric regressor with the first lag,
lead or difference selected cyclically by `ts_operators` (`L`, `F`, `D`).
Thirty-row panels have integer time; `time_gaps: true` inserts two gaps.
Stata `xtset` and stock fixest `panel(..., time.step=1)` run before timing;
the public estimators generate their terms inside the timer. The independent
oracle uses original-row keyed lookups before availability and singleton
deletion. Saved-output comparisons certify retained observation IDs before
aligning Stata's physical-row export with fixest's retained-row output.

`slope_partitions: {"g1": ["z1"], "g2": ["z2", "z3"]}` adds intercept-plus-
slope channels to declared partitions. This surface currently requires numeric
finite regressors and no saved outputs. Both estimators receive matching
varying-slope specifications; bounded pilots use explicit indicator-by-slope
columns and an independent matrix-rank calculation. It does not change DoF
policy or adjudicate large disconnected rank discrepancies.

Surface metadata lives in each run's captured `surface-args.do`, avoiding
Stata startup argument limits for wide generated models. The regular fit
options still use the existing worker. Run `test_surface.py` for the keyed
operator, slope-span and wide-metadata boundary checks.

`factor_rebase_probe.do` is a diagnostic reproducer for the ordinary one-FE
factor-level deletion defect. It verifies that candidate metadata cannot
distinguish `i.f` from `ib1.f`, checks their different final-sample expansion,
and records native outcomes and rollback. Its terminal sentinel confirms the
probe completed, not that the underlying rebasing defect is repaired.

`ado_snapshot.py ARTIFACTS BASE CANDIDATE --overlay stata/FILE.ado` captures an
ado-only change and verifies/reuses the baseline native plugin. `--profile`
adds diagnostic Stata boundary and final-expansion timers to a separate copy.
Its identity records the parent receipt and all captured source hashes; a
reused binary is not described as a native rebuild. Execute newly added
fixtures separately and preserve their exact hashes alongside that identity.

Run `audit.py ROOT OUTPUT` for export comparisons and descriptive timings,
`summarize.py ROOT OUTPUT BASE CANDIDATE` for paired ratios/repeats/native peaks,
and `oracles.py ROOT OUTPUT` for bounded independent explicit designs. Oracles
must run separately from timed fits, with single-thread numerical libraries.
They refuse unsupported options and oversized inputs. Reference agreement
alone does not establish science; unresolved discrepancies remain unranked.
PPML qualification still requires its existing comparator-adjudication/F0
contracts, beyond this bounded development harness.

Use `--trace` only in a separate diagnostic root. Additional transport/session
laps are injected exclusively into isolated builds by `profile.py` and the
build script's diagnostic flags. Capture those source deltas; their clocks
must never enter stock-reference headline medians.
`--profile-owned-fe` separates generic owned-FE projections and fresh
certification, including normalization, QR, span projection and raw moments.
Per-level clocks are diagnostic overhead, not headline performance evidence.

The isolated `--threaded-lsmr-window` build option reproduces the rejected
medium-batch scheduling experiment in `threaded_lsmr_window.py`. It modifies
only the captured build's transport/admission policy, never the working native
source. Its mixed dense-case screen does not authorize production adoption or
another window-size search; see the three-FE window report dated 2026-09-22.

`archive.py ROOT OUTPUT.zip --screens ... --extras ...` seals numerical exports,
plans, captured controllers and original/portable hashes, omitting data,
binaries and licensed log contents. Review the archive and attach source
overlays, independent checks and fixture receipts before documenting acceptance.
Keep current handover information only in `STATUS.md`.
