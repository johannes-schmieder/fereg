# One-FE pass and loading-overlap experiments

This bounded local diagnostic tests safe RHS initialization, concurrent source
moments, parallel finite-value validation, and double-buffered input transport.
Results and adopted/rejected boundaries are in the
[report](../reports/development/one-fe-passes-local-2026-09-19/README.md).
It uses the preceding one-FE profile's frozen generated data, unchanged command
worker, pinned fixest installation and fresh-process import audit. Import and a
small warm-up are outside timing; all estimator work is inside it.

## Entry points

Set `STATA_MP_PATH` to licensed local Stata/MP, select the recorded Rust 1.85.1
and Apple Command Line Tools in `PATH`/`DEVELOPER_DIR`, and use a Python with
NumPy, pandas and psutil. `R_LIBS_USER` must put the preceding profile's stock
fixest library first, followed by its existing jsonlite/digest dependencies.
No package installation or remote launch is performed here.

`ROOT` is a new ignored experiment directory; `PREVIOUS` is the preceding
one-FE profile root; `ACCESSOR` is the preceding input-accessor experiment.
Never overwrite a measured source, plugin or run. Stage separate minimal source
snapshots containing Cargo files, `.cargo`, `crates`, `plugin`, `stata`,
`scripts`, `tests/mock_spi` and an empty `dist`. The baseline uses the preceding
stock plugin. The report's patches apply to a formatted copy of that baseline;
`cargo fmt --all` normalizes it first. Each prototype is independent, except
`fastfinal` and `pipelinefit`, which relink the exact `final` Rust archive.
The complete source manifests and plugin hashes bind the measured artifacts.

```bash
python benchmarks/one_fe_passes/build.py ROOT direct overlap combined
python benchmarks/one_fe_passes/run.py ROOT PREVIOUS --part smoke
python benchmarks/one_fe_passes/run.py ROOT PREVIOUS --part screen
python benchmarks/one_fe_passes/run.py ROOT PREVIOUS --part trace
python benchmarks/one_fe_passes/pipeline.py ROOT/pipeline PREVIOUS
python benchmarks/one_fe_passes/build.py ROOT final
python benchmarks/one_fe_passes/stack.py ROOT ACCESSOR
python benchmarks/one_fe_passes/run.py ROOT PREVIOUS --part confirm \
  --variants baseline final fastfinal pipelinefit fixest
python benchmarks/one_fe_passes/run.py ROOT PREVIOUS --part pipeline-controls \
  --variants baseline fastfinal pipelinefit
```

`patch.py` creates the first direct/overlap variants. The archived patches also
capture columnwise moments, stream initialization and validation variants.
`build.py` formats only isolated snapshots and uses release optimization, fat
LTO, one codegen unit, unwind containment and the actual-plugin ABI test.
`stack.py` requires the still-identical final static archive and records every
relink. Neither changes the repository's installed `dist` plugin.

`--label` prevents collisions between rounds. `screen` uses three rotated wide
blocks; `confirm` uses five. `trace` enables native timers without Stata/R stack
sampling. Warm-up phase lines precede measured phase lines; use the final line
of each phase. `holdout` adds narrow, single-thread and shuffled controls;
`pipeline-controls` deliberately excludes single-thread claims because the
experimental transport adapter creates its own helper thread.

The existing `benchmarks/stata_input_probe/fixtures.py` function runs the three
focused licensed fixtures for any named variant. The pipeline standalone guard
runs with UBSan in `pipeline.py`; `pipeline_bridge_guard_test.c` additionally
links to the experimental bridge with UBSan and checks its integrated batch
path. Mock callbacks assert caller-thread identity and inject mid-read failures.

`archive.py ROOT REPORT` rechecks all accepted outputs, archives only numeric
receipts, hashes and patches, and rejects missing fixture/test sentinels. Raw
licensed logs, data, binaries and source snapshots remain under ignored `ROOT`.
Run build/test activity and command measurements sequentially.

## Experimental pipeline boundary

The C prototype allocates two 4,096-row tiles and starts a joined pthread for
each eligible batch (8–64 numeric columns and at least 16,384 observations).
Only the Stata thread calls the SDK; the helper transposes completed native
copies. Worker startup, tile allocation, copy and final join are charged to
full-command time. The standalone kernel excludes allocation/page commitment,
but includes worker startup and join. Small tiles lose to synchronization.

This adapter is for feasibility measurement only. Its scratch/stack are outside
the Rust allocation ledger and its worker is outside `ThreadTeam`; it must not
be installed as the production loader. Production integration needs Rust-owned,
fallibly admitted tiles and a worker under the resolved thread policy, including
a true one-thread path. Guarded fast-accessor limits from the preceding report
still apply. The only adopted source change is parallel finite-value validation.
