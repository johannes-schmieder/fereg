# Validation status

Johannes accepted the integrated runtime on 2026-10-07. Its final timing campaign
was closed incomplete by explicit decision; this is architectural acceptance,
not a performance pass or a universal speed claim. Historical timing was mixed.
The final incomplete screening included PPML without FE about 19–21% faster and
wide two-FE OLS about 27–36% slower than the fixed development baseline; these are
descriptive observations, not completed confirmation results.

The preceding integrated Mac evidence covered 375 scientific families, 98 holdouts
and 6,375 native commands at 1/2/4/7/8/9/15/16 workers, with zero recorded scientific
failures, exact fixed-thread repeats, exact unaffected results and stable discrete
decisions. Independent oracles, transaction checks and allocation checks passed.
The accepted native SHA-256 was
`20a9d3d9234fa9faed9338e384c6d3987a6c0d2371d2066242f96e1a1ffa6826`.
The compact acceptance receipt is recoverable at
`d83d4af1:bench/history/2026-10-07-m35-architectural-acceptance.json`.

The completed program retired 11,373 net production lines against its fixed
baseline, including dependency additions. Removing historical reports, plans and
builds does not increase that production-code result.

The single-runtime repository cleanup removes the old selector and packaging,
retains independent scientific witnesses and preserves the numerical Rust runtime.
The completed cleanup checks are recorded below. Old raw captures and
builds are deleted with Johannes's explicit authorization; ignored raw files are
not recoverable through Git. Historical tracked source and summaries remain there.

Qualification is local Mac only. GitHub tests remain paused. No new Linux/SCC,
Windows or native Intel-host Stata evidence is implied. Native distribution still
requires the separate [licensing decision](licensing.md).

## Single-runtime cleanup checks

Local validation on 2026-10-07 passed:

- Full `scripts/run_tests.sh`: formatting, Clippy, Rust workspace tests,
  isolated allocation/FFI audits, unchanged complexity ratchet and mock SPI.
- 219 Python tests, including independent recovered graph/Poisson witnesses,
  reference identity, packaging, parser and transaction-runner checks.
- 12 compact benchmark-tool tests and all four smoke models (28 commands),
  with coefficients, covariance, sample, scalars and output comparisons passing.
  This validates the tool; its timings are descriptive only.
- Universal Mac build, binary audit and real-plugin ABI/load/panic containment.
- All 64 licensed Stata integration fixtures: 63 passed in the full run; the
  remaining historical parity fixture passed after the repair described below.
- 73 native replay cases at 1/2/8 workers, passing independent science,
  exact fixed-thread repeats and stable discrete decisions.
- Three staged-installation fixtures, including removed-option rejection,
  missing-plugin failure and rollback, plus OLS and PPML interactive Break.
- Portable shell/Python/strict-C checks and documentation links.

The tested local plugin SHA-256 is
`14dd962c77d52b6e0f2622f10623dacac04457c190e28bcf874fb171c671f5ac`.
No numerical Rust runtime source changed. The sole Rust test edit relocates the
sealed weak-graph reference include; the reference bytes are unchanged.

The old interaction fixture assumed the legacy engine's final-sample names.
It now asserts the original 21 slots/base flags and the declared omission/rank,
then checks a fixed independently specified retained basis using Stata dummy
regression. Coefficients, VCE, residuals, RSS and DoF are checked. The nearly
singular four-way covariance determinant uses a cancellation-scaled 32-machine-
epsilon bound against the explicit determinant and Stata's calculation, while
preserving PSD checks. Native exact-arithmetic tests remain. The original failed
fixture run is not described as passing; only the repaired fixture passes.

Astra independently reviewed the final retirement at high effort and approved it
without blocking findings. The extracted Rust parser, complexity inventory and
sealed reference bytes were independently checked against their prior versions.
Old source-pin campaign guards were retired; retained native allocation tests
are not a claim of exhaustive detection of every possible new allocation.

The working tree previously occupied about 1.20 TiB, primarily generated target
artifacts. The authorized purge removes those builds/captures and old binaries,
retaining current source, tests, Git history and one current Mac plugin. Ignored
raw captures are intentionally discarded, not archived elsewhere.
