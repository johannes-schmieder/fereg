# PPML covariance ownership and exact pivots — local development, 2026-09-23

Adopt `psdowner1` under [ADR 0141](../../../../docs/adr/0141-ppml-covariance-eigensystem-ownership.md).
The K48 whole command improves 125 -> 108 ms; fresh K56 improves 176 -> 146 ms
versus stock's 178 ms, an **18.0% stock lead**. Other sampled wide improvements
are 3–7%. Narrow PPML is essentially unchanged, with visible paired losses.
This is a completed covariance milestone, not completion of the broad <=0.90
stock-runtime objective.

## Sources and comparison

Baseline is committed `d6c18ccb`, artifact `acceptstate2`, plugin SHA-256
`aa3cbf99c229b7d7780c01a871225ff6684476dd8bf5b2251b2f127ab283fd11`.
Candidate is that source plus the single `ppml/psd.rs` overlay, artifact
`psdowner1`, plugin SHA-256
`ae620228d87df873839c06436af6a55f72c0461d4f531d1b1b766bcdad685da1`.
All **541 computational inputs** match the adopted worktree. Source snapshots,
patch, identities, build recipes and ABI/panic-containment evidence are archived.

The comparator is unmodified pinned stock fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust inference and
fitted means. Import/setup precedes both whole-command timers; fereg's command
includes native loading and publication. The screen uses development seed
202609335. Fresh width cases use predeclared seed 202609344; PPML/OLS controls
use 202609345. These seeds are now consumed. No FW inference claim uses ordinary
fixest weights as an oracle. Default-reference accuracy failures remain separate.

These are local Mac development timings, not qualification. Headline blocks
have no detected competing fits, builds, profiles or archives. Diagnostics are
separate artifacts and runs. All application status, sentinel and isolation
receipts remain visible; no failed scientific state is ranked.

## Adopted computation

An unchanged symmetrized covariance owns its existing certified eigensystem
through production acceptance. A duplicate deterministic decomposition of the
same matrix is removed. Actual multiway repair still receives independent clipped
reconstruction certification and a fresh eigensystem of the repaired matrix.
The separate publication validator always inspects the actual output anew,
retaining rank, minimum eigenvalue, report consistency, diagnostic-underreport
and further-repair-distance checks. All other solver, score, objective, final-
Hessian, weight, sample, DoF, separation and transactional checks are unchanged.
No tolerance is relaxed.

The established exact maximum-pivot row cache now serves every width, using
already allocated eigenvalue storage. The exhaustive production branch is
retired and retained as a test oracle. Pivot choices, tie order, rotation
arithmetic, stopping threshold and iteration cap remain exact. No retained
state, additional heap payload, worker team or SPI access is introduced.
This does not resume the rejected residual-extrapolation or serial-slope probes.

An unrepaired covariance now needs one production plus one independent
publication eigensystem, instead of two plus one. Repaired covariance still
needs two production plus one publication eigensystems. One unrepaired call
avoids allocating a work matrix, eigenvector matrix and eigenvalue vector a
second time. Their former overlap was sequential, so this is allocation-volume
reduction, not a claim of lower peak memory. `work-counts.json` distinguishes
these source-derived counts from measured timings.

## Whole-command distributions and losses

Milliseconds: median [minimum, maximum]. Screen/fresh widths use five rotations;
PPML controls use three. W/T/L counts paired candidate gains/ties/losses against
baseline. Science is compared before timing. Every displayed case requests
robust inference and fitted means.

