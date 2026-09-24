# Fixed-effect kernel

Status: implemented in the public 0.5.0 OLS path; exact-SHA platform and
performance qualification remains pending.

The safe core provides:

- indexed queue-based singleton deletion to a fixed point;
- multipartite connected-component labeling;
- one-/multiway weighted projections;
- intercept, slope-only, intercept-plus-slope, and multiple-slope local blocks;
- deterministic batched symmetric MAP, genuine weighted CG, and explicit
  `within`/IT/SQUAREM/LSMR support;
- normalized absorbed-sum and saved-FE recovery; and
- fresh basis-invariant weighted FE-span projection certificates.

Each effect stores compressed level-to-observation membership. MAP allocates
and reuses bounded projection workspaces. Rank-deficient local slope blocks use
pivoted elimination; unidentified parameter directions may be normalized, but
fitted projections must remain identified.

Automatic owned-slope MAP/CG projections may split independent group solves
from disjoint row subtraction on an installed team (ADR 0118). At least 16,384
rows, nonempty RHSs and more than one worker are required. Raw Gram accumulation,
pivot/rank rules and each entry's FMA/subtraction order remain unchanged. An
optional admitted coefficient bank is reused within one solver invocation;
every sweep recomputes its coefficients. Allocation refusal uses the serial
workspace. Direct solvers retain their execution. The reusable publication
PPML MAP owner may admit the same projection storage under ADR 0119, refreshing
all coefficients on every projection. No numerical state crosses a weight or
sample generation.

For multi-effect PPML MAP designs with slopes, an optional certificate bank
retains the identical weighted QR bases only within one absorption call
(ADR 0121). Validity starts false on each call's stack, even when weights
appear unchanged. Residual norms, span projections and raw moments remain
fresh at every check. Basis and rank capacity are admitted for the session
lifetime; budget or allocation refusal keeps uncached certification. This
does not change rank, convergence, final-Hessian or publication requirements.

Publication PPML with multiple effects and absorbed slopes may instead admit
one coefficient-space Schur workspace (ADR 0123). It shares the current-weight
rank-aware QR bank, eliminates the largest partition exactly, and solves the
remaining normal operator with independent per-column conjugate gradients.
Fresh reconstruction and the full residual certificate remain acceptance gates;
cheap coefficient moments only schedule them. Dense response/control warm starts
share a single absorption call. Under ADR 0124, the owning session's explicit
sample/topology/weight generations permit local QR reuse across compatible
calls. Unframed calls, changed generations and failures revoke that authority.
Every RHS still receives fresh acceptance; coefficient solutions are not cached.
An independently admitted group result bank permits tiled parallel fresh
certification with the original scalar arithmetic and ordered merge. Refusing
that bank preserves the Schur solver with serial certification. Refusing the
solver itself retains the bounded generic MAP path.
The public MAP-owned engine identity includes this slope kernel; its iteration
count is the maximum column Krylov count, not a symmetric-MAP sweep count.
Categorical, one-effect, mu-separation and public OLS routing are unchanged.

Under ADRs 0130 and 0134, admitted ordinary PPML slope directions couple the finite
controls to the same FE Schur operator and use one block-preconditioned Krylov
solve. This call reports its joint Krylov count. Reconstructed residuals receive
fresh FE and original-input finite-score checks; the result carries no bread or
within-inverse authority. Initial rank construction remains canonical; eligible
strict acceptance uses a joint Newton witness followed by one independent
canonical final rank/inverse/Hessian construction (ADR 0131). Fast mode reuses
the normalized RHS bank. Low mode streams finite products through admitted
worker buffers, independently of its smaller RHS pool. Optional cross-products
and a residual vector are admitted separately. Failed acceptance revokes geometry
authority before canonical retry. See the ADRs for representative/weight/basis
lifetimes and the mixed-thread allocation-audit limitation.

Under ADR 0144, low-mode joint products borrow original prepared controls instead
of repeatedly scanning retained scratch. They are FE-equivalent representatives;
canonical rank/inference still require retained within products. ADR 0145
replaces serial Hessian row tiles with independent column reads and Hessian rows
on the existing workers, preserving each entry's normalization and observation
order without new storage. A single worker accumulates the row's independent
entries together while retaining direct column loading. Current-weight geometry,
fresh acceptance and numerical fallback are unchanged. Original-input finite score/mean checks use the existing
narrow numeric binding in both modes.

ADR 0136 extends these joint directions and strict finalization to low-storage
categorical MAP with multiple effects and finite controls. Two/three categorical
effects retain their borrowed projector for canonical rank and inverse duties;
four-plus effects use the generic coefficient projector there. Joint results
remain direction-only. Optional-state refusal retains the previous categorical
route without duplicating topology. This supersedes the categorical exclusions
above for that bounded route; fast categorical LSMR and one-FE routing are unchanged.

ADR 0152 extends accepted-frame finalization to an already admitted categorical
LSMR owner. A fresh strict joint Newton witness is followed by one canonical
final rank/bread/Hessian transaction on the unchanged accepted point. Only the
final response refreshes the canonical session, preserving its response-delta
history. Numerical refusal, movement or changed basis retains canonical retry;
current-weight certificates and final inference authority remain independent.
Ordinary LSMR, CMG, one-FE and OLS routing are unchanged.

ADR 0156 applies component scaling to an already configured PPML checkpoint's
zero-iteration shortcut too. A global initial pass on disconnected components
also requires a fresh component-scaled pass at the same tolerance. Original-
weight norms are restored before return or iterative solving. This prevents a
large component from hiding warm projection drift in a small component; finite
regression and all canonical acceptance still use the original weight frame.
No routing or memory admission is extended.

ADR 0153 scopes prepared categorical checkpoint LSMR setup to one immutable
weight/design/RHS call. A failed fresh canonical certificate may retry only its
failing columns, provided retained-output overlap fits the existing full-batch
allowance; otherwise the rejected output is released before a full retry.
Every attempt receives one complete fresh original-weight canonical certificate.
The generic adapter's duplicate certificate is removed only at this boundary.
Other canonical checks, the tighter retry limit and untouched-input MAP fallback
remain unchanged. No prepared numerical state crosses a weight frame.

