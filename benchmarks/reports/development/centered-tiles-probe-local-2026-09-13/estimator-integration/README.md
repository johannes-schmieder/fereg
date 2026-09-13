# P5 centered estimator integration: unadopted prototype

The centered finite solve now constructs an actual `CertifiedPreparedOlsFit`
and feeds the existing conventional, robust and clustered inference consumers.
The [receipt](receipt.json) binds 182 inputs against accepted `436fe2a`, six
changed since [score streaming](../score-streaming/README.md), the complete
patch and eleven sanitized logs. No production estimator route is enabled.

The candidate borrows prepared numeric columns and a virtual intercept, computes
stable current-weight group means, and streams finite moments. Uncertain rank
uses the existing Householder scratch implementation. The result includes actual
coefficient/status/bread, residuals, initial within TSS, solver and execution
reports; it does not copy these from an oracle fit. Scratch bytes are reported
when used. Initial projection timings come from the centered computation.

Packed inference retains its scalar raw-reference norms and fixed-partition raw
means. Larger-cache-policy fits retain the existing raw-moment-cache policy.
A test-only means cache carries those values through intercept normalization
and score construction without an N-by-K score array. Rank reduction preserves
the old cache-mismatch/reconstructed-mean rule. Existing matrix/session consumers
reject this test-only representation instead of silently treating it as a matrix.
The two session changes are test-only match arms; numerical session code is
unchanged. Final residuals still run the existing warm FE and original-row score
checks. No tolerance or rank rule changes.

## Evidence

All **118 distinct final-source tests** pass: 25 centered, 64 existing prepared
fit, nine robust, nine clustered and eleven conventional inference tests.
Non-test core checking passes. Six final-source logs are separate from five
intermediate attempts. Named test counts and terminal summaries agree.

The centered coefficient result now supplies all 28 preceding robust/clustered
covariance comparisons, including independent weighted HC1, literal frequency
expansion and original-row multiway inclusion–exclusion oracles. Three additional
unit/FW/AW conventional covariance comparisons agree with independent weighted
group means and `RSS/df_r * bread`; PW conventional inference retains rejection.
Full fit statistics, coefficient presence/status, rank, kept/dropped coordinates,
DoF/weight/cluster structures, normalized intercept/bread, residuals and discrete
initial/final solver reports agree under existing scientific envelopes. Eligible
cache means and initial within TSS match exactly. These are 31 successful full
covariance comparisons, not public Stata command measurements.

The controls include gapped FE-collinear slots, exact-rank-deficient scratch,
rank zero, no reported intercept, K65 and parallel N32777 rows, and the existing
cluster missingness/singleton distinction (FW 256 rows versus AW 255). Two further
full coefficient comparisons extend the independent large-reference fixture:
the 2^14 column retains beta 7 and bread 1/640, while the 2^40 column remains
FE-collinear under the authoritative original-scale rule in both cache policies.

All 74 injected fit-reservation failures are observed (32 well-conditioned and
42 rank-deficient). Minimum component budgets are 676/1574 bytes; exact budgets
succeed, one byte less and excess allocator capacities fail. The deficient tiny
fit reports 192 scratch bytes and its exact/one-byte-under disk bounds are tested.
Changed absorption configuration and foreign prepared bindings fail before fit
reservations. Existing component scratch-file cleanup and score-reservation tests
also pass. The component cap includes new fit reservations and scratch I/O
capacity. It excludes previously bound state and allocations owned independently
by FE and inference consumers, so it is **not full estimator admission or a
native peak measurement**.

## Failures retained

The initial compile exposed two exhaustive session matches missing the test-only
cache variant and a non-Copy certificate move. One follow-up compile still missed
the borrowed cache match. These are corrected with explicit unsupported exits and
a certificate clone. The first running covariance comparison exposed a real
prototype integration bug: zero candidate columns reached a nonempty-only moment
builder. The candidate now skips that builder and retains the existing rank-zero
fallback result. Four other covariance tests passed in that attempt; all five
pass subsequently. No scientific assertion was relaxed to obtain a pass.

## Remaining before adoption

Production ownership/result wiring, saved-output recovery and full overlapping
memory/scratch admission remain. Initial projection certificate failure still
needs the existing continuation/failure semantics before automatic admission.
The bound currently builds raw moments even when packed inference later selects
its distinct scalar reference policy; that extra pass is not assumed profitable.
Rank-zero fallback currently creates scratch, and the independent centered-score
Gram certificate remains serial. These costs, group-mean storage when G is near
N, normal/fallback passes and dense helper extraction all belong in the complete
command/phase/native-peak comparison. No plugin was built and no timing or memory
gain is claimed. Separate PPML current-weight/rank/checkpoint evaluation and all
P5 checkboxes remain open.
