# Indexed singleton pruning: bounded scale and AKM validation

The diagnosed source matches baseline main `681634d` exactly (singleton SHA256
`3d32d2af0c03f45a2d46445b7bb7a458d40269f0548cb929a0f40c1ff47fba40`). The
candidate removes the observation-zero search from each singleton deletion.
Its isolated universal plugin is
`253bf9eb6e406331f101c3354ceff560ad2a36916e6c97a7582208cc275cf5f2`.
Installed dist and unrelated work remain unchanged. This is local development
and empirical diagnostic evidence, not cross-platform qualification.

## Mechanism and contracts

Each FE level keeps the XOR of active row indices. With validated positive
integer multiplicity, mass one identifies exactly one row; deleting a row XORs
its index out of every affected level. Counts, singleton tolerances, queue
insertion order and the final sample are unchanged. The algorithm takes
O(D*N + L), for D effects, N observations and L total levels. It does not
construct level adjacency or allocate another observation-sized membership bank.

The index is allocated only after the original no-singleton certificate. Its
one word per level replaces the word saved by encoding queue entries as global
level indices instead of effect/level pairs. Existing fallible reservations,
capacity guards, masks and drop-list bounds remain. Every tested paired live
allocation peak is exactly unchanged. The shared core supports multiway and
interaction partitions and PPML singleton replay, with original masks and
frequency semantics. Existing caller-thread interruption/transaction boundaries
remain; pruning had no internal SPI polling and adds none. Workers never call SPI.

## Focused checks

- 33 shared core tests and 95 serial plugin tests pass without warnings.
- 320 deterministic cases, each with unit and frequency multiplicity, compare
  complete results (including deletion order) with the old scanner and masks
  with an independent simultaneous-pass oracle. Cases span one through five
  effects, inactive rows, empty levels and large integer weights.
- A long cascading path beside a stable component preserves exact masks
  and deletion order. Three bounded scale-doubling cases have near-total drops.
- Every one of seven allocation/capacity failure points fails closed. The
  no-singleton path retains its original three reservations. Capacity payloads
  are bounded by the original implementation on tested shapes.
- Four licensed fixtures pass with explicit rc=0 and terminal sentinels:
  existing owned OLS preparation, native OLS input and PPML native preparation,
  plus the new cascading OLS oracle. The new fixture uses an independent
  alternating-count mask and explicit dummy regressions across unit/AW/FW/PW,
  robust/clustered inference and fast/low execution (16 native fits). It checks
  initial exclusions, cluster missingness, row order, effective N, DoF, b/V,
  RSS and residuals without externally pruning the fereg input.
- Universal static and actual arm64 ABI/self-tests pass. The injected ABI panic
  is an expected containment check. An initial unquoted PATH invocation failed
  before Cargo ran; its output is retained separately, and the corrected run
  supplies the test evidence.

## Full pruning scale experiment

108 fresh-process calls comprise 45 baseline/candidate pairs and 18 larger
candidate-only calls. Three repeats use alternating arm order on fixed generated
inputs. Timers include validation, all index/count/queue construction and the
complete cascade. A standalone allocation-audit wrapper tracks live payload;
this measures pruning only, not an estimation command or hardware traffic.
No builds, tests, oracles or data-generation jobs overlap the timing series.

| Cascading path plus four stable rows | Old scanner median | Indexed median | Native peak, both |
|---|---:|---:|---:|
| 16,388 rows | 69.683 ms | 0.308 ms | 557,241 B |
| 32,772 rows | 276.113 ms | 0.611 ms | 1,114,297 B |
| 65,540 rows | 1,106.749 ms | 1.242 ms | 2,228,409 B |

Old times grow approximately fourfold on doubling, indexed times twofold. At
262,148 / 524,292 / 1,048,580 rows, indexed cascade medians are 4.978 / 10.525 /
21.306 ms. The separate 50%-singleton family gives 236.220→0.927 ms at 65,540
rows, and 3.630 / 7.307 / 15.016 ms at the three larger sizes. Every retained
mask hash and drop count agrees exactly across all repeats and paired arms.

