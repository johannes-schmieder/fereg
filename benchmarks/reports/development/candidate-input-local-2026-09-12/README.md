# P1 candidate-run and mixed-source numeric loading

Adopt the bounded loading extension in [ADR 0042](../../../../docs/adr/0042-candidate-run-numeric-loading.md).
Baseline is `5411b9b4d37aab1a856b12431db84ecfce1c6f22`; the candidate changes
only shared input loading and PPML adjustment loading. It leaves classification,
preflight/materialization, solvers, inference and publication unchanged.
[Identities](identities.json) and the [native patch](implementation.patch) bind
isolated source snapshots and plugins. The installed/dist plugin was not replaced.

## Measurement

There are 29 completed pairs / 58 measured calls: nine primary pairs, 16
stratified controls, three complete-adjustment probe pairs and one separate
pilot. Each runs sequentially in a fresh Stata/MP 19 process after a small
untimed warm-up on the local M2 Ultra. Three primary seeds and arm order rotate;
native threads are distinct from the eight-processor Stata license limit.
macOS affinity is uncontrolled. The [protocol](protocol.json) predates timing.
These are local development findings, not new external-comparator or release
claims. Independently computed medians and overlapping phase scopes do not sum.

| Target | Original N / K / native threads | Command baseline → candidate, seconds | Median paired change |
|---|---|---:|---:|
| OLS AW, first half of rows, sparse missing | 983040 / 50 / 16 | 0.615 → 0.445 | −27.6% |
| OLS FW, string cluster, sparse missing | 245760 / 50 / 16 | 0.528 → 0.430 | −18.9% |
| PPML PW, first half of rows, offset/missing | 245760 / 50 / 16 | 1.643 → 1.539 | −6.3% |
| PPML PW, complete input, offset probe | 245760 / 50 / 16 | 2.959 → 2.967 | +0.5% |

The OLS input medians fall 0.3898→0.2170 and 0.2098→0.1128 seconds. Their
fit/inference medians remain 0.1432→0.1450 and 0.2852→0.2845 seconds; outer
cost remains near 0.083 and 0.032 seconds. The gains are located in loading.
The excluded pilot is 0.635→0.456 seconds and agrees with the primary direction.

PPML's preparation timer starts after input loading. Its restricted primary
preparation remains about 0.109 seconds; unchanged finite work varies from
0.443→0.398 seconds and inference 0.0375→0.0437. A separate input timer is not
available, so the total 6.3% gain must not be wholly attributed to loading.
The complete-adjustment probe changes by +14/−65/+27 ms across the three pairs,
with no reliable whole-command benefit. No standalone adjustment speedup is
claimed. Its arithmetic and allocation bounds agree exactly.

## Coverage and cost

| Loading branch | Mechanism and remaining costs |
|---|---|
| Unrestricted, all numeric | Existing bounded bulk reader; same numeric cells and missing classification |
| Large restricted, numeric or mixed | Existing candidate vector filled first; retained numeric runs batch, excluded rows skipped |
| Large input with string sources | Numeric peers batch; each string source preserves original encounter order and final recoding |
| Short/fragmented retained runs | Fewer than 64 rows use scalar reads without per-run batch metadata allocation |
| Small restricted/mixed input | Existing combined row traversal below 16,384 original rows |
| PPML offset/exposure | Same existing value/missing destinations, candidate-run reader; logging and domain checks remain later |

This removes repeated Rust/C boundary and scalar missing-status calls, not the
underlying SDK read of each retained numeric cell. For the contiguous AW primary,
53 numeric columns × 491,520 candidate rows still require 26,050,560 value reads;
eight bounded numeric batch calls replace per-cell scalar adapter calls. The
candidate marker still requires 983,040 reads. These are source/shape-derived
counts, not hardware counters. The numeric destinations are filled directly;
no packing, extra value-copy payload or new observation-sized buffer is added.
Fallible column/slice/FFI metadata is O(number of numeric columns). Existing
preparation capacity checks and PPML allocation-free preflight are unchanged.

