# Lighter systematic SCC campaign: preflight receipt

This is new development evidence for estimator
`c31bcb621ba196e03a070ff2d808812733970f50`, not a restart of the stopped
September 10–11 campaign and not release qualification. Run label:
`20260926-systematic-light-c31bcb62`. ADR 0222 defines the revised evidence policy.

The frozen successor grid contains 140 configurations and 568 comparison groups
(560 warm, eight cold), preserving both models and all eight graph designs.
The original 304-configuration/2,128-group manifest is unchanged. OLS compares
fereg/reghdfe/areg/greg/fixest/pyfixest. PPML compares
feppml/ppmlhdfe/gglm/fixest/fixest_strict/pyfixest, with explicitly registered
historical-budget and unsupported-slope exclusions. Timed outputs are IID OLS
and robust PPML inference, without fitted-mean publication.

Twenty local harness tests passed; their output is attached. Tests cover the
legacy grid and six-repetition policy, successor grid/four-round balance,
explicit exclusions, mixed-model packing, tail filling, timed-out child cleanup,
missing covariance/accounting/DoF/convergence, cross-node rejection and absent
plot points. Twelve added bounded independent slope/zero designs completed
locally through the explicit-design oracle. These are oracle self-exercises;
licensed candidate agreement must still pass on SCC.

Initial historical-cost packing gives 17 production bundles and approximately
174.06 predicted node-hours before refinement with the current pilot. These are
forecasts, not observed runtimes. Sparse historical PPML coverage does not
support a claim that the whole new campaign will finish twice as fast. The
historical OLS command-cost proxy for the reduced grid is 40.6% of the old grid.

Setup job 7745351 finished with SGE failed=0/exit_status=0, four slots,
333 seconds and 3.010G maxvmem. The Linux build and actual-plugin ABI self-test
passed. Copied comparator implementations were checked against the earlier
frozen runtime. Exact build/runtime and subsequent application receipts live
under the new SCC run; STATUS.md carries the mutable resume point.
