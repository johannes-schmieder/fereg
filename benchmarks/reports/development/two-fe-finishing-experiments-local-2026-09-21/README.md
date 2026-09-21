# Two-FE finishing experiments and route comparisons, 2026-09-21

These are local development experiments, not a completed optimization milestone
or an accepted replacement for the routing policy. The archive preserves 663
application records: 631 successes, 19 command timeouts and 13 worker failures.
It also preserves failed warmups, missing-result attempts and their plans.
There are 504 native calls and 159 default-fixest calls. Scientific acceptance
is separate from application completion and elapsed time.

## Changes under investigation

The successive native snapshots preserve coefficient products after an
unreconstructed Schur failure and avoid retrying an already exhausted Schur
solve on the final residual. Numerical residual construction binds source
columns once and avoids a redundant scale pass when a conservative bound is
safe, with the original scan retained for overflow and underflow cases.
Guarded narrow weighted fits overlap final projection with independent finite
work. The saved-output CMG experiments retain correction coefficients and
reuse the same-weight graph after freshly certifying the final input.

The correction investigation exposed a pre-existing nearly-absorbed-input CG
span error that FE-moment checks alone do not detect. The admitted draft
reconstructs the projection from its recorded coefficients instead of
subtracting nearly equal vectors. Its known-solution, weight, thread, output
and memory-guard fixtures pass. Earlier failing tests remain in `checks.zip`.
The explicit and insufficient-budget nonretaining CG case remains unresolved;
this experiment does not claim a general CG repair.

The current graph snapshot is `m5`, plugin
`a07732eea15a47a185903bf7403cd74f111a5eebbd5e558fbb371f02728bef56`.
Its owner flag excludes shared callers from final graph reuse. Every accepted
projection still needs the existing fresh observation-space certificate.
Bulk graph accuracy is chosen from the current input's measured FE defect;
the final certificate tolerance is unchanged. These drafts remain uncommitted
pending the rest of the approved checks.

## Measured results

The 32-cell, three-pair ring/cycle screen covers K6/K50, unit/AW/PW/FW, robust
without outputs and two-way clustering with full outputs. All applications
complete. All 24 comparable cells beat default fixest; eight FW cells are
native controls, since ordinary fixest weights are not a Stata FW-inference
reference. All 64 native direct-inference checks, 192 within-variant exact
repeat checks and 48 m5 output audits pass. All 24 default-reference direct
oracle discrepancies remain failures. A unit-ring robust m5/m4 paired ratio
of 1.088 remains a control requiring confirmation.

The preceding narrow screen gives unit/AW/PW degree-six clustered full-output
medians of 107/116/124 ms versus fixest 132/157/162 ms. Shuffled AW K6 is 262
versus 295 ms. All 21 native direct-inference and nine output audits pass.
These are three-pair screens, not the two final independent five-pair blocks.

Whole-command route exploration attempts six solver families on twelve
dense/degree-six/sparse/weak/ring/cycle unit/AW K6 full-output cells. Fifty-two
calls pass independent science; twenty fail at the application boundary.
Every automatic call passes science. One dense-unit automatic call has a
possible external-process overlap and is excluded from route rankings.
The weighted-dense explicit-CG negative-Hestenes-potential error reproduces on
the original baseline and m5. Unit ring, unit cycle and AW cycle automatic
times are respectively 1.209, 1.241 and 1.449 times the fastest valid family in
this one-repeat exploration. The complete solver reports distinguish Schur-CG
from explicit symmetric CG.

An isolated `cap40` snapshot reduces only the narrow Schur continuation cap
from 64 to 40. All sixteen focused Schur tests pass. Its fourteen-cell,
three-pair screen passes 28 native direct-inference checks, 84 exact native
repeat checks and 21 output audits. Three weak-graph cells have possible
external-process overlaps and cannot establish timing acceptance. In clean
unit-ring cells, paired candidate/m5 ratios are 0.937 for robust inference and
0.976 for clustered full outputs. Dense times change little. These results do
not establish the proposed automatic/fastest-family margin; the production
cap remains unchanged. Both new routing validation seeds remain untouched.

## Memory and remaining evidence

The first memory attempt mishandled the normal minimum reserve and used an
unsupported harness choice; its failed attempts remain recorded. At 300 MB
usable, preparation rejects the cycle K50/AW/full-output fit on both baseline
and m5. At 400 MB usable, fast mode rejects with r(909), while low and auto
reach their 120-second limits. A sampled low-memory m5 run is in the retained
LSMR owner's CG continuation. Further traced attempts show that the limited
budget excludes LSMR; explicit LSMR returns r(909). This does not validate
constrained final graph reuse or complete the memory proof.

The unconstrained 20-second low-memory diagnostic is also preserved as a
timeout. A subsequent longer run, outside this archive, completes; that short
timeout alone must not be interpreted as a solver stall. Follow-on low-memory
product-retention and compact-slope-Gram experiments are separate artifacts.

The protected primary/holdout/native-control matrices, complete expanded large
coverage, final repeated blocks, allocation-proof work and focused SCC/Linux
confirmation remain open. SCC authentication failed before any remote work.
Nothing was installed, pushed or released.

## Archive contents

`evidence.zip` retains requests, results, numerical exports, generated workers,
input hashes and attempt records. Large output vectors and licensed logs stay
local; their hashes, sizes and independent audits bind the omitted arrays.
`sources.zip` contains twelve artifact identities, checked source overlays and
available controllers; `instrumented-sources.zip` supplies other files changed
by isolated diagnostic instrumentation. Together they reconstruct each source
identity from its base commit. `checks.zip` retains independent audits, timing summaries, focused
test attempts and baseline-localization receipts. Hash maps distinguish original
local bytes from portable path substitutions. Historical receipts are unchanged.
