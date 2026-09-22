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

`archive.py ROOT OUTPUT.zip --screens ... --extras ...` seals numerical exports,
plans, captured controllers and original/portable hashes, omitting data,
binaries and licensed log contents. Review the archive and attach source
overlays, independent checks and fixture receipts before documenting acceptance.
Keep current handover information only in `STATUS.md`.
