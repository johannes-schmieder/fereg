# Shared dense slope factors — rejected Mac development experiment

No production change is adopted. The shared factor improves several cold
projection benchmarks but loses in the complete public command. All **554**
accepted runtime inputs still match `frameado3` at source **132b69b6**. The
<=0.90-times-comparable-stock goal remains unfinished.
[ADR 0183](../../../../docs/adr/0183-shared-slope-factor-closeout.md) records the
decision and the next implementation boundary.

## Sources and scope

Accepted `frameado3` plugin SHA-256:
`4a326b8d3b210084620478dfd857ffdca672db5aacfc7acbd82fbc3194b98af8`.
Public prototype `slopefactor1`:
`e923e9a4d8f7c7010ef786cdd050ab6d447bcb2d2bcf24f5d14e8ad91aa75ec2`.
Accepted-source input/counter capture `slopeframecapture1`:
`176af9106d6e866db065527f31dc166956a28f245b577e8295c0fd19ead02a9b`.
The latter's clocks never enter performance rankings. Both newly linked
plugins pass the actual ABI/self-test and injected panic-containment harness.

The reference is unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Public comparisons match threads, robust inference and saved means, with
`glm.tol=1e-10`, `fixef.tol=1e-11` and single-thread BLAS. Generation, import,
process startup and warmup are outside the command clock; native loading,
estimation, inference and requested mean output are inside. These positive,
unweighted numeric cases do not establish separation or frequency-weight
inference comparability. Prior default-reference discrepancies remain separate.

All experiments use existing tuning seed202609347 inputs. No frozen validation
seed becomes a tuning seed, and no fresh holdout campaign follows the public
rejection. No compilation, profiling, archiving or competing fits accompany
the paired measurement blocks. Every completed measured application passes
its process-isolation checks; distributions and every loss remain in receipts.
These are local development experiments, not qualification or a production
memory-policy claim.

## Implementations

Both native prototypes use the existing rank-aware local weighted QR geometry
and symbolic group-pair coupling. They assemble the reduced Schur matrix,
factor a diagonally shifted proposal system with 32-column blocked Cholesky,
and share the factor across all RHSs. The shift is
`sqrt(epsilon)*max(diag(S))`; it changes neither the target problem nor rank.
Refinement uses the unshifted problem and original rows. Only a freshly
reconstructed projection at the unchanged tolerance can proceed to the full
independent FE certificate. Failed proposals retain the certified scalar CG
fallback. Direct residual Gram products remain direct, without canonical
`H-U'U` subtraction.

The first prototype reconstructs after every refinement. The single correction
uses current coupling moments for the first correction, then reconstructs once
for acceptance, and reuses one panel bank during factorization. On the frozen
strict cases it uses two factor solves and one reconstructed check per RHS,
with no CG fallback. The full caller certificate is additional and unchanged.
The public prototype also owns its factor inside the current coupling generation:
construction is lazy, compatible RHS calls share it, and every coupling refresh
invalidates it before new numeric work, including ordinary joint work.

Immutable owner setup is outside both native clocks; numeric geometry, matrix
assembly, factor/panel allocations, normalization, projection, checks, output
materialization and direct residual products are inside. The native baseline
branch uses the accepted CG recurrence in the same test binary. The real public
baseline is the unmodified accepted artifact. Test-source identities and all
three release-test binary identities are archived separately.

## Cold strict frames: promising kernel results were insufficient

Eight prior frozen N61440 frames cover two/three FE, K6/K10 and two weight/
response states; two additional N30720/K6 two-FE frames come from the existing
public design. Each runs T1/T4/T8 with five alternating B/C rotations.

The first prototype loses 18 of 30 medians and 87 of 150 pairs. The corrected
one loses 14 medians and 70 pairs. Representative corrected medians, milliseconds:

