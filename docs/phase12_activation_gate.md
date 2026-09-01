# Phase 12 and public-activation aggregate gate

`scripts/feppml_activation_gate.py` is the final Phase 12 evidence-aggregation
boundary. It closes either the private `preactivation_stub` state or the
public `activated` state for one exact committed source tree. It has no waiver
path and does not turn a status boolean, summary, or free-form log into proof.

The public command was enabled for user testing on 2026-09-01 with Windows and
the formal aggregate explicitly deferred. The gate is intentionally unchanged:
the source may report `activated`, but no aggregate PASS or release
qualification may be recorded until every required input, including Windows,
is present and independently verified.

SHA-256 envelopes provide integrity and byte binding, not authentication. The
authoritative value comes from reopening the retained raw inputs and invoking
the committed verifier for each evidence family. Evidence storage therefore
still needs appropriate access control and immutable archival retention.

## Closed source and artifact set

Every check and the aggregate bind the same full source commit and tree and
the exact five-row artifact inventory:

- the byte-exact `git archive --format=tar <commit>` source archive;
- the macOS universal, Linux x86-64, and Windows x86-64 plugins; and
- the deterministic staged-release ZIP.

All artifacts must remain regular non-symlink files with their recorded
filename, size, and SHA-256. Every plugin must contain the target commit's
12-character identity. The source checkout must be detached at the declared
commit/tree and clean, including no untracked files. A caller-supplied archive
hash or embedded-commit field is never accepted in place of reopening bytes.

The state-independent prerequisite inventory covers source identity,
repository checks, Python contracts, Rust, formatting, Clippy, C/mock SPI,
FFI, native build/audit/ABI, licensed Stata profiles and Break tests, frozen
fereg/F0, feppml qualification, all three licensed platform loads, staged
installation, and both SCC closures. `preactivation_stub` adds
`stata_ppml_fail_closed`; `activated` replaces it with
`stata_ppml_scientific`. The aggregate receipt inventory must be exact: no
missing, duplicate, additional, or waived check is accepted.

## Evidence families

### Local repository, Python, Rust, formatting, Clippy, C, and FFI

The only producer is the exact committed
`scripts/feppml_phase12_local.py`. The `run-check` command reconstructs and
executes its check-specific argv without a shell. The Rust registry includes
default-feature, no-default-feature, and all-feature workspace/all-target
tests. Clippy covers those same three feature modes with warnings denied. FFI
also runs the plugin's FFI all-target profile.

Each subprocess retains its exact argv, exit code, stdout, and stderr in a
canonical raw evidence directory. The producer retains its own stdout and
stderr separately and must emit its check sentinel exactly once. Offline
verification reopens every stream and the process manifest, reconstructs the
committed command registry and toolchain, and requires every exit code to be
zero. A hand-authored summary with plausible hashes cannot pass.

For example, `run-check` must receive the exact committed producer command
for the selected check; the gate rejects arbitrary argv, producer identities,
profiles, toolchains, output roles, or environment-key order:

```text
python3 -I -B -S scripts/feppml_activation_gate.py run-check \
  --check-id python \
  --source-root /clean/fereg \
  --source-sha <40-hex-commit> \
  --public-state preactivation_stub \
  --artifact-index /evidence/artifacts.json \
  --producer-id feppml-phase12-local \
  --producer-version 1 \
  --producer-entrypoint /clean/fereg/scripts/feppml_phase12_local.py \
  --environment-key PATH \
  --toolchain '<exact producer-reported toolchain>' \
  --profile phase12-local:python \
  --output summary /evidence/python-summary.json <expected-summary-sha256> \
  --timeout-seconds 3600 \
  --receipt /evidence/python-result.json \
  -- <resolved-python> /clean/fereg/scripts/feppml_phase12_local.py python \
     --output /evidence/python-summary.json \
     --evidence-dir /evidence/python-summary.json.raw
```

The summary path must not preexist. `run-check` creates the raw process
evidence and then seals the result; do not prewrite a claimed passing result.

### Frozen fereg/F0 and feppml qualification

The gate calls the exact committed authoritative modules rather than adapting
boolean fields:

- `scripts/fereg_f0_regression.py` re-evaluates the frozen manifest and its
  candidate-versus-F0 direct behavior/package observations, including the raw
  observation bundles;
- `scripts/feppml_qualification.py` recomputes the scientific evaluation from
  the manifest and evidence; and
- `benchmarks/scc/fereg_f0_scc.py` and
  `benchmarks/scc/feppml_qualification_scc.py` replay their closed SCC runs,
  retained raw task outputs, submission ledgers, saved qacct, collector
  output, and terminal closure.

The aggregate requires the qualification evidence/receipt to be the exact
files inside the corresponding verified SCC run. For F0 it independently
cross-binds the Linux candidate artifact used for same-node science/performance
and the licensed-host candidate artifact used for direct behavior/package
(macOS is valid). Both verified observations must name the aggregate candidate
commit/tree, and each platform artifact must match its own global
source-archive/build/toolchain receipt. Cross-format plugin bytes are not
expected to be equal. These bindings prevent a passing run and a different
build or source of either platform from being mixed.

### Native and licensed platform evidence

