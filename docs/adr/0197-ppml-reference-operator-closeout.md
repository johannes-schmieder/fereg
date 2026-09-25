# ADR 0197: close reference-operator PPML proposals

Status: rejected for production, 2026-09-25. Local development, not qualification.

## Decision

Retain e7731129/finalreplay1 runtime. Reject chord2, based on 2d720820 with four
core-file overlays, and restore all four files exactly. Do not retain its
fixed-frame cache prerequisite separately. The
[source-bound report](../../benchmarks/reports/development/ppml-chord-closeout-local-2026-09-25/README.md)
preserves both integrated drafts, the earlier nonlinear diagnostics, tests,
complete timing distributions and rejected patch. No fresh validation seed is
consumed. The broad multi-FE 0.90-stock objective remains unfinished.

## Hypothesis and implemented boundary

Repeated weighted-system preparation might be avoided by modified Newton:
use a reference Hessian to propose directions from the current Poisson gradient.
For reference weights h_ref and current accepted eta/mu, solve the weighted
linear problem with response

```
z_ref = eta - offset + q * (y - mu) / h_ref.
```

This is a surrogate proposal, not a claim to solve the current q*mu Newton
equations. A successful first ordinary Newton step establishes the reference
operator. Later responses reuse FE geometry, finite products and factors only
when sample/topology/weight generation, finite basis/design and selected-column
identities match. Changing dependencies or failed solves revoke product reuse.
The prototype uses existing low-memory original-column ownership and no second
observation-by-regressor bank. It adds a selected-column identity vector and
constant metadata to the joint workspace, with corresponding admission charges.

The streamed finite RHS uses original finite columns against the freshly
projected response, applying the projector's weighted self-adjoint identity.
Every new surrogate RHS still receives independent residual and finite-score
checks. The response is validated before mutation. Current Poisson objective
and outer scores stay fresh. Backtracking, stalled score progress, numerical
refusal or entry to a strict checkpoint revoke reference reuse. The strict
checkpoint restores exact current weights and response; final rank, reconstruction,
projection, bread and exact q*mu Hessian remain independent authorities.

Exploratory separation, fast execution and allocation-equivalence probes keep
their existing routes. Chord1 also enabled the prototype with canonical-final
disabled and failed an existing checkpoint-routing assertion. Chord2 requires
the canonical-final policy; the unchanged assertion and all focused tests pass.
Neither draft changes an accepted production policy or numerical tolerance.

## Evidence and rejection

An isolated dense mathematical diagnostic, starting from actual first-accepted
states, needs four fixed-reference steps/one setup versus three fresh steps/three
setups on the large categorical case, and three steps/one setup versus three
steps/three setups on the narrow case. This warranted integration but was not
public performance evidence. The earlier nonlinear block-coordinate alternative
needed 13/9 accelerated sweeps and was not selected for production integration.

Fourteen distinct Rust tests and six licensed scientific fixtures pass for
chord2. All 64 public states, 41 baseline comparisons and 18 direct stock pairs
in its two headline and two trace blocks pass. Nevertheless, three-rotation
pilot medians B/C/stock are 112/110/98 ms for low categorical, 39/39/34 narrow,
80/75/79 low slopes, and 80/80/78 fast slopes. Wide controls produce 86/85/127
with a losing candidate pair. These improvements do not establish a broad
advantage commensurate with another outer-solver policy.

The predeclared frozen-source extension to unbalanced three-FE slopes produces
196/310/231 ms: all three candidate pairs lose. Outer iterations rise 4 to 7,
inner work 54 to 137. The closing trace shows linear score reduction under the
reference operator, two apparent-convergence passes at iterations 4/5, then a
correct strict-profile rejection at iteration 5. Exact Newton resumes and
passes at iteration 7. The strict boundary performs its required scientific
job; its tolerance must not be relaxed to retain this optimization.

For the large two-FE case, projection telemetry falls 23.771 to 20.050 ms while
response/weight refresh rises 2.412 to 3.946 ms. Initial/strict system work and
outer iteration counts remain unchanged. On three-FE slopes, strict work rises
47.376 to 128.869 ms. These are phase medians, not additive attribution or an
instrumented-stock comparison. The evidence rejects the premise that avoiding
these few system preparations alone closes the remaining complete-command gap.

## Consequences

Do not tune reference age, score thresholds or topology-specific exclusions on
these cases. A later modified-Newton proposal would need a materially different,
independently justified work/progress policy and a new measured opportunity.
Do not adopt the dense diagnostic's matrix as a low-memory implementation.

Continue with shared preparation and independent checking inside the exact-weight
algorithm. Consolidate repeated original-column normalization/norm/mean/score
and reconstruction passes, with dependency-valid ownership across consumers;
canonical rank and final-Hessian construction remain substantial separate work.
The narrow-fit lifecycle remains another target because this prototype does not
reach its fast LSMR route. Preserve known scientific, memory-inventory and
platform gaps. No push, release, SCC retry or qualification follows this result.
