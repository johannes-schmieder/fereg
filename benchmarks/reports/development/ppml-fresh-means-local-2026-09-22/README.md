# Fresh weighted column means: local PPML adoption

Reported-intercept assembly now borrows numeric sources and schedules independent
column means while retaining fresh current-weight calculations. This reduces
selected one-/two-FE PPML command times by about 10–12%, without a new buffer or
weighted cache. [ADR 0105](../../../../docs/adr/0105-ppml-fresh-column-means.md)
records the boundaries. Most comparable PPML cases still lose to stock fixest.

Candidate `means1` is `346d80e4` plus the archived `ols/prepared_fit.rs` patch.
All 525 captured computational/test inputs match the working source. Plugin
SHA-256 is `52c3996f5936486cb423f5c904f7275d42504d76735e436370f063af6eae5e9e`.
Baseline `separation1` has the same computational inputs except that patch;
its source snapshot lacks two subsequently added development fixtures. It uses
the unchanged `reference1` plugin and the corrected publication validator.
Full identities and overlays are archived. This is bounded Mac arm64 development
evidence, not PPML/F0 qualification or a release claim.

## Profile and implementation

An isolated diagnostic build of `346d80e4` spends 5.74–5.82 ms computing raw
column means in each retained assembly at N61,440/K10/T4. Three assemblies cost
about 17 ms per fit. Each enclosing bread phase takes 6.53–6.94 ms; raw residual
construction takes about 0.5 ms and the fresh score certificate about 0.7 ms.
Nested clocks overlap and must not be summed as disjoint phases. The diagnostic
plugin is `c8d62aa2753d5b222fb4c97f86b68953707a9f8c9fc6a0a49a09b7f5bb2839b5`;
its exact source patch is retained separately from the final injector helper.
An initial CLI invocation omitted `--revision` and failed before compilation.

The production change uses the existing stack binding for numeric/intercept
columns when no existing mean cache applies. It computes every mean with the
original scaled, compensated routine and fresh denominator. Selected columns
have disjoint K-element destinations, so scheduling does not change per-column
row order or create a partial-sum bank. Current weight frames are never cached.
Generated recipes and K>64 retain their original accessor. Existing OLS caches
remain preferred. Serial, low-memory and slope paths can benefit from binding
when eligible; no solver or memory-mode guard is removed.

Uninstrumented one-FE screen phase medians corroborate assembly savings:
finite solve 17.68→12.85 ms and strict/final solve 49.49→37.98 ms. Preparation
16.32→16.40, projection 23.73→23.69 and publication 9.67→9.63 barely change.
Full phase medians are archived. Public-command timers include parsing,
native loading/preparation, estimation, inference and publication; external
data import and evidence exports are excluded on both sides.

## Alternating command measurements

The three-cell screen uses seed 202609221. Four confirmation and eight native
control definitions were declared before that screen; both fresh blocks use
seed 202609225. All have three repetitions and rotated baseline/candidate/stock
order. No compilation, profiling, archiving, oracle calculation or competing fit
ran alongside headline benchmarks. All 111 attempts complete, with no timeout,
detected overlap or monitor error. No frozen primary/holdout/control definition
or reserved validation seed was changed.

| PPML case, N / K / threads | Baseline / candidate / stock median, ms | Paired candidate/baseline min / median / max |
|---|---:|---:|
| One FE, 61440 / 10 / 4 | 163 / 144 / 50 | .880 / .883 / .900 |
| Degree-six two FE, 61440 / 10 / 4 | 342 / 330 / 98 | .944 / .968 / .971 |
| Dense three FE, 61440 / 10 / 4 | 457 / 459 / 335 | .976 / 1.002 / 1.007 |
| Fresh one FE, 131072 / 17 / 4 | 441 / 388 / 238 | .868 / .885 / .889 |
| Fresh degree-six two FE, 61440 / 31 / 4 | 491 / 440 / 341 | .896 / .898 / .900 |
| Fresh disconnected three FE, 30720 / 17 / 8 | 358 / 356 / 3019 | .992 / .994 / 1.003; stock unranked |
| Fresh one FE with zeros, target 24576 / 12 / 4 | 115 / 111 / 39 | .957 / .965 / .965 |

