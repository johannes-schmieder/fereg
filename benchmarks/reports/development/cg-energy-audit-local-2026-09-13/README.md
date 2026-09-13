# P6 prerequisite: independently audited CG energy roundoff

Adopt the narrow borrowed-CG repair in [ADR 0059](../../../../docs/adr/0059-borrowed-cg-negative-energy-audit.md).
The accepted baseline's saved-output experiment failed on a weighted constant
column: subtracting its computed improvement from its remaining energy overshot
zero by two ulps. A 45-row closed-form weighted example independently reproduces
that failure. The final fixture fails on an isolated accepted core and passes
with the repair. This is a correctness repair, not a measured speedup.

## Boundary and identity

Only a subtraction below the existing negative-roundoff floor enters the audit.
A discrepancy above 64 epsilon times max(previous, decrease, 1) still fails.
Inside that envelope, two scans of the affected RHS independently compute the
current and pending weighted energies using scaling and compensated sums. Both
the previous potential and actual step decrease must agree with the recurrence.
The new potential is the measured pending energy, not an arbitrary zero clamp.

The current iterate, multiply/subtract update, raw reference scales, coefficients,
Hestenes quotient and stop, fresh FE certificate and stricter reconstruction
certificate remain unchanged. Nonfinite quotients still fail. The audit's energy
calculation uses fixed stack arrays, with no heap or retained workspace. Ordinary
successful updates retain their arithmetic; routing, weight and PPML policies do
not change. The consistency envelope is not a convergence tolerance or a promise
that arbitrarily accumulated energy error is recoverable.

[Identities](identities.json) bind 176 source/ado inputs, the [patch](implementation.patch)
and isolated universal plugins against accepted numerical `257f4ce`, evidence
HEAD `d4d7e97`:

- Baseline: `c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`.
- Candidate: `e831a6574268402682aec9432a5275cfce1697cc94feb8472f1a95f2f834ae26`.

Only `fe/borrowed_map.rs` differs among those inputs. Both changed crates visibly
compiled for each architecture with Rust 1.85.1 and locked dependencies; the
universal static audit and actual native arm64 ABI/self-test pass. The installed
plugin was neither a baseline nor replaced. A formatter's unrelated changes were
removed before the final build and tests. Earlier builds remain separate.

## Original workload and scientific validation

The [preceding failed PW screen](../saved-schur-probe-local-2026-09-13/README.md)
is replayed without replacing its data, seed, warm-up, worker, weight, two-way
clustering or partial saved-output request: N245760, K50, 16 native workers.
Its dataset SHA remains `eecf8666668f96734ee06c52310b9920f29c65fc9cce86fd3707be22399fc7d0`.
The repaired command passes in **3.510 seconds**, including requested saved FE
and residual output. The original baseline r(459) is retained as a failure and
is never used as a timing denominator.

A separate same-input trace passes in 3.484 seconds. It confirms exactly one
energy audit, on RHS 51, the explicit constant: previous energy
139583.428571428522, decrease 139583.428571428580, independently recomputed current
energy 139583.428571428551 and pending energy zero. Original raw scales and the
final independent certificates remain authoritative. [Phase records](original-pw-phases.txt)
are separate from the untraced command; no isolated audit-time claim is made.
The source cost is two reads of one affected RHS/direction column plus scalar
weight arithmetic, not an additional full N-by-K pass.

[Validation](validation.json) records **57 distinct candidate Rust tests**:
41 borrowed-map and 16 topology/recovery tests. Coverage includes the independent
weighted mean, nonzero near-constant residual energy, stale/inconsistent steps,
nonfinite/overflow rejection, weighted PPML cancellation, weak graphs, exhausted
iterations, forced continuation, coefficient recovery and fallible memory bounds.
The [baseline reproducer patch](baseline-reproducer.patch) adds only the final
closed-form test to accepted source; its expected failure remains captured.

Both isolated plugins pass 48 saved-output OLS explicit-dummy fits each, covering
unit/AW/FW/PW, fast/low, one/two FE, restricted/missing samples, partial/combined
outputs, normalization and rollback. Both also pass the independent one-FE
Poisson/Hessian/sandwich fixture. These tests retain explicit rc=0 terminal
sentinels; a process exit alone is not acceptance. No full Stata profile, x86
runtime, SCC, Windows, external-estimator timing or qualification was run.

