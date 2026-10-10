# Validation status

Johannes accepted the integrated runtime on 2026-10-07. That is architectural
acceptance, not a performance pass: the final timing campaign closed incomplete
and historical timing was mixed. The Mac is the qualification host. Linux and
Windows evidence comes from bounded build, load and mapped-fixture checks, which
do not establish full platform scientific or performance qualification. GitHub
testing is paused. Native distribution still needs the
[licensing decision](licensing.md).

This page keeps accepted evidence, open failures and standing caveats. The full
narrative ledger, with every run identifier, archive and receipt digest,
campaign parameter and failed attempt, is in Git at commit `140dda27c42a`:
`git show 140dda27c42a:docs/VALIDATION.md`. Ignored raw captures it mentions
were deleted; hashes and summaries are the remaining record.

## Accepted evidence

Artifact digests are SHA-256 prefixes; the ledger gives full values.

| Date | Scope | Host | Artifact | Result |
| --- | --- | --- | --- | --- |
| 10-07 | Integrated runtime: 375 families, 98 holdouts, 6,375 native commands at 1–16 workers | Mac | `20a9d3d9` | No scientific failures, exact repeats, stable decisions |
| 10-07 | Single-runtime cleanup: gate, 64 licensed fixtures, 73 replay cases, staged installs, Break | Mac | `14dd962c` | Pass; one parity fixture only after its repair |
| 10-07 | Windows build, PE audit, loader, Stata smoke (`a8cdd888`) | Windows | — | Pass; the first run failed |
| 10-08 | Handover integration `e3380ede`: 1,647 tests, 73 exact replay cases, mapped fixtures | Mac | `66e35883` | Pass |
| 10-08 | Merged FMA group means: Cachegrind, SCC 7968913 | Linux | `07e5e590` | Exact; group-function instructions −26% and −54% |
| 10-08 | Guarded Linux loading (SCC 7969139) and Stata 19 load-time A/B | Linux | `45c54141` | Pass; bit-identical captures |
| 10-08 | Windows rounding and loading, clean `e3380ede` | Windows | — | Pass |
| 10-08 | Wide-Gram dispatch and `within` CSR pin `297f32d9`, quiet timing on 22 workloads | Mac | `5eb93024` | Exact; largest upper bound 1.044 |
| 10-09 | Windows controller repair rerun, `81c51e7e` | Windows | `08995120` | Pass |
| 10-09 | Cleanup phase 1 (`f5eace5c`–`c4d91623`) against baseline `9debf9bf` | Mac | `49ab9e29` | Exact replay; timing within allowance |
| 10-09 | Phases 2–3 (`09fc5e4c`–`261536ac`); ulp scope `296cbc23` rebaselined replay | Mac | `c0f9aff9` | Exact replay; timing within allowance |
| 10-09 | Phase 4 shipped fault plans (`5dcaaf81`–`e97d355e`) | Mac | `fdd48d56` | Exact replay; timing within allowance |
| 10-09 | Integration checkpoint `8fb67bb5`: Mac gate, Stata full (68), Break, Linux build and stores, Windows driver | All three | `c4363362`, `91933073`, `961afc7c` | Pass, except full staging (see below) |
| 10-09 | Saved-output fixture `9c08204b` on Windows | Windows | `ac92ad81` | Pass; Windows batched stores verified |
| 10-09 | Measurement-driven work (`02936956`–`7dd56c01`); one-pass group-mean ulp scope; baseline `d4b68d26` | Mac | `4c1a432f` | Scoped replay; quiet timing pass |
| 10-09 | Windows FMA kernels and module pin `d5eb37a7`, `04a0e255` | Windows | `c3783aec` | Pass after one intermittent loader crash, fixed by the pin |

Timing rows use paired-median bootstrap upper bounds against max(5%, 1 ms) on
the frozen workloads. They are bounded native-call observations on one host,
not release or end-to-end Stata claims.

