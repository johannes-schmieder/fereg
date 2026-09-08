# ADR 0024: prepared-topology CMG integration for private PPML

**Status:** Historical development decision; public-admission clauses superseded by ADR-0025
**Date:** 2026-09-02
**Decision owner:** Johannes Schmieder

ADR-0025 supersedes this ADR's private-only and MAP-only public-admission
clauses. Its prepared-topology ownership, memory, and numerical decisions and
the evidence below remain authoritative.

## Context

ADR 0023 deliberately optimized fused CMG without changing upstream CMG. It
therefore rebuilt graph topology and returned owned PCG solution objects in
every ordinary IRLS frame. Upstream CMG commit
`90e1fe0b0c14065155532711246ede6678bb4935` now additively provides prepared
fixed topology, changing-weight numeric assembly, caller-buffer batch solves,
compatible retained preconditioners, optional initial guesses, and repeated-
solve memory reports.

Upstream measurements establish that prepared assembly removes useful work and
caller buffers primarily remove allocations. They do not establish an
end-to-end fereg speedup. Retaining a stale hierarchy or a prior solution can
help or hurt as IRLS weights change, so CMG intentionally supplies no rebuild
threshold and leaves that policy to its consumer.

## Decision

1. The exact runtime CMG pin becomes
   `90e1fe0b0c14065155532711246ede6678bb4935`. The lockfile and
   `references.lock.toml` must identify that same revision.
2. Only the private ADR-0023 fused-PPML engine uses the new repeated-solve
   surface. It prepares the two core FE endpoint topology once before ordinary
   IRLS and retains canonical endpoint groups, components, numeric-assembly
   scratch, and normalized-weight storage for the fit.
3. Every ordinary frame still validates all positive finite weights, assembles
   a new current numeric Laplacian, and builds a fresh hierarchy for that
   operator. Current graph products and fresh certificates remain authoritative.

   The 2026-09-05 nuisance-grouping amendment applies only to prepared three-FE
   frames with a nonempty nuisance block whose current graph has only weak
   edges: every canonical edge's weight divided by its weighted degree at
   either endpoint must be strictly below CMG's existing 1/8 threshold. An
   allocation-free scan of the freshly assembled graph determines this anew
   each frame; no policy state is retained. Eligible frames keep CMG's split
   heavy-edge forest without low-effective-degree pruning
   (`low_effective_degree_threshold=0`). This performance policy avoids
   repeatedly traversing almost-full-sized coarse graphs for every nuisance
   RHS. It changes only the fresh preconditioner's aggregation, not the
   current operator, quotient-space solve, recurrence tolerances, or independent
   certificates. Factor-size, hierarchy-depth and fill caps are unchanged;
   both groupings have identical conservative memory bounds. Any strong edge,
   including equality at 1/8, retains the default grouping. Unconditional
   unpruned grouping regressed a mixed sparse diagnostic and is not retained.
   Two-FE and exactly redundant-nuisance frames, and all OLS construction,
   keep CMG's defaults.
   The pinned CMG API already supports this configuration; no upstream change
   or automatic solver routing is introduced.
4. Fused PPML fills reusable contiguous RHS and solution arrays and invokes the
   checked caller-buffer batch API. The existing CMG serial, planned, and
   across-RHS routing decision is preserved. The 2026-09-05 user-authorized
   amendments permit certified nuisance-basis and frozen-control guesses, as
   below. The changing working response starts without a guess. Its warm-start
   experiment introduced a per-RHS certificate retry and costly nuisance rebuild
   and was removed. On a seeded frame the final response is a separate cold
   batch; unseeded frames retain their original batching.
5. Retained stale preconditioning remains disabled. A fully certified ordinary
   PPML frame may retain its current nuisance and frozen-control potentials
   for the next frame. Each seed family checks its own recurrence quality.
   These are guesses, never residuals, factors, or scientific evidence. The
   next assembly consumes eligibility and requires every normalized weight to
   remain within a factor of two of its prior value in both directions. This
   bounds the graph quadratic-form change; it is a conservative fereg policy,
   not a threshold inferred from CMG. All guesses must be finite. The prior
   frame must have passed every per-RHS and combined certificate without a
   tighter retry, and that seed family's solves must have no extra PCG restart.
   CMG's mandatory terminal residual check
   is not an extra restart. After a seeded solve, the largest freshly computed
   initial residual divided by its current RHS norm must be below one half to
   retain eligibility again. Otherwise the next frame starts cold.

   The pinned API centers each guess and recomputes its residual against the
   fresh current graph/RHS, using the unchanged PCG stopping and terminal
   residual rules. Every full-original-FE projection and combined finite/FE
   certificate remains mandatory. Invalid input, any failed/retried frame,
   active-basis replacement, or a committed strict checkpoint invalidates the
   seed. Rejected eligibility uses the existing zero start. Numerical errors
   retain the existing visible retry/MAP-fallback policy; resource, input,
   interruption, and internal errors remain fatal. Seed promotion consumes the
   finished absorber and swaps preflighted buffers only after certification;
   it cannot allocate, mutate canonical state, or expose a stale potential
   basis to another solve. Frozen-control seeds instead overwrite one admitted
   fit-owned buffer while invalid, and are marked eligible only after the same
   full-frame certificates. They store raw core potentials before the nuisance
   Schur correction, in the active finite basis's immutable column order.
   Basis replacement drops the buffer. All dimensions and its exact retained
   capacity are checked. K=0 retains no control-potential elements. There is no
   retained hierarchy or upstream change.
