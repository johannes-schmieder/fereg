# P3 cluster histogram traversal experiment

Baseline: 2243b4db979469db29226e73fdd5744b5b9be112. Local development only.

Change only ordered dense OLS histogram traversal from indirect ordered row indices to original-axis cluster codes. Each current order is a full permutation, so histograms, stable scatter, partitions, arithmetic, reservations and fallback must be identical. PPML paired reducer and OLS scratch stay unchanged.

Audit existing final-source borrowing and per-subset shared production/certificate ordering before considering persistent caches.

Prospective target: at least 5% or 10 ms median paired complete-command reduction on wide two-/three-way cluster targets; protect other commands at max(10 ms,5%). Small focused rotated seed comparison; preserve all attempts. Microbenchmarks can reject a weak candidate but do not establish a command-time gain.
