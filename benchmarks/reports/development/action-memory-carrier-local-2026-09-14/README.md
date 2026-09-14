# P0 action memory reporting: partial implementation, not adopted

This checkpoint implements the first reporting layer against evidence HEAD
`d0c0e86`, accepted native `25bf610`, and isolated baseline plugin `e6978061`.
It does **not** replace public selected-memory reporting, establish a whole-action
model, or close P0. No new release plugin or complete-command measurement exists.
The [prospective protocol](protocol.md) retains the earlier weak-wide regression
control and the original three-FE FW generated-input case.

[Identities](identities.json) bind 178 baseline and 179 candidate native inputs,
the test executables, and [patch](implementation.patch)
`082e1625ce6cdfdbbb553efd5128a7872ead52aec1ed87be1264c8b3b6eeb233`.
The patch changes eleven existing native files and adds one. An isolated
[replay](patch-replay.json) applies it to the accepted baseline and reproduces all
179 candidate input hashes exactly. Main's native inputs and installed dist
remain unchanged; only this implementation/evidence checkpoint is archived.

## Implemented boundary

`SolverWorkspacePeak` is an eight-byte, eight-byte-aligned diagnostic carrier.
The maximum u64 value denotes **unmodeled**, separately from a known zero-byte
scope. Combining any unmodeled scope leaves the enclosing peak unmodeled.
Sequential attempts take their maximum; caller storage retained across a nested
scope is checked-added once. Model/admission calculations remain checked u128;
unaddressable compact values fail closed. No public Stata wire changes.

The implementation reuses existing admission quantities. The ordinary borrowed
MAP path also sums ten fixed numeric-bank capacities at construction, before norm
handoff; these banks cannot grow during projection or certification. This is a
workspace payload envelope, not a new observation-sized allocation or a hardware
memory measurement. No accounting enters numerical recurrence loops or SPI.

| Scope | Prototype coverage | Still missing |
|---|---|---|
| Generic owned MAP/CG/Irons–Tuck/SQUAREM/LSMR | Existing admitted workspace; preserved LSMR input; maximum across admitted fallback attempts | Direct low-level calls and absent-budget scopes remain unmodeled |
| Owned prepared FE adapter | Construction versus resident-design/unit-weight/caller-outcome overlap, relative to the original execution base | Whole fit/action overlap outside the adapter |
| Prepared direct intercept CG | Existing joint CG estimate, including retained coefficient allowance | Empty-design delegation conservatively remains unmodeled at this wrapper |
| Ordinary borrowed MAP | Fixed reserved numeric capacities, including over-reservation and moved norms | Saved coefficient conversion and its overlap |
| Borrowed automatic pilot/LSMR/CG | Admitted attempt envelopes; rejected-before-allocation LSMR is excluded | Previously retained LSMR operator overlap and developer-only bulk-polish scope remain unmodeled |
| Retained explicit LSMR | First construction/solve under an existing budget, without coefficient handoff | Warm operator/pool capacities and coefficient handoff remain unmodeled |
| Low-memory report aggregation | Maximum across batches; unmodeled scope propagates | Fit/inference/action transport of the aggregate |
| CMG, OLS Schur and PPML checkpoints | Default explicitly unmodeled; discarded outer setup/probes cannot disappear into a known direct-solver report | Construction, retained frame, attempted/fallback peak integration |
| Specialized slopes, recovery, inference | Explicitly unmodeled | Their existing estimates, lifetimes and final action/public reporting |

The original three-FE discrepancy is still an incomplete public report, not an
established cap breach. See the preceding [source adjudication](../generated-ols-residual-tiles-local-2026-09-14/memory-gap-adjudication.md).

## Validation and limits

[Validation](validation.json) records **41 distinct focused core tests and all
95 serial plugin unit tests** on the final frozen patch, Rust 1.85.1 and locked,
offline dependencies. It includes:

- Known-zero/unmodeled distinction, maximum/retained arithmetic, sentinel and
  overflow rejection, and eight-byte carrier/report alignment.
- Existing closed-form slope residual oracles and exact/one-byte-short admission
  for generic solvers, preconditioners, unit/supplied weights and one/four workers.
- Discarded explicit LSMR followed by MAP, with exact original residuals and
  certificates; automatic rejection of oversized LSMR followed by admitted CG.
- Direct prepared CG and owned-design/caller storage bounds; fixed-capacity MAP
  reporting under deliberately oversized reservations and norm ownership transfer.
- Constrained weak-chain routing, retained LSMR reuse and low-memory report merges.
  A new cold/warm test verifies that the operator builds once, both numerical
  results agree exactly, and unaccounted warm overlap remains unmodeled.
- Plugin unit transaction, allocation-failure and result-validation checks.
  Subprocess fixtures are counted once by name, with the terminal 95-pass summary.

Whole-function formatting initially tripped a source guard because rustfmt added
closure braces. The inspected transformation and the archive parser's initial
single-line-test assumption are retained in [development findings](development-findings.json).
Neither required changing a scientific tolerance, fixture input or test assertion.

No licensed-Stata oracle, universal build/ABI check, new cap experiment or timing
comparison ran. Eight-byte alignment is measured by the core test; improved
weak-wide performance is only a hypothesis. P0 adoption must await complete
construction/retained/recovery/inference coverage, original-input joint-cap checks
and the fixed paired command controls. Full pass/copy/build and physical-attempt
accounting also remain part of P0. P4/P6/P7 remain open; R1–R5 are excluded, and the
independent reviews, upstream CMG assessment and SCC campaign follow P0–P7.
