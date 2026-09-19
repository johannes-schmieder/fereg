# Stata numeric accessor experiment

This is a local development investigation of `safevdata` versus `vdata`,
callback caching, and destination layout. It makes isolated arm64 plugin
variants; it does not modify the production estimator or installed package.

The [report](../reports/development/stata-input-accessor-local-2026-09-19/README.md)
records results, scientific checks and limits. Raw licensed logs, plugins and
data stay under an ignored local experiment root.

## Reproduce

The preceding [one-FE profile](../one_fe_profile/README.md) supplies `PREVIOUS`:
its exact `baseline/` snapshot, `build-baseline/aarch64-apple-darwin/release/`
Rust archive, source identity, and generated `.dta`/`.json` datasets. Use a new
`ROOT`, Python with NumPy/Pandas/psutil, and the licensed Mac Stata executable
in `STATA_MP_PATH`. `build.py` selects the installed Command Line Tools through
a process-local setting. No Rust compilation or global toolchain change occurs.

Run these sequentially; never overlap benchmark runs with builds or tests:

```sh
python benchmarks/stata_input_probe/build.py ROOT PREVIOUS
python benchmarks/stata_input_probe/kernel.py ROOT PREVIOUS
python benchmarks/stata_input_probe/run.py ROOT PREVIOUS --part smoke
python benchmarks/stata_input_probe/run.py ROOT PREVIOUS --part primary
python benchmarks/stata_input_probe/run.py ROOT PREVIOUS --part holdout
python benchmarks/stata_input_probe/fixtures.py ROOT fast
python benchmarks/stata_input_probe/archive.py ROOT REPORT
```

Process enumeration for the reused measurement harness may require permission.
That check occurs before launching a fit. Existing experiment outputs are not
overwritten. `archive.py` requires all 36 command calls and checks exact native
scientific exports again. The report's explanatory README is authored separately.

`build.py` changes only the C numeric batch reader. `baseline` retains the
original loop. `cached` validates bounds/types/pointers once, caches the checked
callback, and removes the inner pointer test. `fast` is identical except for
the callback. All other accessors/stores retain their existing checked behavior.
The same Rust static archive and identical ado source serve every variant.
Actual-plugin ABI/panic tests run for all three; the mock initializes both SDK
callbacks. Separate guard tests under UBSan exercise invalid dimensions,
overflow, types/pointers, missing callbacks, thread refusal and error propagation.
These are diagnostic variants, not a supported user-selectable fast mode.

The kernel probe uses five modes: current-style checked column writes, cached
checked column writes, cached fast column writes, fast row writes, and fast
256-row tiles with conversion into columns. Allocation, page initialization
and output hashing are outside kernel timing. Mode 3 therefore produces a
different representation and is not an end-to-end transport improvement.
Every mode validates shape/types and checks return codes. Ten rotated repetitions
per mode/width follow one omitted repetition. All output hashes agree; separate
semantic probes compare individual doubles byte for byte, including all missing
codes and reordered plugin varlists.

Command measurements include input allocation, missing masks and all estimator
work. `primary` has five rotated three-arm blocks at N=983,040/K=50/threads=16.
`holdout` has three alternating baseline/fast pairs for K=10/threads=16,
K=50/threads=1 and shuffled K=50/threads=16. `smoke` checks all three variants
against an independently formed explicit-dummy least-squares oracle.

In the measured investigation, the scripts were assembled incrementally.
ASan deadlocked during runtime initialization before `main`; its process was
terminated. A kernel run that overlapped it was discarded and preserved locally.
UBSan guard tests and the isolated final kernel run passed. A subsequent bound
fix makes the diagnostic tile initialization safe for fewer than 256 rows;
the final kernel receipts use that exact source. No ASan success is claimed.
