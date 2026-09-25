# Multi-partition PPML topology transfer: rejected development prototype

Date: 2026-09-25. Decision: reject `partitiontransfer1` under
[ADR 0202](../../../../docs/adr/0202-ppml-topology-transfer-closeout.md).
Retain **c7c4c473 / controlplane3**. This bounded experiment does not complete
the multi-FE <=0.90-times-stock objective.

## Identity and work

The candidate is a four-Rust-file overlay on c7c4c473. Its plugin SHA-256 is
`50405bf84bfc174858eb2accad8fb1e54b1472430c634cf71070b7b6c4b62910`.
The accepted baseline native plugin remains
`1c598ce6ecbdea8e1a6e0ffcc9dfcd655c89165750a8d2b02d8189ec15b9ae1e`.
Both artifacts have 564 captured computational inputs; full identities,
source snapshots and the candidate patch are archived. All accepted inputs
match after removing the prototype, as recorded in restored-source-verification.json.

The candidate moves category code and membership arrays from availability
preparation into finalized topology for multi-partition PPML with single-source
FE recipes and unchanged generation rows. The separation coordinator retains
sample authority. Source/registry order, aliasing and independent topology
validation remain. It does not cache weighted geometry or change numerical work.
Interactions, one-FE models and public OLS retain their prior preparation routes.

Pinned unmodified stock fixest is 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable calls use glm.tol=1e-10 and fixef.tol=1e-11, robust inference and
fitted means. Requested threads match. Public Stata19 clocks include parsing,
estimation, all acceptance, inference and requested publication; startup/import
are excluded and each clock follows an estimator warm-up.

## Predeclared development pilot

Seed **202609347** is a reused development seed, not fresh validation. Three
alternating B/C/stock rotations; all 54 calls complete and sampled monitors report
no overlap/error. Build and science fixtures finish before the clocks; scientific
audits and archiving follow them. No profile or competing fit accompanies clocks.

| Case; N/K/T; mode | Baseline / candidate / stock medians ms | C/stock | Paired W/L/T |
|---|---:|---:|---:|
|Disconnected categorical; 61440/6/4; low|108 / 106 / 99|1.071|3/0/0|
|Narrow categorical; 24960/2/4; fast|37 / 38 / 33|1.152|0/1/2|
|Wide categorical; 23040/36/8; low|83 / 82 / 127|0.646|1/1/1|
|Two slope partitions; 30720/6/8; low|71 / 71 / 81|0.877|2/1/0|
|Unbalanced three-FE slopes; 61440/10/8; low|179 / 179 / 234|0.765|1/1/1|
|Three slopes on two partitions; 38400/8/4; low|134 / 134 / 271|0.494|0/0/3|

Full vectors, in repetition order:

| Case | Baseline ms | Candidate ms | Stock ms |
|---|---|---|---|
|0|107,108,112|104,106,106|96,99,99|
|1|37,38,36|38,38,36|31,33,34|
|2|83,83,81|84,82,81|127,129,124|
|3|71,70,73|70,71,71|81,80,81|
|4|179,185,179|174,192,179|231,237,234|
|5|134,134,134|134,134,134|270,271,272|

**Seven wins, four losses, seven ties.** No new family reaches 0.90 stock.
The two-ms large categorical gain is insufficient relative to the remaining
whole-command deficit, narrow loses one ms, and slope medians are unchanged.
The 192-ms candidate call remains visible. Reject without another correction,
profiling cycle or fresh validation seed. Do not pool this pilot with ADR0201's
fresh seed434, which remains the accepted-source headline comparison.

## Science, memory and limits

All **24 distinct Rust tests** and **six licensed independent fixtures** pass.
The latter cover iterative canonical explicit-design Poisson/Hessian, two- and
three-partition slope oracles, scientific contracts, weights/inference and
separation/outputs. Controller return codes, application statuses and fixture
sentinels agree. Rust tests additionally cover ownership without copying,
reordered/duplicate effects, empty/malformed axes, reservation refusal,
availability samples, generation rollback, weighted projection and solver bounds.

An initial missing-cargo invocation exits127. The first correctly invoked
topology run passes twelve tests and fails the new test's empty-N pointer probe:
the test indexes nonexistent group zero. It is corrected to checked membership;
all thirteen topology tests then pass. This test-only failure and all attempts
are preserved, rather than relabeled successful.

All **54 PPML public states**, **36 baseline-anchored pairs** and **18 direct
candidate/stock pairs** pass independent score/model-space/objective and numerical
audits. All **18 native pairs** have byte-identical coefficients, covariance and
means, with unchanged outer/inner iterations and fallback counts. Scientific
discrepancies would be unranked; none occurs in this block.

Tracked native peak changes are **-240 to -120 bytes**, scratch is unchanged,
and selected-memory telemetry changes **zero to +48 bytes**. This is not physical
RSS evidence or proof of the temporary preparation peak reduction. The prototype
transfers N-sized buffers and adds only model-sized ownership descriptors.
No new OLS public timing, cold-first-fit, warmed-allocation or physical-cap
campaign was run after the unsuccessful pilot. Existing OLS timing concerns and
all unrelated scientific/platform gaps remain open.

## Receipts

`receipts-content-addressed.zip` stores **2,654 logical receipts** in **1,000
unique payloads**, with original/sanitized hashes and a container checksum.
`restore_receipts.py DESTINATION` verifies and restores every logical entry;
all entries have been restored and checked. The archive contains both source
snapshots, declarations, controllers, build/test logs, fixture protocols,
per-call identities, distributions, audits and summary. Licensed raw logs,
machine paths, data and binaries are excluded or sanitized; omitted binaries
and data retain hashes. Privacy/hash verification is recorded alongside it.

The runtime is restored to the accepted source, with no dormant transfer route.
Frozen 77/16/111 definitions and consumed validation seeds are unchanged. No
push, release, SCC retry, stopped campaign or qualification run occurred.
