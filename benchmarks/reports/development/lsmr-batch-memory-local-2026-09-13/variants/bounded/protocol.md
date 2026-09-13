# Joint categorical LSMR memory-model experiment

Baseline: ac47b1907c7bc7bc8ee78de85621b0cc96546a4c. Installed dist is excluded.

Correct per-RHS recurrence axes, dense construction scratch, upstream sorted
frames, scatter reductions, result overlap and pooled Schwarz scratch. Share
the batch estimate with PPML checkpoints. Scientific arithmetic stays fixed.

First check the allocator against the complete solver lifetime, including
owned weights and RHS transport. Use eight small strata spanning serial/parallel,
identity/diagonal/Schwarz, sorted/permuted frames, narrow/wide RHS and dense setup.
These are allocation probes, not scientific convergence or performance claims.
Any excess is a failed model observation, retained and investigated.

Prove the new 65,536-byte solver-budget routing assertion fails on baseline and
passes on candidate. Run focused memory and LSMR tests, and two existing licensed
explicit-dummy oracles on each artifact. Follow with a small unrestricted paired
command comparison: three repeats each for weak OLS and a PPML checkpoint case;
single controls for small/narrow, wide weak, weighted strong Schur, complete IID,
and successful low-memory OLS. Preserve full science/certificates. Investigate
command slowdowns exceeding max(10 ms, 5%). Report total/phase, native/model/RSS,
solver/retry costs; no speedup is expected from accounting alone.

The public OLS selected-model report currently omits joint solver increments.
This admission experiment does not close that reporting gap or constitute full
qualification of opaque upstream factor-allocation constants. No SCC or push.

Pre-measurement correction: the first 65,536-byte assertion also passed on
baseline; it did not distinguish the models. The old model already charges
compensation storage. Use 327,680 bytes and require the explicit memory-routing
warning before LSMR work; retain baseline-routing-proof.log as the failed test
design, not evidence of a baseline defect.
