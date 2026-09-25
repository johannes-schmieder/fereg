# Deferred two-FE PPML Schwarz — local development, 2026-09-24

Adopt the deferred two-FE factor lifecycle in [ADR 0178](../../../../docs/adr/0178-deferred-two-fe-ppml-schwarz.md).
This is a bounded implementation milestone, not completion of the multi-FE
PPML <=0.90-times-comparable-stock objective or a qualification claim.

## Source and execution

Baseline `canonicalfinal1`, plugin SHA-256
`d653f5b858554ed237623de828624b6a88d638cecfc40653350bbb344e54a4ac`,
matches all 551 captured inputs at committed `ab21c9b3`. Its original captured
base is `3d3ef6e8` with the two ADR-0177 overlays; those identities are preserved.
Accepted `lazygraph2` captures `ab21c9b3343d49d0b25f9d5afd84becfb9b07863`
plus `crates/fereg-core/src/ppml/categorical_lsmr.rs`. All **552 captured inputs
match final computational source**. Plugin SHA-256:
`91e59cd5fee3413d9e12eaf47494c1ec374e43a50352db8a89ed57f8d9a108bc`.
The new licensed fast-LSMR oracle is captured separately as a recipe.

`framestats1` is an isolated instrumented baseline, SHA-256
`458f6f7b3d0d335d65fe87c5aa2860e44ec6f73a9ff2a0a859e9535c9e93bf35`.
Its eight applications are diagnostic only. Rejected `lazygraph1`, SHA-256
`9fb9dc6dd5c8b4e39051334725f45b7d7c1ea54316bf083b7ea9337336e997c6`,
has no headline timings. Its failed strict test and sources remain evidence.

