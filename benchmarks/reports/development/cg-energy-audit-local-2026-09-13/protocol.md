# P6 prerequisite: independent audit of negative CG energy

Accepted numerical baseline 257f4ce, evidence HEAD d4d7e97; isolated baseline
plugin c64ead07. Candidate changes only borrowed symmetric CG's previously
fatal, beyond-one-epsilon negative Hestenes subtraction. A discrepancy beyond
64 epsilon times max(previous,decrease,1) still fails. For smaller discrepancies,
recompute before and pending-after weighted energies with scaled compensated
sums; both previous-energy and step-decrease consistency must pass that bound.
Use the measured after energy, retaining the existing epsilon floor, Hestenes
quotient/stop, fresh FE certificate and strict saved-output reconstruction.
No heap allocation, resident workspace, changed weights/scales or SPI calls.
This is a correctness repair, not a new tolerance or a speedup experiment.

Preserve the baseline 45-row weighted constant reproducer failure and every
failed candidate test, including the new fixture's overstrict variable-column
assertion and overflow expectation. Run all borrowed-map and slope-recovery
unit tests; verify the closed-form weighted projection, near-constant nonzero
energy, stale/inconsistent steps, nonfinite/overflow rejection, exhaustion,
forced fallback, weighted PPML cancellation and constrained allocations.

Use exact final candidate inputs and an isolated universal plugin. Run both
artifacts through the 48-fit saved OLS explicit-dummy oracle (expect_schur=0
on both), plus the existing one-FE independent Poisson/Hessian/sandwich oracle.
Then replay the original PW partial-saved failing canonical worker, same data
and warm-up, unchanged command/seed/request. Require rc=0, scientific and saved
certificates; baseline failure remains a failure, never a speed denominator.
A separate same-input trace identifies whether the new audit actually triggers.

After correctness, run eleven rotated, single-seed OLS control pairs: weak,
dense, bottleneck, cycle, disconnected, unbalanced (N30720,K16,T4, robust);
small/narrow AW robust (N1920,K2,T1); low FW cluster (N30720,K16,T4);
unit strong saved both (original N245760,K50,T16); weighted tiny cut
(N30720,K16,T4, AW); complete one-FE IID (N983040,K50,T16).
Also run one shared PPML weak two-FE PW clustered control (N7680,K16,T4).
Seed31415927 for new controls; original strong saved seed104729 reuses its data.
No numerical/rank/DoF/sample/certificate/solver-decision changes on successful
ordinary controls; check full scientific exports, saved outputs and unchanged
modeled allocation. Record peak, scratch, RSS, timing and fallback fields.
Review slowdown exceeding max(10 ms,5%) with three additional rotated seeds,
retaining original observations. Do not average away a scientific failure.

Use fresh local Stata/MP processes with untimed warm-up. No builds, tests or
sampling profiles overlap primary timings. Retain all failure receipts and
original data identities. No P6 Schur prototype in this comparison, no agents,
SCC, Windows, qualification, dist replacement or push.