Under ADR 0146, reusable two-FE categorical initial checks and MAP pilot sweeps
may borrow the idle PCG group banks when row-by-RHS work warrants parallel group
reductions. A scoped owner restores both banks before current-weight PCG
initialization or any error return. No new capacity is admitted; no numerical
PCG state is reused. Projection arithmetic, check frequency and the full
acceptance gates remain unchanged.

ADR 0147 gives publication PPML a direct two-FE Schur entry after initial
certification, replacing its mandatory four-sweep warmup. Disconnected Schur
blocks and their preconditioners are scaled by current component group masses;
immutable level-component roots are prepared once per owner. Reconstruction and
acceptance still use original weights. The PPML planner admits two words per
smaller-partition level. Ordinary OLS construction and checkpoint-LSMR policy
retain their prior behavior; a PPML checkpoint's MAP fallback may use the direct
engine.

ADR 0148 prepares the joint PPML finite Hessian from one symmetric triangle.
Streamed entries delay right-column normalization until after the weighted inner
product, using fixed independent accumulators and borrowed numeric slices where
available. ADR 0155 also uses eight fixed accumulation chains for dense
projected-column Gram entries; RHS and Krylov products remain unchanged. Existing
current-weight geometry, fresh FE/finite scores, reconstruction and canonical
rank/final-Hessian authority remain. No previous-frame factor or new solver route
is retained from the rejected matrix-free experiments.

ADR 0149 resolves original joint PPML columns independently of complete-model
numeric binding. Direct numeric row bindings retain fused scans; recipe-backed
rows use column-oriented norms, reconstruction and inner moments. Finite products
and independent original-input scores borrow each numeric source or ordinary
factor code column separately. The existing current-weight arithmetic and
acceptance remain, with no new heap storage or cross-frame products. Other
consumers retain their numeric-binding admission rules.

ADR 0151 supersedes the remaining single-worker product exception of ADRs
0148–0150: all streamed Hessian entries use the same resolved-column, fixed-
accumulator product kernel. The separate direct-row product loop and its factor-
bank borrowing are removed. Norms, reconstruction and residual moments keep
ADR 0149's storage policy. Eligible serial reductions change order but retain
independent acceptance, without a new workspace, generation or solver route.

Under ADRs 0127, 0132 and 0137, actual one-intercept-FE publication PPML can
analytically concentrate group intercepts during ordinary MAP-owned steps.
Fast execution borrows retained dense finite representatives; low execution
borrows original numeric columns or generated-column recipes. Current means and
centered moments are recomputed, and independent original-input scores and
strict canonical WLS remain acceptance gates. Final rank/bread products are
reused only at the exact accepted Hessian frame with valid generations and
reconstruction. Optional-memory or numerical refusal retains canonical solving.

Retained canonical projection may borrow selected column norms from the same
call's freshly accepted categorical certificate for exactly one FE and exactly
matching weight normalization. Low control tiles use their own column axis.
Other cases retain explicit norm scans; no later generation borrows these norms.
See [ADR 0138](adr/0138-retained-scratch-standardization-and-norms.md).

Under [ADR 0168](adr/0168-fused-canonical-ppml-products.md), admitted low
two-categorical-FE PPML canonical calls share one current-weight grounded Schur
factor across response and controls. A fused original-input residual pass forms
direct fully absorbed Gram/RHS/norms and independent FE moments. Same-call
products go directly to canonical rank solving; retained rows are written once
for unchanged QR/span, residual, inverse and inference consumers. Complete
workspace and structural factor-work refusal preserve the existing projector.
No weighted factor or certificate authority survives the call.

Under ADR 0139, admitted low one-FE PPML canonical products use immutable
original columns minus freshly computed current-weight group centers. A separate
original-input residual-moment replay certifies the projection; finite moments,
rank/QR fallback and inference stream the same implicit rows. No group geometry
is authoritative after a weight change until refreshed. Fast execution retains
its dense representation, and optional-memory refusal retains scratch execution.

A solver recurrence is never sufficient evidence. For each effect-level
block `B`, the final gate computes the projection of `sqrt(W) r` onto
`col(sqrt(W) B)` with scaled, reorthogonalized rank-aware QR and measures it
relative to the outcome/residual scale. Columns are normalized before this
check, making the certificate invariant to absorbed-channel units.

The generic owned-FE certificate computes its raw-moment diagnostic in bounded
32-RHS row tiles (ADR 0113). Each entry keeps its scaled compensated arithmetic,
observation order and overflow saturation. Three fixed arrays contain 768 bytes
of stack payload. Raw moments, residual norms and relative span projections
are freshly evaluated at every scheduled check. Generic automatic slope
absorption may reuse the identical weighted QR bases within one immutable
design/weight call (ADR 0116). Rank rules, reduction order, checks and stopping
remain unchanged. No geometry survives a change of weights or sample.
With an admitted cache, at least 16,384 rows and an already installed team
larger than one worker, automatic owned slope certificates may evaluate groups
in parallel (ADR 0117). A bounded result bank keeps one value per group/RHS
and one raw maximum per group. Merging remains in effect/level/RHS order,
including the RMS FMA recurrence. Each residual still receives the same fresh
norm, span and raw-moment checks. Scratch refusal retains serial certification.

Hitting an iteration limit or failing the fresh certificate is fatal. Solver
threads operate only on Rust-owned storage; no Stata SPI call occurs off the
calling thread. Independent explicit-dummy fits validate small designs,
identified combinations, residuals, absorbed sums, and normalization rules.

## Singleton pruning

The shared OLS/legacy-ledger pruner counts each effect's active multiplicity, then maintains the XOR of
active observation indices for each level. Validated multiplicities are positive
integers: a level with unit mass contains exactly one physical row, whose index
is its XOR. Deleting that row updates every affected level without searching
the observation array. Initial sample masks, frequency multiplicity, queue order
and the recursive fixed point remain authoritative.

