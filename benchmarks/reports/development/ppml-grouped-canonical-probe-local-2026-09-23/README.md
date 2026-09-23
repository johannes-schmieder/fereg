# Implicit one-FE canonical products: frozen development experiment

The implicit centered representation is worth a production integration trial.
On identical frozen one-FE weighted problems, its projection, norms, independent
projection check and finite moments improve by **18.6–47.7%** at K10/K32. All
compared Gram, RHS and within-norm coordinates are bit-identical to the baseline
on the three frozen inputs. An independent explicit-dummy weighted SVD and
Hessian oracle passes on the small frame.

This is **not production adoption or a public-command speed claim**. Production
remains `b7106fcb7e7db3bc9a8958b4ca3f2740d66e5266`, accepted `canontile1`, plugin
SHA-256 `5ede5b83f42a4a291e56dab2b27a94c3711197ac3a32d3ef6d64870461a1b205`.
Its 539 computational inputs are unchanged. The public baseline and stock
comparisons remain attached to the
[preceding report](../ppml-canonical-tiles-local-2026-09-23/README.md).
No production solver, tolerance, rank, memory admission or certificate changed.

## Source and frozen work

The isolated workspace exports Cargo manifests, crates and fixtures from the
production commit, then adds one test-only module. Each variant has 425 captured
source/fixture inputs. The first and refined variants differ only in the probe
module. Separate Cargo targets avoid cross-snapshot executable reuse:

- First binary: `567f5306a9ec32df106f746b36b2723d918ff21970f216171d5e87c7de3c8231`.
- Refined binary: `cceb18fca6ea9750d7c8078036e3903fad7031d4653cbc3a0f8963f9f92f2adb`.

Inputs come from development seed 202609335 and accepted `canontile1` public
means. The three frames have N/K/G equal to 1440/3/288, 61440/10/12288 and
30720/32/6144. Unit-prior Hessian weights are the accepted means; working
responses are constructed as `log(mu) + (y/mu - 1)`. Both native engines receive
the same bytes. This construction is not asserted bit-identical to every
arithmetic operation in the public working-response implementation. Sample,
source-cell and binary-input hashes are in `declaration.json`.

At T1 and T4, each frame has one warm pair followed by five alternating pairs
in one native process. Both use a 1e-14 projection certificate threshold. The
baseline preallocates its reusable categorical projector outside timing; its
timed work includes row-matrix construction, raw norms, current projection and
certificate, selected certified norms, and the production finite moment kernel.
The candidate times current group totals/centers, raw/within norms, independent
residual replay and finite moments. Common immutable inputs and topology are
outside both timers. This does not cover the complete canonical WLS solve,
rank selection, inverse, raw global means, predictor recovery or inference.

There is no new stock kernel or public benchmark here. No assistant-launched
build, fit, profile or archive overlapped the native measurement blocks. No
whole-machine contention monitor was used for this diagnostic experiment.

## Representation and independent checking

The candidate stores current-weight group means for each response/control
column. It reads a centered observation as original value minus its group mean,
without constructing an N-by-(K+1) projected matrix. Group means retain the
baseline's scaled, compensated two-pass arithmetic. Raw and within weighted
norms retain stable scaling and each column's observation order.

Acceptance independently rereads original values minus the proposed group
effect and recomputes residual group moments. It shares current weight totals
and immutable topology, but does not reuse the construction's value sums.
The same relative projection criterion applies. Thus an initial early-exit
projection diagnostic and materialization can be avoided without substituting
a recurrence residual for the final direct projection check.

Finite moments use disjoint Gram-row partitions. Each observation's selected
suffix is centered and standardized once per partition; Gram/RHS updates keep
the existing per-coordinate observation and FMA order. There is no rank solver
in the prototype. Bitwise agreement is observed evidence for these frames,
not a requirement for a future algorithmic redesign.

## Refined distributions and first loss

Median [minimum, maximum] milliseconds, five pairs per row:

| Frame / threads | Baseline components | Implicit components | Ratio of medians |
| --- | ---: | ---: | ---: |
| N1440 K3 T1 | 0.143 [0.138,0.146] | 0.072 [0.071,0.072] | 0.501 |
| N1440 K3 T4 | 0.161 [0.143,0.168] | 0.046 [0.035,0.073] | 0.283 |
| N61440 K10 T1 | 13.103 [12.769,13.249] | 8.606 [8.457,8.723] | 0.657 |
| N61440 K10 T4 | 7.951 [7.754,8.092] | 4.162 [3.663,4.645] | 0.523 |
| N30720 K32 T1 | 18.805 [18.576,18.874] | 15.306 [15.110,15.399] | 0.814 |
| N30720 K32 T4 | 10.017 [9.200,11.239] | 6.100 [5.986,6.647] | 0.609 |

