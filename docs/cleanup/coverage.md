# Bounded M0–M1 acceptance matrix

The machine-readable inventory is [matrix.json](../../tests/replay/matrix.json).
It fixes 73 native replay cases and 20 existing licensed fixtures before M1
corrections. The previous rebuild mapping remains in Git at `599793c3`;
its deletion schedule, replacement architecture and cross-thread bit policy
are superseded by [PLAN](PLAN.md), [SPEC](../SPEC.md) and active ADRs.
This is a coverage mapping; completion is recorded only in MILESTONES.

## Actual release-binary checks

[replay.rs](../../crates/fereg-oracle/examples/replay.rs) reuses the independent
seeded generators. Metadata adapters call no production numerical code.
[actual_plugin_loader_test.c](../../tests/mock_spi/actual_plugin_loader_test.c)
loads one real plugin in each process. Replay inputs carry all columns, masks,
wire matrices, argument strings and optional SPI macros. Numeric input/output
uses IEEE-754 hexadecimal bits. Harness/process errors fail independently of
estimator return codes. Existing assertion-based ABI/rollback checks remain.

Every case compares candidate against the canonical comparator (or a separately
identified parent-next binary) and repeats candidate at the same thread count.
Scientific fields, complete receipts, outputs and available route signatures
must match exactly. Excluded fields are RMEMORY; process/timing portions of
RRUNTIME; OLS RSOLVER columns 9–10; PPML RSOLVER columns 14–16; and bias diagnostic
row 10 (elapsed seconds). Retained RRUNTIME fields are OLS rows 1–3, 22–24,
28–30 and PPML rows 2–11. All other matrices and observation outputs are exact.

| Feature | Native replay | Independent / frontend coverage |
|---|---|---|
| OLS/PPML, zero through three FE terms | 32 seeded cases per estimator | Dense QR OLS and damped-Newton Poisson, independent sample masks |
| Unit/analytic/frequency/probability weights | All OLS kinds; supported PPML kinds | Dense weighted targets; existing weight and singleton fixtures |
| Categorical intercepts, interactions, slopes, slope-only, factors, materialized time series | Seeded slopes; complete cases 12/66/294 | Explicit dummy materialization; parser/interaction/slope Stata fixtures |
| IID, robust, one-way and multiway clusters | Seeded IID/robust/one-way; complete case 12 multiway | Dense covariance, independent PSD target; cluster-aware/nested DoF in retained testkit and Stata fixtures |
| Explicit auto/LSMR/CMG, fast/low | Supported combinations in seeded cases | Retained solver rejection tests, weak-graph and 100-digit fixtures |
| Samples, residuals, fitted means/values | Every successful replay | Independent masks, coefficients, covariance, fitted/residual tolerances from SPEC |
| Saved FE channels | Selected seeded intercept/slope cases | Sum of saved observation contributions equals identified absorbed component; licensed partial-save fixtures |
| Separation | fe/simplex/ir/mu on zero-outcome designs | Dense finite target; existing dropping, sequence and output Stata fixtures |
| Mean bias correction | exact and fixed-seed randomized replay | Core `bias_exact_full_system_matches_independent_explicit_design`, randomized expectation/variation and frequency-multiplicity tests; licensed bias fixture |
| Transactions, interruption, allocation failures, panics | Existing ABI harness and retained native tests | Existing eclass/hidden-transaction Stata fixtures; no new cancellation framework |
| String sources, syntax, reference parity, stored results | Licensed fixtures in inventory | Existing parser/public command contracts; M1 adds only selector/fix-specific gaps |

The simple dense VCE corpus uses nonnested clusters because its correction uses
full model rank. Nested-cluster policies are exercised by retained dedicated
fixtures, not silently compared to a different correction. Scientific replay
checks finite coefficients/VCE, sample and fitted outputs; normalized constants,
DoF metadata and additional reporting are independently covered by the retained
core/testkit/public fixtures and included in every exact comparison.

## Explicit known defects

These are successful reproductions, **not scientific passes**. The corpus stores
independent intended dense answers alongside each observed failure.

| Case | Frozen behavior required in M0 | Intended M1 result |
|---|---|---|
| known-12 | Full reported covariance has minimum eigenvalue below −2e-6 and differs from independently clipped full covariance by more than 1e-6 | PSD repair of the full matrix including the constant; no slope-block restoration |
| known-294 | r(498), `categorical absorbed-DoF slope row is inconsistent` | Successful continuous-slope DoF validation with independently correct fit |
| known-66 | r(459), `prepared OLS finite-score regressor scale is not finite and positive` | Successful frequency-weighted retained-singleton fit with independently correct sample/results |

