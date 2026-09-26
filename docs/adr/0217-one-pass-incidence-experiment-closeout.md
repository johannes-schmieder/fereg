# ADR 0217: close the one-pass canonical incidence experiment

Status: rejected for production, 2026-09-26.

## Experiment

The implicit two-FE canonical builder aggregates and sorts group-pair weights for
its Schur diagonal, then repeats that work to pack incidence coefficients. The
candidate stored the first pass's ordered raw pair masses in the existing N-value,
N-index and P-descriptor banks. After current-weight grounding and scaling became
known, it compacted those banks in place, removing anchors and converting indices
to component-local coordinates. Dense construction retained its recurrence.

There are at most N distinct pairs. Removing anchors moves entries only left,
so compaction cannot overwrite unread input. Complete admission precedes these
allocations; coexistence already belongs to the existing setup bound. No bank,
allowance, cache, worker team or checking authority was added. Prefix/suffix
diagonal arithmetic, current-weight root guards, CG, original-input acceptance,
rank, inference and final-Hessian synchronization remained unchanged.

## Result and decision

Paired diagnostics reduced setup about **40–41 to 34 ms**. Packing saved 11–12 ms,
but retaining raw pair masses added about 5 ms to the first pass. Public pilot
million-row low improved **934 to 926 ms**; fast was nearly flat **859 to 858**.
Fresh large fast/low improved **664 to 655 /708 to 704**, but the disconnected
control slowed **92 to 94 ms**, repeated at **92 to 94** on identical inputs.
The factor control's **712 to 715** loss did not repeat (**715 to 714**).

The predeclared retention gate required useful public savings without a
reproducible excluded-path loss. It was not met. The small setup improvement is
not enough to justify retaining the candidate or tuning it against the fresh
validation seed. No causal claim is made that the compacting kernel itself
explains the complete-command disconnected loss.

Restore the accepted ADR 0216 production source exactly. Preserve the candidate,
test, patch, diagnostics, both control blocks and all unsuccessful comparisons
in the [source-bound report](../../benchmarks/reports/development/ppml-incidence-pass-local-2026-09-26/README.md).
Do not resume threshold, layout or seed tuning of this probe without materially
new evidence and a different computational organization.

The candidate passed 74 distinct Rust tests, nine licensed fixtures, 233 public
PPML state audits and 12 explicit Poisson/Hessian/sandwich comparisons. All 83
native PPML pairs and 18 OLS coefficient/covariance pairs were exact. Ten fresh
large-case stock fitted-mean comparisons fail the unchanged envelope and remain
unranked, as does one older small saved-output comparison. Both native arms agree
exactly; this experiment did not introduce those comparator differences.

## Next boundary

Current-candidate diagnostics leave about **16 ms** in canonical norm scans and
**20 ms** in reported-intercept assembly. The canonical builder already returns
current-call response means and within norms, but the retained owner transfers
only control statistics and finite Gram products. Investigate a single-use
current-frame statistic handoff, preserving the independent raw-residual-mean
identity and score checks. Prove weight units, post-reconstruction rounding,
zero/underflow behavior and changed-generation/rollback ownership; do not assume
a scaled Gram diagonal is interchangeable with a stable residual norm.

This is a different boundary with broader potential across canonical engines.
No scientific requirement is weakened or new handoff authorized by this ADR.
The multi-FE <=0.90-times-comparable-stock objective remains unfinished.
