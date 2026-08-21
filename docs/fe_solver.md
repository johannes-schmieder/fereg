# Fixed-effect kernel

The safe core now includes the complete projection-level contract needed by
both OLS and PPML:

- queue-based singleton deletion to a fixed point;
- multipartite connected-component labeling;
- one-way and multiway weighted projections;
- intercept, intercept-plus-slope, and slope-only local blocks;
- symmetric MAP over batched right-hand sides;
- final `D' W r` infinity- and two-norm certificates.

Each effect stores a compressed level-to-observation index. MAP allocates one
projection workspace before iteration and reuses it for every group and sweep.
Rank-deficient local slope blocks are solved with pivoted elimination and set
unidentified local directions to zero; fitted projections remain identified.

A MAP stop is not called converged unless a freshly recomputed weighted
orthogonality certificate passes. Hitting the iteration budget returns a
noncertified report for the automatic router to escalate rather than silently
accepting the fit.
