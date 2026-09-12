# P4: One-FE PPML retained projection, local development

Adopt the isolated extension against `0889721a7c781618dd9f132642a85d065e34afad`.
Fast PPML with one categorical intercept effect now uses the existing reusable
categorical projector and fused changing-response/retained-regressor projection.
The first group sweep is independently certified; already-within candidates can
still certify before a sweep. One FE allocates no accelerator matrices. No
numerical tolerance, rank policy, solver family, sample/weight/separation rule,
memory admission, reconstruction policy or publication contract changes.

[ADR 0039](../../../../docs/adr/0039-ppml-one-fe-retained-projection.md),
[cost audit](cost-audit.md), [identities](identities.json), source manifests and
[frozen implementation patch](implementation.patch) bind the measured plugins.
A later [test-only refinement](post-build-test-refinement.patch) adds narrow
workspace assertions; executable production source is identical to the measured
snapshot. This is local Apple Silicon/Stata MP development evidence, without
SCC, reference speed comparisons, Windows testing or qualification. The installed
plugin and separate Veneto work remain untouched.

## Complete-command evidence

Nine primary pairs use three seeds, rotated order, fresh processes and a small
warmup. Import, value-hash validation and extraction are outside the complete
estimation timer. Native thread requests are 4 or 16; macOS controls placement.
All paired scientific gates pass.

| Case | N / K / threads | Command medians, seconds | Median paired gain | FE-projection medians, seconds | Strict/final medians, seconds |
|---|---|---|---|---|---|
| Complete one FE, robust | 983040 / 50 / 16 | 14.027 → 11.724 | 16.3% | 3.555 → 2.001 | 5.046 → 4.281 |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 4.422 → 3.708 | 16.0% | 1.201 → 0.730 | 1.292 → 1.102 |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 4.223 → 3.572 | 15.7% | 1.203 → 0.681 | 1.243 → 1.078 |

Percentages are medians of within-pair ratios, not ratios of independent medians.
Preparation remains about 0.84/0.22/0.22 seconds. Finite-solve medians are
2.667→2.642, 0.925→0.888 and 0.854→0.881; inference is 0.105→0.111,
0.098→0.097 and 0.037→0.037 seconds. Thus the gain comes mainly from projection
and strict/final work, with the PW finite-solve increase retained in the totals.
Timers overlap and medians need not sum.

The complete case retains three outer/five reported inner iterations; weighted
cases retain four/six. All public primary pairs have zero step halvings, strict
profile rejections, retries and fallbacks. Full solver setup, solve and
certification times, frame counts and work diagnostics are in
[work counters](work-counters.json) and the numeric archive. No separate complete
construction/discard count or hardware-traffic measurement is claimed.

The MAP solver table records fewer actual iterations: **5→3** for the complete
case and **6→4** for both weighted cases. The runtime's reported inner count
charges at least one per solve, including zero-iteration certified warm solves;
it therefore stays unchanged. Private audit MAP totals fall 23→4/5/5. These
different diagnostics are retained rather than forcing their counts to agree.

Ten protected pairs cover weak/dense/bottleneck/cycle/disconnected/unbalanced
two-FE graphs, small/narrow one FE, restricted PW, FW low memory and saved FE/means/
drop reasons after exact separation. Unaffected graph and low-memory routes retain
exact science. Worst slowdown is cycle **57→64 ms (+12.3%, +7 ms)**, below the
prospectively chosen max(10 ms, 5%) review threshold. Weak is 137→139 ms, unbalanced
61→65 ms and dense 57→58 ms. Small/narrow is 36→30 ms, restricted 68→60 ms and saved
separation 32→29 ms. Low memory is 1.020→0.919 seconds, but its implementation is
unchanged, so this single-pair movement is not attributed to the extension.

The 14.268→11.741-second pilot is excluded from primary aggregates. Three private
allocation-audit/profile pairs deliberately prolong the iteration window; their
49.529→41.282, 13.072→10.818 and 12.890→10.665-second totals are diagnostic rather
than public-command performance estimates. See [protocol](protocol.json),
[all pairs](comparisons.json), [summary](summary.csv) and [archive](archive.json).

## Memory and operational limits

| Primary case | Native action peak, decimal GB | Change | Unchanged public admitted model, GB | Sampled process-tree RSS, GiB |
|---|---|---|---|---|
| Complete | 2.438450 → 2.522611 | +3.45%, 84.16 MB | 2.962954 | 1.885 → 1.958 |
| FW | 0.612453 → 0.633492 | +3.44%, 21.04 MB | 0.745433 | 0.504 → 0.526 |
| PW | 0.624302 → 0.645342 | +3.37%, 21.04 MB | 0.935507 | 0.551 → 0.544 |

The extra group/RHS workspace is already charged by the fast memory model. All
observed action peaks in these pairs stay inside the selected bound; private
phase probes also stay inside every corresponding modeled phase. RSS samples
measure the process tree and are distinct from allocator accounting; their
movement need not match native allocations.

The [allocation audit](allocation-audit.json) preserves an existing limitation:
both arms record **16 allocation events in 16 audited iterations**, zero live or
peak growth, stable PPML/linear/scratch capacities, and passing fresh/warm and
retained-session reuse checks. Both whole-iteration allocation-free certificates
are **false**. Event sizes and their call sites were not isolated here; no
allocation-free whole-iteration claim is made. The passing mock allocator unit
test does not replace these real allocator observations. The audit's warmed
iteration medians improve 1.982→1.637, 0.509→0.415 and 0.505→0.413 seconds.

## Scientific validation and remaining work

[Validation](validation.json) records 140 focused Rust tests and five licensed
fixtures with explicit r(0) sentinels, plus five profile-registration checks.
The new Rust oracle compares weighted residuals with direct group means across
1/4/16 workers, changing weight frames and RHS widths, verifies exact warm reuse
and fixed capacities, and excludes accelerator state at narrow widths.

The new licensed fixture independently fits full-dummy Stata Poisson models and
forms their full Hessian and robust/clustered sandwich in Mata. It checks finite
coefficients, predicted means and covariance for unit/FW/PW weights. Existing
analytic Poisson, literal frequency expansion, pweight rescaling, separation,
rank, memory failure, rollback and transactional-output tests remain passing.
The fixture is registered for the existing manual scientific/full profiles;
those full profiles were not run.

Across all 23 pairs, samples, statuses/rank, DoF, weight/cluster/separation tables,
receipt identities and solver family/engine/frame/retry/fallback decisions are
exact. Reported outer/inner counts are unchanged; actual MAP iteration totals
decrease as detailed above. Maximum
absolute b difference is 5.99e-17 and V difference 1.28e-20. All independent native
certificates pass and saved outputs satisfy the registered comparison tolerances.
Roundoff-level numerical differences are not reported as exact equality.
See [scientific summary](science-summary.json).

Setup failures are retained in validation: initial Cargo path resolution, two new
Mata fixture syntax/dimension errors, and accidental inclusion of solver timing
columns in an exact comparator. Corrected fixtures pass, and every final pair was
revalidated. No timing pair was silently replaced. Universal binary audits and
the native arm64 ABI/panic-containment harness pass; x86_64 was built and statically
audited but not executed.

Remaining P4 work includes raw RHS construction, stable reference norm/mean
passes, warm stride movement and the unexplained per-iteration allocation event.
Skipping fresh-input checks, broader norm reuse and centered tiles remain
hypotheses. Two/three-FE LSMR, weighted Schur applications, low-memory/slopes,
inference ownership and broader generated/output paths need separate evidence.
