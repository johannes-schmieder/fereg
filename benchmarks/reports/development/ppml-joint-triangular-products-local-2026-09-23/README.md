# Triangular blocked products for joint PPML

Local macOS arm64 development milestone, September 23, 2026. Adopt `jointtri1`
under [ADR 0148](../../../../docs/adr/0148-triangular-blocked-ppml-finite-products.md).
Compute one symmetric half of the current finite Hessian, move right-column
normalization outside its reductions, and use bounded independent accumulators
with direct numeric slices where available. No solver cache or heap bank is added.
Fresh low-mode medians improve 8.9–22.5%; all twenty-five corresponding pairs
improve. Fast slopes have a smaller gain with one loss. Dense categorical PPML
still misses the objective of whole public time **<=0.90 times comparable stock**.

## Source, comparator and measurement

Baseline `directtwo3` matches `d9e9d2f7`, plugin SHA-256
`9b027385d827c8e71ee4997ef10dc94d3c5a626c4666a7344e74090488122bd9`.
Candidate `jointtri1` overlays only:

* `crates/fereg-core/src/fe/slope_coefficients/joint.rs`
* `crates/fereg-core/src/ols/linear_session.rs`

All **542 captured computational inputs** match the adopted worktree. Candidate
plugin SHA-256:

`481755f5ee9d42fa417f810d8373095506e05f21dc80e27b8bc2d5e64f968d7c`.

Stock fixest 0.14.2 is unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial BLAS.
Public clocks include robust inference and fitted means; stock includes response
prediction. Startup, generation/import, warmup and export are outside both clocks.
Native low mode versus stock is not physical-memory-matched. Weighted/clustered
science is checked separately by independent licensed oracles, not ordinary
fixest weights treated as a frequency-weight inference oracle.

Orders rotate baseline/candidate/stock in fresh processes. Builds, tests,
diagnostics and archiving are outside headline blocks; no attempt has detected
overlap or a monitor error. Development seed 202609347 is reused. Fresh seed
**202609354** was predeclared after the final source froze and is now consumed.
Frozen 77/16/111 definitions and protected cycle seeds 211/212 are unchanged.

## Competing matrix-free design: tested and rejected

The first coherent candidate eliminates repeated finite products after seeding a
finite Cholesky preconditioner on an earlier frame. Fresh column norms transport
that factor's coordinate units; the exact current-weight observation operator
and independent reconstructed-residual acceptance remain. A selected-column
identity bank and design/error invalidation protect reuse. The previous factor
has no current Hessian, rank or covariance authority.

`jointmf1` has plugin SHA-256
`c0e1e47acf38c67492eccbe74d78f4e1a5a57892a411ab619a898bda26d3842c`.
Its frozen inner comparison uses N16320, identical current weights, controls and
responses for full-product and matrix-free solves, with an analytical weighted
pair-contrast residual and known coefficients. Four designs, three repetitions
and four frames give 96 accepted inner solves. The first shared cold frame is
excluded from these warm medians:

| Frozen row-major design | Full products ms | Matrix-free ms | Typical Krylov steps |
|---|---:|---:|---|
| K8/T4, categorical | 2.920 | 4.568 | 10–11 / 10 |
| K24/T8, categorical | 11.781 | 12.999 | 11 / 11 |
| K64/T8, categorical | 68.041 | 46.480 | 15 / 15 |
| K24/T4, slopes on both | 13.452 | 17.653 | 14–15 / 14–15 |

These are inner diagnostics with row-major `DenseMatrix` controls, not public
timings. Nearly unchanged iteration counts do not imply comparable work: the
matrix-free engine performs observation operations on each step, while the
prepared operator largely works with coefficient products.

The public source uses column-oriented numeric inputs. The initial matrix-free
forward kernel gathers complete rows, and the apparent K64 gain does not survive:

| Public low case | Baseline median [min,max] ms | `jointmf1` | Stock |
|---|---:|---:|---:|
| N16320/K24/T8 | 155 [148,155] | 217 [217,225] | 71 [69,73] |
| N30720/K6/T4 | 106 [101,106] | 137 [133,137] | 59 [59,60] |
| N16320/K64/T8 | 546 [533,554] | 777 [775,784] | 192 [190,195] |

