# P4: stable reference passes and frozen-solve scratch

Adopt the two separately isolated changes against committed main
`d32f05e45519aaee5c5d44bc87caddb4f68482f3`:

1. Reuse the existing identity scratch bank in frozen finite solves. This removes
   one needless allocation per warmed solve without changing arithmetic.
2. Schedule current-weight raw reference norms and means by column for fast
   one-categorical-FE PPML with at least 65,536 rows, eight stored columns and
   more than one installed native worker. Preserve original row order within
   each column, scaled norm accumulation and compensated means.

These are local Apple Silicon development findings. No installed plugin was
replaced, no SCC work ran, and no reference-speed, release or cross-platform
claim is made. [Identities](identities.json) bind all three isolated universal
plugins, source hashes, patches, build IDs, dependencies and worker scripts.
The [combined implementation](implementation.patch), [allocation-only patch](allocation.patch)
and [scheduling-only patch](norm-only.patch) separate their effects.

## Complete-command evidence

The scheduling comparison uses the allocation-only plugin as its baseline.
Nine public primary pairs use three seeds and rotated arm order. Command
medians and median paired changes are computed independently.

| Public PPML case | N / K / native threads | Baseline seconds | Candidate seconds | Median paired gain |
|---|---:|---:|---:|---:|
| Complete one FE, robust | 983,040 / 50 / 16 | 11.800 | 10.179 | 13.7% |
| FW, exposure, sparse missingness, robust | 245,760 / 50 / 4 | 3.741 | 3.342 | 10.7% |
| PW, offset, sparse missingness, two-way cluster | 245,760 / 50 / 16 | 3.546 | 3.075 | 14.8% |

The separate pilot was 11.794→9.862 seconds and is excluded from these medians.
All targets exceed the prospectively specified 3% primary gain. The scheduling
experiment contains 24 pairs: nine primary, twelve controls, two separate private
profiles and the pilot. The allocation-only experiment contains twelve pairs:
three single-pair public primaries, four controls, two low-memory rechecks and
three private profiles. Together they contain 72 measured calls. These are
selected strata, not a Cartesian product.

The allocation-only primary pairs were 11.669→11.612, 3.674→3.711 and
3.618→3.506 seconds. No command-speed gain is attributed to that small scratch
repair. Its adoption rests on removing the demonstrated allocation with exact
science and unchanged capacity/admission.

## Where the scheduling gains occur

| Case | Preparation | FE projection | Finite solve | Strict/final solve | Inference |
|---|---:|---:|---:|---:|---:|
| Complete | 0.847→0.851 | 2.001→1.160 | 2.757→2.770 | 4.264→3.428 | 0.107→0.106 |
| FW | 0.225→0.224 | 0.731→0.481 | 0.904→0.917 | 1.110→0.959 | 0.096→0.097 |
| PW cluster | 0.221→0.219 | 0.678→0.370 | 0.876→0.900 | 1.081→0.864 | 0.038→0.037 |

Values are seconds. Strict/final timers include subordinate work; these scopes
and independently computed medians must not be added. The isolated scheduling
patch changes only reference-statistic execution. The existing phase attribution
locates its effect in retained projection and strict/final consumers; it does not
provide a standalone hardware or reference-pass timer.

The [cost audit](cost-audit.md) records the tradeoff: both versions evaluate N×K
raw values. Column scheduling adds a denominator pass and repeats weight/root
arithmetic per column, in exchange for column-local reads and concurrency. It
does not skip norms, remove a mathematical pass, cache stale weighted products,
subtract large raw cross-products, or add an observation-sized buffer.

## Science, memory and protected controls

Every completed pair preserves the full exported b/V, sample hash, rank/status,
DoF, weight/cluster/separation diagnostics, scientific certificates and requested
saved outputs exactly. Solver decisions and actual/reported iteration counts,
step halvings, strict retries and fallback counts are exact. No tolerance or
automatic solver-family region changes.

Scheduling leaves primary native allocation peaks exactly unchanged: 2.523 GB,
633.5 MB and 645.3 MB, respectively. Primary sampled process-tree RSS medians
are 1.965→1.960 GiB, 0.609→0.639 GiB and 0.545→0.541 GiB. Native telemetry and
process RSS measure different scopes; no RSS reduction is claimed. Small control
native-peak differences range from −125,528 to +21,760 bytes. All observed native
action/profile phase peaks remain inside their unchanged models.

