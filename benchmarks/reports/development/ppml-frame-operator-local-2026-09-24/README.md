# PPML projection lifecycle and frame-owned operator comparison

Local macOS arm64 development evidence, 2026-09-24. **Select the frame-owned
reduced matrix for production integration experiments; do not adopt either
prototype as production here.** The edge-only organization has inconsistent
performance. The matrix substantially improves warm solves on small reduced
systems but loses on the larger disconnected system. Whole-command gains,
changing-weight ownership and admission remain to be established.

This checkpoint repairs a pre-existing test-accounting failure. Production
solver behavior, tolerances, rank policy, inference, memory admission and
publication remain unchanged. The broader whole-public PPML <=0.90-times-stock
goal for at least two FE remains unfinished.

## Source and immutable evidence

The accepted computational baseline is **`2c34503d`**, artifact `jointpanels2`,
plugin SHA-256
`4d4c669d6716583b14be437e2732133d61f733840b610414429d8df5614168b1`.
The isolated diagnostic **`frameledger1`** is that revision plus captured span
and frame-generation instrumentation, plugin
`0d6eacc2262fc3cf9b4275aa0dbb5a9031fea6e8df91e8f8d56d82918472964d`.
Its source identity, complete sources, instrumentation, build and actual-plugin
ABI/self-test evidence are preserved. This instrumented artifact is not stock
and does not support headline native timing claims.

Two isolated release Rust test binaries compare the original recurrence with
prepared edge and matrix operators. Their hashes are in `probe-run.json` and
`matrix-run.json`; the full matrix source and original edge module, every input
hash and all controller/oracle scripts are archived. These prototypes are only
compiled under `cfg(test)`. Their convenience allocations and external fixed-frame
lifetime are **not production memory admission or generation safety**.

The [archive](receipts-content-addressed.zip) has **2,123 logical receipts**, 832
payloads, 5,938,416 bytes, SHA-256
`edbd169f91aa568a6e8c61595741bf2072315c8ef6c00bf34682df85becba15b`.
The [restore tool](restore_receipts.py) verified and restored all files. Raw
licensed logs/data/binaries are omitted with hashes; machine paths are sanitized.
The archive includes passing and failed build receipts, source snapshots,
profiles, public exports/audits, all frozen distributions, oracle results and the
three test-accounting regression logs. Historical receipts are unchanged.

No fixest fit, fresh seed campaign or new headline public comparison was run in
this milestone. Prior ratios stay attached to their source-bound reports. Builds
finished before diagnostic fits or frozen timings; there was no competing fit,
profiling or archive work during their paired measurements. The public diagnostic
controller records no overlaps or monitor errors. All processes are terminal.

## Current-source attribution

Seven development cells from seed **202609347**, two runs each: wide low two FE,
unbalanced low two FE, narrow fast two FE, disconnected fast two FE, unbalanced
fast three FE, fast two-slope partitions and its low counterpart. All **14 public
applications and independent public-state checks pass**. Checks cover sample,
original-design score, span and objective replay; they are not a full covariance
oracle per cell. Timing is diagnostic, with instrumentation overhead.

Nested spans use one monotonic epoch and preserve call containment. Exclusive
spans partition the observed plugin root; inclusive children must not be added
again. Generation events identify compatible calls. On wide low N16320/K32/T8,
the canonical projector performs **11 Schur solves across two weight frames**:
initial and final controls are tiled 10/10/10/2, with separate response/residual
calls. There are **78 operator applications and 15,014,400 observation-by-RHS
visits** (two passes per operator). Both repetitions have identical counts.

Wide Schur totals in the two instrumented runs are initialization 5.031/3.860 ms,
operators 12.495/12.627, reconstruction 2.718/2.013 and final certificates
8.459/8.696. The wider initial/final certificate tree takes 10.288 ms in the
second run. These boundaries overlap: do not sum that tree with its child
Schur certificates. Initialization includes RHS construction, so caching group
totals alone cannot remove all 4–5 ms, much less the whole remaining public gap.

Unbalanced low N16320/K12/T4 makes **seven Schur solves**, **35 operators** and
**4,080,000 observation-by-RHS visits**, again across two weight frames. Operator
totals are 5.005/4.994 ms; initialization 1.076/1.176; final certificates
3.857/3.749. Full per-call events, nonoverlapping span summaries and operation
counts are in `profile-summary.json` and `operation-attribution.json`.