The corrected source-oriented forward kernel reads columns and pulls fixed
direction scaling outside its row traversal. `jointmf2`, plugin
`cd95377174e0d13bc1b2c9df6a14f1d753bbc67ad97553e8de3edf6ef4de7210`,
substantially reduces the loss, but remains unsuitable as a broad replacement:

| Same public case | Baseline | `jointmf2` | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| K24 | 149 [149,153] | 144 [144,146] | 73 [71,74] | -9/-3/-5 |
| K6 | 100 [100,104] | 116 [111,120] | 53 [52,57] | +11/+16/+16 |
| K64 | 549 [537,557] | 573 [568,589] | 190 [190,191] | +19/+32/+36 |

Each public screen has three rotations. Separate one-rotation small pilots retain
their ties/losses in the archive. Across both prototypes, all 72 public applications,
18 small independent Poisson results, 24 native pairs and 24 direct candidate/stock
pairs pass. The first prototype passed 56 focused Rust tests before adding the
ignored frozen diagnostic; that diagnostic then passed on the captured first
artifact source. The second captured source passes the same 56 focused tests.
This distinguishes test snapshots instead of relabeling an earlier receipt.

Reject both prototypes. Their preconditioner history, selected-column bank and
alternate recurrence are absent from production. Do not fit width/aging guards
to these cells. The evidence rejects these implementations, not every possible
matrix-free or warm Newton organization. Source layout matters enough that a
row-major inner benchmark cannot establish public speed.

## Adopted current-weight product organization

For V = sqrt(W) X, current eliminated-partition projection P0 and checked norms s,
the finite entry is still `(P0 V_i / s_i)' V_j / s_j`. Compute only i<=j, then
copy the upper triangle. Dense storage retains its existing dot arithmetic over
projected columns. Streamed storage divides by s_j after accumulation, avoiding
one right normalization per observation and entry. It no longer computes the
reverse entry and averages the two directions.

For K24 this reduces entries from 576 to 300; for K64, from 4096 to 2080. For
each streamed entry, N right-coordinate divisions become one scalar division.
These counts concern finite-Hessian construction only: column preparation,
geometry, cross-products, reconstruction and certificates remain.

With multiple admitted column workers, eight fixed accumulators break the single
sum dependency. Borrow existing contiguous numeric slices; generated or unsupported
sources keep fallible column access. The single-worker path retains its complete-
row organization and independent entry chains, restricted to the same triangle.
There is no specific hardware-vectorization claim. Worker scheduling cannot
change an entry's order; cross-thread agreement uses the existing numerical
contract, not old-recurrence bit equality.

Current-weight geometry, column norms, FE/control products, finite factorization,
fresh FE and original-input finite scores, reconstruction, outer objective and
strict state checks remain. Canonical rank, final bread/Hessian and publication
are unchanged. No sample, tolerance, weight, DoF, separation or failure-atomicity
policy changes. Fast categorical LSMR, one-FE and public OLS retain their algorithms.

## Final public timings

Development seed347, three rotations, median [min,max] milliseconds:

| Case | Baseline | Candidate | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| Low two FE N16320/K24/T8 | 157 [155,163] | 135 [129,135] | 71 [69,71] | 1.901 |
| Low two FE N30720/K6/T4 | 106 [106,110] | 97 [97,99] | 56 [56,57] | 1.732 |
| Low two FE N16320/K64/T8 | 558 [551,600] | 452 [443,459] | 191 [190,193] | 2.366 |
| Low two FE N16320/K24/T1 | 271 [271,276] | 248 [248,250] | 229 [227,229] | 1.083 |
| Low slopes on both, N30720/K6/T4 | 140 [138,143] | 131 [130,132] | 118 [116,118] | 1.110 |
| Fast slopes on both, same N/K/T | 125 [125,126] | 125 [125,125] | 118 [117,119] | 1.059 |

All fifteen low-mode pairs improve: K24 changes -28/-22/-26 ms, K6 -11/-9/-9,
K64 -141/-115/-99, T1 -23/-26/-23 and low slopes -8/-11/-9. Fast slope pairs
are 0/0/-1 ms. The small final pilot ties at 16, 17 and 15 ms against stock
17, 15 and 16; one observation per variant is not confirmation.

