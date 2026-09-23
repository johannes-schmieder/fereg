# Single-frame PPML slope finalization: accepted development milestone

Local macOS arm64 evidence, September 23, 2026. Accept `jointfinal2`, based on
`00e9b30c`, under [ADR 0131](../../../../docs/adr/0131-single-frame-ppml-slope-finalization.md).
All **536 computational inputs** match candidate plugin
`96bea000c5ca47fa0176ae37302b793fec13470efbcbbf657b7446e7cf99506b`.
Baseline `jointprod2` is the accepted computational source at `4527694e`, plugin
`5ca6e1a81a548aef3c733146f04929b68da849493c35574a0361973a94aa8730`.

This changes strict/final computation, with **3.5–7.2% fresh native gains** and
unchanged median tracked/modeled peaks. It does not complete the whole-command
0.90-times-stock objective. Several important cases still lose, and one fresh
stock fitted-mean comparison is scientifically unresolved and unranked.

## Adopted computation

After two objective/score convergence passes, use the existing joint finite/FE
engine at strict tolerance as a current-weight Newton witness. Its fresh FE,
original finite-score and reconstruction certificates remain mandatory. If its
coefficients, predictor and objective agree with the accepted state, keep that
state fixed and construct canonical rank, within design, finite solve and bread
once using the final predictor response at exactly the same Hessian weights.

Canonical rank/basis and final predictor/coefficients/objective checks remain
independent. The final Hessian must match `q * mu` exactly. An unaccepted witness
or transaction returns to the existing canonical checkpoint/resume policy.
No joint-only output gains inference authority. Exact owned weight bits, every
bread-scale field and sample/topology generations govern geometry relabeling;
revoked authority cannot be revived. No new numerical cache or heap bank is added.

This removes one full canonical construction. It is distinct from ADR 0128's
rejected same-point/response-history probes, which retained repeated finite
construction. It also leaves the ordinary joint algorithm unchanged. Low memory,
categorical, one-FE and other solver families do not use the strict shortcut.

## Public rotations

Reference: unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
`glm.tol=1e-10`, `fixef.tol=1e-11`, matched application thread counts and
single-thread BLAS. PPML work includes robust covariance and fitted means under
the established comparable normalization. The whole public command includes
loading/preparation, estimation, inference and publication; process startup,
import, warmup and exports are outside the timer. Native low versus ordinary
stock is not a matched-memory comparison.

Three rotating baseline/candidate/stock applications per main cell. Values are
median [minimum, maximum] milliseconds. No build, profile, archive, independent
oracle computation or assistant-launched competing fit accompanied headline
timings. All **195 applications** pass, with no detected overlap or monitor error.
Application success does not erase the scientific discrepancy below.

Development seed **202609330**:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N30720 K6 T4 | 149 [146,149] | 139 [139,139] | 116 [116,118] | 1.198 |
| Unbalanced three FE N30720 K6 T4 | 155 [153,156] | 150 [148,151] | 116 [116,117] | 1.293 |
| Larger unbalanced three FE N61440 K10 T8 | 337 [334,341] | 321 [317,322] | 230 [224,230] | 1.396 |

Fresh seed **202609331**, cells and seed declared before development timing,
without candidate changes between development and confirmation:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N23040 K8 T1 | 159 [157,162] | 149 [148,155] | 294 [294,295] | Unranked |
| Unbalanced three FE N38400 K6 T4 | 195 [192,196] | 184 [183,184] | 154 [153,155] | 1.195 |
| Wider unbalanced three FE N46080 K12 T8 | 283 [281,287] | 273 [271,276] | 960 [956,960] | 0.284 |
| Multiple slopes N7680 K6 T4 | 53 [53,54] | 51 [51,51] | 50 [48,51] | 1.020 |
| Weak barbell N15360 K6 T4 | 69 [68,69] | 64 [63,65] | 1146 [1143,1148] | 0.056 |
| Disconnected N15360 K6 T4 | 92 [89,92] | 88 [88,89] | 70 [70,72] | 1.257 |

