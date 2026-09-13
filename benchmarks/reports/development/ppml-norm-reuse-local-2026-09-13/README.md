# P4: reuse certified current-frame PPML projection norms

Adopt [ADR 0052](../../../../docs/adr/0052-ppml-same-call-projection-norms.md)
against repaired native `6b7d0fea6ae556caba55aefbe07a13794183796f`.
The earlier stale-norm behavior is not a permissible performance baseline.
This change removes redundant norm work while preserving that repair.

## Ownership, coverage and pass cost

The fused fast one-FE PPML projector already computes stable current-weight
column norms to certify the values it returns. A narrow same-call entry now
copies the trailing control norms into the session's existing retained buffer.
It never exposes an independently callable last-result accessor or takes the
projector's norm vector away from its retained workspace.

Every successful one-FE return, including an initial iteration-zero warm
certificate, has workspace norms for the exact returned matrix. The one-FE
route has no multi-FE accelerator/LSMR handoff. Reuse requires the linear frame's
solver divisor to equal MAP's maximum raw weight. Unsupported multi-FE returns
and deliberately different frame divisors leave the output buffer untouched;
the existing stable current-frame matrix scan remains authoritative there.
Projection errors also leave the output norms untouched. The surrounding session
retains its existing invalidation and strict transaction boundaries.

Each eligible fused refresh removes an N-by-K value read and its per-value
scaled-square arithmetic, adding one N-weight maximum check and K copied norms.
At N983040/K50 these nominal payloads are 393,216,000 value bytes avoided versus
7,864,320 weight bytes read. They are source-derived per-call payloads, not
measured hardware traffic or totals over the command. No observation-sized
buffer, worker team, reduction partition or memory-admission change is added.

Raw reference norms and means still refresh, warm candidates still receive the
useful initial certificate, and strict rank/score checks use the current frame.
Fresh raw-input certificate removal and fresh-design compaction norms remain
separate P4 work. Ordinary two/three-FE joint LSMR, generic/slope/low-memory
projection, saved recovery and one-shot OLS retain their implementations.

## Complete-command measurements

The [prospective protocol](protocol.md) required at least 3% and 10 ms median
paired command savings on a primary, with no unexplained protected increase
above max(10 ms,5%). The initial three-target screen passed, authorizing the
predeclared two further seeds for all three targets. All observations remain:
**15 pairs / 30 commands**, comprising nine public primary pairs, five controls
and one separately reported private allocation-audit pair.

Fresh local Stata/MP 19 processes on the M2 Ultra use isolated artifacts, a small
untimed warm-up and rotated arm order. Native workers are distinct from Stata's
license processor cap. No builds/tests run during timing; affinity is uncontrolled.
These are local development results, not release or comparator claims.

| Public target | N / K / workers | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 10.317 → 9.689 | −8.5% |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 3.380 → 3.231 | −4.5% |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 3.035 → 2.933 | −4.5% |

Every primary pair improves. Median paired changes need not equal the ratio
of the displayed independently computed medians. The large first-screen pair
is 11.563→9.846 s and has a sizeable unmodified preparation difference,
1.5569→0.8592 s. It is retained, not attributed to the norm optimization or
excluded as an outlier. Its two repeat pairs are 10.317→9.443 and 10.164→9.689 s.
The bounded result is not a claim that every millisecond saved has one cause.

Across the three seeds, projection medians fall 1.3970→1.1370, 0.5614→0.4707
and 0.4545→0.3665 s. Strict/final scopes fall 3.7174→3.3637,
0.9815→0.9270 and 0.8986→0.8468 s. Median preparation and inference remain
close. These scopes overlap and must not be added. The separate private audit
falls 8.661→8.027 s (−7.3%); it is excluded from public command aggregates.

All fifteen native peaks are exactly unchanged and within unchanged models.
Primary peaks remain 2522.61, 633.49 and 645.34 MB against 2962.95, 745.43 and
935.51 MB models. Sampled RSS moves both ways: primary medians are
1.9591→1.9596, 0.5253→0.5294 and 0.5630→0.5438 GiB. Explicit MAP rises
0.0617→0.0916 GiB. No resident-memory saving is claimed.

The protected command differences are weak two FE −2 ms, low FW −28 ms,
small one FE −1 ms, saved/separated −2 ms and explicit strong two-FE MAP
+5 ms (+6.0%, below the 10 ms absolute review threshold). These single pairs
are controls, not causal speed claims. All audit allocation events and live/
peak growth remain zero; retained capacities, fresh/warm equivalence and the
allocation-free certificates pass in both arms.

## Scientific and source evidence

All fifteen full b/V exports are byte-identical. Samples, rank/status, DoF,
weight/cluster/separation structure and discrete solver decisions match.
Independent runtime certificates and saved-output tolerances pass. Complete
phase, retry/fallback, iteration and memory records are in
[comparisons](comparisons.json) and the [checksummed numeric archive](archive.json).

[Validation](validation.json) records 23 distinct candidate tests in a separate
candidate-only Cargo target. New tests cover changing weights, extreme column
scales, RHS width, empty trailing outputs, iteration-zero warm returns, capacity
retention, failure nonpublication, multi-FE handoffs and mismatched divisors.
An independent hypot accumulation checks the returned norms. The closed-form
stale-rank regression now exercises both matching and alternative divisors:
strict staging rejects the obsolete prior basis and exposes no current output;
explicit basis refresh matches the fresh rank-zero solution.

Both artifacts pass the same independent full-dummy Poisson/Hessian/sandwich
Stata oracle for unit/FW/PW with robust/clustered inference and offsets.
Universal static audits and actual native arm64 ABI/self-tests pass. Initial
compilation supplied the scale struct instead of its divisor field; that was
corrected before successful tests, artifact construction or measurement. No
scientific expectation or tolerance was relaxed.

[Identities](identities.json) bind all 152 native inputs and the
[implementation patch](implementation.patch). Candidate plugin SHA-256 is
`a4d4a33aeb8ac6a0c870f38bb86b57d1b10cd81386c004dccb0970b1a992cfee`;
repaired baseline is `313e85aa7ccd298f8333e9dc3c3845ab963b38bbfcf37964a88a2944c06687e6`.
Changed source mtimes were refreshed before using the release build cache;
both architectures visibly compiled the changed core. Main and isolated native
source hashes match the measured candidate. The installed plugin is unchanged.

For replay, place `compare.py`, `collect_results.py` and `run_oracles.py` in an
isolated experiment root containing baseline/candidate source and artifacts.
Set `FEREG_LOCAL_BENCH_ROOT`, `PYTHONPATH` to `benchmarks/local_fixest_gap`, and
`STATA_MP_PATH`; use the retained protocol and source hashes. Licensed logs,
local paths, binaries and data remain outside Git. Historical receipts and
separate Veneto work are intact; no push, agents, cluster work or qualification
occurred. The combined P4 fresh-input/norm item remains open until fresh-input
checks are evaluated; current-frame fused one-FE norm reuse is adopted.
