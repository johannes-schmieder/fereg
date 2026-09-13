# P5 centered frame and core certificates: unadopted prototype

This [181-input snapshot](receipt.json) extends the previous
[scratch fallback](../scratch-fallback/README.md) against accepted `436fe2a`.
Five inputs change from that preceding snapshot. The complete
[prototype patch](prototype.patch) has SHA-256
`c904f708083b898cc32f1175b68dcd352cb0f49f34bdb14b9c0ff8dc61cd2630`.
All centered routing remains test-only. Shared norm/cache helper refactors and
the immutable borrowed-effect `Copy` implementation are unadopted as well.
No plugin, licensed command, benchmark, native allocation peak or release
qualification was run. The installed plugin remains unchanged.

## Frame binding and numerical boundaries

`BoundCenteredFrame` binds the actual prepared numeric columns, final FE
membership, response, raw weights and reference weight convention. Continuous
columns and response are borrowed; the intercept is virtual, without an
N-element ones vector. Only the descriptor table and group means are owned.
No centered matrix is retained by the prototype. The test oracle deliberately
materializes matrices for independent path comparisons.

The frame uses the existing raw-moment cache through a private source interface,
with unchanged loop, reduction and normalization arithmetic. Its raw reference
norms are distinct from the projector's norms: at supported sizes, the former
use balanced partitions based on 16,384 rows (at most 64), while the latter
use fixed 4,096-row partitions. The extracted projector norm kernels preserve
that distinction. Group means reuse the established scaled/compensated kernel.
A separate FE certificate recomputes moments of centered values, using original
projection reference scales. It does not infer success from stored group means.

The actual caller tolerance is respected. Non-auto solver requests, retained FE
coefficient requests and nonmaximum explicit weight divisors return unsupported
before centered-state allocation. Non-numeric generated columns and designs
other than exactly one intercept FE retain their existing route. Tests directly
exercise no-FE, explicit MAP, retained-coefficient and alternate-divisor exits;
no broader generated/slope route coverage is claimed from those controls.

Each fresh binding recomputes current response/weight products. The immutable
owner has no refresh or last-result accessor that could attach stale means or
norms to a new frame. Changed response only, weights only, and both together
match separately prepared matrix oracles while the original owner stays intact.
This is a private OLS prototype, not admission of a PPML retained representation.

## Focused evidence

All **154 distinct tests** pass on the final bound source: 18 centered-filter
checks (including one existing slope test), 64 prepared-fit, 39 borrowed-MAP,
23 finite-kernel and 10 scratch-storage tests. Non-test core checking also
passes. The receipt identifies the six final-source logs separately from ten
intermediate logs; earlier stage receipts are not relabeled or pooled.

Six frame comparisons cover unit/FW/AW/PW, N257/1024/32777/65537, finite widths
3/10/13/64/65, and 1/4/16 workers. An exactly FE-collinear interior generated
slot and reported intercept additionally exercise gapped mappings. Raw cache
means/scales/norms/weight sums, centered values, certified within norms, finite
moments and initial FE maximum/absolute certificate diagnostics are exact
against the matrix route. Diagnostic FE two-norm agreement allows only its
existing group-reduction rounding difference.

Five well-conditioned fits and one exact-rank-deficient scratch fit proceed
through existing independent raw-residual construction, normalized reported
intercept/bread finalization, warm final FE certification and original-row
score certification. They agree with the full matrix-based core coefficient
fit under the unchanged coefficient/bread/residual envelopes; rank/status,
bread coordinates/weight scales and within TSS agree. Corrupted final residuals
are rejected even with the valid earlier finite-score draft. These are core
coefficient/finalization checks: robust/clustered covariance, DoF publication
and saved-variable transactions have not yet consumed centered scores.

Across all 13 scratch comparisons in the final centered-filter run, maximum
absolute coefficient/bread/residual differences are 8.79e-11/8.75e-7/1.21e-12,
within unchanged registered mixed envelopes. The new frame-based fallback adds
to the prior twelve cases; numerical maxima are not advertised as error bounds.
The independent rational group-moment corruption test detects a one-unit mean
error as a raw FE moment of ten. Existing rational/frequency-expansion,
large-between/small-within, rank-zero, nonfinite and scratch cleanup tests pass.

The initial rational-mean test incorrectly required bit-exact agreement with
-0.5. Existing scaled mean arithmetic returns -0.49999999999999994. Only that
oracle assertion now permits four epsilons times max(1, |mean|); exact matrix
parity remains required. The failing log is retained. No fit tolerance, rank
threshold or scientific policy changed.

## Memory and pass costs still to measure

For Q generated RHS columns, G final groups and N active rows, binding owns
G*Q doubles plus Q optional borrowed-slice descriptors. Raw cache and both norm
outputs are column-scale; temporary raw-cache partials are at most 64*3*Q
doubles. Parallel projector norm partials use 2*Q*ceil(N/4096) doubles at
eligible widths. Norm scratch is admitted before allocation, and actual
capacities are checked. Five injected fit-reservation failures, overcapacity,
invalid-axis and undersized-state tests fail closed. In the tiny fixture the
minimum successful state budget is 336 bytes, versus 288 cumulative charged
bytes after norm scratch is released. One byte below that minimum fails.
These are explicit capacity charges, not measured estimator peaks.

The current prototype still validates source values, constructs stable group
means, makes the raw-cache passes, computes separate projection reference and
within norms, and recomputes FE moments. Centered moment consumers reconstruct
values in bounded tiles. The established scratch fallback writes all generated
columns and incurs repeated QR reads/writes; finalization adds original-row
residual and certificate passes. Existing owned input, topology, temporary
norm/Gram banks, residuals, inference, fallback and publication must be admitted
together before production use. No fewer-pass, speed or peak-memory claim is
made merely from removing the resident within matrix.

Next integrate the centered result with estimator reports and original-row
robust/clustered score consumers, matching the packed inference path's distinct
reference-norm policy as well as the raw-cache policy tested here. Add saved
outputs and full overlapping admission, then compare complete commands, phases,
copy/pass costs, native peaks and fallback I/O against isolated accepted source.
Review the shared helper refactors' dense-path cost too. PPML needs a separate
current-weight/rank/checkpoint evaluation. All P5 checkboxes remain open; no
agents, cluster work, qualification, push or installed-dist replacement occurred.
