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

## Pending qualification

### Performance-plan B: ordered FMA coverage, 2026-10-11

This is an arithmetic-preserving candidate, **not an accepted B workstream**.
It extends the existing detected FMA/AVX table from five to twenty entries.
The six areas named in the plan now dispatch ordered dot/projection, joint
iteration, two-FE PCG and canonical upper-Gram loops. Core bodies remain safe
and shared with the portable path; the plugin owns target-feature wrappers.
Per-element arithmetic, reduction order, padding and inactive-RHS behavior are
preserved. The intentionally non-fused reduced-matrix product is unchanged.
There are no tolerance, sample, rank, omission, separation, route, SPI ownership,
certificate or strict-PPML-solve changes. Some fallback/reconstruction,
reusable-certificate and smaller canonical-product loops remain scalar;
profiling must determine whether more coverage is needed.

An independent high-effort Astra review found no correctness issue. Its request
for explicit padding/inactive sentinels and the active `-1.0` sentinel was
implemented. The complexity ratchet records +231 core and +252 plugin production
lines for the shared bodies and typed wrappers, with no new oversized function
or file. The existing joint iteration gains eleven lines from explicit typed
call arguments. Dispatch-call cost is included in the Mac measurements below.

Mac checks pass: `scripts/run_tests.sh`, universal native build/audit/load,
73/73 exact replay cases against the previous plugin (zero differences and
unchanged solver signatures), all 67 licensed full-profile fixtures, and both
OLS and public-scope PPML PTY Break checks. The replay's independent scientific
witnesses pass in all 73 cases. These are working-tree artifact checks bound by
the source-patch and plugin hashes below; the plugin's embedded source label
predates the runtime patch.

The x86 Mac slice compiles all twenty entries. Static disassembly of each of
the fifteen added target-feature bodies contains no library-FMA call; fourteen
contain hardware FMA instructions and PCG direction uses the equivalent
multiply/add form for its unit multiplier. This is instruction-selection
evidence, not an executed instruction count. Rosetta exposes neither FMA nor
AVX here: all five hardware tests skip, despite their test-runner `ok` labels.
No x86 execution or Linux qualification is claimed. SCC SSH authentication
fails with `Permission denied (keyboard-interactive,hostbased)`, so frozen-input
Linux A/B replay, hardware tests and cachegrind remain required. In particular,
the requested disappearance of the `__fma_fma3` execution share is unverified.
B stays unqualified and subsequent workstreams have not begun. A1 also awaits
Johannes's section-4 decision; SPEC §13 is unchanged.

The quiet Mac run used the twenty hash-verified frozen workloads, one warmup
and seven alternating pairs per cell, a 90-second process limit and an 1800-second
budget. It completed in 643.59 seconds, with all 320 fits exact and all twenty
paired-median 95% bootstrap upper bounds within max(5%, 1 ms). No builds or
agents ran during timing; ordinary desktop background processes were present.
The largest upper bound is 1.035739. Paired medians range from 0.966264 to
1.009575; costs as well as gains are retained below. These small observations
do not establish a speedup or a release-performance claim. Times are seconds;
ratios are medians of paired ratios, not ratios of the displayed medians.

