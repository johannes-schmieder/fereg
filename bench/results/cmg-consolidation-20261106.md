# fereg 0.5.1 CMG consolidation qualification

Release acceptance is pending. The fixed Linux and Mac schedules each passed 11
of 12 unique performance comparisons; `weak-cmg-t4` was inconclusive on both.
All measured scientific, fixed-worker repeatability and cross-worker checks
passed. No timing was excluded or repeated to obtain a pass.

The [structured evidence](cmg-consolidation-20261106.json) embeds the frozen plans,
input and oracle identities, every comparison, native checks, accounting records,
artifact provenance and operational failures. Companion
[Linux measurements](cmg-consolidation-20261106-linux-measurements.json) and
[Mac measurements](cmg-consolidation-20261106-macos-measurements.json) retain every
warmup and timed call, block order, score, scientific fingerprint and executed
solver code. They are derived benchmark records; raw process logs, private
paths and generated datasets are not committed.

## Scope and decision

The shared CMG revision is `a4c927f49eb7a5391a63963d2c4c651a51cf1af5`.
Relative to the prior fereg pin `5c5b5b2b138226ea928514fa9db27439ba1cffdb`,
retained numerical code is unchanged except for deletion of unreachable portfolio
helpers. No eligible family changes arithmetic, storage, routing, admission or
stopping. The declared scientific scope is exact; no new hidden/public route or
derived residual row is activated. Version and dependency provenance change.

The predeclared panel uses weak and long two-FE designs with 100,000 observations,
5,000 worker levels, 1,000 firm levels, two regressors, robust inference and seed
20261106. It covers Auto, explicit CMG and explicit CMG-Schur degree 5 at one and
four workers. Traces confirm explicit CMG code 7 and CMG-Schur code 8; Auto selects
CMG-Schur in these cells. Independent witnesses, fixed-worker repeats and
cross-worker scientific/discrete checks passed before timing. This representative
OLS panel does not establish full routing coverage or PPML performance; PPML has
separate functional evidence below.

The unchanged protection margin is `max(5% of reference time, 1 ms)`. Original
and incumbent both name the published 0.5.0 plugin, so each report's 24 rows
represent 12 unique comparisons. The report label `smoke_complete` means that the
representative schedule completed; it is not a performance pass.

| Platform, analysis unit | weak-cmg-t4 median ratio | Median excess (s) | Lower bound (s) | Upper bound (s) | Decision |
| --- | ---: | ---: | ---: | ---: | --- |
| Linux, six physical hosts | 1.007065732780525 | -0.10707487401039908 | -0.11763466910395658 | 0.1145468974056516 | Inconclusive |
| Mac, six mirrored blocks | 1.0111249412522936 | -0.03912817206126348 | -0.06261055980179814 | 0.011138773233392907 | Inconclusive |

These are one-sided .025 bounds on excess seconds **after** the allowance, not
ratio confidence intervals. They are conditional on independent unit scores with
a common median; there is no simultaneous interval claim. Neither platform
classified a regression, but the positive upper bounds prevent acceptance under
the frozen rule. Every favorable and adverse unit score remains in the record.

Linux used six independently observed whole-node allocations, each requesting
`-pe omp28 28 -l num_proc=28`, a fixed 15-minute distinct-host reservation barrier,
two mirrored blocks and one warmup per arm. Array 8484075 tasks 1–6 ran on scc-we4,
scc-ul3, scc-um2, scc-xb3, scc-xf2 and scc-xc1. All 720 calls completed; all six
accounting records have `failed=0` and `exit_status=0`. Isolation checks and
scientific checks passed. Report job 8484076 deliberately exited 1 because its
all-comparisons acceptance guard found the inconclusive cell. There was no Linux
retry or exclusion allowance.

Mac used one host, six mirrored blocks and one warmup per arm: 312 completed
calls. The frozen external-interference policy allowed at most four contaminated
cells one retry each, with a 416-call cap and sealed observer evidence. It was
not used: zero retries, zero discarded attempts, zero host/scientific errors.
Ordinary desktop activity remained measurement noise.

[AGENTS.md](../../AGENTS.md) requires: “Never sample until a pass, discard
unfavorable timings, or silently replace an inconclusive candidate.” It also
requires discussion of inconclusive decisions with Johannes. Both outcomes are
retained pending that decision. No successor candidate or additional timing
schedule is implied by this report.

## Source and native evidence

Runtime source `0d0c9e0da3df9da166ab2a8683a4d29b00334d93` contains the 0.5.1
metadata, consolidated CMG pin and exact vendor provenance verification. Source
`3650b7e8726fed7613993c85807a44550f901885` changes only two pinned input SHA256
strings in `tests/reference/ppml_bias_replay.json`, after proving equal-length
0.5.0-to-0.5.1 version-token-only changes. Independent bias targets are unchanged.
Both commits have maintained runtime identity
`59c0fcd44d8f142a9148135aa15ad8776e4d88dbdfc964cb48510420e7f412d9`.
This report does not relabel any binary's actual build source.

