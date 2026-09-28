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

The bounded zero-outcome oracle includes full-model recession-cone separation
(ADR 0223), in addition to zero-only groups and singleton cascades. For an
independent audit of already captured small zero cases, `adjudicate_zeros.py
RUN_ROOT OUTPUT` regenerates and verifies the exact original bytes before
comparing to the captured native outputs. Use the same compute runtime as the
original run; cross-platform regeneration is rejected if any value hash differs.
This audit never rewrites original receipts or reruns an estimator, and does not
by itself authorize a campaign with another unresolved smoke failure.

## Current-source fixest development sweep

`fixest_sweep.py` registers a successor from the completed light campaign's
`runs.csv`. It preserves all 140 configurations, four warm seeds, eight cold
anchors and scientific settings, selecting native/default fixest/strict PPML
fixest arms. Exact previously timed-out reference seeds are explicit budget
exclusions; successful neighboring seeds remain live. The two strict slope
pilot timeouts remain live because later stock adjudication completed them.
Historical failures never become current measurements. Unchanged other commands
retain their separate comparison and adjudication evidence.

The manifest's explicit `pilot_cell_ids` selects eight registered large-N and
two-FE slope groups. Smoke still checks all 28 small independent-oracle groups,
including three-FE slopes and zeros. Both ordinary stage gates remain required;
scientific discrepancies in the full sweep remain unranked. The original light
manifest, enumeration and default pilot are unchanged.

When an accepted Linux artifact already exists for the exact package inputs,
`reuse_setup_job.sh` verifies the deployment, archived inputs, origin source,
plugin and successful Linux validation receipt before copying that artifact.
Set `ORIGIN_SOURCE` and `REFERENCE_RUN` explicitly. Supply `input/source.json`,
`input/linux-validation.json`, `input/deployment.json`, and the ordinary candidate
archive/identity/manifest/history inputs. Stage the two historical CSV fixtures
at their repository-relative `reports/` paths beside `controller/` for the 31
controller tests. Pinned reference directories are linked, then their actual
runtime bytes are frozen and rechecked before execution. No compilation occurs
in timing jobs. Follow the same smoke, pilot, packing, accounting and collection
sequence above; use a new run directory, never an old results tree.
