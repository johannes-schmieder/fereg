# ADR 0240: Reject native-only host switching; fix the Break handshake

Status: accepted development decision, 2026-09-28.

## Decision

Do not retain the candidate that sets Stata processors to one immediately
around the PPML plugin call. Its two setting calls and protected restoration
are safe under the tested transaction boundaries, but full public-command
timings do not establish a sufficiently consistent benefit. Do not add
case-specific guards or change the user's persistent processor setting.

Retain the independent correction to the hidden native Break fixture hook.
Complete `spi.display(NATIVE_BREAK_MARKER)` before creating the controller's
file handshake. The PTY driver may send Ctrl-C as soon as it sees that file;
arming it earlier races with Linux Stata's display callback, which can return
code 1 as an SPI display error instead of reaching the interruption poll.
The existing native test asserts that the handshake is absent during display.
Actual main-thread interruption polling, scoped-worker shutdown, no-store
failure behavior and transactional rollback remain authoritative.

## Evidence and scope

The [source-bound report](../../benchmarks/reports/development/ppml-host-processors-scc-2026-09-28/README.md)
preserves the failed validation, corrected artifacts, rejected wrapper patch,
all 60 headline applications, native/stock scientific comparisons and complete
distributions. All 20 native coefficient and covariance pairs are byte-identical.
Iteration counts and modeled memory agree; existing three-FE overlap accounts
for tracked-peak differences in both directions. No solver, sample, rank,
weight, tolerance, certificate, inference or memory-admission rule changes.

The median T2 gain is inconsistent across its two pairs. Principal T4, low and
larger cases improve only about 1–2%; wide and slope controls have losses.
Fresh confirmation and direct scaling were registered but not run after the
benefit gate failed. Diagnostic evidence must not be transferred into a stock
timing claim or a universal core-scaling guarantee.

A separate post-timing profile materially narrows the next experiment.
Setting one host processor before the entire public command, then restoring
the caller's count, nearly reproduces the persistent-host-one diagnostic.
Native-only versus whole-public diagnostic command medians are 213 versus
134.5 ms for one FE and 274.5 versus 185.5 ms for two FE. Stata self-sample shares
also fall sharply. All 96 fits preserve native four-worker requests, host
settings and identical coefficient/covariance exports. These are fixed-order,
shared-node profiles, not alternating headline measurements.

The supported inference is that earlier control-plane work activates competing
Stata workers before the native-only scope begins. The exact activating command
is not isolated. A successor should test a scope covering public parsing and
temporary-output preparation, with independent restoration checks for syntax
errors, native failures, Break and successful publication. It must preserve
quiet/noisy behavior, native thread selection, one substantive plugin call and
the existing transaction. No passive-wait environment policy is adopted.

## Retained result

Only the hidden Break ordering correction and strengthened boundary fixtures
remain. The original PPML ado call is restored. Both Mac and Linux exercise the
retained package's host-state fixture and genuine native Break; the unchanged
numerical binary also carries the candidate's focused independent-oracle,
ABI/panic and smoke evidence. This is development evidence, not qualification
or an accepted performance improvement.