| Frozen workload | Baseline median | Candidate median | Paired ratio | 95% bootstrap interval |
| --- | ---: | ---: | ---: | --- |
| `ols-no-fe-k5-t1` | 0.005282 | 0.005280 | 0.999621 | [0.991737, 1.005359] |
| `ols-no-fe-k5-t4` | 0.004686 | 0.004704 | 1.001930 | [0.999575, 1.007042] |
| `ols-three-unit-k5-t1` | 0.585763 | 0.585808 | 1.000077 | [0.996792, 1.001798] |
| `ols-three-unit-k5-t4` | 0.299557 | 0.299698 | 1.000087 | [0.997239, 1.001967] |
| `ols-two-pw-k5-t1` | 0.653873 | 0.656214 | 1.000259 | [0.997755, 1.011564] |
| `ols-two-pw-k5-t4` | 0.372277 | 0.372096 | 1.001750 | [0.989284, 1.005846] |
| `ols-two-unit-k20-t1` | 1.203934 | 1.205990 | 1.005488 | [0.994109, 1.010733] |
| `ols-two-unit-k20-t16` | 0.445681 | 0.442693 | 0.999367 | [0.987844, 1.008383] |
| `ols-two-unit-k20-t4` | 0.574316 | 0.567609 | 0.990337 | [0.982508, 0.999854] |
| `ols-two-unit-k20-t8` | 0.471380 | 0.474140 | 0.997908 | [0.978926, 1.034033] |
| `ols-two-unit-k5-t1` | 0.478811 | 0.480858 | 1.001146 | [0.987009, 1.004367] |
| `ols-two-unit-k5-t4` | 0.263885 | 0.263871 | 0.995747 | [0.994644, 1.005684] |
| `ols-wide-k129-t1` | 0.763857 | 0.752693 | 0.974630 | [0.970251, 1.005925] |
| `ols-wide-k129-t4` | 0.398176 | 0.401504 | 0.990082 | [0.965212, 1.035739] |
| `ols-wide-k64-t1` | 0.295673 | 0.295657 | 0.993140 | [0.988308, 1.023617] |
| `ols-wide-k64-t4` | 0.168455 | 0.163700 | 0.966264 | [0.959143, 1.002210] |
| `ols-wide-k65-t1` | 0.312483 | 0.311014 | 0.996615 | [0.978531, 1.011322] |
| `ols-wide-k65-t4` | 0.172959 | 0.176653 | 1.009575 | [0.957675, 1.029921] |
| `ppml-two-unit-k5-t1` | 0.405354 | 0.404991 | 1.000059 | [0.993464, 1.004761] |
| `ppml-two-unit-k5-t4` | 0.229143 | 0.228294 | 1.001377 | [0.991459, 1.003422] |

A separate instrumented Mac diagnostic used locally generated campaign-shaped
AKM inputs: one million rows, 16 controls, 200,000 worker and 10,000 firm
levels, seed 739391, one worker, OLS IID and PPML robust inference, unrestricted
candidate, nondeterministic worker policy, and only the required sample output.
These inputs are not the frozen SCC campaign inputs and cannot substitute for
them. Two alternating pairs per model, with a 90-second fit limit and 600-second
budget, give eight successful fits with exact science and solver signatures.
PPML has four outer iterations in every fit and keeps its fresh strict solve.
The following means are descriptive; no timing acceptance is inferred from
two pairs with tracing enabled. Unobserved phases stay missing.

| Diagnostic runtime field | Baseline mean | Candidate mean |
| --- | ---: | ---: |
| OLS harness native call | 0.884388 | 0.885726 |
| OLS preflight (row 18) | 0.000202 | 0.000200 |
| OLS input (row 19) | 0.133730 | 0.134039 |
| OLS native (row 20) | 0.748780 | 0.749800 |
| OLS total (row 21) | 0.882712 | 0.884039 |
| PPML harness native call | 4.773539 | 4.703071 |
| PPML total (row 1) | 4.771736 | 4.701227 |
| PPML preparation (row 28) | 0.289071 | 0.288944 |
| PPML separation FE (row 29) | 0.004087 | 0.004130 |
| PPML separation simplex (row 30) | missing | missing |
| PPML separation IR (row 31) | missing | missing |
| PPML separation mu (row 32) | missing | missing |
| PPML weight/response refresh (row 33) | 0.060927 | 0.061665 |
| PPML FE projection (row 34) | 1.963144 | 1.927000 |
| PPML finite solve (row 35) | 0.560771 | 0.560114 |
| PPML objective/line search (row 36) | 0.279796 | 0.280245 |
| PPML strict final solve (row 37) | 1.165864 | 1.133371 |
| PPML inference (row 38) | 0.085691 | 0.083245 |
| PPML FE recovery (row 39) | 0.000001 | 0.000001 |
| PPML publication (row 40) | 0.121995 | 0.122015 |

Failed attempts: initial Clippy checks rejected complex function-pointer types
and a macro placed after tests; named types and placement fixed both. The plan's
literal `output_sample=0` recipe failed before estimation because both wire
parsers require a positive sample output. The corrected case keeps sample
column 21 and disables only optional outputs; the plan recipe is corrected.
A generator retry refused to overwrite its existing template; reusing that
unchanged template resolved setup. These failures do not count as successful
scientific or timing observations. No estimator repair or tolerance change was
made in response.