For N observations, D effects and L total levels, work is O(D*N + L). The XOR
index needs one machine word per level; storing global level indices in the
queue instead of effect/level pairs removes one word per queue slot. Counts,
masks and the dropped-row list retain their existing allocation bounds. Every
reservation is fallible. If the initial counts contain no singleton, pruning
returns before allocating the XOR index, markers, queue or dropped-row list.
Caller-thread interruption and transactional publication boundaries are unchanged;
the pruning kernel does not call Stata SPI.

Production prepared PPML uses its immutable generation's existing compressed
membership and authoritative partition codes. It counts exact `u128` frequency
multiplicity (physical rows for unit/probability weights), then queues each
deleted row once. Counts decrease in all affected partitions. A level can reach
unit mass at most once, so finding its survivor scans that level's membership
at most once after counting. A final mask scan returns sorted compact-row
proposals without a dropped-row sort. Each singleton fixed-point invocation is
O(D*N + L), including cascades after certified separation. This does not assert
that the complete multi-method separation/rebase sequence has that bound.

This prepared path is common to fast, low and automatic memory modes, solver
choices, weights, covariance choices, interactions and slope-bearing partitions.
It reserves no observation-by-effect index and skips the queue/selection-mask
allocation when no singleton exists. Its distinct per-level and row workspaces
are charged before generation-zero admission; see `docs/memory_model.md`.
Ledger generation, drop-reason precedence and interruption/publication boundaries
are unchanged.

## Comparator-equivalent MAP and CG

The comparator-equivalent transform is one complete symmetric Kaczmarz sweep.
Unaccelerated `solver(map)` applies that transform until the independent
certificate passes. `solver(cg)` uses the positive-semidefinite complement
operator `Pi(x) = x - M(x)`, where `M` is the symmetric residual sweep:

1. `r = Pi(y)` and `u = r`;
2. `v = Pi(u)`;
3. for each RHS, `alpha = <r,r>_W / <u,v>_W`;
4. update `y = y - alpha u` and `r = r - alpha v`;
5. use the Fletcher–Reeves ratio for the next search direction.

Outcome and regressor RHSs are processed as one batch but have independent
recurrences. Categorical-only designs borrow finalized codes and membership
directly; they do not copy observation codes merely to use CG. Slope-bearing
effects use the shared owned projection design.

Nonfinite curvature, invalid recurrence values, iteration exhaustion, or an
uncertified terminal value fail closed. A finite nonpositive denominator from
loss of conjugacy restarts that RHS from its true residual; a broken RHS that
already passes its own certificate is deactivated while other RHSs continue.
The recurrence stop matches reghdfe's Hestenes energy rule: each RHS tracks the
recent `alpha * <r,r>_W` improvement relative to its remaining improvement
potential. Publication requires both Hestenes convergence and the joint fresh
FE certificate. A certificate alone cannot terminate an ordinary CG path, and
the recurrence alone can never authorize publication.

For borrowed categorical CG, a small negative remaining-energy subtraction may
be independently audited against the current and pending weighted iterates;
[ADR 0059](adr/0059-borrowed-cg-negative-energy-audit.md) defines the bounded
consistency check. An accepted audit uses the measured remaining energy and
retains the same Hestenes stop and fresh certificate. Inconsistent steps,
larger discrepancies and nonfinite recurrence errors still fail closed.

Coefficient recovery uses the same recurrence but adds a stricter terminal
condition: `max(abs(target - D alpha)) / max(abs(target)) <= 1e-10` (or the
tolerance-derived stricter value). If the ordinary Hestenes condition fires
first, recovery continues from the current CG state; it does not throw that
work away and start a second solver. This additional scan and stop rule are
used only for saved-effect recovery and do not change ordinary CG estimation.

## Current automatic policy

Borrowed automatic absorption reuses its freshly computed initial reference
norms for the identical initial residual-norm request. All FE moments are
still freshly checked, and each new input gets its own norm scan. This changes
no solver selection, tolerance or workspace allowance; see
[ADR 0094](adr/0094-borrowed-auto-initial-norm-handoff.md).

For fast OLS, [ADR 0035](adr/0035-guarded-wide-ols-schur.md), extended to supplied
weights by [ADR 0037](adr/0037-weighted-ols-schur-admission.md), adds a guarded
two-FE Schur route at 16–64 actual RHS columns and at least 16,384 rows.
[ADR 0076](adr/0076-narrow-weighted-ols-schur-admission.md) additionally
admits 3–15 RHS columns for unit or supplied weights on at least eight requested workers.
An eight-step probe on three RHSs selects full-width coefficient-space CG
only when its contraction is at most 0.01 and workspace plus continuation
fit the joint budget. The route retains fresh FE/OLS certificates and can
continue with ordinary symmetric CG within the shared iteration budget.
Requested `auto`, used `cg`, eight pilot iterations and an accepted contraction
identify the route;
explicit `solver(cg)` keeps its symmetric-MAP/Hestenes contract. Saved-FE OLS
may use final-target recovery under [ADR 0064](adr/0064-saved-ols-final-target-recovery.md).
Low-memory, out-of-envelope narrow, higher-dimensional and slope models retain
their previous Schur eligibility. One-FE OLS uses its direct initial projection and
reuses the residual norms from its fresh certificate.

Eligible fast automatic OLS fits with one ordinary slope partition reuse
fixed-weight group Grams and rank-aware QR certificate bases across the initial
batch and final residual. The envelope is at least 16,384 rows, at most four
absorbed channels and 64 RHSs, with admitted construction/projection storage and
an available certified fallback. Every returned residual gets fresh checks;
rejection releases the cache and resumes from untouched input, charging attempted
work. Norm handoff to OLS is limited to unit weights because the generic slope
projector and OLS use different supplied-weight divisors. Explicit solvers,
low-memory and shared/PPML projection paths remain separate. See
[ADR 0087](adr/0087-single-partition-ols-slope-geometry.md).
Under [ADR 0092](adr/0092-compact-ordered-single-slope-passes.md), Grams use
actual channel-square storage. Serial fits with ordered groups combine
projection, reconstruction and fresh norm/maxima accumulation while preserving
observation and arithmetic order. Independent QR and raw-moment checks still
run on the returned residuals; shuffled/threaded fits keep their prior traversal.

