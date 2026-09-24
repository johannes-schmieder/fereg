# ADR 0168: fused canonical products for low two-FE PPML

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Adopt the direct-product candidate selected in ADR 0167 at initial and changed-
weight canonical rank boundaries for admitted low-memory, two-categorical-FE
PPML MAP sessions. The ordinary joint IRLS direction, fast storage, slopes,
one/three-plus effects and one-shot OLS retain their existing implementations.
This is a production integration, not admission of the earlier algebraic Gram
subtraction, lazy-row design or frame-operator experiments.

`ols/canonical_products.rs` borrows immutable original numeric columns or their
existing generated-column recipes and finalized FE codes/membership indexes.
It eliminates the larger partition, assembles the exactly symmetric weighted
Schur Laplacian, grounds the largest-mass retained level in each connected
component, diagonally scales the free system and attempts guarded Cholesky.
Positive pair couplings are evaluated with the larger mass divided first.
An unusable coupling or factor declines the optional solve; it cannot establish
FE rank or change the existing canonical policy.

All response/control RHSs share this call's current-weight factor. A final
pass assigns complete eliminated groups to workers and reconstructs normalized
residuals directly from the original columns and solved FE coefficients. It
forms the full response/control Gram, within norms, and independent moments
against both original FE partitions. The one-use Gram/RHS goes directly to
`solve_full_retained_rank`; it is not recomputed through retained-row scans.
This replaces the separate response projection, repeated control-panel
projection/patches, and initial finite-moment scan on the admitted path.

Retained rows remain necessary for canonical QR/span, finite/inverse checks and
final inference. They are reconstructed once into the existing scratch format.
The new complete-row writer uses its existing bounded I/O tile and writes every
cell once, without reading old tiles. A callback, nonfinite value or I/O error
poisons the private file; readers and further rewrites refuse it. Dropping it
removes the file. Only fully certified results reach the unchanged publication
transaction. SPI access remains on Stata's main thread.

## Lifetime and admission

No factor, weighted geometry, coefficient solution or product cache survives the
call. Original response, selected generated-column axis, raw weights and solver
weight divisor are bound together before construction. A changing weight frame
therefore prepares fresh geometry; compatible RHSs share it within that frame.
The only handoff is certified rows plus same-call moments, consumed immediately
by canonical rank construction. Existing same-frame design reuse and response-
only solving are unchanged. Previous-weight guesses grant no authority.

Let N be rows, P the eliminated partition's level count, Q the other count,
W the response-plus-control width, and T = min(requested workers, P). Admission
requires positive dimensions, valid MAP controls, and

```
Q^3/3 + sum_g choose(min(members_g, Q), 2) <= N W (W+1)/2.
```

The left side conservatively bounds factor and pair-assembly work without an
allocated graph or benchmark-specific threshold. It deliberately refuses some
systems whose actual component structure could make a different organization
cheap. The current factor is dense even for disconnected components.

Every dynamic numeric/index reservation is checked and fallible. A conservative
payload sums all allocations without lifetime credits: two N-double banks,
group masses/scratch, two Q-square matrices, P-by-W coefficients, three Q-by-W
banks, scalar/norm arrays, the W-square Gram and T private reductions. A reduction
has W-square + Q-by-W + 5W doubles. Four Q-index vectors, 32W source-binding
bytes and 2,048 bytes of fixed descriptor/scalar allowance are included. Actual
Vec capacities are charged before initialization; unrepresentable shapes refuse.

This complete bank must fit in the already planned low projection's two
N-by-pool double reservations, less any already-live reusable projection-block
capacity. The existing scratch I/O bank, retained scales/means/norms, finite
workspace, prepared owner and strict rollback state remain separately charged.
No planner budget increases and no N-by-K original panel are introduced.
Numerical/work/budget refusal precedes scratch mutation and preserves the old
solver. Real allocation errors propagate through the existing atomic failure
boundary. This does not repair the separately open constrained-fit failures.

## Independent accuracy and checking schedule