| Frame | T | Baseline | Shared factor | Ratio |
|---|---:|---:|---:|---:|
| Three FE, K10, first weight frame | 1 | 115.836 | 72.032 | 0.622 |
| Same | 4 | 43.528 | 31.607 | 0.726 |
| Same | 8 | 34.173 | 26.362 | 0.771 |
| Three FE, K10, second weight frame | 8 | 34.621 | 26.341 | 0.761 |
| Three FE, K6, first frame | 8 | 21.059 | 22.184 | 1.053 |
| Two FE, K10, first frame | 8 | 19.234 | 20.272 | 1.054 |
| Smaller two FE, K6, first frame | 8 | 5.641 | 7.016 | 1.244 |

These cold frames use 1e-12 throughout. They do not reproduce the initial
public tolerance schedule, all actual RHS blocks, or compatible-call reuse.
The two prototype roots preserve all 600 calls and both complete distribution
tables, not just the favorable subset above.

## Complete public screen

Three rotations, milliseconds shown as median [minimum–maximum]. The public
prototype has no production admission for its extra banks and is capped at
2,048 reduced coordinates solely to bound the experiment. Do not interpret this
cap as an adopted speed guard or its public memory counters as a complete model.

| Robust + means, existing seed347 | Accepted | Prototype | Stock | Accepted / stock |
|---|---:|---:|---:|---:|
| Unbalanced three FE/slopes, N61440 K10 T8 fast | 223 [222–224] | 238 [235–238] | 231 [227–234] | 0.965 |
| Same, low | 240 [238–244] | 243 [243–244] | 230 [230–232] | 1.043 |
| Two slope partitions, N30720 K6 T8 | 104 [103–105] | 107 [106–111] | 80 [80–82] | 1.300 |
| Unbalanced three FE/slopes, N30720 K6 T4 | 114 [112–114] | 119 [119–122] | 117 [113–119] | 0.974 |
| Two slope partitions, N30720 K6 T1 | 150 [146–151] | 148 [147–152] | 317 [316–320] | 0.473 |
| Two categorical FE, N21120 K2 T4 control | 36 [35–36] | 36 [36–37] | 31 [30–31] | 1.161 |

There are **14 losses, two ties and two wins in 18 pairs**. Four case medians
worsen, one improves and one ties. The serial 2 ms gain does not justify the
other losses or another solver bank. The goal's remaining accepted-source
budgets on these particular cases are 15.1 ms fast, 33 ms low, 32 ms two-slope
T8, 8.7 ms smaller three-FE and 8.1 ms narrow categorical. Earlier fresh415
losses retain their own source/case evidence; these tuning cases do not replace it.

## Actual-call diagnosis and matched replay

An accepted-source diagnostic records the original canonical RHS blocks,
current weights, FE codes/slopes, geometry-reuse flags and actual CG operation
counts. Both counter runs pass exact scientific-output comparisons against
the accepted artifact. The first records valid counters but no input files:
the normal benchmark worker strips unapproved `FEREG_*` environment keys.
The explicit-forwarding wrapper initially fails on a missing timeout argument,
before any fit; that error is retained. The corrected separate run captures
the inputs. No counter/dump clock is ranked.

The initial tolerance is the existing scheduled 1e-6; strict is 1e-12. The
replay obtains those values from the unchanged scheduling functions and retains
the original owner's maximum RHS capacity. It selects every full-size multi-RHS
block, six in total, and runs five rotations at T1/T4/T8. Each replay rebuilds
geometry, so it charges setup even for captured calls marked reusable. It does
not claim to reproduce complete public or compatible-call timings.

| Actual canonical block, T8 | Captured CG products | Baseline replay | Factor replay |
|---|---:|---:|---:|
| Large fast initial, 12 RHS | 87 | 20.512 | 26.881 |
| Large fast strict, 11 RHS | 170 | 26.684 | 26.057 |
| Large low initial, 10 RHS | 80 | 20.407 | 26.178 |
| Large low strict, 10 RHS | 155 | 28.489 | 25.675 |
| Smaller two-FE initial, 8 RHS | 49 | 5.530 | 7.046 |
| Smaller two-FE strict, 7 RHS | 105 | 5.652 | 6.784 |

