# Bounded local request transport: adopted development repair

Adopt [ADR 0055](../../../../docs/adr/0055-bounded-local-request-transport.md).
Long OLS/PPML model metadata now reaches the existing native action through a
fixed caller-local macro instead of exceeding Stata's plugin-option capacity.
The canonical payload, scientific wires, numerical core and one-call transaction
are unchanged. This is a transport repair, not a speedup claim.

The baseline is accepted `22b5106`; main before this experiment is evidence-only
`81a96cd`. [Identities](identities.json) bind 176 source/ado/fixture inputs and the
exact [implementation patch](implementation.patch). All fereg-core inputs match
accepted source. The final isolated universal plugin is
`7b7c6664132041e60425382edb99728a20bc258c628a68369de32852d0bb65fe`, compared with
baseline `ad39e2329579cf87295796379e12035805d923a3417b8021af37df0a198322f6`.
Both changed and numerical crates visibly recompiled for both architectures.
The installed plugin was not used as a baseline or replaced.

## Boundary and cost

The ado adapters retain inline `bundle=` requests through 4,000 payload bytes.
Longer payloads stay in local `fereg_request_bundle`; the action receives only
`bundle_local=<byte-count>`. Rust requires a sole canonical positive-decimal
descriptor and admits at most 16 MiB before reading the fixed local through SPI
on Stata's thread. The buffer holds the seven-byte decoder prefix, payload, one
extra content byte and NUL. No observation-sized preparation state is introduced.