The source rebuilds weighted group totals with RHS preparation per absorption;
OLS-only weighted-neighbor scratch does not give PPML frame-owned reuse.
However, topology inspection rejects a simple edge-compression explanation:

| Frozen case | N | Left/right levels as generated | Unique pairs | N/pairs |
|---|---:|---|---:|---:|
| Wide low | 16,320 | 2720 / 68 | 16,320 | 1.000 |
| Unbalanced low | 16,320 | 204 / 68 | 9,415 | 1.733 |
| Narrow fast | 12,000 | 2000 / 50 | 12,000 | 1.000 |
| Disconnected fast | 20,160 | 3360 / 336 | 20,160 | 1.000 |

Wide and narrow still have small reduced dimensions (68 and 50). Preparing a
weighted reduced matrix once per frame can replace repeated observation visits
even when no group pair repeats. That is materially different from the rejected
per-call dense checkpoint in ADR 0164.

## Frozen comparison and independent authority

For each of those four cells, take the diagnostic artifact's accepted final
means as fixed positive weights. Freeze original controls and working response
`log(mu) + y/mu - 1`, FE codes and tolerance **1e-12**. Both native variants see
identical bytes. Probe RHS widths 1, min(10,K+1), K+1 and T1/T4/T8, deduplicating
widths, for **33 configurations per candidate**, seven alternating pairs each.
Copy inputs outside the clock. Report symbolic preparation, weighted refresh
and matrix construction separately from reused complete absorption calls.
These are frozen final frames, not actual changing-weight public IRLS runs.

The edge prototype sorts immutable rows by right/left pair and row identity,
prepares grouped weights once, then applies both sides of the original Schur
operator through group-pair adjacency. The matrix prototype forms the same
reduced weighted group covariance once and substitutes dense matrix products
in the existing component-scaled PCG recurrence. It is an operator replacement,
not a direct factorized solve. It retains original-input RHS preparation,
reconstruction, stopping budget and full fresh certificate. No old or cached
residual certificate grants acceptance.

Every exported residual is compared with independently implemented SciPy sparse
LSMR on the explicit weighted dummy design at 2e-15 internal tolerances.
Normalized projection error must be <=1e-10 and original-design score <=1e-12.
All **582 edge checks and 582 matrix checks pass**, across both variants and
all widths/threads. These checks establish these frozen projections only;
extreme changing weights, weak topology, cancellation, invalidation, allocation
refusal and public Poisson/Hessian integration still require production tests.

## Distributions and setup costs

All repetitions below are milliseconds. B is the unchanged production
recurrence in the same isolated binary; C substitutes the prepared operator.
Case labels W/U/N/D denote wide/unbalanced/narrow/disconnected above. Setup is
charged separately; it must be included in any public adoption decision.

### Edge-only organization

