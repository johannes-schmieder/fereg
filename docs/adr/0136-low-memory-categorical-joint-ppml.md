# ADR 0136: joint categorical PPML directions in low memory

Status: accepted for local development, 2026-09-23.

## Decision

Extend the existing joint finite/FE Schur engine to publication MAP sessions
with multiple categorical effects, finite controls and low storage. This
supersedes the categorical exclusions in ADRs 0130, 0131 and 0134. Use the same
block-preconditioned conjugate-gradient recurrence, current-weight geometry,
bounded scratch scans, strict Newton witness and independent final construction.
No new solver family, tolerance, rank policy or public option is introduced.
Fast categorical LSMR, one effect, mu separation, other solver families and
public OLS retain their existing routes. Slope routing is unchanged.

Joint ordinary directions avoid repeatedly constructing the complete within
design and inverse. They carry no rank, bread or inference authority. Current
finite moments, original-input scores, FE projection and predictor reconstruction
remain mandatory. Changed weights rebuild numerical factors; sample/topology/
weight generation authority permits only compatible geometry reuse. A retained
representative or previous response is an initial input, never current acceptance.

## Distinct ordinary and canonical duties

For two or three categorical effects, retain the established borrowed categorical
projector for initial rank and canonical strict/final transactions. The generic
owner holds the one owned FE design and joint workspace; the canonical projector
borrows immutable membership from preparation. There is no second owned topology
or N-by-K design. Four or more effects use the generic coefficient projector at
canonical boundaries, replacing the generic repeated-MAP implementation there.

This distinction is evidence-driven. An initial candidate used the coefficient
projector for all categorical phases. It failed the existing small dynamic-range
test: disconnected components with very different masses produced relative
predictor error 7.44e-10 against the unchanged 1e-10 envelope. Supplementing its
stopping test with a local relative criterion did not converge, including a
bounded attempt to separate recurrence scheduling from fresh checks. Neither
probe is adopted. The final organization passes the original test and keeps the
established local-mean arithmetic where that projector is available.

After two accepted outer convergence passes, the strict current-weight joint
Newton witness must agree with the accepted predictor, coefficients and objective.
Then one independent canonical construction establishes within design, rank,
finite solve, inverse, reconstruction and final-Hessian synchronization, holding
the accepted state fixed. Exact raw-weight identity is still required. Numerical
refusal, changed basis or a moved state invokes the existing canonical retry;
rejected transactions roll back. No scientific check or threshold is removed.

## Admission and lifetime

Admit certificate geometry, coefficient workspace, prior response and joint
finite payload against the complete existing session envelope. Optional coupling
and parallel certificate results retain their separate admission. Two-/three-FE
coexistence additionally charges both boxed owner descriptors, borrowed effect
records and seven RHS scalar buffers. The base envelope already includes generic
projection/certificate storage and categorical observation/graph banks; do not
charge those payloads a second time. All canonical capacities enter the session
fingerprint. Both owners are released before inference.

Budget refusal retains the previous route. If certificate geometry, coefficient
workspace or prior-response allocation is refused after owner construction,
recover the owned FE design by consuming the generic owner and restore the
categorical projector without cloning topology. Failure to construct required
canonical storage remains a transactional allocation error. No worker calls SPI;
safe-core/FFI and publication boundaries are unchanged.

This is bounded low-memory execution with an explicit speed/storage tradeoff,
not a claim that optional joint state is free. Fresh tracked increments range
from 1.11 to 16.02 MB and remain below the admitted model. Fixed-configuration
production-plugin audits at pool widths 1/2 and T1/T4 have exactly zero allocation
events and zero current growth, stable capacities and warm/cold equivalence.
The older mixed-thread reclamation failure, static inventory drift and deferred
physical-cap/platform checks remain open.

## Evidence and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-categorical-joint-production-local-2026-09-23/README.md)
preserves both candidates, rejected numerical probes, independent truth and
Poisson/Hessian checks, public rotations, fresh cases, controls and all losses.
Fresh median native reductions are 15.7–97.7%; four of six fresh cells are below
0.90 times comparable stock. Dense development and two other fresh cells still
lose. The sparse development fitted-mean discrepancy remains unranked.

The dense diagnostic accepts both its strict joint witness and canonical final
transaction; the approximately 170-ms final phase is real remaining work, not
repeated refusal. Do not infer that its current implementation is an unavoidable
cost of certification. Retain this coherent gain, keep the rejected all-coefficient
canonical evidence, and move to whole-command one-FE concentration before another
long categorical checkpoint tuning cycle. The broader PPML objective is unfinished.
