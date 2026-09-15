# Isolated OLS continuation and preserved PPML pair audit

The original PPML job 7580026 ends at its 3-hour allocation: wallclock 10,801 s,
exit 137, scheduler failed 100. First native and both reference commands finish;
second native is interrupted without a fit result. The original analysis 7580041
fails on missing final manifests and does not run the scientific comparisons.
These failed records remain intact. One complete timing pair is available;
this continuation does not claim two completed PPML pairs or a successful
original campaign. All native code and artifact identities stay unchanged.

The OLS bootstrap failure precedes every estimator call. Pinned ftools'
`ms_compile_mata` searches for a writable directory in S_ADO, then conditionally
calls an unguarded mkdir for its l subdirectory. The shared target is consistent
with the observed concurrent-startup r693. The isolated runner gives every
Stata process private pinned source copies and a private precreated compiled/l
directory. Explicit BASE/pinned/native/cache paths remove ambient package
lookups. Private ftools compilation and its resolution are checked before the
estimation timer, in both arms. Shared compiled-library bytes are unchanged.
No reference source or estimator numerical policy is modified.

The new runner records an atomic progress manifest after each command and marks
it complete only when every expected command succeeds. Failures and interruptions
remain visible. It never overwrites the original failed OLS attempt.

Focused retest 7580500 passes complete scheduler accounting, two concurrent
single-threaded private bootstrap checks, the two four-thread OLS commands on
the existing 9,000-row synthetic fixture, and independent normal-equation and
model-space checks. All numerical results equal the earlier fixture's captured
values. The retest costs 4 s with 1.190G maxvmem. This is the one focused retest
of the bootstrap fix, not a new benchmark campaign.

Job 7580504 now runs the requested empirical OLS comparison and its independent
checks with 16 reserved slots/48 GiB, four application threads, two alternating
pairs and the original 3-hour bound. Private package setup is outside each
complete-command timer; process timing includes setup and outputs.

Job 7580499 audits only the complete PPML pair. Its selector first reruns the
unchanged audit on the existing small synthetic exports, then checks the
preserved empirical exports without re-estimation. It selects explicit completed
per-command receipts and does not fabricate or overwrite the failed campaign's
runs.json. Numerical helper ASTs and scientific tolerances are unchanged;
only family/work/report selection and manifest handling differ. The second
reference remains supplementary; the truncated second native is not a result.
The PPML audit 7580499 ends failed=0/exit=1 after 607 s (1.841G maxvmem).
The complete pair passes exact all-row/sample ordering, normalized rank/DoF,
coefficient and covariance comparisons: maximum absolute differences are
2.7700e-14 and 2.7228e-15. Native prediction replay and all score checks pass
(maximum normalized score 2.1903e-14). However, the independent unpreconditioned
LSMR FE-span projection reaches its 5,000-iteration bound (stop 7) with fresh
relative residual 3.3016e-6 and normal residual 1.3284e-8, above the unchanged
1e-10/1e-12 gates. This is an indeterminate model-space result, not evidence
that the public mean is infeasible. The full native audit and comparison remain
unaccepted. The reference public-mean audit and objective adjudication are not
reached. No fit or scientific audit is retried and no tolerance is relaxed.
The failure must be localized before a further expensive model-space audit.
The OLS empirical job is still running.

Preserved timings are 1,369.344 s native and 4,904.909 s reference (ratio 3.582)
for the single complete pair, not a scientifically accepted speed claim.
The pair helper's copied local timing label is inapplicable: these are SCC
four-thread commands. Use the first-pair application receipt's GNU-time RSS
and the scheduler records here for resource evidence.

The full local pinned ppmlhdfe reference is still timed on the Mac. No local
build, data generation, large output scan or numerical audit accompanies these
SCC operations. The combined 15-minute monitor must continue both workflows.