At 3–8 RHS columns the same-call OLS owner may retain useful full-width pilot
state under [ADR 0086](adr/0086-useful-narrow-ols-schur-pilot.md). The original
selected three columns still determine weighted admission. Rejected pilots
return untouched input; unsafe additional-column state keeps the prior solve.
Accepted safe state uses the existing owner, independent norms and certificate,
counting the eight retained steps once within the former eight-plus-64
allowance and unchanged overall continuation budget. Wider and shared/PPML
pilots retain their previous behavior.

An accepted initial probe on at least eight workers may be reused
for the same fit's final one-RHS Schur projection. The final solve retains its
fresh strict tolerance and independent certificate, its 64-step cap and charged
CG continuation. Reuse reports zero new pilot iterations. Eligible saved effects
can then be recovered by graph propagation, component normalization and complete
reconstruction certification, with the existing iterative recovery fallback.
[ADR 0077](adr/0077-weighted-final-projection-and-graph-recovery.md) extends both
routes to supplied weights; weighted normalization and the pointwise
reconstruction check remain mandatory, including on tiny-weight edges.

The Schur probe forms its three RHS group statistics and degrees using the
actual normalized weights, then applies that same weighted operator. Tiny
bridge weights cannot be hidden by an unweighted connectivity assessment.
Group statistics run in parallel, preserving canonical row order within each
group. Supplied weights retain their original kind and inference semantics.
The admitted OLS solve uses disjoint
parallel recurrence updates above seven workers as well; the chain-tuned
seven-worker crossover still governs the shared PPML path. Probe thresholds,
iteration accounting and continuation eligibility are unchanged.

Eligible capped unit-weight attempts combine repeated FE pairs into counted
edges in idle group-statistics scratch. With 2N words both gathers are cached;
with N words the right gather may be cached after a bounded repeated-edge
sample passes. OLS may also borrow idle left-RHS storage for the other gather
when it fits N, restoring original left means before reconstruction; shared
reusable/PPML callers opt out. See [ADR 0082](adr/0082-two-sided-unit-ols-edge-scratch.md).
N must fit u32. See
[ADR 0079](adr/0079-narrow-one-sided-schur-edge-cache.md). Codes and multiplicities are stored as private bits; operator arithmetic
uses the count as a weight. This changes summation order but preserves the
mathematical operator and independent certification. Construction uses disjoint
membership ranges on the existing team. It adds no heap owner, and the final
certificate overwrites the bank. Insufficient-scratch calls retain their
observation-membership operator.

Supplied-weight capped OLS attempts may cache the right gather when the same
repeat sample passes and both existing group scratch and idle left RHS means
fit N words. Separate banks hold exact codes and f64 normalized edge weights.
The original left means are recomputed before reconstruction; independent
certification then reuses group scratch. No new owner is allocated, and shared
reusable/PPML calls explicitly opt out. See
[ADR 0080](adr/0080-weighted-schur-edge-scratch.md).
When both compact orientations and their offsets fit those existing banks,
the weighted cache is compacted and transposed in place. Both gathers use the
same f64 edge sums in canonical neighbor order. Capacity rejection preserves
the one-sided cache; no owner or allowance is added. See
[ADR 0083](adr/0083-compact-weighted-ols-edge-scratch.md).
When left means were used as edge scratch, OLS restores them during the left
reconstruction traversal, retaining separate row-order sums and the original
group weight totals. Both banks are overwritten before packed codes enter
arithmetic. The uncached path and final certificate remain unchanged; see
[ADR 0085](adr/0085-ols-schur-restoration-during-reconstruction.md).

Wide one-FE/Schur OLS certificates use fixed 1,024-group partials in the
existing norm scratch when that storage fits. One-/two-FE OLS also uses these
partials at 8–15 RHS columns when the group-by-RHS table has at least 65,536
elements. Each moment, absolute maximum,
relative maximum and pass/fail comparison retains its arithmetic. The
reported two-norm merges partial sums in group order; its last bits may differ
from the previous single accumulation. It does not determine acceptance on
these routes. PPML, smaller tables and the existing weak-graph routes retain
their certificate reductions. This scheduling adds no workspace allocation.

Outside this envelope, `solver(auto)` implements the ADR-0021 categorical AKM
policy with the ADR-0022 iterative-terminal and ADR-0023 direct-terminal guards,
while retaining the ADR-0009 generic/slope policy:

- categorical intercept-only graphs first select CMG, without a MAP pilot,
  when there are at most nine nuisance Schur columns, at least three
  observations per highest-cardinality level, and at least a 10:1 level-count
  ratio between the two core effects;
- an auto-eligible iterative-terminal hierarchy remains CMG only when its
  deterministic repeat-weighted work is at most four finest-level passes; a
  rejected iterative hierarchy is released before nuisance-factor/PCG work
  and the fit uses direct LSMR;
- an auto-eligible direct terminal uses the locked hierarchy/RHS/thread cost
  guard. It selects direct CG only when the broader cost score rejects CMG,
  depth-work exceeds 200 per observation or nuisance width reaches nine, and
  execution is serial or the initial RHS width fits within the thread width;
  otherwise it retains CMG;
- one FE: direct one-pass demeaning and immediate certification;
- other categorical intercept-only graphs select direct
  Schwarz-preconditioned LSMR without a MAP pilot, with internal tolerance
  `max(1e-14, requested_tolerance * 1e-2)`;
- slope-bearing and generic multiway designs retain at most four symmetric MAP
  pilot sweeps, the 0.008 contraction boundary, and genuine CG for the
  remaining cases; and
- if auto-selected LSMR returns a finite but uncertified candidate, continue
  certified CG from that candidate within the remaining iteration and memory
  budgets. Report all attempted LSMR work; do not restart from the pilot state.

