# feppml platform load evidence

Phase 12 platform evidence is deliberately narrower than scientific
qualification.  The shared receipt proves that one exact source-bound plugin
artifact loaded in licensed Stata; native build/audit/actual-loader proof is a
separate platform authority (the Linux closed run includes both).  It sets
`scientific_claim=false`; mock SPI or a binary audit alone is never eligible.

## Closed evidence layers

`tests/stata/installation/feppml_platform_load_capture.do` is the common Stata
program on macOS, Linux, and Windows.  It loads the supplied plugin by exact
path, invokes only the non-scientific `version` action, and writes the closed
`feppml-licensed-stata-load-v1` JSON only after that call succeeds.  The JSON
binds the artifact SHA-256 and size, Stata version and flavor, exact capture
program SHA-256, full candidate commit, embedded 12-character commit, and the
terminal `FEPPML_LICENSED_STATA_LOAD_COMPLETE` sentinel.  It always records
`mock_only=false` and `scientific_claim=false`.

`scripts/feppml_platform_load_receipt.py` then resolves the full commit in the
local candidate repository, derives its exact tree, requires the embedded
commit in the artifact bytes, and seals the platform record with the source
archive SHA-256 and size, the UTC build date derived from the commit epoch,
and the separate controller commit/tree.  Qualification calls that
authoritative verifier and cross-binds its returned source, archive, artifact,
and capture-program identities to the candidate.  An otherwise valid receipt
for another archive, build date, artifact, or controller therefore fails.
The sealed `qualification_profile` is `candidate` only when the reviewed
target/harness lineage is exact; a deliberately distinct historical target is
typed `f0-baseline` and is ineligible for candidate activation.

## macOS capture and seal

Use a clean committed source and retain its exact Git archive outside the
repository.  Build the universal artifact with `FEREG_GIT_COMMIT` set to that
full commit and `FEREG_BUILD_DATE` set to its commit-epoch UTC date, then run:

```text
python3 -I -B -S scripts/capture_feppml_platform_load.py \
  --platform macos-universal \
  --artifact /path/to/fereg_macos.plugin \
  --stata /Applications/Stata/StataMP.app/Contents/MacOS/stata-mp \
  --source-repo /path/to/fereg \
  --source-commit <full-40-character-commit> \
  --source-archive /path/to/exact-git-archive.tar \
  --controller-repo /path/to/reviewed/controller \
  --controller-commit <full-40-character-controller-commit> \
  --attestation /new/evidence/root/native-attestation.json \
  --output /new/evidence/root/capture.json \
  --receipt /new/evidence/root/receipt.json \
  --record /new/evidence/root/platform-record.json
```

The driver isolates Stata's writable system directories, verifies the exact
plugin commit in both artifact bytes and plugin version output, requires a
closed Stata status plus exactly one terminal sentinel, rehashes the artifact
and program after the process, and retains the exact status, Stata log,
canonical wrapper, invocation, and committed Python producer as immutable
sidecars.  The offline verifier regenerates the only accepted wrapper byte for
byte and rejects extra, reordered, or early-exit code.  All outputs are
create-only.  Raw licensed logs are run evidence and must not be committed to
the source repository.

This macOS receipt establishes byte-level reproducibility under a trusted
local-host/operator assumption; its seals are content hashes, not signatures
from an external execution service.  A capture JSON by itself, or a
hand-edited wrapper/log bundle, cannot pass the verifier, but an operator with
authority to fabricate every local input is outside this evidence model.
Cryptographic proof against that operator would require a separately trusted,
signed macOS runner service.

## Standalone Linux SCC bootstrap

Linux evidence must exist before the 33-cell SCC qualification can be
prepared.  The standalone lifecycle avoids that dependency cycle and is
reusable for both the preactivation and activation commits.  Target source
and controller source are deliberately separate: an old F0 target need not
contain today's trusted capture, receipt, or SCC controller code.

Prepare a new local run directory from exact target and controller commits:

```text
python3 -I -B -S benchmarks/scc/feppml_linux_platform_scc.py prepare \
  --run-root /private/tmp/fereg-platform-runs/RUN_ID \
  --candidate-repo /path/to/target/fereg \
  --candidate-commit <full-40-character-target-commit> \
  --harness-repo /path/to/controller/fereg \
  --harness-commit <full-40-character-controller-commit>
```

Preparation retains a canonical Git tar and a resolving Git bundle for each
identity.  The job clones the target bundle for the build and the controller
bundle for orchestration, verifies both exact commits and trees, and retains
the canonical target tar for the shared platform receipt.  The formal
`FEREG_BUILD_DATE` is the UTC date derived from the target commit epoch; the
wall clock cannot change artifact bytes.

Then use, in order:

```text
benchmarks/scc/feppml_linux_platform_deploy.sh LOCAL_RUN_ROOT
benchmarks/scc/feppml_linux_platform_submit.sh LOCAL_RUN_ROOT
benchmarks/scc/feppml_linux_platform_collect.sh \
  LOCAL_RUN_ROOT /path/to/target/fereg /path/to/controller/fereg
```

Do not reuse a failed run ID.  Submission is one four-slot, one-hour SGE job
owned by `johannes` under `-P welfgr`; this is a bounded native build/load
gate, not a performance benchmark.  The exact `qsub` argv, unique
`feppml_platform_RUN_ID` job name, submit cwd, stdout path, script path, and
submitted script SHA-256/size are one sealed ledger.  The shell executes the
command generated from that ledger, so a hand-edited submission cannot
masquerade as the recorded job.  The job uses Rust 1.85.1, runs
`build_linux.sh` (including binary audit and the target-native actual-plugin
loader), and runs the current controller's licensed-Stata capture against the
target artifact.  That capture retains a sealed execution attestation,
Stata-owned status, exact Stata log, canonical wrapper, exact invocation, and
committed capture-producer bytes.