| Case / RHS / T | B repetitions ms | C repetitions ms | C/B median | Total setup ms |
|---|---|---|---:|---:|
| W / 1 / 1 | 1.813;1.619;1.662;1.661;1.600;1.563;1.568 | 1.847;1.951;1.903;1.764;1.936;1.801;1.917 | 1.175 | 1.267 |
| W / 1 / 4 | 1.671;1.543;1.611;1.532;1.616;1.518;1.564 | 1.309;1.377;1.294;1.338;1.236;1.292;1.282 | 0.827 | 1.181 |
| W / 1 / 8 | 1.832;1.633;1.602;1.650;1.650;1.850;1.577 | 1.528;1.383;1.518;1.371;1.496;1.640;1.521 | 0.920 | 1.335 |
| W / 10 / 1 | 5.337;5.160;5.334;5.232;5.064;5.296;5.341 | 5.117;5.817;5.204;5.247;5.280;4.962;5.281 | 0.991 | 1.254 |
| W / 10 / 4 | 4.347;4.514;4.614;4.575;4.491;4.844;4.450 | 3.001;3.142;2.885;2.822;3.059;3.009;4.236 | 0.667 | 1.102 |
| W / 10 / 8 | 4.838;4.816;4.457;4.533;4.743;5.120;4.925 | 3.384;3.238;3.228;3.921;2.896;2.919;3.399 | 0.672 | 1.254 |
| W / 33 / 1 | 15.495;15.414;16.286;15.886;16.608;15.428;16.759 | 14.900;13.867;14.654;14.787;14.240;15.924;14.306 | 0.922 | 1.183 |
| W / 33 / 4 | 13.072;12.954;13.717;13.149;12.879;13.217;13.556 | 8.234;8.217;7.842;7.885;7.532;7.937;10.169 | 0.604 | 1.261 |
| W / 33 / 8 | 12.902;13.453;13.498;12.977;13.425;12.665;12.987 | 8.313;8.100;7.896;7.565;8.189;8.029;7.111 | 0.618 | 1.210 |
| U / 1 / 1 | 1.486;1.425;1.411;1.463;1.459;1.416;1.403 | 1.344;1.256;1.272;1.266;1.385;1.312;1.286 | 0.903 | 0.912 |
| U / 1 / 4 | 1.380;1.326;1.352;1.337;1.529;1.306;1.636 | 1.006;1.175;1.119;1.119;1.018;1.020;1.149 | 0.828 | 1.013 |
| U / 1 / 8 | 1.304;1.481;1.902;1.427;1.714;1.497;1.678 | 1.274;1.062;1.260;1.250;1.071;1.228;1.542 | 0.835 | 1.096 |
| U / 10 / 1 | 4.229;4.755;4.228;4.260;4.338;4.619;4.202 | 3.598;3.316;3.623;3.418;3.280;3.224;3.666 | 0.802 | 1.018 |
| U / 10 / 4 | 3.466;3.490;3.427;3.364;3.651;3.457;3.475 | 2.145;2.328;2.132;2.087;2.174;2.192;2.098 | 0.619 | 0.953 |
| U / 10 / 8 | 3.448;3.713;3.980;3.413;3.496;3.530;3.508 | 2.653;3.065;2.773;2.336;2.456;2.135;2.340 | 0.700 | 0.985 |
| U / 13 / 1 | 5.265;5.349;5.828;5.179;5.847;5.577;5.415 | 4.880;4.740;4.213;4.709;4.121;4.694;4.198 | 0.867 | 0.973 |
| U / 13 / 4 | 4.395;4.381;4.418;4.754;4.663;4.487;4.627 | 3.013;2.861;2.729;2.972;2.769;2.791;2.747 | 0.622 | 1.037 |
| U / 13 / 8 | 4.200;4.382;4.649;4.576;5.018;4.745;4.774 | 3.423;3.634;3.341;3.025;3.072;2.762;3.530 | 0.719 | 0.946 |
| N / 1 / 1 | 1.246;1.218;1.228;1.240;1.256;1.217;1.213 | 1.506;1.458;1.460;1.424;1.536;1.513;1.439 | 1.189 | 0.825 |
| N / 1 / 4 | 1.272;1.223;1.204;1.226;1.253;1.255;1.198 | 1.070;1.271;1.254;1.108;1.677;1.209;1.061 | 0.986 | 0.921 |
| N / 1 / 8 | 1.765;1.335;1.250;1.420;1.331;1.210;1.568 | 2.173;1.671;1.197;1.560;1.375;1.853;1.060 | 1.169 | 0.901 |
| N / 4 / 1 | 2.096;1.998;2.002;2.207;2.128;2.069;2.057 | 2.175;2.312;2.152;2.426;2.246;2.329;2.243 | 1.086 | 0.816 |
| N / 4 / 4 | 1.945;1.874;1.908;2.059;1.939;1.965;1.921 | 1.755;1.342;1.321;1.214;1.613;1.602;1.261 | 0.692 | 0.838 |
| N / 4 / 8 | 1.919;2.165;1.983;2.298;2.202;2.329;2.442 | 1.508;1.418;2.090;2.646;2.018;2.243;2.088 | 0.948 | 0.922 |
| D / 1 / 1 | 2.316;2.303;2.176;2.135;2.098;2.246;2.271 | 2.472;2.638;2.586;2.519;2.532;2.573;2.549 | 1.135 | 1.444 |
| D / 1 / 4 | 2.056;1.487;1.501;1.543;1.712;1.672;1.603 | 1.702;1.592;1.707;1.500;1.395;1.523;1.470 | 0.950 | 1.533 |
| D / 1 / 8 | 1.854;1.761;1.578;1.778;2.061;1.906;1.941 | 1.869;1.974;1.455;1.685;1.618;1.980;2.236 | 1.008 | 1.429 |
| D / 10 / 1 | 7.363;7.807;7.260;6.889;7.024;7.272;7.247 | 8.094;7.734;7.095;7.852;7.518;7.327;7.674 | 1.057 | 1.350 |
| D / 10 / 4 | 3.586;3.358;3.316;3.602;3.847;3.355;3.388 | 3.663;3.606;3.719;3.807;3.742;3.554;3.554 | 1.081 | 1.487 |
| D / 10 / 8 | 4.132;4.216;4.249;3.566;3.847;4.017;4.074 | 4.188;4.640;4.668;4.306;4.487;4.641;4.521 | 1.110 | 1.467 |
| D / 13 / 1 | 9.756;9.916;9.872;9.623;9.325;9.410;10.004 | 8.996;9.346;9.405;9.162;9.542;9.074;9.206 | 0.944 | 1.369 |
| D / 13 / 4 | 4.620;4.374;4.512;4.743;4.316;4.413;4.337 | 4.656;4.848;4.699;4.753;4.735;4.578;4.665 | 1.065 | 1.454 |
| D / 13 / 8 | 5.468;4.767;4.797;4.477;5.266;4.636;4.822 | 5.027;5.829;5.163;4.976;4.991;5.931;5.237 | 1.076 | 1.480 |