| Case | Baseline | Candidate | Stock | Candidate/stock | Paired W/T/L |
|---|---:|---:|---:|---:|---:|
| screen 0: low, one_fe, N61440/K10/T4 | 94 [92, 99] | 94 [92, 97] | 54 [51, 58] | 1.741 | 2/0/3 |
| screen 1: low, one_fe, N30720/K32/T4 | 114 [113, 118] | 111 [110, 113] | 118 [116, 122] | 0.941 | 4/1/0 |
| screen 2: low, one_fe, N16384/K48/T4 | 125 [123, 127] | 108 [107, 111] | 107 [102, 109] | 1.009 | 5/0/0 |
| screen 3: low, one_fe, N16384/K65/T4 | 197 [196, 197] | 191 [190, 194] | 185 [177, 190] | 1.032 | 5/0/0 |
| screen 4: low, one_fe, N3840/K3/T1 | 17 [16, 22] | 20 [15, 22] | 11 [11, 12] | 1.818 | 2/1/2 |
| fresh-validshape 0: low, one_fe, N24580/K24/T1 unbalanced | 134 [130, 138] | 133 [129, 137] | 127 [120, 130] | 1.047 | 3/1/1 |
| fresh-validshape 1: fast, one_fe, N12288/K40/T4 | 88 [87, 90] | 82 [79, 83] | 69 [65, 72] | 1.188 | 5/0/0 |
| fresh-validshape 2: low, one_fe, N20480/K56/T4 | 176 [174, 177] | 146 [143, 148] | 178 [175, 184] | 0.820 | 5/0/0 |
| fresh-validshape 3: fast, one_fe, N8192/K80/T8 | 182 [179, 183] | 170 [169, 172] | 112 [109, 115] | 1.518 | 5/0/0 |
| ppml-controls 0: fast, three_fe_dense, N30720/K6/T4 | 197 [196, 197] | 198 [197, 202] | 238 [232, 239] | 0.832 | 0/1/2 |
| ppml-controls 1: low, three_fe_dense, N30720/K6/T4 | 340 [338, 344] | 348 [347, 353] | 232 [231, 240] | 1.500 | 0/0/3 |
| ppml-controls 2: low, strong_d6, N30720/K6/T4 two slope partitions | 152 [151, 154] | 151 [149, 152] | 118 [117, 124] | 1.280 | 2/0/1 |
| ppml-controls 3: fast, strong_d6, N30720/K6/T4 two slope partitions | 123 [123, 124] | 126 [125, 127] | 120 [115, 123] | 1.050 | 0/0/3 |

All 25 K40/K48/K56/K65/K80 pairs improve; K32 has four gains and a tie.
Fresh serial K24 has a 2 ms paired loss, alongside three gains and a tie.
Narrow K10 has paired changes -5,+2,+2,-2,+1 ms. Small K3 has -1,0,+4,+1,-1 ms;
its median worsens 17 -> 20 ms. These losses are not discarded, and no universal
timing non-regression is claimed.

The first low three-FE control has +8,+15,+3 ms, and fast slopes +1,+4,+3 ms.
A prospectively declared unchanged-source, identical-data, same-seed five-rotation
recheck follows; it does not replace or pool away the initial block:

| Recheck | Baseline | Candidate | Stock | Candidate minus baseline, paired ms |
|---|---:|---:|---:|---|
| Low three-FE | 362 [354,365] | 359 [352,364] | 238 [237,241] | -6,-12,-10,+10,+4 |
| Fast two slope partitions | 127 [125,128] | 127 [126,129] | 118 [117,119] | -2,0,0,+1,+4 |

The initial median slowdowns do not persist in that recheck. Their cause is not
identified. Both controls remain substantial targets against stock. Two OLS
controls have medians 67.5/67.5 and 63/63.5 ms, paired differences -2,+2 and -1,+2,
with exact scientific exports. These are focused controls, not a whole-family
performance proof.

The original fresh block stopped in data preparation: N24576 leaves one final
singleton in the five-row panel generator. No dataset or fit was produced.
Before any fresh results, only that count was changed to N24580; the seed and
other cells stayed fixed. The failed `fresh` preparation, declaration and
replacement `fresh-validshape` receipts are retained. `wideledger1` likewise
failed instrumentation before compilation due to a misspelled function name;
`wideledger2` is the corrected baseline diagnostic. Neither failed setup has a
timing claim. No application timeout or scientific discrepancy occurred in the
completed blocks.

## Attribution and remaining work

Separate one-rotation `wideledger2`/`wideledger3` traces record wall intervals.
Exclusive accounting subtracts the union of captured child spans and sums to
each plugin interval. These diagnostic intervals are not headline observations.