`scripts/feppml_native_qualification.py` is the single native authority. On
macOS its producer retains the exact build, binary-audit, and actual-loader
argv/status/stdout/stderr; verification independently reruns the nonmutating
audit and target-native ABI loader against the bound plugin. Linux must pass
`benchmarks/scc/feppml_linux_platform_scc.py`, including actual licensed load
and closed scalar qacct evidence. Windows must carry the accepted shared
controller PASS and source-side result archive produced by the committed
Windows workflow.

All formal builds use the target commit's UTC date as `FEREG_BUILD_DATE`.
Candidate-mode records require the candidate source/archive and harness to
match the aggregate target. Windows may have a distinct synthetic controller
commit, but its authoritative provenance must bind the exact candidate target
and harness; that controller identity is not mislabeled as candidate source.
The generic platform-load receipt proves load only and is never treated as
native build/audit/ABI evidence.

### Licensed Stata profiles

`ci/run_stata_ci.py` materializes an exact `git archive` of the requested
commit instead of executing working-tree files. The retained `source.tar` and
`source.json` bind commit, tree, archive SHA/size, and commit-derived build
date. Formal evidence forbids the tracked-file-manifest override.

`ci/supervise_stata.py` retains the full profile command argv, cwd, governing
environment digests, process status, and separate stdout/stderr bytes. The
gate reconstructs the exact committed profile/config/repository/run-root/
tested-SHA invocation. It then reopens every committed fixture, raw fixture
log, status file, profile summary, and Break receipt, requiring one exact
terminal sentinel and a successful Stata status. The plugin evidence must be
the exact qualified macOS artifact. An ordinary CI summary or a source-tree
run without this retained closure is not eligible.

### Staged installation

The staged adapter requires the supervised evidence layout documented in
`docs/feppml_activation_checklist.md`: `process.json`, separate supervisor
stdout/stderr, `staged-summary.json`, and raw fixture logs/status files. It
reconstructs the exact staged-runner argv, cwd, target commit/build-date
environment, Stata executable identity, archive identity, and ordered fixture
inventory.

The archive is safely extracted and checked member by member. Every committed
Stata/root file must equal its target Git blob. All three packaged platform
plugins and checksum files must equal the separately qualified artifacts. The
gate independently regenerates and byte-compares the dependency audit, SBOM,
`MANIFEST.sha256`, and release manifest. Preactivation requires the fereg
installation smoke; activation requires the ordered pair of fereg and feppml
installation smokes.

## Sealing check receipts

Each check has one exact ordered upstream role/type registry. `seal-check`
reopens those files, invokes the family-specific verifier, and then writes a
read-only `feppml-phase12-check-v1` receipt with exclusive creation. The check
sealer itself must be the exact committed activation gate:

```text
python3 -I -B -S scripts/feppml_activation_gate.py seal-check \
  --check-id feppml_qualification \
  --source-root /clean/fereg \
  --source-sha <40-hex-commit> \
  --public-state preactivation_stub \
  --artifact-index /evidence/artifacts.json \
  --producer-id feppml-activation-gate \
  --producer-version 1 \
  --producer-entrypoint /clean/fereg/scripts/feppml_activation_gate.py \
  --upstream qualification_evidence=/run/output/evidence.json \
  --upstream qualification_receipt=/run/receipts/qualification.json \
  --receipt /evidence/checks/feppml_qualification.json
```

Raw evidence types and sealed receipts are distinct in the schema. Every
upstream records its encoding, path, file SHA/size, payload SHA, and exact type.
Generic, Stata, and staged checks first use their dedicated producer/adaptation
command, then pass the resulting `producer_result` to `seal-check`.

## Aggregate evaluation and independent verification

The aggregate index is a closed object containing the public state, source,
artifact files, check-receipt files, activation lineage, an empty waiver list,
and the terminal input sentinel. Run:

```text
python3 -I -B -S scripts/feppml_activation_gate.py evaluate \
  --index /evidence/phase12-index.json \
  --source-root /clean/fereg \
  --receipt /evidence/phase12-aggregate.json

python3 -I -B -S scripts/feppml_activation_gate.py verify-receipt \
  --receipt /evidence/phase12-aggregate.json \
  --index /evidence/phase12-index.json \
  --source-root /clean/fereg
```

`verify-receipt` is not a structural seal check. It requires the original
index and source repository, reopens every artifact/receipt/upstream/raw file,
reruns `evaluate`, and requires the recomputed payload and sealed document to
match byte for byte. Without those authoritative inputs, no aggregate PASS is
reported.

## Activation lineage

An activated aggregate must name the original preactivation aggregate,
preactivation index, and parent source root. The gate recursively re-evaluates
that preactivation evidence and requires its exact passing commit to be the
single parent of the activation commit. A self-sealed or fabricated parent
receipt is insufficient.

The activation diff is closed to a reviewed control-plane path set with no
deletions, renames, copies, or type changes. The private PPML ado files, shared
public PPML fixtures, and activated help fixture must be byte-identical to the
prequalified parent. Package and stager inventories preserve their parent
order and add exactly the three private ado files in the required order.
`quick` and `full` are exact additive transforms; unrelated profile settings
and all non-public profiles are frozen. The public help must byte-match and
pass the semantic contract in `scripts/feppml_help_contract.py`. The workflow
must change exactly one manual-dispatch line from `ppml-fail-closed` to
`ppml-scientific`, preserving `ppml-hidden` and every other byte.

Only after every source, artifact, upstream, raw-process, licensed, SCC,
staged, and lineage check passes does the evaluator create the aggregate
receipt. A preactivation receipt cannot be relabeled or carried across the
activation commit.
