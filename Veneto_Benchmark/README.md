# Veneto benchmark

This directory compares public OLS commands `fereg`, `reghdfe`, `greg`, Stata 19
`areg`, R `fixest::feols`, and Python `pyfixest.feols`. The analogous PPML
comparison uses `feppml`, `ppmlhdfe`, `gglm, family(poisson)`, R
`fixest::fepois`, and Python `pyfixest.fepois`. Areg has no corresponding
absorbed Poisson command in this comparison.

The requested execution platform is now SCC. See [the SCC protocol](scc/README.md)
for its 1/2/4/8/14/28-core campaign and staging requirements. The local protocol
and receipts below are preserved as diagnostic preparation. The final PDF will
use SCC measurements only.

The deliverable is `output/pdf/Veneto_Benchmark.pdf`. Aggregate receipts and
package versions are under `results/`; all raw data, row-level extracts,
private logs, and installed environments stay in ignored `work/`. The source archive stays local; SCC receives only derived model inputs after
approved storage is established. Current progress is recorded only in the repository's
`STATUS.md`.

## Data and model

The input is the FRDB delivery of Veneto Workers Histories in the user's
research folder. It is the same source family used by Kline, Saggio, and
Sølvsten (2020), *Leave-Out Estimation of Variance Components*, Econometrica
88(5), 1859-1898, [doi:10.3982/ECTA16410](https://doi.org/10.3982/ECTA16410).
The delivery is not established as byte-identical to their historical data.
This is a real-data computational benchmark, not replication of their
variance-component estimates or leave-out connected sample.

`scripts/prepare.py` reads all spell records and keeps 1999-2001, removes
duplicates on worker/year/firm/paid-days/remuneration, then selects the
highest-remuneration spell per worker-year. Ties use smallest firm ID and
then most paid days. It validates unique worker and firm merge keys, keeps
employers in the seven Veneto provinces, ages 19-63, and finite positive
remuneration and paid days. It then removes worker and firm singletons to
a fixed point. It uses no random sampling for the main panel. Original IDs
are kept at full precision during construction and recoded to consecutive
integers for all estimators. `results/sample.json` binds the archive, inputs,
sample counts, and resulting Stata files.

Both models absorb worker and employer indicators and use four identical
controls: age squared divided by 100, age cubed divided by 1000, and indicators
for 2000 and 2001. A linear age term would be collinear with worker and year
effects. OLS uses log daily remuneration; PPML uses positive daily remuneration
in delivered units. PPML permits this continuous positive response; it is not
a count-data replication or the OLS model on a different scale. No wage or
wage-change trimming, public-sector exclusion, sex/nationality restriction,
or leave-out connected-set selection is imposed.

## Local diagnostic measurement (superseded for the final report)

Runs are sequential fresh processes on the local Mac. The requested thread
ladder is 1, 2, 4, 8, 16, 24. macOS does not provide supported process CPU
affinity here, so these are thread limits, not pinned physical cores. The
M2 Ultra has 16 performance and 8 efficiency cores. Stata/MP is licensed for
8 processors: the Stata setting is `min(request, license, machine)`; fereg's
native Rust setting and R/Python library settings are recorded separately.
Reghdfe's optional multi-process `parallel()` is not used. Gtools' absorption
implementation may remain serial. Flat scaling is a valid measurement.

Each configuration has one cold diagnostic and three warm repetitions.
Warm fits initialize the same public API on a separate, deterministic
7,680-row dataset in each fresh process; no target fit state is reused.
Command wall time includes covariance computation and excludes data loading,
import hashing, initialization and result export. Native solver defaults
are retained. OLS uses native IID covariance; PPML uses robust covariance.
R/Python PPML use HC0 with an extracted covariance multiplier N/(N-1) to align
the Stata reporting convention; the multiplier is applied outside timing.

The controller checks imported columns as little-endian IEEE-double SHA-256
sequences, samples absolute process-tree RSS every 10 ms during the command,
and handshakes before starting the timer. RSS includes loaded data and
language/runtime state, and can miss peaks shorter than the sample interval.
It is not native allocated bytes or memory incremental to the loaded dataset.
The per-command limit is 600 seconds. Timeouts remain censored lower bounds;
errors, convergence failures, sample differences, and accuracy disagreements
are retained, never silently dropped or repaired by changing the model.
Figures distinguish successful fits, time/memory consumed before an error,
and timeout lower bounds. Error markers must not be interpreted as completed
estimation times. Pilot fits are diagnostic and can overlap setup or other
local work; they never enter the main figure medians.

## Numerical checks and interpretation

`scripts/validate.py` checks both candidate commands on a deterministic,
singleton-pruned real-data subset against a separately implemented SciPy
explicit-dummy OLS/Poisson oracle. It checks coefficients, standard errors,
and sample counts at the repository tolerances. This bounded check does not
prove accuracy of the full panel. Main-fit coefficients, covariance summaries,
objectives and sample identity are compared with the pinned Stata reference.
Native DoF policies differ when the graph has disconnected components;
standard-error disagreement is reported separately from coefficient agreement.
Elapsed times alone are not a scientific-equivalence or release claim.

## Local diagnostic reproduction

Use an isolated Python environment with the pinned PyFixest source archive,
NumPy, pandas, SciPy, psutil, Matplotlib, reportlab and pypdf. Use R fixest
0.14.2, jsonlite and digest. The exact comparator source archives and versions
are listed in `results/comparator_archives.json`; references are staged under
`work/refs`. The candidate's source SHA and frozen plugin/ado hashes are in
`results/runtime.json`. Estimator workers and the independent oracle are
adapted from this repository's `benchmarks/systematic/` harness.

1. Run `scripts/prepare.py INPUT_ARCHIVE` using the isolated Python executable.
2. Build the current Mac plugin, stage the pinned reference closure, and
   compile its ftools Mata library in isolated Stata.
3. Run `scripts/run.py --pilot`, then `scripts/validate.py`; inspect results.
4. Run `scripts/run.py` for the local campaign. Completed receipts are reused;
   an unfinished invocation requires inspection and remains preserved.
5. Run `scripts/report.py`, then render every PDF page and inspect the layout.

The campaign manifest freezes the thread ladder, repetitions, timeout, and
measurement semantics. Changes require a distinct campaign identity. Pilot
timings do not enter final figure medians.