SHA-256 records (raw task captures are disposable; these summaries and hashes
are the retained record). The source patch is relative to `38fd8296` and includes
the new ordered-kernel file and complexity inventory. Its production code is
identical to the timed build; only the later sentinel test assertions differ.

| Artifact / observation | SHA-256 |
| --- | --- |
| Previous universal plugin | `f59d2a2989dc672045fdda86dd88fa2a4191ab0e15a42d833f7bd0711c4d8ca4` |
| Candidate universal plugin | `b04e08a2a085e249f8f72eb6df8c6979e431a7062d210864c48e5e79c205da75` |
| Runtime source patch | `15f29992707e954323d93e8ea86b9e9c4f63357b4114dfb382fafa38470b41e2` |
| Full gate | `439bafa7ff5bba99524e15021d19ed2bdcc7320f2e6dece573719f9c81a3dbf5` |
| Exact replay summary | `519c063ef73872ae8b27d9a8a1d639e900ccf12aca0a42efa8e42c01797c1760` |
| Licensed fixture summary | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Mac timing summary | `43286b38795729eff6a9005a4d0e6c3c5a11e411a95eb5301928f0640e131bad` |
| Local phase input manifest | `42e783208ab7e61d84bacf38afa0d3e3d0ad734b27be9dc41f80a51767497322` |
| Rejected zero-sample-output probe | `bea85baec7aa5e5a97d1535d63173a4e745da91cd675af6399a47f39792c207c` |
| Successful phase diagnostic | `c106aa856c2b2f3c6459ceed8ec6cacbb03339379292923940f996ed0d003fb9` |
| Static x86 instruction summary | `19a6e94524b6afb5195ca9da9a482bed30ada4f2f80bbea9ee247baebdcad4d9` |
| x86 tests (hardware skipped) | `a340e4da6363a29dddcbea72ebbb1decb133b3c7239c4e7238a5201ad7d93552` |
| Candidate build/audit/load log | `b14b52bccc47b8986c2e2deeb05c48c34e3c653b758332b5e9c8779e199bc96e` |
| Local OLS AKM case | `6503640e23c0955ba2b711b02f82917085503ccd16f2c58011bc64b74f04fd3e` |
| Local PPML AKM case | `029fae2a6399992230044ff0847d11cdac4e05758704c3d6eef17538c8c1404e` |

## Accepted evidence

### Performance-plan tooling, 2026-10-11

G1/G2 are arithmetic-preserving tooling changes with an unchanged production
artifact relative to `cad23f41`: the production-identity check passes for all
168 runtime modules and 58 artifact inputs. The campaign captures OLS runtime
rows 18–21 and all 13 PPML phase rows, and new manifests explicitly retain
separate default/strict OLS arms. Strict OLS uses `fixef.tol=1e-11` and
`ssc(K.exact=TRUE)`, reporting covariance DoF separately from raw `nparams`.
Historical campaign manifests and retained observations are unchanged.

All 42 benchmark tests and `scripts/run_tests.sh` pass on Mac. The R witness
uses fixest 0.14.2 and checks disconnected IID/robust OLS coefficients and
covariance against explicit-dummy least squares and an independent sandwich.
A licensed Stata 19 probe executes the campaign metadata block for both models,
returns normally with `CAMPAIGN_METADATA_PASS`, and records four OLS fields
and PPML total plus 13 phases, including an unobserved phase as missing.
Local documentation links and the complexity ratchet pass. No new estimation,
sample, rank, omission, separation, solver, tolerance or certificate behavior
is introduced, and no performance gain is claimed.

Failed setup attempts remain recorded here: the default shell selected Python
3.9 without the scientific packages, and the first gate lacked Cargo on PATH.
The successful checks used a disposable Python 3.13 environment and the installed
pinned Rust toolchain. A disconnected clustered diagnostic with strict fixest
reported `df.K=13`; its expected nesting-adjusted count was 7. That comparison
is not a passing covariance witness. Strict clustered campaign cells retain
their actual count and must pass the existing comparison gate; no native
convention or comparison tolerance was changed to accommodate fixest.