Fresh seed354, five rotations:

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90× stock target |
|---|---:|---:|---:|---:|---:|
| Low two FE N18240/K32/T8 | 204 [199,214] | 158 [151,165] | 94 [90,96] | 1.681 | 84.6 |
| Low unbalanced two FE N15120/K24/T8 | 127 [124,130] | 109 [105,111] | 59 [59,61] | 1.847 | 53.1 |
| Low serial two FE N12000/K12/T1 | 101 [98,103] | 92 [90,94] | 89 [85,92] | 1.034 | 80.1 |
| Low disconnected two FE N20160/K12/T4 | 109 [108,114] | 95 [95,97] | 63 [60,68] | 1.508 | 56.7 |
| Low unbalanced three FE, multiple slopes, N28800/K10/T4 | 199 [196,203] | 180 [177,182] | 220 [215,222] | 0.818 | 198.0 |
| Fast two FE, slopes on both, N30720/K12/T4 | 154 [152,156] | 151 [149,153] | 207 [200,208] | 0.729 | 186.3 |

The multiple-slope case has g1 slopes z1/z2 and g2 slope z2. All twenty-five
low-mode pairs improve. Median reductions by row are 22.5%, 14.2%, 8.9%, 12.8%,
9.5% and 1.9%. Paired changes are:

* -42/-48/-59/-41/-45 ms
* -20/-19/-20/-16/-19 ms
* -13/-7/-9/-8/-8 ms
* -14/-16/-14/-13/-17 ms
* -26/-17/-17/-16/-24 ms
* -1/-6/-6/-3/+1 ms

Two of six fresh targets clear the stock margin. The first four remain losses;
the table's target budgets are not predictions of attainable runtime.

Excluded and weak-connectivity controls, three rotations:

| Control | Baseline | Candidate | Stock | Native paired changes ms |
|---|---:|---:|---:|---|
| Fast categorical two FE N16320/K16/T4 | 115 [112,117] | 115 [110,115] | 63 [60,64] | +3/-2/-5 |
| Low one FE N16320/K12/T4 | 41 [40,41] | 40 [40,40] | 26 [25,27] | -1/-1/0 |
| Low weak barbell two FE N7680/K4/T4 | 30 [30,32] | 30 [30,32] | 221 [218,222] | -2/+2/0 |
| OLS categorical two FE N16320/K16/T4 | 36 [35,36] | 34 [33,35] | — | -2/-3/0 |
| OLS slopes on both N30720/K6/T4 | 66 [66,68] | 67 [66,70] | — | -2/+1/+4 |

The OLS slope median loses 1 ms, with mixed pairs and exact exports. Fast
categorical and weak PPML also retain individual losses. These distributions do
not establish universal timing non-regression. The newly measured small one-FE
comparison passes science; it does not adjudicate the historical large one-FE
stock-mean discrepancy.

## Final validation and memory

**156 distinct final-source Rust tests** pass, including analytical joint WLS
under changing weights, scaled/permuted controls, rank deficiency, failed-frame
revocation, strict rollback, dynamic-range/profile boundaries, prepared IRLS and
estimation, and all 51 PPML memory tests. The added independent inner-product
test covers magnitudes 1e-300/1/1e300, lengths 1/7/8/17/8192, contiguous and
fallible access. Production ABI and panic-containment checks pass.

**Six licensed fixtures pass**, with process status, explicit Stata return code
and terminal sentinels: the N4800/K8 explicit Poisson/Hessian/robust-and-cluster
sandwich oracle for unit/FW/PW at T1/T4; the actual-plugin allocation audit;
multipartition slope and separation oracles; checkpoint publication/failure
atomicity; and generated factor/product terms with PW and numeric FE shifts
0/1000. The separate 1e6-shift failure is not rerun or repaired.

Final-candidate blocks contain **192 applications**, **180 public-state checks**,
**9 independent Python oracle results**, **66 native scientific pairs**,
**60 baseline/stock** and **60 direct candidate/stock pairs**: all pass. Twenty
native export pairs are exact. No scientific exclusion, application failure,
timeout or detected contention occurs. Adding the rejected prototypes gives
264 applications, 27 small independent oracle results and 84 direct stock pairs;
keep the separate source identities when using those totals. Six instrumented
applications and the 96 frozen inner solves are separate from headline totals.

All 60 final PPML pairs have identical outer/inner iterations, halvings, fallback/
strict rejections, rank/DoF, modeled peaks and scratch metadata. Tracked peaks
match in 59; one fast categorical control decreases by 766,352 bytes. No tracked
growth is introduced in these pairs. The actual-plugin audit records zero
allocator events/current growth during sixteen ordinary joint steps, stable
capacities and fresh/warm equivalence. This window directly exercises the changed
ordinary product preparation; it does not qualify all allocation lifetimes.

