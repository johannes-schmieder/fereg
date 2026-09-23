# Contiguous RHS arithmetic in generic owned-FE projection

Local Mac development milestone, 2026-09-22. Baseline `rawtile1` matches source
`738ce242`. Candidate `projrow1` is that source plus the captured
`crates/fereg-core/src/fe/projection.rs` overlay; all 529 build inputs match the
resulting computational source. Candidate plugin SHA-256:
`4bec0e08a01eb8f642c87f9398186d1c3345ce1b9ba3b090b78c6c4b9bc5c522`.
Baseline plugin:
`016f3fb22bbf2b50f81b5b900a4c90d0e82997507686aefb57e41382cb9ab343`.

## Opportunity and implementation

The preceding inventory identified large current losses in multi-partition
slopes. The accepted one-partition geometry cache is deliberately guarded out;
these models use generic owned-FE projection and fresh certification. This
milestone separates projection costs before considering a larger geometry
cache. It does not reopen closed single-partition or serial-window experiments.

For the original two-partition and unbalanced three-FE cases, separate baseline
diagnostic builds measure initial-batch RHS moments at 46/67 ms, small solves
at 14/2 ms, subtraction at 39/57 ms and validation at 3/5 ms. RHS arithmetic is
the larger immediately avoidable cost. Generic projection totals include
per-group clock overhead and must not substitute for uninstrumented timings.

[ADR 0114](../../../../docs/adr/0114-owned-fe-projection-rhs-rows.md) binds
contiguous input and channel-moment rows, then reconstructs fitted values in
32-RHS tiles. Per-coordinate observation/channel order, weighted-basis FMA,
zero initialization and final subtraction are unchanged. Raw Grams, pivoting,
rank cutoff, certificates, solvers, DoF, weights and publication remain intact.

Candidate diagnostics reduce moments to 29/43 ms and subtraction to 36/44 ms.
Whole projection totals fall from 125→108 and 135→98 ms. Certificate totals
remain about 176/176 and 174/177 ms, with the same 10/9 certificate calls and
36/50 projection calls. Both instrumented artifacts remain separate from all
headline timings. Stock fixest 0.14.2 is uninstrumented throughout.

The change adds one fixed 256-byte stack tile, no heap allocation or retained
geometry. It carries to existing generic MAP and accelerator callers, including
current-weight shared callers when they reach this kernel. It does not change
borrowed categorical solvers, LSMR or the one-partition cache. Nothing persists
across PPML calls, changing weights, samples or generations; fresh residual,
score and final-Hessian certificates are still required.

## Alternating timings and losses

Three repetitions rotate baseline, candidate and stock order. Builds, oracles,
profiling and archiving occur outside headline blocks. Dataset import and input
hashing precede command timing; fereg's native SPI loading, term preparation,
estimation and requested public results remain inside. These rows request no additional saved outputs;
licensed saved-slope fixtures separately protect reconstruction. Stock uses
its ordinary memory strategy, so the low-memory row is not an equal-budget
comparison. Times are complete requested-command medians in milliseconds.
Ratios are paired candidate/baseline minimum, median and maximum.

| Case (N, K, threads) | Baseline | Candidate | Stock | Paired ratio min / median / max |
| --- | ---: | ---: | ---: | --- |
| Two slope partitions (61440,10,4) | 370 | 333 | 58 | .898 / .911 / .926 |
| Unbalanced three FE, two slope partitions (61440,10,4) | 428 | 393 | 72 | .879 / .929 / .939 |
| Fresh wide two-partition slopes (76800,33,1) | 1027 | 901 | 688 | .861 / .874 / .887 |
| Fresh unbalanced three slope partitions (76800,18,8) | 902 | 736 | 132 | .808 / .816 / .824 |
| AW robust two-partition control (30720,10,4) | 328 | 303 | — | .918 / .930 / .938 |
| PW clustered two-partition control (30720,10,4) | 326 | 305 | — | .921 / .936 / .939 |
| Positive-DoF weak native control (15360,10,4) | 103 | 94 | unranked | .904 / .913 / .931 |
| Disconnected native control (30720,7,1) | 2533 | 2513 | unranked | .984 / .992 / .993 |
| Low-memory two-partition slopes (30720,12,1) | 375 | 378 | 97 | .982 / 1.013 / 1.024 |
| Cached one-partition control (61440,10,4) | 29 | 29 | 24 | .604 / .966 / 1.036 |
| Categorical one-FE control (61440,10,4) | 18 | 19 | — | .905 / 1.000 / 1.056 |
| Categorical two-FE control (61440,10,4) | 79 | 78 | — | .987 / .987 / 1.026 |
| Public categorical PPML control (61440,6,4) | 208 | 208 | — | .995 / 1.000 / 1.059 |

Seven eligible slope rows improve 6.4–18.4% by paired medians. All six direct
stock timing rows still favor fixest; the three-slope-partition inference
comparison uses the existing conservative DoF policy, discussed below. The
weighted rows deliberately absorb x1/x2 and preserve omitted statuses; they
are native controls, not stock-weight or frequency-weight inference claims.