## Protected cost and memory review

The [prospective protocol](protocol.md) specifies twelve single-seed rotated
pairs in fresh local M2 Ultra Stata/MP 19 processes after an untimed small warm-up.
Builds, tests and profiles do not overlap timing. All **24 control commands** pass;
full scientific exports, sample/rank/status/DoF, saved outputs, certificates and
solver decisions are exact. All twelve native peaks, memory models and scratch
sizes are exact. [Summary](summary.csv), [comparisons](comparisons.json) and the
[checksummed numeric archive](archive.json) preserve all observations.

| Control | Command seconds, baseline → candidate | Native peak |
|---|---:|---:|
| Weak / dense two FE | 0.042→0.043 / 0.034→0.039 | unchanged |
| Bottleneck / cycle | 0.036→0.036 / 0.055→0.051 | unchanged |
| Disconnected / unbalanced | 0.032→0.031 / 0.033→0.033 | unchanged |
| Small AW robust / low FW clustered | 0.007→0.007 / 0.208→0.204 | unchanged |
| Original unit strong, saved both | 2.890→2.938 | 839.05 MB |
| Tiny weighted cut | 0.053→0.051 | unchanged |
| Complete one-FE IID | 0.575→0.566 | 925.87 MB |
| Shared weak two-FE PW PPML | 0.157→0.153 | 71.55 MB |

No increase exceeds max(10 ms, 5%), so the prescribed additional seeds are not
triggered. Largest absolute slowdown is saved-unit +48 ms / 1.7%; about 35 ms
lies in unchanged input loading and 13 ms in native work. This does not establish
an external cause. Largest percentage increase is dense +5 ms / 14.7%. These
single-pair fluctuations are not repeatable speed claims. The saved-unit pair
retains its existing fallback flag and 59 combined initial/final iterations;
all other controls report zero fallback, with exact work counts. No extra retry
cost is suppressed. PPML's native-time column uses its estimator total; an
independent SPI-input timer is unavailable there. Overlapping phase scopes must
not be summed.

Two controls retain existing observed/model gaps in both arms: low FW is
16.18 versus 14.94 MB, and saved unit is 839.05 versus 418.80 MB. The newly
successful original PW command reaches 841.84 MB against a 424.69 MB model.
These are unresolved P0/P6 admission/reporting gaps, not memory acceptance.
The audit adds no observation-sized storage and does not correct those models.
The traced PW peak differs by one byte from the untraced call; no exact equality
is claimed across instrumentation modes. Sampled RSS moves both ways, including
weak PPML 0.133→0.158 GiB and disconnected 0.077→0.095 GiB. No general process
memory reduction is claimed; RSS includes Stata/allocator state and is sampled
at 10 ms.

## Disclosed failures and next boundary

The original new test demanded 1e-13 on its variable companion after fixing the
constant. The unchanged single-column route itself has 4.99e-11 cancellation
error. The companion now uses the existing borrowed-CG 2e-10 oracle bound; the
constant retains 1e-13. No preexisting test or public tolerance was relaxed.
A separate initial test incorrectly expected success when a finite 1e300 energy
produces an overflowing Hestenes quotient; it now independently verifies finite
energy computation and the unchanged fail-closed quotient. Earlier failures and
diagnostic witnesses remain in [checks](checks/).

The first PPML oracle did not execute because its fixture was absent from the
isolated source copy (r601). It was copied explicitly, hashed, and rerun; the
already successful baseline OLS fixture was not rerun. A collector initially
used nonexistent PPML memory fields, and its next draft misindexed the OLS
fallback column. Both were corrected from the current runtime/wire definitions
before final reporting; superseded collectors remain in the archive. No measured
command was discarded or rerun for these collection repairs.

The separate saved-Schur prototype remains archived and unadopted. Resume its
original PW screen against this repaired baseline, then prove overlapping final
recovery/fallback admission and constrained-memory behavior before adoption and
primary repetitions. P6 and the remaining #27 work stay open. No agents, cluster
jobs, unrelated Veneto edits, installed-artifact replacement or push occurred.