## Failures, resolutions and gaps

- Resolved on Mac 10-10: `ols_three_fe_batch_oracle.do` had failed with
  `r(459)` on every binary: explicit
  LSMR stopped its recurrence at `tolerance()` and certified per-level FE
  moments at that same tolerance, leaving one weighted column at 1.0167e-8
  against 1e-8. The same gap remained at the 1e-14 final residual projection
  used for saved outputs. In a 128-configuration sweep at 100,000 rows, 2 of 64
  failed on the base plugin. Johannes's decisions A and A' (2026-10-10, commits
  `5413cfe` and `a551feb`) make explicit OLS LSMR iterate to a hundredth of the
  tolerance, floored at 1e-15 and never looser than it, and certify at the
  tolerance. With that, the native witnesses
  `explicit_lsmr_three_fe_batch_matches_explicit_dummy_oracle` and
  `explicit_lsmr_final_residual_projection_keeps_a_margin` pass, and the sweep
  has no failures. Mean LSMR iterations change from 16.7 to 20.7 in the main
  projection and from 16.1 to 13.7 at the residual step on strong graphs.
  Instructions on one 100,000-row explicit-LSMR case rise 3.7%. Linux replay
  against `e237c217` passes under the declared scope
  `tests/replay/scopes/2026-10-10-explicit-lsmr-direct-tolerance.json`: only
  ols-10 and ols-13 change (coefficients at most 1.4e-15 relative, covariance
  at most 2.1e-10 relative), and 71 cases stay exact with 2- and 4-worker
  checks. The licensed fixture and Mac boundary timing now pass; see the
  qualification record below.
- Full release staging failed at `8fb67bb5` because its input held only the
  Mac plugin; only the Mac review archive passed.
- One passing Windows run after the module pin bounds, but cannot exclude, a
  residual loader race; the mapped-after-release assertion is deterministic.
- The LSMR helper's unpreconditioned retry after a positive-definiteness
  breakdown keeps the full allowance and reports only its own iterations.
- Default solver routing is unchanged pending a decision: automatic PCG
  exhausted a reduced budget on weak and chain graphs where LSMR certified.
- Deferred review items: PPML-10 (result-changing), FE-11, OLS-4, FE-16 and
  the PLUG-12 runtime missing value (next wire bump).
- Not adopted: PGO (three-FE regression), the experimental all-FMA build and
  the automatic PCG continuation (failed an 80-level chain at `iterate(128)`).

## Standing caveats

- Mac-only qualification does not establish Linux, SCC, Windows or Intel-host
  Stata evidence; Rosetta exposes no FMA/AVX hardware path.
- FFI allocation tests observe the whole process and run serially; a parallel
  invocation fails by design and is not a runtime defect.
- Retained native allocation tests do not claim exhaustive detection of every
  new allocation.

## 2026-10-10 improvement programme

PR #31 was fast-forwarded from `140dda27` to `06ed5f94` on 10-10. Each stack was
checked on Linux with `./scripts/run_tests.sh` and the 73-case replay against a
`140dda27` baseline plugin (`e237c217`), with zero exact differences; commit
messages record each result. Behaviour changes are confined to admission and
failure paths: the separation report bank's memory charge (plan D9; admission
only loosens), Break reaching saved-FE recovery and long serial row passes, the
PPML validator keeping `r(1)`, PPML rejecting string numeric inputs at layout,
test fault plans compiled out of shipped builds (plan D4, reversing phase 4's
choice), and the OLS multiway PSD certificate floor becoming `f64::MIN_POSITIVE`
(plan D1; it can only reject, and every replay case was unchanged). At
`2c34533`, Linux Cachegrind on two 100,000-row single-worker cases (OLS two-FE
robust; PPML two-FE clustered) compared the base plugin `e237c217` with
`3a4fdc32`: exact outputs, total instructions −0.0027% (OLS) and −0.015% (PPML),
against repeat spreads of at most 5,415 instructions. This is an instruction-count
observation, not timing. The explicit-LSMR decisions (`5413cfe`, `a551feb`) are
the programme's only deliberate result change. After them the Linux replay
anchor moved from `e237c217` (`140dda27`) to `ba350134` (`71c289a`), which
reproduces the declared scope against the old anchor and replays exactly
against itself. The Mac qualification and replay re-anchor are recorded below.

