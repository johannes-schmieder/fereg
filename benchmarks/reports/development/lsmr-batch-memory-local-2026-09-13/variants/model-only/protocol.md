# Diagnostic model-only decomposition

The wide-only batch candidate still has PPML K17 command regressions (+21,+36,+38ms), with exact science and non-time decisions. Build a narrow-only diagnostic variant restoring ac47b19 fe/borrowed_map.rs, fe/lsmr.rs and fe/mod.rs while keeping the revised shared memory model. No batch adapter is compiled into this variant. Its accounting is valid only for full-RHS shapes (RHS<=64 here); do not admit or benchmark wider shapes with this diagnostic.

Compare six PPML weak PW/offset/cluster N30720/K17/four-thread pairs: seeds15485863,67867967,49979687 twice in opposite order. Same exact scientific and solver gates; only model equality omitted. Same max(10ms,5%) review envelope; retain all runs and measure phases/native/RSS. Purpose is attribution, not adoption. No other work alongside timings.
