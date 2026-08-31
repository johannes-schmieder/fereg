# SCC benchmark workflows

## F0 regression qualification

fereg_f0_scc.py is the Linux/SCC execution layer for the internal
candidate-versus-F0 regression contract. It never decides the final scientific
or performance gate itself. After collecting the closed shard inventory it
calls:

    python3 scripts/fereg_f0_regression.py evaluate \
      --manifest benchmarks/manifests/fereg_f0_regression_v1.json \
      --evidence evidence.json \
      --output qualification.json

The workflow uses /projectnb/welfgr/fereg/runs/<RUN_ID> and keeps immutable
inputs, logs, work, outputs, and receipts separate:

    runs/<RUN_ID>/
    ├── input/       # exact git archives, manifest, tasks, run-scoped harness
    ├── logs/        # SGE, build, Stata, and evaluator logs
    ├── work/        # extracted sources, built bundles, shared data, task work
    ├── output/      # one cell JSON per task plus merged evidence/qualification
    └── receipts/    # setup, task, collector, qacct, and final sealed receipts

The stages are:

1. fereg_f0_scc.py prepare creates exact git archive inputs for the F0 and
   candidate commits, records their tree/lock hashes, freezes the 14-row task
   manifest, and copies the committed harness bytes into the run.
2. fereg_f0_deploy.sh transfers that already-prepared run without --delete
   and verifies every input hash on SCC. It refuses to overlay an existing run
   ID.
3. fereg_f0_submit.sh submits setup, a scheduled task-1 smoke gate, the held
   tasks 2--14 paired array, and a held collector with -P welfgr, recording
   every numeric job ID. Later tasks fail fast if the smoke receipt is absent.
4. Setup builds and audits both exact Linux plugins, stages two complete
   isolated Stata bundles, and generates each deterministic dataset once.
5. Each array task runs both bundles on the same host. Scientific preflights
   complete before timing; nine fresh-process pairs alternate AB/BA and use two
   untimed stabilizing calls. A required confirmation sequence stays in the
   same task and therefore on the same node. The constrained-memory cell
   independently proves low-mode success and fast-mode r(909) for each arm
   under one common imposed limit.
6. The collector rejects any missing, duplicate, stale, or hash-mismatched
   task and passes the complete evidence document to the generic evaluator.
7. After the collector leaves qstat, fereg_f0_close.sh waits boundedly for
   qacct, requires failed=0, exit_status=0, project welfgr, and complete
   accounting for all 14 array tasks, then writes receipts/RUN.SUCCESS last.

The unsealed evidence handoff has this top-level shape:

    {
      "schema_version": 1,
      "evidence_type": "fereg-f0-regression-v1-evidence",
      "profile": "scc_governing",
      "manifest": {"id": "...", "sha256": "..."},
      "baseline": {"source": {}, "artifact": {}},
      "candidate": {"source": {}, "artifact": {}},
      "cells": []
    }

Each cell contains its own eight-field environment, scientific records for
both arms, nine ordered pairs, an optional nine-pair confirmation, and memory
records. Host identity is cell-scoped because SGE may place different array
tasks on different nodes; raw timings are paired only within a cell.

The held collector is deliberately not the final acceptance boundary:
-hold_jid proves completion rather than success, and a running collector
cannot have its own final qacct record. The post-job close stage binds setup,
every array task, and collector accounting to the already-hashed evaluator
output before publishing the sole run-level success marker.

### Exact execution sequence

Run preparation from a clean local checkout only after the harness, manifest,
and generic evaluator are committed at the candidate SHA. Use a new run ID for
every attempt:

    RUN_ID=YYYYMMDDTHHMMSSZ-f0-candidate
    LOCAL_RUN_ROOT=/private/tmp/fereg-f0-runs/$RUN_ID
    python3 benchmarks/scc/fereg_f0_scc.py prepare \
      --run-root "$LOCAL_RUN_ROOT" \
      --baseline-repo /path/to/fereg-f0 \
      --baseline-commit 40_HEX_F0_COMMIT \
      --candidate-repo /path/to/fereg-candidate \
      --candidate-commit 40_HEX_CANDIDATE_COMMIT \
      --manifest /path/to/fereg-candidate/benchmarks/manifests/fereg_f0_regression_v1.json
    bash benchmarks/scc/fereg_f0_deploy.sh "$LOCAL_RUN_ROOT"
    ssh scc "bash /projectnb/welfgr/fereg/runs/$RUN_ID/input/harness/fereg_f0_submit.sh /projectnb/welfgr/fereg/runs/$RUN_ID"

After the collector has left qstat, close the run on SCC so qacct is read from
the cluster accounting database rather than inferred from queue disappearance:

    ssh scc "bash /projectnb/welfgr/fereg/runs/$RUN_ID/input/harness/fereg_f0_close.sh /projectnb/welfgr/fereg/runs/$RUN_ID"

No governing run can pass while the manifest's Linux F0 artifact is pending or
while the baseline candidate is unsealed. The first exact setup build supplies
the Linux plugin SHA-256, size, embedded commit, build logs, and sealed setup
receipt. Review and bind that external identity in a new manifest commit before
running the full SCC matrix. A scientifically and operationally green but
unsealed matrix produces evaluator evidence for review; only a later run against
the reviewed frozen manifest may publish RUN.SUCCESS. Never edit a prepared run
in place to advance between these phases.
