# P4 retained scratch and reference-statistic audit

Baseline: `d32f05e45519aaee5c5d44bc87caddb4f68482f3`. The allocation-only
and subsequent scheduling variants are separately frozen and measured.

| Boundary | Baseline cost / behavior | Change |
|---|---|---|
| Frozen finite solve | The caller moves out the authoritative `kept` identities. The callee allocates into the emptied slot for QR scratch, even when Cholesky succeeds. Restoring `kept` drops that scratch. | Reuse the existing `candidate_positions` bank. QR scratch dies before that bank is filled with the inverse-certificate coordinates. No arithmetic, QR policy, capacity or admission change. |
| Current-weight raw reference norms and means | One row loop interleaves all stored generated columns. It uses scaled sums of squares and compensated weighted means, preserving raw reference scales. | For fast one-categorical-FE PPML with at least 65,536 rows, eight stored columns and an installed team larger than one, assign columns to that team. Each column visits rows in the same order with the same accumulators. |
| Weight denominator | Compensated sum in original row order within the reference-statistic loop. | The parallel branch sums it once in a separate serial row pass. Every column reads current normalized weights itself. |
| Small, narrow, single-thread, low-memory, generic/slope and multiple-FE routes | Existing serial reference-statistic loop and route-specific solver. | Reference-statistic dispatch unchanged. The scratch-bank fix still serves eligible frozen solves. |
| Initial fresh solve | Raw response/X construction, raw norms and means, projection, within norms and finite solve. | Unchanged reference-statistic construction. This change serves retained refreshes, including ordinary, strict/checkpoint and final-current-Hessian consumers. |
| Preparation, separation, inference and saved publication | Existing row classification, sample generation, memory admission, weight rules, score reductions, independent certificates and final-state recovery. | Unchanged. No worker calls SPI. |

For R active rows and C stored columns, both refresh implementations evaluate
R*C raw values. The parallel route does not remove a mathematical pass: it
replaces interleaved reads with column-local work, recomputes normalized weights
and their square roots per column, and adds R denominator reads. This trades
more weight arithmetic/reads for locality and concurrency. It adds no resident
observation-sized buffer or thread-by-group-by-column bank. No hardware traffic
measurement is inferred from these source-derived counts.

Reference norms and means always use the current weight generation. Scaled norm
accumulation, compensated means and original row order remain unchanged; no raw
cross-product subtraction or stale cached product enters rank decisions. The
existing guarded Cholesky/QR path and independent projection, finite, predictor
and covariance checks remain authoritative. Public results remain subject to the existing successful solve and transaction
boundaries.

A temporary allocator backtrace localized a 32-byte allocation in a small debug
fixture to `stream_frozen_solve`'s identity `try_reserve`. That trace perturbs
allocation behavior and is call-site evidence only. Uninstrumented builds and
the real action allocator supply the performance/memory/certificate receipts.
The weak-graph profile retains five other events; their origins remain open.

Existing phase timers overlap: strict/final includes subordinate projection and
finite work, and fresh session work contributes to preparation. The isolated
scheduling patch changes only reference-statistic execution, so its command gain
can be attributed to that change; timer differences are reported by scope, not
summed into a constructed total. No dedicated reference-pass timer was added.
