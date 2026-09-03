# Private Windows qualification

Windows qualification uses two independent source identities.  The **target**
is the exact `fereg` commit whose plugin and public package are under test.
The **harness** is the current reviewed controller, build audit, native loader,
capture program, and bounded Stata fixtures.  Keeping those identities
separate lets the current harness build the historical F0 commit even though
F0 predates the harness.  No file is copied into or attributed to F0.

## Prepare the clean controller

From a trusted local checkout, synthesize a new clean controller repository:

```text
python3 scripts/prepare_windows_feppml_controller.py \
  --output-repo /new/windows-controller \
  --target-repo /path/to/fereg \
  --target-commit <exact-target-commit> \
  --harness-repo /path/to/fereg \
  --harness-commit <exact-reviewed-harness-commit> \
  --mode candidate
```

Candidate mode requires target commit/tree to equal harness commit/tree, so a
stale or mismatched harness cannot qualify `feppml`.  The synthetic controller
commit remains distinct because it adds the sealed inputs.  Use
`--mode f0-baseline` for F0; that mode requires target and harness commits to
differ.  Preparation embeds the canonical `git archive --format=tar` of the
target plus a sealed contract containing its commit, tree, archive hash/size,
embedded 12-character commit, and UTC build date derived from the target
commit timestamp.  It separately records the harness commit/tree and hashes
every controller-owned program that can influence the result.  The synthetic
controller is then committed with deterministic metadata and must be clean.

Run the accepted private wrapper from that synthesized repository, not from
the target checkout.  The repository driver is `windows-ci.do`; it never
performs AWS, authentication, transfer, or license discovery.

## Repository driver contract

The driver performs one fail-closed sequence:

1. Require the backend-provided `WINDOWS_CI_RUN_ID`; never invent a run ID.
2. Verify the sealed preparation and every controller contract file, verify
   the target archive hash/size and embedded full Git commit, and materialize
   it under `target/windows-ci-target-source`.
3. Build from that target root.  `build_windows.ps1` receives the full target
   commit and the target commit's frozen UTC date, sets `FEREG_GIT_COMMIT` and
   `FEREG_BUILD_DATE` before Cargo, then uses the controller-owned binary audit
   and target-native actual-plugin loader.  Loader mode is closed: candidate
   mode retains the candidate-specific `estimate-ppml` routing assertion; F0
   mode skips only that assertion while retaining ABI, version, doctor,
   protocol-mismatch, and panic-boundary checks.  There is no `unknown` commit
   or wall-clock build-date fallback for a materialized build.
4. Parse the target's `stata/fereg.pkg`, substitute only the exact built
   Windows plugin, isolate all Stata lookup paths, and resolve every staged
   package file. Candidate mode requires and consumes the target-owned public
   PPML files; F0 mode explicitly excludes them.
5. Hash the exact staged artifact and controller capture program, run the
   common licensed-Stata load capture, and independently reparse its closed
   JSON.  The Stata program—not a handwritten receipt—publishes the successful
   load result.
6. Generate the sealed source-side manifest.  It binds the controller run ID,
   selected mode, target commit/tree/archive/hash/size/build date, controller
   and harness identities, artifact and capture hashes, capture-program hash,
   and licensed load result.
7. Run the bounded OLS fixture in both modes and the public PPML command
   contract plus guarded internal transaction regression only in candidate
   mode.
8. Create deterministic `feppml-windows-results.zip` with the exact closed
   inventory `artifact/fereg_windows.plugin`, `evidence/capture.json`, and
   `evidence/source-manifest.json`.
9. Write the exact 15-byte repository status marker only after every prior
   assertion and the result-bundle creation succeed.  It is the sole success
   publication and the final driver action.

The result bundle is reproducible: entries are stored without compression,
have fixed metadata, and are ordered by name.  Its producer also revalidates
the target receipt, checksum, built/staged artifact receipts, capture-input
receipt, and capture-verification receipt before writing those three members.

## Required private-backend enhancement

Repository support is complete, but the currently accepted shared wrapper and
backend do not yet expose all authority needed for formal evidence.  A formal
run is blocked until the fixed private controller does all of the following:

- inject its authoritative run ID as `WINDOWS_CI_RUN_ID`;
- preserve and return the exact pre-upload controller ZIP (including the clean
  `windows-ci-source.json`) instead of deleting the only copy;
- upload and return the exact repository-produced
  `feppml-windows-results.zip`;
- independently hash and size that ZIP, add `results_sha256` and
  `results_size_bytes` to the sanitized controller receipt, and require
  `checks.results_hash=true`; and
- add `RESULTS_SHA256`, `RESULTS_SIZE_BYTES`, and the sanitized
  `/private/tmp/windows-ci-results/<run-id>/results.zip` path to controller
  output while still proving cleanup, lock release, and stopped state.

Do not substitute a reconstructed ZIP, a copied directory, a hand-authored
manifest, or a run ID supplied by repository source.  A repository status
marker or source-side manifest alone is diagnostic, not formal Windows
qualification.

## Local sealing after backend support

After the enhanced wrapper returns all exact bytes, extract the three closed
result members without changing them and seal through the authoritative
verifier:

```text
python3 -I -B -S scripts/feppml_platform_load_receipt.py create \
  --platform windows-x86_64 \
  --artifact /evidence/artifact/fereg_windows.plugin \
  --capture /evidence/evidence/capture.json \
  --receipt /evidence/windows-platform-load.json \
  --record /evidence/windows-platform-record.json \
  --source-repo /path/to/target-repository \
  --source-commit <exact-target-commit> \
  --source-archive /evidence/feppml-windows-target-source.tar \
  --controller-repo /path/to/synthesized-controller \
  --controller-commit <exact-synthetic-controller-commit> \
  --capture-program /path/to/synthesized-controller/tests/stata/installation/feppml_platform_load_capture.do \
  --windows-controller-receipt /evidence/controller-receipt.json \
  --windows-controller-output /evidence/controller-output.txt \
  --windows-controller-archive /evidence/exact-controller-source.zip \
  --windows-results-archive /evidence/feppml-windows-results.zip \
  --windows-source-manifest /evidence/evidence/source-manifest.json
python3 -I -B -S scripts/feppml_platform_load_receipt.py verify \
  --receipt /evidence/windows-platform-load.json \
  --record /evidence/windows-platform-record.json
```

The artifact, capture, and source manifest arguments must be the exact bytes
inside the returned results ZIP.  Raw Stata startup logs, machine identifiers,
paths, credentials, and license details are never qualification artifacts.
