# P1: PPML preparation ownership, local development

Adopt the isolated change against `b19a1cfac038dcc6fbf015cfbfaf9f51022dafb4`.
Production PPML now retains a numeric row ledger and reports after generation-zero
admission, without constructing the compact outcome/source sample that its owner
immediately discarded. It consumes the loaded offset/exposure allocation through
stable compaction. Raw outcome/model sources and raw pweights remain available on
the original axis for separation rebuilds. Solver/session/reconstruction code,
scientific rules, and memory admission are unchanged.

[Identities](identities.json), [source manifests](candidate-source.json), and the
[implementation patch](implementation.patch) bind the measured universal plugins.
This is local Apple Silicon/Stata MP development evidence, with no SCC, reference
performance comparison, Windows run, push, or qualification. The installed/dist
plugin and separate Veneto work are preserved.

## Complete-command results

Nine public-command pairs use three seeds, rotated order, fresh processes and a
small warmup. Import, input-value hashing and extraction are outside the timer;
estimation, VCE and automatic publication are inside. Native requests are 4 or
16 threads; macOS controls placement. All primary scientific exports are exact.

| Case | N / K / threads | Command medians, seconds | Median paired gain | Preparation medians, seconds |
|---|---|---|---|---|
| Complete one FE, robust | 983040 / 50 / 16 | 15.395 → 14.408 | 6.4% | 1.615 → 0.880 |
| FW, exposure, sparse missing, one FE, robust | 245760 / 50 / 4 | 4.683 → 4.534 | 4.5% | 0.351 → 0.227 |
| PW, offset, sparse missing, one FE, two-way cluster | 245760 / 50 / 16 | 4.591 → 4.452 | 3.3% | 0.358 → 0.221 |

Paired percentages are medians of within-pair ratios, not ratios of independently
computed medians. Preparation improves 35–46%; inference is essentially unchanged.
The existing preparation timer combines numeric-owner preparation with IRLS
initialization; it is not a new isolated copy timer.
Every pair preserves outer/inner iterations, line searches, solver decisions,
rank/status, all b/V entries, DoF, sample hash, weight/cluster/separation tables,
independent native certificates and saved outputs where requested. No fallback
occurs. The primary one-FE fit takes three outer iterations; both weighted targets
take four. Full counts/timings remain in the numeric archive.

Ten protected pairs cover weak/dense/bottleneck/cycle/disconnected/unbalanced two
FE, small/narrow one FE, FW low memory, heavily restricted PW with an offset, and
saved FE/means/drop reasons after exact separation. All preserve science exactly.
The largest slowdown is 1 ms (disconnected and restricted). Small/narrow remains
27 ms. Low-memory FW falls 0.925 → 0.843 seconds. No control crosses the prospectively
recorded review threshold of max(10 ms, 5%). These are bounded controls, not an
exhaustive supported-surface performance claim.

The exploratory pilot is separate from all primary aggregates. Three additional
private `allocationaudit`/Stata-profiler pairs are diagnostics, not public command
comparators: their extra audit work raises total time substantially. See
[protocol](protocol.json), [all pairs](comparisons.json), [summary](summary.csv),
[profiler](stata-profiler.json), and [archive inventory](archive.json).

## Cost and coverage audit

| Stage | Actual coverage and cost | Disposition |
|---|---|---|
| SPI source loading | Shared loader batches unrestricted all-numeric sources in 65,536-row chunks; restrictions or strings use candidate-gated scalar reads. FW/PW remain raw. | Unchanged; all SPI stays on Stata's thread. |
| Adjustment loading | Separate scalar candidate-gated read into values plus missing mask. | Unchanged; batched adjustment loading remains P1 work. |
| Preflight | Borrowed source views and model-sized role table; allocation-free authoritative classification in original row order. | Preserved before every observation-sized preparation allocation. |
| Admitted numeric preparation | Second invocation of the same classifier/PPML visitor; row map and drop ledger, weight validation/canonical reports. Previously copied outcome and every registry source and then compared those copies with their source owner. | Removes the unused N-by-source copy and comparison pass. All source roles, FW/PW and VCE/FE forms enter this same selector. |
| Adjustment ownership | Already classified offset/exposure values consumed; stable forward compaction or fallible smaller allocation when incoming capacity exceeds candidate N. Exposure logged exactly once. Missing mask is released. | Adopted, including restricted samples and oversized input capacity. |
| Generation rebuilds | Owner retains raw original-axis outcome/model sources and initial raw pweights. Existing categorical preparation/rebasing constructs each generation. | Unchanged and still a copy/pass cost; a further ownership redesign must preserve original-row identities and sample-specific pweight normalization. |
| Worker use | OLS retains its earlier requested-team preparation. PPML selector remains serial; the existing solver teams are unchanged. | No new worker pool or SPI thread boundary. Post-admission parallel classification is a separate measured extension. |
| Ado preparation | `feppml`, `_fereg_ppml`, and `_fereg_estimate_ppml` each use `sortpreserve`. An unrestricted input gets a temporary completeness marker, `markout` over sources/weights/adjustment, then `count`; the result only asserts completeness in the loader. Parsing and TS materialization do not explicitly sort observations. | Source audit identifies removal candidates; no ado optimization is adopted here. Failure exits, TS metadata/order and offset/exposure rules need isolated checks before removal. Output `count`/drop-reason checks certify publication and must not be treated as redundant input work. |

