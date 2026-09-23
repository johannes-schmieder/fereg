# Multipartition slope solver replacement — Mac development, 2026-09-23

The accepted `slschur3` artifact replaces repeated residual-space slope MAP
with current-weight, rank-aware coefficient Schur/CG absorption, independent
column stopping, balanced RHS scheduling, and dense response/control fusion.
It also removes the full *input* certificate used only for an early return;
original-RHS reconstruction and the full fresh final certificate remain.
[ADR 0123](../../../../docs/adr/0123-ppml-coefficient-schur-slopes.md)
records the algorithm, checking decision, ownership and fallback boundary.

## Identity and measurement scope

All **531 computational inputs** at this checkpoint match `slschur3` exactly.
The archive contains their hashes, changed source files, controllers, declarations,
audits, protocols and artifact identities. Its native plugin SHA-256 is
`ed5e7855efbea24863d6ab9d6c0dcecf260459339be75563d19feaa0bd689467`.
The source base is `7394c7d6`; baseline `ppmlgeom1` has the unchanged 529-input
production computation from the preceding geometry milestone, plugin
`895b2ae2ce57094a1290255d6e6f02ae15f7e374441e340caf5052d490003875`.
Intermediate full-normal, Schur, checking-schedule and diagnostic artifacts
retain their own identities. Their timings do not become final-artifact claims.

The stock comparator is pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
installed library SHA-256
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
Every ranked PPML comparison uses `glm.tol=1e-10`, `fixef.tol=1e-11`, robust
inference and timed fitted means, with matching threads and samples. Ordinary
fixest weights are not used as a Stata frequency-weight inference oracle.
Prior default-reference accuracy failures remain separate evidence.

Public clocks exclude data generation/import, process startup and warmup, and
include estimation and requested mean output. Native SPI loading remains part
of the native command. Rotations alternate variant order by case/repetition;
there was no compilation, profiling, archiving or competing fit during headline
blocks. The 225 completed attempts across development, validation and controls
all have passing application results, no timeout, overlap or monitor error.
Scientific acceptance is separately audited. These are bounded local development
receipts, not qualification, Linux confirmation, or a constrained-RSS claim.

The [receipt container](receipts-content-addressed.zip) preserves 6,905 logical
files as 1,747 distinct payloads; only exact duplicates are deduplicated. Its
SHA-256 is `7d41ec832b8c5961acf037a3416f008fb12a5359d81ed37cdabeeb903ba1b899`.
[restore_receipts.py](restore_receipts.py) verifies the container and every file
hash while restoring the original logical paths into a new directory. The
full original container is preserved locally; the committed representation
loses no logical bytes. Sanitization hashes distinguish original from archived
text, and raw licensed logs/binaries are excluded.

## Final public results

Milliseconds are median [minimum–maximum] from three rotations. The first two
rows reuse the established seed 202609300 designs. Later rows use fresh seed
202609310, selected before execution. No guard or tolerance was tuned to them.

| Case, robust + means | Baseline | Final | Stock | Final / stock |
|---|---:|---:|---:|---:|
| Two slope partitions, N30720 K6 T4 | 727 [727–734] | 213 [212–215] | 120 [118–120] | 1.78 |
| Unbalanced three FE, two slope partitions, N30720 K6 T4 | 524 [524–526] | 193 [192–194] | 117 [115–120] | 1.65 |
| Fresh two slope partitions, N30720 K6 T4 | 768 [725–779] | 216 [214–216] | 120 [116–120] | 1.80 |
| Fresh unbalanced three FE, N61440 K10 T8 | 1661 [1633–1662] | 447 [446–464] | 231 [230–232] | 1.94 |
| Fresh two slope partitions, N30720 K6 T1 | 1347 [1344–1351] | 312 [304–314] | 325 [321–327] | 0.96 |
| Fresh unbalanced three FE, N30720 K6 T4, native low memory | 689 [681–718] | 355 [354–365] | 117 [116–120] | 3.03 |
| Fresh multiple slopes, N7680 K6 T4 | 633 [621–637] | 74 [74–75] | 52 [51–56] | 1.42 |

The established cases improve 70.7% and 63.2% by ratio of medians. Fresh
improvements range from 48.5% to 88.3%. The serial case is close to stock and
slightly faster in this block; other rows remain losses against stock. The
archive retains every observation, pairing, scientific check and earlier block,
including the wider spread and baseline drift between blocks.

Two predeclared small difficulty pilots on fresh seed 202609308 also pass both
independent Poisson/Hessian and public-state checks: weak two-FE slopes take
15 ms versus stock 23 ms; disconnected three-FE slopes take 39 ms versus stock
356 ms. These are single runs, not distributional speed claims. The historical
larger disconnected stock covariance discrepancy remains unresolved. No stopped
large disconnected campaign was restarted.