Baseline product counts match the captured native calls exactly in every
repetition and thread setting. The factor loses 58 of 90 replay pairs. Several
separate scalar refreshes use zero or one CG product; these are retained as
captures and counters but not included in the six-block replay. Construction
of a dense factor and unconditional refinement do not amortize across this
mixture of cheaper initial/scalar solves and modest strict gains. This is
measured attribution; it does not establish that tolerance alone explains
every difference between the earlier cold frames and the public command.

## Science, memory and limits

- **780 native timed calls**, all application/isolation checks passing.
- **156 independent explicit-design configurations**: 120 strict cold WLS
  projection/Gram/coefficient/bread checks, plus 36 actual-block projection/Gram
  checks at their scheduled targets. The arbitrary canonical blocks are not
  treated as response-first finite regression problems.
- Three executions of the analytic weighted-contrast Rust fixture across the
  captured test sources, covering two/three FE, duplicate slope channels,
  changing weights, T1/T4 and columns scaled by 1e±60.
- **66 public states**, **42 baseline comparisons**, **18 direct stock
  comparisons**, all passing. Twelve states are diagnostic, not ranked.
- Three licensed fixtures pass with explicit return codes and terminal
  sentinels: multipartition slope Poisson/Hessian, three-partition slope
  Poisson/Hessian, and three-FE OLS batch oracle. No broad Rust, qualification,
  constrained-memory, SCC or cross-platform campaign was run.

The 909-coordinate factor retains **6,610,248 numeric bytes**; factor plus
panel requires **6,842,952 bytes**. The 256-coordinate case needs 524,288 and
589,824 bytes respectively. Existing geometry, coupling, observation and worker
banks remain. Assembly has separate temporary row/index buffers; these payload
figures exclude descriptors, allocator overhead and the rest of the fit.
The inherited native `extra_bytes=0` print field is explicitly a placeholder,
not a memory measurement. Its clarification is archived. Production preflight,
fallible owner integration, capacity audits and physical caps were not pursued
after the public rejection. No new memory policy or runtime source is adopted.

Existing static-inventory drift, weighted constrained-fit and nonretaining-CG
failures, prior comparator discrepancies and platform deferrals remain open.
This experiment neither repairs nor masks them.

## Receipts and next work

The [verified receipt container](receipts-content-addressed.zip) holds **4,272
logical receipts / 1,098 payloads / 9,855,150 bytes**, SHA-256
`3183af44df30a279de31d001bb52ffe33c18c5224299f031a005f1e56f5b33f8`.
[restore_receipts.py](restore_receipts.py) verifies and restores each logical
file. A complete restore and licensed-header scan pass. Source identities,
prototype/capture sources, all observations, declarations, audits, failures,
controllers and protocols are included. Large generated data/binaries and raw
licensed logs have hashes; raw licensed logs are excluded. Work roots are
`target/ppml-slope-factors-20260925`, `target/ppml-slope-factors2-20260925`,
`target/ppml-slope-factor-replay-20260925` and
`target/ppml-slope-factor-public-20260925`.

Move to the ordinary joint-direction boundary. The preceding accepted-source
whole-command diagnosis places it near 68 ms for large slopes and 38 ms for the
smaller two-partition fit. Attribute its current-weight normalization, group/
finite moments, operator applications, reconstruction and checking on actual
scheduled frames. Then test a coherent sufficient-statistic construction that
performs elimination in coefficient space and avoids repeated observation
passes. Such work remains a direction proposal with fresh original-input
acceptance and the observation-operator fallback; canonical rank, direct Gram,
inverse and final Hessian retain their existing authority. The failed dense
factor, paired lanes, retained-coefficient layout and metadata experiments are
not further tuning targets. No push, release or goal-completion claim follows.
