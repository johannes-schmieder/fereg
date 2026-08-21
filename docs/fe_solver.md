# Fixed-effect kernel

Status: implemented as an isolated numerical kernel, but Phase 3 is not yet
certified end to end. Estimation actions remain disabled while the broader
data, singleton, solver, and parity gates are incomplete.

The safe core now includes the complete projection-level contract needed by
both OLS and PPML:

- queue-based singleton deletion to a fixed point;
- multipartite connected-component labeling;
- one-way and multiway weighted projections;
- intercept, intercept-plus-slope, and slope-only local blocks;
- symmetric MAP over batched right-hand sides;
- final basis-invariant weighted FE-span projection certificates, with raw
  `D' W r` retained only as a nonfatal diagnostic. Unrepresentable nonzero raw
  moments saturate to infinity and never affect certificate status.

Each effect stores a compressed level-to-observation index. MAP allocates one
projection workspace before iteration and reuses it for every group and sweep.
Rank-deficient local slope blocks are solved with pivoted elimination and set
unidentified local directions to zero; fitted projections remain identified.

A MAP or LSMR stop is not called certified unless a freshly recomputed weighted
orthogonality certificate passes. For each effect-level block `B`, the gate
computes the projection of `sqrt(W) r` onto `col(sqrt(W) B)` with a
column-pivoted, twice-reorthogonalized QR. The reported violation is

```text
|| P_col(sqrt(W) B) sqrt(W) r ||_2
------------------------------------------------- .
max(||sqrt(W) y||_2, ||sqrt(W) r||_2)
```

Columns are normalized before QR so the gate is invariant to FE channel units.
Numerical rank uses a machine-precision, dimension-scaled threshold independent
of the requested convergence tolerance. This avoids false certificates when
an intercept and absorbed slope are nearly collinear. Hitting the iteration
budget or failing this strengthened certificate returns a noncertified report
for the automatic router to escalate rather than silently accepting the fit.

This repair hardens only the final certificate. It does not certify the Phase 3
solver checkpoint or change projection, MAP, LSMR, singleton, OLS, PPML, or
separation algorithms.