Mac follow-up found two qualification defects at `5d548274`. Its replay had
71 exact cases and only `ols-10`/`ols-13` differences, but both input hashes
differed from the Linux scope; they match the existing Mac corpus. Separate
Mac scope `2026-10-10-explicit-lsmr-direct-tolerance-macos.json` preserves the
Linux scope unchanged and covers two additional one-ulp fields: adjusted R²
and the rank condition proxy. Input-derived 70-digit Decimal witnesses now
check both fields at rtol 1e-12/atol 1e-14. Scoped replay passes all 73 cases
with 2/4-worker checks against a fresh `140dda27` Mac plugin (`4842d6f3`).

The bounded separation report bank also retained an unintended logical ceiling:
600 rows, 257 zero outcomes with separate indicator regressors, then positive
outcomes, with `separation(simplex) keepsingletons`, succeeded on the base but
returned `r(3499)` at the PR tip. Separating initial reservation from the full
logical ceiling and reserving exact-FE follow-up records fixes it. The native
257-generation witness and public sample/intercept/robust-variance checks at
1/4 workers pass; all 73 replay cases remain exact against the original PR
plugin (`04157bcc`). Both that public fixture and
`ols_three_fe_batch_oracle.do` pass on Mac Stata/MP 19. These focused results
use candidate plugin `2d2dd904`; final integration results follow.

| Final qualification (10-10) | Artifact / receipt SHA-256 prefix | Result |
| --- | --- | --- |
| Mac build and replay, merged `06ed5f94` | plugin `336015be`; replay `c8e08323` | Universal audit and ABI/load pass; 73 scoped cases and 146 worker checks pass. Re-anchored from `4842d6f3`; only ols-10/13 differ. |
| Mac timing against `140dda27` | timed plugin `8c1b779e`; summary `18e5f922`; host observations `d67117e9` | All 18 workloads exact and within max(5%, 1 ms), 15 alternating pairs each. Largest bootstrap upper bound 1.04017 (K=129, four workers); paired medians 0.97417–1.00678. Johannes accepted practical quiet; brief background bursts and every pair were retained. Earlier seven-pair upper bound 1.05118 failed; two interrupted confirmations were inconclusive. No release-speed claim. |
| Licensed Mac Stata/MP 19 and PTY Break | full `4ea95203`; hidden `1cab8469`; development `4a8fe8dc`; Break `6e34ae90` / `756572c6` | Full 69/69, ppml-hidden 12/12, development 64/64, including the three-FE oracle; OLS/PPML Break pass. Initial development failures led to fixture repairs in `4f6d0427`; tracing was disabled for allocation assertions. |
| Bounded Windows build/load | plugin `de000832`; receipt `f18dba21` | Native build, ABI/load and licensed smoke pass; machine stopped. |
| Bounded SCC Linux build/load and report-bank regressions | job `8016714`; plugin `eff64e7b`; result `03b58901`; accounting `220ed385` | glibc 2.28 audit, ABI/load, both UBSan variants and both targeted tests pass; scheduler failed=0/exit=0, 216 s, maxvmem 3.886G. Original job `8016626` failed linking system UBSan; explicit GCC 12.2 fixed it and is pinned in campaign setup by `06ed5f94`. |

Full hashes are in the documentation commit. Evidence built at `ac7a7a9` and
`4f6d0427` is reused through `06ed5f94`: all 168 production modules and 58
artifact inputs are identical; `run_tests.sh` passes at the final source.
