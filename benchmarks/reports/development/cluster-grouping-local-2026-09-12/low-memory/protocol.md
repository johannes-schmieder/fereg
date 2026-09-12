# P3 low-memory cluster sequential score prototype

Baseline2243b4d; separate from histogram trial.

Retain stable original row order within each group and lexicographic group order. Reuse the existing dense group-score accumulator only when its compensated score/presence buffers fit within the displaced N*sizeof(usize) tuple ordering budget. Never use the dense parallel certificate route for scratch: retain existing per-group bread and compensated independent covariance. Overlarge Cartesian products retain tuple ordering. No new row-sized cache, worker group table or preflight expansion.

Prospective target: >=10% median paired full-command gain on repeated low-memory few-cluster targets. Protect high-cardinality fallback, weak graphs, weights/VCE, small inputs and fast mode at max(10ms,5%). Exact science expected.
