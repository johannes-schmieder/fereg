# Reference-operator PPML proposal closeout

Local Mac arm64 development evidence, 2026-09-25. Not qualification.
[ADR 0197](../../../../docs/adr/0197-ppml-reference-operator-closeout.md) rejects
the integrated modified-Newton candidate. Accepted runtime remains
**e7731129/finalreplay1**. All four changed core files are restored, and all
**558** captured accepted computational inputs match. The multi-FE <=0.90-stock
goal remains active; neither a reusable-factor prerequisite nor this negative
experiment completes it.

## Source and computational organization

Both integrated drafts are based on **2d720820**, with overlays to
`fe/map.rs`, `fe/slope_coefficients/joint.rs`, `ols/linear_session.rs`, and
`ppml/prepared_irls.rs` under `crates/fereg-core/src/`.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
|finalreplay1|Accepted comparator|84db18f8e91f31eb64f26c854df98609b3722a3d49828edbad040118b3f188b3|
|chord1|Integrated draft; checkpoint-routing test failed|eb5ee850ad4da54fb0f01d98797f06c73b9b07767851cbce794f8b5897962369|
|chord2|Measured candidate; rejected|21bfbdf9da84b56ba609e8b2a92e8325b95c71e7aad2b8a96cd3a5d8b38b445a|
|nonlinearseed1|Isolated first-accepted-state capture, base f8a3fef6|fea1a4147359cdd14c6d386c8058096ee38b93b3f4fe8f92b25ab9df11cde266|

The candidate retains a reference-weight joint operator after the first ordinary
Newton step. Subsequent surrogate responses use current eta/mu and the fresh
Poisson gradient. Exact dependency identity permits reuse of FE geometry,
finite products and factors; changed dependencies or failure revoke it. The
streamed finite RHS uses original finite columns against the freshly projected
response. New residuals and finite scores are independently checked each call.

The current accepted-state objective and outer scores remain authoritative.
Numerical refusal, a halved step, stalled score progress or a strict checkpoint
disable reference reuse. Failed line-search proposals can retry once through
the exact canonical path. Strict/final work restores current q*mu and the
canonical response. No reference Hessian supplies final inference authority.
Low original-column owners are eligible; fast, exploratory separation and
allocation-equivalence routes are excluded. Chord2 additionally requires the
canonical-final policy. No sample, tolerance, rank, DoF, inference, separation
or transactional publication rule is weakened.

## Preliminary alternatives, not headline timings

The earlier pending diagnostic captures the exact first accepted eta and beta
on development seed347 without altering estimation. Its local recipes and
protocol traces are now archived. Two independent proposal organizations were
examined from those states:

- Nonlinear coefficient-block updates alternate finite and FE updates, with
  objective-checked acceleration. The standalone Rust diagnostic needs 13
  accelerated sweeps on the large categorical case and nine on the narrow
  case. Seven serial repetitions have medians about 107.2 and 19.1 ms for this
  proposal work alone. These are standalone diagnostic clocks, not monitored
  public or stock comparisons; the probe has no covariance/publication path.
  Its first failed draft compile and corrected source are preserved.
- The dense Schur modified-Newton diagnostic needs four steps/one setup on the
  large case versus three steps/three setups with fresh Newton, and three
  steps/one setup versus three steps/three setups on the narrow case. Terminal
  coefficient/mean comparisons against accepted exports fit the registered
  envelopes. This is feasibility evidence, not an independent complete PPML
  acceptance result. Its large reduced matrix occupies 8,421,408 bytes and is
  expressly excluded from the production low-memory design.

These results selected the integrated chord candidate. They did not establish
how much public-command cost would disappear, nor how a strict checkpoint
would behave on a different FE geometry.

## Complete public measurements

Pinned unmodified **fixest 0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
uses `glm.tol=1e-10`, `fixef.tol=1e-11`. OpenMP/Rayon match requested threads;
BLAS remains one. Robust inference and fitted means are included. Generation,
data import, process startup and warmup precede the clock; public loading,
estimation, checks, inference and requested output remain inside it.

All headline blocks use three alternating B/C/stock rotations on development
seed **202609347**. The source is unchanged between the initial pilot and its
predeclared width/three-FE extension. No compilation, profiling, archiving or
competing fit overlaps headline timing; process monitors report no overlaps or
errors, subject to their sampling limits. These are not fresh holdouts. No
favorable rerun or source tuning follows these clocks.

Milliseconds in declared repetition order:

| Block; case N/K/T, mode | Baseline | Candidate | Stock | B/C/S medians | C/stock | C/B wins/losses/ties |
|---|---|---|---|---|---:|---|
|Pilot; disconnected categorical 61440/6/4, low|110,112,112|110,110,109|97,99,98|112/110/98|1.122|2/0/1|
|Pilot; narrow categorical 24960/2/4, fast|39,39,41|39,37,39|34,34,30|39/39/34|1.147|2/0/1|
|Pilot; two slope partitions 30720/6/8, low|78,80,81|75,75,77|79,79,81|80/75/79|0.949|3/0/0|
|Pilot; two slope partitions 30720/6/8, fast|78,80,80|78,83,80|76,78,80|80/80/78|1.026|0/1/2|
|Extension; wide categorical 23040/36/8, low|85,86,89|83,85,92|127,127,128|86/85/127|0.669|2/1/0|
|Extension; unbalanced three-FE slopes 61440/10/8, low|200,195,196|314,310,310|234,231,229|196/310/231|1.342|0/3/0|