Every paired native change in these nine main cells improves. Fresh median gains
are 6.3%, 5.6%, 3.5%, 3.8%, 7.2% and 4.3%. Ratios and every pair are in the
captured `summary.json`; small pilots are science checks, not headline speed claims.
The multiple-slope case does not retain the prior seed's 0.900 stock ratio.

**Unresolved comparison:** in the first fresh cell, all three baseline/stock pairs
fail one fitted-mean coordinate. Maximum mean error is 6.437e-7; the maximum
tolerance ratio is 1.027866. Coefficients, covariance, sample and objectives pass,
as do all independent public-state checks and native candidate/baseline pairs.
This evidence does not identify which large returned mean is responsible. Small
explicit-design pilots pass but cannot adjudicate this larger input. Keep the
stock comparison unranked and preserve all three failures. No tolerance or stock
setting was changed. Historical reference failures also remain unresolved.

## Excluded controls

Fresh seed331, three rotations:

| Case | Baseline | Candidate | Stock |
| --- | ---: | ---: | ---: |
| Unbalanced slopes N30720 K6 T4 low | 226 [223,229] | 221 [217,236] | 113 [113,116] |
| Sparse categorical two FE N61440 K10 T4 | 236 [236,239] | 234 [229,237] | 136 [134,136] |
| Unbalanced categorical three FE N30720 K10 T4 | 181 [178,183] | 181 [178,183] | 113 [109,115] |
| Dense categorical three FE N30720 K6 T4 | 190 [190,197] | 193 [190,194] | 232 [232,235] |
| One categorical FE N61440 K10 T4 | 129 [128,130] | 128 [127,128] | 52 [52,52] |

OLS controls cover multipartition slopes, low-memory three-FE categorical,
one-FE full outputs/robust covariance, and low-memory factor interactions.
Initial baseline/candidate medians are **66/67, 63/66, 17/17 and 66/66 ms**.
Preserve those initial small losses. Five more fixed-input native pairs produce:

| Follow-up | Baseline | Candidate |
| --- | ---: | ---: |
| Dense categorical PPML | 191 [188,199] | 191 [189,195] |
| Slope OLS | 69 [65,71] | 66 [65,69] |
| Low three-FE OLS | 64 [62,64] | 64 [63,68] |

Input hashes match their initial counterparts. No candidate changes occurred.
The follow-ups do not establish persistent regressions; ranges overlap. No speed
improvement is attributed to these excluded routes. All **20 excluded PPML** and
**22 OLS** native scientific-export pairs are exact. Frozen 77 primary, 16 holdout
and 111 native-control definitions remain unchanged.

## Measured costs and next mechanism

Native phase medians from the larger development public receipts, in ms; these
are separate medians and are not asserted to sum to the median public timer:

| Phase | Baseline | Candidate |
| --- | ---: | ---: |
| FE projection | 108.583 | 108.296 |
| Finite solve/check work | 21.622 | 21.315 |
| Strict/final construction | 86.945 | 72.783 |
| Preparation | 34.173 | 34.372 |
| Line search/outer checks | 31.322 | 31.214 |
| Weight refresh | 9.297 | 9.283 |
| Inference | 1.598 | 1.359 |
| Publication | 10.124 | 9.808 |

Outer iterations remain six. The reported mixed inner count moves 82 to 98
because a tight joint Krylov solve replaces a canonical per-column solve;
these raw counts are not comparable work units. No fallback is reported.

Separate diagnostic builds `finalgeomdiag2` and `finalgeomdiag4` use identical
large input hashes. They do not supply headline timings. The baseline's two
strict transactions take **43.483 and 34.953 ms**. The candidate's strict joint
witness takes **20.704 ms** and its one canonical transaction **44.486 ms**.
Within the latter, fused reprojection takes **23.369 ms**, norm refresh **3.857**,
and rank/finite assembly **16.704**, including **10.459 ms** projecting the
nearly zero final finite residual. A cold final canonical refresh remains more
work than the baseline's already-refreshed second transaction.

Candidate joint ordinary calls take about **18.2–20.8 ms** each in this diagnostic.
Their recurrence/reconstruction section takes **5.7–8.8 ms**, finite products/RHS
about **2.7–2.8**, control projection/cross-products **1.3–1.4**, and numerical
coupling refresh **1.4–1.9**. These sections exclude other setup/check work.
Fresh FE certificate calls are roughly **0.8–1.1 ms** for a single RHS and **2.1 ms**
for the final eleven-RHS bank. This is measured attribution, not a claim that
certificates are free or that every remaining operation is avoidable.

