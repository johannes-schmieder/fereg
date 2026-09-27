# Categorical PPML separation: exact coefficient proof

Decision: adopt candidate04 for local development under
[ADR 0230](../../../../docs/adr/0230-exact-coefficient-certificates-for-ppml-separation.md).
The original sparse-zero failure cases now complete with independently verified
final samples. This is a correctness repair; diagnostic clocks are not headline
timings or fixest ratios. Linux confirmation and broader performance work remain.

## Source and mechanism

Base checkpoint: `5da73c0a`. `candidate04-source.json` binds all 211 computational
inputs, including the new graph module. The universal Mac plugin is
`571eccb27a6cafb3239b163a37344ffb3e0e917d56b2b7d53c5a73762f250a50`.
Neither this candidate nor local validation modifies the separately frozen
Schwarz-only SCC timing job 7760466.

The prior graph-plus-CG candidate failed while projecting an exact FE product
again to establish model-space membership. This candidate instead retains its
integer FE coefficients and independently reconstructs their observation product
through the original group-member lists. Exact equality proves span membership.
Every proposed deletion still passes the original independent sign, margin,
row-identity and generation checks. Remaining full-model numerical IR uses
current-weight retained-product CG with all original projection certificates.
No graph result supplies no-separation authority. The terminal chain, publication
IRLS, final score/objective/Hessian, inference and transaction remain mandatory.

Candidate01 established this scientific result. Candidate03 additionally requires
explicit detector-memory admission, so an unrelated solver budget cannot activate
the graph path. Candidate04 adds a finite-regressor-only separation control and
formats the implementation; all four licensed controls and all six probes were
rerun against its source-bound plugin. Rejected predecessors and their evidence remain in the
[earlier report](../separation-graph-repair-local-2026-09-27/README.md).

## Original failures and independent sample evidence

The inputs are literal hash-matched originals from the SCC campaign:

| N61440/K10/T4 case | Proven separated rows | Following singleton rows | Final N | Result |
|---|---:|---:|---:|---|
| Sparse two FE | 631 | 3 | 60,806 | Pass |
| Sparse three FE | 580 | 1 | 60,859 | Pass |

These include the previously missed 81 and 70 observations beyond zero-only FE
levels. `candidate04-sample-verification.json` checks every original observation
against the independent integer-potential proof plus independently implemented
singleton/zero-level closure. Both samples match exactly. No input row was
manually removed before native estimation.

The public Poisson oracle separately compares against explicit Stata dummy
Poisson and a separately formed Hessian/sandwich. It passes 12 fits spanning
two/three FE, unit/frequency/probability weights, robust/cluster covariance,
fast/low memory, T1/T4, offset and saved mean/eta/residual outputs. This avoids
treating ordinary reference weights as a frequency-weight inference oracle.

## Fresh cases and controls

`registration.json` predates the fits. All four fresh cases pass on candidate04:

| Design | N / K / T | Memory | Seed | Final N |
|---|---|---|---:|---:|
| Sparse two FE | 7680 / 6 / 1 | Fast | 202609275 | 7581 |
| Sparse three FE | 7680 / 6 / 4 | Fast | 202609276 | 7618 |
| Dense two FE | 1920 / 6 / 4 | Low | 202609275 | 1910 |
| Sparse three FE | 1920 / 6 / 1 | Low | 202609276 | 1903 |

Forty separation tests pass, including exhaustive independent enumeration
of all 512 three-by-three graph outcome patterns, reversed rows, malformed
coefficient/direction proofs, stale coordinates, refused graph authority and
transaction boundaries, including separation outside the graph submodel and its
nonseparated counterpart. Fifty-four memory tests pass on the unchanged memory
source. Counts are not a release qualification claim.

Four licensed fixtures pass with explicit terminal sentinels: the new separation
oracle, categorical PPML final Poisson/Hessian oracle, three-FE OLS oracle, and
multipartition PPML slope oracle. Mac binary audit and actual-plugin ABI/panic
containment checks pass. `candidate04-stata.json` and `candidate04-artifact.json`
bind that validation. Tests used a separate Cargo target from the accepted
Schwarz snapshot. No full qualification or static-allocation successor ran.

## Memory, timing and preserved failures

Graph traversal/replay and CG projection have separately admitted sequential
workspace; they do not remain live through publication IRLS. Optional refusal
preserves the previous route. Low mode also charges the dense matrices required
by separation. This fixes a failure, not a promise of reduced memory.

Candidate04's two original process clocks were 1.257 and 2.857 seconds; direct
child peak RSS was approximately 122 and 175 MiB. Fresh process clocks ranged
from 0.153 to 0.437 seconds, with 43–73 MiB peak RSS. These include process work
and diagnostic tracing, lack paired baseline/reference timing and do not support
speed or memory ratios. RSS comes from macOS `wait4`, not the static allocator
inventory. Local checks used a separate Mac while SCC headline fits remained
on their isolated node.

Candidate01's optional `/usr/bin/time -l` wrapper returned failure after all six
Stata applications passed because the sandbox denied `sysctl kern.clockrate`.
Those receipts remain in `candidate01-wrapper-failure.json`; direct-process
reruns passed. Candidate02's new boundary test initially supplied the OLS
default collinearity setting to a PPML projection and hit its policy-consistency
guard. The test now supplies the governing PPML policy; no tolerance changed.
Its failed test receipt remains, and no candidate02 plugin was built.

`replay_candidate04.py` preserves the diagnostic driver (candidate03 is retained) with the machine-local
Stata path supplied through `STATA_BIN`. `verify_original_samples.py` reproduces
the independent row comparison. Raw licensed logs, DTA inputs and plugins stay
under ignored `target/`. This does not resolve constrained/nonretaining CG,
allocation-inventory drift, all comparator sample differences, or the complete
<=1.20-stock/monotone-thread performance targets. A reference retaining a different
sample remains unranked even when the native failure is repaired.
