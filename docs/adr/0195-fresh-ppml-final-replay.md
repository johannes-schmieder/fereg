# ADR 0195: shared fresh final PPML replay and linear sample-ledger checks

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

Evaluate final fitted and null Poisson likelihoods in one fresh original-input
row pass after computing the null mean. Compute outcome log-Gamma once within
each row and share it between the two sums. Keep stable per-row deviance and
likelihood formulas and compensated reductions. Do not form a likelihood by
subtracting large aggregate sufficient-statistic totals.

Compute the null mean from separately scaled positive weights and outcomes.
Validate raw domains first; accumulate normalized weights/products with
compensation. Any nonnormal positive intermediate or result refuses the shortcut
to the existing log-space method, so a small product cannot silently underflow
before a large outcome scale restores its significance. The all-zero outcome and
unrepresentable final state retain their failure classes. This is a numerical
implementation change; it does not redefine the constant-only null model or
public fit statistics. Normal-case null values can change rounding.

After existing strict-order/range checks, traverse sorted retained and dropped
observation lists together. Require exactly one membership per candidate row and
agreement with its active mask and drop reason. This replaces two binary searches
per row with a linear ordered pass. Preserve independent count, generation, seal,
method-certificate and requested-chain checks.

## Independence and scope

The final replay reads original outcomes, prior weights and published means. It
never takes cached outcome terms or objective values from PreparedPoissonInputs.
Compare the freshly computed objective to the reported objective exactly as
before; the replay cannot overwrite drift. Only the independently recomputed null
result is passed into likelihood-statistic publication.

IRLS state, stopping schedule and tolerances, current-weight projection and finite
scores, model-space/reconstruction, canonical rank/bread, exact final q*mu Hessian,
VCE/DoF, separation/sample semantics and transactional publication are unchanged.
No weighted authority crosses a generation boundary. The row arithmetic helper is
shared with existing objective evaluation; independent mathematical oracles check
it, and existing threaded/prepared-family bit comparisons remain green.

This common publication boundary reaches categorical and slope PPML in both memory
modes. It does not change public OLS or introduce another solver. No observation
bank, heap allocation, persistent cache or worker team is added. Safe-core/FFI,
main-thread SPI and failure atomicity boundaries remain intact.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-final-replay-local-2026-09-25/README.md)
retains all distributions and receipts. Diagnostic certification drops from
8.101 to 5.728 ms on large low mode and 3.346 to 2.491 ms on narrow fast. The first
public pilot is mixed: one median improves 1 ms and two lose 1 ms. One predeclared
fresh block follows the measured phase saving without source changes or tuning.

Fresh low improves 114 to 112 ms and slopes 106 to 105; three other medians tie.
There are 12 winning pairs, six losses and seven ties. This modest common-work
reduction earns retention without claiming broad target attainment. Low and
narrow remain 1.191 and 1.147 times stock; no further tuning of this isolated
publication experiment is planned.

Fifty-two Rust tests, six licensed independent fixtures, 114 public states,
74 baseline comparisons, 34 direct stock comparisons and nine exact OLS control
pairs pass. All 34 production PPML pairs have exact coefficient/covariance/mean
exports. An independent fsum/SciPy audit validates 68 null-statistic states.
The new draft rational-mean test's initial bit-equality failure is preserved;
only that new assertion is corrected to allow two machine epsilons.

Scratch and tracked peak deltas are zero; selected-model telemetry varies by
48 bytes in either direction. No physical-memory or new allocation qualification
claim follows. Frozen definitions and known failures/discrepancies remain open.
Seed202609430 is consumed. The larger multi-FE 0.90-times-stock objective remains
active; current implementation priorities belong in STATUS.md.
