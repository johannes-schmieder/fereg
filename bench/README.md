# Benchmark suite

`suite.toml` declares the 22 cells, immutable seed, comparator policy and tiers.
`generate.py` ports the preserved synthetic generators and writes a single
numeric DTA for every estimator. The first controls are invariant to K. Seeds,
data hashes and generator versions are stored beside the temporary datasets.
Veneto uses an external prepared DTA with `obs_id y x1 g1 g2`; its source data
and preparation stay outside Git (see `real/veneto/prepare.py`).

Prerequisites: Python 3.11+ with NumPy and pandas, R with its base `foreign`
package, the pinned Rust toolchain, and licensed Stata/MP. Use a private run root:

```sh
python3 bench/setup.py --root /tmp/fereg-bench-env
./scripts/build_macos.sh
python3 bench/run.py --tier smoke --engine legacy \
  --environment /tmp/fereg-bench-env --root /tmp/fereg-bench-smoke
```

Setup verifies the archive hashes in `references.lock.toml`, stages the three
bundled Stata references plus pinned ppmlhdfe, and builds fixest 0.14.2 in a
private R library. It does not change PLUS, PERSONAL or the user's R library.
Acquisition/builds happen before timing. Source-index files stay with their
package; only runtime files share the private Stata reference path.

Each cell/thread/comparator receives one untimed warm-up process followed by
five fresh measurement processes. The reported value is the median of five
successful estimation times, excluding process startup and dataset loading.
Requested residual/FE outputs are included in estimation work. A separate
fereg fit at tolerance 1e-12 supplies reference coefficients and exact sample
IDs. fixest defaults and matched accuracy are distinct arms (OLS FE tolerance
1e-10; PPML GLM 1e-10 and FE 1e-11). Stata host processors are capped at its
license limit; native/R threads follow the manifest. SCC caps Stata at four.

OLS input/native/total-to-precommit times come from `e(fereg_runtime)`;
legacy PPML exposes total time in `e(feppml_runtime)` but no separate SPI-input
and native-time fields, so those columns stay empty. The wall timer covers the
whole estimation command. Neither unavailable telemetry nor a failed fit is
represented as zero.

The current legacy ado has no `engine()` selector. `--engine legacy` selects its
only implementation. `--engine next` fails before running until M5 enables the
manifest's explicit engine switch, after which both choices are passed through.
`--solver lsmr|cmg` is for the separately declared supported-case comparisons;
its results are labeled and must not enter the default-policy aggregate.

`standard` uses all 19 non-SCC cells at 1/8 threads; `full` adds the three SCC
cells and the declared 1/2/4/8/14/28 thread ladder. Use `--plan-only` to inspect
jobs without executing them. `--cells` restricts diagnostic runs and marks their
summaries partial. Standard/full command timeouts are 30 minutes/two hours.
A failed or timed-out command stops that arm's remaining repetitions; there is
no automatic retry. Other independent arms continue. A new invocation requires
a new root, and summaries never overwrite earlier evidence.

`compare.py` writes a CSV of at most 100 KB to `history/` (or `--summary`), and
prints pairwise ratios, geometric means, worst cells and timeouts. Missing
commands/repetitions, reference failures, sample disagreements and nonfinite
coefficients remain visible. Only complete accurate pairs enter aggregates;
the passing/expected count is always printed. Partial or dirty-source smoke
results are diagnostic, not committed-source baseline evidence. M1.6 establishes
the committed Mac/SCC baselines. Raw logs and datasets stay in the run root.

Harness checks (no licensed command needed):

```sh
python3 -m unittest discover -s bench/tests -v
```
