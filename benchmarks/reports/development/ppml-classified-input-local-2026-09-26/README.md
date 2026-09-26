# Borrow-bound PPML numeric selection

Local development evidence, 2026-09-26; not qualification. Adopt
**classifiedinput1** under [ADR 0214](../../../../docs/adr/0214-borrow-bound-ppml-numeric-selection.md).
The multi-FE 0.90-times-comparable-stock objective remains unfinished.

## Exact identities and change

Baseline **directstart1**, accepted at **ea2fed22**, plugin SHA-256
`aab4f8976e164d0f48c18d1c0f6948cd0ebe435a9cc28bf12c017b317e16a832`.
Candidate **classifiedinput1**, base ea2fed22 plus two Rust overlays,
`ppml/prepared.rs` and `ppml/prepared_estimator.rs`, plugin SHA-256
`6a9708ab9508de9e0c91394a9b0278f023476e586ec27c62cc3fb000aeed5b96`.
All **566 computational inputs** match the candidate. Sources, identities, builds,
ABI checks, declarations, controllers and independent audits are archived.

The authoritative numeric classifier now returns a private proof object holding
an immutable borrow of its complete input view. Candidate mask, values, missing
masks, source roles, weights and offset/exposure cannot change while that proof
is live. Scalar counts still inform memory admission; they cannot authorize
selection on another input. Selection consumes the proof rather than accepting
an independent input/count pair.

If every candidate survived classification, construct the existing row map and
drop ledger directly from the candidate mask after admission. Samples with
numeric drops retain the complete row visitor and count-agreement check. This
removes redundant validation using an ownership guarantee; it changes no domain
rule, error ordering, weight normalization, adjustment transformation, row order
or capacity bound. No observation-sized state is allocated before admission.
There is no new bank, pool, cache, allowance, OLS path or changing-generation reuse.
Independent separation, projection, model-space, rank, finite-solve, score,
objective, reconstruction, final-Hessian and transaction checks are unchanged.

Stock fixest remains unmodified and pinned at archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparisons use glm.tol=1e-10, fixef.tol=1e-11, matched thread settings, BLAS one,
robust or declared clustered inference and fitted means. Command clocks include
loading, estimation and output publication, excluding harness import/setup.
No compilation, audit, profiling, archiving or competing task-owned fit ran
alongside headline benchmarks. All current screens have clean process monitors;
there were no interrupted attempts or timeouts in this milestone.

## Attribution

Separate **inputprofile2/inputprofile3** builds bind the current baseline and
candidate. Two clean paired rotations per family establish:

|Boundary; medians ms|Million-row fast B/C|Million-row low B/C|Wide clustered factors B/C|
|---|---:|---:|---:|
|Initial authoritative classification|26.62 /26.39|26.55 /26.51|7.77 /7.81|
|Post-admission selection|29.38 /2.50|29.42 /2.46|8.54 /0.40|
|Core preparation, contains both above|60.94 /33.67|60.69 /33.72|18.19 /10.11|
|Separate initial generation construction|50.59 /49.81|49.87 /50.25|33.24 /33.41|

The authoritative visitor's call count falls from two to one in each fit.
The remaining scan is preserved. Do not add nested classifier/selection or model
builder durations twice. These instrumented phases are diagnostic, not stock
headline timings. Initial model construction remains a distinct opportunity.

## Public rotating performance

B=directstart1, C=classifiedinput1, S=comparable pinned stock. Medians are ms.
[distributions.md](distributions.md) preserves ordered observations, all ratios,
losses and protected OLS clocks. No source or guard was tuned on fresh validation.

|Development case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;983040/6/8|902 /881 /860|1.017–1.033|
|Same low memory|1003 /982 /871|1.124–1.131|
|Unbalanced three-FE factors, cluster;153600/19/8|579 /571 /708|0.804–0.819|
|Unbalanced three-FE, two slopes;61440/10/8|155 /152 /231|0.655–0.668|
|Two-FE, two slopes, low;30720/6/4|73 /72 /120|0.600–0.632|
|Two-FE numeric serial;245760/6/1|548 /543 /938|0.576–0.585|
|Small unbalanced three-FE;30720/6/4|72 /73 /81|0.890–0.934|
|One-FE protected control;61440/6/4|61 /61 /unranked|Mean discrepancy|

Three rotations give **19 wins and five losses** against baseline. Four of seven
multi-FE development families meet 0.90 in every pair. Million-row fast/low gains
are **2.3/2.1%**; retain the small three-FE median loss.

Fresh **seed 202609444** uses five predeclared rotations; this seed is consumed.
Every fresh direct C/stock scientific comparison passes.

|Fresh case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;737280/6/8|688 /670 /760|0.874–0.891|
|Matched low memory|766 /742 /761|0.974–0.982|
|Two-FE numeric fast;368640/6/4|429 /418 /656|0.624–0.641|
|Unbalanced three-FE factors, cluster;184320/19/8|694 /677 /3192|0.211–0.214|
|Unbalanced three-FE, two slopes;76800/10/8|195 /192 /300|0.633–0.660|
|Unbalanced three-FE, four slopes, low;38400/6/8|120 /117 /116|0.983–1.054|
|Two-FE numeric serial;245760/6/1|548 /542 /945|0.562–0.580|
|Weak_d3;61440/6/4|163 /163 /832|0.193–0.207|