## Memory, protected controls and science

Primary native peak medians are unchanged: 537,619,094 / 315,828,853 /
377,154,745 bytes against unchanged models of 1,663,324,488 / 418,795,848 /
434,160,326 bytes. Peaks are exact in 28/29 pairs; the remaining AW pair changes
533,694,838→537,619,094 bytes (+0.7%). Every observed action peak fits its model.
Primary sampled process RSS medians are 1.351→1.345 / 0.580→0.584 /
0.397→0.397 GiB. No general native-peak or RSS improvement is claimed.

All six OLS graph controls pass: weak, dense, bottleneck, cycle, disconnected
and unbalanced. Worst slowdown is weak robust, 217→225 ms (+8 ms / 3.7%);
dense changes 196→203 ms. Complete all-numeric OLS is 577→578 ms, and fragmented
restricted OLS 129→130 ms. Small/narrow OLS is 7→8 ms. OLS and PPML low-memory,
restricted and saved-output controls pass; PPML low-FW is 938→831 ms in its one
pair, which is not promoted to a repeatable speedup. No control exceeds the
predeclared max(10 ms, 5%) review envelope.

All 29 pairs have exact full b/V, sample hashes, status, DoF, weight/cluster
statistics, fit and certificate exports. PPML separation, convergence, line
searches and saved outputs also match exactly. Initial/final solver decisions,
iterations, strict retries and fallback counts remain unchanged; no new solver
attempt or discarded fallback is introduced. The shared loader never calls SPI
from workers.

[Validation](validation.json) records 43 focused plugin action tests, three
candidate licensed fixtures and the new oracle on baseline, all with explicit
r(0) and terminal sentinels. Unit tests check exact candidate coordinates,
63/64/65,536-row boundaries, excluded cells absent from SPI, shape errors,
interruptible excluded scans and string code identities. The large independent
Stata fixture covers all 27 missing codes, numeric/string clusters, AW/FW/PW
explicit-dummy OLS, Poisson slopes, offset/exposure equivalence, original order
and atomic invalid-weight/exposure failure. It checks domain failure codes;
it is not a separate oracle for the wording of first-row error messages.
Existing action tests cover memory-mode agreement and allocation/store failure.
Universal static binary audits and native arm64 ABI/self-tests pass; x86 runtime,
SCC, Windows and formal qualification were not run.

The initial PPML control setup stopped before estimation because an OLS case with
the same name had populated the shared input cache at a different width. PPML
control recipes now have distinct names; all four were run afterward. No measured
OLS pair was discarded or overwritten. The collector also now replaces only the
final arm suffix in paths, preserving case names containing `candidate`.
Initial unit-test compile/expectation corrections are retained in validation;
they do not represent weakened scientific checks.

## Replay and disposition

The [OLS](ols/comparisons.json) and [PPML](ppml/comparisons.json) summaries and
numeric archives contain every accepted pair, diagnostic export, input identity
and per-run worker copy. Numeric input columns are independently hashed inside
Stata; string clusters are deterministically derived from audited integer codes
outside timing, identically in both arms. Seeds, restrictions and the conversion
recipe are recorded. Licensed logs, binaries, local paths and synthetic datasets
stay outside Git.

Use the existing local harness with `FEREG_LOCAL_BENCH_ROOT` bound to isolated
`baseline`/`candidate` source trees and `STATA_MP_PATH` set:
`candidate_input_compare.py --part primary|controls|adjustment` (`pilot` is
optional; `ppml-controls` resumes just that subset). Collect with the existing
OLS/PPML collectors and primary names from that script.

P1's remaining loading investigation is adopted with gains on its wide OLS
targets. The adjustment-only performance hypothesis is unsupported. No raw input
checks or scientific contracts are relaxed. Full P0 construction/pass counters,
remaining P2/P3/P4 work, centered tiles, generated/output/low-memory expansion and
scheduling remain separate roadmap work.