The low-memory row has a 1.3% paired median loss and an individual +9 ms loss;
PPML has a +12 ms pair despite equal medians. Other control losses reach
+1/+2 ms. The cached one-partition baseline has one 48 ms outlier, retained in
the distribution; it does not establish a cache improvement. No favorable
subset, tile-size search or benchmark-specific guard follows these results.
Disconnected improvement is below 1%; that case is closed for this experiment.

## Scientific checks and comparison boundaries

All 96 headline application calls succeed, without timeout, detected overlap
or monitor error. All 39 native pairs and 52 repeat comparisons pass unchanged
scientific envelopes. Every native pair has identical scientific CSV exports,
including status, DoF, certificates and covariance. All 18 direct stock
headline comparisons pass their unchanged envelopes.

Independent bounded pilots cover strong two-partition slopes, unbalanced
three-FE slopes, the new weak graph and three slope partitions. The first
three pass independent coefficients, full finite covariance, sample, RSS and
rank checks on baseline, candidate and stock. The fourth exposes a deliberate
policy distinction, not a projection discrepancy:

- Explicit FE rank is 315, while both native artifacts and stock apply the
  existing conservative count 316: residual DoF 1598 rather than exact-rank
  1599. Coefficients, sample and RSS pass the original oracle; all 36 finite
  covariance entries fail its exact-rank scale (maximum 1.24e−7).
- A separate, archived diagnosis counts group channels independently, verifies
  each pairwise graph is connected, verifies full within-group channel rank,
  and exhibits the shared-global-z1 null direction across g1/g3. The declared
  pairwise/continuous policy removes the two intercept redundancies, retaining
  the conservative extra slope count.
- Fresh explicit-dummy covariance using that independently calculated policy
  denominator passes for both artifacts and stock. Native maximum error is
  6.73e−14; stock 2.44e−12. The original exact-rank failures remain archived.
  Neither the oracle nor production rank/DoF policy or tolerance is rewritten.

The additive weak design has 24 observations per worker and four observations
linking two well-populated blocks. Its first N1920 pilot establishes FE rank
167, finite rank 6 and positive residual DoF 1747. Native science passes;
stock misses four covariance coordinates by at most 2.28e−10, up to 2.16 times
the unchanged envelope. A fresh-seed weak pilot passes for all three artifacts,
but does not erase the first failure. The larger native-only weak screen has
matching successful results; no stock timing ranking is claimed for that
family. Prior weak_d3 warmup failures and disconnected stock disagreements
remain unresolved and their stopped campaigns were not restarted.

Additional validation passes:

- 31 distinct focused Rust tests: projection, automatic routing, all generic
  joint-budget regressions, accelerators, eight independent FE certificate
  oracles and eight absorbed-slope oracles.
- Both projection tests also pass in optimized builds. Scalar bit checks cover
  widths 0–129, intercept and slope-only forms, unequal/unit weights, empty
  levels, duplicate/zero channels, extreme scales, signed zeros and repeated
  workspace use.
- Actual-plugin ABI/panic containment, five harness boundary tests and four
  licensed fixtures: multiple slope-only OLS, intercept-plus-slope OLS,
  public PPML saved slopes, and OLS eclass failure atomicity. Fixture return
  codes and terminal sentinels are checked explicitly.

## Memory, scope and next subsystem

All 39 paired tracked native heap peaks are identical. The added 256-byte
stack tile is not a whole-process memory bound; no tighter constrained-budget
claim is made. The static allocation audit continues to fail documented source
drift and is not regenerated. Weighted constrained-fit failures, nonretaining
CG span instability, factor-base provenance and SCC/Linux deferrals remain
open. The 77 primary, 16 holdout and 111 native-control definitions are intact.
Seeds 202609255–264 belong to this bounded work; reserved validation seeds
202609211/212 are untouched.

Close this projection-kernel experiment. The next useful subsystem remains
fresh certification in generic owned-FE solves: about 176 ms versus 98–108 ms
of projection in the diagnostics. Before adding a cache, trace which same-call
weighted normalization/QR geometry can be reused under a precise ownership
and memory budget, and compare solver/certificate scheduling with an isolated
pinned fixest diagnostic. Fresh residual/span checks must remain independent.
Keep weak-reference precision and conservative-DoF adjudication separate from
speed tuning. Do not chase the disconnected <1% residual or tune either tile.
This milestone is local development, not broad completion, qualification,
push or release.

## Immutable receipts

The accompanying archive and checksum preserve declarations, exact artifact
identities, captured controllers, input hashes, all attempts and exports,
independent diagnoses, tests, phase clocks and sanitized fixture protocols.
Data, binaries and raw licensed logs are excluded. The source binding verifies
all 529 candidate build inputs. The stock archive and loaded library hashes
are retained separately; no instrumented comparator enters the timing table.

[Receipt archive](receipts.zip) and [checksum](receipts.sha256): 3,581 verified
entries plus the hash manifest, 5,125,019 compressed bytes. SHA-256:
`f2f1dcf149f93dcd6e368e919619e18f164aa50f44e7c4b76ddf6f6daa7a5b13`.