- Source 0d: the source-only gate passed 349 Python tests and 126 Rust result
  groups, totaling 2,289 passes and 15 ignored results across the configured runs;
  formatting, Clippy, fuzz compilation, static checks, complexity and mock guards
  passed. Fourteen focused packaging tests passed. Source 365's five focused bias
  reference tests passed.
- Mac: the final source-365 universal plugin passed native dependency/export
  audit and ABI/selftest guards, then 77 independent replay cases with exact
  incumbent agreement, fixed-worker repeats and two/four-worker science checks.
  The runtime-equivalent source-0d plugin passed 74 licensed Stata fixtures and
  live Break checks for public OLS, public PPML and the four-worker CMG builder.
- Linux: build job 8483872 passed the source-365 native audit and ABI guards.
  Smoke job 8484018 passed two licensed fixtures and the 48-call science-only
  panel. Job 8484019 passed 77 independent replay cases and five affected
  OLS/PPML Stata fixtures. Job 8484151 installed the exact staged archive and
  passed three installation/binary fixtures, bound to its archive and plugin
  checksums. All these completed jobs have successful accounting.
- Windows: private run `win-20261106T211158Z-c97d3578` passed build, ABI, installed
  package and full licensed native driver at source 0d. Cleanup and stopped
  instance were verified. Its receipt SHA256 is
  `bf142b48c3af6140997e9dd30f22f48aae8fb6c82098ee1591b4de7478198e23`.

Native Intel-host Stata testing remains outstanding; the Mac artifact includes
both processor slices. The Linux replay establishes independent scientific
agreement; its separate comparison panel establishes exact incumbent agreement
and repeat/cross-worker behavior for the affected representative paths.

## Artifact and packaging identities

| Artifact | Actual source | SHA256 |
| --- | --- | --- |
| macOS universal plugin | 3650b7e8 | `f24caeb662babf10e4a742cde770649948e690f66c7c865a5e3fe6267df717bd` |
| Linux x86-64 plugin | 3650b7e8 | `ca2ecf79dc435c92821ae1d377c1eca9106c04e55108233dc635c3b8c3f2e237` |
| Windows x86-64 plugin | 0d0c9e0d | `bca90f29b645b10d2ec47ad5aaf71531c1ac3fc1ba2bebea813d592f09278500` |
| Staged native archive | Packaging 3650b7e8; sources above | `1d1419b61b1690170bd8a95b083b50048c8ab1392b18df5c02c1e38e2fe61260` |
| Corresponding source archive | 3650b7e8 | `81e345314c8cf405c8b15460d11562e0b12c3417dfea0fdce830575db78658c6` |

The source archive includes the complete vendored Cargo closure and relocatable
offline configuration. Exact CMG revision, Cargo-normalized source inventory,
vendor bytes and upstream MIT license were verified; no license workaround is
used. Cargo audit 0.22.1 with a refreshed advisory database found zero advisories.
Rust 1.85.1 notices, third-party licenses and SBOM are included. A release plugin
build from the extracted archive passed offline/locked with an empty Cargo home.
The native archive embeds the actual source per platform, and static extraction
validation and Linux installed-byte checks passed.

These are prepared artifacts, not published release assets. Acceptance of the
performance uncertainty, any remaining exact staged-package platform checks and
the final main/tag/HTTP/github-install gates remain with the release lead. Old
release archives remain immutable; GitHub Actions remain paused.

## Preserved failures and bounded repairs

The initial Mac replay failed because two pinned hashes still described the old
version header. The source-365 test-only correction and final replay are recorded
above; no numerical tolerance or target changed. The first offline build ran from
the wrong directory, preventing Cargo from discovering the vendored configuration;
the same archive then built from its extracted source root.

SCC smoke 8483892 failed before application calls because its wrapper compared a
whitespace-padded accounting exit code to literal zero. Trimming that field was
the only repair; smoke 8484018 passed. Its doomed dependent 8483893 was canceled
before start and replaced by 8484019. These failed/canceled attempts remain in
the evidence alongside successful accounting.

The first Mac timing invocation was blocked by the sandbox's denial of `ps` to
the process observer. It started **zero native calls** and ran for
0.09367750000092201 seconds. Its blocked status, error and host record are
preserved. The same frozen plan then ran once with process-observation permission.
This repaired observer access without excluding any timing. The Linux and Mac
inconclusive outcomes received no repair, resampling or reinterpretation as passes.
