# Single-residual PPML checkpoints: bounded local adoption

The existing certified Schur mechanism now handles one actual checkpoint RHS,
reducing selected two-FE PPML command times by about 10–17%. The implementation
generalizes the probe to available columns while keeping current-weight
construction, fresh certificates and untouched-input fallback. It adds no
persistent buffer or weight-generation cache.
[ADR 0106](../../../../docs/adr/0106-single-rhs-ppml-schur-checkpoint.md)
records the scope. This closes the local residual experiment; it does not
establish a universal win over fixest or formal PPML/F0 qualification.

## Source and dominant cost

Baseline `means1` matches `55e9c48f`'s 525 captured source/test inputs. Candidate
`residual2` is that revision plus the archived `fe/borrowed_map.rs` patch;
all 525 captured inputs match the final source. Its plugin SHA-256 is
`44090d1ab6db8a6a9afea8640c28e440de1eb4562010308bd22e3d604800c792`.
The initial screen uses `residual1`, plugin
`758b7148da0ab7f54cff9a11e8969e0c782c4f28cae7cf22ad50957b1543f896`.
Its production code is identical after excluding comments; only explanatory
comments and tests changed before the final build. Full identities, the first
source file and the explicit comparison are archived. The new licensed fixture
was added after the build and is separately bound by its executed-file hash.

Separate diagnostic runs show the degree-six final residual requiring an LSMR
attempt and tighter independent retry. Their setup/solve/certificate costs sum
to about 34.8 ms. The candidate's current-weight probe contracts to 2.92e−7,
and its certified Schur attempt costs 1.75 ms setup plus 3.74 ms solve. The
sparse input contracts only to .0730, fails the unchanged .01 gate, and returns
to LSMR after a 2.03 ms probe. These are diagnostic clocks, not headline fits.
The initial fresh certificate already existed and remains mandatory.

Whole-command phase medians support that location: screen degree-six strict/
final solve falls 122.65→88.87 ms; fresh wider degree-six falls 108.64→66.82 ms.
Sparse strict/final rises 113.16→117.01 ms. Ordinary engine selection, weighted
geometry lifetimes, working-response refresh, public output requests and
canonical scientific policy are unchanged. The trace harness now enables the
existing solver trace only for explicit diagnostic runs.

## Timing distributions and losses

Three alternating repetitions per cell; initial seed 202609221 and fresh seed
202609227. Confirmation and six native controls were declared before the screen.
Two more controls were added before confirmation: wide OLS shares the probe
helper, and K65 PPML has no reusable Schur owner. No scientific guard changed
after the screen. Frozen 77 primary, 16 holdout and 111 native-control definitions
and the reserved validation seeds remain untouched.

All 93 headline attempts complete, without timeout, detected competing fit/build
or monitor error. No compilation, profiling, archiving, oracle calculation or
competing fit ran alongside these blocks. Timers cover the whole public command,
including native loading/preparation, estimation, inference and publication.
External data import and evidence export are outside both timers. Stock fixest
is the unchanged pinned, uninstrumented 0.14.2 artifact with matched threads,
numeric regressors, robust inference HC0*N/(N−1) and no saved outputs.

| PPML case, N / K / threads | Baseline / candidate / stock median, ms | Paired candidate/baseline min / median / max |
|---|---:|---:|
| Degree-six two FE, 61440 / 10 / 4 | 324 / 292 / 98 | .867 / .901 / .932 |
| Sparse two FE, 61440 / 10 / 4 | 335 / 340 / 1977 | .959 / 1.018 / 1.033 |
| Fresh degree-six two FE, 61440 / 17 / 8 | 346 / 287 / 149 | .815 / .827 / .866 |
| Fresh dense two FE, 30720 / 6 / 4 | 149 / 149 / 79 | .966 / 1.000 / 1.007 |
| Fresh weak two FE, 30720 / 10 / 4 | 293 / 293 / 217 | .997 / 1.003 / 1.021 |

All 15 candidate-versus-stock sample/beta/V/objective checks pass, as do the
baseline comparisons. The pairwise-comparable rows contain one stock timing
win and four losses. These selected development comparisons do not supply the
full pinned-ppmlhdfe and same-cell independent-oracle qualification gate.
The sparse candidate's small native-baseline loss is retained, not discarded
because both native versions are much faster than stock on this input.

