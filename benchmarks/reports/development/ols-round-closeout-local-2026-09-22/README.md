# OLS optimization round: local development closeout

Johannes requested ending this round and moving to other optimization work,
even with remaining serial-slope losses. This checkpoint commits the six
remaining graph, residual-pass and scoped LSMR transport files without changing
their measured bytes. It closes the development round, not the original goal
of beating fixest in every domain. [ADR 0101](../../../../docs/adr/0101-ols-optimization-round-closeout.md)
records the decision and guards.

## Source and retained work

All 523 computational input hashes match captured `dtk1`, whose native macOS
plugin SHA-256 is
`77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151`.
The closeout starts from commit `2ed67ac0bc0c8965ee9beb81e3729add5c797607`
plus exactly these existing source changes:

- `fe/borrowed_map.rs`: admitted original-input Schur fallback, retained products,
  actual continuation reporting, stable correction reconstruction.
- `fe/cmg.rs`: guarded same-weight final graph reuse, admitted correction products
  and batched subtraction with pre-mutation checks.
- `fe/lsmr_batch.rs` and `memory.rs`: the scoped serial retained OLS window and
  matching admission model in ADR 0095.
- `ols/prepared_fit.rs` and `ols/prepared_rhs.rs`: solver follow-through, guarded
  weighted overlap and fewer residual-mean passes with underflow fallback.

Paths above are under `crates/fereg-core/src/`. The previously accepted slope
geometry/recovery, loader, score-mean and small covariance improvements remain.
Rejected small-Gram, norm-fusion, traversal and batched-fresh-check experiments
remain reverted. Experimental cycle routing remains absent.

`source.json` records all computational hashes and the exact artifact binding.
`changes.patch` captures the six-file delta. The existing
[dtk1 archive](../fixed-dense-triangle-local-2026-09-22/README.md)
preserves the reconstructible source and artifact/build identity. No rebuild or
new headline timing was necessary: the source and plugin bytes are unchanged.

## Closeout checks

Focused release-mode Rust tests exercise Schur admission and forced fallback,
independent MAP projections, CMG correction products and mutation rejection,
LSMR window equivalence and later-window failure, memory envelopes, weighted
overlap/error ordering, extreme residual-mean scales, slope geometry and shared
PPML exclusions. See `core-tests.json` and the sanitized Rust logs.

Three licensed Stata/MP 19 fixtures pass against the bound plugin, each with
process return code zero, explicit Stata return code zero and exactly one
terminal success sentinel:

- `ols_narrow_weighted_schur`: explicit weighted dummies, robust/clustered
  covariance, predictions, saved effects, literal frequency expansion and
  weight-scale checks.
- `estimate_ols_saved_outputs_parity`: saved-output parity and reconstruction.
- `estimate_ols_slope_only_multiple_parity`: the frozen multiple-slope fixture.

`stata-summary.json` preserves application results; `checks.json` binds the
fixture/runner hashes, local raw-log hashes and source. Raw licensed logs and
binaries are excluded. These are focused integration checks, not qualification
or a rerun of the full benchmark matrix.

The static allocation-source audit still fails against its frozen inventory.
This is the previously documented post-optimization source/inventory drift,
including already committed files outside the six-file delta. The failure is
preserved in `allocation-audit.txt`; no manifest was silently refreshed to make
it pass. An initial audit invocation used a Python without `tomllib`; the
supported project interpreter ran the actual audit. A reviewed inventory
successor remains required before qualification.

## What the existing measurements establish

| Evidence | Result | Scope |
|---|---|---|
| [Slope recovery](../slopes-recovery-local-2026-09-21/README.md) | K10/T16 51 vs 58 ms; K50/T16 141 vs 184 ms | Historical recovery artifact; early threaded baseline was 336/854 ms |
| [Final categorical Mac blocks](../current-ols-final-blocks-local-2026-09-21/README.md) | 93/93 cell medians beat fixest in both blocks | Captured `owf1`, not the full later `dtk1` source |
| [Scoped window](../scoped-ols-window-local-2026-09-21/README.md) | Unit ring K50/T1 saves 121,868,720 tracked bytes; paired time ratio 0.9948 | Captured scoped-window screen |
| [Small covariance triangles](../fixed-dense-triangle-local-2026-09-22/README.md) | Confirmed modest serial robust gains; exact exports and tracked peaks | `dtk1` focused screen, confirmations and ordinary-FE controls |
| Same `dtk1` screen | AW K10 robust/full 226 vs 188 ms; PW K6 robust/full 209 vs 152 ms | Remaining serial losses, approximately 20% and 38% |

Historical reference discrepancies remain recorded. The
[six sparse-case investigation](../sparse-reference-stopping-local-2026-09-22/README.md)
localizes those cases to reference stopping behavior using independent oracles;
it does not replace the stock comparator or change benchmark tolerances.
Ordinary fixest weights remain unsuitable as a Stata frequency-inference oracle.

## Deferred work

This round stops serial-slope tuning. Linux T1/8/16 and constrained-memory
confirmation await renewed SCC access. Cycle routing is still experimental;
the two frozen validation seeds must not be reused for tuning. Weighted
constrained-fit failures, explicit/insufficient-budget nonretaining CG span
instability, the reviewed allocation inventory and exact-final-source broader
validation remain open. There is no universal-win, memory-neutrality, release,
installation or cross-platform acceptance claim.

The next development task can target a different subsystem without first
eliminating the remaining serial-slope gap.