SHA-256 records: benchmark tests
`35b70ecc6a18a22e7d21fba5d7662fa389f180dd8b34009b8cf4221f3a990d97`;
full gate
`e9e85645d44ffa168e8d039381dad1e7c0765137c8b5dc502958b4b7c9d211fb`;
licensed metadata probe
`c0e664c0b33221fa48372db5fece7f8fa9c4d4e91bd0b62f5e273d49cf92d903`;
OLS metadata
`63600d9f89e51edc7d60779df4a7cebc11ec904b62c0dae1b6e0a74b010429dc`;
PPML metadata
`afd7b1d23cae9af6a224b19c7c2fc0960f0b248e1a9fc333e1e0417bd11e6120`.

A downstream audit caught that the comparison whitelist initially omitted the
new `ols-*-K.exact=TRUE` policy label. The follow-up admits that OLS label and
tests that differing correction DoF still fail. All 42 benchmark tests pass;
log SHA-256
`6deb4315f564ba536984df97986fef3111afad4568dd98360ef4fa487bf09860`.
This follow-up changes no runtime or comparison tolerance.

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

### Packed SCC campaign after PR #31

Run `20261010T155133Z-pr31-5e2951a4` uses merged source `5e2951a4`
(archive SHA-256 `7b168204`) and Linux plugin `a259496b`. Its report and
complete per-configuration comparison are retained under that run in
`/projectnb/welfgr/fereg/runs/`, in `report/` and `comparison/`; compact
accounting is in `collected/`. This is a descriptive comparison with the
reconstructed summaries of `20261007T171717Z-akm-current` (production
`7942263`), not a universal speed or platform claim.

| Evidence | Result |
| --- | --- |
| Setup `8016862`, smoke `8016895`, production `8016905.1–4` | All six accounting records have failed=0/exit=0; sentinels, frozen identities and all 492 planned fit positions verified. glibc 2.28 audit, ABI/load, UBSan guards, 40 harness tests and 11 native smoke oracles pass. Production used 28 slots on 28-core nodes, without the exclusive consumable. |
| Scientific outcomes and unavailable cells | Native repeatability and cross-worker checks pass for 55/56 configurations; four-million-row three-sparse PPML times out as before. Across 164 arm cells: 131 eligible, 25 asterisked, 8 without timing, versus 129/18/17 previously. The capture repair exposes seven remaining reference discrepancies and makes two strict-fixest cells eligible; no formerly eligible cell becomes ineligible. All failed fits and reference smoke failures remain reported. |
| Timing and scaling | Native current/previous median ratio 0.9521 across 55 configurations; four-worker medians 0.9390 OLS / 0.9582 PPML. OLS AKM at 8/16 workers is 20.2%/20.4% slower on the same node, despite nearly unchanged fixest controls; the follow-up below isolates and repairs the cause. Its 16-worker speedup falls from 2.573× to 1.692×. PPML AKM gains only 2.6% in elapsed time from 8 to 16 workers. No explicit-LSMR arms were scheduled. These separate campaigns do not replace the Mac qualification timing. |
| Memory and report | Native peak sampled RSS 5.032 GiB, essentially unchanged; median 1.120 versus 1.174 GiB across 51 resolved cells. Unresolved memory stays NR. All 19 PDF pages rendered and visually inspected. PDF SHA-256 `ba133153`, configuration summary `f0ab36f9`, full comparison CSV `6c8b9cc9`, retained-file manifest `a4bb00bc`; full hashes are in the documentation commit. |

### OLS high-worker slowdown investigation

Three bounded SCC experiments on `scc-zj4` used nine alternating or rotating
pairs per arm at 1/4/8/16 workers, with 28 slots on a 28-core node and no
exclusive consumable. The frozen AKM input has one million rows, 16 regressors,
200,000 worker and 10,000 firm levels, iid inference and seed 739391 (input
SHA-256 `5fd1a5cf`). Full-size warmups precede timing; diagnostic traces follow
it. Inputs and the timed Stata command are unchanged from the campaigns.
Reproducers, observations and accounting are retained under
`/projectnb/welfgr/fereg/dev/20261010-ols-slowdown/`.

Commit `b21244aa` replaced private scalar norm accumulators with repeated writes
to adjacent shared scratch slices. Keeping its contiguous column partition but
privatizing the recurrence removes the regression. The selected repair uses
bounded 64-column stack tiles (1 KiB per active worker), then publishes scratch
once. It preserves each column's observation order and arithmetic and adds no
heap allocation. The intervention supports a shared-scratch implementation
cause, consistent with false sharing; no hardware cache-counter measurement
establishes that mechanism exclusively. Restoring the older per-column loop was
also measured, but retaining blocks performed better at four/eight workers.