Six rows pass all stock sample/beta/V/objective checks, and all six still lose
to stock. The disconnected row matches samples, V and objectives but stock
coefficients differ by up to 4.98e−9, about 32.4 times the frozen coordinate
tolerance. Its large apparent speed advantage is **not ranked**. Stock is the
unchanged pinned uninstrumented fixest 0.14.2, with matched threads, numeric
regressors, robust VCE normalized as HC0*N/(N−1), and no saved outputs.
The zero case uses the publication repair and passes unchanged sample checks.

| Native control, N / K / threads | Baseline / candidate median, ms | Paired min / median / max |
|---|---:|---:|
| One-FE PPML serial, 61440 / 10 / 1 | 196 / 186 | .949 / .965 / .969 |
| Two-FE PPML low memory, 30720 / 10 / 4 | 363 / 351 | .967 / .972 / .992 |
| One-FE PPML K65, 16384 / 65 / 4 | 301 / 303 | 1.000 / 1.010 / 1.013 |
| Two-FE PPML PW/cluster, 30720 / 10 / 4 | 181 / 179 | .978 / .989 / .989 |
| Two-FE PPML absorbed slope, 30720 / 10 / 4 | 1213 / 1218 | .998 / 1.002 / 1.014 |
| One-FE OLS, 61440 / 10 / 4 | 20 / 19 | .900 / 1.000 / 1.000 |
| Two-FE OLS cluster, 61440 / 10 / 4 | 85 / 84 | .988 / .988 / 1.012 |
| Three-FE OLS, 61440 / 10 / 4 | 103 / 101 | .944 / .990 / 1.010 |

The K65 and slope losses remain visible; no guard tuning followed confirmation.
OLS differences at this scale do not establish an OLS speedup. Per-call times,
within-artifact repeat checks and all distributions are in the summary files.

## Science, memory and remaining opportunity

All 45 native pairs have byte-identical scientific exports, including separation
receipts and final certificates. Within-artifact repeats pass. The final artifact
passes 382 distinct focused Rust tests in 387 executions: 373 PPML tests, two
new mean tests and 12 reported-intercept tests, with overlap removed. New tests
exercise serial/parallel cutoffs, reversed/noncontiguous coordinates, explicit
intercepts, changing unit/FW/PW frames, extreme scales, cancellation, bad axes
and nonfinite inputs. Four licensed fixtures pass explicit zero-return and
terminal-sentinel checks: `feppml_reference_refresh`,
`feppml_simplex_ir_publication`, `feppml_native_preparation`, and
`ols_narrow_weighted_schur`. They cover independent dummy fits/current-Hessian
sandwiches, weights, offsets, generated terms, saved means, slopes and failure
atomicity. Ordinary fixest weights are not used as an FW inference oracle.

Two fresh bounded oracle cases use seed 202609226. One-FE N4100/K9 passes for
both artifacts and stock. Disconnected three-FE N3600/K6 initially failed
log-likelihood against an oracle that incorrectly retained 10 singleton rows.
The wrapper now independently applies recursive categorical singleton pruning,
checks the exact retained-ID hash and N, then calls the unchanged explicit
SciPy design oracle. All three fits pass beta/V/likelihood/deviance at N3590;
this does not adjudicate the larger disconnected discrepancy. Cascade,
empty-sample and nonpositive-PPML boundaries pass. The original failed oracle
receipt is preserved. The first oracle setup used N4096, which created a final
singleton worker and was rejected by the generator before any fit; the fresh
setup uses whole workers at N4100, without changing sample policy.

No additional heap buffer, matrix bank or persistent state is introduced.
Tracked candidate-minus-baseline peaks range from −709,048 to +3,790,608 bytes,
with exact equality in many cells. Threaded solver allocation overlap still
varies; exact peak neutrality is not claimed. The static allocation-source audit
still fails on documented drift, and its frozen inventory was not regenerated.
Weighted constrained-fit failures, nonretaining CG instability and deferred
Linux/constrained-memory confirmation remain open.

The next bounded investigation is single-residual strict PPML checkpoint
projection: the two-FE diagnostic spends about 40 ms there despite already
owning reusable topology. Its existing initial certificate is mandatory and
already attempted; the wide Schur checkpoint guard deliberately excludes this
RHS. Any narrower generalization needs current-weight probe evidence, unchanged
fresh residual certificates and fallback/admission checks. Do not extend raw
mean tuning or reopen closed serial-slope experiments.

`evidence.zip` contains 3,551 source-bound portable entries, including original
and archived hashes, captured controllers, plans, exports, diagnostics, checks
and log hashes. Data, binaries and raw licensed logs are excluded. No push or
release was performed.