The collect step first verifies remote outputs, refuses an active job, waits
for raw `qacct` under a bounded 30-attempt policy, and closes only when the
accounting file contains exactly one scalar record.  It requires owner
`johannes`, project `welfgr`, the exact run-and-role job name and job ID,
parallel environment `omp`, four slots, ordered qsub/start/end timestamps,
`failed=0`, `exit_status=0`, and well-formed resources.  When qacct exposes
cwd, submit command, or scriptfile, each must equal the sealed ledger.  An
unrelated successful accounting record is rejected.

The collected `qacct` text is content-bound but is not a cryptographically
signed scheduler statement.  Formal collection must obtain it directly from
SCC through the guarded collector.  Offline verification detects byte changes
and cross-run substitution, but cannot authenticate a file supplied by an
untrusted party as having originated from SGE.

The qualification-ready outputs are:

- `output/fereg_linux.plugin`
- `output/capture.json`
- `output/native-attestation.json`
- `output/native-attestation.json.status`
- `output/native-attestation.json.stata.log`
- `output/native-attestation.json.wrapper.do`
- `output/native-attestation.json.invocation.json`
- `logs/build.log`
- `receipts/platform-load.json`
- `output/platform-record.json`

The outer build/load/accounting proof is
`receipts/linux-platform-close.json`.  It seals the source and archive
identity, commit-derived build date, job ID, normalized qacct fields, SHA-256
of the raw qacct bytes, submission ledger, build log, native capture records,
platform-record and receipt identities, and their scope booleans.  The shared
receipt verifier is authoritative for artifact/capture/source/controller and
native provenance.  The SCC verifier is authoritative for the outer
scheduler proof:

```text
python3 -I -B -S benchmarks/scc/feppml_linux_platform_scc.py verify-closed-run \
  --run-root LOCAL_RUN_ROOT \
  --candidate-repo /path/to/target/fereg \
  --harness-repo /path/to/controller/fereg \
  --expected-source-commit <full-40-character-target-commit> \
  --expected-harness-commit <full-40-character-controller-commit>
```

The final `RUN.SUCCESS` binds that close receipt and is written last.  Do not
infer a pass from a log line or construct capture JSON by hand.

## Windows capture

Windows uses the two-source controller described in
[Private Windows qualification](windows_qualification.md).  The target is a
sealed canonical Git tar and may be the pre-harness F0 commit.  The separate
synthetic controller commit contains the current trusted materializer, build
audit, actual-plugin loader, capture program, and bounded fixtures.  The
driver builds and stages only target package/plugin bytes, freezes the embedded
commit and build date from the target commit, and writes a source-side manifest
that separately binds mode, target, harness, controller, artifact, capture, and
the backend-provided run ID.  Candidate mode is eligible only when target and
harness commit/tree are identical; the explicitly labeled F0 mode requires a
distinct target commit and omits only the candidate-specific PPML loader check.

After all fixtures, the repository creates deterministic
`feppml-windows-results.zip` with exactly these members:

- `artifact/fereg_windows.plugin`
- `evidence/capture.json`
- `evidence/source-manifest.json`

Formal evidence additionally requires the fixed private backend to retain the
exact controller source ZIP, return that exact results ZIP, and bind its
SHA-256 and size into both the shared PASS receipt and sanitized output.  The
closed controller checks include `results_hash=true`; output includes the
matching `RESULTS_SHA256`, `RESULTS_SIZE_BYTES`, and sanitized results path.
This backend enhancement is not present in the current shared wrapper, so a
repository status marker or locally copied ZIP is not yet formal Windows
qualification.  Do not reconstruct either ZIP or synthesize a run ID.

Once the enhanced backend returns the authoritative bytes, seal them locally:

```text
python3 scripts/feppml_platform_load_receipt.py create \
  --platform windows-x86_64 \
  --artifact /sanitized/windows/artifact/fereg_windows.plugin \
  --capture /sanitized/windows/evidence/capture.json \
  --receipt /new/evidence/root/windows-platform-load.json \
  --record /new/evidence/root/windows-platform-record.json \
  --source-repo /path/to/target-repository \
  --source-commit <exact-target-commit> \
  --source-archive /sanitized/windows/feppml-windows-target-source.tar \
  --controller-repo /path/to/synthesized-controller \
  --controller-commit <exact-synthetic-controller-commit> \
  --capture-program /path/to/synthesized-controller/tests/stata/installation/feppml_platform_load_capture.do \
  --windows-controller-receipt /sanitized/windows/controller-receipt.json \
  --windows-controller-output /sanitized/windows/controller-output.txt \
  --windows-controller-archive /sanitized/windows/exact-controller-source.zip \
  --windows-results-archive /sanitized/windows/feppml-windows-results.zip \
  --windows-source-manifest /sanitized/windows/evidence/source-manifest.json
python3 scripts/feppml_platform_load_receipt.py verify \
  --receipt /new/evidence/root/windows-platform-load.json \
  --record /new/evidence/root/windows-platform-record.json
```

The verifier reconstructs the synthetic controller tree from the retained
source ZIP, validates the sealed two-source preparation, matches the shared
receipt to controller and results ZIP hashes/sizes, requires its run ID in the
source-side manifest, and requires the artifact/capture/manifest arguments to
equal their exact result-bundle members.  It separately resolves the target
commit/tree/archive/build date in the target repository.  Raw Stata startup
logs, license data, machine identifiers, and credentials are not platform
evidence.
