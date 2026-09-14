# Positive-outcome PPML report capacity — local development

This source-bound checkpoint tightens separation-record capacity for strictly
positive outcomes. Parent source is `a1d0fd6` with plugin `9d920101`; candidate
universal plugin is `e1c72f56`. `source.patch` replays on that parent; `inputs.json`
binds 177 compiled inputs. Installed dist remains `4187efdf`.

The full German PPML call previously returned r(909) before pruning or estimation:
its pool-1 plan required 156.87 GB versus 130.50 GB available and 26.10 GB headroom.
The plan reserved 87.40 GB of method records plus 0.73 GB of generation hashes.
With positive y, no separation detector can delete a row. Initial recursive
singleton pruning reaches its fixed point in one commit; only the final replay
remains. Capacity therefore needs at most two generations (one with singletons
disabled), capped by N. Zero-containing samples keep the general N bound.

The post-classification raw-y scan retains one flag and polls interruption every
65,536 rows. Excluded nonpositive rows can only loosen admission. The prepared
coordinator independently bounds fallible record/hash reservations and appends.
All detector replays, row reasons, sample/weight semantics and certificates remain.
The FE-level bound and physical-memory headroom are unchanged. A possible
consecutive-FE-key-run bound was deferred rather than adopted.

## Evidence

351 PPML-related core tests, 97 final plugin tests, two constrained-memory tests,
universal static/ARM64 ABI tests, and 26 licensed native fits pass. Controls cover
one/two/three FE, tiny weighted mobility links, pools 1/2/10000, FW/PW, clustering
and missingness, initial and post-separation singleton cascades, and interruption
before admission. Existing constrained-memory expectations also failed on the
parent; they now account for the previously adopted low categorical projector.

The first plugin suite recorded one unexpected warmed-window allocation event.
The exact isolated repeat and full serial rerun passed without a source change.
This unresolved intermittent observation is retained in the evidence; it is not
an uninterrupted qualification pass. Initial test-fixture setup errors, a TSV
parser failure on space-padded Stata missing values, and resolved reruns are kept.

Twelve fresh-process paired commands use the same two German subsets, memory(low),
pool 4, threads 8, and the selected exp(logrwage2) outcome. Pair order alternates;
three pairs per subset. No own tests/builds/oracles overlap timing. An unrelated
project's test finished before these pairs began. Solver tracing is enabled in
both arms. These are local diagnostic timings, not frozen-suite release claims.

| Input / retained rows | Parent command median | Candidate | Native allocated peak, parent → candidate | Process RSS median, parent → candidate |
|---|---:|---:|---:|---:|
| 7,759 / 6,654 | 1.172 s | 1.246 s | 12.52 → 5.01 MB | 72.60 → 72.65 MB |
| 75,350 / 65,503 | 12.281 s | 12.307 s | 112.89 → 39.95 MB | 109.95 → 112.87 MB |

Preparation medians are 2.014→1.920 ms and 19.544→19.663 ms. Small command time
increases 6.31%, larger time 0.21%; three pairs do not establish these as systematic.
This change is adopted for admission/capacity, not a measured command speed gain.
Reserved/native allocated bytes are distinct from RSS; large untouched reservations
need not become resident. Scratch remains 0.96/9.43 MB; all fallbacks/retries are zero.

Masks/order, fitted means/absorbed predictor, b/V, statuses, DoF, objectives,
iteration counts, certificates and non-timing solver fields match exactly in all
six pairs. Pinned ppmlhdfe masks/order match; 18 slopes differ by at most 5.2e-15
and covariance by 1.26e-12, within registered envelopes. Candidate independent
score and SciPy LSMR model-space audits pass. Reference reported objectives remain
authoritative under ADR 0026; no new claim about reference reconstructed means.

A single candidate pool-1 check at 75,350 rows costs 28.694 s versus the pool-4
median 12.307 s. It passes the same independent checks. This supports trying
pool 4 on the full sample; it is not a paired pool-size speed claim.

Full 91,045,290-row admission-only probes confirm 68.74 GB for pool 1 and 88.41 GB
for pool 4. The measured pool-1 modeled saving is exactly 88,131,838,784 bytes.
The one-byte cap deliberately rejects both before workspace allocation, with zero
planner allocator growth and no public results. This does not by itself prove a
normal full fit. The first normal pool-4 retry stopped with r(3499) due to an
incompatible diagnostic receipt option; the second omits that harness option.
Full estimation is pending at this checkpoint. No retained/full PPML estimate is
claimed here. Full OLS success and earlier singleton receipts remain unchanged.

Raw licensed logs, data and estimates remain local. The root named in
`local-evidence-hashes.json` is under the system temporary directory; hashes bind
individual receipts. No SCC job, installed plugin replacement or push occurs.