Edge-only warm medians improve in 22 configurations and lose in 11. Only nine
improve after charging all setup to one call. Wide width10/T8 improves 4.816 to
3.238 ms, but narrow width1/T8 loses 1.335 to 1.560 and disconnected width10/T4
loses 3.388 to 3.663. The latter already used a parallel observation operator.
These losses rule out broad edge-only adoption; no favorable-case guard is fitted.

### Frame-owned reduced matrix

| Case / RHS / T | B repetitions ms | C repetitions ms | C/B median | Total setup ms |
|---|---|---|---:|---:|
| W / 1 / 1 | 2.002;1.670;1.674;1.703;1.758;1.626;1.765 | 1.145;1.131;1.110;1.110;1.137;1.294;1.098 | 0.664 | 1.531 |
| W / 1 / 4 | 1.743;1.526;1.524;1.533;1.546;1.579;1.603 | 0.997;1.014;0.913;0.979;0.972;0.882;1.034 | 0.633 | 1.357 |
| W / 1 / 8 | 1.700;1.603;1.582;1.645;1.786;1.714;1.551 | 1.306;1.225;1.124;0.973;1.013;1.128;0.946 | 0.684 | 1.584 |
| W / 10 / 1 | 5.281;5.109;5.117;5.115;5.097;5.137;5.168 | 2.833;2.827;2.767;2.767;2.761;2.809;2.773 | 0.542 | 1.323 |
| W / 10 / 4 | 4.366;4.396;4.527;4.258;4.329;4.424;4.287 | 2.139;1.914;2.100;1.905;2.060;2.073;2.109 | 0.475 | 1.254 |
| W / 10 / 8 | 4.472;4.707;4.432;4.559;4.651;4.529;4.577 | 2.336;2.399;2.396;2.376;2.251;2.663;2.438 | 0.526 | 1.283 |
| W / 33 / 1 | 15.410;14.923;14.670;14.585;14.719;14.777;14.757 | 7.492;7.655;7.415;7.385;7.531;7.549;7.453 | 0.508 | 1.299 |
| W / 33 / 4 | 12.350;12.614;12.772;12.588;12.419;12.569;12.440 | 5.126;5.700;5.736;5.253;5.052;5.045;5.077 | 0.408 | 1.241 |
| W / 33 / 8 | 12.341;12.709;13.005;13.207;13.212;13.795;13.361 | 6.102;6.177;5.611;6.130;5.802;6.716;5.837 | 0.462 | 1.287 |
| U / 1 / 1 | 1.487;1.476;1.462;1.482;1.488;1.466;1.511 | 1.037;1.054;1.049;1.064;1.043;1.082;1.047 | 0.708 | 2.377 |
| U / 1 / 4 | 1.307;1.329;1.325;1.314;1.307;1.326;1.332 | 0.982;0.867;0.824;0.905;0.873;0.834;0.828 | 0.655 | 2.339 |
| U / 1 / 8 | 1.446;1.298;1.527;1.455;1.371;1.379;1.527 | 1.246;0.945;0.999;0.872;0.876;0.879;1.002 | 0.654 | 2.366 |
| U / 10 / 1 | 4.241;4.044;4.111;4.023;4.029;4.080;4.099 | 2.758;2.521;2.493;2.427;2.576;2.556;2.495 | 0.618 | 2.396 |
| U / 10 / 4 | 3.282;3.515;3.458;3.526;3.510;3.355;3.487 | 1.675;1.972;2.050;1.901;1.920;1.933;1.852 | 0.551 | 2.291 |
| U / 10 / 8 | 3.686;3.429;3.232;3.300;3.387;3.245;3.257 | 1.987;2.042;1.949;1.912;2.067;1.923;2.044 | 0.602 | 2.369 |
| U / 13 / 1 | 5.139;5.625;5.409;5.491;5.359;5.444;5.082 | 3.336;3.564;3.413;3.432;3.391;3.340;3.495 | 0.631 | 2.281 |
| U / 13 / 4 | 4.268;4.553;4.574;4.418;4.590;4.438;4.254 | 2.330;2.510;2.526;2.281;2.264;2.360;2.307 | 0.525 | 2.292 |
| U / 13 / 8 | 4.485;4.205;4.405;4.515;4.569;4.424;4.261 | 2.562;2.697;2.531;2.728;2.804;2.629;2.623 | 0.594 | 2.395 |
| N / 1 / 1 | 1.278;1.288;1.301;1.291;1.290;1.295;1.378 | 0.789;0.798;0.802;0.790;0.796;0.793;0.789 | 0.614 | 0.899 |
| N / 1 / 4 | 1.191;1.220;1.272;1.220;1.220;1.203;1.216 | 0.753;0.757;0.690;0.703;0.698;0.723;0.708 | 0.580 | 0.954 |
| N / 1 / 8 | 1.798;1.284;1.274;1.382;1.283;1.334;1.419 | 0.958;0.827;0.967;0.773;0.754;0.764;0.865 | 0.620 | 1.063 |
| N / 4 / 1 | 2.162;2.124;2.143;2.249;2.330;2.115;2.149 | 1.291;1.260;1.240;1.231;1.226;1.260;1.238 | 0.577 | 0.914 |
| N / 4 / 4 | 1.837;1.778;1.748;1.717;1.732;1.715;1.724 | 0.810;0.924;0.795;0.833;0.868;0.809;0.775 | 0.467 | 0.891 |
| N / 4 / 8 | 2.035;1.813;1.813;1.941;1.914;1.985;1.973 | 0.967;1.028;0.892;1.042;1.156;1.034;1.273 | 0.532 | 0.905 |
| D / 1 / 1 | 2.367;2.281;2.260;2.232;2.175;2.236;2.175 | 5.462;5.467;5.416;5.381;5.354;5.413;5.497 | 2.422 | 1.648 |
| D / 1 / 4 | 1.392;1.158;1.223;1.205;1.202;1.185;1.154 | 1.838;1.938;1.841;1.916;1.919;1.869;1.833 | 1.555 | 1.523 |
| D / 1 / 8 | 1.327;1.216;1.425;1.246;1.320;1.372;1.471 | 1.639;1.595;1.611;1.545;1.670;1.501;1.528 | 1.202 | 1.596 |
| D / 10 / 1 | 7.185;7.251;7.602;7.043;7.244;7.059;7.047 | 9.288;8.813;9.464;8.839;8.661;8.619;8.603 | 1.227 | 1.552 |
| D / 10 / 4 | 2.520;3.010;2.957;3.006;2.915;3.018;3.112 | 3.653;3.951;3.695;3.546;3.708;3.642;3.757 | 1.229 | 1.548 |
| D / 10 / 8 | 3.595;3.179;3.632;3.058;3.176;3.250;2.947 | 3.306;3.642;3.473;3.430;3.325;3.238;3.379 | 1.063 | 1.782 |
| D / 13 / 1 | 9.040;9.572;9.491;9.224;9.530;9.512;9.482 | 11.418;11.835;11.578;11.802;11.587;11.551;11.056 | 1.220 | 1.516 |
| D / 13 / 4 | 3.645;3.932;4.120;3.610;3.531;3.489;3.600 | 4.709;4.555;4.433;4.654;4.469;4.449;4.603 | 1.262 | 1.620 |
| D / 13 / 8 | 3.993;3.727;3.738;4.052;3.798;3.791;3.744 | 4.184;4.190;4.201;4.152;3.979;4.117;4.151 | 1.095 | 1.645 |