| Evidence | Result |
| --- | --- |
| Endpoints, job `8017329` | PR31/October-7 paired ratios at 8/16 workers: 1.0942 / 1.1274; 95% bootstrap intervals [1.0772, 1.1890] / [1.0883, 1.2688]. Norm setup and certificate time increase; operator time and 14 solver iterations remain stable. This reproduces the campaign slowdown with smaller magnitudes under paired measurement. |
| Controlled intervention, job `8017369` | Private-block/control ratios at 1/4/8/16 workers: 1.0278 / 0.8076 / 0.7322 / 0.7981; upper bounds 1.0367 / 0.8358 / 0.7949 / 0.8524. The observed single-worker cost is retained. Exact science and solver signatures across variants; scalar norm/scratch witnesses through 257 columns and 1/2/4/8/16 workers pass. Linux glibc 2.28 audits, ABI/load and both UBSan guard variants pass. |
| Direct repaired/October-7 confirmation, job `8017720` | Ratios 0.8269 / 0.7950 / 0.8649 / 0.9210 at 1/4/8/16 workers; all upper bounds below 1.0. Coefficients, covariance and model metadata are exact. Historical residual/certificate diagnostics differ already before the repair; current-to-repaired diagnostics are exact. Peak sampled RSS remains about 650 MiB. All three jobs have failed=0/exit=0 (181/697/172 s). |
| Mac science and transactions, plugin `c5190816` | `run_tests.sh`, universal audit/load, 73 exact replay cases and 292 checks at 2/4/8/16 workers pass. Working-tree licensed profiles: full 69/69 and ppml-hidden 12/12, including OLS/PPML PTY Break. Source patch and plugin hashes bind these profiles; their source label is the parent commit. No fresh Windows check is claimed. |
| Mac timing and future coverage | All 20 frozen workloads retain exact science/signatures. Seven-pair run passes 19: PPML/four-worker upper 1.06765 fails during observed backup/sync activity. One predeclared 15-pair confirmation passes (upper 1.00211); pooling all 22 pairs also passes (1.00538). Largest upper among the other 19 is 1.03420. Practical-quiet observations and every failed pair remain retained; no release-speed claim. The new eight/sixteen-worker K=20 guards pass, and the verifier now rejects unlisted/missing/modified workloads. |

The repair's Linux plugin is `e5eed20e`, built from `5e2951a4` plus the measured
private-block patch; its production tokens equal the integrated repair. Full
artifact, source-patch and result hashes are in the repair commit. The old 18
frozen workload hashes are unchanged. These are bounded workload observations,
not a universal speed or platform claim.

### Packed SCC campaign after the OLS repair

Run `20261010T192805Z-ols-repair-e29164e6` compares source `e29164e6`
(archive SHA-256 `3129d28a`, Linux plugin `57cc9789`) with the actual preceding
PR31 campaign `20261010T155133Z-pr31-5e2951a4`. Compact results, all 164
per-configuration arm comparisons and the 19-page visually inspected PDF are
retained under the new run in `/projectnb/welfgr/fereg/runs/`, in `collected/`,
`comparison/` and `report/`. Full hashes are in this documentation commit.