For complete N983040/K50/one-FE inputs there are 51 model registry sources.
The removed compact outcome/source payload is **408,944,640 bytes** per preparation,
plus its source-copy and subsequent comparison reads. This is source-derived
logical payload, not measured hardware traffic. The common row classifier still
reads the sources twice, once before admission and once afterward. Weighted
preparation still materializes a temporary weight vector for the unchanged exact
canonicalization; raw pweights are retained separately for rebasing.

## Memory and scientific boundaries

Action native peak medians are 2.438452135 → 2.438450350 GB for the large complete
case, 0.614666170 → 0.612452545 GB for FW, and 0.626515999 → 0.624302304 GB for PW
(decimal GB). The weighted reductions are about 2.21 MB: the original adjustment
values/missing mask no longer remain live beside the canonical offset through the
fit. The complete-case overall peak is effectively unchanged. Candidate sampled
process-tree RSS medians are 1.888, 0.533 and 0.553 GiB respectively, versus 1.885,
0.625 and 0.639 GiB; sampled RSS is not a precise allocation bound.

Private allocation audits observe preparation peaks of 1.9821, 0.4986 and
0.5105 GB in the candidate, all below their unchanged modeled preparation peaks.
The preparation phase also includes subsequent IRLS initialization and the
initial retained linear session, whose peak can dominate the eliminated earlier
numeric copy. Therefore this experiment
does **not** claim a 409 MB reduction in the preparation phase maximum or overall
fit peak. All observed action peaks are below their admitted models. Preflight
remains conservative; no lowered memory threshold or expanded automatic route
is adopted.

The selector shares the preflight's authoritative row visitor, including deferred
unmasked nonfinite-domain and weight error precedence. It checks exact original,
candidate and active counts after admission. Cluster missingness remains before
singleton deletion, which the PPML coordinator owns. Owned adjustment capacity
cannot exceed the old candidate bound. All reservations remain fallible; the
memory-limit fixture still proves no preparation phase opens on failed admission.

[Validation](validation.json) records 136 focused Rust tests, including a new
capacity/pointer-reuse test and differential selection checks in the existing
numeric cases, plus four licensed Stata fixtures with explicit r(0) sentinels.
They include analytic Poisson means, offset/exposure equivalence, literal FW
expansion, PW rescaling/extreme scales, sample/cluster/rank/separation behavior,
saved outputs and transactional errors. Both universal builds pass static binary
audits and actual arm64 ABI/self-tests. The x86_64 slice was not executed.

One weighted attempt completed its command but failed result extraction because
the shared benchmark do-file was edited while Stata read it incrementally. It is
excluded and retained locally; both arms were rerun. The harness now snapshots and
hashes each run's do-file. [The exclusion](excluded-extraction.json) records the
cause without turning the failed attempt into successful timing evidence.

## Remaining work

P1 numeric ownership is adopted; batched adjustment/restricted-source loading,
ado removal, parallel post-admission classification, and generation copy reduction
remain separate experiments. P0 traffic/build counters remain incomplete.

For P4, the large complete candidate's first primary seed spends 3.585 seconds in
FE projection, 2.840 in finite solves and 5.184 in the strict-final scope; these
scopes overlap and must not be summed. Audit fresh one-FE RHS/norm/mean construction,
warm projection and final-Hessian work separately. Reuse reference scales and
current-weight generation rules; do not alter the separate reconstruction repair.
