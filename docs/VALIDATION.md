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

Beta.3 packages source `e370f3d84f1d` with unchanged accepted native runtime
(200 modules and 58 artifact inputs equal `8de43810`). The universal plugin was
built offline from the vendored corresponding-source archive using Rust 1.85.1.
The dependency audit reports zero advisories. Binary privacy/export/dependency
audit and actual ABI/load/panic-containment checks passed.

The default `./scripts/run_tests.sh` passed, including installed-package admission,
316 Python tests, Rust/Clippy, FFI allocation audits, portable and bridge checks,
and the complexity ratchet. All 353 maintained benchmark tests and 12 focused
packaging tests passed. The initial cleanup benchmark run exposed two removed
shared test/document dependencies and an inherited fake resource trace charged
incorrectly; those tooling fixtures were corrected without native calls or
relaxing scientific checks.

The staged package passed four licensed Stata 19/MP fixtures on Apple Silicon:
installation smoke, single-runtime repeatability, two-FE OLS against dummy-variable
OLS (Auto/LSMR/CMG, Fast/Low), and PPML against full-dummy Poisson with independent
sandwich covariance. A fresh local `net install` passed the three numerical
fixtures and selftest; all 31 installed files matched. Actual public OLS
marker-only Break/rollback passed on this binary at the pre-preparation checkpoint.

The binary SHA-256 is
`b9c5ff17712e11f897f1536c8c92d92ea84160f3271b34ccad785a37b482f807`;
the corresponding-source archive SHA-256 is
`55383fbf48b9496e1845a61688dc87a16bead4d43af025506a42ff9ff40825b8`.
[Package metadata](../dist/fereg-native.json) records the identity and caveats.
[Beta.3](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.3)
is published with 46 assets. A fresh installation from its public versioned URL
passed selftest and the three independent OLS/PPML fixtures; all 30 installed
package files matched the published package. After the fast-forward, a fresh `github install johannes-schmieder/fereg` also
passed selftest and all three numerical fixtures, and all 31 installed root-package
files matched. Both installation routes used isolated user ado directories.
GitHub Actions remain disabled.

## Completed cleanup

The completed programme is merged into main and beta.3 is published. Two clean
prototype worktrees were removed after their unmerged tips were preserved under
remote `archive/compact-cell-prototype-20261024` and
`archive/staged-cmg-prototype-20261024` tags. Their branches and the merged
Auto-routing branch were deleted locally/remotely; 17 merged local branches were
pruned in total, in addition to the two archived prototypes. Three unrelated
unmerged local branches remain preserved.

Task outputs, expendable Cargo/fuzz caches, mock build files and Python caches
were removed. `target` retains only the current replay baseline (about 16 MB).
Git history remains intact. No monitor is left for this completed workstream.
