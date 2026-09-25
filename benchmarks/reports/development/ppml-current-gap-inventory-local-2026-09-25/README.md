# Current PPML gap inventory — 2026-09-25

This closes the already launched four-case baseline and informs the revised
implementation plan in STATUS.md. No runtime candidate was built or adopted.
Production remains **7cd9c579 / pairedscan2**. All **565** captured computational
inputs match that source; plugin SHA-256 is
`4d4ae065abac8b9250c8f9218172ea6853f9b3d0e90a869e702a47ce53dc3020`.
The pinned, unmodified stock fixest archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.

Three alternating native/stock repetitions use existing development seed
**202609347**, T8, robust or two-way clustered inference as declared, and fitted
means. Stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`. Imports and scientific
exports are outside the clocks; estimation, inference and requested means are
inside. No compilation, profiling, audit, archiving or competing fit accompanied
the headline runs. All 24 attempt monitors are clean, application results pass,
and the controller records its terminal sentinel.

| Case | Native ms, repetition order | Stock ms, repetition order | Median ratio | Required native time for 10% lead |
|---|---|---|---:|---:|
|Three unbalanced FE, N153600, finite K19 factor cross, low, two-way clusters|1080, 1089, 1096|725, 709, 717|1.519|645.3 ms|
|Same dimensions/topology, K19 numeric, low, two-way clusters|677, 673, 674|654, 634, 631|1.063|570.6 ms|
|Factor case, fast storage|966, 960, 978|719, 716, 708|1.349|644.4 ms|
|Two FE, N983040/K6, fast, robust|1258, 1230, 1258|886, 864, 878|1.433|790.2 ms|

All **24 public-state checks** and **12 direct native/stock pairs** pass the
existing audit, including sample, coefficients, covariance, objectives and means.
These large cells did **not** run new independent explicit-design WLS/Poisson
oracles. Earlier independent surface/oracle evidence belongs to its earlier
report. An implementation candidate still requires focused independent checks.

Native phase medians, milliseconds:

| Case | Preparation | Projection | Finite | Line search | Strict final | Inference | Publication |
|---|---:|---:|---:|---:|---:|---:|
|Low factor|102.6|435.8|40.0|95.2|237.0|38.8|23.2|
|Low numeric|76.7|234.7|33.8|13.9|194.9|38.6|23.0|
|Fast factor|98.2|403.5|50.9|95.6|166.3|12.9|19.8|
|Million rows|233.0|246.7|116.0|51.8|271.4|6.7|122.3|

These are recorded phase labels, not a causal attribution or an exhaustive sum
of the public timer. The numeric comparison changes the mathematical controls
and resulting weighted frames; its difference cannot all be charged to factor
evaluation. Frozen identical-frame experiments are the next attribution step.
Fast storage removes only 123 ms from the factor median and still misses the
target by 322 ms; replacing scratch storage alone cannot close this observed gap.
The million-row gap persists on current computation and requires about 468 ms.

Source inspection confirms that direct canonical products admit exactly two
categorical FE, while three-FE retained design construction follows block
projection. `JointOriginalColumns` binds numeric and simple categorical columns,
but interactions still use the general point evaluator. Scratch block reads and
writes already use bounded complete-row tiles. These facts identify experiments;
they do not establish the dominant cost or justify blindly removing a guard.

The [receipt archive](receipts.zip) includes the declaration, result identities,
timings, scientific audits, input hashes and protocol-only logs. All **531**
archived files passed restoration, hash and privacy checks; see
[verification](archive-verification.json). SHA-256:
`33f3e00a79858048662ec8f7071f9d07b04b077c8fafffbe72b9fa045ff2e1e0`.
It excludes binary data, row-level means, licensed logs and machine paths.
The twelve omitted native output CSVs retain original/sanitized hashes in
`omitted-output-hashes.json`; full local evidence is preserved. Generated inputs remain reproducible from
the captured declaration and committed harness. No new implementation tests,
physical-memory cap, Linux/SCC run, qualification, push or release is claimed.
Historical output/DoF discrepancies, allocation inventory drift and constrained
solver failures remain open. The next implementation and acceptance gates live
only in STATUS.md.