All stock runs use the pinned unmodified fixest archive and strict comparable
Poisson configuration, `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads,
robust inference and fitted means. Data/loading setup is outside the command
clock identically to the preserved harness. Native command loading, estimation,
requested-output construction and publication are inside the native clock.
Stock and native internal phases are not equated by their names.

Development seed202609347 was already a tuning seed. Fresh seeds
**202609401/402/403 are now consumed and frozen**. Declarations precede fits.
No concurrent build, profile, archive, audit or competing fit occurred during
headline runs. Every application and isolation monitor passed. Local sampled
monitoring does not establish absence of all ambient OS variation.

## Diagnosis and implementation

The profile verified existing reuse: the joint LSMR owner already retains the
Schwarz factor across weight frames; its pinned `within` clone is O(1).
Creating the first factor dominated the remaining disconnected fast-mode solve.
The instrumented baseline attributes about 15.2 ms of a 99.6-ms native call to
preconditioner preparation. Other significant costs are canonical construction,
family evaluation, original-input checks, loading and publication. Full nested
and exclusive span distributions are preserved; they are not additive headline
phase estimates.

The candidate uses the existing current-weight diagonal before a factor exists.
Existing staleness, recurrence exhaustion or fresh certificate refusal promotes
to Schwarz. Promotion restarts from zero; a preexisting factor retains its old
warm continuation and refresh policy. This adds no benchmark selector, workspace,
new solver layer, tolerance change or acceptance shortcut.

Public development disconnected receipts directly measure joint setup
**16.96→0.83 ms**, recurrence **8.62→9.60**, certificate **3.37→3.16**.
Joint Krylov iterations rise 10→16; outer iterations stay four. Whole-public
median falls **111→96 ms**. Fresh disconnected setup falls **7.99→1.02 ms**
and recurrence grows **8.63→10.80**. Its net improvement is smaller.
These are measured phase changes; attributing every whole-command fluctuation
to preconditioning would be unwarranted.

## Current-source coverage and distributions

Current inventory remeasured narrow two-FE fast/low and disconnected fast/low,
three baseline/stock rotations: **43/31, 40/30, 111/77, 108/77 ms**.
All twelve comparisons passed. Thus those losses were current-source evidence,
not claims transferred from older artifacts. The focused candidate screen and
fresh controls below extend this inventory; it is not an exhaustive matrix.

Values are median [minimum, maximum] milliseconds. B=baseline, C=accepted
candidate, S=comparable stock. Native pair losses count repetitions with C>B;
ties are not losses. Stock discrepancy cases retain raw distributions and are
explicitly unranked. All cases are two categorical FE and fast memory unless
specified. All request robust inference and fitted means.

### Development: three rotations

| Case | B | C | S | C/S | Native pair losses |
|---|---:|---:|---:|---:|---:|
| N21120/K2/T4 | 43 [43, 44] | 41 [40, 41] | 30 [30, 33] | 1.367 | 0/3 |
| Same, low | 41 [40, 42] | 41 [40, 41] | 30 [30, 30] | 1.367 | 1/3 |
| Disconnected N42240/K10/T8 | 111 [108, 112] | 96 [94, 100] | 77 [76, 78] | 1.247 | 0/3 |
| Same, low | 110 [109, 110] | 110 [108, 110] | 77 [74, 78] | 1.429 | 0/3 |
| N30720/K10/T1 | 97 [91, 98] | 92 [89, 95] | 198 [193, 200] | 0.465 | 1/3 |
| N30720/K40/T4 | 128 [127, 129] | 126 [125, 127] | 317 [317, 322] | 0.397 | 0/3 |
| Weak barbell N17280/K6/T4 | 37 [36, 38] | 39 [39, 39] | 909 [909, 914] | 0.043 | 3/3 |
| Three FE unbalanced N30720/K6/T4 | 90 [90, 93] | 91 [90, 96] | 82 [81, 82] | 1.110 | 1/3 |

### Fresh: five rotations, seed202609401

| Case | B | C | S | C/S | Native pair losses |
|---|---:|---:|---:|---:|---:|
| N23040/K2/T4 | 45 [44, 47] | 44 [43, 45] | 32 [30, 33] | 1.375 | 1/5 |
| Disconnected N46080/K12/T8 | 113 [109, 116] | 107 [106, 111] | 96 [89, 97] | 1.115 | 1/5 |
| N34560/K10/T1 | 108 [101, 110] | 93 [90, 95] | 209 [207, 210] | 0.445 | 0/5 |
| N38400/K32/T4 | 131 [130, 134] | 127 [126, 128] | 281 [280, 286] | 0.452 | 0/5 |
| Weak barbell N19200/K8/T4 | 43 [42, 46] | 43 [43, 57] | 926 [925, 932] | 0.046 | 2/5 |
| Unbalanced N26880/K6/T4 | 57 [56, 57] | 55 [54, 57] | 41 [36, 45] | 1.341 | 1/5 |
| Sparse N28800/K6/T4 | 132 [126, 139] | 133 [132, 133] | 7537 [7517, 7544] | unranked | 2/5 |
| Three FE unbalanced N33600/K8/T4 | 98 [95, 99] | 97 [96, 99] | 165 [164, 167] | 0.588 | 2/5 |
| N23040/K2/T4, low | 45 [43, 46] | 44 [43, 45] | 33 [29, 34] | 1.333 | 2/5 |
| Three FE unbalanced slopes N65280/K12/T8, low | 338 [336, 365] | 338 [336, 340] | 349 [344, 354] | 0.968 | 3/5 |

Fresh serial improves 13.9%, wide 3.1%, disconnected 5.3%, unbalanced 3.5%,
and narrow 2.2%. The narrow outer count rises three→four, while serial falls
four→three; recurrence changes can affect outer progress even at unchanged
accuracy. The fresh weak median is unchanged but one candidate call is 57 ms,
versus the baseline range 42–46. Development weak loses every pair and 2 ms
at the median. The small disconnected and weak oracle calls lose 1 and 2 ms
(13→14 and 15→17); these single calls are science controls, not distributions.
No threshold tuning follows these results.

Three-FE, low-memory slopes and low categorical controls keep their algorithms,
iteration counts and modeled/tracked peaks. Individual timing losses remain
visible. OLS seed202609403 has twelve successful calls and six exact scientific
export pairs: three-FE sparse 57 [57,58]→58 [57,59] ms; two-FE strong
42 [41,42]→42 [42,44]. This is not a universal non-regression claim.

## Independent science and failures

All **260 exported PPML states** (including eight diagnostic applications) pass
independent current score, model-space and objective replay checks. All **76
candidate/baseline pairs** pass. Of 76 direct candidate/stock comparisons,
**71 pass and five fresh sparse pairs remain unranked**. The same sparse
baseline/stock pairs fail fitted means: max absolute difference 1.0720e-5,
tolerance ratio 53.3332, 101 coordinates. Coefficients, covariance, likelihood,
deviance and each estimator's own state checks pass. We have not established
which mean state is more accurate; no timing ranking or relaxed envelope is used.
All development comparisons and both small direct stock comparisons pass.

The six small explicit-design Poisson/Hessian/sandwich/mean states (two designs,
B/C/S) pass unchanged oracles. **135 distinct focused Rust tests pass** across
categorical LSMR, prepared IRLS and prepared estimator boundaries. The inherited
`refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model` assertion
still reports zero versus one strict rejection; its old baseline evidence is
preserved and the assertion is unchanged. This is not a wholly green Rust suite.

Six licensed fixtures pass with explicit process/Stata codes and terminal
sentinels: two- and three-partition slope oracles, extreme component truth,
low categorical canonical-product oracle, the new fast categorical LSMR oracle,
and three-FE OLS batch oracle. They cover independent weighted Poisson/Hessian/
sandwich science, frequency multiplicity, probability weights, clustered scores,
requested predictions/reconstruction and T1/T4. The new fast fixture closes an
important coverage gap: the existing categorical fixture explicitly used low MAP.

The first candidate kept the diagonal iterate during promotion and failed
`fast_categorical_lsmr_strict_final_preserves_truth_and_canonical_retry` with
uncertified normal equations. The cold-promotion successor passes its original
mathematical envelope and all fallback modes at T1/T4. An initial unit assertion
also assumed an eager factor; the test now covers both deferred construction
and reuse of an explicitly prepared factor without changing its accuracy bounds.

Before fitting, one declaration preparation used a nonexistent design key and
failed its assertion; the corrected declaration uses the existing barbell
augmentation. Two build launches failed before compilation (old Python's missing
`tarfile` filter; missing Cargo PATH). The isolated corrected build passed its
ABI harness. Derived-summary scripts initially rejected missing Stata cells and
an unavailable timing median; corrections preserve nulls and original receipts.
These are retained tooling failures, not discarded application timings.

## Memory

Modeled peaks are identical in every pair: complete fallback storage remains
reserved. No additional allocation owner is introduced. Development disconnected
tracked peak falls **111,665,598→33,767,374 bytes**. Fresh serial falls
37,930,909→25,263,469; wide 75,904,516→61,395,428; disconnected
43,931,964→40,486,404. Fresh weak instead rises **12,279,474→12,286,258 bytes**;
the admitted promotion overlap retains the pilot result. Sparse and excluded
controls are unchanged. Full per-case/per-repetition peaks are in the receipts.
These are native allocation measurements, not RSS or constrained physical-memory
qualification. Static allocation inventory drift remains unresolved.

## Remaining opportunity and stop decision

Retain this small lifecycle change and stop tuning the diagonal pilot. It avoids
substantial unnecessary factor work on several independent cases, with no new
storage or acceptance machinery. It does not make the remaining losses inevitable.

Fresh narrow needs another **15.2 ms (34.5%)** to reach 0.90× stock; disconnected
needs **20.6 ms (19.3%)**; unbalanced **18.1 ms (32.9%)**; low narrow
**14.3 ms (32.5%)**; low large slopes **23.9 ms (7.1%)**. Relevant candidate phase
medians, in ms:

| Case | Preparation | FE projection | Finite | Objective | Strict final | Publication |
|---|---:|---:|---:|---:|---:|---:|
| Narrow |5.45|6.70|3.23|3.51|4.11|3.83|
| Disconnected |14.90|25.74|8.65|7.08|14.24|7.70|
| Unbalanced |6.90|8.33|4.25|4.16|7.59|4.45|
| Low large slopes |34.96|138.55|22.85|15.10|74.88|11.99|

Next investigate a shared current-frame owner for original-input weighted
statistics and reconstruction, and parallel family row evaluation with ordered
reductions. Those costs cross ordinary solves, canonical checks and final output,
and may benefit low slopes as well as categorical fits. Reuse must respect exact
sample, basis and weight generations; independent fresh acceptance stays mandatory.
Start from these phase budgets, not another blind recurrence threshold change.

Known weighted constrained-fit/nonretaining-CG failures, older comparator/reporting
issues, static inventory drift, Linux/thread/physical-memory confirmation and SCC
access remain open. Frozen 77/16/111 definitions and prior receipts are unchanged.
No push, release, SCC retry, stopped campaign or full qualification was performed.

## Receipts

The content-addressed archive contains source identities, exact captured source,
predeclared cells, controller/process receipts, scientific exports, audits,
profiles, distributions, tests and rejected attempts. Raw data/binaries and raw
licensed logs are omitted with hashes; sanitized terminal protocols are retained.
Use `restore_receipts.py` to verify and restore the archive. Archive identity is
recorded in `receipts-content-addressed.sha256` and STATUS after verification.
