# Same-input candidate attempt traces

After all untraced comparisons, run candidate auto on the original three target
inputs and six wide graph controls (weak, tiny weighted cut, bottleneck, cycle,
disconnected and unbalanced). Explicitly inject FEREG_SOLVER_TRACE after the
harness environment sanitizer, preserving the thread environment. Also enable
the existing phase trace. Compare all scientific exports and discrete solver
fields exactly to each same-artifact untraced call. Exclude traced times/peaks
from speed aggregates. Separate warm and full calls by their recorded row counts.
