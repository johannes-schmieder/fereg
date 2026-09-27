# One-shot OLS slope owner: bounded development candidate

This report preserves a substantial two-partition OLS solver reuse experiment,
the rejected three-partition extension, and local scientific validation. The
fresh confirmation is registered for isolated SCC execution; it has not yet
produced an accepted timing distribution. This is not qualification or completion
of the broader performance repair plan.

## Attribution and implementation

The original dense two-partition N61440/K10/T4 diagnostic spends approximately
195 ms in initial absorption and 55 ms in final residual absorption. The first
candidate spends 41.5 and 6.5 ms respectively; the finite solve is approximately
1 ms. These are separate instrumented observations, not headline paired clocks.

[ADR 0231](../../../../docs/adr/0231-one-shot-two-partition-slope-owner.md)
documents a private OLS owner of the existing PPML coefficient-space Schur
projector. Its immutable design and normalized weights permit reuse of
rank-aware QR and admitted coupling products across initial RHS blocks and the
final residual. Columns stop independently. Every accepted reconstruction still
passes a fresh full certificate. The owner drops before inference and saved-FE
recovery. It changes neither the shared numerical kernel nor PPML routing.

Automatic two-partition slope fits receive the new owner in fast and low modes.
Explicit solver choices, categorical, single-partition and three-plus-partition
fits retain their previous routes. Numerical failures fail closed; allocation
or admission refusal retains the previous automatic route. Design construction,
weights, solver/QR banks and optional coupling/certificate scratch are admitted
before allocation. A new boundary test caught and repaired an initially
unmodeled report peak. Static allocation-inventory drift remains unresolved.

## Source-bound pilot, candidate01 only

Three alternating repetitions, original seed 104729, unit weights, IID
covariance, public residuals and fitted reconstruction. Median milliseconds:

| N61440/K10/T4 | Baseline | Candidate01 | Stock default | Stock strict | Paired C/B median |
|---|---:|---:|---:|---:|---:|
| Two dense partitions, fast | 294 | 88 | 122 | 188 | 0.299 |
| Two dense partitions, low | 403 | 123 | 121 | 188 | 0.302 |
| Three sparse partitions, fast | 701 | 756 | unranked | unranked | 1.068 |

Two-partition C/B ranges are 0.291–0.300 fast and 0.295–0.306 low. C/strict
ranges are 0.465–0.468 and 0.633–0.654. Strict stock uses the pinned, unmodified
fixest library with fixef.tol=1e-11. Default two-partition coefficient/VCE/RSS
checks pass, but requested fitted values fail the predeclared pointwise envelope;
default comparisons are therefore **unranked**. Three-partition stock remains
unranked for numerical/inference differences. All candidate/baseline science
checks pass. No denominator is silently switched to a more convenient reference.

The three-partition extension is **rejected**: its paired slowdown ranges from
0.993 to 1.090, median 1.068. Candidate02 restricts ownership to two partitions
and corrects the workspace report. Candidate01 timings are not transferred to
candidate02. Its observed process peaks fall from approximately 147 to 92 MiB
fast and 144 to 91 MiB low, but current-candidate memory confirmation is pending.

The pilot ran 19:02–19:04 local time, before the subsequently observed competing
Mac Stata work. No compilation/profiling ran alongside it. Continuous ambient
process monitoring was not yet enabled, so this remains a pilot, not the fresh
confirmation. Full per-repetition results and science are preserved in
`pilot01-results.json` and `pilot01-science.json`.

## Validation and fresh registration

Candidate02 passes 79 Rust tests (one ignored) and four licensed Stata fixtures.
The new 32-fit explicit WLS/full-sandwich oracle crosses two/three FE, multiple
slopes, unit/analytic/frequency/probability weights, fast/low, T1/T4, named FE,
residual and fitted reconstruction. Three FE is an excluded-path control.
Other fixtures protect categorical PPML, three-FE OLS and multipartition PPML
slopes. Existing kernel tests cover rank deficiency and changing weights.
The universal Mac build, binary audit and actual-plugin ABI checks pass.
`validation.json` also preserves unsuccessful setup/test attempts.

Fresh seeds 202609279 and 202609280 were frozen before data generation, with
seven cases and four rotations each. All local fresh attempts stopped before
any estimator fit because unrelated Stata work was active; that work was left
untouched. `scc-registration.json` explicitly amends execution to SCC and omits
stock reruns for the two scientifically incomparable excluded three-FE controls.
The other five cases retain default and strict stock arms. This is 192 fits,
with literal already-generated inputs, serial/wide/larger/low-memory boundaries,
a separate Linux build/smoke/oracle stage, and isolated alternating rotations.
Requested outputs, scientific envelopes and the original registration remain
preserved. Stock disagreements cannot receive timing ranks.

The baseline manifest binds parent 61a72fc8 and Mac plugin SHA-256
571eccb27a6cafb3239b163a37344ffb3e0e917d56b2b7d53c5a73762f250a50.
Candidate01 and candidate02 manifests bind 208 and 209 selected source/test
inputs, exact experiment patches and plugins 0632c104… and a6c44662… respectively.
These selected manifests are not a claim about all qualification inputs. The
SCC baseline is the separately accepted f8803410 Linux plugin 5dd3956f…; the
candidate build must pass its own exact-source Linux receipts before timing.
