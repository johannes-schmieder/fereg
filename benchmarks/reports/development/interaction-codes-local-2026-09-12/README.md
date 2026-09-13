# P3/P6 bounded categorical interaction construction

Adopt the shared bounded factorizer in [ADR 0046](../../../../docs/adr/0046-bounded-interaction-coding.md)
against main `9c2bcc5348c3f25dda70ab1340c3cbbdd48a089c`. This changes interaction
code construction in ordinary OLS clusters and the existing shared categorical
FE/cluster factorizer used by OLS and PPML. It does not change a solver region.
The installed plugin and unrelated Veneto source/evidence remain untouched.

## Coverage and construction cost

| Consumer | Baseline cost / existing reuse | Adopted change |
|---|---|---|
| Simple OLS cluster | Borrows authoritative final categorical codes | Unchanged; no copy or new cache |
| Ordinary OLS interacted cluster | N-row lexical comparison sort, then N output codes | Shared dense tuple mapping if bounded; exact old sort otherwise |
| Categorical OLS/PPML interacted cluster | Shared interaction factorizer, including exact validation | Same bounded helper through that existing factorizer |
| Interacted categorical FE | Shared construction/rebase/validation factorizer | Same bounded helper; repeated/canonical topology sharing remains |
| Covariance subset ordering | Existing order sharing between covariance and certificate | Unchanged; no persistent order cache |

For physical retained rows N, arity A and checked Cartesian level product P,
the dense route admits only `P <= u32::MAX` and `P*sizeof(u32) <= N*sizeof(usize)`.
Its table replaces the old row-order allocation, and the required output codes
temporarily hold exact mixed-radix tuple slots. A lexical scan assigns contiguous
codes only to occupied slots. It costs O(A N + P), versus O(A N log N) comparison
sorting, without concatenated strings, hashing or an N-by-A key matrix.
Reservations remain fallible/capacity checked. Failed planning allocates neither
table nor output and retains the old route; no speculative failed solve occurs.

In the first OLS primary, 980,065 retained rows and 97-by-3 cluster components
replace a 7,840,520-byte order with a 1,164-byte mapping. The 3,920,260-byte output
exists in both routes. These are source/shape-derived logical bytes and passes,
not measured hardware traffic. Generic component validation still precedes every
allocation; ordinary OLS borrows already validated final-sample codes. Original
row order, missingness/singletons, weight kind, lexical codes and all numerical
operations remain unchanged. Workers and Stata SPI are not involved in this helper.

## Complete-command evidence

The [prospective protocol](protocol.md) required either a 10% median paired
command gain or a repeatable 20 ms saving with preparation attribution.
There are 27 pairs / 54 measured commands: nine primary pairs, fifteen protected
pairs, one separate pilot and two separate public phase profiles. Each arm uses
a fresh Stata/MP 19 process and a small untimed warm-up on the local M2 Ultra.
Three primary seeds and arm order rotate. No other build/test workload ran during
timing. Native threads are distinct from the eight-processor Stata license limit;
macOS affinity is uncontrolled. This is local development evidence.

| Target, sparse missing | Original N / K / native threads | Command medians, seconds | Median paired gain |
|---|---|---:|---:|
| OLS PW, interacted cluster | 983040 / 10 / 4 | 0.439 → 0.393 | 10.5% |
| OLS FW, interacted FE and cluster | 245760 / 50 / 16 | 0.370 → 0.345 | 6.8% |
| PPML FW, interacted FE/cluster, offset | 122880 / 10 / 4 | 0.383 → 0.370 | 3.4% |

The two OLS targets save 38–54 and 24–26 ms across their respective three pairs;
both meet the stated criterion. Their separate traces locate the improvement:

| OLS trace | Preparation, seconds | Fit, seconds | Score-design certificate, seconds |
|---|---:|---:|---:|
| Interacted cluster, pilot | 0.14142 → 0.09985 | 0.11025 → 0.11366 | 0.01652 → 0.01700 |
| Interacted FE/cluster, profile | 0.05586 → 0.03394 | 0.11092 → 0.10857 | 0.02648 → 0.02595 |

These traces are separate from the primary medians. Input and outer work remain
similar; primary OLS native medians fall 0.31603→0.27390 and 0.26274→0.23828.
PPML's recorded preparation medians barely move (0.03716→0.03665), while unchanged
finite work also varies. Its total 3.4% gain does not independently meet the target
and is not wholly attributed to the preparation timer. Adoption rests on OLS;
PPML supplies affected-surface correctness and cost evidence. Independently
computed medians and nested phase scopes need not sum.

## Memory and protected costs