The [Stata SPI documentation](https://www.stata.com/plugins/index.html) specifies
that the macro read may truncate and that its character limit excludes the
terminator. The extra content byte detects an overlong value even after truncation.
Exact NUL placement, UTF-8, framing and the existing closed decoder are required;
the buffer is released after parsing, before metadata validation and observation
loading. Short requests perform no macro read or payload allocation. The ado
local remains live until the transaction returns; its Stata allocation is not
native heap storage. There is no new solver, data pass or output channel.

The [prospective protocol](protocol.md) specifies eight paired cost controls and
review of increases above max(10 ms, 5%). After final SPI review, the same eight
were repeated on the final binary. Each set has one seed and rotated arm order,
fresh local Stata/MP 19 processes and an untimed smaller warm-up. No builds or
tests overlapped timing. These are local M2 Ultra development observations,
not a release, cross-platform, or external-comparator claim.

| Final-binary control | N / K / native workers | Command seconds, baseline → candidate |
|---|---|---:|
| Complete robust PPML, one FE | 983040 / 50 / 16 | 7.354 → 7.500 |
| FW PPML, exposure/missing, one FE | 61440 / 129 / 4 | 4.735 → 4.720 |
| Weak three-FE PPML | 7680 / 10 / 4 | 0.138 → 0.138 |
| Low-memory FW PPML | 30720 / 10 / 4 | 0.863 → 0.759 |
| Small one-FE PPML | 7680 / 2 / 1 | 0.032 → 0.026 |
| Saved/separated PPML | 7680 / 3 / 4 | 0.028 → 0.029 |
| AW robust OLS, missing, one FE | 61440 / 129 / 4 | 0.575 → 0.579 |
| Narrow robust OLS, no FE | 7680 / 2 / 1 | 0.008 → 0.006 |

No final control crosses the prospective review envelope. The largest absolute
increase is complete PPML +146 ms/2.0%; finite solving varies 1.2604→1.3309 seconds,
strict solving 2.4537→2.4922 and preparation 0.8394→0.8642. FW K129 preparation
is 0.1106→0.1076, finite solving 1.2256→1.2271 and inference 0.8208→0.8122.
OLS K129 input is 0.0463→0.0472 and native work 0.4685→0.4715. The unchanged
numerical work varies; neither the low-memory reduction nor any precise transport
overhead is inferred from these single pairs. Overlapping phase times must not
be summed. Full fields are in [PPML](final/ppml/comparisons.json) and
[OLS](final/ols/comparisons.json) receipts.

Native peaks are exact on seven final pairs; FW K129 falls by 4,302 bytes, from
288,013,568 to 288,009,266. All fit their unchanged models. Sampled process RSS
moves both ways: FW K129 increases 0.3342→0.3804 GiB and weak PPML
0.1243→0.1547 GiB. No general memory reduction is claimed. Full b/V exports are
exact in every pair, samples/status/rank/DoF and discrete solver decisions agree,
and scientific certificates pass. PPML reports zero fallbacks, step halvings and
strict-profile rejections throughout. No fallback-heavy timing claim is made.

## Validation and preserved failures

[Validation](validation.json) records all 94 plugin tests, 17 OLS boundary/static
tests and nine package tests passing. Tests cover ordinary/inline/local request
equivalence, malformed lengths and frames, missing/truncated/non-UTF-8 payloads,
pre-read errors, early memory refusal, every output-store failure, panic
containment and worker-team changes. Universal static audits and actual arm64
ABI/self-tests pass; the x86 slice was built but not executed.

Both artifacts pass the existing independent weighted OLS and one-FE
Poisson/Hessian/sandwich oracles. The new
[long-name fixture](../../../../tests/stata/development/local_request_transport_oracle.do)
has 60 controls at N4096: OLS AW robust against built-in regression, PPML FW robust
and PW clustered against full-dummy Poisson plus separately formed covariance,
and transactional failure checks. The old plugin fails r(1003) on its first long
request; that is expected transport-failure evidence, not a scientific pass. The
final fixture passes on the final candidate, including direct malformed/over-cap
macro reads and preservation of prior b/V/sample and requested-output rollback.

The initial candidate `1c2aedb8...` is superseded. Its [source](initial/identities.json),
patch, original fixture and eight pairs remain under `initial/`; they are not
pooled with the final eight. Review exposed a real boundary error: its FFI bound
included the terminator, and its fake reader did not model SPI truncation. With
documented truncation semantics, a new test accepts a valid request prefix while
discarding undeclared trailing bytes, so the expected-rejection assertion fails.
The final extra-byte read and separate terminator capacity fix this, without
changing the request parser or weakening the test. The failing test log hash is
preserved. Only the final candidate is adopted.

An added licensed malformed-request probe initially used an unavailable plugin
program and failed before executing the intended check. Its exact
[fixture](final/failed-plugin-registration-fixture.do) and receipt are preserved.
Explicit registration fixes the fixture; the complete long-name oracle passes
on rerun. Initial Python 3.9 imports, a wrong discovery directory, and an omitted
repository PYTHONPATH failed before tests; corrected runs pass. No failed run is
counted as passing, and no scientific tolerance changed.

## Wide replay and next boundary

The [exact K257 replay](wide-results.json) preserves the original wider-probe
request: PW, one FE, offset, missing data, two-way clusters and 16 workers. With
the final transport binary, the N30720/seed104729 full request succeeds with an
explicit r(0) sentinel. The original N7680/seed104730 warm-up reaches native code
but fails r(430): its covariance PSD report is inconsistent with the published
matrix. The earlier initial-candidate runs give the same outcomes and remain
archived. The full replay is a functional result, not an independently checked
wide covariance export or a paired performance observation.

This later PSD failure was previously hidden by r(1003). Its validation and all
numerical code remain unchanged in this repair. Investigate it independently,
then resume the original wider-moment protocol against the repaired baseline.
The wider two-tile prototype remains absent and unadopted; no smaller substitute
target or altered warm-up is used. Broader P4/P6 and other #27 items remain open.
No agents, cluster jobs, push or qualification occurred; unrelated Veneto work,
installed dist and historical receipts remain intact.

The [comparison driver](compare.py) uses existing `benchmarks/local_fixest_gap`
modules with `FEREG_LOCAL_BENCH_ROOT` selecting isolated `baseline`/`candidate`
trees and `STATA_MP_PATH` selecting Stata. The oracle driver can be copied to
that root. Numeric ZIPs retain full exports, input recipes/hashes and worker
copies; raw licensed logs, binaries, datasets and machine paths remain outside
Git. The archive manifests bind every numeric ZIP by SHA-256.
