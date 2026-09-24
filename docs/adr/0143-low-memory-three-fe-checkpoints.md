# ADR 0143: bounded LSMR canonical checkpoints for low three-FE PPML

Status: accepted for local development, 2026-09-23.

## Decision

For low-storage publication MAP sessions with exactly three categorical intercept
effects and finite controls, optionally admit the existing canonical LSMR
accelerator at the pool-bounded RHS width. Keep the joint finite/FE owner for
ordinary directions. This extends ADRs 0027 and 0136; it introduces no new public
solver family or option. One/two effects, slopes, four-plus effects, separation,
fast categorical routing and one-shot OLS retain their existing choices.

Profiling identified repeated symmetric observation-space CG projections as a
substantial avoidable canonical cost. Two effects already have level-space
Schur/PCG and showed no useful gain from this extension, so retain that route.
This is a reuse of the existing independently certified solver, not another
competing recurrence layered onto ordinary IRLS.

## Current-weight projection and independent acceptance

Every canonical tile uses authoritative original responses/controls and the
current Hessian weights. For disconnected FE components, divide each component's
weights by its own maximum inside absorption only. A positive constant multiplying
one independent FE block's least-squares objective leaves that block's projection
unchanged. This keeps small-mass components visible to the recurrence. Finite
regression, rank decisions, objectives, inference and all canonical acceptance
checks continue to use the original weights. No transformed weight frame escapes
the projector; no factor or residual is reused across changing weights.

The unscaled experiment failed the existing extreme-component-mass accuracy test.
Component scaling passes an independent analytically constructed weighted
projection oracle with changing component masses, multiple RHSs, zero columns,
T1/T4 and forced fresh-certificate rejection. It does not relax stopping criteria.
The existing independent original-weight certificate accepts the reconstructed
residual, with the existing one tighter retry and untouched-input MAP fallback.
Allocation/interruption errors remain fatal. The original inputs survive every
speculative attempt. Canonical rank, finite solve, both inverse certificates,
score, objective, reconstruction and final q*mu synchronization remain required.

A zero-iteration certificate on an already absorbed column is neutral when
aggregating low-storage tile reports. A real MAP fallback governs the aggregate;
all certificate maxima, clocks, retry work, warnings and workspace peaks remain.
A uniform preconditioner is retained; a heterogeneous aggregate has no single
preconditioner identity. Private one-shot OLS report merging is unchanged.

## Memory and audit boundaries

Charge the complete existing LSMR transient model at the bounded RHS width,
plus 8*N + 32*L + 512 bytes for component scaling and construction scratch
(L is the sum of FE levels). The three-effect union-find construction and labels
fit below that bound; it also covers simultaneously live labels, component
maxima and the new N-vector. Admission occurs after existing optional joint
storage, so refusal retains that owner and the preceding canonical route.
No resident N-by-K bank is added. A direct one-byte-below-admission test verifies
refusal, unchanged joint storage and equivalent final science.

The LSMR call records the installed Rayon team size rather than estimating a
larger automatic host team. This aligns its memory estimate with actual execution.
Topology and numerical geometry remain ephemeral per canonical tile; sharing
compatible current-frame geometry across tiles is future work, not a claimed gain.

The warmed allocation certificate describes sixteen ordinary MAP iterations.
Its independent fresh numerical equivalence solve and final LSMR checkpoint occur
outside that window. These algorithms need not have equal identities. Permit
that distinction only for the admitted low three-effect categorical route;
retain zero events, zero growth, stable capacities, scratch identity and unchanged
fresh/warm numerical envelopes. Record the equivalence projection on the actual
checkpoint engine row. Native, plugin and Stata validators enforce the separate
ordinary/checkpoint identities. No allocation guarantee is extended to the
canonical construction itself.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-low-checkpoints-local-2026-09-23/README.md)
contains exact production/diagnostic identities, original failed candidates,
paired distributions, independent oracles, fresh cases, regression controls,
allocation tradeoffs and unresolved stock comparisons. Production timings use
unmodified pinned fixest with glm.tol=1e-10 and fixef.tol=1e-11. Diagnostic timings
remain separate. This is a development milestone, not qualification or a claim
of a universal 10% lead. Physical-cap and Linux confirmation remain deferred.
