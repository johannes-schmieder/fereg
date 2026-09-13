The weak_d3 manifest is a two-FE graph with degree three, not three FE. The
shared helper correctly selected g1/g2 for it; the PSD protocol/report label
was wrong. The initial explicit-g3 attempt therefore failed r(111) during
warm-up because g3 was absent. Preserve that setup failure and the valid
original two-FE pair. Use the existing three_fe_sparse manifest (mobility 0.02)
with explicit g1/g2/g3 at N7680/K10/t4, seed31415927, candidate first. This
adds the intended low-mobility three-FE coverage without modifying old data or
substituting for the original weak two-FE control. Both artifacts and all
scientific checks remain unchanged. No build/test during timing.
