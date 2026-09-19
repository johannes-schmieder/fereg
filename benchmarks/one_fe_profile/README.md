# One-FE OLS profiling

Bounded local development diagnostic for the complete `fereg` and `fixest::feols`
commands. The [September 19 report](../reports/development/one-fe-profile-local-2026-09-19/README.md)
contains measured results, identities, limitations, and optimization priorities.
This does not change an estimator, installed package, or frozen benchmark.

## Reproduction

Use a fresh output directory containing:

- `baseline/`: an exact source snapshot with an isolated release Mac plugin.
- `detailed/`: the same snapshot, patched by `instrument_fereg.py`, with its
  own build directory and plugin; retain release optimization/LTO and add symbols.
- `fixest.tar.gz`: the frozen archive from `benchmarks/systematic/references.json`.
- `fixest/`: the unmodified archive contents; `stock-src/fixest/`: a second
  extraction patched by `instrument.py`. These directory names follow the
  measured run; neither is an installed user library.
- `R-stock/` and `R-traced/`: separate libraries built from those two sources
  using identical compiler options and OpenMP support.
- `source-commit.txt`: the full source commit.

**Change into each source snapshot before invoking `scripts/build_macos.sh`.**
That script's Cargo invocation uses the caller's working directory. Use a
different `CARGO_TARGET_DIR` for every source/build variant and verify the
actual plugin hash and diagnostic timer signatures. Never replace the installed
plugin. The measured diagnostic build used `CARGO_PROFILE_RELEASE_STRIP=false`
and `CARGO_PROFILE_RELEASE_DEBUG=1`, preserving release optimization and fat LTO.

On a Mac with separately installed Command Line Tools, a process-local
`DEVELOPER_DIR` can select that toolchain without changing the system setting.
The measured R build also needed `CPLUS_INCLUDE_PATH` pointing to that SDK's
`usr/include/c++/v1`. Record the actual compiler, SDK and R identities.

Use Python with NumPy, Pandas, SciPy and psutil. Set `STATA_MP_PATH` to the
licensed Stata executable, put the intended `Rscript` on PATH, and supply the
isolated R dependency library (digest, jsonlite, Rcpp and fixest dependencies).
Run sequentially, after all compilation has finished:

```sh
python benchmarks/one_fe_profile/run.py ROOT --part smoke --r-dependencies R_DEPS
python benchmarks/one_fe_profile/run.py ROOT --part primary --r-dependencies R_DEPS
python benchmarks/one_fe_profile/run.py ROOT --part deep --r-dependencies R_DEPS
python benchmarks/one_fe_profile/run.py ROOT --part outer --r-dependencies R_DEPS
python benchmarks/one_fe_profile/run.py ROOT --part sample --r-dependencies R_DEPS
python benchmarks/one_fe_profile/run.py ROOT --part shape --r-dependencies R_DEPS
python benchmarks/one_fe_profile/instrument_fereg.py ROOT/detailed --ado-only
python benchmarks/one_fe_profile/run.py ROOT --part publication --r-dependencies R_DEPS
```

`smoke` checks four calls against an independent explicit-dummy least-squares
oracle. `primary` runs three input sizes/widths at 1/4/16 threads: three stock
pairs with reversed order, then two traced fits per package. `shape` adds fewer
groups and shuffled rows at 4/16 threads. No samples, tolerances, rank policies,
VCE corrections or outputs are relaxed. Inputs are loaded and their numerical
bytes checked before timing; the basic fit has no optional saved effects.

`deep` adds allocation/read/preparation/finite-solve laps. `outer` uses Stata's
program profiler. `sample` samples all process threads during eight repeated
commands; its timings are never performance results, and stack counts are
not wall-time percentages. `publication` exports timer values after the quiet
command, separating preparation, plugin, validation/naming, `ereturn post`,
and remaining publication. These diagnostics check full native outputs against
the stock artifact. Unavailable/empty stack samples remain explicitly unavailable.

`collect.py ROOT OUTPUT` writes numeric results and median paired ratios.
`archive.py ROOT OUTPUT` additionally requires all 152 intended calls, checks
scientific outputs, and writes sanitized identities, patches, diagnostic
summaries and hashes. For that exact archival layout it also expects
`environment.json`, `cpu.txt`, and the preserved controller snapshots
`controller-primary/`, `controller-followup/`, and `controller/`. A new run
should preserve its own actual controller identities rather than imitate the
historical snapshots. Keep raw licensed logs, datasets, binaries and local
machine paths outside the report.

The source patches are diagnostic instruments only. The source archive hash
is checked before extraction in the measured run. Instrumentation requires
exactly one matching source anchor and fails on unexpected reference changes.
