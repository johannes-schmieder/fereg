# Canonical PPML callchain attribution

Job **7763108** completes24 diagnostic public fits, 12 per artifact, after every
headline fit in7763099. All application, convergence and terminal checks pass;
`perf` recording/reporting succeeds with zero lost samples. Accounting is clean:
18s, four shared slots, scc-wm3, maxvmem1.113G. This is CPU attribution, never a
headline timing comparison. Full-process user-cycle samples include input,
output and host activity. `perf -F199 -e cycles:u --call-graph dwarf, 8192` and
self reports with a0.5% display threshold are registered in harness04.

Same original profiled input: N30720/K6/T4, seed 202609282, two categorical FE,
robust PPML. Baseline8e2d5af2 and candidate d1156e6d hashes, input/script/trace/log
hashes and full textual reports are retained in results.json; licensed raw logs
and disassembly are not committed. Solver traces are diagnostic-only.

## Attribution

Baseline callchains resolve `canonical_products::build` →
`joint::normalized_product` → out-of-line libm hardware FMA, including from
`CertifiedLinearSession::prepare_retained_solve`. The earlier shared-helper
attribution ambiguity is resolved for this sampled work. Normalized-product
self share is 4.50%; displayed FMA entry/return self shares total 9.39%.
Candidate normalized-product is absent above the 0.5% display threshold;
canonical build closure is 2.98%, and FMA entry/return total 2.15%.
This supports the candidate's instruction-call hypothesis. It does not turn
sampled CPU shares into wall-time gains; use the isolated rotations for those.

Displayed DSO self shares (rows≥0.5%, not a complete 100% decomposition):

| DSO | Baseline | Candidate |
|---|---:|---:|
| Stata |52.90%|62.89%|
| plugin |14.31%|13.61%|
| libm |9.39%|2.15%|
| libc |0.98%|1.02%|

The larger host fraction can reflect the denominator changing. It does not
prove that host work increased. A same-artifact host-setting experiment already
rejected a blanket change because slope controls lost on every repetition.

## Remaining concrete work

Candidate callchains still contain ordinary
`ReusableCategoricalJointLsmr::solve` / `run_once` / `lsmr_from_bidiag` /
`AugmentedCategoricalOperator::apply_adjoint`. Schwarz reconstruction also appears:
`rebuild_schwarz` → `Solver::new` → `build_preconditioner` →
`accumulate_cross_block`, under memset and accumulation/CSR samples.
Canonical proposals themselves remain material. Inlining and display thresholds
prevent a complete phase decomposition; these are measured callers, not
estimates of total savings available from replacing them.

Investigate ordinary categorical IRLS operator/preconditioner work and whether
the existing certified coefficient-space engine can serve it efficiently.
Inspect admission, ownership and previous experiments before changing routes.
Keep current-weight geometry and all independent scientific checks mandatory.
Small dense PPML still takes roughly twice stock in fresh headline timing;
this profile does not establish that its stronger checks require that gap.
