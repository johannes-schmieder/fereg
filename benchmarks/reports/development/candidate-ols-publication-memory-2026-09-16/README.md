# OLS publication memory admission

The unadopted OLS prototype reserves its publication buffers before loading
observations. The
reservation uses the full reported coefficient axis, including omitted slots,
for both square matrices. It also covers coordinate/determinant scratch,
result tables, requested derived outputs and nested saved-channel registries.
The fitted-term vector and empty saved-output registry use fallible exact
reservations. Actual publication capacities are checked before any store.

Publication and absorption occur sequentially, so the new publication reserve
does not reduce absorption's separate solver allowance. Saved-effect recovery
adds only the capacities actually live in its caller; returned channels and
recovery scratch remain in the existing recovery model. The later xbd and
shadow allocations are not counted during recovery. Existing public base
memory fields and numerical/transaction contracts remain unchanged.

The isolated native snapshot is `5240e58-ols-publication-admission1`, based on
`5240e589` (full SHA and all 162 file hashes in `native/identity.json`). SCC job
7590002 passes **189 distinct native checks**: 82 focused core, 102 plugin,
3 testkit and 2 constrained-memory checks. Exact/one-byte-short fast/low
admission rejects before row reads; excess capacity rejects before any stores.
The existing interruption, repeated-call, allocation-failure, saved-output,
weighted/VCE and scientific controls also pass. The job has failed=0,
exit_status=0 and the explicit `OLS_PUBLICATION_ADMISSION_PASS` sentinel.

All **40 focused Python tests**, both allocation audits and the complete
repository checker pass. OLS inventory v20 and action inventory v3 preserve
historical inventories and F0/reviewed boundary pins. The new action inventory
covers 123 source files and 2,494 allocation syntax sites; this is structural
coverage, not a count of live allocations or a complete byte proof.

The five-case screen completes all **15 alternating pairs** (30 fresh-process
commands) with exact scientific matrices, sample/order, saved columns and
non-time solver fields. The complete-command gate **fails** for saved outputs:
its median rises 154 ms / 24.64%, beyond both declared thresholds.

| Case | Baseline median (s) | Candidate median (s) | Change |
|---|---:|---:|---:|
| small_narrow | 0.038 | 0.038 | +0.00% |
| wide_one_fe | 1.037 | 1.013 | -2.31% |
| weak_wide | 4.598 | 4.713 | +2.50% |
| three_fe_fw_generated | 0.559 | 0.574 | +2.68% |
| saved_generated_aw | 0.625 | 0.779 | +24.64% |

The original failure is retained and blocks adoption pending investigation.
Each saved-output arm reports exactly 8,998,670 native allocation bytes and
5,898,240 scratch bytes. Preflight remains below 1 ms except one slower
baseline run; most variation lies in native execution and outside the plugin.
`paired-results.json` includes complete/phase times, allocation, sampled RSS,
scratch and retry/solver fields. Discarded-attempt time is not separately
observed and remains null. The one-pair diagnostic in job 7590011, using the same artifacts and input,
passes exact science and records 0.362/0.367 s. It does not reproduce the
slowdown, but does not resolve or replace the original three-pair gate. Native
preparation is 10.664/10.696 ms, scratch is identical, and neither arm copies
score-cache columns. Detailed phase receipts, successful scheduler accounting and the diagnostic
terminal sentinel are retained. All task jobs are terminal. Adoption remains
pending; no additional fits or retries are scheduled.

The initial paired build job 7590005 failed before compilation because the
wrapper invoked Cargo outside the source directory. Its scheduler/log receipts
are preserved. The single narrow wrapper fix changes the working directory;
job 7590006 uses separate clean baseline/candidate source and Cargo targets.
The initial missing transfer directory was created before any successful
transfer or test submission; no scientific result was produced by that setup
failure. No failed estimator result was retried.

The implementation addresses an incremental publication/recovery-parent
omission, but is not adopted because the original cost flag is unresolved. It does not
establish complete whole-action coverage. OLS preparation/inference parents,
slope recovery, compiler/topology metadata and opaque dependency/worker-pool
capacities remain open. The whole-action ratio stays undefined. These are
Linux development receipts, not a frozen Mac/Linux review candidate, a Mac
build, or formal qualification. The installed plugin and preserved empirical
outputs are unchanged. No full German fit or reference was rerun.

Portable receipts replace execution-root paths; original receipt hashes are
retained in `raw-receipt-hashes.json`. Raw licensed Stata logs and data stay out
of Git.
