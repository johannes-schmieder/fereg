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

## Open failures and gaps

- `ols_three_fe_batch_oracle.do` fails with `r(459)` (uncertified default
  diagonal-LSMR route) on production and candidate binaries alike; preserved,
  not repaired.
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

Branch `ccr-7c76506a-21m47l` carries the programme's commits. Each stack was
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
observation, not timing. Mac boundary timing, the licensed Stata full profile and
the PTY Break fixtures are Johannes's to run.