Weak, dense, bottleneck, cycle, disconnected and unbalanced graphs, small/narrow,
low-memory FW, restricted PW and saved/separated-output controls preserve exact
science. Worst scheduling control slowdown is disconnected 55→62 ms (+12.7%,
+7 ms), below the predeclared max(10 ms, 5%) review threshold. Small/narrow is
32→32 ms; low FW is 884→888 ms. At the row boundary, 65,535 rows gives
175→178 ms and 65,537 gives 178→173 ms with eight controls/four workers.

The initial allocation-only low-FW control was 878→963 ms (+9.7%). Two
independent-seed repeats reverse arm order and give 733→735 and 775→734 ms;
the flagged original remains archived. The largest sampled RSS increases are
31.8 MiB on the allocation-only weak profile (0.063→0.095 GiB), and 29.2 MiB on
the scheduling disconnected control (0.072→0.101 GiB), with unchanged or slightly
lower native peaks. Sampling/caches are not treated as proof of a memory gain.

## Real allocator evidence and its limit

The original finite solve moved out its authoritative `kept` identities, then
reserved QR identity scratch in the emptied slot and discarded it when restoring
`kept`. A temporary debug backtrace identified a 32-byte allocation there in a
small fixture. That instrumentation is removed and is not timing/peak evidence.
The replacement uses `candidate_positions`, whose QR and inverse-certificate
lifetimes do not overlap and whose existing capacity is already retained.

| Sixteen warmed audit iterations | Baseline events | Allocation-only events | Whole-iteration certificate after repair |
|---|---:|---:|---|
| Fast PW, one FE, two-way cluster | 16 | 0 | Pass |
| Low-memory FW, one FE, cluster | 16 | 0 | Pass |
| Weak two-FE graph | 21 | 5 | **Still fails** |

Live/peak growth is zero in all three; retained capacities and fresh/warm
equivalence pass. Subsequent four-thread FW and 16-thread PW scheduling profiles
both retain zero events and passing allocation certificates. Native peaks are
unchanged in every allocation-only pair.

The blanket zero-event assertion initially failed on the weak graph. The
[original protocol](protocol.json) is preserved; [exceptions](exceptions.json)
record that failed broad assertion and explicitly narrow the adoption claim.
Five residual weak-route events remain unresolved. Passing memory and scientific
checks do not substitute for its failed operational certificate.

## Validation and reproducibility

[Validation](validation.json) records 148 distinct focused Rust tests, including
the real allocator probe, all fit-lifetime allocation-failure phases, retained
session generation/rank/capacity checks and forced standardized QR fallback.
The allocation-only plugin passes three licensed fixtures. The final scheduling
plugin passes a new independent full-dummy Poisson/Hessian/sandwich oracle at
exactly N=65,536, eight scaled regressors and 64 groups: unit robust, FW robust
and PW cluster. Every licensed receipt includes r(0) and a terminal sentinel.
Five profile-registration checks pass; no full scientific profile was run.

The main generator rejects N=65,536 because its topology introduces a singleton;
that rule was preserved. The performance comparison therefore straddles the
threshold at 65,535/65,537, while the independent balanced oracle tests the exact
threshold. Setup failures before process launch are also recorded in exceptions.

Use `ppml_passes_compare.py` with `--part allocation`, or `pilot`, `primary`,
`controls`, `boundary` and `profile`, with isolated source directories selected
through `FEREG_LOCAL_BENCH_ROOT`. The archived scheduling run selected only FW
and PW private profiles; the driver also allows a complete-data profile. Set
`STATA_MP_PATH` to a licensed local Stata executable. Data recipes, per-column
import hashes, per-run immutable worker copies and portable numeric receipts
are in [allocation evidence](allocation/archive.json) and [scheduling evidence](norm/archive.json).
Raw licensed logs and plugins remain local.

Current-frame norm reuse, raw RHS/stride passes, fresh-input certificate removal,
the residual weak-route allocator events, inference ownership and centered tiles
remain open. This checkpoint changes scheduling and scratch lifetime only.
