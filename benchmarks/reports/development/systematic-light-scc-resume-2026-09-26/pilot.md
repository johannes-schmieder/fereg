# Pilot: completed application, rejected comparability gate

Candidate/controller 58284f76a65b226e6df09a48cafa5caa68461af2, frozen runtime
041bf47963205620c13f2137c17a25a47afd70d73c5d03c42a936bbb39ce29e6.
Job 7745818/task 1 completed all 12 registered groups, SYSTEMATIC_BUNDLE_COMPLETE
present, no pending groups. Accounting: failed=0, exit_status=0, 28 slots,
1,795 seconds (29.9min), maxvmem 7.170G. The bundle receipt SHA256 is
246aa54a8198e5751cd82df0a0d95a06940d9203316fa9423a0fbeb56ad9f48e.
No estimator or frozen controller bytes changed after the accepted smoke.

All 12 native commands completed with convergence reported. Of 52 arm attempts,
47 passed, three hit command deadlines and two failed initialization. Of 40
pair comparisons, 28 passed, seven failed accuracy checks and five were unavailable.
These are single warm-1 observations, not four-repetition performance summaries.
Small-input independent oracles passed in the earlier 28-group smoke; full-size
pilot outputs have native certificates and the recorded reference comparisons,
not a new large explicit-design oracle. Discrepant pairs remain unranked.

The unchanged pilot gate rejected three groups:

| Case, all N61,440 | Native outcome | Missing comparable reference |
|---|---|---|
| PPML three FE, two slopes on each of first two partitions, K10/T28/fast | Pass, 15.342s | Default fixest fails coefficients/SE/full covariance/deviance; strict stock hits 120s; ppmlhdfe warm-up fails r(3201). |
| Same PPML design, K6/T4/low | Pass, 15.619s | Same failed checks, strict stock 120s timeout, ppmlhdfe warm-up r(3201). |
| OLS same FE/slopes, K10/T28/fast | Pass, 3.489s | Default fixest reports nonconvergence and fails coefficients/SE/full covariance/RSS; reghdfe hits 120s. |

All completed default-fixest comparisons above have identical retained sample
hashes; sample mismatch does not explain these discrepancies. Default-fixest
PPML deviances exceed native by 0.00741579 and 0.00499794 respectively. OLS RSS
is 5745.00913946515 for fixest versus 5744.85454071067 native. These observations
localize a numerical agreement problem; they do not independently adjudicate
which full-size result is accurate. The ppmlhdfe logs locate initialization at
`reghdfe_panel_precondition(): 3201 vector required`, called from weight loading.
No request reached the timed command for these two arms. No retries were made.

The strict reference option sets and 120s caps were frozen in pilot.json before
submission. Strict fixest uses unmodified pinned stock with glm.tol=1e-10 and
fixef.tol=1e-11. Its initial cap came from the historical-cost planner's 120s
minimum, not evidence that the full-size slope call would finish that quickly.
Changing a budget for a diagnostic continuation must preserve this timeout
receipt and cannot retroactively turn the pilot into an accepted result.

Selected scientifically passing single-pair observations illustrate both useful
speed and remaining losses. Full paired values and failed checks are attached in
pilot-summary.json; no distributional or universal speed claim is made.

| Case | fereg seconds | Reference seconds | Ratio |
|---|---:|---:|---:|
| OLS dense two FE,N983,040/K10/T28 |0.591|1.209 default fixest|0.489|
| OLS strong degree 6,N3,932,160/K10/T28 |1.792|1.931 default fixest|0.928|
| OLS two slope partitions,N61,440/K10/T28/fast |0.485|0.151 default fixest|3.212|
| OLS two slope partitions,N61,440/K6/T4/low |0.873|0.199 default fixest|4.387|
| PPML dense two FE,N983,040/K10/T28 |3.784|5.496 strict fixest|0.689|
| PPML strong degree 6,N3,932,160/K10/T28 |32.803|8.699 strict fixest|3.771|
| PPML two slope partitions,N61,440/K10/T28/fast |0.559|1.253 strict fixest|0.446|
| PPML two slope partitions,N61,440/K6/T4/low |0.593|1.993 strict fixest|0.298|

The dense categorical PPML default fixest also passes at 3.747s (ratio 1.010).
Default fixest is scientifically discrepant in the two-partition PPML slope
cases, so strict-reference values are used there. IID OLS/robust PPML and no
fitted-mean publication match the frozen work definition. The Stata license
reports four control-plane processors even on T28 jobs; fereg's native requested
and admitted threads and fixest's effective threads are 28. Those fields remain
in the underlying receipts; no claim of 28 licensed Stata processors is made.

Production is blocked by the unmodified gate; no production plan or jobs were
submitted. The existing campaign heartbeat is paused. The next useful step is
bounded adjudication of these three captured designs: permit enough time for
the strict stock PPML comparison, establish a converged OLS reference, and check
against an independent sparse full-design oracle. Samples, accuracy requirements
and existing failures must remain visible. Do not silently remove the cases,
waive the gate or restart the full pilot. Any subsequent diagnostic runtime and
plan need separate identities; accepted current pilot groups remain reusable
only under their original frozen identity. The full 568-group campaign is incomplete.
