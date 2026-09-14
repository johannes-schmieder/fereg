# Three-FE discrepancy: reporting is incomplete; a cap breach is not established

The captured fact is unchanged: native peak 50,265,168 bytes versus public
selected-memory value 22,983,168 bytes, identically on both artifacts. This
comparison fails the experiment's reported-model check, but alone does **not**
prove that execution exceeded the actual joint admission budget.

Source inspection at accepted `25bf610` distinguishes two quantities:

- `MemoryPlan::selected_bytes()` in `crates/fereg-core/src/memory.rs` returns
  the base fast/low execution estimate.
- `estimate.rs` constructs `FeSolverMemoryBudget` with that base as
  `execution_bytes` and the available usable limit as `usable_bytes`.
- `FeSolverMemoryBudget::fits()` checked-adds incremental solver workspace to
  the execution model and compares the sum with the usable cap. Solver adapters
  use this separate admission boundary.
- `docs/memory_model.md` already states that public selected-memory reporting
  omits topology-specific solver increments. The earlier
  [expanded reporting prototype](../solver-memory-reporting-expanded-local-2026-09-13/README.md)
  was not adopted and left this gap open.

The new three-FE observation therefore adds a concrete original-input case to
the known P0 reporting gap. The existing captures are not rewritten, and the
residual prototype remains rejected independently because PW misses its repeated
primary command-gain gate. Do not increase a memory multiplier to cover the
observed number or characterize this case as a demonstrated cap breach.

Next complete action-level base/solver/overlap reporting across actual attempts,
including discarded and fallback work, and test the joint boundary on these
same original inputs. Determine whether any admission defect also exists from
that stronger evidence. Preserve weak-graph timing controls: the previous
reporting prototype had repeated weak-wide OLS overhead and incomplete slope,
generic, recovery and construction coverage.
