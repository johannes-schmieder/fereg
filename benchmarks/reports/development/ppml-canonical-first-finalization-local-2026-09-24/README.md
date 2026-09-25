# Canonical-first PPML finalization: local development evidence

Accepted local milestone, 2026-09-24; **not qualification**. The >=2-FE public
PPML target of <=0.90 times comparable pinned stock remains unfinished.
[ADR 0177](../../../../docs/adr/0177-canonical-first-ppml-finalization.md) records
verification, numerical refusal and ownership. No frozen 77/16/111 definitions,
scientific tolerances, samples, rank/DoF, weight semantics or separation changed.

## Source and immutable receipts

Baseline **fastproducts1**, plugin SHA-256
`96156a9c3970c2001d4e04795ef9e82eade204622e3eae3ce315f5b0ec3c1d3e`, has all
551 captured inputs matching **3d3ef6e85fac956e37b7f13c4f44a2062b2f5f8c**.
Accepted **canonicalfinal1**, plugin SHA-256
`d653f5b858554ed237623de828624b6a88d638cecfc40653350bbb344e54a4ac`, captures that
base plus ppml/prepared_irls.rs and ppml/prepared_estimator.rs. All **551 captured
inputs match final computational source**. No production changes follow the fresh
case declaration. The fourth-slope harness extension and new licensed fixture are
separately captured; neither changes an existing benchmark input or plugin.

Diagnostic **multifrontier1** instruments the committed baseline only; plugin
`04e2a10b0a63e39f9d2b4db38f83c18238880fb28b78afa59ee18746afe293db`.
Diagnostic timings never enter stock-comparator headline tables. Source hashes,
patches, raw spans, build/ABI receipts, controllers and failed probes are preserved.

Apple Silicon, Rust 1.85.1 release arm64 plugin, licensed Stata 19 MP8. The pinned
unmodified fixest 0.14.2 uses matched native threads, BLAS1, glm.tol=1e-10 and
fixef.tol=1e-11. Reference archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.

[Receipt archive](receipts-content-addressed.zip): **10,110 logical receipts /
2,535 payloads / 23,175,058 bytes**, SHA-256
`b48b4bec02d0a3c98998a4c1f1b2645942d580ee8739f2654e2c697d57f94e8d`.
Use `restore_receipts.py NEW_DIRECTORY` to verify and restore every logical file.
Licensed headers, machine paths, data and binaries are omitted or sanitized;
omitted file hashes and exact data-generation recipes remain. Work root:
`target/ppml-multife-frontier-20260924`.

## Current-source opportunity and attribution

Before editing, three rotations of baseline/stock on development seed 202609347
establish the remaining opportunity. Robust inference and saved means throughout.
The numbers below apply to the captured baseline, not later revisions.

| Baseline inventory | fereg / stock median ms | Classification |
|---|---:|---|
| Two FE low, N21120/K2/T4 |40 /31|Measured loss|
| Disconnected two FE low, N42240/K10/T8 |110 /76|Measured loss|
| Three FE dense fast, N61440/K10/T8 |311 /503|Unranked coefficient discrepancy|
| Same, low |397 /500|Comparable and competitive|
| Three FE dense fast, N61440/K64/T8 |1026 /2366|Comparable and competitive|
| Same, low |1411 /2372|Comparable and competitive|
| Three FE unbalanced slopes fast, N61440/K10/T8 |258 /231|Measured loss|
| Same, low |273 /230|Measured loss|

The dense fast K10 case misses the frozen stock coefficient envelope in all three
pairs: maximum absolute error 2.122205e-10, maximum tolerance ratio 1.44186,
two coordinates. Scores, model space, means, covariance and objectives pass.
Its stock comparison remains unranked. No inference from historical categorical
wins or the much older five-to-eightfold slope gap overrides this inventory.

An isolated six-application baseline profile uses two repeats of each case.
Nested spans are unioned; inclusive and exclusive costs are preserved, not added
twice. All six diagnostic public states pass independent public-state checks.

| Diagnostic case | Native plugin ms | Canonical preparation ms, inclusive | Ordinary joint boundary ms | Numerical recurrence ms, excluding reconstruction |
|---|---:|---:|---:|---:|
| Large three-FE slopes, fast |247.48–249.34|77.09–78.92|90.58–90.69|32.25–32.29|
| Same, low |259.47–263.87|92.45–94.68|87.28–88.38|28.71–28.82|
| Two slope partitions, N30720/K6/T4 fast |109.99–110.24|21.88–21.94|48.76–49.60|5.06–5.16|

