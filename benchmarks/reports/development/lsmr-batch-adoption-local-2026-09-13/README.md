# Bounded LSMR batches: local adoption after ordinary-command review

Adopt [ADR 0049](../../../../docs/adr/0049-bounded-lsmr-batches-and-memory.md)
against native baseline `ac47b1907c7bc7bc8ee78de85621b0cc96546a4c`.
The immutable [preceding investigation](../lsmr-batch-memory-local-2026-09-13/README.md)
at `cd298d5` records the then-unadopted prototype, all 72 comparison pairs,
allocation probes, oracles and timing failures. This report supersedes its
adoption decision without rewriting or excluding that evidence.

## Implementation and identity

The private adapter retains one weighted solver and windows independent RHSs
only when their number exceeds both 64 and four times the installed worker
team. Each window has at most that team's width. Narrow batches call the
original upstream entry point. All returned columns and diagnostics remain in
original RHS order; any window failure drops the partial result before the
existing whole-batch fallback. New transport reservations are checked/fallible
and retain allocation-error classification. Numerical recurrences, tolerance,
rank, original reference scales and independent certification are unchanged.

Shared OLS/PPML memory accounting now includes coefficient-space recurrences,
the actual batch window, transformed inputs and output overlap, primary dense
cross-tabs as well as weighted compensation, permutation/scatter state and
Schwarz worker stacks/pools. The ordinary PPML joint solver's arithmetic is
unchanged. Its shared setup estimate changes. A configured machine-default
thread count is not substituted for the installed team.

[Identities](identities.json) bind 152 final native inputs. The reviewed source
differs from the measured wide-guard source only by a corrected test comment:
the omitted primary table is not, by itself, larger than the old whole estimate.
There is no executable or assertion change. Its universal rebuild is byte-identical
to the measured candidate, SHA-256
`c9db0dceea82dc7b7b87a51979d0fd7ca1f83f1e71f04f034be32312527990f6`.
The baseline plugin is
`6c364b062dc505f478ad9079a8106db4e89c0aed4079735ad89d939f7d1a0497`.
[Seven final changed files](changed-native-inputs.zip) and their
[checksums](source-archive.json) preserve the source delta. Installed dist was
neither used as the baseline nor replaced.

## Memory result and protected command cost

These repeated wide-guard results are retained in the preceding report:

| Target | N / K / threads | Native peak, MB | Command medians, s | Median paired command change |
|---|---|---:|---:|---:|
| Weak OLS, AW robust | 122880 / 65 / 4 | 403.31 → 351.32 (−12.9%) | 0.995 → 1.006 | +1.5% |
| Weak PPML, PW/offset/cluster | 30720 / 65 / 4 | 182.18 → 169.43 (−7.0%) | 1.332 → 1.344 | +0.5% |

There are three seeds per target. Wide OLS saves exactly 51,985,712 native peak
bytes per pair. Its input/native/outer medians are 0.048804/0.918453/0.028069
versus 0.048377/0.928307/0.028372 seconds. PPML preparation, projection, finite,
strict/final and inference medians are all retained in the earlier summary.
Independently calculated and overlapping medians must not be summed. Six OLS
controls include narrow/small weak graphs, strong Schur, complete IID, low FW
and generic explicit Schwarz LSMR. No control increase exceeds 7 ms. Successful
science and non-time solver/retry decisions remain exact. These are memory
gains within a command-cost envelope, not speedup claims.

The preceding six traced K17 PPML pairs include three increases of 21/36/38 ms
(6–11%) and a +4.0% median paired change. Model-only and adapter decompositions
narrow attribution but do not explain those losses. All remain evidence. The
model-only diagnostic is narrow-only: its unbounded runtime does not enforce
the new wide memory estimate and must not be adopted.

## Prospective ordinary-command follow-up

The [protocol](protocol.md) specifies six identical-binary A/A pairs interleaved
with six accepted/candidate A/B pairs. The same weak K17 PPML input, PW/offset/
cluster options and four native workers use three seeds, each in both arm orders.
Each command has a fresh local Stata/MP 19 process and an untimed warm-up on the
M2 Ultra. Stata profiling, phase tracing and native solver tracing are disabled;
ordinary runtime phase timers remain. No build or test runs alongside timing.

| Comparison | Pairs | Command medians, s | Median paired change | Per-pair candidate minus baseline, ms |
|---|---:|---:|---:|---|
| Identical accepted binary A/A | 6 | 0.3445 → 0.3490 | +0.4% | +18, −2, −19, +5, −15, +5 |
| Accepted/candidate A/B | 6 | 0.3635 → 0.3485 | −3.3% | −6, −8, −17, +3, −16, −23 |

All A/B commands fit the original max(10 ms, 5%) review envelope. Five improve;
the sole increase is 3 ms. A/A demonstrates measurable variation with identical
code, but does not identify the cause of earlier traced losses. Do not interpret
the −3.3% result as a causal narrow-path speedup or erase the traced regressions.
The ordinary commands do not reproduce a persistent cost sufficient to block
the repeated wide memory improvement. That is the adoption rationale.

[Comparisons](comparisons.json), [summary](summary.csv),
[numeric archive](numeric-evidence.zip), [checksum](archive.json) and
[same-artifact controls](same-artifact-controls.json) preserve all 12 pairs /
24 measured commands. Full b/V, samples, status/DoF, weights/clusters, certificates,
saved-output checks and all non-time solver decisions are exact. A/A also requires
identical selected-model bytes. Every target has seven outer iterations, no
halvings and no fallback; inner work matches within each pair. Native allocation
and sampled process RSS remain separately reported; no narrow memory or universal
RSS saving is claimed.

## Validation and remaining limits

[Validation](validation.json) links the preceding 67 LSMR-filter and 77 memory-filter
tests (134 distinct), three licensed independent fixtures in both arms, and nine
source-bound allocation probes. The tests are unchanged apart from the comment
correction. Universal static and native arm64 ABI checks pass on the reviewed
rebuild. The 65-control Fourier Poisson/dummy-Hessian/meat oracle exercises unit,
FW/PW, robust/clustered inference and 1/4 workers. Its small-sample multiplier is
the registered policy, not an independent policy oracle. The earlier ill-conditioned
smooth-column expansion fails the same bread guard in both arms and remains a
preserved failure.

The weighted 320-row admission test distinguishes the old underestimate: at a
327680-byte solver budget, baseline wrongly admits an independently measured
425777-byte construction; candidate selects certified CG. Nine final allocation
probes fit the revised estimates, but three deliberately unconverged probes are
allocation evidence only. Opaque factor constants and stack-bound tightness remain
under audit; this is not a universal proof of memory bounds. Public OLS selected
memory still omits solver increments. The pre-existing low-FW and saved-AW weak
certificate failures are not fixed or counted as passing controls.

P0 actual attempted/fallback peak reporting and construction counters remain
open, as do broader R1 reuse and P7 scheduling. No dependency pin, SPI boundary,
sample, scientific certificate, transactional publication or installed artifact
changes. No SCC, broad profile, Windows test, qualification or push occurred.

Replay `compare.py` with the repository's `benchmarks/local_fixest_gap` on
`PYTHONPATH`, `FEREG_LOCAL_BENCH_ROOT` containing the bound baseline/candidate
source trees, and `STATA_MP_PATH` set to licensed Stata. The adjacent comparison
override changes only the scientific export selection and A/A source fingerprint.
Synthetic datasets, binaries, raw licensed logs and machine paths remain outside Git.
