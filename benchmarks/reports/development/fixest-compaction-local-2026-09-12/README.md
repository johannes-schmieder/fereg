# Native compaction of incomplete OLS samples

The retained change removes most of the preparation penalty caused by sparse
missing values in wide, unweighted IID regressions. It preserves the complete
sample fast path and all solver decisions. This is bounded local development
evidence on the M2 Ultra, not SCC or release qualification.

## Results

Times are whole estimation commands in seconds, including Stata parsing,
loading, native computation, and publication. The first three rows report
medians over three frozen seeds with rotated estimator order. Estimators run
sequentially in fresh processes after a small untimed warmup. Native thread
requests are 4 or 16; macOS controls placement on the machine's 16 performance
and 8 efficiency cores. Stata's own MP license permits eight processors.

| Input | Original rows | Controls | Native threads | Previous fereg | Current fereg | fixest |
|---|---:|---:|---:|---:|---:|---:|
| One FE, about 0.1% missing | 983,040 | 50 | 4 | 1.461 | 0.926 | — |
| One FE, about 0.1% missing | 983,040 | 50 | 16 | 1.230 | 0.613 | — |
| One FE, complete | 983,040 | 50 | 16 | 0.585 | 0.580 | 0.472 |
| One FE, 90% missing, retained pairs | 245,760 | 50 | 4 | 0.176 | 0.135 | — |
| One FE, 90% missing, retained pairs | 245,760 | 50 | 16 | 0.180 | 0.133 | — |

The sparse-missing case improves by **36.6% at four threads and 50.2% at
sixteen**. At sixteen threads it now takes only 5.7% longer than the complete
sample in these probes, compared with 110% longer previously. These are
slightly different estimation samples, not a fixed-sample speed comparison
between complete and missing inputs. The retained-pair rows are single-seed
holdouts; they improve by 23–26%.

The complete-data change is less than one percent and should be treated as
unchanged. **Current fereg still takes 22.9% longer than fixest in this
sixteen-thread one-FE case.** This implementation addresses missing-data
preparation rather than closing that remaining gap.

One four-thread candidate observation took 1.300 seconds, with 0.625 seconds
in input loading; the other two took 0.926 and 0.892 seconds. Its native phase
was consistent with the improvement. A separate reverse-order repeat of the
same seed took 1.388 seconds previously and 0.860 currently. The original
three-seed summaries retain the slow observation and exclude the extra repeat.
No cause for the input-time outlier is asserted.

The missing-input probes are Stata-only: their binary import audit encodes
system missing as Stata's double sentinel, which must not be interpreted as a
finite observation by the R worker. No missing-sample speed or numerical
comparison to fixest is claimed. Complete-data fixest comparisons use the
existing frozen generator and pinned reference environment.

## What changed and why it helps

Previously, any exclusion or missing value sent owned unweighted input back
through the general borrowed preparation routine. That routine classifies a
row and then pushes each retained value into a separate destination column.
With 50 controls, this combines repeated cross-column accesses with a full
copy of the selected data.

The retained implementation:

1. Uses the existing authoritative row classifier to fill a native drop mask.
   On wide inputs, independent 16,384-row blocks run on the already installed
   requested team. Indexed error selection preserves the earliest original-row
   error; exclusions and missing-reason precedence are unchanged.
2. Constructs the stable retained-row map from that mask, then compacts
   independent columns. It reuses owned buffers only within the general
   path's candidate-axis capacity bound. Otherwise it reserves compact storage
   fallibly, so an `if`-restricted sample does not retain full-data capacity.

The complete unweighted ownership shortcut is retained. Weighted, complex
categorical, robust/clustered and PPML preparation remain on their existing
paths. PPML's allocation-free classification visitor is unchanged. Workers
never call Stata SPI. No numerical formula, tolerance, FE route, certificate,
rank policy, singleton rule, DoF calculation, or publication transaction changes.

Two exploratory traced pairs isolate the mechanism:

| Experiment | Previous command | Prototype command | Previous preparation | Prototype preparation |
|---|---:|---:|---:|---:|
| Owned column compaction, serial classifier | 1.176 | 0.865 | 0.617 | 0.317 |
| Add parallel row classification | 1.232 | 0.612 | 0.640 | 0.045 |

In the second pair, inference takes 0.191 and 0.190 seconds respectively.
The gain is localized to preparation, rather than a change to the estimator.
These are exploratory single pairs, separate from the final repeated results.

Native phase medians on the final sparse-missing input fall from 1.063 to
0.508 seconds at four threads and from 0.841 to 0.242 at sixteen. Input-loading
time is essentially unchanged apart from the noted outlier. On the complete
sixteen-thread input, current component medians are 0.303 seconds loading,
0.210 native and 0.069 outer Stata work. Component medians are calculated
independently and need not sum to the median command time.

## Preserved strengths, memory and correctness

