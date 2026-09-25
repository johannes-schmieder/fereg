# Retained-coefficient slope canonical products — 2026-09-24

Reject both prototypes under [ADR 0180](../../../../docs/adr/0180-slope-canonical-product-closeout.md).
Production computational inputs remain those of `711802bf`; the experiment
starts from documentation checkpoint `cd19fd12`. All 554 inputs captured by
accepted `familyrows2` still match. No production estimator code changes.

## Work and identities

Eight frozen frames derive from the current diagnosis's existing development
seed 202609347, N61440 unbalanced design. Two or three partitions have respectively
439/615 or 439/615/31 levels; the first two carry intercepts plus one slope.
K is 6 or 10. Frame0 uses the accepted fitted mean and working response; frame1
perturbs log means by 0.35*x1 + 0.15*sin(g1), then recomputes both working weights
and responses. Both use identical positive normalized weights and untouched RHSs
for every engine. Dropping the third partition changes the inner WLS problem;
this is not a claim about a two-FE public Poisson fit on that predictor.

Each native block has five alternating pairs at T1/T4/T8, 240 complete dense
projection/product calls. Common immutable owner construction is outside both
clocks. Every call charges weight-dependent geometry, normalization, coupling,
solving, output construction and the complete fresh certificate. The baseline
uses accepted absorption plus a direct residual-Gram reduction; it is not the
entire production canonical rank/inference pipeline. Candidate row reconstruction
forms that same direct Gram without subtraction of large original/absorbed Grams.
Neither benchmark invokes public Stata or claims a whole-command improvement.

The first candidate repeats intermediate reconstruction before retaining all
coefficients and reconstructing canonical output. The successor uses recurrence
stopping only for the proposal, reconstructs all columns together and applies
the unchanged full original-input certificate to the physical output. Both use
independent per-column recurrences. Every paired reported maximum per-column
iteration count agrees. The probe does not record a complete scalar-operation count.
Source, declarations, exact binaries' hashes, complete logs and export hashes are
archived. The isolated source has 185 captured files per build:

- `ppml-slope-products-20260924`: `6925d5347e5f3d3c9c913a3b06e9d74b7d3e04b64a3e7cb1fd882f363263c8f7`.
- `ppml-slope-products2-20260924`: `0d779c7567dd09c585b3a48df081a0b7fccd14dbbed281b562a74f8677841f91`.
- `ppml-slope-products-profile-20260924`: `6e8848e7d762e785193ac2cae7bcafb5286e23f5f7c40ddf6078d36217767090`.

The third artifact instruments only the successor's subphases and runs one frame,
three thread settings and two pairs. All 18 exported within/Gram/scale files are
bitwise identical to the corresponding second-block exports. Diagnostic timing
must not be substituted for either uninstrumented block.

No compilation, oracle audit or archival work ran alongside timed kernels. Local
process snapshots were taken around the main native blocks, but
these fixed-frame diagnostics did not use the public harness's sampled isolation
monitor. Raw process snapshots remain local; their hashes are preserved. These
are development observations, not qualification or uncontended-system proof.

## First candidate distributions

Milliseconds are median [minimum,maximum]. Frame names encode FE count, K and
weight frame. Twenty-three of 24 medians lose; 116 of 120 paired calls lose.

