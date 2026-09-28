# ADR 0239: reject broad checked-reduction inlining after one-FE profiling

Status: rejected, 2026-09-28. Accepted source remains computational snapshot
5c0d4685 (main 524c8e8b); no production scientific or execution policy changes.

## Hypothesis and experiment

Current-source one-FE public PPML profiling attributes 9.03% of serial CPU samples
to two checked scalar accumulator helpers, with a further 4.66% in group means.
The initial portable-FMA hypothesis is not the dominant measured explanation.
A seven-line candidate exposes these helpers for inlining, preserving every
arithmetic expression, ordered recurrence, overflow/finite check and independent
certificate replay. There is no new owner, heap bank, guard or tolerance.

## Decision

Reject the candidate as a package. Initial and predeclared fresh measurements
show useful serial and wide one-FE gains, but the larger fresh T8 case loses 8.7%
and goes from 1.140 to 1.228 times comparable stock. Two-FE PPML loses in both
blocks; low benefit shrinks and slope controls lose in the fresh block. The
broader improvement is not reliable enough to justify changing shared hot paths.
Do not split this result into fitted case-specific guards or continue tuning the
same annotations. This is not evidence that checked numerical loops cannot be
improved with a materially different organization.

All 74 focused Rust tests and six licensed oracle/control fixtures pass on Mac
and Linux. All 144 headline applications, 48 native pairs and 48 stock pairs pass.
Native coefficient/covariance exports are byte-identical and work/modeled memory
are unchanged. The rejection is for practical performance, not accuracy.

## Consequence

The exact accepted computational source and universal Mac artifact are restored.
The patch, candidate artifacts' identities, complete distributions and losses
remain in the [source-bound report](../../benchmarks/reports/development/one-fe-portable-scc-2026-09-28/README.md).
No historical claim is relabeled. Existing performance, memory and scientific
contracts remain binding; the universal stock ceiling and core scaling goal are
unmet.

A separate post-timing profile shows substantial Stata embedding-worker activity:
passive waiting or one host processor, with four native workers verified, sharply
reduces CPU activity and diagnostic elapsed time. It is not headline evidence or
an adopted policy. The next bounded experiment should test scoped host/native
coordination, charging per-call switching/restoration and independently validating
failure/Break/panic restoration, samples, inference and requested thread behavior.