Primary native peaks are identical at 271,104,388 / 242,762,686 / 192,102,199 bytes,
within unchanged models of 424,674,248 / 420,761,928 / 217,146,702. Later solver and
inference lifetimes dominate these peaks. Primary sampled RSS medians are
0.4310→0.4302 / 0.4018→0.4039 / 0.1331→0.1299 GiB; no general RSS saving is claimed.
OLS scratch peaks are identical, including 4,900,480 bytes in the low/saved control.

Controls cover weak/dense/bottleneck/cycle/disconnected/unbalanced OLS graphs,
small/narrow, no FE, unchanged complete IID, exact table admission/decline, sparse
fallback, low-memory saved FE/residuals, PPML weak graphs and saved separation.
The largest command increase is unchanged IID 0.574→0.578 seconds (+4 ms/0.7%);
weak OLS is +1 ms/1.7%. None exceeds the prospective max(10 ms,5%) review envelope.
Single-pair controls are diagnostics, not repeatability claims.

All primary and non-graph control peaks fit their models. The six OLS graph
controls expose pre-existing accounting discrepancies: five have identical peaks
in both arms, about 0.4–9.5% above the 13,640,648-byte model. Weak OLS is
30,612,672→30,864,128 bytes against that same model. The 0.8% peak difference is
not localized; both are within the baseline range observed in the preceding
weak-graph experiments. No successful model-bound check is claimed for these
controls. The smaller interaction table does not resolve the broader P0 gap.
PPML weak peaks are 79,800,631→79,718,711, below their unchanged model.

## Scientific validation and identity

[Validation](validation.json) records 41 distinct focused Rust tests. They include
independent BTreeMap tuple coding across 60 shapes; row permutations; unused
levels; exact physical-byte boundaries; sparse/overflow fallback; malformed input
before allocation; reservation failure/undersize; original-sample singleton/FW
semantics; shared topology/rebase; generated columns; and weighted projection.
The categorical-module test filter selected zero tests and is explicitly not
counted as coverage. Its shared factorizer/topology tests and public oracle pass.

Both artifacts pass the independent [Stata/Mata oracle](../../../../tests/stata/development/interaction_codes_oracle.do)
with r(0) and the terminal sentinel. OLS uses explicit FE dummies for unit/AW/FW/PW,
full/deficient rank, numeric/string cluster interactions and both simple/interacted
FE syntax, with exact samples/order and full slope covariance checks. PPML uses
an independent full-dummy Poisson fit and Hessian/sandwich calculation, FW
multiplicity, PW squared scores, offsets and saved means. That PPML covariance
oracle uses the existing reported correction scalar; it is not a new independent
DoF oracle. All 27 paired comparisons have exact full scientific exports, sample
hashes, saved outputs where requested and non-time solver decisions. PPML retains
outer/inner counts, line searches, strict retries and fallback counts.

[Identities](identities.json) bind 150 baseline/measured native inputs and both
plugins; the accepted baseline matches the previous report's candidate inputs.
The baseline artifact has an older embedded build label, so source attribution
uses that checked input mapping rather than the label. The [measured patch](implementation.patch)
is immutable; [reviewed patch](reviewed-implementation.patch) adds accurate comments.
The reviewed rebuild is byte-identical to the measured plugin and passes
universal static and native arm64 ABI checks, as recorded in validation. No x86 runtime, SCC, Windows or release
qualification was run. An initial comment-only rebuild changed the binary hash;
reflowing the comments to retain executable line positions restored exact byte
identity. Both build outcomes are retained in validation.

## Replay and remaining work

[OLS receipts](ols/comparisons.json), [PPML receipts](ppml/comparisons.json) and their
numeric archives retain complete successful pairs, source/input fingerprints,
full numeric outputs, sampled memory, traces and per-run workers. Archives exclude
licensed logs, binaries and machine-specific paths. No failed timing attempt was
discarded. The earlier eight-test run overlaps the 38-test run and is not counted
twice; the empty test selection is retained in validation.

Create isolated baseline/candidate snapshots of the named commit and apply the
reviewed patch to the latter. Build from each snapshot's own working directory,
set `FEREG_LOCAL_BENCH_ROOT` to their parent, set `STATA_MP_PATH`, and place the
repository's `benchmarks/local_fixest_gap` on `PYTHONPATH`. The [paired driver](replay/paired.py)
accepts `--part pilot` or `--part primary`; [controls](replay/controls.py) and
[profiles](replay/profile.py) reproduce the remaining tiers. Run the oracle with
`scripts/run_stata_tests.py`, binding the actual plugin checksum. The
[collector](replay/collect.py) accepts the experiment root and output directory.

This addresses remaining interaction construction cost under P3/P6. A persistent
cluster-order cache is not adopted: simple codes already borrow final state and
covariance/certificate consumers share orders. Numerical reduction scheduling,
score reconstruction, generated finite-column evaluation and intercept-solver
admission remain separate tasks. In particular, this does not close P6's broader
syntax-versus-mathematical-structure routing item or the full #27 roadmap.
