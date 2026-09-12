# P3 OLS inference ownership: adopted local development checkpoint

Adopt the ordinary OLS inference policy in [ADR 0043](../../../../docs/adr/0043-ols-inference-strided-cache-policy.md):
pack candidate score values up to 32 MiB, then use the existing retained strided
route where the finite solve permits it. Shared weighted-linear/PPML policy,
explicit cache-limit overrides, IID, low memory, rank and memory admission remain
unchanged. This selects existing stable reference/residual machinery as well as
avoiding a packed copy; it is not an isolated covariance-kernel improvement.

The native baseline is `f4cebe71bb8afcd29e5acb2e97a1d1875a340ba7`. The subsequent
`02005d5` commit archives the [preliminary experiment](preliminary/README.md)
without changing native source. The [final source/artifact identities](adoption/identities.json)
bind 150 native inputs, the final patch and both isolated plugins. Reviewing the
comments and rebuilding produces **the same candidate plugin bytes** as the
prototype: `dc327ec0cfe16123e200cfb2a88e18fffa3e25b287c0f78e0375ecb48409d0d9`.
Baseline is `c1c9d8d15fe2586028c82b1c28b3b63a5bd489f68fef6f05d57d5cbba9f00f83`.
Installed/dist remains untouched. No historical receipt is rewritten.

## Command, phase and memory results

There are **29 completed pairs / 58 measured calls**: nine primary pairs,
sixteen protected controls, three separately traced QR pairs and one traced
pilot. Primary cases rotate three seeds and arm order. Each uses a fresh local
Stata/MP 19 process after a small untimed warm-up on the M2 Ultra. Native threads
are distinct from Stata's eight-processor license limit. Affinity is uncontrolled.
These are development findings, not SCC, external-comparator or release claims.

| Target | N / K / native threads | Command medians, seconds | Median paired change | Native peak reduction |
|---|---|---:|---:|---:|
| Robust, sparse missing | 983040 / 50 / 16 | 1.312 → 0.803 | −38.8% | 25.7% |
| PW robust, complete | 245760 / 50 / 16 | 0.368 → 0.253 | −31.4% | 25.5% |
| FW cluster, sparse missing | 245760 / 50 / 4 | 0.539 → 0.409 | −24.1% | 25.2% |
| Rank-deficient QR (profiled) | 98304 / 50 / 4 | 1.507 → 1.447 | −4.0% | 25.5% |

Primary native peak medians fall 1,245,142,607→925,443,580,
313,492,811→233,452,576 and 315,828,839→236,091,748 bytes. Every measured action
peak fits its unchanged model. Full modeled/native/scratch/RSS values are retained
in the [preliminary summary](preliminary/summary.csv) and
[adoption controls](adoption/summary.csv); native allocation reduction is not
equated with a portable process-RSS reduction.

The separate pilot trace locates the complete-route savings. Stable raw moments
fall 0.1214→0.0202 seconds and bread/raw-residual construction 0.4119→0.0103.
The scope containing packed-cache construction falls 0.0521→0.000002.
Strided score-design certification costs **more**, 0.0564→0.1019, while covariance
formation stays near 0.094 seconds. The raw moment cache preserves stable scales
and means; the residual builder reuses those values and existing row-major
operations. No subtraction of large uncentered cross-products is introduced.
Phase medians and overlapping scopes must not be summed.

At N983040/K50 the packed score payload is approximately 393 MB before missing
rows are removed. The candidate retains the already allocated solve matrix,
including its row stride and scales, instead of allocating that additional
packed value array. These are source/shape-derived payload counts, not measured
hardware traffic. Inference accesses the retained current-frame values through
the established score view; it still performs the required robust-meat arithmetic.

## QR and protected paths

Both rank-deficient QR arms report `reabsorbed=1`. A packed cache carrying the
original candidate column identities also becomes unusable after rank reduction,
so the baseline already reconstructs scores. The measured reconstruction cost is
about 0.12 seconds in both arms; the new policy does not eliminate it. All three
complete QR commands improve, including that work and all publication checks.

The newly eligible weak graph improves 0.459→0.411 seconds. Its native peak is
essentially unchanged because earlier solver work dominates. The six graph-family
controls preserve solver decisions and science. Largest absolute control slowdown
is complete IID 571→583 ms (+2.1%); largest percentage is small/narrow 6→7 ms.
Both stay below the predeclared max(10 ms, 5%) review threshold. Low-memory,
fragmented-restriction and saved-output controls pass.

Three additional single-seed controls close the adoption coverage review:

| Newly eligible control | N / K / native threads | Complete command, seconds | Native peak, bytes |
|---|---|---:|---:|
| No FE, robust | 245760 / 50 / 16 | 0.415 → 0.297 | 305118720 → 208739589 |
| One FE, 65 controls, robust | 98304 / 65 / 4 | 0.346 → 0.327 | 160049622 → 118490756 |
| Interacted FE, two-way cluster | 245760 / 50 / 16 | 0.500 → 0.388 | 321918121 → 239927786 |

These supplement the repeated primary evidence; their single-pair gains are not
promoted to repeatability claims. Their [numeric archive](adoption/numeric-evidence.zip)
is separate from the unchanged preliminary archive.

## Scientific validation and disposition

All 29 comparisons preserve exact samples, coefficient status, DoF, FE/cluster
tables and solver decisions. Full b/V maximum absolute differences are
3.34e-16 / 7.20e-21; original rank, score, bread, covariance and FE certificates
pass. Saved outputs agree under the unchanged registered tolerances. No
convergence tolerance, omission rule, weight semantics, PSD policy or public
transaction is changed. The original pilot's overly broad exact assertion on
floating certificate diagnostics is disclosed in the preliminary validation;
actual decision fields remain exact and numerical checks were not weakened.

[Validation](adoption/validation.json) records eighteen distinct focused Rust
tests across strided ownership, robust and clustered science/allocation failures.
The new [licensed oracle](../../../../tests/stata/development/ols_inference_ownership.do)
passes baseline and candidate with explicit r(0) sentinels: full slope covariance
against Stata `regress` with FE dummies at N98304/K50, unit/AW/FW/PW,
robust/clustered and full/deficient rank. The tests retain independent
frequency-replication and covariance checks. Universal static binary audits and
native arm64 ABI/self-tests pass. No full scientific profile, x86 runtime, SCC,
Windows or qualification ran.

The 32 MiB policy is adopted for local development. It is a measured crossover,
not a universal optimum or a new numerical family. This closes #27's packed/
strided lifetime comparison. Cluster-state reuse, tiled multi-consumer scores,
skew scheduling and low-memory duplicate work remain separate P3 investigations.
P0/P2/P4–P7 and the later research backlog remain open.

Replay the scripts in `preliminary/` with the existing `benchmarks/local_fixest_gap`
modules on `PYTHONPATH`, `FEREG_LOCAL_BENCH_ROOT` pointing to the isolated source
trees and `STATA_MP_PATH` set. The supplementary `adoption/adoption_compare.py`
also imports the archived preliminary `compare.py`. All input recipes/hashes,
worker copies, full numeric exports, phase probes and failed-attempt disclosures
are retained. Licensed logs, binaries, local machine paths and datasets stay
outside Git.
