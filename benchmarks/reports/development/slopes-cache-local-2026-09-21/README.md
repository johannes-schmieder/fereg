# Single-partition slope geometry checkpoint, 2026-09-21

Local development evidence for the `slope-cache10` artifact, plugin
`854222899e3a66f0c2535071dd64a2773dbcca324cabeb6ec8724d95c400807b`.
The isolated source uses baseline `0efa78bd` plus eleven recorded overlays.
The complete source map and pinned reference identity are in `evidence.zip`.
No installed package, remote branch or release qualification changes.

At N=245,760, G=49,152, seed 104729, an intercept plus one slope and three
alternating fresh-process pairs, the final screen reports:

| Regressors | Workers | Baseline median ms¹ | Candidate median ms | fixest median ms | Candidate paired ratio |
|---:|---:|---:|---:|---:|---:|
| 10 | 1 | 359 | 128 | 95 | 1.358 |
| 10 | 16 | 336 | 53 | 60 | 0.883 |
| 50 | 1 | 1,035 | 403 | 672 | 0.598 |
| 50 | 16 | 854 | 143 | 192 | 0.745 |

¹Baseline measurements are the separately captured original screen, not new
same-block pairs. The final block pairs candidate10 with candidate9 and fixest.
Every final application call and coefficient/covariance/RSS/sample comparison
passes. These are three-pair development screens; the milestone's final two
five-pair blocks and untouched validation seeds remain pending. Serial K10
still loses. `summary.json` retains every earlier primary screen and each pair.

The implementation caches rank-aware certificate geometry across RHS columns
and both fit projections, parallelizes independent groups, removes discarded
continuous-slope categorical encodings after authoritative sample classification,
and admits the existing loading/RHS pipeline for eligible slope requests.
Projection rank rules, fresh infinity-norm certificates, weight semantics and
fallback from untouched inputs remain unchanged. See [ADR 0087](../../../../docs/adr/0087-single-partition-ols-slope-geometry.md).

The instrumented cache8 serial K10 fit takes 86.1 ms: geometry/RHS preparation
16.6 ms, raw moments 13.3 ms, initial absorption 33.3 ms, finite solve 12.0 ms,
and final absorption 7.3 ms. Initial absorption includes 9.8 ms projection,
5.8 ms reconstruction, 3.2 ms norms and 14.3 ms fresh checks. These nested
timers must not be added twice. Subsequent contiguous loops reduce those costs.
Large weighted output diagnostics identify saved-effect recovery as a separate
remaining bottleneck (78.3 ms in one PW three-slope-only diagnostic).

Focused checks cover unit and supplied weights, one through three slopes,
slope-only/intercept models, zero/duplicate/nearly dependent channels, extreme
channel scales, unbalanced/interleaved membership, multiple RHS batches, thread
counts one/four, exact projection comparison, fresh certificate comparison and
unchanged-input budget/certificate rejection. Full-fit guards verify the sample
threshold, explicit MAP, low memory, iteration caps and the PPML/shared
projection boundary. Preparation tests retain exact samples, metadata and
topology with missing values, singleton deletion, weights and dual-use sources.
Mock SPI checks exercise missing slopes/groups and weighted inference through
the speculative RHS path.

Small Stata/R tests pass independent explicit-dummy IID, robust and clustered
oracles. FW tests additionally pass literal expansion for coefficients, rank
and IID/robust/cluster covariance. Large AW/PW/FW runs pass baseline comparisons
and independent saved-channel span, normal-equation, prediction and RSS audits.
The final artifact repeats weighted output science at N=16,385, K=6 and sixteen
workers. Four final-artifact licensed fixtures pass explicit terminal sentinels:
intercept/slope parity, multiple slope-only parity, insufficient-DoF failure,
and PPML saved slopes. Science-only timings are diagnostic, and some overlapped
compilation; they are not headline comparisons.

Earlier failures remain recorded. Initial output-oracle code confused saved
slope contributions with coefficients; its failure included the unchanged
baseline. A nested-cluster oracle incorrectly exempted slope channels as well
as intercepts. The corrected oracle agrees with both implementations. An R
worker initially passed unsupported `weights=NULL`. A large mock-SPI test then
identified an actual experimental bug: cached raw-weight norms were consumed
as normalized OLS norms. The robust statistic certificate rejected the fit
while legacy MAP passed. The final implementation restricts norm handoff to
unit weights; the original fixture and weighted application checks now pass.

Measured native allocation rises from 68.1 to 105.6 MB at K10 and from 225.4 to
373.0 MB at K50. The separate candidate matrix preserves untouched-input fallback;
group coefficient and geometry storage also contribute. The observed final peaks
are 1.014 and 0.891 times the unchanged public fast estimates. Explicit workspace
admission is implemented, but this checkpoint does not assert a complete
whole-action allocation proof or constrained-memory/platform qualification.

`evidence.zip` contains portable numerical exports, exact identities, generated
workers, plans, oracle receipts, test outputs and extracted native phase lines.
`archive-hashes.json` binds original and path-redacted bytes. Large output vectors
remain local, bound by `local-output-hashes.json` and independent full-vector
audits; raw licensed logs and generated data are not committed. Original local
artifacts remain under `target/ols-next-20260921`. Allocation-inventory refresh
follows the source checkpoint, without rewriting earlier manifests.

Next: improve final residual certification and saved-effect recovery, close the
narrow serial gap where practical, then complete the approved graph/weight/
inference/output coverage, narrow two-FE wins and measured auto-route revision.
