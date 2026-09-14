# P0 CMG and Schur memory reporting: tested continuation, not adopted

This continues the [compact carrier](../action-memory-carrier-local-2026-09-14/README.md)
on evidence HEAD `2739915`, accepted native `25bf610` and isolated baseline
plugin `e6978061`. The candidate stays isolated. Public selected-memory reporting,
production source and installed dist are unchanged; there is no new speed or
whole-action memory claim.

[Identities](identities.json) bind 178 baseline and 179 candidate inputs, locked
dependencies and the test executables. The full [implementation patch](implementation.patch)
is `2e03030b9150fd0c9aa945344330afc0be4ab06e6d9822c0de5740b213268445`.
The [incremental patch](incremental.patch) changes five files relative to the
previous frozen carrier, whose patch is `082e1625`. The full patch changes
13 files relative to accepted native source. A fresh [replay](patch-replay.json)
reproduces all 179 candidate inputs.

## Implemented coverage

| Boundary | Reported workspace and lifetime |
|---|---|
| Borrowed CG | Ten MAP numeric banks, eight recurrence scalar banks, three recurrence matrices when allocated, and optional coefficient banks including metadata. Capacities are counted before ownership handoff; initial certification avoids charging unallocated recurrence matrices. |
| Independent borrowed certificate | Same certificate arithmetic, with the fixed workspace payload returned alongside it. |
| CMG construction | Reuses the existing admitted conservative preflight. The caller-owned scalar survives a later setup error, hierarchy rejection or direct-CG preference. Rejected admission records no unallocated construction attempt. |
| CMG owned bulk projection | Existing projection envelope plus the actual full-width coefficient output; output width may exceed the CMG batch width. |
| CMG ordinary polishing | Exact retained upstream hierarchy/plan/pool and nuisance capacities coexist with CG, retained coefficient output and reference norms. Recount after bulk solving observes lazy pool growth. |
| CMG final residual | Certification retains the hierarchy. If correction is needed, the hierarchy is released first; the report takes the maximum of those stages rather than adding the released hierarchy to CG. A passing certificate allocates no CG recurrence. |
| CMG reference norms | Counts the three-vector construction phase and the single vector retained afterward. |
| OLS Schur | Counts the allocated three-RHS probe and reuses the admitted full-attempt model. Forced CG continuation includes the surviving original reference norms after the Schur session is dropped. |
| Prepared OLS attempt merge | Carries discarded CMG setup and rejected Schur probe peaks into the selected result, even without a setup timer or warning; consumes each pending peak once. |

The CMG construction and projection numbers are conservative model envelopes,
not measured allocation peaks. Capacity reports cover principal owned workspace
payload. The implementation does not change budget decisions, reference scales,
iteration limits, weighted operators, certificates, rank or publication policy.
No accounting is added inside numerical recurrence loops; workers never call SPI.

## Validation

[Validation](validation.json) records **86 distinct focused core tests and 95
serial plugin unit tests** on the final frozen source, using Rust 1.85.1 and
locked offline dependencies. The suite includes six new tests and strengthened
existing hierarchy/Schur oracles:

- Closed-form CG residuals under deliberately oversized reservations, one/four
  workers, an already-within early exit and retained coefficient products.
- CMG admission rejected before construction; a real one-iteration nuisance
  construction failure after admission; the existing mixed-component hierarchy
  rejection and all-tiny accepted control.
- Lazy upstream pool growth from one to four workspaces; current retained bytes
  increase and are included in the result.
- Five-RHS coefficient recovery through a two-RHS CMG batch, independently
  reconstructed fitted effects, retained polishing and released final correction.
  A separate final certificate passes without correction. The phase-isolation
  test clears the already-accounted construction scalar; it is not a new public
  memory-cap experiment.
- Weighted Schur versus independent MAP, preserved weighted-cut rejection,
  deterministic thread/effect ordering, forced continuation and the existing
  exact/one-byte-short admission boundaries.
- Weak-chain and retained-LSMR controls, generic/owned/direct-CG admission,
  low-memory report merging, and plugin transaction/allocation-failure checks.

The final runs produce no compiler warnings. Earlier script anchoring, a test's
incorrect upstream pool accessor, and production imports of test-only adapters
are preserved in [development findings](development-findings.json). All 95 plugin
tests passed in the preliminary adapter build, but that build emitted warnings;
the adapters were restricted to tests and validation reran on the final source.
No scientific assertion, tolerance or fixture was loosened.

## Remaining work

This checkpoint does not close P0 or adopt the reporting prototype. Warm retained
LSMR/operator overlap, retained MAP coefficient conversion, failed CMG bulk work
with requested coefficients, prepared PPML parent/frame accounting, specialized
slopes, recovery, inference and final action/public transport still need coverage.
CMG construction without an existing budget remains explicitly unmodeled.
Unknown scopes propagate rather than appearing as zero or a complete bound.

No release-plugin build, licensed-Stata oracle, universal/ABI audit, new public
cap check or performance comparison ran here. Verify the original three-FE joint
boundary and the earlier weak-wide reporting cost control after complete action
reporting is connected. Admission questions require original-input evidence;
this report establishes no new cap breach and changes no admission policy.

Full pass/copy/build and physical-attempt accounting also remain open. Seven
P0–P7 items remain; R1–R5 are excluded. Independent reviews, upstream CMG assessment
and the bounded SCC campaign follow P0–P7. The dependency pin, installed plugin,
97 unrelated STATUS additions and all 358 original untracked paths are preserved.
