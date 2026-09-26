# Lighter SCC development campaign

Use `manifest-light-v2.json` with ADR 0222. `manifest.json` and the stopped
campaign retain their original meaning. The candidate is pinned independently
of the harness commit. See the new manifest for exact exclusions and options.

Prepare a new run root containing `input/candidate.tar.gz`, its `candidate.json`
(commit and SHA-256), `input/manifest.json` copied from the light manifest, and
`input/historical-runs.csv` copied from the immutable stopped report. Deploy
this directory as `controller/`. The setup job copies and verifies the previous
pinned comparator environment; it builds only the new candidate. Set RUN_ROOT,
REFERENCE_RUN, FEREG_SCC_RUNS_ROOT and FEREG_SCC_RUST_ROOT explicitly. Never reuse
a stopped campaign's results directory or submit its existing plan.

After successful setup application and four-slot accounting, load the same
Python module and use the new run's `env/python/bin/python` as PYTHON:

```bash
"$PYTHON" "$RUN_ROOT/controller/provenance.py" freeze "$RUN_ROOT"
"$PYTHON" "$RUN_ROOT/controller/campaign.py" smoke "$RUN_ROOT" "$RUN_ROOT/plans/smoke.json"
```

Submit `job.sh` with RUN_ROOT, PLAN and FEREG_SCC_RUNS_ROOT. Smoke uses one hour;
pilot and production allow twelve. Reserve 28 cores, E5-2680v4 and 3G per core.
After each stage, capture every task with `accounting.py RUN_ROOT JOB_ID COUNT`,
and run `light_control.py RUN_ROOT PLAN` to validate smoke/pilot. Never substitute
scheduler exit zero for the independent oracle and application checks.

Generate pilot then production plans with the corresponding `campaign.py` mode.
Pilot comparisons are registered warm-1 groups and are reused by production only
under the same frozen identity. Production requires both accepted stage receipts.
Submit the exact plan as an array, one bundle per task, with `-tc 8`. Each
comparison stays on one node; tasks execute groups sequentially. Forecasts
include setup/import/initialization estimates and measured pilot whole-group
elapsed time. Deadline checks use conservative per-arm caps; recorded pending
groups can be packed into a new immutable plan after all original tasks stop.
Do not mutate a running plan or repeat completed groups in another active task.

Use `campaign.py audit RUN_ROOT OUTPUT` and `report.py RUN_ROOT OUTPUT` after
accounting collection. The light report writes runs, paired ratios, four-repeat
quartiles/min/max, per-pair losses and a Markdown summary. `--allow-incomplete`
retains missing groups instead of fabricating results. Default fixest and stricter
stock PPML are separate. Whole-arm elapsed and timed command costs are separate;
absolute sampled RSS includes the language runtime. Cold anchors stay out of warm
summaries. Raw Stata logs stay on SCC or in ignored local evidence directories.

For local controller checks, run `python -m unittest discover -s
benchmarks/systematic -p 'test*.py'` in the established scientific Python
runtime. Tests need NumPy, SciPy, pandas, psutil and matplotlib, but invoke no
estimators. The Linux smoke exercises the actual licensed workers.
