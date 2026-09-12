# Local fereg–fixest gap investigation

This is a development experiment for one-FE overhead and well-connected
50-control absorption. It does not submit SCC work or change the frozen
systematic campaign. The report and source-bound prototype patches are in
[`../reports/development/fixest-gap-local-2026-09-11/`](../reports/development/fixest-gap-local-2026-09-11/README.md).

The generator, topology definitions and independent oracle are reused from
`benchmarks/systematic/`. Each invocation runs in a fresh Stata/R process,
performs an untimed small warm-up, checks imported values against canonical
column hashes, and times the complete estimation command with IID covariance.
Processes run sequentially. Native phase tracing is off for the repeated
comparison. RSS is sampled at 10 ms and includes the process tree. macOS CPU
placement is uncontrolled; thread limits and launch load averages are recorded.

## Reproduction

Use a local macOS machine with licensed Stata, the repository's pinned Rust
toolchain, Python with NumPy/Pandas/SciPy/psutil, and a separate R library with
fixest 0.14.2, jsonlite and digest. The tested fixest source archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Set `STATA_MP_PATH` to the Stata executable and `R_LIBS_USER` to that R library.

Set `FEREG_LOCAL_BENCH_ROOT` to an isolated output directory. Create
`work/refs` there; no external Stata reference command is used. Place two
source/artifact trees there, named `baseline` and `schur-prototype`, each
containing `stata/` and `dist/fereg_macos.plugin`. Construct both from local
commit `d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd`; apply the report's
`patches/schur-prototype.patch` only to the latter. Build each using
`scripts/build_macos.sh`, keeping the artifacts separate. Record the resulting
artifact hashes; a rebuild need not have the historical binary identity.
For the final confirmation, also stage `vector-prototype` from the same base
commit plus `patches/vector-prototype.patch`.

From this directory:

```sh
python final_compare.py --part primary
python final_compare.py --part oracle
python final_compare.py --part holdout
python final_compare.py --part overhead
python final_compare.py --part pool
python vector_confirm.py
```

The primary comparison rotates command order over three seeds at 4 and 16
native threads, with 983,040 rows and 50 controls. Stata's internal processor
count is capped by the installed license; the Rust plugin uses the separately
requested native team. Each command has a 90-second cap; graph holdouts have
60 seconds. Oracle fits use at most 7,680 observations. The optional `run.py`
entry point accepts a source tree, design, sample size, control count, thread
count, solver and trace flag for individual screens.

The checks include exact sample hashes, full native coefficient-status/rank/DoF
parity, all native certificate flags, both FE certificates, full fereg
coefficient/covariance matrices including the reported constant, and slope
coefficient/covariance/RSS agreement with fixest. The independent oracle uses
explicit sparse FE indicators, column-equilibrated SciPy LSMR and a separate
finite least-squares calculation. No tolerance is relaxed.

The September 11 Schur prototype is intentionally restricted to diagnostic copies. It is
not a supported Stata solver or an admitted automatic route. Its eligibility,
extra workspace, iteration accounting and fallback policy still require
production integration. In particular, do not replace an installed plugin
with a diagnostic artifact. Generated datasets, licensed logs and binaries
belong in the isolated output directory, not in the repository.


## Guarded route follow-up (September 12)

The [follow-up report](../reports/development/fixest-gap-local-2026-09-12/README.md)
records the guarded route now implemented for local development under
[ADR 0035](../../docs/adr/0035-guarded-wide-ols-schur.md). The earlier experimental
patches and receipts remain historical evidence.

With the same local environment, an isolated output directory and a built
baseline tree at the pinned commit, run:

```sh
python route_compare.py --part primary --baseline /path/to/baseline
python route_compare.py --part holdout --baseline /path/to/baseline
python route_compare.py --part narrow --baseline /path/to/baseline
```

`--source` defaults to this repository and its built plugin. The primary block
has 54 sequential invocations and rotated order. The holdouts add weak, dense,
bottleneck, disconnected, unbalanced and cycle graphs. The narrow block checks
1/10-control fits. Each output directory must be fresh. The recorded follow-up
reused the three earlier baseline custom-holdout receipts; replay instead
measures those baselines anew. Explicit joint dummy adjudication of the two
native reference disagreements is available through `route_oracle.py DATA OUTPUT`.
This separate 61,440-row focused adjudication leaves the frozen oracle cap and
benchmark tolerances unchanged. `collect_route.py ROOT OUTPUT` archives numeric
receipts and native certificates, including recorded reference disagreements.

When building source variants, use separate Cargo target directories or force
a source rebuild. A shared target directory and preserved file modification
times can otherwise reuse a previous variant's artifact. Every measured arm
must record the actual plugin hash, even when two source trees have the same
base commit. Mac-specific experiments in this report required no SCC jobs.

### Numeric-pass follow-up

The later September 12 pass investigation uses the same `route_compare.py`
primary, holdout and narrow entry points. Its baseline is the **retained Schur
candidate** from `fixest-gap-local-2026-09-12`, not the earlier solver baseline.
Use a fresh `FEREG_LOCAL_BENCH_ROOT`; reconstruct that baseline using the
previous report's candidate patch on its recorded base commit and bind its
recorded plugin identity. The pass report includes a separate incremental
patch as well as a complete patch against that base commit.

After the untraced primary comparisons, `passes_profile.py --source SOURCE`
runs six traced candidate measurements and checks them against the matching
untraced results. `collect_route.py ROOT OUTPUT` collects the numerical
receipts. As with the solver investigation, compilation, tests and estimator
processes run sequentially; these entry points never submit cluster work.

The pass follow-up repeats the weak, 10-control holdout at seeds 59047, 59059
and 59063 with `passes_profile.py --part narrow-repeat --baseline BASELINE`.
Run `passes_audit.py ROOT` after the comparison to verify byte-identical full
b/V exports, fit/certificate tables, solver fields apart from timings, and
reported fit statistics against the retained Schur baseline.

The slow one-FE/sixteen-thread seed receives two separate, order-reversed
checks with `passes_profile.py --part outlier-repeat --baseline BASELINE`.
The original observation remains in the primary results; repeats never replace
it. Raw RSS summaries must be read with the pass report's memory-quality flags.

### Scheduling follow-up

The [scheduling report](../reports/development/fixest-scheduling-local-2026-09-12/README.md)
uses the retained numeric-pass implementation as its baseline. Reconstruct it
from the preceding pass report's complete candidate patch and recorded base
commit. Stage the baseline's `stata/` and plugin separately, choose a fresh
`FEREG_LOCAL_BENCH_ROOT`, and run `route_compare.py` for `primary`, `holdout`
and `narrow`, followed by `passes_profile.py` and
`scheduling_audit.py ROOT`. The final source is the scheduling report's
candidate patch applied to its recorded base commit. Complete patches and
plugin identities also identify the rejected scheduling prototypes.

The scheduling audit requires identical full b/V exports, sample, status,
DoF, public fit/certificate tables, solver acceptance maxima, routing,
iterations and primary native allocation; holdout allocation differences are
recorded separately. Fixed group partials can change the last
bits of the diagnostic two-norm and normal-equation residual; these fields
are compared separately and their observed difference is reported. The
maximum FE moment still controls acceptance at the unchanged tolerance.
The explicit-dummy wide Stata fixture includes 1, 4 and 16 native threads.

The scheduling follow-up uses `scheduling_repeats.py --part one-fe-repeat --baseline BASELINE` for the small four-thread one-FE timing difference, and
`--part thread-eight` for the changed recurrence crossover. These additional
calls remain separate from the primary medians.
