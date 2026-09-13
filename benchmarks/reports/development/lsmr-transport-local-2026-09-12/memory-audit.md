# P0: LSMR model diagnosis at 15bd21d

This audit establishes undercounts; it does not adopt a new memory formula,
change routing, or reinterpret historical exact-source receipts. The companion
transport change reduces actual lifetimes while these accounting defects remain
open. Source references below refer to the pinned inputs in `identities.json`.

## Reporting and admission are distinct

`memory::MemoryPlan::selected_bytes()` returns the chosen fast/low execution
estimate. `estimate.rs::build_runtime_shadow` publishes that value as runtime
row 25 and uses it as the allocation-ratio denominator. Topology-dependent
solver increments are checked by the native joint solver budget but are not
carried through `SolveReport` or the prepared execution report into this scalar.
Thus a native/public-model excess does not by itself measure admission error.

There is also a genuine admission undercount. `estimate_fe_solver_memory` charges
one observation-sized recurrence/history block, with no RHS multiplier. The
pinned `within` batch implementation runs an independent solve for each RHS.
Each owns observation/coefficient recurrence vectors and coefficient-space
history. Correcting only the public denominator would conceal that defect.

## Independently measured recurrence storage

Pinned within/schwarz-precond revision:
`1ac3522a251c2622dbeab451415924f9ebe117b9`.
Its `ModifiedGolubKahanBuffers` owns two N-vectors, three L-vectors, and two
history lanes of length hL. `SolutionState` additionally owns three L-vectors.
Here L is total FE coefficients and h=min(requested history,N,L). Cold modified
LSMR therefore owns

```
8 * R * [2*N + (6 + 2*h)*L]
```

bytes of recurrence payload if R recurrences coexist. The unpreconditioned
variant instead has five L-vectors and one history lane. Weights, row permutation,
scatter, preconditioner application, RHS transport and batch results are extra.

The isolated [allocator probe](allocator-probe/src/main.rs) implements a separate
rectangular operator with allocation-free matvecs and identity preconditioning.
A bounded barrier in the first iteration ensures all requested recurrences
coexist. A single iteration deliberately does not certify convergence; this is
an allocation test, not a scientific oracle. Six cases pass against the actual
pinned recurrence implementation; [bound receipt](recurrence-final.json):

| N / L / simultaneous RHS / history | Vector prediction | Observed allocation peak |
|---|---:|---:|
| 8192 / 4096 / 1 / 0 | 327,680 | 328,016 |
| 8192 / 4096 / 1 / 4 | 589,824 | 590,160 |
| 8192 / 4096 / 4 / 4 | 2,359,296 | 2,359,632 |
| 16384 / 4096 / 4 / 4 | 2,883,584 | 2,883,920 |
| 8192 / 6144 / 4 / 4 | 3,276,800 | 3,277,136 |

The additional 336 bytes are outside the predicted vector payload. A sixth tiny
case requests history 100 with L=8 and verifies the history cap within an 8 KiB
bookkeeping envelope. This independently distinguishes observation and
coefficient axes and proves the per-RHS multiplier. The probe's unsafe allocator
exists only in the standalone diagnostic executable; fereg's core stays safe.

## Pinned batch allocations

Four [source/artifact-bound batch probes](watched-probe.jsonl) create a real
`within::Solver`, then reset the allocator peak before `solve_batch`:

| N / RHS / threads / preconditioner | Current fereg solver model | Observed solve-only increment |
|---|---:|---:|
| 30,720 / 11 / 4 / diagonal | 5,114,240 | 13,146,800 |
| 30,720 / 11 / 4 / Schwarz | 5,225,352 | 13,396,736 |
| 122,880 / 11 / 4 / diagonal | 20,455,904 | 51,151,088 |
| 122,880 / 51 / 4 / Schwarz | 60,223,232 | 151,877,667 |

These increments exclude the already retained operator and fereg's adapter
copies. The four-thread Schwarz probes converge in 31/34 steps. The fixed
80-step diagonal probes remain unconverged and are not scientific acceptance
evidence. Setup/retained deltas are also archived; their observation starts
after allocation of the supplied weight vector, so they exclude that buffer.

A watched allocation size tracks the two four-vector history lanes. These four
runs observe four concurrent recurrences. This does not prove that nested Rayon
scheduling always bounds live suspended workspaces by thread count. The existing
PPML checkpoint estimate already charges per-RHS coefficient history, but uses
min(RHS,threads); its concurrency assumption and Schwarz reentry pools also need
validation before sharing that model with OLS.

## Remaining source-derived gaps and next correction

- `within::Solver::solve_batch` retains individual RHS solutions while allocating
  combined coefficient/demeaned banks. Model those simultaneous banks.
- Each RHS owns a `DesignOperator` scatter buffer sized to its largest FE block.
  Small blocks use nested fold/reduce allocations; larger blocks use the retained
  atomic buffer. Current OLS scalar thread reductions do not describe this work.
- An unsorted dominant FE triggers within's owned categorical permutation and
  row-permutation vector. Weighted RHS and permuted input are per-RHS vectors;
  square-root weights and the design are retained across solves. The prepared
  fereg memberships are not passed to `Effect::new`.
- OLS Schwarz topology accounting lacks the complete dense cross-tabulation
  payload. PPML already has a more detailed pair-wise build/frame model in
  `ppml/categorical_lsmr.rs`, including dense tables and weighted corrections.
  Reuse and audit that machinery rather than introduce a second approximation.
- Schwarz pool capacity four limits pooled top-level objects, not a proven total
  count of every nested worker scratch buffer. Retained factors and simultaneous
  setup/apply lifetimes require a source-derived bound and allocator checks.
- Carry the admitted route model through attempted as well as final solvers.
  The final solver alone cannot explain a discarded setup, preconditioner retry,
  checkpoint retry or higher earlier peak. Preserve result-wire compatibility.

The measured small weak control reaches 55 MB against a 1.045 MB public model;
the automatic wide target remains 403 MB against 269 MB even after the adopted
transport saving. The independent batch probes establish why this cannot be
treated purely as a reporting defect. P0 remains open. No observed fitting cap,
new admission bound, universal allocator guarantee or corrected PPML concurrency
formula is claimed by this audit.

For replay, copy `allocator-probe` to a scratch `probe` directory beside a
`baseline` archive of `15bd21d`. Its Cargo manifest points to that archive's
core and the exact pinned dependency; use the included lockfile and release
build. The `recurrence N L RHS HISTORY` command runs the independent payload
test. Batch arguments are `N RHS THREADS LEFT_LEVELS RIGHT_LEVELS PRECOND WEIGHTED`;
the four complete argument lists and executable identity are in the receipt.
