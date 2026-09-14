# P4 remaining predictor pass: prospective local evaluation

Baseline is accepted e65d6f9, isolated plugin 365bee73. Retained ordinary frozen
PPML already forms its combined predictor in O(N). Fresh and strict retained
solves still form finite predictors by visiting each coefficient and each row
through fit_column_value. They independently reconstruct and certify the FE part.
Use existing NumericOlsColumns bindings and 128-row tiles for eligible numeric
models (same N>=4096, generated width 8..64 guard). Feed selected (generated
identity, coefficient) pairs in their original coefficient-slot iteration order;
never reorder FMAs or infer kept/omitted columns from width. Reuse the already
owned predictor vector and a fixed stack-only selected-column list. Keep generic,
narrow/wide/generated fallback and reconstruction arithmetic/certificates.
No numerical generation may become reusable solely from topology equality.

Validate bitwise scalar-versus-tiled accumulation under 1/4/16 installed workers,
partial tiles, reordered and repeated selected identities, omitted directions,
zero/nonfinite cases, and existing exact-rational reconstruction/strict rollback
checks. Run the established independent PPML dummy Poisson/sandwich oracle.
First screen the original one-FE complete, FW exposure/missing and PW
cluster/offset requests from the retained-moments experiment, preserving exact
full/warm data. Record requests and input hashes before calls. Use command,
phase, native/RSS and all rank/status/DoF/solver/convergence/certificate fields.
Gain gate: both 5% and 10 ms; material native memory benefit also counts.
If a target qualifies, repeat in rotated order and add the existing small,
no-FE, low, generated/slope, weak and 65-column boundary controls. Retain all
results, especially costs above both 5% and 10 ms. No cherry-picked inputs or
unmatched-start comparison. Diagnostic profiling is separate from timing and
binds its own artifact if instrumentation requires one.
No builds/tests/oracle math/data generation/archive compression overlaps timing.
Installed dist and unrelated files stay; no agents or cluster before #27 is done.
