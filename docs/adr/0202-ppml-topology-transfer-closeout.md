# ADR 0202: close the multi-partition PPML topology-transfer experiment

Status: rejected for production, 2026-09-25. Local development, not qualification.

## Decision

Retain **c7c4c473 / controlplane3** and its unchanged slopetopology1 native
plugin. Remove the four-file `partitiontransfer1` prototype. All **564 accepted
computational inputs** match after restoration. Preserve its source snapshot,
tests, public distributions and audits in the
[source-bound report](../../benchmarks/reports/development/ppml-topology-transfer-closeout-local-2026-09-25/README.md).
No fresh validation seed is consumed and no candidate code remains in production.

## Hypothesis and boundary

Multi-partition PPML availability preparation already owns encoded category codes
and group membership. When the PPML coordinator retains all generation rows and
each FE recipe refers to one categorical source, generic topology finalization
rebuilds these unchanged arrays. The prototype transfers their ownership instead.
Candidate-order source storage and first-use registry ordering are preserved;
repeated terms share one topology entry. Independent code/member validation,
finalized-topology checks and fallible reservations remain mandatory.

The route is restricted to multi-partition PPML with coordinator-owned pruning
and single-source recipes. Interaction recipes, one-FE preparation and public
OLS keep their existing routes. No numerical geometry, response, solution or
authority crosses weight or generation boundaries. All numerical solvers,
certificates, rank, sample, weight, inference and transaction policies are unchanged.

This removes a real repeated preparation stage. It adds about 130 production
lines and model-sized ownership maps, plus focused tests. It cannot remove more
than the prior prepared-model ledger's roughly 1.1 ms narrow / 3.3 ms large
categorical cost; that is insufficient for the accepted-source 11 / 21.7 ms gaps.
Those prior diagnostic spans are not measurements of this prototype.

## Evidence and rejection

The predeclared development seed202609347 pilot runs three alternating
baseline/candidate/pinned-stock rotations over six cases. All applications and
sampled contention monitors pass. Baseline/candidate/stock medians are:

- Disconnected low categorical: **108 / 106 / 99 ms**.
- Narrow categorical: **37 / 38 / 33 ms**.
- Wide low categorical: **83 / 82 / 127 ms**.
- Low T8 two-partition slopes: **71 / 71 / 81 ms**.
- Unbalanced three-FE slopes: **179 / 179 / 234 ms**.
- Multiple slopes: **134 / 134 / 271 ms**.

There are **seven paired wins, four losses and seven ties**. No new family reaches
0.90 stock. The large categorical gain is consistent but small; narrow loses,
and three slope medians are unchanged. This does not justify the extra production
path or another correction/validation cycle. No threshold tuning follows.

All 24 distinct focused Rust tests and six licensed independent scientific
fixtures pass. The topology tests compare generic and transferred finalization,
including source reorder/alias cases, pointer-preserved ownership, malformed axes
and every injected reservation refusal. Existing sample, weight, generation,
rank, weighted projection and Poisson/Hessian tests remain independent authorities.
An initial test-only empty-membership indexing error was corrected; its failed
log and an earlier missing-toolchain invocation are preserved.

All 54 public PPML states, 36 baseline-anchored pairs and 18 direct stock pairs
pass scientific audits. All 18 native pairs have exact coefficient, covariance
and mean exports and unchanged iterations/fallbacks. Tracked peak deltas are
-240 to -120 bytes, scratch unchanged, selected telemetry zero to +48 bytes.
Preparation storage removal does not imply a comparable whole-command peak drop.
No new OLS public clocks, cold-start, warmed-allocation or physical-cap campaign
is claimed. Early rejection makes those unnecessary for production acceptance.

## Next boundary

Close this narrow ownership experiment. Retaining a second finalization route
for two milliseconds would not materially advance the remaining objective.
Continue with a coherent weighted-frame lifecycle or materially different block
solve whose measured cost budget can remove at least ten public milliseconds.
Shared small changes can accompany that implementation when they simplify it;
do not begin another isolated preparation or canonical-matvec tuning cycle.

Prior OLS timing losses, categorical three-FE scientific discrepancies,
allocation-inventory drift, constrained-fit failures and platform deferrals
remain open. The broad multi-FE <=0.90-stock goal remains active and unfinished.
