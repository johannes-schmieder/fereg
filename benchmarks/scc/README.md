# SCC benchmark workflows

## Choose the smallest lane

The default while fereg is a prototype is the development smoke, not either
qualification workflow below. Use a fresh directory under
`/projectnb/welfgr/fereg/dev/`, stage the exact source as `source/`, and submit
`fereg_dev_smoke_job.sh` with all three variables explicit:

```bash
mkdir -p /projectnb/welfgr/fereg/dev/RUN_ID/logs
qsub -wd /projectnb/welfgr/fereg/dev/RUN_ID \
  -o /projectnb/welfgr/fereg/dev/RUN_ID/logs \
  -v RUN_ROOT=/projectnb/welfgr/fereg/dev/RUN_ID,FEREG_DEV_PROFILE=ols,FEREG_DEV_SNAPSHOT=SOURCE_ID \
  /projectnb/welfgr/fereg/dev/RUN_ID/source/benchmarks/scc/fereg_dev_smoke_job.sh
```

`FEREG_DEV_PROFILE` has no default: choose `ols`, `ppml`, or `shared` to match
the changed boundary. The job uses four slots, 30 minutes, and 4 GB per core,
and leaves a concise `output/summary.json`, fixture logs, plugin evidence, and
`output/DEV_SMOKE.SUCCESS`. Inspect it directly after submission; do not create
a heartbeat for this normal short job.

The qualification sections below are archival/claim-grade procedures. Run
them only when Johannes explicitly requests qualification, detailed tests, or
release evidence. Their existence and any open historical gate are not
standing authorization to submit them.

The paper AKM runner uses `BENCH_MODE=benchmark-canary` for its single
8-million-row diagnostic and `BENCH_MODE=production` for the full measurement
schedule. The canary is still a large focused benchmark, not a smoke test; run
it only to answer a concrete performance or memory question.

## feppml Phase 12 qualification

`feppml_qualification_scc.py` is the governing exact-source execution layer
for the frozen manifest in
`benchmarks/manifests/feppml_qualification_v1.json`.  It is separate from the
development PPML memory/performance evaluator: development evidence can guide
engineering, but it cannot publish a qualification receipt.

The preparation-time capture specification is a deliberately non-executable
template reserving the closed evidence header and the expected Stata version.
It contains no interpreter, program, plan, or caller-supplied runtime argv.
Setup reconstructs those identities from the committed harness and the sealed
materialized plan.  The preflight
program writes the complete candidate/comparator/oracle scientific evidence
with explicit timing placeholders and attests that zero timed arms have
started.  The arm program runs exactly one licensed-Stata arm in a fresh
process and writes its elapsed time, externally observed peak RSS, process ID,
node, Stata version, requested/effective thread control, and exact artifact
SHA.

Create both pre-prepare inputs in one fail-closed invocation.  This command
does not load or require a capture plan:

    python3 scripts/generate_feppml_qualification_spec.py --template \
      --qualification-manifest benchmarks/manifests/feppml_qualification_v1.json \
      --development-manifest benchmarks/manifests/ppml_memory_performance_v1.json \
      --evidence-header /path/to/exact-evidence-header.json \
      --stata-version "19.0 MP" \
      --output /path/to/capture-spec-template.json \
      --output-request /path/to/materialization-request.json

The second output is the closed
`feppml-capture-materialization-request-v1` document passed to the historically
named `prepare --capture-plan` argument; it is a request, not an executable
plan.  The generator rejects runtime interpreter/program/plan arguments in
template mode.
Setup creates `work/capture/plan.json`, `work/capture/spec.json`, and the sealed
materialization receipt only after the exact Stata executable, datasets,
do-files, plugin, comparator tree, oracle fixtures, and tools exist. Every
Stata recipe
must write its closed scientific/diagnostic record plus READY/START/DONE
markers; the capture boundary never manufactures missing certificates or
allocator counters.

`generate_feppml_qualification_recipes.py` expands the frozen development
manifest into all 33 deterministic cell recipes. It assigns data seeds by
scientific dataset identity (so thread, route, and saved-output comparisons use
the same bytes), emits one untimed generator per distinct dataset, and emits
separate normal candidate/comparator timing programs. Candidate allocation
audits are distinct commands containing `allocationaudit`; normal timing
programs never contain that option. SCC setup compiles these recipes, builds
the exact candidate archive with `scripts/build_linux.sh`, stages the complete
frozen ppmlhdfe/reghdfe/ftools/require closure, materializes each dataset once
in licensed Stata, and records dataset, generator, artifact, and source hashes
before scientific preflight.