| Frame | T | Baseline ms | Candidate ms | C/B | Paired losses |
|---|---:|---:|---:|---:|---:|
|e2-k10-f0|1|67.152 [67.045,68.136]|76.531 [76.397,77.390]|1.140|5/5|
|e2-k10-f0|4|24.221 [24.133,24.522]|26.572 [26.023,27.254]|1.097|5/5|
|e2-k10-f0|8|22.651 [21.166,24.799]|23.277 [23.237,27.234]|1.028|5/5|
|e2-k10-f1|1|68.165 [67.921,68.612]|77.215 [77.098,80.263]|1.133|5/5|
|e2-k10-f1|4|25.243 [24.979,25.710]|29.233 [28.522,29.340]|1.158|5/5|
|e2-k10-f1|8|18.307 [18.181,18.567]|20.435 [20.341,21.577]|1.116|5/5|
|e2-k6-f0|1|43.970 [43.724,46.667]|49.863 [49.702,50.363]|1.134|5/5|
|e2-k6-f0|4|18.887 [18.673,19.756]|20.136 [19.727,21.648]|1.066|5/5|
|e2-k6-f0|8|13.513 [13.389,13.873]|13.657 [13.381,14.752]|1.011|3/5|
|e2-k6-f1|1|44.531 [44.216,44.763]|51.099 [50.428,52.136]|1.148|5/5|
|e2-k6-f1|4|18.280 [18.215,18.617]|20.187 [19.788,20.972]|1.104|5/5|
|e2-k6-f1|8|13.454 [13.331,13.749]|13.395 [13.325,13.998]|0.996|3/5|
|e3-k10-f0|1|112.086 [112.026,112.612]|125.740 [124.932,127.465]|1.122|5/5|
|e3-k10-f0|4|41.142 [40.901,45.752]|46.812 [45.547,47.526]|1.138|5/5|
|e3-k10-f0|8|32.276 [32.005,33.719]|35.087 [34.502,36.437]|1.087|5/5|
|e3-k10-f1|1|112.023 [111.698,114.472]|125.078 [124.469,125.690]|1.117|5/5|
|e3-k10-f1|4|41.055 [40.721,43.185]|45.924 [45.687,47.017]|1.119|5/5|
|e3-k10-f1|8|31.951 [31.245,33.691]|36.936 [33.903,38.055]|1.156|5/5|
|e3-k6-f0|1|73.412 [72.900,75.365]|81.604 [80.979,85.007]|1.112|5/5|
|e3-k6-f0|4|28.199 [27.900,30.493]|32.133 [31.555,34.527]|1.140|5/5|
|e3-k6-f0|8|19.940 [17.670,20.625]|21.908 [21.205,23.244]|1.099|5/5|
|e3-k6-f1|1|72.591 [72.432,73.273]|81.566 [80.899,83.207]|1.124|5/5|
|e3-k6-f1|4|28.494 [27.987,28.861]|31.734 [31.107,33.065]|1.114|5/5|
|e3-k6-f1|8|19.620 [17.942,20.422]|20.934 [20.541,22.813]|1.067|5/5|

## Successor distributions

Fourteen of 24 medians improve, with 60 of 120 paired losses. The best median
ratio is 0.946 and the worst 1.086. These differences are too small/inconsistent
for production integration, particularly given new memory and ownership costs.

| Frame | T | Baseline ms | Candidate ms | C/B | Paired losses |
|---|---:|---:|---:|---:|---:|
|e2-k10-f0|1|67.235 [67.066,68.444]|67.091 [66.863,69.726]|0.998|2/5|
|e2-k10-f0|4|24.260 [24.039,24.365]|26.345 [26.275,27.191]|1.086|5/5|
|e2-k10-f0|8|19.839 [18.952,20.073]|19.799 [19.594,20.559]|0.998|3/5|
|e2-k10-f1|1|68.165 [67.904,69.817]|68.514 [67.727,71.057]|1.005|3/5|
|e2-k10-f1|4|25.812 [25.412,26.338]|26.799 [26.177,28.705]|1.038|5/5|
|e2-k10-f1|8|22.088 [21.491,22.274]|20.887 [20.399,22.341]|0.946|1/5|
|e2-k6-f0|1|43.804 [43.606,44.171]|43.997 [43.874,44.550]|1.004|5/5|
|e2-k6-f0|4|18.966 [18.596,19.967]|18.114 [17.914,19.047]|0.955|0/5|
|e2-k6-f0|8|13.123 [12.982,13.479]|13.389 [13.238,13.573]|1.020|5/5|
|e2-k6-f1|1|44.291 [44.197,44.732]|44.463 [44.453,45.166]|1.004|5/5|
|e2-k6-f1|4|19.050 [18.875,19.369]|18.307 [18.212,18.788]|0.961|0/5|
|e2-k6-f1|8|13.307 [13.063,13.443]|13.610 [13.116,14.781]|1.023|4/5|
|e3-k10-f0|1|113.309 [112.447,116.189]|112.780 [112.230,113.466]|0.995|2/5|
|e3-k10-f0|4|43.532 [43.327,43.848]|42.699 [42.428,43.694]|0.981|1/5|
|e3-k10-f0|8|34.113 [33.900,34.745]|33.426 [32.477,33.861]|0.980|0/5|
|e3-k10-f1|1|112.214 [112.001,115.882]|112.914 [111.959,113.312]|1.006|3/5|
|e3-k10-f1|4|43.166 [42.668,43.419]|42.341 [41.854,43.365]|0.981|1/5|
|e3-k10-f1|8|33.773 [33.318,33.830]|32.811 [32.474,34.426]|0.972|2/5|
|e3-k6-f0|1|73.731 [73.324,75.340]|74.798 [73.747,77.495]|1.014|4/5|
|e3-k6-f0|4|30.099 [29.543,31.052]|29.912 [29.577,30.205]|0.994|2/5|
|e3-k6-f0|8|20.959 [20.829,21.480]|20.408 [19.935,20.550]|0.974|0/5|
|e3-k6-f1|1|72.925 [72.639,73.244]|73.395 [73.062,74.002]|1.006|5/5|
|e3-k6-f1|4|29.853 [29.372,30.473]|29.324 [29.215,30.271]|0.982|1/5|
|e3-k6-f1|8|21.126 [21.062,21.293]|20.269 [19.996,22.000]|0.959|1/5|

