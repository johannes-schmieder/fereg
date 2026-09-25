# ADR 0178: defer two-FE PPML Schwarz construction

Status: accepted for local development, 2026-09-24.

## Decision

The categorical joint LSMR `Reuse` policy starts with its already allocated,
fresh current-weight FE diagonal while no Schwarz factor exists. The existing
staleness rule (four consecutive contractions above 0.75), exhausted recurrence,
or failed independent certificate promotes that frame to Schwarz. After
promotion, retain the factor and its existing refresh policy. Three-plus-FE
core-pair, explicit diagonal, canonical checkpoint, slope, low MAP and OLS
policies are unchanged. No design-name, seed, width or timing threshold is added.

Promotion from an uncertified diagonal iterate restarts from zero. Continuing
from that iterate failed the existing weighted strict-final/fallback truth test;
the cold restart passes its original accuracy, thread and fallback cases.
Continuation with an already established Schwarz metric keeps its prior iterate.
This is a change in initial preconditioning and fallback ownership, not in the
weighted operator, acceptance tolerance or number of permitted recovery attempts.

Every returned direction still requires recurrence convergence and a fresh
original-input finite/FE normal-equation certificate. Outer score, objective,
model-space, two consecutive convergence passes, strict rank/bread/reconstruction
and final-Hessian checks remain. Diagonal success has no canonical rank or
inference authority. Changed weights always refresh the diagonal; topology and
active-basis replacement retain their existing invalidation boundaries.

## Cost and memory

Current-source profiles show that the factor is already reused between ordinary
IRLS frames and its clone is O(1). Initial construction, not repeated cloning,
is the avoidable cost here. In the development disconnected case, measured joint
setup falls 16.96→0.83 ms, recurrence grows 8.62→9.60 ms, and whole-public time
falls 111→96 ms. That attribution comes from public solver phase receipts as
well as a separate instrumented baseline; diagnostic timings are not headlines.

No buffer or admission credit is introduced. The full Schwarz build, retained
factor, prior result and continuation overlap remain charged. Tracked allocation
can fall substantially when the factor is avoided, but rises 6,784 bytes in the
fresh weak case because promotion overlaps the retained pilot result. Modeled
peaks are unchanged. This does not repair the static allocation inventory or
establish physical-cap/RSS qualification.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-deferred-schwarz-local-2026-09-24/README.md)
preserves exact identities, all distributions, independent oracles, controls
and rejected attempts. Fresh five-rotation medians improve serial 108→93 ms,
wide 131→127, disconnected 113→107 and unbalanced 57→55. Narrow improves only
45→44; its outer count grows from three to four. Serial instead falls from four
to three outer iterations. Small oracle cases and the development weak case
lose 1–2 ms. These tradeoffs justify a small, general lifecycle change, not a
claim of universal non-regression or completion of the <=0.90-times-stock goal.

All 260 PPML public states pass their own independent checks. All 76 native
candidate/baseline pairs pass. Fresh sparse fitted means disagree with stock on
all five repetitions, for both native versions; that comparison is unranked.
All six small explicit Poisson/Hessian/sandwich/mean oracle states pass. There
are 135 distinct focused Rust passes, six licensed science fixtures, and six
exact OLS export pairs. The known baseline checkpoint-count assertion still
fails and is unchanged. No qualification, remote mutation or SCC retry follows.

The next boundary remains shared current-frame statistics, reconstruction and
family evaluation across ordinary/canonical/publication work. Do not tune this
pilot's thresholds against the consumed fresh seeds 202609401/402/403. The
report quantifies the remaining gaps and useful phase budgets.
