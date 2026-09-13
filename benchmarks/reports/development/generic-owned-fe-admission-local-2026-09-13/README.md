# Generic owned FE admission: local correctness adoption

Adopt [ADR 0062](../../../../docs/adr/0062-generic-owned-fe-memory-admission.md)
against accepted numerical `f576bc5` at evidence HEAD `7ae569d`. Candidate
`ad725932b0df047699900fd5590cdd31413d90a2831f6d486aec95175092578a`
binds 178 source/fixture inputs. The installed plugin is preserved; all builds
and measurements use isolated sources. This change excludes both archived
saved-output precision and saved-Schur/recovery prototypes.

## Changed boundary

Generic `absorb_fixed_effects` now enforces its supplied execution-plus-solver
budget for MAP, CG, Irons–Tuck, SQUAREM and LSMR. Auto admits its MAP pilot first;
an unaffordable required LSMR attempt can continue through the admitted existing
CG route, with a fallback warning. Explicit LSMR includes its preserved input,
uses a fallible copy and rechecks MAP before certificate fallback. No budget means
no added planning. Generic projection buffers also use fallible reservations.

The prepared owned adapter admits effect construction before copying observation
codes/slopes/memberships, then charges retained design/unit weights alongside
the selected solver. For zero finite controls it also charges three simultaneous
caller vectors: normalized weights, retained within outcome and final residual.
The source-specific estimator reuses existing LSMR/window/scatter accounting,
expands coefficient channels, and includes slope whitening, signed Schur covers,
certificate QR and transport. Intercept estimates remain exact. Classification,
weight types, FE/sample ordering, rank, arithmetic, tolerances, inference and
transaction policy do not change. No worker calls Stata SPI.

## Admission and scientific evidence

The earlier [four accepted-source red regressions](../ols-saved-residual-precision-local-2026-09-13/generic-admission-red/)
are retained. They match known intercept/slope residuals unrestricted, then wrongly
succeed at a one-byte joint budget. All four pass after the guard. Further tests
cover exact and one-byte-short limits, unchanged input on explicit rejection,
1/4 installed workers, all four preconditioners, accelerator families, affordable
MAP without LSMR, and a weighted slope graph whose rejected LSMR continues with CG.
Owned constructor capacities, checked overflow and prepared unit/provided-weight
overlap are tested separately.

The final source passes **134 focused core checks** (55 memory, 11 auto,
66 prepared-fit, two PPML constrained-model checks), **95 serial plugin tests**,
**48 saved-output dummy fits** and **four weighted omission/output oracle cases**.
Both architecture slices build and the actual arm64 ABI/self-test passes.

A new public AW intercept/slope fixture with zero finite controls fixture (zero finite controls) compares
residuals with an independent Stata dummy regression. At a **537,677,240-byte
artificial limit**, **806,328 bytes** remain after existing headroom; observed
native peak is **730,668 bytes**. One byte below admission returns `r(909)` with
previous b/V, sample, row order and requested-output absence preserved. This is
one source-bound boundary, not a universal allocator proof.

## Preserved failed candidate and diagnosis

V2's functional exact/one-byte-short boundary passed, but its **708,024 usable
bytes** undercounted the same **730,668-byte** native peak. It was not adopted and
no performance comparisons started. The separate instrumented artifact
`8b7b9eca887aef7ea1334d41fd13d30eae55ce677b1351d7d794fd464d7cb89f`
locates the overlap: prepared resident state is about 251 KB, before the first
owned-design call about 319 KB, and before the final call about 352 KB. The base
is only 262,144 bytes. The solver's own increment fits its reserve; the caller's
normalized weights/within outcome/final residual need another three 32,768-byte
vectors. V3 adds that source-derived reserve. Diagnostic overhead is excluded
from command timings; its peak includes probe overhead. Source variants and
[allocation trace](allocation-trace.json) remain immutable evidence.

The first V2 build compiled both slices but lacked the audit harness in its
source-only snapshot. Exact HEAD harness inputs were copied, then static/actual
ABI checks passed without a numerical rebuild. V3 passes the complete build.
An initial diagnostic setup expected three declarations where source has two;
its assertion stopped before build and was corrected. A syntax check initially
hit a nonwritable Python bytecode cache; compilation without bytecode writes
passes. These are harness corrections, not changed scientific tests.

## Complete-command cost

Ten prospectively fixed paired commands use rotated artifact order, fresh local
Stata processes and the original inputs. Builds, tests, oracle solving, data
generation and archive compression do not overlap timing. No increase exceeds
both 5% and 10 ms, so no follow-up repetitions are triggered.

| Request | Baseline → candidate seconds | Change |
|---|---:|---:|
| Original slope, auto | 0.338 → 0.340 | +2 ms / 0.6% |
| Same slope, explicit LSMR | 0.147 → 0.150 | +3 ms / 2.0% |
| Same slope, explicit CG | 0.245 → 0.244 | −1 ms |
| Unit saved | 2.907 → 2.992 | +85 ms / 2.9% |
| PW partial saved | 3.490 → 3.508 | +18 ms / 0.5% |
| Complete IID | 0.575 → 0.569 | −6 ms |
| Weak saved | 0.095 → 0.094 | −1 ms |
| Tiny weighted cut saved | 0.056 → 0.057 | +1 ms |
| Small AW | 0.008 → 0.006 | −2 ms |
| Weak PW PPML | 0.033 → 0.034 | +1 ms |

Every scientific CSV is byte-identical; full b/V/sample/status/DoF comparisons
pass and all OLS non-time solver fields are exact. Existing inaccurate saved
outputs on PW/weak/weighted-cut baseline requests remain unchanged and are
pending the separate precision repair. Their timings describe guard cost, not
an accuracy-matched optimization claim. No speedup is claimed for this repair.

Native peaks are exact in **nine of ten pairs**. The three slope peaks remain
**28.871 / 42.713 / 25.074 MB** for auto/LSMR/CG. Weak saved native peak rises
**34.105 → 34.636 MB** (+530,456 bytes, 1.6%); its allocation source is untraced.
Public bases and scratch are unchanged; sampled RSS moves both ways. Several
protected routes retain pre-existing observed/public-base discrepancies. Public
base telemetry still excludes solver increments and is not a whole-action bound.
No general memory or RSS saving is claimed. Full input/native/setup/solve times,
peaks, fallback flags, probe counts and scientific exports are in
[summary.csv](summary.csv), [cost review](cost-review.json), and the checksummed
numeric archives. Public solver column 12 is warning count, not checkpoint retries.

## Remaining work

This adopts the unified generic owned-admission repair. It does not finish
specialized slope MAP, saved recovery, public whole-action reporting or opaque
upstream factor-storage proofs. The signed-cover allowances extend the existing
pinned factor model; these local checks do not establish universal bounds.
Next rebase and validate the separate final-output precision repair, then resume
saved-Schur/recovery comparisons. Neither prototype is counted as adopted here.
No agents, SCC, qualification, dependency update or push occurred.

The archive collector initially expected 19 allocation trace entries; the captured
trace contains 20. The count assertion was corrected and only the unfinished
receipt-writing tail resumed. Existing numeric archives were not regenerated.