The factor-availability defect occurs in shared ado before the native request.
`tests/stata/development/reference_factor_precedence.do` records `regress` and
pinned `reghdfe` behavior; M1 adds engine-specific assertions before its fix.
Covariance and factor precedence may intentionally depart from pinned reghdfe.
Record those named differences rather than changing tolerances or parity silently.

## Bounded qualification commands

- `./scripts/run_tests.sh`: normal Rust/FFI, isolated existing allocator and mock checks.
- `cargo test -p fereg-oracle --profile ci -- --include-ignored`: retained 2,000-design campaign and independent oracle checks.
- `./scripts/run_plugin_replay.sh NEXT_PLUGIN BASELINE_PLUGIN`: actual binary corpus, fixed-thread repeatability, exact and independent checks.
- Run the quick licensed profile and the 20 inventory fixtures using the existing Stata runner, plus only M1 selector/fix-specific additions.

Coverage instruments the mapped long oracle suite and 73-case binary replay,
including the actual ABI assertions, in a separate next build. The complete
Rust/allocation/transaction suite remains a required uninstrumented check.
Coverage gaps remain explicit; no percentage is a gate. Canonical comparator
and timing binaries remain uninstrumented.
Only gaps revealed by this matrix or concrete failures add work to M1.3.

## M1.2a declared change boundary

Only next's ordinary-factor validation/expansion may change: rows excluded by
availability no longer cause factor errors or contribute expansion levels.
Eligible invalid values, including base/omitted terms, return r(452); native
callers now receive that same code. Samples, drop reasons and numerical results
on unaffected native cases remain exact versus parent-next M1.1 (`50de7189`).
The frozen binary and its candidate-wide frontend expansion remain unchanged.
`installation/factor_availability.do` first reproduced both engines' failure,
then checks missing outcome/regressor/FE/cluster/slope/weight/adjustment, analytic
zero weights, time-series gaps and outside-if sources, and independent explicit
dummy regress/poisson coefficients. Domain failures remain errors.

## M1.2b declared change boundary

`tests/replay/intentional_changes.json` permits only known-12 covariance,
Wald F and inference certificate changes against parent-next `744ef7f4`.
Coefficients, sample, observation outputs and route signatures stay exact.
The complete covariance including the constant must match the independent
Jacobi-clipped dense target and be PSD. A separate analytic two-by-two projector
checks the explicit dummy covariance in the retained testkit. Historical
Stata scientific receipt assertions run explicitly on legacy; adjacent next checks
require PSD and unchanged coefficients. Fixed-thread repeat differences are
never covered by intentional-change allowances.

The old two-/three-way Stata hex-literal receipt also fails on the unchanged
canonical legacy binary. Its scientific reference caps remain; exactness now
uses a fresh fixed-thread canonical repeat with the binary hash recorded,
then requires next coefficients to match. No numerical tolerance was relaxed.

## M1.2c declared change boundary

Parent-next `dbc8505a` reproduces known-294's r(498) slope-row validation error.
Only that case may change from an empty failed transaction to complete outputs;
input columns and wire metadata remain exact. Continuous-DoF slope rows may have
bounded redundancy, with coefficients and aggregate totals still checked.
A six-row intercept/constant-slope witness reproduces the validator failure
and rejects seven malformed report variants after repair. Independent replay
requires dense fit/VCE, sample, effective N and residual DoF; a licensed explicit
dummy regression checks the public fast and low paths. All other cases and
fixed-thread repeats remain exact, including the preceding covariance fix.

### M1.2d declared change boundary

The parent-next seed-66 frequency-singleton witness fails with r(459) during
finite-score validation. The intended change is a successful fit matching the
already recorded independent dense design, including effective N=514 and
residual DoF=501. The replay policy permits success/results only for known-66;
all other cases and input fields remain exact against the preceding next.
A public Stata regression additionally compares explicit dummy/slope fits and
checks frequency multiplicity, pruning and keepsingletons in fast/low modes.

The seed-66 replay initially allocated an extra constant slot despite its
slope-only design. The earlier r(459) masked that fixture error. M1.2d corrects
its constant request flag and output dimensions to match the public ado, then
replays those same
corrected inputs against both binaries: the parent must still reproduce r(459).
Data, term metadata and independent scientific targets are unchanged.

### M1 bounded qualification

`scripts/replay_plugins.py --thread-counts 2 4 8 16` reuses all 73 inputs and
dense targets. Each count has an exact repeat; cross-thread checks require
identical samples, drop reasons, coefficient statuses and discrete DoF. Floating
results use the original scientific tolerances. The 20 licensed fixtures and
32-fixture quick profile overlap; together with selector and correction tests
they comprise 46 distinct fixtures. Compact source/binary/frontend-bound
evidence is in `bench/history/2026-10-02-m1-scientific-qualification.json`.