The CMG selector applies in fast and low execution. It remains narrower than
explicit `solver(cmg)`, which continues to
support up to 64 non-base nuisance columns. One hierarchy and workspace pool
are retained across outcome/regressor blocks. Final OLS residuals normally
already satisfy the FE certificate, so they skip graph PCG; the normal
certified CMG path runs only when correction is required. The graph PCG systems
use one-hundredth of the requested FE tolerance because their projection feeds
the more sensitive OLS score certificate. CMG setup failure selects direct
LSMR with a warning; a solve failure marks the fit-lifetime route failed and
uses certified CG thereafter.

Neither planned hierarchy veto is a setup failure or fallback: each emits no
warning, does not set the fallback flag, and reports its actual LSMR or CG used
solver. Graph and hierarchy construction time is nevertheless charged to
solver setup time. Explicit CMG bypasses both automatic guards. The direct-CG
guard is deliberately conservative; it leaves some measured CG wins routed to
CMG rather than adding less-supported threshold clauses.

The report retains requested solver `auto` and records used solver, pilot
iterations, pilot contraction, total iterations, stop reason, setup/solve time,
fallback, and certificate norms. There is no FE-coefficient cap. The prepared
categorical route passes the already finalized codes and memberships directly
to `within`, avoiding the former observation-scale owned-code and membership
copies. Schwarz construction and LSMR reuse those prepared structures.

Low-memory projection batches may finish with different engines under that
same automatic policy. Under ADR 0033, certified LSMR and flagged CG
continuation reports with identical LSMR preconditioner metadata aggregate as
requested `auto`, used `cg`, and fallback true. Iterations and certificate
norms retain their existing maximum-over-batches convention; timing is summed
and warnings retain batch order. This reports the continuation without
changing its numerical acceptance. Other incompatible report mixtures remain
errors.

For low-memory categorical OLS, ADR 0032 retains one LSMR operator and
preconditioner across the response and regressor projection batches when
direct LSMR is selected. The sample, design, weights and configuration stay
fixed. Construction is lazy and charged once; RHS work and fresh independent
certificates remain per batch. Retention ends before QR and output recovery.
A CG continuation releases the operator and returns subsequent batches to the
existing one-shot route. Fast fits and generic/slope adapters retain their
existing lifetimes and report policies.

The direct LSMR configuration followed calibration of requested/tight tolerances,
off/diagonal/Schwarz preconditioners, forced restarted CG refinements of
4/8/16 iterations, and the prior coarse-LSMR/ordinary-CG hybrid. Direct tight
Schwarz-LSMR was the surviving lowest-runtime arm; CG refinement is therefore
not a production stage. The forced-refinement test path bypasses CG's initial
certificate return and proves that the comparison arm really executes its
registered number of restarted iterations.

## Public PPML solver families

`feppml` accepts exactly `solver(auto|map|lsmr|cmg)`. Auto selects LSMR for
exactly two or three categorical intercept effects when fast execution and the
complete ordinary/checkpoint workspace fit the generation-zero memory plan.
Other designs, low-memory execution and allocation audits select MAP. A modeled
workspace veto selects MAP before construction and is not a numerical fallback;
actual allocation errors remain fatal. Auto never selects CMG. This development
policy is admitted under ADR 0029. The PPML request records the requested family, while the
result separately reports the selected family, concrete engine, final effective
route, and checkpoint family. This policy does not alter `fereg` or its OLS
solver session.

One reusable categorical joint-incremental engine operates between canonical
checkpoints. It applies one augmented operator to the Newton correction over
active controls plus categorical FE coefficients and carries a fit-lifetime
correction warm start. A zero-active-control fit uses the same engine while
retaining the public `lsmr_fused` concrete identity; positive-rank fits report
`lsmr_joint`. Two-effect problems retain Schwarz state and rebuild it only when
recurrence or certificate evidence requires that work. Problems with three or
more effects retain one Schwarz core for the two largest retained effect
blocks and a fresh current-weight positive diagonal action for every additional
effect. A matrix-free symmetric block-factor bridge applies the current-weight
cross Gram between that core and the additional effects. Its action is
`T diag(C,D) T'`, where `C` is the retained core action and `D` is the positive
extra-effect diagonal action, so the combined preconditioner remains symmetric
positive semidefinite. The bridge uses finalized memberships directly and
retains no additional pair graph. This covers the full FE axis while avoiding
the former all-pairs Schwarz policy, whose extra factors made the augmented
recurrence dramatically slower. Exact finite and per-level normal equations
still certify the completed recurrence; the preconditioner is never evidence.

The preconditioner and reuse policy are internal implementation choices, not
additional command options. The initial rollout requires at least two
categorical intercept-only effects, the fast memory route, and no allocation
audit. Saved fixed effects are recovered once from the accepted canonical
final-Hessian state without disabling the fast ordinary-frame engine.
`memory(auto)` may select fast; explicit `memory(low)` and
other recognized but unsupported combinations return `r(498)`.

The canonical retained session still owns the first weighted-linear frame and
freezes its active basis; Mu separation remains canonical MAP. Under ADR 0027,
explicit LSMR and CMG fits with two or three categorical effects and fast memory
use a fresh current-weight batch LSMR
projection for the initial response-plus-all-controls frame as well as strict
and final-Hessian checkpoints at every positive retained sample size. The
LSMR candidate is independently re-certified by the canonical session before
the unchanged all-width rank/status decision, bread, score, objective,
reconstruction, and exact `q_i * mu_i` final-Hessian checks run. An uncertified
numerical candidate receives at most one tighter batch on untouched inputs,
with unchanged canonical tolerance and explicitly counted retry work. A second
numerical rejection falls back to MAP on the untouched authoritative inputs;
other errors remain fatal. At a strict checkpoint, the predictor, objective,
score, rank, basis, and all estimable coefficients must agree; an unidentified
normalization coordinate cannot force another IRLS frame, and the canonical
strict state replaces it before publication.