| Native control, N / K / threads | Baseline / candidate median, ms | Paired min / median / max |
|---|---:|---:|
| Two-FE PPML serial, 30720 / 10 / 1 | 249 / 250 | .968 / 1.004 / 1.016 |
| Two-FE PPML PW/cluster, 30720 / 10 / 4 | 195 / 187 | .949 / .959 / .990 |
| Two-FE PPML low memory, 30720 / 10 / 4 | 347 / 351 | .987 / .988 / 1.032 |
| Two-FE PPML slope, 30720 / 10 / 4 | 1268 / 1264 | .978 / .997 / 1.007 |
| One-FE PPML, 61440 / 10 / 4 | 158 / 161 | .982 / 1.006 / 1.025 |
| Three-FE PPML, 61440 / 10 / 4 | 424 / 422 | .983 / .984 / 1.002 |
| Wide two-FE OLS, 61440 / 31 / 4 | 51 / 50 | .926 / .941 / 1.060 |
| Two-FE PPML K65, 30720 / 65 / 4 | 665 / 665 | 1.000 / 1.000 / 1.001 |

Medians of paired ratios need not equal ratios of medians; low-memory dispersion
illustrates this. Protected-path differences are mostly small or mixed. No
broad excluded-path or OLS gain is claimed. All times, repeats, native peaks
and public runtime phases are preserved in the three summary JSON files.

## Independent checks and memory

All 39 native pairs pass unchanged scientific tolerances and exact sample checks.
Thirty-six have byte-identical scientific exports. The three fresh wide pairs
have maximum beta/V differences of 1.00e−16/7.62e−21; objective and sample are
unchanged. Within-artifact repeat checks pass. There were no application failures
in this milestone, including the four separate diagnostic calls.

The final source passes 378 distinct focused Rust tests in 390 executions:
17 checkpoint tests and 373 PPML tests with 12 overlaps. Three checkpoint tests
extend coverage to single-RHS input, exact/one-byte-short copy allowance,
current supplied/scaled weights, poisoned inactive storage, unchanged capacities,
independent tighter MAP results, forced finite cap and certificate rejection,
bit-identical untouched-input LSMR fallback, final MAP fallback, allocation and
interruption errors. A weak weighted cut rejects before copying. Wider existing
probe arithmetic is checked against the original independent entry point.

Actual-plugin ABI containment passes. Five licensed fixtures pass explicit
return-code and terminal-sentinel checks: `feppml_reference_refresh`,
`feppml_simplex_ir_publication`, `feppml_native_preparation`, `ols_schur_wide`,
and the new `feppml_single_residual_schur`. The last forces public explicit LSMR
on N49,152/K12/two FE, checks unit/FW/PW, offsets, robust/clustered inference,
saved means and T1/T4 against three independent dummy-Poisson fits and directly
formed current-Hessian sandwiches. Its trace proves six actual single-RHS
Schur acceptances, so the public oracle exercises the changed path. Ordinary
fixest weights are not treated as an FW inference oracle. The first candidate
also passed three extended checkpoint tests, 373 PPML tests and the existing
refresh fixture before its screen.

The existing owner supplies topology and level-space storage. Current group
weights and numerical state are rebuilt for each probe/attempt. One transient
N-element candidate replaces larger LSMR banks under the existing checked
allowance; it is dropped before fallback. There is no retained weight-dependent
cache or new memory mode. Tracked candidate-minus-baseline peaks range from
−5,141,008 to +207,184 bytes; threaded allocation overlap still varies. This
is not exact peak neutrality or constrained-memory qualification. The static
allocation inventory still fails on documented drift and was not regenerated.

## Closeout and next subsystem

This bounded experiment is accepted and closed. The current series has delivered
three-FE LSMR transport improvements, fresh PPML reference/mean pass reductions,
a zero-outcome publication repair and this residual projection improvement.
Their earlier timing claims remain attached to their own captured artifacts;
the historical 93-cell categorical OLS wins are not transferred here.

The next useful subsystem is narrow **multi-RHS PPML canonical checkpoints**:
the diagnostic still spends roughly 38 ms on the initial 12-RHS frame and
45 ms on the strict 11-RHS frame in the degree-six target. Those RHS widths
remain deliberately outside the wide Schur route. Investigate current-weight
batch geometry and assembly as a separate bounded study; do not simply remove
its guard or tune the new single-RHS rule. Three-FE upstream solve cost remains
a separate larger algorithmic opportunity after the accepted transport savings.

Large disconnected PPML and sparse three-FE OLS reference discrepancies remain
unranked; generated-term, saved-output and constrained-memory coverage remains
incomplete. Weighted constrained-fit failures, nonretaining CG instability,
closed serial-slope losses and deferred SCC/Linux checks are unchanged. No
qualification campaign, stopped reference campaign, push or release was run.

`evidence.zip` contains 3,000 portable source-bound entries with original and
archived hashes, captured controllers, plans, numeric exports, traces, tests,
fixtures and log hashes. Data, binaries and raw licensed logs are excluded.
