# feppml public-activation checklist

This remains the formal no-waiver release checklist. Public `feppml` was
activated for full user testing on 2026-09-01 by explicit decision while
Windows licensed-Stata testing and the machine-enforced cross-platform
aggregate were deferred. That exception changes the public testing state, not
the evidence required for a tag, publication, or source-qualified release.
Nothing in this checklist should be read as claiming that deferred evidence
passed.

## Preconditions

Activate only from one committed, clean source SHA after all Phase 12 gates
are complete and attached to that SHA:

- full Rust, Python, C/mock-SPI, format, and warnings-denied Clippy evidence;
- licensed hidden PPML transaction, runtime, science, prediction, and Break
  profiles on the final native artifacts;
- the full `fereg` scientific, allocation, performance, and failure-atomicity
  matrix against the frozen F0 baseline;
- PPML oracle/comparator, constrained-memory, and saved-output qualification;
- exact macOS, Linux, and Windows artifact/load receipts; and
- source commit/tree/archive and embedded-commit identity on every platform
  receipt, with the Linux native build/load additionally closed by successful
  scalar qacct evidence;
- no unresolved waiver, dirty-source receipt, or mixed-source universal
  binary.

Close the complete inventory with the machine-enforced aggregate procedure in
[`phase12_activation_gate.md`](phase12_activation_gate.md). An individually
passing log or lower-level receipt is not the activation decision; the final
aggregate must report `public_state=preactivation_stub` and a passing committed
source-contract verdict.

## One activation diff

The activation change must contain no Rust, shared-engine, OLS-wire, solver,
memory-threshold, or numerical-policy edit.  It should do only the following.

1. Replace the public stub with an eclass, sort-preserving program whose only
   action is ``_fereg_ppml `0'``.  It must not parse a second time, expose
   `inject()`, call the plugin, post metadata, or display after the protected
   transaction.  Caller noisiness and the standard estimation table are
   already handled inside the protected private transaction.
2. Add `_fereg_ppml.ado`, `_fereg_estimate_ppml.ado`, and `feppml_p.ado` to
   both `stata/fereg.pkg` and `PRODUCTION_STATA_FILES` in
   `scripts/stage_release.py`.
3. Flip the package line, `stata.toc`, release-manifest metadata, and support
   caveats from OLS-only/fail-closed wording to the qualified OLS and PPML
   surface.  Keep package version `0.5.0`; record the additive PPML request and
   result wires without changing the OLS wire.
4. Replace `stata/feppml.sthlp` with the byte-exact activated help contract in
   `tests/stata/documentation/feppml_activation.sthlp`, and update the shared
   package references in `stata/fereg.sthlp`, `README.md`, the user guide,
   compatibility matrix, architecture/scientific/plugin-protocol docs,
   clean-machine instructions, release process, status, and changelog.  The
   help must match only implemented options, exact `e()`/prediction semantics,
   drop-reason codes, and failure classes.
5. Invert activation-sensitive source contracts.  Public `feppml` must be a
   one-call wrapper; the three production ado files must be present in both
   package inventories; and no active profile may retain the obsolete direct
   public-stub `r(498)` fixture.  Legitimate `r(498)` assertions for missing
   retained-FE prediction state and wrong staged bindings remain required.
6. Add the public `ppml-scientific` profile using the seven shared public
   science fixtures, add `tests/stata/documentation/feppml_help_examples.do`
   to activated `full`, keep the private transaction/runtime/Break and hidden
   help-example coverage, remove the public fail-closed fixture from active
   profiles, and schedule both staged-installation smokes. In
   `.github/workflows/stata-ci.yml`, replace the one manual-dispatch choice
   `ppml-fail-closed` with `ppml-scientific` while retaining `ppml-hidden`.
   The activation gate requires this exact one-line substitution and rejects
   every other workflow-byte change.

## Staged-installation proof

Do not accept a source-tree `which` check as package evidence. Produce the
deterministic binary release outside the repository and bind its SHA-256. The
staged runner alone is not a closed receipt: retain its exact invocation,
working directory, governing environment, process status, stdout, stderr, and
per-fixture logs through the committed supervisor. Use an exact clean checkout
of the target commit and an evidence directory dedicated to this run:

```bash
SOURCE_ROOT=/clean/exact-sha/fereg
EVIDENCE_ROOT=/evidence/staged
PYTHON=/absolute/path/to/the/python3-used-by-the-gate
STATA=/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp
ARCHIVE=/evidence/artifacts/fereg-0.5.0.zip
ARCHIVE_SHA256=<64-hex-digest>
SOURCE_SHA=<40-hex-target-commit>
BUILD_DATE=<UTC-YYYY-MM-DD-from-the-target-commit-committer-timestamp>

mkdir -p "$EVIDENCE_ROOT/fixture-logs"
FEREG_GIT_COMMIT="${SOURCE_SHA:0:12}" \
FEREG_BUILD_DATE="$BUILD_DATE" \
"$PYTHON" "$SOURCE_ROOT/ci/supervise_stata.py" \
  --timeout-seconds 1800 \
  --process-json "$EVIDENCE_ROOT/process.json" \
  --stdout-log "$EVIDENCE_ROOT/process.stdout.log" \
  --stderr-log "$EVIDENCE_ROOT/process.stderr.log" \
  --cwd "$EVIDENCE_ROOT" \
  -- \
  "$PYTHON" "$SOURCE_ROOT/scripts/run_staged_stata_tests.py" \
  --stata "$STATA" \
  --archive "$ARCHIVE" \
  --archive-sha256 "$ARCHIVE_SHA256" \
  --log-dir "$EVIDENCE_ROOT/fixture-logs" \
  --summary-json "$EVIDENCE_ROOT/staged-summary.json" \
  "$SOURCE_ROOT/tests/stata/installation/installation_smoke.do" \
  "$SOURCE_ROOT/tests/stata/installation/feppml_installation_smoke.do"
```

Use the same resolved Python executable for the supervisor, child command, and
aggregate verifier. The gate reconstructs the command above byte for byte and
requires the exact `process.json`, `process.stdout.log`,
`process.stderr.log`, `staged-summary.json`, and `fixture-logs/` layout. The
runner rejects traversal, links, duplicate roots, unsealed files, dirty source
manifests, archive or plugin hash drift, and any plugin outside the extracted
ado root. The staged gate independently reconstructs package metadata and
cross-binds all three packaged plugins to their qualified artifacts. It also
requires the exact ordered pair above:
the existing `fereg` installation smoke remains mandatory, and the additive
`feppml` smoke may not replace it. The PPML fixture independently verifies
that the public command, both private transaction files, `feppml_p.ado`, the
package files, and the platform plugin all resolve to exact paths inside that
extracted release.

## Exact activation qualification

On the activation SHA, rerun the repository suite, native platform builds and
loaders, licensed `quick`, `fereg-regression`, public `ppml-scientific`,
hidden PPML, `full`, both Break fixtures, and the staged-installation proof.
Then rerun the frozen fereg/F0 and feppml/oracle/comparator qualification
matrices.  Receipts must bind the activation source SHA, artifact SHA and
size, Stata host, profile, fixture sentinels, and staged release archive.

If any activation-only gate fails, revert the one activation diff.  Do not
repair it by changing shared numerical code under the public command.
After the exact activation rerun, close a new aggregate with
`public_state=activated`; pre-activation evidence is intentionally ineligible
for that receipt.
