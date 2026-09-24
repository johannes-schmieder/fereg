# ADR 0159: close the retained dense observation-tile experiment

Status: rejected for production, 2026-09-24.

## Decision

Keep ADR 0158's production computation unchanged. Do not adopt the eight-row
standardization tile for the existing at-most-64-column retained dense moment
helper. Preserve the prototype and measurements in the
[source-bound report](../../benchmarks/reports/development/ppml-dense-moment-tile-closeout-local-2026-09-24/README.md).
Do not tune width/thread guards or repeatedly chase the small whole-fit residual
in this experiment.

The prototype retains each Gram/RHS entry's exact observation and FMA order,
selected mapping, current weights, scales and existing Gram-row partitioning.
It keeps each accumulator in a register through eight observations instead of
repeatedly loading and storing it. Temporary stack arrays grow from 512 to
4,224 bytes per active partition, with no new heap, Gram bank or cross-frame
state. One-shot OLS, wider/unscaled/one-column partitions, scratch and implicit
rows keep their old routes. No scientific policy changes.

## Evidence and interpretation

An isolated paired experiment borrows identical immutable weights, responses
and designs for both helpers in one binary. All products are bit-identical;
six kernel medians improve 19–41%. Seventy-one focused Rust tests and four
licensed fixtures pass, including independent weighted projections, explicit
Poisson/Hessian/sandwich oracles, rank boundaries, generated terms, saved
outputs and the ordinary-iteration allocation audit.

All 120 public applications pass. Thirty-six native PPML pairs have identical
coefficients, covariance and means, and 36 direct candidate-stock comparisons
pass. Six OLS pairs have exact scientific exports. The candidate's tracked and
modeled PPML memory is unchanged; tracked heap does not include the added stack.

Whole-command gains are insufficiently broad. The development wide median
improves 92→90 ms, but fresh wide remains 70 ms; fresh unbalanced improves 81→77,
while fresh slopes lose 85→87 and weak connectivity 41→43. The latter losses may
include noise, but they do not justify global adoption in exchange for small
gains. The candidate would also move that slope case from below to above the
0.90-times-stock target in this block. Do not substitute kernel wins for public
command evidence or infer a scientific failure from these timing losses.

Retain the accepted group-center preparation change. Next split final inference
into row access/score construction, independent meats, covariance products,
PSD and Wald work before selecting a replacement. Its existing 16,384-row
blocks and fresh certificate work are measurable candidates, not permission
to change partitioning or remove independent checks without evidence.

Seed 371 is consumed/frozen. The 77/16/111 definitions, reference pins,
scientific envelopes and existing unresolved comparisons remain unchanged.
No qualification, release, SCC retry or remote action follows.
