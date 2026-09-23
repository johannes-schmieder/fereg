# ADR 0109: Reuse resident targets for low-memory QR pivot norms

Status: accepted for local development, 2026-09-22.

## Decision

In scratch-backed finite Householder QR, calculate each next trailing norm from
the target immediately after successfully writing its transformed column.
Store it in the existing pivot-norm vector. The initial independent scan supplies
the first pivot; subsequent pivots consume these freshly calculated norms.
No other column update can change that target's stored values before selection.

Use the same stable segment norm, row order and arithmetic as the former
read-back calculation. This is an exact recomputation, never a norm downdate.
Preserve deterministic pivot ties, reference scales, rank thresholds, omission
ordering, reflector arithmetic and the final independent scratch rank scan.
Fresh residual, score, bread and covariance certificates remain unchanged.

The norm vector and target already belong to the current QR call. No allocation,
scratch file, memory admission change or cross-call/weight-generation cache is
introduced. Scratch I/O errors and nonfinite norms remain terminal before
public results can be published. Fast finite regression and retained iteration
kernels keep their existing paths.

## Evidence and limits

The [low-memory report](../../benchmarks/reports/development/low-memory-qr-local-2026-09-22/README.md)
binds the exact isolated artifact, independent oracles and alternating stock
comparisons. Eleven low-memory rows improve by paired medians of 1.8–4.5%,
including fresh serial, wide, factor and saved-output cases. These gains are
below the preferred 10% margin but reliable, broadly applicable and inexpensive
in complexity. All 65 native pairs have identical scientific exports.

The two fast controls are flat and 2 ms slower by median, respectively;
individual losses remain visible. All eleven low-memory rows still lose to
ordinary stock fixest, whose memory strategy is not budget-matched. Fifty-one
distinct Rust tests, three licensed oracle/transaction fixtures, four small
independent designs and the actual-plugin ABI harness pass. No constrained-memory,
cross-platform or release claim follows. The separate factor-rebasing defect
and earlier constrained-fit/certificate issues remain open.
