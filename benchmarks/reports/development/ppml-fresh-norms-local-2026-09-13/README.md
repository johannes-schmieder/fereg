# P4 fresh retained PPML within norms: adopted local checkpoint

Adopt [ADR 0053](../../../../docs/adr/0053-ppml-fresh-design-norm-reuse.md).
Fresh fast one-FE PPML construction now takes its stored control norms from
the same successful categorical projection. It uses the existing generated
column mapping to select norms, excluding the response and absorbed intercept.
The caller validates selection length and bounds before projection or norm
publication. No index vector or last-result accessor is introduced.

The construction still compacts the residualized matrix in place. Eligible
calls omit per-value stable norm arithmetic during that copy and the two
K-element norm scratch allocations. Reuse requires one categorical FE and
matching frame/projector weight divisors. Alternative divisors, low memory,
generic/slope and multi-FE routes retain their stable norm calculation.
The existing fused trailing-norm call shares this validated selection helper.
Raw reference norms/means, the useful initial certificate, projection itself,
strict rank policy and independent final checks remain unchanged. The preceding
rejected fresh-input certificate experiment is absent.

## Source and evaluation boundary

Accepted native baseline is `ccc1f3373744318296f55ac66bf9d85a7e7a9d23`;
main before this experiment is evidence-only `0195491f2464256a156874164998cb1d0a76216a`.
[Identities](identities.json) bind 152 native inputs, the exact
[implementation patch](implementation.patch), locked dependencies and isolated
universal plugins. Every baseline input matches that accepted Git revision;
every candidate input matches the adopted source. The candidate plugin is
`ef4f1a61a8f5c240e9a7a6de8b2aef835344ab51206e784d12b68c8480d09a75`.
Its changed crate was visibly compiled for both architectures before measurement.
The installed plugin was neither the comparison baseline nor replaced.

The [prospective protocol](protocol.md) requires at least 2% and 10 ms median
paired command savings on a primary, with evidence in changed projection/
construction work. The first screen authorized two further seeds for all three
targets. All observations remain: **15 pairs / 30 commands**, comprising nine
public primary pairs, five protected controls and one private allocation audit.
The local M2 Ultra measurements use fresh Stata/MP 19 processes, a small untimed
warm-up and rotated arm order. Native workers are distinct from the licensed
Stata processor limit. Affinity is uncontrolled; no build or test ran during
timing. These are development findings, not release or comparator claims.

## Command, phase and memory results

| Public target | N / K / workers | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 9.604 → 9.703 | +0.02% |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 3.304 → 3.204 | −2.4% |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 3.129 → 3.022 | −5.1% |

Median paired changes need not equal ratios of independently computed medians.
FW saves 79/31/118 ms across the three pairs, with projection savings of
36/30/26 ms. Projection medians fall 0.4704→0.4444 s. Unchanged finite work
also varies, so the entire command gain is not attributed to norm reuse.
PW projection medians fall 0.3883→0.3437 s. Its largest pair, 3.579→3.024 s,
also has lower preparation and finite/strict costs; all of that observation is
retained, without claiming that the changed norm work explains the whole gain.

Complete one-FE projection medians fall 1.1419→1.0234 s, but finite/strict
work increases in repeats. Commands change −266/+99/+2 ms. There is **no
repeatable complete unweighted command gain** in this experiment. The FW
result independently satisfies adoption; no primary or protected pair exceeds
the prospective max(10 ms, 5%) command regression envelope. Overlapping and
independently computed phase medians must not be added.

All fifteen native peaks are exactly unchanged and within unchanged models.
Complete/FW/PW peaks remain 2522.61/633.49/645.34 MB. Sampled primary RSS medians
are 1.9593→1.9597, 0.5263→0.5628 and 0.5447→0.5415 GiB, respectively.
This is not a native peak or RSS saving. Removing the small norm scratch does
not lower the dominant whole-command peak.

Protected command changes are weak two FE 0 ms, low FW −13 ms, small one FE
+1 ms, saved/separated +1 ms and explicit strong two-FE MAP 0 ms. These are
single-pair controls. The private audit changes 8.763→8.828 s (+0.7%, +65 ms)
and is excluded from public aggregates. Both arms retain zero allocation
events and zero live/peak growth over 16 audited iterations, stable retained
capacities and passing fresh/warm and allocation-free certificates.

## Scientific validation and retained limitations

All fifteen complete b/V exports are byte-identical. Samples, rank/status,
DoF, weight/cluster/separation structure and discrete solver decisions are
exact. Independent runtime certificates and saved-output comparisons pass.
The [comparisons](comparisons.json), [summary](summary.csv) and
[checksummed numeric archive](archive.json) retain phases, solver/retry fields,
modeled/native/RSS memory and the private audit separately.

[Validation](validation.json) records 29 distinct candidate Rust tests in an
isolated target, both independent full-dummy Poisson/Hessian/sandwich Stata
oracle arms, universal static audits and the actual native arm64 ABI/self-test.
The new selection test covers gapped/reordered/repeated columns, invalid
selection nonmutation, extreme column scales and capacities against independent
hypot accumulation. The fresh session test compares rank/status, bread,
coefficients and residuals to the generic reference under unit/FW/PW weights
and matching/alternative explicit divisors. Existing changed-weight stale-rank,
strict rollback, generation invalidation and iteration-zero tests remain green.

The new cross-projector test initially required absolute FE-certificate
diagnostics to agree to 512 machine epsilon. The unchanged accepted baseline
fails identically: 5.002265680432952e-12 versus 1.9553313000909888e-11.
Only that new test's diagnostic comparison was corrected: its inputs and
2e-12 scientific-product envelope remain, and each projector must certify
independently. The global comparison helper and runtime contracts were not
changed. Raw log hashes preserve both failures and the final baseline pass.
A postprocessing display's nonexistent `seed` key was corrected without
changing measurements or acceptance checks.

Fresh raw reference norms remain duplicated between retained construction and
the projector; the N-by-K compacting copy also remains. Other P4 passes, P0
accounting gaps and the remaining #27 packages are not closed by this change.
Unrelated Veneto work and historical receipts remain intact. No installed-dist
replacement, agents, SCC, push or qualification occurred.

The drivers use the existing `benchmarks/local_fixest_gap` helpers. Reproduction
requires isolated `baseline`/`candidate` trees and `FEREG_LOCAL_BENCH_ROOT`;
the portable oracle driver reads the executable from `STATA_MP_PATH`. Raw
licensed logs, input datasets and binaries are excluded from this archive.