Under [ADR 0143](adr/0143-low-memory-three-fe-checkpoints.md), publication MAP
sessions with three categorical intercept effects, finite controls and selected
low storage may also admit canonical LSMR at the bounded pool width. Disconnected
components use independent positive weight scales inside absorption only; all
canonical finite/rank/score/Hessian checks use the original weights. Optional
workspace refusal retains the established projector and joint ordinary owner.
The warmed allocation window measures ordinary MAP directions independently of
its out-of-window canonical LSMR equivalence and finalization; both identities
remain explicit and all numerical/capacity/event checks remain required.

On already admitted two-FE checkpoint-LSMR sessions, the retained Schur
workspace may accelerate 1–64 actual RHSs at N>=16,384. The eight-step
current-weight probe uses every column for batches of up to three RHSs and
the established three-column selection for larger batches. It requires
contraction at most 0.01. A bounded candidate must pass the unchanged fresh
canonical certificate; rejection resumes LSMR on untouched inputs. The
single- and narrow-batch extensions add no persistent state or changing-weight
cache. Public OLS retains its original probe selection. See
[ADR 0106](adr/0106-single-rhs-ppml-schur-checkpoint.md) and
[ADR 0107](adr/0107-narrow-ppml-schur-checkpoints.md).

The reusable categorical MAP CG accelerator forms its symmetric `I-S` action
by summing weighted FE group contributions. It does not obtain that action
by subtracting nearly equal observation vectors: cancellation in that
subtraction can create an off-model-space component that a weak-graph CG
recurrence amplifies without failing a normal-equation check. Group means use
the existing scaled compensated reductions, and the action reuses admitted
buffers. Fresh FE certificates, strict and final-Hessian synchronization, and
publication tolerances are unchanged; no extra saved-FE recovery is imposed
on fit-only calls. The ordinary OLS projection retains its existing arithmetic.

The initial public tranche makes no allocation-free, large-scale, or
thread-scaling claim and no automatic-routing claim. The fused engine still
owns per-frame weight and projected-result storage, and either engine can spend
one tighter recurrence pass after an uncertified result. When the selected
policy owns a retained Schwarz preconditioner, it is rebuilt for that pass; the
original publication certificate tolerance is unchanged. The result wire
measures those costs separately from canonical checkpoint work.

The pinned `ppmlhdfe` comparator uses its default conjugate-gradient
acceleration of symmetric Kaczmarz for multiple effects and unaccelerated
projection for one effect; it must not be described generically as
LSMR. The development harness does not force a second comparator route. It
parses the pinned solver's verbose route line and fails if either default
technique changes. If a candidate disagrees scientifically with the timed
default fit, the harness runs one untimed fit from the same pinned closure with
`tolerance(1e-10) itolerance(1e-12)`. That strict fit keeps the same
observed default technique (unaccelerated for one effect). Agreement with the strict fit
adjudicates the stopping discrepancy while retaining the original default-fit
mismatch in the analysis. If both default and strict comparisons still differ
only in finite coefficients, the development harness may run a final untimed
public-state audit. Fresh processes save the public fitted means from MAP and
from both pinned reference fits; a separate Mata calculation recomputes every
control, constant, and FE-level score plus likelihood and deviance from the raw
data. [ADR 0026](adr/0026-ppml-comparator-objective-adjudication.md) makes each
pinned reference fit's reported `e(ll)` and `e(deviance)` the authoritative
cross-estimator objective pair, subject to finite, domain, and Poisson-identity
checks. Fereg's certified objective must be no worse than that pair within the
registered tolerance. The reference public prediction, its replayed objective,
KKT score, and model-space projection remain mandatory diagnostics. A raw
predicted state is an additional objective witness only when independent
checks certify both its control-plus-FE model-space feasibility and its full
control, constant, and FE-level stationarity; fereg must also be no worse than
every such witness. A projected state alone is not a witness. A state that does
not meet the model-space threshold remains indeterminate rather than proven
infeasible, but that indeterminacy no longer invalidates the reported reference
objective.

Fereg remains subject to the stronger self-consistency rule: MAP must satisfy
the public `1e-8` score tolerance, its published mean must replay its reported
objective and lie in the declared model space, and all solver, rank, sample,
and result certificates must pass. Fresh normal residuals, recurrence drift,
the loose-to-final fitted-vector tail, and the Poisson identity
`deviance + 2*ll` remain checked. Sample, VCE, total-DoF, route, fallback,
certificate, or non-objective failures remain fatal. The analysis retains the
reported and replayed reference objectives, all prediction diagnostics, and
all hashed public-state inputs. This development-only audit uses the pinned
comparator's observed default projection route and never forces a different
`ppmlhdfe` solver.

The version-4 development manifest derives explicit-solver eligibility from
the generated dataset metadata before timing. It excludes an unsupported
solver arm instead of treating its documented `r(498)` as a failed benchmark;
the public rejection remains covered separately. For CMG, the runner mirrors
the prepared engine's core choice: the two largest-cardinality effects are the
core graph, and the remaining non-base width must not exceed 64. This envelope
counts third-effect indicators, not the finite regressors K. Version 4 adds
local one-FE, intermediate mixing, checkpoint-boundary, nuisance-width,
finite-width, thread-scaling, large-easy, and held-out-seed profiles. Historical
version-3 manifests and receipts remain unchanged.

Under [ADR 0028](adr/0028-one-effect-ppml-initialization.md), cold one-effect
publication fits begin at the analytic group-intercept optimum for zero finite
slopes when it is representable. This only seeds the existing estimator: rank
freezing, ordinary IRLS, strict convergence, final-Hessian synchronization,
and every independent certificate remain mandatory. Unusable seeds restore
the original global initialization; warm starts and Mu exploration are unchanged.

The public CMG family means only the prepared fused-FWL engine introduced by
ADR-0023 and updated to the exact prepared-topology pin by ADR-0024. It prepares
the two core FE endpoint topology once for the fit. On each ordinary IRLS frame
it validates and assembles the current-weight numeric
graph, builds a fresh hierarchy, projects all frozen active controls and the
working response through caller-owned batch buffers, and skips the generic
observation-space CG polish. It then uses the same standardized pivoted-
Cholesky finite solve as fused FWL and requires fresh per-column projection
certificates plus a complete control-and-FE normal-equation certificate.