The 230 ms stock median gives a **207 ms target**, leaving **114 ms** from the
candidate's 321 ms. The remaining final canonical construction and ordinary/
initial work must change substantially to close it. Repeated projection of the
known final predictor/residual is a concrete future acceptance-design question,
requiring an independently justified replacement rather than a relaxed check.
Do not keep tuning this shortcut's thresholds.

Next prioritize the larger **one-FE whole-path loss**: 128 versus 52 ms, an
81.2 ms reduction to reach 46.8 ms. The captured accepted-source one-FE diagnostic
has 18.4 ms initial retained construction, 15.6 plus 11.3 ms strict transactions,
and 37.3 ms total strict/final phase including surrounding checks. Its existing
analytic concentration already converges in three outer iterations. Thus merely
accelerating ordinary Newton steps or removing a small copy cannot be sufficient.
The next candidate should share accepted-weight centered-design/moment ownership
and canonical final products with independent rank/bread/reconstruction checks,
while removing the separate selected-design copy. Measure initial construction
and public/outer passes too; exact-state reuse must never adopt stale moments.

## Science and memory

- **53 distinct Rust checks pass**, including analytic weighted Poisson truth,
  exact final `q * mu`, T1/T4, disabled shortcut, forced predictor movement,
  forced canonical basis change, session/reconstruction and memory boundaries.
  Geometry tests verify exact-frame relabeling without another preparation,
  sample mismatch refusal and no revival after an error. One pre-existing
  environment-dependent FE-MAP harness test remains ignored.
- **Five final-artifact licensed fixtures pass**, with explicit process/Stata
  return codes and terminal sentinels: independent unit/FW/PW robust/cluster
  multipartition Poisson/Hessian, actual allocation audit, saved slopes,
  separation methods and OLS memory modes. The unchanged warmed allocation audit
  passes T1/T4 with zero events/growth and fresh/canonical agreement.
- Five small explicit-design Poisson/Hessian pilots pass all **15 oracle checks**.
  Candidate maximum absolute errors are 4.448e-13 for coefficients, 1.762e-16 for
  covariance, and 3.759e-10 for fitted means. All **151 public states** and **52
  native PPML pairs** pass. Of 47 stock pairs, 44 pass and the three fitted-mean
  failures above remain visible. Bitwise recurrence equality is not required.
- Actual-plugin ABI and panic-containment checks pass. Candidate and baseline
  median tracked/modeled peaks are identical in every development, fresh and
  excluded cell. The larger development peak is **58,342,238 bytes**, modeled
  **65,262,268 bytes**. No additional heap workspace is introduced. Some existing
  excluded runtime allocation variation remains visible in `summary.json`.

Static allocation inventory drift, weighted constrained-fit failures, generic
nonretaining-CG instability, historical reference discrepancies and Linux/Windows/
physical-cap qualification remain open. No SCC retry, stopped campaign, push,
release or full qualification ran.

## Immutable evidence

`receipts-content-addressed.zip` restores **9,799 logical receipts** from 2,086
payloads (20,959,838 bytes), SHA-256:
`5fa293ccf608f3ecedab1c0cbb7410c66fbee617d681342df96957c5e7527dce`.
The independent restore result is in `verification.txt`.

```bash
python3 restore_receipts.py /new/path/to/restored-receipts
```

The container preserves accepted/candidate and diagnostic source identities,
predeclarations, all rotations and science exports, controllers, oracle source,
tests, profiles, failed attempts and numerical discrepancies. It also preserves
the unadopted working patch captured during planning. `jointfinal1` failed to
compile because repeated overlay arguments selected only the last file;
`jointfinal2` supplies all overlays together. Diagnostic setup failures, the
missing required archive argument and repaired receipt-controller path errors
are retained separately and are not counted as successful applications. Raw
licensed logs, data and binaries are omitted; hashes and generation recipes remain.
This is bounded development evidence, not release qualification.