Single-seed complete-data holdouts cover weak degree-three graphs, dense two-FE
graphs, bottlenecks, disconnected blocks, unbalanced graphs and cycles. They
show differences of at most nine milliseconds between previous and current
fereg. For example:

| Holdout | Threads | Previous fereg | Current fereg | fixest |
|---|---:|---:|---:|---:|
| Weak degree three, 61,440 rows | 4 | 0.135 | 0.139 | 0.409 |
| Weak degree three, 61,440 rows | 16 | 0.153 | 0.150 | 0.171 |
| Dense two FE, 245,760 rows | 4 | 0.355 | 0.364 | 1.111 |
| Dense two FE, 245,760 rows | 16 | 0.270 | 0.263 | 0.447 |

All 20 final candidate/previous pairs have byte-identical full coefficient
and covariance exports, exact samples, statuses, DoF, fit/certificate tables,
and solver diagnostics apart from timings. All seven fixest comparisons
pass the existing sample, coefficient, covariance and residual-sum checks.
The change does not alter the solver on any measured graph. The holdouts
support preservation in these local cases; they are not a new universal
performance or cross-platform claim.

Modeled fast/low memory budgets agree in every paired comparison. Reported
native allocation peaks agree in 18 of 20 pairs. On the two retained-pair
holdouts, the peak falls from **220,448,053 to 120,419,044 bytes**, a
100,029,009-byte reduction. Those samples avoid a preparation copy that was
previously the peak. Sparse-missing whole-command peaks remain unchanged;
the optimization does not reduce every workload's maximum memory requirement.

The initial heavy-filter recipe kept only every twentieth row. Because the
frozen one-FE input has five consecutive observations per group, it left
12,288 available observations in 12,288 separate groups. Singleton deletion
therefore removes all available rows. The original failed benchmark receipt
is preserved, and the candidate independently returns the same Stata code
2001. These two `worker_failure` receipts are explained negative controls,
not successful runtime observations. The separate retained-pair recipe keeps
two rows in each selected group and uses a different frozen file and hash.

The evidence contains **53 completed invocations: 51 successful estimates and
two all-singleton negative controls**. Focused validation also passes:

- 37 Rust OLS preparation tests, five sample tests and 40 plugin action tests.
  Coverage includes error ordering across block boundaries, original-row
  mapping, retained floating-point bits, capacity bounds, FE-source roles,
  empty samples and singleton cascades.
- Eight licensed Stata fixtures. The extended native-input fixture compares
  sparse extended missing values, dense exclusions and `if` restrictions to
  independent dummy regressions at 1/4/16 threads. Existing weight, robust,
  transaction, time-series, categorical, memory-mode, saved-output and PPML
  smoke checks also pass.
- Universal arm64/x86_64 build and binary audit, plus the actual arm64 plugin
  ABI/self-test. No x86_64 scientific execution is claimed.

## Remaining bottlenecks

Serial Stata SPI loading still consumes about 0.30 seconds on the complete
million-row, 50-control case, slightly more than half of the command. The
native estimation phase still costs about 0.21 seconds. Closing the remaining
fixest gap requires improving these stages; more parallelism in the now-small
sample classifier will have limited benefit.

The analogous weighted and robust/clustered preparation paths still use their
existing materialization routines. Extending ownership and column compaction
to them is a concrete next experiment, but is not implemented or benchmarked
here. Small/narrow one-FE gaps documented in the preceding report are also
unresolved. The measured gains here should not be generalized to those cases.

## Evidence and reproduction

[Source identities](identities.json) bind all four local source variants and
replayable patches against base commit
`d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd`. The retained plugin SHA-256 is
`43377269d0bbfd7dc1bc2a964e117d9f282e123350820252289e3f6b76a92fae`.
The baseline matches the preceding [input report](../fixest-input-local-2026-09-12/README.md).
[The incremental patch](patches/incremental-from-input.patch) isolates this
change from the earlier input work.

[Measurements](measurements.csv), [missing summaries](missing-summary.csv),
[complete-data summaries](primary-summary.csv), [filtered summaries](filtered-summary.csv),
[component medians](command-parts.csv), [phase timings](phase-timings.json),
[exact scientific checks](extra-science.json), [negative controls](singleton-check.json),
[validation](validation.json), and [numerical evidence](evidence.zip) preserve
the observations. The archive contains numerical exports and identities for
24 inputs/warmups, without raw licensed logs, data matrices or host paths.

With the existing local harness environment and immutable `baseline` and
`candidate` trees under `FEREG_LOCAL_BENCH_ROOT`, run
`benchmarks/local_fixest_gap/compaction_compare.py` with `--part missing`,
`--part primary`, `--part filtered`, `--part all-singletons`, and `--part recheck`
sequentially. Use `route_compare.py --part holdout` with the two source-tree
arguments for graph preservation checks. Input hashes, harness hashes and
all original error receipts are retained. No SCC, remote job, commit, push,
or release action was taken.