6. The prepared topology, assembly scratch, and fereg-owned normalized weights
   and the additional graph-vertices-by-nuisance and graph-vertices-by-active-
   control seed bases are charged with
   the fit-lifetime fused workspace, including early preparation/frame peaks
   and exact retained-byte checks. Current graph, hierarchy,
   optional plan, caller buffers, diagnostics, workspace pool, and nuisance
   factor remain subject to conservative preflight and exact retained-memory
   checks.
7. ADR-0023's projection, retry, fallback, checkpoint, and certificate rules do
   not change. In particular, a three-FE tighter retry still rebuilds its
   current-weight nuisance factor and hierarchy.
8. OLS continues to construct `CmgAbsorber` without prepared PPML state and
   continues through the prior owned-result CMG methods, CG polish, and
   fallback behavior. Public `feppml` remains MAP-only.

## Local development evidence

The warm-start amendment's local diagnostic evidence is recorded separately in
the development reports and `STATUS.md`; the original pin-admission evidence
below is unchanged. Warm setup traces distinguish warm RHS, summed nuisance
iterations, total/extra restarts, normalized weight change, fresh initial
residual quality, and RHS/PCG/Schur time. Separate `cmg_projection` traces record
warm control RHSs, warm and total summed iterations, terminal/extra residual
restarts, fresh initial residual quality, and RHS/PCG time for first and retry
attempts. These do not change the public maximum-RHS iteration units.
Existing frame traces retain scheduled
and effective tolerances, selective retries, graph work, and certification
time. Setup tracing also records the effective grouping threshold alongside
the actual hierarchy dimensions and construction time. Iteration counts alone
do not measure recursive hierarchy work. These measurements do not authorize
an automatic route or qualification.

The upstream `repeated_topology` suite passed 9/9 at the exact pin. Fereg's
focused CMG filter passed 17/17, including two- and three-FE MAP-oracle parity,
selective retry, permanent numerical fallback, and the existing OLS CMG tests;
the complete core library passed 655/655 and the two focused Python contract
modules passed 25/25. The universal macOS plugin passed architecture,
dependency/export, loader, and panic-containment checks, and the focused
licensed-Stata OLS auto/explicit-CMG fixture ended with `rc=0`; the plugin's
SHA-256 is
`47e09bc53ab35c03baa69590dd277854ff21b8bc5df3d9de7faf096264c77683`.

Matched local old-artifact/new-artifact screens used five repetitions and the
same current harness. Median fused-CMG seconds were 0.487/0.492 for balanced
two-FE at N=100,000 and K=10, 0.198/0.197 for balanced three-FE at N=25,000
and K=0, and 0.366/0.365 for the same three-FE design at K=10. The two-FE trace
reduced three-frame setup from 0.01063 to 0.00515 seconds, but that small phase
did not produce an end-to-end win. Three-FE setup and solve changed only a few
percent because nuisance-Schur work dominates.

On the new artifact, diagnostic medians were 0.492 seconds for fused CMG versus
1.684 for pinned default `ppmlhdfe` in the N=100,000 two-FE K=10 cell, and
0.365 versus 0.529 seconds in the N=25,000 three-FE K=10 cell. The difficult
chain median was 6.458 seconds, with the same six-frame iteration sequence and
all-column certified retry behavior as before. These dirty-worktree, local,
selected-cell results are diagnostic only. They do not establish a general
speedup, an automatic route, or qualification.

## Consequences

- Fused PPML no longer repeats endpoint sorting or component discovery as
  Hessian weights change, and it no longer allocates owned CMG result vectors
  for each RHS batch.
- The measured benefit is currently phase-local and topology-dependent. The
  dominant remaining costs are current-weight hierarchy construction,
  three-FE nuisance-Schur formation, and difficult-topology certified PCG plus
  retry.
- The next CMG optimization round should prototype adaptive retained-
  preconditioner and warm-start policies with explicit fresh-rebuild controls.
  It must measure total ordinary-frame time and failure/restart behavior, not
  only iteration counts.
- No SCC, broad matrix, allocation audit, thread qualification, route
  activation, or release claim is authorized by this decision.