Geometry preparation aggregates approximately 15 ms in the large slope cases;
canonical residual absorption approximately 12.8 ms. These are nested costs, not
additional additive rows. The evidence points to whole-fit verification and
construction as well as recurrence. The chosen change removes a redundant strict
joint solve when one fully certified canonical Newton checkpoint already confirms
the point and can provide exact-current-weight inference products.

## Accepted mechanism and rejected experiments

Two accepted objective/score passes still precede strict verification. A strict
canonical Newton checkpoint independently absorbs current original inputs,
checks rank, finite solve, inverse and reconstruction, and compares predictor,
coefficients and objective. When it confirms the point, retain that accepted
point and reuse its exact-weight canonical products only after the independent
weight/scale/generation/q*mu/reconstruction checks. The final score remains fresh.

A moved checkpoint revokes this optional route for the fit, then follows the
existing accept/backtrack/resume transaction. Subsequent eligible finalization
uses the existing joint strict witness plus canonical predictor solve. The old
fallback remains necessary; no threshold, additional recovery budget or public
solver option is introduced. Ordinary directions and line searches are unchanged.

A prior generalization of conditional-intercept trials to slopes and 3+FE passed
its analytic group-optimum checks but failed an extreme-component strict probe.
Revocation only at canonical rebuild did not repair it. A checkpoint-only version
without numerical refusal then reproduced the problem: at 1e13 component weight
contrast and requested outer/score tolerance 1e-12, canonical Newton reconstruction
stagnated near 1.07e-10 movement while the established route converged below the
unchanged 1e-10 truth bound. The accepted early refusal/resume policy passes that
same fast T1/T4 boundary, including stale-product and corrupted-predictor controls.
These failures are retained; the broader trial update is not adopted.

The first default-tolerance low probe also misses a newly imposed 1e-10 truth bound
in both baseline and candidate with identical 1.0069622913e-10 error. The stricter
requested-accuracy experiment does not erase or repair that baseline observation.
No production tolerance or existing test envelope was relaxed.

## Whole-public distributions

Generation and separate warmup are outside timing; loading, estimation, all
required verification, inference and saved means are included. Baseline/candidate/
stock order rotates. All reported completed calls have clear captured isolation
monitors and no application failure, timeout or monitor error. No builds, profiles,
audits, archives or competing fits overlap headline runs. This does not establish
absence of ambient operating-system load.

Development seed 202609347, three rotations, median B/C/stock ms:
259/235/232, 273/251/231, 120/115/120, 123/118/120, 315/258/502,
100/91/81, 51/50/54, 40/40/29. The first four are large three-FE slopes
fast/low and two-FE slopes fast/low. The fifth dense categorical stock comparison
remains unranked; all native baseline comparisons pass. The final two are existing
two-categorical-FE controls. Full distributions and paired outcomes are in receipts.

Fresh seed **202609398**, five rotations. Fast unless stated. Median [min–max] ms.
Native losses count candidate times above the paired baseline; ties are not wins.

| Case | Baseline | Accepted | Stock | Accepted/stock | Native losses |
|---|---:|---:|---:|---:|---:|
|0|324 [321–326]|296 [293–300]|351 [350–356]|0.843|0/5|
|1|367 [364–372]|347 [338–350]|352 [351–357]|0.986|0/5|
|2|134 [133–136]|124 [123–126]|168 [162–169]|0.738|0/5|
|3|117 [115–118]|112 [112–117]|133 [132–134]|0.842|0/5|
|4|62 [62–63]|60 [60–61]|1456 [1452–1457]|0.041|0/5|
|5|160 [159–162]|142 [140–145]|536 [533–536]|0.265|0/5|
|6|194 [191–198]|173 [169–177]|774 [773–788]|0.224|0/5|
|7|220 [219–220]|203 [202–206]|452 [449–453]|0.449|0/5|
|8|36 [36–37]|37 [34–38]|22 [19–23]|1.682|3/5|
|9|43 [42–43]|43 [43–43]|30 [25–31]|1.433|2/5|
|10|218 [216–219]|209 [206–211]|781 [776–785]|0.268|0/5|

0. Three FE, unbalanced, slopes on g1/g2, N65280/K12/T8.
1. Same, low memory.
2. Three FE, unbalanced, slopes on g1/g2, N28800/K8/T4.
3. Two intercept-plus-slope partitions, N28800/K8/T4.
4. Weak barbell two-partition slopes, low, N17280/K6/T4.
5. Three FE dense categorical, N40320/K10/T4.
6. Three FE disconnected categorical, N23040/K12/T4.
7. Three FE unbalanced categorical, low, N33600/K28/T4.
8. One-FE categorical excluded control, N21120/K6/T4.
9. Narrow two-FE categorical excluded control, N21120/K2/T4.
10. Two partitions, two slopes in each, N23040/K12/T1.

