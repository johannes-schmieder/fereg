# Small Gram solver specialization: rejected local experiment

Reject `sgk1` and restore the complete `dtk1` computational source. Fixed
dimensions reduce narrow projection time in diagnostic traces, but the
whole-command screen does not establish a consistent serial robust gain.
No specialization or new tests from this experiment remain in production.

## Experiment and decision

The previous [statistical profile](../fixed-dense-triangle-local-2026-09-22/README.md)
contains 177 generic matrix-solver samples within 697 serial AW projection
samples (25.4%) and 261 within 853 PW samples (30.6%). The solver and slope
projection source hashes are identical between that sampled `scb1` artifact
and the current `dtk1` baseline. These are historical statistical shares,
not new baseline phase times. The source-bound attribution script and receipt
are preserved in this archive.

`sgk1` dispatches dimensions 1..4 to const-generic copies of the existing
Gaussian elimination loop. Other dimensions retain the dynamic loop. Buffer
and tolerance validation still precede mutation. Scale, rank threshold,
pivot/tie choice, swaps, FMA sequence and back substitution are unchanged;
there are no additional allocations or solver-selection changes. The shared
helper also serves generic slope projections, so focused protected-path
fixtures precede performance measurements.

The sixteen-cell screen runs three alternating pairs at N=245,760 on the local
Apple M2 Ultra, Stata MP 19, T=1/8/16. Every root reports no detected overlap.
Builds, tests, diagnostic profiling and archiving are separate from these
headline fits. Selected paired candidate/prior medians are:

| Case | Paired ratio |
|---|---:|
| AW K10 T1 robust, full outputs | 1.0000 |
| AW K10 T1 robust, no outputs | 1.0335 |
| PW K6 T1 robust, full outputs | 0.9953 |
| PW K6 T1 robust, saved effects | 1.0106 |
| PW K6 T1 two-way clustered, full outputs | 0.9297 |
| Unit K10 T1 robust, full outputs | 1.0147 |
| Unit K10 T1 iid | 0.9580 |
| AW K50 T1 robust, full outputs | 1.0224 |

Threaded results range from modest gains to small losses. The complete table
is in [timings.md](timings.md). Ratios are medians of per-repeat ratios, not
ratios of marginal times. This is mixed screening evidence, not proof of a
regression in each unfavorable cell. No weight/VCE-specific guard is fitted,
and no five-pair confirmation is run for this rejected candidate. Ordinary
fixest is excluded from FW inference comparisons.

Separate single diagnostic traces show projection/reconstruction changing
from 10.743 to 9.380 ms for AW K10 and 13.344 to 11.547 ms for PW K6. K50
changes from 30.842 to 29.408 ms. This saves about 1.4–1.8 ms in the narrow
phase, too little to establish a useful whole-command benefit in this screen.
The larger geometry, certification and remaining data passes stay open.

## Science and evidence

All 141 screen calls pass. All 48 native pairs have identical six numerical
exports, samples, requested saved arrays and 28 non-timing solver fields,
with equal tracked allocation peaks. Nine separate diagnostic calls preserve
exact exports and saved arrays across prior diagnostic, candidate and candidate
diagnostic artifacts. There are 150 successful application records in total.

Ten focused release Rust fixtures pass in six invocations. A frozen copy of
the prior solver compares exact ranks, mutated matrices and solutions across
5,400 dimension/RHS/matrix/scale/tolerance combinations, including singular
and skipped pivots, ties, extreme scales and nonfinite values. Separate tests
check validation before mutation, known solutions, independent slope
certificates, cache eligibility, generic weighted projection and protected
PPML slope behavior. Both new plugins pass actual ABI/self-test harnesses.
These are focused local development checks, not formal qualification.

| Artifact | Plugin SHA-256 |
|---|---|
| Prior dtk1 | `77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151` |
| Rejected sgk1 | `79ea6ec702f2d6717bab8988a5e072ecef6cce881b2fb0abb8903ae48896ddcd` |
| Prior diagnostic dtk1d | `12161eee6e6cc37c8920183f63fe2be34812e590f26cbb5652b27a1f0401b769` |
| Rejected diagnostic sgk1d | `0918f4704d099a556e79a7ac2ab35394c4ba59a1394b189f599c3d30f51546aa` |

The rejected candidate is based on `e6ce7808`, with the six separately pending
graph/window drafts and the two solver/test files. Its complete captured tree
has 524 files; the restored `dtk1` tree has 523. `sgk1-rejection1.json` records
exact restoration of every captured baseline source hash. The six pre-existing
drafts remain untouched.

`evidence.zip` retains portable application records, numerical exports,
worker sources and hashes of local-only output arrays. `sources.zip` preserves
the rejected implementation, tests, four complete-source overlays and exact
controllers/auditors. `checks.zip` contains source-bound builds/tests, audits
and filtered diagnostic traces. The sampling attribution links by hash to the
earlier committed statistical archive. Raw licensed logs and binaries are
excluded. `verify.py` verifies 3,624 stored entries, 132 local-only arrays and
four reconstructed source trees; `privacy-check.json` records the privacy scan.

Serial losses, pending graph/window adoption, Linux access and final integration
remain unresolved. Earlier accepted results retain their original source and
artifact identities. No routing rule, installed package or remote state changes.
