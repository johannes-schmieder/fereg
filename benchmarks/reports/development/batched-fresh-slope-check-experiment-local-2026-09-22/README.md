# Batched fresh slope-check experiment

Reject this draft. It preserves the numerical results but does not establish a
useful improvement in the remaining serial weighted-slope targets. Current
computational source is restored exactly to dtk1; no weight-specific guard or
production change is retained.

## Experiment

The serial, row-ordered cached-slope projector processes 256 groups and then
checks their freshly written residuals while the batch is still local. It uses
the same independent QR directions, compensated raw moments, row/FMA order,
weighted maxima and final ordered reduction. Existing check and coefficient
buffers remain in use; there is no new heap owner on the successful path.

The provisional checks use the original reference norms. Any enlargement by
the final residual norms invalidates all provisional checks and reruns the
ordinary complete certificate. A provisional numerical overflow is deferred
until projection finishes, then the ordinary final check runs. Other errors
remain fatal. Check-block clock intervals preserve the separate reported
certification time. No original input is mutated, and rejected certification
preserves the established fallback. Eligibility, rank, tolerances and the
threaded/shuffled/shared/PPML boundaries remain unchanged.

The key fallback fixture uses 513 groups, RHS widths 1/12/52, input scales up
to 1e200 and deliberately zero/tiny provisional reference norms. It verifies
exact residuals, fresh final certificates and tracked peaks against the
ordinary path, including overflow before final norm enlargement. Existing
independent projection/certificate, rank, extreme-scale, weight, thread,
shuffled-input, budget and full-fit scope checks also pass. Six release Rust
fixtures pass in three source-bound invocations; the candidate passes the
actual native ABI/self-test harness.

## Measurements

Sixteen cells with three alternating fresh-process pairs give 141 successful
worker calls. There is no detected competing-process overlap, and no build,
test, audit, compression or diagnostic fit ran alongside the headline screen.
The design remains N=245,760, seed 104729. AW/unit/FW have an absorbed intercept
and one slope; PW has three slopes without an absorbed intercept. All 48
native pairs have exact six numerical exports, samples, requested saved
vectors and all 28 non-timing solver fields, with equal tracked allocation
peaks. Embedded independent saved-output checks pass.

| Serial case | Prior seconds | Draft seconds | Draft / prior paired ratio |
|---|---:|---:|---:|
| AW K10 robust, full outputs | 0.237 | 0.237 | 1.0000 |
| AW K10 clustered, full outputs | 0.284 | 0.284 | 1.0071 |
| AW K10 robust, no outputs | 0.181 | 0.177 | 0.9779 |
| PW K6 robust, full outputs | 0.212 | 0.211 | 0.9953 |
| PW K6 clustered, full outputs | 0.246 | 0.245 | 1.0203 |
| PW K6 robust, saved effects | 0.189 | 0.187 | 0.9894 |
| FW K10 robust, full outputs | 0.229 | 0.232 | 1.0090 |
| Unit K10 robust, full outputs | 0.205 | 0.198 | 0.9659 |
| Unit K10 IID | 0.119 | 0.115 | 0.9664 |
| AW K50 robust, full outputs | 0.858 | 0.846 | 0.9977 |

Ratios are medians of within-repeat ratios, not ratios of the displayed
marginal medians. Threaded controls and pinned fixest comparisons appear in
[all timings](timings.md). Ordinary fixest weights are excluded for FW
inference. The AW/PW robust/full reference times are 0.193/0.148 seconds; the
remaining losses are explicit. The apparent unit-weight gains and mixed small
weighted changes are screens, not confirmed improvements or regressions.
No confirmation blocks are run for this rejected draft.

Six separate diagnostic calls preserve exact paired exports and outputs.
Their existing outer phase clocks give initial absorption, including projection
and fresh checks, of:

| Case | Prior | Draft |
|---|---:|---:|
| AW K10 | 26.172 ms | 25.529 ms |
| PW K6 | 26.130 ms | 26.737 ms |
| AW K50 | 83.076 ms | 85.336 ms |

These single traces are diagnostic only. They provide no consistent phase gain
and cannot rank whole-command performance. The experiment rearranges work for
cache locality but does not remove its arithmetic. Future work should target
the repeated geometry/norm calculations identified in the existing profiles.

## Source and evidence

- Prior dtk1 plugin: `77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151`.
- Rejected sbf1 plugin: `86a44b3b012ca15f953e776eaecdada1c1841682248d16bb79d47aa81707f7ef`.
- Candidate base: `f978df0f`, with six existing graph/window drafts and this
  isolated change to `fe/single_slope.rs`.

The archive contains 147 successful worker records, tests, exact source
identities, controllers, audits and filtered diagnostic traces. Large saved
arrays remain local with explicit hashes; raw licensed logs and binaries are
excluded. Initial edit-script syntax failure and preliminary development runs
are preserved. The warning about a mutable constant temporary was fixed before
the captured source-bound tests/build; no scientific failure was accepted.

`verify.py` checks 3,488 stored entries, 129 omitted local arrays and both
complete 523-file source reconstructions. Privacy checks pass. Every current
computational source hash again matches dtk1, including the six unchanged
pre-existing drafts. No push, installation, release or qualification occurs.
Linux confirmation, routing adoption, serial reference losses and final
integration remain open; this experiment does not complete the milestone.