All nine changed fresh cases improve in every pair; median native gains are
3.2–11.3%. Eight of ten selected >=2-FE medians reach the 10% stock lead. This is
not a package-wide win rate. Low large slopes are only 1.4% ahead of stock; narrow
two-FE loses every stock pair. The excluded one-FE control loses three native pairs
and all stock pairs, with a 1 ms median increase; narrow has two native paired
losses and unchanged median. Do not attribute these unchanged-path fluctuations
to an algorithmic gain. OLS control medians are 64→60 and 41→41 ms, with all six
pairs' exported scientific files identical; individual timing losses remain visible.

## Validation and memory

- **148 distinct focused Rust tests pass** in the final source. The existing
  `refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model` assertion
  expects one strict rejection but sees zero. An isolated build of the accepted
  baseline reproduces it. Its assertion is unchanged; the new mathematical check
  verifies both memory routes at the original truth bound. Earlier exploratory
  test failures and one review-script key error are retained.
- **Six licensed science fixtures and two real sixteen-iteration allocation
  audits pass**, including two-/three-partition slope explicit Poisson/Hessian/
  sandwich checks, FW replication, PW scores, cluster inference, offset, low/fast,
  saved effects, categorical PPML and three-FE OLS. Both T1/T4 allocation audits
  report zero events, zero growth and fresh/warm equivalence.
- All **300 public states** across inventory, profile, development, fresh and
  small-oracle screens pass their own score/model-space/objective checks. All
  **82 candidate/baseline pairs** pass. All **55 fresh direct stock pairs** pass.
  Three development stock pairs remain unranked, as do the three earlier inventory
  pairs for that same design. No estimator application failed or timed out.
- Fresh seed **202609399** supplies three bounded explicit-design Poisson/Hessian/
  sandwich/mean oracles. All **six fereg states** pass. Stock fails covariance in
  the disconnected categorical oracle: max absolute error 4.19594e-10, maximum
  tolerance ratio 2.00636, nine coordinates. Its own public-state checks and other
  oracle quantities pass. That direct stock pair is unranked; no tolerance changes.
- Fresh seed **202609400** supplies the two OLS controls, twelve applications and
  six exact scientific-output comparisons. These are focused regression checks,
  not a full OLS campaign or qualification.

No heap workspace or modeled memory allowance is added. Modeled and tracked peaks
are unchanged in every fresh case. The development dense categorical case increases
tracked peak by 1,766,560 bytes with unchanged modeled peak; this is an existing
product-lifetime tradeoff, and its stock timing remains unranked. No RSS or physical
limit claim, static inventory repair, or cross-platform qualification is made.

The initial fresh launch rejected unsupported slope name z4 before generation or
fits. Extend the existing bounded harness allowlist to z4; retry the identical
predeclared cases. The first OLS launch similarly refused the local wrapper's PPML
flag before fits; the corrected wrapper preserves its inputs. Both attempts are
archived. An initial OLS declaration prose typo said robust while its unchanged
inputs requested default IID; the correction and original prose are preserved.
Seeds 202609398/399/400 and all previous validation seeds are frozen, not tuning data.

## Remaining opportunity

The large development fast slope case is still 235 versus 232 ms: reaching 0.90x
requires another 26.2 ms (11.1%) saving. Its measured native phases are preparation
31.7 ms, FE projection 84.0, finite solves 17.6, objective/line search 11.9, strict
final 43.1 and native publication 9.8. Strict final fell from 65.9 ms, while the
ordinary phases and five outer iterations stayed essentially unchanged. Inner
reported counts fall 83→67, but counts across algorithms are not comparable work.

Fresh low large slopes need another 30.2 ms (8.7%) to reach 0.90x stock. Narrow fast
two-FE needs 16 ms (37.2%). Prior baseline profiling already shows its numerical
recurrence is a small fraction of whole-fit cost. Prioritize shared original-input
statistics, current-frame geometry ownership and reconstruction/certificate scans
at ordinary and canonical boundaries. Retain independent acceptance; measure which
passes can supply multiple required quantities once. A recurrence-only tuning loop
cannot close that narrow gap. Small disconnected low execution remains a measured
3d3ef6e8 inventory loss and needs candidate-source remeasurement before a new claim.

Broader conditional trials may merit a materially redesigned experiment with
explicit strict-refusal ownership; this report does not establish their safety or
speed. Do not repeat the exact rejected probe or infer a general impossibility from
it. Categorical wide/three-FE cases already competitive should remain controls.
Known allocation-inventory drift, weighted constrained/nonretaining-CG failures,
other historical comparator discrepancies, SCC access and platform/thread/physical-
memory gaps remain open. No remote mutation, stopped campaign or release action.
