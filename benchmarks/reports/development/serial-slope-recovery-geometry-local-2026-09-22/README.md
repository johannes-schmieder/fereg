# Serial slope recovery geometry

The frozen `rgc1` candidate reuses scaled group Gram matrices and channel scales
between the direct saved-effect solve and its fresh residual certificate. Two
independent five-pair blocks find modest gains in serial PW saved-output cases
(about 1–4%) and FW (about 3–4%). AW robust is effectively neutral. This retains
a local refinement under ADR 0097; serial small-regressor fits still lose to
fixest, and the broader milestone remains open.

| Serial saved-output case | Block 1 paired candidate/prior | Block 2 paired candidate/prior |
|---|---:|---:|
| AW K10 robust, full outputs | 0.9922 | 1.0123 |
| AW K10 two-way cluster, full outputs | 0.9832 | 0.9966 |
| PW K6, three slopes only, robust, full outputs | 0.9871 | 0.9731 |
| PW K6, three slopes only, two-way cluster, full outputs | 0.9762 | 0.9838 |
| PW K6, three slopes only, robust, saved effects | 0.9571 | 0.9671 |
| Unit K10 robust, full outputs | 1.0000 | 0.9783 |
| FW K10 robust, full outputs | 0.9724 | 0.9636 |

[All timings](timings.md) include the twelve-cell three-pair screen with stock
fixest and both native-only confirmations. The screen includes no-output,
threaded and wider-regressor controls. All use N=245,760 and seed 104729.
Ordinary fixest weights do not reproduce Stata FW inference; FW remains a
native comparison. The candidate is frozen across both confirmation blocks.

All 245 screen/confirmation calls pass with no detected overlap. All 106 native
pairs have exact six-bundle numerical exports, samples and requested saved
arrays. Independent output reconstruction and normal-equation checks pass.
Tracked native peak allocation is equal in all pairs: the bounded recovery cache
does not raise the measured whole-command maximum in these cells. Five focused
release fixtures cover checked storage sizes, full/zero/duplicate/near-dependent
slopes, unit and extreme raw weight scales, shuffled groups, exact certificate
equality, rejection with multiple workers, budget boundaries and protected PPML
allocation failure/atomicity. Both normal and instrumented artifacts pass ABI.

Six diagnostic applications compare `rn1`, `rgc1` and instrumented `rgcd1` in
AW K10 and PW K6. Four native comparisons have exact outputs. The PW trace
reduces recovery from 34.5 to 26.7 ms; the latter includes 13.6 ms in group solves
and 11.4 ms in finishing checks. AW recovery falls from 20.5 to 16.8 ms. These
single traces explain the change and are not headline performance evidence.

The cache retains `levels * channels * (channels + 1)` doubles within one serial
recovery call. Fit geometry is deliberately not handed across: its raw Gram/QR
policy differs from recovery's scaled, compensated elimination. Fresh residual
scales, cross moments, rank solve and all output checks remain required. Exact
joint-budget admission preserves the old uncached direct attempt and MAP fallback.
PPML and threaded recovery are unchanged; no solver-selection rule changes.

The archive preserves all 251 calls, complete identities for three source trees,
controllers, reference library hashes, filtered traces, test logs and audit
receipts. `verify.py` verifies 6,978 stored entries and reconstructs all captured
sources. The 233 omitted output arrays remain local with hashes. Binaries and
raw licensed logs are excluded. Measured sources include the six preceding
graph/window drafts; previous final-block claims retain their original source
scope. Linux confirmation remains open. No push, installation or release occurs.
