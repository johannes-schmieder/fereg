# Independent weighted-statistics phase profile

Eight successful local diagnostic calls on 2026-09-20 compare the accepted
`weightedboth` artifact with `weightedstatsprofile`. The latter uses source
`3ea948de` and changes only diagnostic timers in `prepared_unadjusted.rs`.
All four pairs preserve b, V, status, DoF, certificates and fit statistics
bitwise. The recorded instrumentation replays exactly; 515 other source files
are unchanged. Its actual-plugin ABI/panic test passes. There is no production
algorithm change or new licensed-fixture claim in this checkpoint.

The environment is the M2 Ultra Mac, Stata/MP 19.0 (8 licensed processors),
16 native workers; embedded build date September 19. Profile plugin SHA-256:
`59c6a80b4c8eb460ffc93ed64cf539852dae335107cb86bb322f0c347f75e564`.
The timers, isolated builder, run script, raw-result hashes and all warm-up
and measured phase lines are retained. This instrumentation is not a
headline performance result or a fixest comparison.

## Measured phases

Milliseconds below are one instrumented call per cell, K=6. Except the
N=245760 degree-six case, N=983040. The weight-report total includes domain,
raw sum, reference sum and remaining kind/scale checks, so its components
must not be added to that total. Outcome mean and squares are sequential;
IID residual squares already run alongside the outcome computation.

| Phase | Shuffled AW IID | Degree-six AW IID | Ordered FW IID | Ordered PW robust |
|---|---:|---:|---:|---:|
| Weight domain and maximum scale | 2.733 | 0.692 | 2.775 | 2.764 |
| Raw weight sum | 3.610 | 0.910 | 3.609 | 3.664 |
| Reference weight sum | 3.917 | 1.001 | 3.907 | 4.010 |
| Complete IID weight report | 10.309 | 2.668 | 20.218 | — |
| Outcome mean | 6.281 | 1.643 | 6.528 | 6.676 |
| Outcome sum of squares | 3.967 | 1.025 | 4.080 | 4.217 |
| IID residual sum of squares | 4.404 | 1.140 | 4.229 | — |

Fit/DoF consistency validation is about 0.001 ms in the IID calls. The
shuffled AW IID statistics total is 20.679 ms: the weight report and outcome
work each account for roughly half. Frequency metadata has substantial
additional work beyond the three displayed weight subphases. Missing PW
aggregate/RSS timers do not imply zero work; this probe measures those
aggregate boundaries only on IID.

## Next experiment

Moving only the independent outcome scan cannot remove the independent
weight-report cost. Test overlapping both source-based weight checks and
outcome statistics with fitting, preserving their exact calculations and
original deferred error order. Cache only scalar scan results from immutable
prepared rows; validate the fitted weight scale before consuming a speculative
result, with the existing scan as fallback on any mismatch. The TSS multiplier
must match the fit's direct compensated normalized sum: deriving it by
multiplying and dividing a raw sum can change its bits. Frequency effective N,
AW/PW scaling and all independent inference checks remain authoritative.

The observed IID tail could ideally fall toward the residual-only cost, but
actual wall-clock improvement depends on contention and checks that remain
after the fit. That is a hypothesis to measure, not a promised speedup.
Retain unit/one-FE controls and explicit frequency-expansion evidence. After
that, duplicate Schur probe/initialization and left-mean restoration remain
concrete targets for the narrow weighted and unweighted losses.