The PPML graph batch capacity covers the larger of the response/control width
and the nuisance Schur width, capped by the existing pool size. This lets a
narrow finite design solve independent nuisance directions across the admitted
thread team. Early memory admission fixes that capacity for initial ordinary
engine construction and any strict-basis replacement; current-weight setup,
per-RHS convergence, and every fresh certificate remain mandatory. Private
frame tracing reports the capacity as well as all setup, solve, and retry work.

Prepared PPML submits graph RHSs in batches aligned to the actual admitted
workspace-pool width. The effective width is the largest pool multiple within
the allocated capacity; only the final batch may be partial. This avoids an
underfilled execution wave at every capacity boundary without enlarging the
memory plan or changing any recurrence, reduction order within an RHS, or
certificate. The rule applies to nuisance setup and response/control solves,
including selective retries. Private tracing reports `pcg_batch_width`
separately from `batch_capacity` and retains actual RHS/batch counts. Serial
pools and the OLS owned-result route keep their original batch boundaries.

Prepared PPML retains the current frame's nuisance graph potentials while
forming its Schur factor. The correction for each projected response/control
column uses their linear combination instead of solving that combination a
second time. This is within-frame linear algebra, not cross-frame warm state:
the weights, graph, hierarchy, nuisance solves, and factor are rebuilt together.
Every completed column still passes the fresh full-FE certificate, followed by
the combined finite-and-FE certificate. A tighter nuisance retry rebuilds the
potentials with its factor. Checked admission and exact retained-memory checks
charge core-vertex count times nuisance width; no observation-by-nuisance
matrix is retained. Private tracing separates actual graph RHSs/batches from
nuisance RHSs completed by reuse. Maximum-RHS iteration counts retain their
existing units. OLS keeps its owned-result correction solve and coefficient
recovery path.