All 24 small-reduced-system warm medians improve, by approximately 29–59%; all
nine disconnected medians lose, by 6–142%. Only eight configurations improve if
charged all setup for a single call. Narrow width4/T1 is essentially a cold tie,
2.149 baseline versus 2.155 including setup. The reason to proceed is reuse
across several compatible calls, not a claim that setup is free.

Representative decomposition and prototype payload capacities:

| Case / RHS / T | B median ms | C warm ms | Topology ms | Weight refresh ms | Matrix ms | Payload bytes |
|---|---:|---:|---:|---:|---:|---:|
| W / 10 / 8 | 4.559 | 2.396 | 1.082 | 0.063 | 0.138 | 844,208 |
| U / 10 / 4 | 3.487 | 1.920 | 0.909 | 0.029 | 1.354 | 713,600 |
| N / 4 / 1 | 2.149 | 1.240 | 0.796 | 0.020 | 0.098 | 717,632 |
| D / 10 / 4 | 3.006 | 3.695 | 1.342 | 0.027 | 0.179 | 2,203,024 |

Payloads count symbolic arrays, weight values and the dense matrix, not complete
public peak memory. The probe's convenience vectors may overreserve; production
needs checked bounds and fallible fixed capacities. Unbalanced matrix preparation
is appreciable because eliminated groups have more distinct neighbors. No
addition of these frozen medians proves a public lifecycle saving: actual RHS
contents, tolerances and iteration counts vary across initial/final calls.

