# ADR 0218: close the canonical response-statistics experiment

Status: rejected for production, 2026-09-26.

## Design tested

Extend the existing single-use canonical moment handoff with the current original
response mean, reconstructed-response norm and constant-column norm. Accumulate
the response norm on actual materialized row values during the independent
original-input certificate pass, using stable scaled squares and a fixed-order
worker merge. Do not substitute the normalized Gram diagonal. Accumulate the
constant norm during weight normalization in original row order. Use the fresh
original response mean for the reported-intercept identity, retaining its
independent raw-residual-mean calculation.

Two- and three-FE canonical product builders share this boundary. The scalar
handoff is consumed within that same weight/response call, never stored in the
retained design. Generic paths keep their scans. Dead certificate row scratch
holds worker norm pairs; the response-only three-FE builder finishes its single
worker norm directly. No heap bank, pool, admission allowance, cache generation
or acceptance frequency is added.

Current solver-weight units and reconstructed-row rounding are explicit.
Fixed-worker reduction order can differ from the former scalar scan; numerical
acceptance, not bitwise recurrence equality, is the requirement. Independent
weighted/hypot and Poisson/Hessian oracles govern the experiment. Samples,
weights, rank, DoF, VCE, separation, fresh projection/score/model-space/objective,
reconstruction, final-Hessian and transactional publication remain unchanged.

## Result and rejection

Separate paired diagnostics remove about 16 ms of norm scans and 9–10 ms of
reported-intercept work, adding about 5 ms to weight preparation. Containing
canonical construction saves about 19–20 ms. Public million-row low/fast medians
improve **944 to 918 /866 to 844 ms**. Fresh large low/fast improve
**712 to 698 /657 to 648**.

However, the pilot three-FE slope control slows **147 to 153 ms**, the fresh
larger slope control **190 to 195**, and the one predeclared identical-input
repeat **148 to 151**. Fresh factor/four-slope and protected large OLS median
losses also remain in the report. The slope views decline the changed categorical
builder; the repeated whole-command loss is measured, but its binary-wide cause
was not localized. Do not attribute it to the new norm formula without evidence.

The declared gate required useful public savings without a reproducible control
regression. It fails. **Reject responsestats1 and restore residualrows2 exactly.**
All 567 computational inputs match the accepted artifact. Preserve the complete
candidate, test-only initial compile failure, science, distributions, control
repeat and restoration proof in the
[source-bound report](../../benchmarks/reports/development/ppml-response-statistics-local-2026-09-26/README.md).
Do not resume threshold, layout or favorable-seed tuning of this probe.

The candidate passed 76 distinct Rust tests, nine licensed fixtures, 223 public
PPML state audits and 12 explicit Poisson/Hessian/sandwich checks. All 78 native
PPML and 18 OLS scientific export pairs were exact. One existing small stock
saved-output discrepancy remains unranked; all 45 fresh stock comparisons pass.
Tracked heap/scratch peak deltas are zero; model bounds vary -112..+48 bytes with
unchanged allowances. No qualification or scientific policy change follows.

## Next implementation boundary

Leave canonical statistic/packing micro-tuning closed. Investigate the shared
accepted-point and working-state lifecycle in `ppml/memory.rs`, `family.rs` and
`prepared_irls.rs`. The current diagnostic IRLS-exclusive span is about 95 ms
fast /76 ms low; separate outer score checks cost about 40 ms. These figures do
not establish that all of the exclusive span is avoidable.

`accept_trial` swaps a validated eta/mean pair, then ordinary iteration refresh
recomputes means from eta before forming Hessian weights, working responses and
scores. Trial eta and mean construction also use separate passes. Attribute
these operations, then implement a coherent lifetime/pass organization using
existing banks. Prove which accepted eta/mean pairs remain valid after every
mutation, failed trial, normalization, strict rollback and generation change.
Prior-state guesses grant no authority. Preserve independent fresh scores,
objective acceptance, two convergence passes and exact final-Hessian replay.
This is a concrete broader hypothesis shared by categorical and slope PPML;
the <=0.90-stock goal remains active, not achieved by this closeout.
