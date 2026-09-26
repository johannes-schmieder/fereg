# ADR 0216: parallel retained-scratch residual reconstruction

Status: accepted for local development, 2026-09-26.

## Decision

After the finite solve has consumed its moment partials, reuse that admitted
buffer as disjoint full-width row buffers for retained scratch residuals. Bound
row partitions by the requested installed Rayon team, sample size and available
complete buffers. Each partition owns its own immutable scratch cursor. Fewer
than two buffers, one worker, zero rows or zero stored columns retain the serial
path. There is no sample-size or benchmark-name crossover.

Read and validate every stored column, including columns omitted by the finite
rank decision. Preserve each row's normalized weight, square root, ordered
coefficient FMA sequence and division. Only independent rows run concurrently.
No reduction is reordered. Scratch range, poison, nonfinite and positional I/O
checks still fail the private solve. No new numerical state survives the call;
future moment construction overwrites the reused bank before consuming it.

Current-weight absorption, finite/rank, original-input score and model-space,
objective, reconstruction and exact final-Hessian acceptance remain unchanged.
Dense and implicit retained reconstruction, frozen finite reconstruction and
one-shot OLS are unchanged. This extends ADR 0129's read-cursor ownership under
ADR 0070's existing moment-workspace admission; it is not a new solver/cache or
an authority to relax certificates.

## Memory and boundaries

There is no additional explicit heap bank, observation bank, file, worker pool or
admission allowance. Each concurrent read partition holds an 8 KiB cursor page
on an existing worker stack, versus one active page in the serial scan. This
uses the existing runtime/stack headroom and requires physical-cap confirmation
before qualification. It does not establish complete stack accounting.

All scratch access remains safe Rust over native-owned storage. No worker calls
Stata SPI. Partial private residual writes on an error have no publication
authority; the estimation transaction remains failure-atomic.

## Evidence and limits

The independent closed-form weighted Walsh test covers narrow/wide rank-deficient
designs, T1/T3/T8, a short last partition and changing weights. Focused controls
include corrupt/truncated scratch, independent worker pages, zero/single rank,
wide QR, requested teams, strict rollback and canonical-frame weighted contrasts.
The [source-bound report](../../benchmarks/reports/development/ppml-parallel-residuals-local-2026-09-26/README.md)
records 76 distinct Rust checks, nine licensed fixtures, 204 audited public PPML
states, 12 explicit Poisson/Hessian/sandwich checks, 68 exact native PPML pairs
and 18 exact OLS coefficient/covariance pairs. One existing small stock saved-output
discrepancy remains unranked; all 40 fresh stock comparisons pass.

Paired instrumentation reduces million-row low residual reconstruction from
45.7 to 15.4 ms. Complete public low-memory medians improve 993 to 962 ms in the
pilot and 754 to 730 ms on the predeclared fresh design. Whole-fit tracked heap
and scratch peaks are unchanged. Serial and small-case losses remain visible.
The <=0.90 stock goal remains active; no platform or qualification claim follows
from this local change. Stop tuning this loop. The next bounded hypothesis is
one-pass weighted incidence construction within the canonical frame, using the
already admitted factor/index storage; no changing-weight authority may cross calls.