The governing lifecycle is:

1. `prepare` resolves full candidate and reference commits, creates clean git
   archives, verifies the frozen ppmlhdfe/reghdfe/ftools/require closure,
   rejects a production `estimate-ppml` source that still mentions
   `fit_ppml_separated` or `profile_irls_step`, binds the archive identities to
   the evidence header, and snapshots every harness input.
2. `feppml_qualification_deploy.sh` transfers a new run ID without overlay or
   deletion, verifies every input byte on SCC, and seals an independent remote
   deployment receipt containing the locally computed preparation SHA-256.
3. `feppml_qualification_submit.sh` submits setup, the closed 33-cell array,
   and the held collector under project `welfgr`.
4. Setup extracts/builds the exact archives, materializes and verifies the
   capture plan, rebinds the executable spec (including staged platform
   evidence), and completes the full scientific preflight before writing
   `SETUP.SUCCESS`.  The marker and setup receipt bind preparation, plan,
   materialization receipt, and spec hashes. No timed task can start without
   that sentinel.
5. Each task runs one cold and nine warm same-node pairs in the frozen
   `AB, BA, AB, BA, AB, BA, AB, BA, AB` order.  Every arm is a fresh process;
   raw seconds, PIDs, RSS, node, Stata, and artifact hashes are sealed in a
   cell receipt.
6. The collector requires all 33 sealed shards, reconstructs the development
   evidence without replacing any raw measurement, then evaluates science,
   memory/saved-output gates, and paired performance in that order.
7. After all jobs leave the queue,
   `feppml_qualification_close.sh` requires complete `qacct` records with
   `failed=0`, `exit_status=0`, and project `welfgr` for setup, all 33 array
   tasks, and collection.  `RUN.SUCCESS` is written last and only for a passing
   sealed qualification receipt.

Preparation intentionally fails until the candidate and harness are committed
and the live plugin route no longer reaches the legacy fitter.  No SCC run is
part of implementation itself; use a new immutable run directory when the
candidate is ready.  A typical preparation command is:

    python3 -I -B -S benchmarks/scc/feppml_qualification_scc.py prepare \
      --run-root /private/tmp/fereg-ppml-runs/20260831T120000Z-feppml-q \
      --candidate-repo /path/to/fereg --candidate-commit 40_HEX_SHA \
      --manifest /path/to/fereg/benchmarks/manifests/feppml_qualification_v1.json \
      --capture-spec /path/to/capture-spec-template.json \
      --capture-plan /path/to/materialization-request.json \
      --ppmlhdfe-archive /path/to/ppmlhdfe-pinned.tar.gz --reghdfe-repo /path/to/reghdfe \
      --ftools-repo /path/to/ftools --require-repo /path/to/stata-require

The three platform records contain immutable `artifact`, `capture`,
`source_archive`, `capture_program`, and `receipt` file records plus the full
platform provenance closure.  Native records carry the producer, attestation,
status, Stata log, wrapper, and invocation sidecars.  Windows records carry
the controller receipt/output/archive, results archive, and source manifest.
Preparation invokes the authoritative platform verifier on the original
records, copies every verified companion into the sealed input without
renaming runtime-sensitive files, and setup rebinds and verifies the copied
closure before it can enter the late spec. They remain artifact/load receipts
only. In particular,
the Windows x86-64 receipt must record an actual licensed Stata load of the
exact artifact, set `licensed_stata_load=true`, `mock_only=false`, and
`scientific_claim=false`.  A build or mock-SPI test is useful engineering
evidence but cannot satisfy this platform gate and is never represented as
Windows scientific parity.

Every licensed child receives a complete closed environment rather than the
controller's inherited process environment.  The capture fixes its private
home/temp roots, Stata executable path, locale, timezone, and the narrowly
allowed constrained-memory variables; pre-existing `FEREG_*`, `PYTHONPATH`,
dynamic-loader injection, thread-control, and module variables never cross the
process boundary.  Capture-side parser/normalizer imports are resolved from
the staged sibling files, not from `PYTHONPATH`.  Raw supervisors bind this
entire environment and offline replay requires exact equality.  The
independent Python oracle uses its own equally closed environment and the
exact isolated argv `python -I -B -S <sealed-program>`; its local scientific
modules are loaded by exact staged file identity.

