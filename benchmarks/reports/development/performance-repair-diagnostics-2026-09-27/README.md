# Performance repair: diagnostic baseline and harness checks

Development evidence, not qualification or headline timings. Baseline source is
`a05882933fb0f1fc668e2a09bed8ff768b7329be`; `baseline-source.json` binds all 210
computational files. `baseline-plan.json` freezes ten attempts using the unchanged
systematic generator and original seed 104729. Linux plugin SHA-256:
`259e27939df8288456a254bdebe150682dac6d30fff321e2bc32d0bdac04d909`.

SCC job **7759645** uses one isolated E5-2680v4 node, 28 slots, 3 GiB per slot
and a two-hour limit. Individual processes have nested CPU affinities and native
thread counts. Stata itself is limited to its four licensed processors. Logs and
traces are retained under development run `20260927-performance-repair-a0588293`,
`diagnostic01`. Instrumentation clocks must not be compared with stock headline
timings. Failed attempts remain in the inventory.

## Harness repairs checked locally

- Four new worker-diagnostic tests pass: warmup exceptions produce receipts;
  failure stage and application return code survive quiet logs, command failures,
  startup timeouts and extraction timeouts. Six existing campaign tests also pass.
- Two capacity-preflight tests pass. The bias benchmark controller checks the
  maximum requested Stata/Mata team across the entire frozen plan before any
  timed fit; it never silently clamps either arm. A real Stata 19 preflight at
  one processor passes; an intentionally impossible request returns r(198)
  before any timed fit. New supported plans require fresh frozen identities.
- A real licensed Stata error probe verifies that `capture noisily quietly`
  preserves the error text and return code while successful commands remain
  quiet. Previous `capture quietly` suppressed the error text.
- Broader local systematic discovery could not import two plotting-dependent
  test modules because matplotlib is absent in the bundled Python environment.
  This is an environment limitation, not a passing check. No dependencies were
  installed and no original receipts or comparator settings were modified.

Raw licensed logs stay outside Git. The changes add failure attribution and
early capacity rejection; scientific eligibility, timing gates and tolerances
are unchanged. The controller's source fingerprint now includes the failure
helper so later frozen harnesses cannot omit its identity.
