# Performance contract

Phase 0–1 does not claim speed superiority over `reghdfe` or `ppmlhdfe`.
Performance claims begin only after the scientific parity gates pass.

Required benchmark reporting:

- wall-clock median and dispersion across warm runs;
- peak resident memory;
- effective sample and singleton/separation deletion counts;
- FE/IRLS iteration counts and final certificates;
- thread count, CPU model, OS, Stata edition/version, and plugin hash;
- comparison against the pinned reference versions in `references.lock.toml`.

The manifest ladder must include tiny, medium, multi-million-row, low-mobility,
high-degree, string-FE, absorbed-slope, clustered, and PPML-separated designs.
A target of 1.2–1.5x median runtime improvement is meaningful only where sample,
coefficients, VCE, and stopping certificates match the compatibility contract.