## F0 regression qualification

`fereg_f0_scc.py` is the exact-source Linux/SCC layer for the direct
candidate-versus-F0 contract. The generic evaluator remains the only
scientific/performance decision maker. The governing inventory has 42 cells;
all cells, raw captures, scheduler identities, and accounting records are
closed before a run-level result exists.

Every preparation contains three independent git archives: the historical F0
target, the target under test, and the reviewed controller/harness commit. For
sealed manifests, preparation also copies only the external bytes named by the
artifact and qualification seals into `input/seal-evidence`. Its sorted file
inventory records every relative path, SHA-256, and size. Evaluator code comes
from the exact harness archive while seal paths resolve through this separate
copied root, so raw licensed evidence stays outside Git without becoming an
unverified input.

Each task receipt inventories the complete retained task directory. The live
task and offline verifier independently reparse the do-files, raw Stata
status/log/capture, output signatures, e-return inventory, terminal sentinel,
process status, and RSS evidence to reconstruct the cell JSON. Fweighted
memory envelopes use sealed physical rows, never weighted `e(N)`.

Formal setup builds both arms and runs the small `ci/fereg_smoke.do` fixture
against each exact Linux plugin. Only after both pass does one array job run
tasks 1 through 42, followed by the collector. There is no separate serial
"smoke" job consuming a full governing benchmark cell.

Submission retains each raw `qsub -terse` response, start/completion
timestamps, exact argv, cwd, job name, hold dependency, submitted script hash,
and parsed job ID. The close command has no caller-supplied qacct override: it
queries SCC directly and checks exact owner, project, job/task inventory,
slots, scheduler timestamps, and exposed cwd/script/argv. Raw qacct bytes are
SHA-bound into the close receipt. Offline verification is relocation-aware: it
reads evidence files below the collected local run root, but replays the sealed
original SCC submit cwd, script paths, argv, and qacct fields rather than
regenerating those paths on the verifier host. SGE accounting is an
administrative database, not a cryptographically signed external service; the
verifier proves exact internal cross-binding and retained bytes but cannot add
a scheduler signature that SCC does not provide.

### Artifact-sealing input

`seal-artifacts` consumes a closed JSON input. Every path is relative to the
repository/evidence root, and bundle filenames must be preserved because the
behavior receipts bind them.

```json
{
  "local_full_receipt": "build_evidence/local/f0-2dee5d8/receipt.json",
  "direct_behavior_observation": "build_evidence/f0/behavior/baseline-observation.json",
  "direct_behavior_bundle": "build_evidence/f0/behavior/baseline-bundle.tar",
  "macos": {
    "artifact": "...", "capture": "...", "receipt": "...", "record": "...",
    "source_archive": "...", "capture_program": "...",
    "native_attestation": "...", "native_producer": "..."
  },
  "linux_run_root": "build_evidence/f0/platform/linux-run",
  "windows": {
    "artifact": "...", "capture": "...", "receipt": "...", "record": "...",
    "source_archive": "...", "capture_program": "...",
    "windows_controller_receipt": "...", "windows_controller_output": "...",
    "windows_controller_archive": "...", "windows_results_archive": "...",
    "windows_source_manifest": "..."
  }
}
```

The shared platform verifier must classify every record as `f0-baseline`.
macOS and Linux bind the reviewed controller directly. Windows binds the same
reviewed harness through authoritative provenance while keeping its synthetic
prepared controller commit distinct.

### Exact execution sequence

This template is not authorization to submit. Use full 40-hex commits and a
new run ID for every attempt.

First create the artifact seal and an auxiliary artifact-sealed manifest. The
auxiliary manifest must be committed with the controller code before the
diagnostic run. Before sealing, capture and verify the exact baseline licensed
behavior bundle on the native platform whose F0 artifact will be used (macOS
is supported and is the currently registered baseline):

```bash
python3 -I -B -S scripts/fereg_f0_behavior.py capture \
  --role baseline --phase baseline \
  --source /clean/f0 --plugin /evidence/macos/fereg_macos.plugin \
  --stata /Applications/Stata/StataMP.app/Contents/MacOS/stata-mp \
  --f0-manifest benchmarks/manifests/fereg_f0_regression_v1.json \
  --run-root build_evidence/f0/behavior/baseline-run \
  --output build_evidence/f0/behavior/baseline-observation.json \
  --bundle-output build_evidence/f0/behavior/baseline-bundle.tar \
  --identity-output build_evidence/f0/behavior/baseline-observation.identity.json
python3 -I -B -S scripts/fereg_f0_behavior.py verify \
  --receipt build_evidence/f0/behavior/baseline-observation.json \
  --bundle build_evidence/f0/behavior/baseline-bundle.tar \
  --identity build_evidence/f0/behavior/baseline-observation.identity.json
```