## What caused the gap

The pinned `demeaning.cpp`, `estimation.R` and `miscfuns.R` were checked against
the archive before inspection. An isolated instrumented build lives in a
separate library; it never supplies headline clocks. Source inspection confirms:

- `FEClass` prepares weighted group slope Grams and triangular factors once per
  weighted demeaning call. Native generic `ProjectionWorkspace` rebuilt and
  solved those systems on every effect projection.
- Stock iterates FE coefficients, with specialized two-FE cross maps, reverse
  block Gauss–Seidel, Irons–Tuck acceleration, independent column stopping,
  periodic stabilization/SSR work, and additional three-plus-FE strategy.
- Stock passes prior fitted means into successive weighted calls. Native slope
  sessions previously warmed controls but did not fuse response and controls.
- Stock's final inference uses the last weighted linear fit; it does not perform
  fereg's additional independently certified exact-final-Hessian synchronization.
  This is an organizational difference, not proof of the cause of any previous
  comparator discrepancy.

On the same seed300 public inputs, the captured old native path makes 20/16
absorption calls for two/three FE, 1356/1100 effect projections, 3492/2820
RHS-by-effect applications, and 133/71 full certificates. The final diagnostic
makes 12/10 absorptions, prepares geometry once in each, and executes 12/10 full
certificates. Its 67/53 columns take 410/299 total Schur operator applications
and one fresh reconstruction each. One Schur application is a forward map over
remaining partitions, exact projection off the eliminated partition, and a
transpose over remaining partitions; it contains no repeated group factorization.

Using those counted sweeps, their fixed symmetric order and the captured group
cardinalities, the old path builds 4,686,336/271,480 local slope Grams. The new
path builds 62,976/6,170 rank-aware slope QR blocks, versus stock's 31,488/3,085
slope factor blocks. These derived preparation-event counts exclude the third
categorical partition. Thus much of the original gap was repeated local setup,
not a requirement to compute a more accurate mathematical projection.

Stock's corresponding public diagnostics make 6/5 demeaning calls and 42/35
column solves. Two FE use 868 coefficient GS evaluations and 1820 cross maps;
three FE use 539 GS evaluations, 1617 group solves and 3339 coefficient-sum
reconstructions. These are different operations: neither a package's reported
iteration nor a GS evaluation equals a native symmetric sweep or Schur step.
Native outer IRLS reports 7/5 iterations versus stock 6/5 in these diagnostics.
Warm-start fusion and independent column stopping reduce repeated compatible RHS
work; stronger checkpoint and Hessian acceptance still require additional work.

Measured native phase changes support the attribution to projection/checkpoint
organization rather than setup or inference. For the final wide fresh row,
reported native time is 430 ms: preparation 32, ordinary projection 201, finite
solve 26, objective/line search 26, strict final work 110, inference 1.5 and
publication 10 ms. For low memory it is 339 ms: preparation 14, projection 106,
finite solve 84, strict final work 91, inference 16 and publication 5.6 ms.
Refresh, separation and remaining native overhead account for the balance.
Whole-command medians additionally include frontend/final-store costs outside
that native timer; those differences are not labelled pure loading time.

## Identical frozen linear problems

Synthetic captures bind weights, RHS columns, FE codes/slopes and initial
responses exactly. An independent explicit-design weighted LSMR oracle checks
projection and model-space feasibility; local QR checks independently reproduce
the projection bound. The previous disconnected frame also has the separate
dense-SVD/QR diagnosis. All final native outputs pass the strict `1e-12` local
projection gate and the predeclared independent WLS/model-space envelopes.

At N30720, the changed-weight two-FE six-RHS frame takes 44 symmetric MAP sweeps
in the old replay, 32 full-normal coefficient CG steps, and 15 Schur steps.
The three-FE eight-RHS frame needs 30 full-normal versus 16 Schur steps. The
935-row disconnected frame that exhausted 16000 old MAP sweeps now passes in
257 Schur steps at `1e-12`; full-normal CG needed 502. Coefficient-space
Irons–Tuck needed 1435–2288 iterations on that difficulty frame and was not
chosen. The full-normal route was replaced, not retained as another production
solver layer.

Single diagnostic native call times are 14.4/19.9 ms for the final two/three-FE
frames, plus 0.36/0.30 ms workspace construction. The corresponding stock C++
diagnostic calls take 18.4/38.8 ms, with total R `demean` calls 22/42 ms and
`tol=1e-11`; independently checked stock residuals also pass `1e-12` here.
Native design indexing was already prepared. These clock boundaries differ and
are not public-command rankings. The old two-FE MAP replay was 81.8 ms at the
same strict native tolerance. Detailed per-column operations and setup costs
are retained rather than equating recurrence counters.