## M22 numerical change boundary

Retained PPML frames, including initial, ordinary and strict frames, now estimate
finite-coefficient error from reconstructed row residuals and apply at most
three inverse-Hessian corrections when the existing
publication coefficient envelope requires them. Requested tolerances, samples,
rank policy and Hessian/bread calculations are unchanged. Coefficients, fitted
means, objectives and derived inference may change when a frame requires a
correction and along its subsequent IRLS trajectory. A frame receiving the same
inputs and requiring no correction retains its original arithmetic outputs.
The no-FE constant-outcome regression has the independently known unique
optimum (all slopes zero, constant ln(2)). The licensed strict fixture defines
unit/FW/PW, fast/low, threads 1/4 and robust/cluster inference. All 24 cells
pass on the isolated PPML correction; clean-source and Linux qualification
are recorded separately in the milestone ledger.

CMG's existing CG polishing uses the shared group-mean projection action to
avoid subtractive contamination outside the FE span, with optional products
recorded by that same action. It preserves the CMG bulk solve and all existing
energy, recurrence and FE-moment certificates. Ordinary standalone CG retains
its previous action. Cases requiring polishing can change coefficients,
residuals, VCE, iteration diagnostics and saved FE products; unpolished cases
and frontend/model/sample/weight/DoF semantics remain unchanged. Executable
Hadamard regressions independently check forward accuracy and reconstruction,
including weighted scaling, threads 1/4 and RHS widths 4/65 and the first width
beyond the fused tile. The direct polishing test requires product reconstruction;
the CMG batch test checks products only when its existing fallback retains them.
The full licensed near-within fixture checks estimation and final saved-FE
publication. All48 enhanced fast/low, threads1/4, weighted cells pass; saved cells require
original-model and saved sum reconstruction within1e-14. Final-source local
qualification passes; clean-source and Linux evidence is recorded in the ledger.
Successful existing recovery retains its original arithmetic. Only an intercept-only failed
strict target-span recovery invokes one fresh ordinary retained CG projection
of (y-Xb)-accepted residual, normalized by its maximum absolute value. This
projection permits a nonzero off-span remainder. Normalize the complete FE
coefficient product once, restore its scale, reconstruct the original model
residual independently and check FE orthogonality at the unchanged tolerance.
The final span certificate uses directly evaluated D*alpha, while independent
original-model reconstruction and known-design witnesses establish accuracy.
The host recomputes Xb from original columns before publication; it never
replaces the accepted fit, residual, coefficient or covariance. Only affected
saved FE/absorbed outputs and recovery work can differ from the parent.
Weighted disconnected known-design tests cover common scales 1e-100/1/1e100,
saved-only/full output materialization and contaminated accepted-residual
rejection. Slope failures keep their original errors; unsupported direct recovery calls
fail before intercept-only memory planning. The existing allocation audit runner
covers the admitted CG envelope
and one-byte-short r909; ordinary rollback/Break checks remain required.

Strict constant-outcome PPML also exposes a PSD eigensystem gap: the global
Jacobi stopping cutoff can pass while a small covariance row fails the
unchanged row-relative reconstruction certificate. Only a failed eigensystem
is refined against the smaller row scale, reusing the existing rotation budget
and admitted buffers. Accepted covariance arithmetic remains unchanged; PSD,
rank, clipping and reconstruction tolerances are preserved. A positive-definite
three-by-three block with a closed-form spectrum reproduces the old failure
under common scales from 1e-150 to 1e100.

Rounded Gram meat can lose score-factor null directions before B*M*B' is
formed: high-precision multiplication of the actual rounded meat still yields
negative variances in the strict PPML witness. Robust/one-way inference now
uses a conservative Cauchy/roundoff envelope to request a factor check at the
existing row-relative covariance tolerance. Zero/subnormal or nonfinite scales
request verification; the envelope does not certify observation-level group
score construction. The factor check independently accumulates forward/reverse
bread-transformed scores, preserving both compensated group-score parts and
multiplication remainders. It changes covariance/derived inference only when
that check disproves the original covariance or its diagonal is invalid; an
agreeing check preserves the old covariance, PSD report and certificate bits.
Strict PSD tolerances and clipping policy are unchanged. Multiway signed meat
retains its existing full-matrix policy. Independent exact-null, positive
near-null and representable-after-score-square-underflow witnesses prevent
variance clamping from masquerading as a repair. The cluster memory ceiling
now includes complete sorting, partial and merge banks; retaining the old
compact covariance reuses the admitted result-covariance allowance before
full-axis expansion. All final gates remain explicit in MILESTONES.md.
