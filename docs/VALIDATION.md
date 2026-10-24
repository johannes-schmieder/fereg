# Validation status

The many-FE, two-FE structural-gate and ownership/deferred-cell improvements are
accepted. The runtime at `8de43810` completed source, native and public integration
checks on 2026-10-24. The cleanup/publication pass preserves that runtime.

## Accepted results

| Change | Evidence and scope |
| --- | --- |
| Many-FE Auto | [Final compact result](../bench/results/incremental-many-20261018-compact.json): Linux qualification and practical Mac recovery passed all 126 protected comparisons per platform; public activation and independent science passed. |
| Two-FE structural gates | [Linux](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json) and [Mac](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json): all 84 protected bounds per platform passed. [Activation](../bench/results/two-fe-policy-gates-activation-20261023.json): exact public/hidden science, worker, gate, hierarchy and resource checks passed after the documented transport-accounting amendment. |
| Ownership and deferred cells | [Linux](../bench/results/ownership-qualification-final-linux-20261024.json): 10,440 calls, 58 settings and 12 distinct physical hosts, all 116 protected bounds passed. [Practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json): 870 calls and all 116 paired-median comparisons within max(5%, 1 ms). The Mac check is descriptive, not a confidence-bound claim. |
| Public integration | [Final activation](../bench/results/ownership-public-activation-20261024.json): 174 actual hidden/public/public-repeat calls per platform, independent witnesses, fixed-worker exactness, cross-worker science, resource/gate/hierarchy checks, Mac/Linux ABI/load/panic checks and the full source suite passed. Six licensed Mac Stata fixtures and actual public OLS Break/rollback passed. |

Ownership/deferred cells activate only for OLS Auto/Auto with a global intercept
and exactly two categorical-intercept FE terms. Explicit injections and paths
outside the scope retain their contracts. The Break test interrupts at the
native pre-preparation checkpoint; it does not establish interruption inside a
cell constructor. Scientific behavior is specified in [SPEC](SPEC.md).

## Historical outcomes and limitations

- The earlier two-FE candidate failed the four-worker weak-graph Linux bound.
  The later independently qualified structural-gate candidate replaced it.
- Eager cells failed per-setting regressions. The accepted deferred redesign
  was qualified separately; historical failures remain failures.
- The ninther CMG setup milestone was accepted for continued development with
  two inconclusive setup bounds. This is not a claim that every setup improved.
- Scheduler/environment, transport-accounting and analysis failures occurred.
  Completed scientific calls were retained where valid; changes to adapters or
  reporting did not turn failed attempts into passes. The final Linux analysis
  timeouts and abandoned reconstruction-verifier attempt remain recorded.
- The source suite previously failed installed-package binding because the
  distribution still contained the older beta.2 binary. Package admission is a
  separate check on the newly built and installed beta.3 artifact.
- Linux evidence establishes Linux behavior, not Mac speed. The distributed
  plugin contains Apple Silicon and Intel slices; licensed installation tests
  run on Apple Silicon. No native Intel-host Stata, Linux binary distribution or
  Windows distribution qualification is claimed by this release.
- GitHub Actions remain paused.

The retained final JSON records are original summaries and may name historical
call parts or captures. Detailed experiments, removed parts, failed attempts and
previous validation narratives remain in Git at `8de43810` (for example,
`git show 8de43810:docs/VALIDATION.md`); deleted temporary captures are not retained
proof. New work does not need to replay those campaigns.

## Package checks

Current package identity is in [dist](../dist/README.md). The release result is
recorded here after testing the actual beta.3 package and public installation.