| Evidence | Result |
| --- | --- |
| Setup `8018263`, smoke `8018275`, production `8018282.1–4` | All six scheduler records have failed=0/exit=0; sentinels, frozen identities and 492 fit positions verified. glibc 2.28 audit, ABI/load, both UBSan variants, 41 harness tests and 11 native smoke oracles pass. Production used 28 slots on 28-core nodes without exclusive. |
| Scientific and failure outcomes | Native repeatability and cross-worker checks pass for 55/56 configurations; the same four-million-row three-sparse PPML case times out. All 164 arm cells retain eligibility and failure reasons: 131 eligible, 25 asterisked, 8 NR. The 15 reference smoke capture failures and 18 oracle failures are unchanged. |
| OLS scaling on identical inputs and `scc-zj4` | AKM one-million-row times at 1/4/8/16 workers are 2.2325/1.2916/1.0426/0.9551 s; current/previous ratios 1.0342/0.7961/0.7134/0.7486. This confirms the controlled repair at 8/16 workers while retaining the 3.4% single-worker cost; fixest controls remain within 1%. Sixteen-worker speedup improves from 1.692× to 2.337×, but 8→16 still saves only 8.4%. |
| Breadth, PPML and memory | Four-worker median ratios are 0.8857 OLS / 1.0124 PPML. Native ratios across 55 eligible cells range 0.7134–1.0870; increases remain visible. PPML AKM robust 1/4/8/16 times are 12.8068/6.4174/5.3236/4.9661 s on a different node, with only 6.7% improvement from 8→16. Peak sampled native RSS is 5.03249 GiB versus 5.03227; common 51-cell median RSS ratio 0.99999. Unresolved memory remains NR. |
| Cross-campaign input audit `8018721` | All 150 successful records in 50 configurations with identical numeric inputs have exact captured estimator science, including all OLS configurations. Five PPML inputs generated on different nodes differ only in response values by at most two ulps (verified hashes; failed=0/exit=0). Their 15 fits differ only in coefficient/covariance rounding, within unchanged SPEC tolerances; discrete metadata and reported objectives remain exact across all 165 successful fits. The three existing unavailable records remain unavailable. |

SHA-256 prefixes: PDF `5dc95256`, configuration summary `4a3440b3`, full
comparison `c4c91585`, retained-file manifest `5603b923`. The complete comparison
flags input and host changes.
Separate campaigns are descriptive: they neither replace Mac qualification nor
establish universal speed or platform claims. No explicit-LSMR arms were run.

### General scaling investigation against fixest

The [detailed investigation](fixest_benchmarks.md#generalization-follow-up)
uses unchanged runtime `e29164e6`, Linux plugin `57cc9789` and pinned fixest
0.14.2. Reproducers, frozen synthetic inputs, captures and CPU samples remain
under `/projectnb/welfgr/fereg/dev/20261011-fixest-general-scaling/`.
Compact results and full hashes are in the linked report's retained evidence.

| Evidence | Result |
| --- | --- |
| Phase/solver panel `8022196`, holdout `8022206`, CPU sample panel `8022215` | All 70 planned fits complete; all three scheduler records failed=0/exit=0, 284/519/90 s. The holdout extends to 16m rows, 4/64 controls, skewed/weak networks and 28 workers. Native captured science agrees across workers; all four original fixed-worker repeats are exact. Four default weak-network reference comparisons remain ineligible; they are not counted as speed wins. |
| Generalization and memory | Fixed-cardinality, 16m-row OLS takes 20.441/11.758 s at 4/28 workers versus fixest 7.427/6.829; PPML 63.258/54.484 versus 31.992/25.888. Peak sampled native RSS is 6.422/10.614 GiB for OLS/PPML. These are single instrumented fits, not qualification timings. |
| Root causes | Dense two-FE PPML setup remains about 8.9 s at 4/16 workers. Its serial neighbour-pair assembly grows 8× for 4× rows; annotation attributes 96.33% of builder-local sampled cycle weight to that loop. Dense three-FE OLS MAP controls take 22–24% less time than auto in the sampled panel, while retaining scientific agreement. Runtime and solver defaults are unchanged. |
| Independent weak-network audit `8022296` | Direct sparse OLS agrees with native slopes within 1.11e-13, RSS within 7.22e-9 and exact graph rank/DoF. Tight fixest convergence plus exact FE counting resolves the OLS discrepancy; two strict PPML arms time out at 120 s and remain unavailable. Audit failed=0/exit=0, 318 s, 1.947G; original launcher `8022280` exited 1 before science and was repaired for four-slot affinity. |

The [compact follow-up evidence](../bench/results/scc-20261011-scaling-diagnostics.json)
has SHA-256 `3d864721`; its full hash and retained-file manifest hash are in
the documentation commit. The stale PPML help description of auto selecting
LSMR was corrected to match the existing MAP-owned policy. `run_tests.sh`,
the 16 focused help checks, unchanged-production identity and local link checks
pass; the Mac run reports the expected Linux mock-plugin audit skip. No new
Mac timing, Windows qualification or release-performance claim is made.
