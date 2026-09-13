# P3 categorical interaction certificate: prospective evaluation

Accepted baseline 257f4cee74ee0b3e80c55e1f1867b486e26410b5 and isolated plugin
c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da.
Test the independently checked representative-tuple certificate already captured
in probe.json. It replaces only categorical cluster re-factorization during
validation, not construction, canonical codes, singleton/error order, weights,
solver choice, VCE or numerical certificates. One usize per observed group must
fit the replaced factorization scratch. All SPI stays on Stata's main thread.

Run the existing interaction_codes_oracle on both artifacts: independent OLS
FE-dummy/weight/cluster covariance and full-dummy Poisson/Hessian/sandwich checks.
Keep the three focused partition rejection/allocation/order tests and universal
static/native arm64 ABI checks. No installed dist replacement or cluster work.

Screen three fixed primaries at seed104729, rotating arm order:
1. OLS FW, g1#c2 FE and c1#c2 cluster, sparse missing, N245760/K50/16 workers.
2. OLS PW, same syntax, shuffled sparse cluster tuples, N245760/K10/4 workers.
3. PPML FW, same syntax/tuples plus offset/missing, N122880/K10/4 workers.
Sparse tuples use a seeded random permutation of distinct row labels for c1,
retaining its existing missingness; c2 is unchanged. Their Cartesian product
exceeds the dense factorizer admission. This is a fixed generated shape, not a
replacement chosen after inspecting target results. Warm inputs stay N7680.

If any screen saves >=5% AND >=10 ms in the complete command, repeat all three
at seeds8675309 and20260819, with rotated order. Adoption requires a primary's
median paired gain to meet the same threshold and savings attributable to
preparation. Otherwise reject as a standalone optimization; do not search extra
seeds until a target passes. Keep every observation.

Protected seed31415927: unchanged complete IID OLS; ordinary simple-FE PW
interacted clustering; small AW absorbed slope with interacted clustering;
low AW interacted FE/cluster with saved effects/residuals; weak two-FE PW OLS;
dense-interaction FW PPML; saved/separated FW PPML; explicit low-mobility three-FE
PW PPML with interacted clustering. Use explicit actual FE counts/absorbed terms.
Review increases above max(10 ms,5%), preserve native/model/scratch/RSS and all
retries/fallbacks. Two additional public phase profiles use seed67280421 for
sparse OLS and sparse PPML, outside the primary aggregates. No private repeated-
IRLS audit is needed: the changed preparation certificate runs before that state.
If screens fail, retain focused affected/unaffected controls sufficient to
support rejection; no automatic repetition campaign.

Planned successful screen/adoption branch: 9 primaries +8 controls +2 public
phase profiles =19 pairs. Require exact scientific CSVs, samples, cluster and
solver decisions. Every modeled memory boundary remains authoritative. Results
are local development evidence; no external-comparator or qualification claim.
