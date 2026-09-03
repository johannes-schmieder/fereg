# Direct F0 command and package contract

This note is the normative companion to the broader
[`fereg` regression contract](fereg_regression_contract.md). The timed F0
matrix protects representative estimation behavior, memory, and performance;
this separate contract protects the public command and staged-package surface
that timing cells do not exercise.

The machine-readable authority is
`benchmarks/manifests/fereg_f0_behavior_v1.json`, and the only producer and
verifier is `scripts/fereg_f0_behavior.py`. The contract covers syntax,
defaults, validation and return codes, utility actions, prediction and
postestimation, failure atomicity, all executable `fereg.sthlp` examples, and
the exact package inventory. It contributes no timing observation.

## Closed evidence

Each observation must come from a clean detached source checkout, its exact
native plugin, and one isolated licensed-Stata installation. The producer
retains both a sealed JSON observation and a canonical USTAR evidence bundle.
The bundle contains the exact fixture/resource sources, sanitized raw logs and
statuses, staged package/plugin inventory, help blocks, and environment
identity. A receipt without its bundle is not verifiable evidence.

Capture the reusable baseline observation:

```bash
python3 -I -B -S scripts/fereg_f0_behavior.py capture \
  --role baseline --phase baseline \
  --source /path/to/clean/f0 \
  --plugin /path/to/f0/fereg_macos.plugin \
  --stata /path/to/StataMP \
  --run-root /path/to/new/f0-command-baseline-run \
  --output /evidence/f0-baseline-observation.json \
  --bundle-output /evidence/f0-baseline-bundle.tar \
  --identity-output /evidence/f0-baseline-observation.identity.json
```

Capture the current public candidate with `--phase activation`:

```bash
python3 -I -B -S scripts/fereg_f0_behavior.py capture \
  --role candidate --phase activation \
  --source /path/to/clean/candidate \
  --plugin /path/to/candidate/fereg_macos.plugin \
  --stata /path/to/StataMP \
  --run-root /path/to/new/f0-command-candidate-run \
  --output /evidence/candidate-observation.json \
  --bundle-output /evidence/candidate-bundle.tar \
  --identity-output /evidence/candidate-observation.identity.json
```

Evaluate the two observations and then independently reopen every input:

```bash
python3 -I -B -S scripts/fereg_f0_behavior.py evaluate \
  --baseline-receipt /evidence/f0-baseline-observation.json \
  --baseline-bundle /evidence/f0-baseline-bundle.tar \
  --candidate-receipt /evidence/candidate-observation.json \
  --candidate-bundle /evidence/candidate-bundle.tar \
  --output /evidence/candidate-vs-f0-comparison.json \
  --identity-output /evidence/candidate-vs-f0-comparison.identity.json

python3 -I -B -S scripts/fereg_f0_behavior.py verify \
  --receipt /evidence/candidate-vs-f0-comparison.json \
  --identity /evidence/candidate-vs-f0-comparison.identity.json \
  --baseline-receipt /evidence/f0-baseline-observation.json \
  --baseline-bundle /evidence/f0-baseline-bundle.tar \
  --candidate-receipt /evidence/candidate-observation.json \
  --candidate-bundle /evidence/candidate-bundle.tar
```

The baseline and candidate use the same pinned macOS controller, native
platform, and identical licensed-Stata environment. Linux SCC performance is
a separate, source-bound receipt with its own Linux artifacts; it does not
reuse or require byte equality with the macOS behavior plugin. The full F0
sealing and SCC lifecycle is documented in
[`benchmarks/scc/README.md`](../benchmarks/scc/README.md).

Capture is fail-closed to the registered macOS Stata/MP executable bytes. Each
fixture runs with a private HOME/TMPDIR and closed environment; the evidence
bundle retains the exact wrapper, invocation/argv/cwd/environment, merged
process stream, Stata log, status file, and process/Stata return codes.

## Preactivation and activation boundaries

The phase names are historical schema vocabulary for interpreting frozen
receipts; they do not represent a pending public-surface switch. Current
candidates use `activation` because the public PPML files ship in the package.

For a preactivation comparison, every installed baseline file except the
native plugin remains byte-identical. This includes the public `feppml` stub,
both public help files, `fereg.pkg`, and `stata.toc`. The plugin is bound
separately by source, platform, SHA-256, size, and embedded commit and is
exercised by the frozen fixtures.

The historical activation capture uses `--phase activation` and permits only
the additive allowance frozen in the behavior manifest: the three PPML package
members may be added in their prescribed order, and only the enumerated shared
package/toc/help files may change. Every `fereg`-owned control-plane byte and
executable help block remains exact. This phase label preserves interpretation
of existing receipts; it is not a current public-surface switch.