| Width | Baseline eigensystems, ms | Candidate eigensystems, ms | Baseline/candidate native total, ms |
|---|---:|---:|---:|
| K10 | 0.123, three calls | 0.117, two calls | 77.331 / 77.776 |
| K32 | 4.266, three calls | 1.385, two calls | 94.115 / 91.226 |
| K48 | 19.502, three calls | 4.362, two calls | 100.427 / 85.718 |
| K65 | 16.343, three calls | 10.749, two calls | 173.788 / 164.872 |

The K48 eigensystem reduction localizes most of its improvement. The experiment
adopts the two compatible changes together; it does not factorially attribute
every millisecond between ownership and caching. Unchanged kernel fluctuations
are not assigned to the patch. Current diagnostic robust-score reductions still
take 11.052 ms at K48 and 17.931 at K65; duplicate bread products take less than
2 ms. Production fresh K56 inference falls 35.393 -> 13.585 ms and publication
16.435 -> 7.312 ms. These are nested marginals and must not be added as an
exclusive total.

Remaining <=0.90 targets are concrete: K32 needs 111 -> 106.2 ms; K48 needs
108 -> 96.3; fresh fast K40 needs 82 -> 62.1; fast K80 needs 170 -> 100.8.
Narrow K10 is still 94/54 and needs 48.6 ms. Low three-FE remains 348/232 in its
initial control and 359/238 on recheck; fast slopes are 126/120 and 127/118.
The fresh K56 case and three-FE fast control meet the desired margin in their
captured cases, without establishing a family-wide win.

Next, investigate common current-weight centered finite products shared by
concentrated one-FE directions and canonical construction, plus the measured
robust-score reductions. Reuse requires exact current sample/weight/basis
identity and retains independent final-Hessian witnesses. Narrow K10 needs
roughly half its current total removed, so preparation, canonical construction
and publication must also be addressed. For low multi-FE, the recheck's strict/
final marginal is 177.963 ms, versus only 0.664 ms inference: profile that
checkpoint and its existing coefficient/joint/categorical owners on identical
frozen frames. Another covariance threshold or likelihood-cache experiment
cannot close that gap. Keep at most two materially different candidates per
cost center, then move on when gains are small or inconsistent.

## Science, memory and immutable evidence

- **391 distinct Rust tests** pass, including closed-form Householder spectra,
  tiny-negative/rank policy, multiway repair, forged-report/matrix rejection,
  current PPML contracts and exhaustive-versus-cached pivot checks through
  dimensions 1/2/7/10/16/32/48/63/64/65/129/257 and extreme scales.
- **Four licensed fixtures** pass with explicit application return codes and
  terminal sentinels: independent full-dummy Poisson/Hessian/sandwich with
  FW/PW, offsets and robust/cluster inference, multipartition slopes, one-FE
  oracle controls and separation methods. Production ABI/panic containment passes.
- **218 passing headline-block applications**, 210 passing PPML state audits,
  nine small independent explicit-design Poisson oracles, 70 baseline/stock
  plus 70 direct candidate/stock comparisons. All **70 native PPML pairs have
  byte-identical scientific CSV exports and fitted-mean binaries**; four OLS
  pairs have exact scientific exports. Eight separate diagnostic applications
  pass their public-state audits.
- All 70 native PPML pairs have exactly equal outer/inner counts, tracked action
  peaks, modeled selected memory and scratch bytes. K48 peak is 20,993,654 bytes
  against a 26,384,465-byte model; fresh K56 is 29,711,898 against 37,033,540.
  Optional routing and memory budgets are unchanged. This is not a physical-cap
  result or repair of the historical allocation inventory/constrained-fit issues.

The restored content-addressed archive verifies **8,522 logical receipts**,
1,946 payloads, 14,166,306 bytes. SHA-256:
`1f63e1cb755150e171a477985a624bf9f29995e41ec5f24e3fd3aa7dc4370e78`.
It preserves exact source identities, harnesses, patches, declarations, audits,
all distributions, failures and sanitized outputs. Licensed raw logs, binaries
and datasets are omitted with hashes. Run `restore_receipts.py` into a new
folder to reconstruct and verify the logical receipts.

Frozen 77/16/111 definitions and protected seeds 202609211/212 are unchanged.
No SCC retry, stopped campaign, release, push, full qualification, Windows/Linux
or constrained-memory confirmation occurred. Their existing deferrals remain.