The same mathematical WLS problems also test OLS applicability: the existing
owned automatic route selects CG and takes 55.7/59.7 ms in single diagnostics;
its explicit-design/model-space audits pass on these frames. The new shared
kernel is promising for one-shot OLS. Public OLS routing is deliberately
unchanged: a borrowed-design adapter and corresponding full-fit admission are
needed to avoid introducing another design copy or an unaccounted slow fallback.
This does not resolve the separately documented generic nonretaining CG issues.

## Verification and memory

54 distinct focused Rust tests pass on the final source, covering the new
analytic weighted projection, duplicate/zero channels, slope-only and intercept
terms, disconnected products, two/three FE, changing weights, extreme common
weight scaling, thread reproducibility, admission boundaries, capacity reuse,
short-solve/nonfinite rejection, generation invalidation, strict rollback,
reconstruction, saved-effect allocation failure and current-Hessian recovery.
Three source-bound licensed fixtures pass with process rc, Stata rc and terminal
sentinels: the new explicit multipartition Poisson/Hessian oracle for unit/FW/PW
and robust/cluster inference; saved slope outputs; and separation methods.
Actual-plugin ABI/panic-containment checks pass for the accepted artifact.

Independent small public Poisson/Hessian pilots pass before and after the check
redesign. The final difficult-topology pilots pass independently as well. All
final established/fresh public comparisons pass coefficients, covariance, means,
sample, objective replay, fresh scores and model-space checks. Bitwise equality
with the old slope recurrence is neither expected nor used as an acceptance
condition. Rank, samples, weight semantics, DoF, objective, reconstruction,
separation, final Hessian and transactional publication remain enforced.

Added tracked allocation peak across the five final fresh cases is respectively
4.34, 11.29, 3.25, 3.35 and 1.41 MB (decimal). This includes the normalized RHS
bank, square-root weights, per-worker coefficient/observation banks and prior
response storage, replacing the old optional projection bank. The plan accounts
for capacities through strict linear work and releases them before inference
and recovery. Local QR bases are rebuilt on every call, including low-memory
blocks; no stale PPML weight cache is possible. The coefficient vectors retain
slots for the eliminated partition, a conservative memory cost that could later
be reduced. Static allocation inventory drift remains an open failure; this is
not a whole-action allocation proof or constrained-machine validation.

## Excluded paths, failures and remaining work

Categorical three-FE PPML and four OLS controls have exact scientific exports
against baseline. First-block medians (baseline/final ms) are categorical
198/199, multipartition OLS 67/69, categorical three-FE OLS 44/42, one-FE saved
outputs 16/16, and generated-factor OLS 50/52. A predeclared five-pair confirmation
of the two 2-ms losses gives 68/69 and 51/50, with overlapping distributions and
exact exports. Keep the small OLS timing losses visible; these bounded controls
do not establish universal absence of performance regressions.

The initial Python Schur experiment used a relative threshold against a nearly
zero RHS and exhausted its cap on an exactly fitted constant; its naive diagnostic
also masked a NaN. Those outputs are rejected, retained and never used as science
or speed evidence. The native implementation uses original-RHS scaling and
explicit finite checks, with independent oracles. A corrected Python script is
retained but was not used for production selection. Initial manual reference
replays accidentally forced T1 and overlapped compilation; their clocks are
excluded, and separately named uncontended T4 replays supply the diagnostics.
A missing controller CLI argument failed before any control fit; its log remains.

Final instrumented native absorptions spend 113/101 ms for the established
cases, including 22/13 ms geometry and 37/38 ms full final certificates. The
wide diagnostic spends 280 ms in absorption, of which 28 is geometry and 128
certification. Low memory makes 19 absorptions, with 24 ms geometry and 42 ms
certification in 157 ms total. Timers nest; per-column clocks overlap across
workers and must not be summed into wall time. These are diagnostic inputs,
separate from fresh headline rows.

The next bounded target is the remaining fresh certificate work (especially
serial raw-moment/span scans in wide fits), followed by low-memory finite solving
and repeated response-only projection. Existing generic certificate tiling/group
parallelism offers a mechanism to evaluate without changing acceptance. Same-frame
geometry reuse across low-memory calls would need explicit generation authority;
a pointer match is insufficient. OLS should reuse this kernel through a borrowed
owner rather than duplicating its algorithm. None of these potential gains is
claimed here. Linux T1/8/16, physical memory constraints, the allocation inventory,
known constrained-fit/CG issues and prior comparator discrepancies stay open.
No push, release, qualification, SCC retry or stopped campaign was initiated.