The helper adds only eight stack f64 accumulators per call. There is no new heap
bank, observation matrix, worker, memory formula or cross-frame state. Optional
joint admission and constrained-route refusal remain. Physical-cap and
cross-platform confirmation remain deferred; static allocation inventory drift,
weighted constrained-fit failures and nonretaining-CG instability remain open.

## Isolated phase attribution and next subsystem

Baseline diagnostic `directtwoledger3`, plugin
`6091752be87101126c3213ccaf802c0deb84f37a47c9516dafd8c9f5ae814d8e`,
matches the baseline implementation plus its captured probes. Candidate
`jointtriledger1`, plugin
`128a7ac697827756969b940e8f17aa69e10eb575d633f243913a49b85634c124`,
adds the same RAII spans to the adopted candidate. One rotation per case, after
all headline fits, has six passing applications and public-state audits.

| Inclusive diagnostic phase, ms | K24 baseline→candidate | K6 baseline→candidate | K64 baseline→candidate |
|---|---:|---:|---:|
| Finite-product tiles, six calls | 29.922→5.110 | 10.382→2.793 | 206.597→91.346 |
| Complete joint directions, six calls | 58.553→33.827 | 35.169→27.211 | 308.919→192.188 |
| Column norms | 6.135→5.827 | 2.129→2.111 | 27.824→27.533 |
| Reconstruction | 10.742→11.456 | 4.996→5.001 | 55.804→55.586 |
| Exclusive recurrence | 0.874→0.887 | 1.384→1.346 | 2.133→2.116 |
| Canonical projection wrappers combined | 26.537→25.917 | 13.789→15.983 | 53.095→54.534 |
| Canonical rank work | 16.549→15.955 | 10.529→10.949 | 39.978→40.456 |
| Inference | 4.608→4.608 | 0.606→0.618 | 22.590→22.930 |
| Publication wrapper | 4.952→6.025 | 5.647→6.011 | 12.179→24.573 |
| Native root | 141.887→117.429 | 95.136→89.524 | 528.477→427.537 |

Rows nest; do not sum them. Single intrusive clocks are attribution evidence,
not headline ratios. In particular, the K64 publication wrapper rises by about
12.4 ms and is not attributed to the product mechanism. Preserve that variation.
The matching six-call counts and unchanged final iteration metadata support
reduced preparation work, rather than fewer scientific checks or looser solves.

Close this finite-product experiment, including accumulator-width tuning. The
next material opportunity is **original-column binding and repeated norm,
reconstruction and certificate scans**, together with compatible current-frame
canonical products. At K64, norms plus reconstruction alone consume about 83 ms;
the narrow joint wrapper still has 13.2 ms outside its inner solve. Inspect those
passes and their valid shared statistics, preserving independent original-input
score and model-space acceptance.

Source inspection finds `MAX_BOUND_NUMERIC_COLUMNS=64`, with all-or-nothing
binding based on the complete prepared model width. Verify whether the implicit
intercept sends the public K64 case beyond this limit before claiming attribution.
Generalize per-column or bounded-batch binding, including mixed generated/numeric
models, instead of merely increasing a threshold. The current wide diagnostic
supports that investigation; it does not prove the size of its potential gain.
Canonical current-frame geometry/product ownership, fast categorical PPML and
one-FE whole-command losses remain on the broader worklist.

## Immutable receipts

The content-addressed archive contains **11,449 logical receipts**, 2,444 unique
payloads and 19,873,534 bytes. SHA-256:

`420d6eed3af4b49620b21e54410283928845c6b5506b7aba5c81179457b299f1`.

`restore_receipts.py NEW_DIRECTORY` validates the container, index and every
logical payload; restoration was verified before handover. `matrix-free/`
contains rejected sources, declarations, tests, frozen measurements and public
comparisons. `products/` contains the final sources, declarations, build/test
receipts, fixture recipes, timings, scientific audits, metadata and diagnostics.
Each has its own `summary.json`; final phase and memory details are in
`products/ledger-summary.json` and `products/metadata-details.json`.
Generated data/means and omitted licensed logs have hashes. Raw licensed logs,
binaries and machine-specific paths are excluded or sanitized. These are local
development receipts, not qualification or claims transferred to another artifact.