Ordinary PPML CMG can seed fresh nuisance-basis solves from the immediately
preceding fully certified frame (ADR-0024's warm-start amendment). The fit-owned
cache contains potentials only. Admission requires finite guesses, normalized
Hessian weights within a factor of two in both directions, no prior tighter
retry or extra restart, and useful freshly measured initial-residual reduction
on any prior seeded solve. Current graph, hierarchy, RHSs, recurrence residuals,
Schur factor, and all full-model certificates are still recomputed. Invalid
eligibility starts cold; failures, retries, committed checkpoints, and basis
replacement invalidate the cache. Promotion consumes the completed absorber
and swaps preflighted buffers only after full certification. Memory charges one
additional core-vertices-by-nuisance array, never an observation-by-nuisance
matrix. `phase=cmg_nuisance` tracing records warm RHS, summed nuisance iterations,
restarts, weight change, initial residual quality, and setup subphase times;
the existing public iteration counts remain maximum-RHS counts. OLS and CMG
designs without an actual nuisance block do not use nuisance guesses.

Frozen active controls have a separate fit-owned warm cache, using the same
finite-guess, factor-two current-weight transition, full-frame certification,
and own-recurrence quality rules. It stores raw core potentials before nuisance
correction, never residuals, in the frozen finite-column order. Each fresh solve
consumes eligibility before overwriting the admitted buffer; only a completely
certified frame without a tighter retry can promote it again. Failure,
checkpoint commit and basis replacement reset both seed families. Early and
retained-frame memory admission charge exactly core vertices times active
controls, with checked retained capacity; K=0 has no control-potential buffer.
The working response remains cold: its warm-start experiment triggered an
expensive three-FE certificate retry. A seeded control batch therefore stops
before the response column, which uses the original cold recurrence. Cold
frames keep the original batching. Every original-FE and combined certificate
is unchanged. `phase=cmg_projection` reports warm RHSs, warm/total summed
iterations, residual recomputations, fresh initial residual quality and
RHS/PCG timings, separately for first and retry attempts. OLS uses neither cache.

Prepared CMG frames with an actual nuisance block may retain the split heavy-edge
forest without low-effective-degree pruning. Eligibility requires every current
canonical edge's weight divided by its weighted degree at either endpoint to
be strictly below CMG's existing 1/8 threshold. An allocation-free current-graph
scan recomputes eligibility each frame; mixed/strong-edge graphs retain default
grouping. This changes only the fresh
preconditioner: it prevents nearly unreduced coarse levels from multiplying
every nuisance RHS's work. The current graph, PCG tolerances/residual checks,
and independent full-FE certificates are unchanged. The existing direct-factor,
depth and fill caps also remain unchanged; both groupings have identical
conservative memory bounds. Two-FE, exactly redundant-nuisance, and OLS routes
keep the default grouping. Setup traces report the effective grouping threshold;
hierarchy dimensions and elapsed time must accompany iteration comparisons.

If exact retained membership proves that the third effect is a function of
either selected core effect, ordinary PPML CMG projects through the two-effect
core alone. This removes an identically zero nuisance Schur block without a
numerical rank decision. The original three-effect model still governs memory
admission, the 64-indicator envelope, canonical rank/DoF/checkpoints, saved
effects, and every fresh per-FE certificate. Private frame tracing records the
one-based redundant effect position (zero when none) and the actual nuisance
solve width. Near-nesting is not sufficient. The topology proof is fit-owned
and is rebuilt with the engine; changing weights never invalidate exact span
containment. OLS is unchanged.

Only failed projection columns receive one tighter retry. If that independent
certificate demonstrates that the recurrence target was insufficient, the
next changing-weight frame begins at the learned tighter scale; the public
certificate tolerance is unchanged. Retry reuses the current hierarchy when
the nuisance solve width is zero; a nonempty nuisance block requires a rebuild
because its Schur factor depends on the original solve tolerance. A remaining
numerical failure runs certified MAP on that frame and disables CMG for the
rest of the fit.
Unsupported topology, invalid input, memory failure, interruption, and internal
errors remain visible failures. Mu frames remain MAP; initial, strict, and
final-Hessian projections follow the certified checkpoint policy above.

CMG has the same initial fast-memory requirement as LSMR but accepts exactly
two or three FE dimensions. Three-FE nuisance width is capped at 64, saved
effects use the same canonical final-Hessian recovery, and allocation audit is
disallowed. Its
retained fereg-owned buffers and the
topology-aware CMG hierarchy are charged before use. Prior-frame preconditioning
remains disabled; only the certified nuisance/control-potential guesses described above
cross ordinary frames. Caller buffers remove
owned per-batch result construction but do not imply an allocation-free IRLS
frame. The OLS CMG construction, polish, and fallback path is unchanged.

Result wire 1.2 publishes four fixed engine rows: MAP, fused LSMR, joint LSMR,
and prepared CMG. Each row repeats the route identities and records its own
ordinary/checkpoint frames, retries, fallback evidence, iterations, timings,
and certificate maxima. Numerical CMG nonconvergence records one visible
fallback, solves the same frame with certified MAP, and leaves the effective
ordinary route as MAP. Numerical checkpoint LSMR nonconvergence is separately
visible and retries the untouched checkpoint with MAP without changing the
ordinary route. A viable route that cannot fit
its checked workspace returns `r(909)`; unsupported topology or feature
combinations return `r(498)`.

## OLS saved fixed-effect recovery

This section describes `fereg` solver products. PPML saved effects are
recovered once from the accepted canonical final-Hessian state as specified
for the public PPML families above.

Saving absorbed effects is treated as an optional solver product, not as a
second estimation problem. In `memory(fast)`, categorical intercept routes use
the cheapest coefficient-space product available:

- LSMR retains the coefficient vectors it already solved for.
- CMG converts its exposed worker--firm PCG potentials to effect coefficients
  and combines them with the nuisance-effect Schur solution. A nonzero CG
  polish invalidates that product and deliberately triggers the fallback.
- CG carries compact FE coefficient vectors through the same alpha/beta
  recurrence as its observation-space directions.
- Irons--Tuck applies every accepted extrapolation to the parallel compact
  coefficient states.
- categorical `solver(auto)` retains the coefficient product of direct LSMR or
  CMG; generic pilot-routed auto adds the MAP pilot product to its selected
  LSMR or CG continuation.
- unaccelerated intercept-only MAP records the group coefficients from its
  original batched `y`/`X` sweeps, then forms
  `alpha_y - sum_j beta_j alpha_Xj` after the finite coefficients are known.

This MAP coefficient accumulation removes the recovery solve completely. The
running product uses the projection kernel's native level-major layout; large
tables add each group mean to that product inside the same parallel pass, and
the result is transposed once only after MAP certifies. When the exact DoF
result has already proved the AKM graph connected, normalization uses that fact
instead of rebuilding components. Saved-only publication also constructs the
absorbed target in its final buffer rather than allocating a separate `xb`
vector. Derived observation products execute inside the estimator's bounded
native thread team. Intercept-only retained products are scattered
observation-major, and saved-only calls do not materialize the additional
combined absorbed vector.

Publication remains independently fail-closed. Its intercept-only path
computes the reconstruction residual, weighted norms, and FE moments with
fixed deterministic chunks. The one-FE path evaluates these quantities
directly from the retained level coefficients, so it neither allocates a
second observation-length residual nor invokes the generic slope-capable
per-level solve. Multi-FE intercept recovery uses the same strict moment
definition with parallel level chunks; slope-bearing recovery retains the
general rank-revealing certificate. None of these publication kernels changes
the reconstruction threshold or FE tolerance.

Routes without a retained product, including `memory(low)` and SQUAREM, first
run one reconstruction-aware CG recovery capped at 256 iterations. If that
does not certify, intercept-only recovery performs four forward-Kaczmarz
sweeps with exact steepest-descent line searches while accumulating level
coefficients, then runs coefficient-producing LSMR only on the remaining
target. One-effect intercept recovery is direct. Slope-bearing recovery uses
strict coefficient-producing LSMR. Certified symmetric MAP remains the final
fail-closed fallback if the bounded coefficient solvers fail.

Single-source recovery can carry the prepared validation result through an
immutable borrow of the same finalized topology into its borrowed design.
The receipt cannot be reused with another topology; layouts outside its
single-source, single-effect guard keep their full validation path. Numeric
sample, recipe and recovery-argument validation retain their order, and the
independent reconstruction and FE certificates still run. The direct route
allocates no iterative direction or coefficient-increment buffer. Existing
recovery memory admission remains a conservative bound.

Every retained or recovered coefficient product is normalized by connected
component, scattered only for requested channels, checked against the absorbed
target in observation space, and independently subjected to the unchanged FE
moment certificate. Solver retention never relaxes the publication tolerance.

For an LSMR-routed residual, bounded fixtures must match the sealed 100-digit
oracle and large fixtures must remain within the registered envelope against a
100-times-tighter solve. Reghdfe residual distance remains a diagnostic for
this route; sample, singleton, coefficient/status, VCE, DoF, RSS, fit, FE-score,
and OLS-score contracts remain hard publication gates.

## OLS residual reuse and projection cutoff

Fast OLS always reconstructs the within-regression residual from the QR state.
That residual is submitted directly to the selected FE certificate/solver: an
already-within residual returns at iteration zero, while a failed certificate
triggers refinement. The raw and within residual vectors are not retained
simultaneously. The initial certificate reuses the column norms computed over
the same unchanged residual during setup, removing one redundant full-matrix
scan without reusing or weakening the independent FE-moment calculation.

Matched cutoff profiling at 32,768, 65,536, 131,072, and 262,144 rows selected
65,536 observations for parallel borrowed projections. It improved the
registered eight-thread standard CG median by more than 10 percent without a
measurable one-thread regression. Small batched RHS projections use a fused
per-group kernel, CG reuses its projection matrix, and large weighted dot
products use deterministic 32,768-row partial reductions. Final publication
still requires the complete independent certificate.

Eligible single-partition OLS slope recovery uses independent scaled group solves
and fresh parallel group certificates with ordered reduction. Final cached fit
residuals can avoid projection only after a fresh input certificate passes.
Admission, fallback and the protected PPML boundary are in [ADR 0088](adr/0088-direct-single-partition-slope-recovery.md).