All 30 refined pairs improve. Small sub-millisecond observations are useful
primarily for correctness; they do not establish a stable public-command margin.
K10/T4 pair ratios range 0.461–0.599; K32/T1 0.801–0.824; K32/T4 0.545–0.664.
Every individual result and phase time remains in `zip-summary.json`.

The first candidate **lost K32/T1 by 7.4%**, 18.454 versus 19.825 ms. Its indexed
inner Gram loop cost 11.167 ms versus the baseline's 4.964 ms. The next experiment
was declared before its measurements and used the existing production kernel's
contiguous slice iteration over disjoint Gram rows. No data, thresholds,
selection or arithmetic order changed. The refined K32/T1 moment median is
6.527 ms against baseline 5.217 ms: that component still loses, while avoiding
projection materialization wins overall. The first loss, all first pairs and
both source snapshots are preserved. This is one localized implementation
correction, not a new benchmark-dependent route.

## Science, memory and limits

Two distinct native tests pass on each version, four successful invocations.
The frozen test checks 30 measured pairs per version. The boundary test covers
18 configurations per version: T1/T4, three different weight frames and common
weight scales 1e-200/1/1e200. It includes columns scaled 1e-120/1e120, large
group offsets with small within variation, a truly FE-collinear column and a
rank-deficient duplicate. It compares norms/moments/projection acceptance,
**not canonical rank or collinearity-status decisions**.

The small-frame independent oracle constructs the full FE dummy design and
uses SciPy's SVD least-squares solver. It checks projected values, identified
finite coefficients, the inverse independently formed within Hessian, raw
norms and within norms. All five checks pass for both versions. Maximum absolute
errors are 1.33e-15 for projected values, 1.55e-15 for finite coefficients and
2.03e-19 for bread. Both versions emit identical small-frame binary exports.
This is an independent frozen WLS/Hessian test, not a new nonlinear Poisson or
public sandwich result.

Center payloads are 1,081,344 bytes for K10 and 1,622,016 for K32, compared with
5,406,720 and 8,110,080 bytes for their baseline projected matrices. These count
selected payloads only. Current group totals, norms, Gram/RHS, worker rows,
prepared originals, allocator overhead and complete lifetime coexistence are
not a measured peak. In particular, there is no demonstrated low-memory
admission result. If G approaches N, group storage may be expensive; singleton
or frequency-weight semantics must not be used to assume G is always small.

The prototype is test-only and assumes validated finite input. Production needs
fallible allocation, interruption/error behavior, explicit lifetime and current
generation ownership, deterministic rank/guarded-QR fallback, and fresh final
finite/score/objective/reconstruction/Hessian acceptance. Shared immutable
prepared ownership can support implicit rows through final inference without
copying another N-by-K bank. That ownership change has been inspected but is
not implemented or accepted here. The active plan lives only in `STATUS.md`.

At the preceding public stock median, low N61440/K10 must fall from 119 to
47.7 ms and fast from 104 to 48.6 ms. These component gains alone do not explain
how to close that gap. Accepted-state scans, canonical recovery and the public
control plane remain separate substantial work. Do not transfer a component
percentage to the public command or treat the present checking implementation
as an irreducible cost.

## Receipts and preserved failures

The restored content-addressed archive verifies **464 logical receipts**, **456
payloads**, **2,226,180 bytes**, SHA-256
`b4aa8622bd002fac29df341f46d8c3a70770f807dc8dd2154ace7f3cbf0bb5b4`.
It includes source, input/output hashes, declarations, both binaries' identities,
all native results, independent oracle and regeneration/verification recipes.
Data and binaries are omitted; original and sanitized receipt hashes are retained.

Tooling failures are preserved separately: an initial export omitted compile-time
fixtures; system Python lacked the requested tar extraction option; archive
guards caught their own literal prefix and existing fake redaction-test paths.
These failed before scientific tests or archive publication. No numerical
tolerance or bound changed to obtain a pass. The corrected builds and receipt
restore completed. No new licensed run, full qualification, SCC retry, campaign,
push or release occurred. Existing allocation drift, constrained-fit/CG issues,
comparator discrepancies and platform deferrals remain unresolved.