The balanced no-singleton control is unchanged: about 3.199→3.129 ms at
1,048,580 rows, with the same 1,048,636-byte peak. This variation is not a
separate speedup claim. There is no solver/fallback work inside these timings.

## Empirical reproduction

The original 2001–2009 West German AKM script's data, controls, factor years,
recursive singleton policy and `solver(cmg) threads(8) memory(low)` options are
preserved. Only the package root/output paths and explicit retained-row
assertions change in the isolated runner. Source/output script hashes are
recorded; original data, scripts, historical logs and estimates are untouched.

The worker-ID smoke loads 75,350 rows, drops 9,847 and retains 65,503, passing
its completion sentinel and exact row assertion. Its command is 1.561 seconds;
this single diagnostic is not a paired performance claim.

The original full estimation attempt clears native preparation in **27.574 s**
and completes its CMG projection/polishing batches, then fails closed with
**r(459), `prepared OLS bread inverse certificate did not pass`**, during the
unchanged low-memory numerical path. No full-sample estimates are produced.
A one-second stack sample during this attempt locates low-memory pivoted QR,
Householder updates and scratch reads/writes. The process runs 813 seconds,
including the script's import/descriptive work; the estimation-only timer and
native action peak are not published on failure. The largest observed RSS
sample is 61.61 GB, not an established peak. The final `time -l` resource summary
fails because its `kern.clockrate` query is sandbox-denied; no missing memory
value is treated as zero.

A **separate diagnostic artifact** (`e9b2141e`) changes only the plugin boundary:
after the unchanged preparation returns, it prints counts and intentionally
returns interruption before any estimation. This is not an estimator success
or a change adopted into production. Its tiny patch and identities are archived.
The diagnostic do-file retains original data/controls/options, omits read-only
descriptive tables, and writes separate evidence. It confirms:

- Original and candidate rows: **91,045,290**.
- Singleton drops: **2,169,026**.
- Retained rows: **88,876,264**, exactly matching reghdfe's reported fixed point.
- Native preparation: **29.582 s**; complete diagnostic process: **86.26 s**.
- Process maximum RSS: **48.16 GB**; peak footprint: **46.02 GB**; zero swaps.
- Expected r(1) and an explicit assertion that no coefficient matrix is published.

These RSS values include Stata and all preceding preparation, not just pruning.
The retained-count match establishes the singleton fix despite the
later numerical failure. The full-model bread certificate is a separate unresolved
blocker; no tolerance, rank policy or solver path was changed to bypass it.

The completed smoke's 18 requested coefficients differ from the owner's saved
reghdfe result by at most **1.77913e-14**; the corresponding V entries differ by
**1.37152e-17**, RSS by **1.00849e-14** relatively. N and residual DoF agree exactly.
This is diagnostic comparator evidence. Full-sample coefficient/VCE parity is
unavailable because fereg failed before publication.

No `keepsingletons`, external sample pruning, numerical tolerance change or
solver-policy change is used. The old full fereg run was stopped in pruning,
so a completed old-versus-new full-command speedup cannot be computed.

## Replay and evidence limits

`implementation.patch` applies to the diagnosed baseline, including the new
Rust and Stata tests. `built-identities.json` binds compiled inputs and artifact;
`validation.json` separates successful fixtures, the failed full estimation and
the deliberately stopped preparation audit. Raw licensed data/logs and binaries
remain local and are not committed. The scale series is reproducible with
`replay_scale.py --baseline-source BASELINE --candidate-source CANDIDATE
--rustc RUSTC --output OUTPUT`, using Rust 1.85.1 and the archived driver.

The first diagnostic build used only arm64 and correctly failed the repository's
universal-architecture audit. A restoration attempt copied the script onto itself;
that attempt also remained unsuccessful. Restoring the normal universal builder
produced the final audited diagnostic artifact. Original failures remain preserved.
