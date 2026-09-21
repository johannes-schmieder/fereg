# SCC execution

The user redirected this benchmark to SCC on September 11, 2026. Local pilot
and stopped-campaign receipts remain immutable; SCC results form a separate
campaign. The final report will use SCC measurements.

Use the existing digest-bound Linux candidate/comparator runtime from the
systematic campaign read-only. Stage this controller in a new mode-0700 run
directory. Stage only the generated synthetic initialization datasets for
the first compute-node smoke. The smoke checks every command at one and 28
threads and checks both candidates against the independent explicit-dummy
oracle. Native comparator errors remain diagnostic outcomes.

Licensed Veneto records must not be transferred until the approved SCC
storage classification/path is established. The derived benchmark inputs
contain individual remuneration and worker-firm links even though original
identifiers have been recoded. Transfer only the model files, never the raw
archive, worker demographics, or employer administrative records. Preserve
owner-only directory and file access; bind the authorization and input hashes
in `input/` before enabling the real-data campaign.

The SCC thread ladder is 1/2/4/8/14/28. Reserve a whole 28-core E5-2680v4 node
for each task so its comparator fits run sequentially with nested CPU affinity
subsets and no oversubscription. One task covers all 11 commands at one thread
count and one repetition: 24 tasks, 264 invocations, one cold plus three warm
repetitions. Use at most four concurrent tasks initially. All core settings
use the same CPU model; the same setting/repetition shares a node across
commands. Different task groups may use different physical nodes.

Use 3 GiB per reserved slot (84 GiB total), justified by the full-node CPU
scaling reservation and existing memory measurements. The synthetic smoke
gets 30 minutes. Production tasks get 10 hours, with 2,400-second public-command
limits and 600-second setup limits. The larger command cap accommodates the
older SCC CPUs and matches the established systematic benchmark cap; no model,
solver or numerical tolerance is changed. Failed commands and timeouts remain
terminal measured outcomes. Technical/controller failures stop the task.

Require complete `qacct` records, `VENETO_SCC_TASK_COMPLETE` and complete
per-call receipts before accepting any task. Submit the production array held on the smoke-acceptance job after authorized
data staging. The acceptance job depends on the smoke job and exits 100 on
failure, leaving production held. Production starts only after successful
scheduler, application, independent-oracle and data-hash checks. Record actual job IDs and
immutable submission identities under `results/scc/`; current progress belongs
only in the repository `STATUS.md`. Collect aggregate outputs, not row-level
files, for the single final PDF.

## Auditing collected results

`audit_existing.py SNAPSHOT OUTPUT` analyzes only complete tasks in a fixed
local snapshot, checking accounting, identities, returned coefficients and
full slope covariances, samples, runtime/RSS and independent graph topology.
It never launches an estimator. `outcomes.py` interprets both Boolean false
and numeric zero native convergence flags; this corrects the original
collector's treatment of Stata e(converged)=0 without altering receipts.
`report_scc.py` applies this interpretation before plotting or counting fits.
A successful scheduler task or public return code alone is not convergence.

## Reporting a stopped campaign

Keep complete tasks and finished calls from interrupted tasks separate.
`analyze_stopped.py` validates both sets, identifies user-cancelled commands,
checks the PPML outcome distribution and worker mobility, and binds failure
interpretation to the frozen source. It does not launch estimators. It reads
the snapshot recorded in `results/scc/campaign-stopped.json` and the
complete-task output from `audit_existing.py`.

`report_stopped.py --font-dir FONT_DIRECTORY` builds the single PDF from
`results/scc/stopped-analysis/`. The font directory must contain the four
LiberationSans Regular/Bold/Italic/BoldItalic TTF files. Main figures use one
cold observation per cell with the available individual warm observations;
they never imply a completed repetition grid. Completed calls in interrupted
tasks enter the supplementary table only. Runtime and memory before failed
or timed-out completion are visibly distinguished from successful estimates.
Render and inspect every page after building. The three focused tests in
`test_outcomes.py` exercise interpretation of native convergence flags.

## Auditing the PPML diagnostic

`analyze_ppml_diagnostic.py` reads the preserved original and instrumented
diagnostics under `work/ppml-diagnostic/`. It verifies application outcomes,
each initial-regression certificate, source identity against the frozen Git
revision, and differences between the original and instrumented traces. It
does not launch estimators or change benchmark observations. Its aggregate
receipt and the diagnostic addendum live under `results/scc/ppml-diagnostic/`.
Keep the original controllers and their failure receipts intact; submission
receipts identify the corrected controller used for the completed experiment.