The factor is a way to propose FE coefficients, not an acceptance certificate.
After solving, original-input residuals are independently reconstructed and
checked against every original FE level in the current normalized weight frame.
For each normalized column, the reported infinity measure is

```
max_g |sum_i_in_g w_i r_i| / sqrt(sum_i_in_g w_i)
------------------------------------------------.
                  max(1, ||r||_W)
```

Original column norms supply the normalization; zero columns retain their
zero-norm rank status. The pass also computes the RMS and raw absolute diagnostics.
Nonfinite products or a failed unchanged MAP tolerance decline the proposal.
This check never uses the factor's equation residual as its witness.

For a sum of m terms, ordinary floating-point reduction has absolute error at
most gamma_m times the sum of absolute terms, where gamma_m = m*u/(1-m*u)
when m*u < 1. The eliminated-level score sums visit each level's original rows;
retained-level scores use worker-local sums followed by a compensated merge.
Gram entries use FMA accumulation and the same compensated merge. Thus their
bounds apply to direct residual products, not subtraction of two large Grams.
Normalization limits column scale, and direct diagonal products preserve tiny
within variances without the additional `H-U'U` cancellation error identified
in ADR 0167. Reconstruction still has the ordinary rounding of original values
minus FE coefficients. These are forward-error descriptions, not permission to
add an error allowance to any acceptance threshold. Extreme-scale, near-FE-
collinear, thread and independently constructed contrast tests validate the
implemented reductions; exact old-recurrence bits are not required.

Canonical FE-collinearity thresholds, guarded finite factorization, pivoted QR
and materialized dropped-span fallback remain. The rank consumer's inverse,
finite-solve, original-input score, residual projection and reconstruction
checks remain independent of the new FE proposal. Outer objective/score,
strict accepted-point comparison, final-Hessian synchronization, covariance,
weight/DoF/sample/separation rules and transactional publication are unchanged.
There are no speculative iterations needing extra full certificates: one
factor proposal receives one full original-design certificate before use.
The direct FE solve reports one solve in existing MAP diagnostics; its count is
not an operation-equivalent MAP iteration.

## Evidence and limitations

The [source-bound report](../../benchmarks/reports/development/ppml-canonical-products-integration-local-2026-09-24/README.md)
contains exact source/artifact identities, declarations, complete distributions,
phase and memory evidence, failures and independently verified receipts.
Thirty-three distinct focused core tests and one plugin budget/publication test
pass. Ten successful licensed fixture identities cover explicit Poisson,
Hessian/sandwich, generated factors, FW/PW, cluster inference, saved outputs,
strict rollback, allocation audits and focused OLS controls. The initial
factor-oracle draft aliased the factor with an FE and failed; its evidence is
retained and the corrected independent design passes.

Fresh seed 202609379 was declared before generation and is consumed/frozen.
Five stock/baseline/candidate rotations include robust inference and saved
means. All 180 applications and public-state checks, 60 native comparisons and
60 candidate/stock comparisons pass. Wide N23040/K36/T8 improves 160 to 112 ms
(stock 129); larger N51840/K24/T8 improves 226 to 168 (stock 200). Every native
pair improves in these families. Serial wide and weak designs also improve.
No default-reference accuracy claim substitutes for the pinned stock comparator
at glm.tol=1e-10 and fixef.tol=1e-11.

The unbalanced fresh case improves 115 to 104 ms but trails stock's 88.
K72 refuses the conservative memory admission (4,421,888 required versus
4,300,800 available bytes); its 406 versus 352 ms loss remains. Disconnected
Q352 refuses dense factor work; 88 versus 70 ms remains. These refusals are
structural, not benchmark routing. The new disconnected comparison passes;
it does not adjudicate the earlier seed-378 stock discrepancy.

Whole-action tracked peaks fall on admitted anchors and modeled budgets remain
unchanged. This is not equal-budget stock benchmarking or constrained-memory
platform qualification. OLS two-FE and slope timing medians are unchanged;
small one-FE and three-FE controls each add 1 ms and remain visible. No universal
10% advantage or zero-regression claim follows. The broad goal remains active.