Fresh B/C outcomes are **35 wins, three losses and two ties**. Six of eight
fresh families beat comparable stock by at least 10% in every rotation. The
737280 fast family reaches that target; low memory and four-slope low do not.
Large fast/low improvements against baseline are **2.6/3.1%**. The wide-factor
advantage is specific to this design and seed, not a universal fourfold claim.
The previous seed's unresolved stock comparisons remain separate evidence and
are not retroactively accepted by these results.

## Science, controls and memory

**93 distinct focused Rust tests pass**: numeric preparation 26, estimator 58 and
model generation 9. Tests compare the selection, ledger, weight/adjustment reports
and error messages against separately materialized preparation; cover equal-shape
input binding, weights, source roles, if/in and missing masks, numeric drops,
finite/domain failures, admission rejection and changing sample generations.
Final test receipts follow the formatting-only edit; earlier successful runs are
also retained and are not counted again.

**Seven original licensed fixtures pass**: runtime validator, hidden transaction,
72-fit categorical final oracle, three-partition slope oracle, higher-dimensional
oracle, public scientific contract and weight/inference. All production and
diagnostic builds pass ABI/panic-containment checks. This is not qualification.

All **216 main-screen PPML states** pass independent original-input sample,
score, model-space and objective checks: 204 production and 12 diagnostic states.
The small screen supplies **12 explicit-design Poisson/Hessian/sandwich checks**.
All **74 complete B/C pairs** pass and have exact coefficient, covariance, mean
and requested full-output exports. There is no algorithmic recurrence change.

Anchored comparisons pass **138/142**. Direct C/stock comparisons, including
identified requested outputs, pass **64/68**. There are no missing planned pairs.
Retain four unresolved comparisons: one small full-output residual mismatch and
three protected one-FE mean mismatches. They reproduce on both native arms and
remain unranked; no tolerance, requested output or sample is changed. The direct
stock auditor includes full identified outputs from the outset in this milestone.

**12 OLS coefficient/covariance pairs are exact.** Control medians B/C are
**66/67, 77/76, 14/15, 209/206 ms**. The small losses remain visible; there is no
universal timing non-regression claim. The OLS implementation is unchanged.

Selected production tracked peak allocation, modeled/observed scratch and
allocation-peak deltas are zero. Modeled bounds vary 0..+48 bytes, without changing
any memory formula or allowance. The proof object stores references and scalar
counts on the stack; it adds no observation buffer. No physical-cap, constrained
memory or platform qualification. Existing static-inventory drift, hidden-allocation
wire-minor assertion mismatch, baseline-reproduced strict checkpoint-count test
failure, weighted constrained-fit and nonretaining-CG failures remain open.
Frozen 77/16/111 definitions are unchanged. No SCC retry, stopped campaign restart,
push or release.

## Remaining cost and next implementation

The million-row development cases still need about **107/198 ms** to reach 0.90
of comparable stock. The authoritative first numeric scan remains about 26 ms;
model generation remains about 50 ms. The post-admission selection repetition is
closed; do not keep tuning its remaining 2.5 ms.

A separately built current-candidate **modelprofile1** subdivides model generation
into sample preparation, coding, singleton pruning and topology construction.
Its source and diagnostic evidence are archived under `next-model/`; it does not
supply headline timings or change production behavior. All six additional
diagnostic states pass independent science, bringing the milestone total to **222 public states** (204 production, 18 diagnostic).

Two repetitions on each family show generation construction **48.95/50.12 ms**
for million-row fast/low and **37.68 ms** for wide clustered factors in this finer
instrumented build. Within the million-row construction:

|Boundary|Fast / low ms|
|---|---:|
|Numeric sample preparation|7.80 /8.12|
|Availability FE membership construction|8.88 /8.97|
|Categorical singleton pruning|2.49 /2.53|
|Final categorical topology construction|15.96 /16.03|
|Containing final categorical preparation|24.93 /25.23|

The last row contains pruning and final topology; do not sum it again. The
full-retained numeric shortcut is already active, so model construction is not
another 26-ms generic numeric scan. Wide factors instead spend 17.17 ms validating
categorical domains, plus 3.86 ms final topology. Finer-profile clocks are separate
from the earlier coarser profile and production timings.

Source inspection finds an existing guarded one-FE transfer of available codes
and membership when the singleton sample is unchanged. Multi-FE construction
still finalizes new topology. The next coherent candidate is to generalize that
ownership transfer for eligible ordinary single-source partitions after proving
the no-singleton fixed point and exact source/level/row correspondence. Composite
partitions, duplicate source recipes, changed samples and incompatible ownership
must keep a validated fallback until their invariants are established. Do not
remove the guards blindly or change rank, weight, singleton or memory rules.
Independent topology/oracle tests and public OLS controls are required if this
shared path changes. This successor is not implemented in this checkpoint.

## Receipt archive

The content-addressed archive contains exact sources, builds, controllers,
process-monitor revisions, ordered timings, independent audits, tests, isolated
profiles and current-candidate model-construction attribution. Generated DTA,
binary row outputs and compiled artifacts are hash-bound and retained locally.
Licensed logs retain protocol/sentinel lines only; paths are sanitized. Restore
and verify with `restore_receipts.py NEW_DIRECTORY`; the adjacent hash and
verification receipt identify the immutable container.