The candidate has nine paired wins, five losses and four ties across these two
separate blocks. Wide controls are its only median meeting 0.90, a boundary the
baseline already meets. The unbalanced baseline also meets the target in this
development block; the candidate destroys that advantage. The unchanged narrow
and fast-slope routes remain controls, not attributable optimization gains.
Do not pool these clocks with seed430 or transfer its 27.4/8.4-ms planning gaps.

## Why reference reuse does not close the gap

The first native trace confirms reference proposals on iterations 3 and 4 for
the two eligible pilot cases, with exact-weight initial/strict construction.
The excluded controls never use them. The large categorical case still has
four outer iterations and unchanged reported inner work; low slopes save one
inner iteration. Reuse is functioning, but most necessary work survives it.

Phase telemetry medians in milliseconds, interpreted separately rather than
summed as disjoint attribution:

| Case; phase | Baseline | Candidate |
|---|---:|---:|
|Large categorical; FE projection|23.771|20.050|
|Large categorical; weight/response refresh|2.412|3.946|
|Large categorical; strict/final solve|17.127|16.961|
|Two slopes; FE projection|18.456|13.825|
|Two slopes; weight/response refresh|1.100|1.843|
|Unbalanced three-FE slopes; FE projection|53.004|70.409|
|Unbalanced three-FE slopes; strict/final solve|47.376|128.869|

Response validation/construction offsets some avoided preparation. These phase
measurements are consistent with modest two-FE savings, not removal of the
large ordinary or canonical boundaries. They are not exact causal decompositions
of the rounded public clocks.

The closing unbalanced trace explains the regression. Current score falls from
1.593e-6 after iteration 2 to 3.270e-8, 6.787e-10 and 1.401e-11 under three
reference proposals. Apparent convergence occurs twice at iterations 4/5.
The independent strict solve then finds beta movement 1.178e-6 on its registered
relative scale and correctly rejects profile consistency. Reference reuse is
revoked, exact Newton resumes, and iteration 7 passes. Outer counts rise **4→7**
and inner work **54→137** in all three headline pairs. A zero fallback counter
does not imply absence of checkpoint work: this is a strict-profile correction.

Do not suppress that check, loosen its bound, or add a favorable topology guard.
The candidate is rejected rather than tuned to this case. The next opportunity
is reducing repeated preparation and independent-check passes inside the
exact-weight algorithm, including canonical construction; narrow fits separately
need lifecycle savings because their fast route is untouched here.

## Validation and memory

Chord2 passes **14 distinct Rust tests** covering analytical weighted joint
oracles, generation/column invalidation, changed frames, strict rollback and
current-Hessian truth, memory refusal, failed warm/canonical recovery, line
search and exact-weight identity. The new response arithmetic test checks an
independent rational normal-equation identity, common weight scaling, zero
outcomes and nonrepresentable/domain refusal. An earlier compile failed on an
unavailable diagnostic getter; its log remains. Chord1 failed a checkpoint-count
assertion with canonical-final disabled. The scope correction leaves that
assertion unchanged and passing; both source snapshots are retained.

All six licensed fixtures pass with explicit successful results and zero process
and Stata return codes: iterative canonical, multi-partition slopes, three-
partition slopes, public scientific contract, weights/inference, and separation
outputs. They include independent Poisson/Hessian/sandwich and weight-semantic
oracles. Builds pass the actual-plugin ABI/self-test and intentional panic
containment harness.

The candidate's pilot, extension and two trace blocks contain **64 passing public
states, 41 passing baseline comparisons and 18 passing direct stock pairs**.
No scientific discrepancy, timeout, application error or sampled competing fit
occurs in these blocks. The earlier seed-capture diagnostic is separate evidence.
No new OLS campaign, warmed-allocation audit, physical-cap proof or cross-platform
qualification is claimed. Restoring all accepted computational inputs protects
the runtime at closeout; it is not a new test result.

No additional observation-sized bank is allocated. Peak telemetry rises **96
bytes** on admitted K6 joint owners, **128** on K10 and **336** on K36; the
narrow excluded owner is unchanged. Scratch is unchanged. Selected telemetry
has additional occasional 48-byte variation. The identity vector and metadata
are charged in workspace admission. Known allocation-inventory drift,
constrained-fit/nonretaining-CG failures, older three-FE coefficient discrepancy,
baseline checkpoint-count assertion and deferred platforms remain open.

## Immutable evidence

`receipts-content-addressed.zip`: **4,365 logical receipts, 1,149 payloads,
11,966,371 bytes**. SHA-256:
`f7755c419ef7eff9368dfa324b4bb47ead24f3a73b9462965d1e8a011b9b5a3b`.

Run `python restore_receipts.py DESTINATION` to restore the complete source
snapshots, build/test recipes, rejected patch, declarations, monitors, scientific
audits, timing vectors and protocol traces. Sources and receipts are hashed;
machine paths are sanitized. Data, binary artifacts and diagnostic array banks
are omitted with hashes and reproduction recipes. Raw licensed logs are excluded.
`production-restoration.json` verifies the accepted source restoration.
The archive is restored and checked before the closeout commit. Seed431 remains
unused. No push, release, stopped campaign restart or SCC retry occurred.