## Test-accounting repair and failures

The prior checkpoint-count failure is a **test expectation bug**. A rank-zero
external joint Newton witness uses the LSMR-fused reporting row and counts as a
checkpoint, although it does not refresh the canonical session. The test added
canonical refreshes plus the initial checkpoint and compared that with a row
containing both canonical work and that witness, yielding 2 versus 3.

Add a test-only counter at the successful fused witness observation, reset with
the canonical counter, and include its actual count in the shared accounting
assertion. Do not change production reporting or manufacture a refresh. Three
focused regressions pass on the changed source: rank-zero fused identity,
explicit LSMR versus MAP (two/three FE), and three-FE strict basis-refresh
rebuild. The tests retain fit/science comparisons and checks that ordinary
specialized work bypasses canonical refresh. No new public plugin or whole-suite
claim is attached to this test-only change.

The first isolated probe build omitted the pinned fixture directory. A first
repair attempt used the system Python lacking `tarfile.extractall(filter=...)`,
so the second build repeated the missing-fixture failure. The supported local
runtime restored the exact pinned tests; the third edge build and matrix build
pass. All failed logs remain in the archive. No solver failure, oracle exclusion
or tolerance change occurred in the completed frozen comparisons.

## Integration decision

Proceed with **one frame-owned reduced operator**, retaining the original solver
when structural work or complete memory admission refuses it. Compare q² matrix
work against the two observation passes and account for assembly degree costs;
use geometry dimensions and work bounds, not these labels or seeds. The larger
disconnected matrix demonstrates why a dense operator is not a universal route.
The existing sample/topology/weight generations should own validity. Response
changes can reuse a valid matrix; changed weights must rebuild it. Errors and
unwinds revoke authority. Keep component scaling and original-input certificates.

Production construction should preserve exact symmetry, handle extreme masses
and cancellation, use fallible admitted capacities, and retain safe fallback.
The probe's unconstrained allocation and fixed-frame external lifetime must not
be copied into production. Remove superseded numerical paths only after their
replacement is independently established. Protect OLS and other PPML routes.

Next evidence must be whole-public baseline/candidate/pinned-stock rotations,
independent weighted WLS/Poisson/Hessian fixtures, invalidation and budget-boundary
tests, then fresh predeclared cases. The last accepted public ratios remain those
of `jointpanels2`; no new 10%-faster claim is made here. This is a completed design
selection milestone, not completion of the broader performance objective.
