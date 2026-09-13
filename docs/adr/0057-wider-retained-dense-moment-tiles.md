# ADR 0057: Bounded tiles for wider retained dense moments

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

Extend ADR 0054's scaled retained-dense moment reuse above 64 active columns.
Within each existing disjoint Gram-row partition, retain at most 64 weighted
left values and 64 standardized right values at a time. Reuse the right tile
across those left rows, traversing each observation in its existing order.
Large partitions iterate through additional fixed tiles; storage does not grow
with the model width. No new rank/width threshold or automatic solver region
is introduced.

Every Gram/RHS entry preserves its exact observation and fused-multiply-add
order, selected-column mapping, weight normalization and reference scale.
The existing up-to-64 cache remains; unscaled, single-column partitions,
scratch-backed and one-shot OLS moment sources retain their existing loops.
QR, dropped-span/score/inverse certificates and strict current-weight basis
transactions remain authoritative.

Each partition uses 1024 bytes of stack payload, with no heap reservation,
observation-sized copy, partial Gram bank, new worker team or SPI. This is a
source-derived stack bound, not a native-allocation or RSS reduction. The
existing layout still decides partition count; no batching crossover is copied
from an unrelated kernel.

The [local source-bound report](../../benchmarks/reports/development/ppml-wide-moments-local-2026-09-13/README.md)
compares the extension with accepted PSD/transport repairs already in both arms.
The earlier two-pair probe is not pooled into that evidence. Independent exact
scalar moments cover 128/129/257 columns and 1/4/16 workers; complete commands,
protected controls, independent Poisson inference and retained allocation audit
supply the adoption evidence. Broader scratch/layout/predictor/state work remains
separate under #27.