## Attribution and memory

For e3-k10-f0, separate instrumented T8 spans put coupling preparation at
2.007–2.030 ms and parallel proposal/capture at 19.941–20.697 ms. Full original
geometry/norm setup is 3.240–3.559 ms; reconstructed rows/products 1.795–1.860 ms;
full final certificate 2.044–2.082 ms. At T1, proposal/capture is about 90 ms and
coupling about 2 ms. This identifies recurrence/application and per-column work
as a larger remaining opportunity than retaining coefficients or merely
parallelizing coupling construction. The proposal/capture span is inclusive;
this probe does not claim all of it is sparse matvec arithmetic.

Recorded extra coefficient, row-lookup and reduction payloads are 1,141,024 to
1,737,288 bytes. Both candidates retain the accepted normalized workspace and
materialize dense residual output. These numbers exclude a complete production
phase bound; the successor also temporarily transposes coefficients. No bounded
low-memory admission, allocation audit or RSS claim follows. Product capture
would need a justified complete memory plan before any future integration.

## Independent science and stock diagnostic

Both native artifacts pass 48 explicit-design configurations each: current-
weight projection error <=1e-10, original FE normalized scores <=1e-12, independent
and self-replayed direct Gram error <=1e-10, finite coefficients <=1e-7 and inverse
relative error <=1e-6. The oracle uses separately constructed sparse FE/slopes and
equilibrated SciPy LSMR at atol=btol=2e-15. No source-engine reference is the oracle.
Both artifacts also pass the analytic weighted nonzero-residual fixture, including
locally redundant slope channels, changing weights, two/three FE, T1/T4 and column
scales from 1e-60 to 1e60. All full internal 1e-12 certificates pass.

No complete Poisson/Hessian/sandwich oracle, licensed public fixture or OLS
regression campaign was run for these isolated prototypes. Their scope is the
inner weighted-linear projection/product problem. Production is unchanged.

Stock is unmodified fixest 0.14.2 from source archive
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Its separate block calls `demean` at tol=1e-11 on the same matrices, weights,
intercept/slope partitions and requested threads, then forms original norms and
the direct full Gram. Reading/export is outside the clock. This is not an
alternating native/stock or public timing claim.

The initial standalone script inherited OMP_NUM_THREADS=1. Its requested T4/T8
calls were clamped to T1; preserve all 120 calls and the warning follow-up as
unmatched diagnostics. Their 24 exported states pass mathematics, but those
thread labels do not authorize performance comparison. The corrected declaration
sets OMP_NUM_THREADS=8, retains single-thread BLAS, calls setFixest_nthreads and
asserts getFixest_nthreads equals the requested width before every call. All 120
corrected calls have no warnings, finish with the terminal sentinel, and their
24 exports pass the same independent checks. The existing public harness's
thread_environment already sets the cap per cell; it is unaffected.

Corrected stock T8 medians [min,max] are e2/K6/frame0 18 [18,18] ms and frame1
18 [18,19]; e2/K10 31 [31,31] and 30 [30,31]; e3/K6 26 [26,27] and 27 [27,27];
e3/K10 49 [47,49] and 48 [47,48]. These separate-block observations help compare
work but do not replace public PPML ratios. Cold full-original demeaning is also
not the same as fixest's warm IRLS progression.

## Preservation and next boundary

The archive contains 624 logical receipts / 240 payloads / 1,777,356 bytes,
SHA-256 `020f58e1fbdf0d4e4835b4493759aa5a5c4f94904fa0d94e73d4beac8e3d722b`.
`restore_receipts.py` verifies and restores sanitized sources, recipes, identities,
declarations, logs and numerical checks. Large binary frames/exports and process
snapshots remain local with hashes. No raw licensed logs or test binaries are
committed. The archive includes both rejected candidates and the independent
instrumentation, not merely a selected favorable result.

No fresh seed was consumed and no frozen benchmark definition changed. Close
retained-coefficient layout tuning. The next proposed mechanism shares coupling
loads across active RHS lanes inside the coefficient operator, retaining separate
stopping and complete fresh acceptance. Public gains and complete memory admission
are required before adoption. STATUS.md remains the sole mutable handover record.