The `candidate_unsealed` manifest permits this capture only when the native
slot is pending or the recorded `captured_unsealed` bytes match exactly; the
subsequent artifact seal re-verifies and qualifies the same artifact.

Then create the seal:

```bash
python3 -I -B -S scripts/fereg_f0_regression.py seal-artifacts \
  --manifest benchmarks/manifests/fereg_f0_regression_v1.json \
  --evidence-spec build_evidence/f0/seal/artifact-input.json \
  --controller-repository . --controller-commit "$HARNESS_SHA" \
  --artifact-seal-output build_evidence/f0/seal/artifact-seal.json \
  --manifest-output benchmarks/manifests/fereg_f0_regression_artifacts_sealed_v1.json
```

Prepare the exact F0-versus-F0 diagnostic. It carries no candidate behavior
comparison: the behavior contract correctly forbids calling F0 a distinct
candidate, while the baseline behavior bundle is already inside the artifact
seal. Its evaluator is deliberately blocked; only `complete_nonqualifying` is
valid.

```bash
RUN_ID=YYYYMMDDTHHMMSSZ-f0-diagnostic
LOCAL_RUN_ROOT=/private/tmp/fereg-f0-runs/$RUN_ID
python3 -I -B -S benchmarks/scc/fereg_f0_scc.py prepare \
  --run-root "$LOCAL_RUN_ROOT" \
  --baseline-repo /path/to/fereg --baseline-commit "$F0_SHA" \
  --candidate-repo /path/to/fereg --candidate-commit "$F0_SHA" \
  --harness-repo /path/to/fereg --harness-commit "$HARNESS_SHA" \
  --manifest /path/to/fereg/benchmarks/manifests/fereg_f0_regression_artifacts_sealed_v1.json \
  --seal-root /path/to/fereg
bash benchmarks/scc/fereg_f0_deploy.sh "$LOCAL_RUN_ROOT"
ssh scc "bash /projectnb/welfgr/fereg/runs/$RUN_ID/input/harness/fereg_f0_submit.sh /projectnb/welfgr/fereg/runs/$RUN_ID"
# Only after all submitted jobs have left qstat:
ssh scc "bash /projectnb/welfgr/fereg/runs/$RUN_ID/input/harness/fereg_f0_close.sh /projectnb/welfgr/fereg/runs/$RUN_ID"
python3 -I -B -S benchmarks/scc/fereg_f0_scc.py verify-diagnostic-run \
  --run-root /path/to/collected/$RUN_ID \
  --manifest benchmarks/manifests/fereg_f0_regression_artifacts_sealed_v1.json
```

Freeze only from that collected closure. Keep the artifact-sealed auxiliary
manifest because the qualification seal binds its exact bytes. Review the
staged frozen manifest, copy those exact bytes to the canonical path, validate,
and commit the canonical frozen manifest plus the auxiliary manifest:

```bash
python3 -I -B -S scripts/fereg_f0_regression.py freeze-baseline \
  --manifest benchmarks/manifests/fereg_f0_regression_artifacts_sealed_v1.json \
  --diagnostic-run-root build_evidence/f0/diagnostic/$RUN_ID \
  --qualification-seal-output build_evidence/f0/seal/qualification-seal.json \
  --manifest-output build_evidence/f0/seal/fereg_f0_regression_frozen_v1.json
cp build_evidence/f0/seal/fereg_f0_regression_frozen_v1.json \
  benchmarks/manifests/fereg_f0_regression_v1.json
python3 -I -B -S scripts/fereg_f0_regression.py validate-manifest \
  --manifest benchmarks/manifests/fereg_f0_regression_v1.json \
  --evidence-root .
```

After the frozen manifest and exact preactivation candidate SHA are committed,
prepare the final candidate-versus-F0 run. Reuse the exact baseline observation
and bundle already sealed above, then capture the candidate on that same
licensed host, native platform, and Stata environment:

```bash
python3 -I -B -S scripts/fereg_f0_behavior.py capture \
  --role candidate --phase preactivation \
  --source /clean/candidate --plugin /evidence/macos/candidate/fereg_macos.plugin \
  --stata /Applications/Stata/StataMP.app/Contents/MacOS/stata-mp \
  --run-root build_evidence/f0/behavior/candidate-run \
  --output build_evidence/f0/behavior/candidate-observation.json \
  --bundle-output build_evidence/f0/behavior/candidate-bundle.tar \
  --identity-output build_evidence/f0/behavior/candidate-observation.identity.json
python3 -I -B -S scripts/fereg_f0_behavior.py evaluate \
  --baseline-receipt build_evidence/f0/behavior/baseline-observation.json \
  --baseline-bundle build_evidence/f0/behavior/baseline-bundle.tar \
  --candidate-receipt build_evidence/f0/behavior/candidate-observation.json \
  --candidate-bundle build_evidence/f0/behavior/candidate-bundle.tar \
  --output build_evidence/f0/behavior/candidate-vs-f0-comparison.json \
  --identity-output build_evidence/f0/behavior/candidate-vs-f0-comparison.identity.json
python3 -I -B -S scripts/fereg_f0_behavior.py verify \
  --receipt build_evidence/f0/behavior/candidate-vs-f0-comparison.json \
  --identity build_evidence/f0/behavior/candidate-vs-f0-comparison.identity.json \
  --baseline-receipt build_evidence/f0/behavior/baseline-observation.json \
  --baseline-bundle build_evidence/f0/behavior/baseline-bundle.tar \
  --candidate-receipt build_evidence/f0/behavior/candidate-observation.json \
  --candidate-bundle build_evidence/f0/behavior/candidate-bundle.tar
```

This licensed-host comparison and the Linux SCC comparison intentionally use
platform-specific plugin bytes. Both must name the same exact baseline and
candidate source commit/tree; the Phase 12 aggregate additionally cross-binds
the licensed-host candidate artifact to that platform's authoritative
source-archive/build/toolchain receipt, while SCC independently binds the
Linux artifacts and same-node measurements. No cross-format plugin-byte
equality is required. The behavior capture remains a real prerequisite, not a
receipt synthesized by the governing run. The final stage is the only one
allowed to produce `RUN.SUCCESS`:

```bash
RUN_ID=YYYYMMDDTHHMMSSZ-f0-final
LOCAL_RUN_ROOT=/private/tmp/fereg-f0-runs/$RUN_ID
python3 -I -B -S benchmarks/scc/fereg_f0_scc.py prepare \
  --run-root "$LOCAL_RUN_ROOT" \
  --baseline-repo /path/to/fereg --baseline-commit "$F0_SHA" \
  --candidate-repo /path/to/fereg --candidate-commit "$PREACTIVATION_SHA" \
  --harness-repo /path/to/fereg --harness-commit "$PREACTIVATION_SHA" \
  --manifest /path/to/fereg/benchmarks/manifests/fereg_f0_regression_v1.json \
  --seal-root /path/to/fereg \
  --behavior-baseline /path/to/f0-baseline-observation.json \
  --behavior-baseline-bundle /path/to/f0-baseline-bundle.tar \
  --behavior-candidate /path/to/candidate-preactivation-observation.json \
  --behavior-candidate-bundle /path/to/candidate-preactivation-bundle.tar \
  --behavior-comparison /path/to/candidate-vs-f0-comparison.json
bash benchmarks/scc/fereg_f0_deploy.sh "$LOCAL_RUN_ROOT"
ssh scc "bash /projectnb/welfgr/fereg/runs/$RUN_ID/input/harness/fereg_f0_submit.sh /projectnb/welfgr/fereg/runs/$RUN_ID"
# Only after all submitted jobs have left qstat:
ssh scc "bash /projectnb/welfgr/fereg/runs/$RUN_ID/input/harness/fereg_f0_close.sh /projectnb/welfgr/fereg/runs/$RUN_ID"
python3 -I -B -S benchmarks/scc/fereg_f0_scc.py verify-closed-run \
  --run-root /path/to/collected/$RUN_ID \
  --manifest benchmarks/manifests/fereg_f0_regression_v1.json
python3 -I -B -S scripts/fereg_f0_regression.py verify-receipt \
  --receipt /path/to/collected/$RUN_ID/output/qualification.json \
  --manifest benchmarks/manifests/fereg_f0_regression_v1.json
```

Never edit or overlay a prepared run to advance lifecycle state. Artifact
sealing, diagnostic closure, freezing, and final qualification require
distinct immutable outputs.
