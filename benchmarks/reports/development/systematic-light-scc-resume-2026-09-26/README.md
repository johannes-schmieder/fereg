# Lighter SCC campaign: corrected-source continuation

Candidate and controller are committed as
`58284f76a65b226e6df09a48cafa5caa68461af2`. Run label:
`20260926-systematic-light-58284f76`. The separate old c31bcb62 smoke remains
rejected and immutable. ADR0224 repairs automatic RHS report aggregation;
ADR0223 corrects the independent zero-count separation oracle. The Stata
initialization log now retains native errors outside the timed command.

The successor run manifest changes only candidate_commit relative to the
approved light-v2 grid: 140 configurations / 568 groups, both OLS and PPML,
four warm repetitions and eight cold anchors. Reference pins, strict-stock
settings, samples, tolerances, workload, and skips remain unchanged. Whole-group
packing targets 10.5h in 12h tasks at concurrency eight; pilot costs will refine
the forecast. This is development evidence, not release qualification.

Setup 7745641 passed: four slots, 205 seconds, 3.010G maxvmem, failed=0 and
exit_status=0. Linux native build, binary audit and actual-plugin ABI/self-test
passed. Pinned reference copies are hash-verified against the original frozen
runtime. Controller tests: 21 passed locally. Local correction science is in
the separate ols-auto-report-merge report.

Setup/launch, frozen identities and later stage receipts are separate immutable
files here. STATUS.md contains the current campaign state. Pilot and production
must only use the newly frozen controller after their respective stage gates;
do not promote the older rejected evidence or pair fits across source identities.
