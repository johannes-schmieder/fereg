# Three-FE threaded windows: reject the isolated scheduling change

The current-source screen confirms remaining dense three-FE OLS losses to
stock fixest. A bounded experiment limiting medium batches to one active RHS
per worker reduces memory, but its dense gains are small or mixed. Reject it
under the predeclared stop rule and retain the existing production transport
and admission policy. Do not search more window sizes or select a guard from
the favorable rows.

## Source and hypothesis

Production `batch1` matches all 527 captured inputs at `37af7d82`; plugin
SHA-256 is `a191585c5a156809f8a572bbce37d948b2ede1e0ca8579a85f15e4cb32547d14`.
The isolated `threadwindow1` plugin is
`1941e628502e2dda4cca8c63375cb58ab6f78d248de24bbffa8d57fac649743e`.
Its only native-source difference is the archived five-line addition to
`memory.rs`: threaded batches of 16–64 actual RHSs use a window equal to the
installed worker count when smaller than the full batch. The same private
window function drives transport and the diagnostic memory model. No working
native source, installed plugin, dependency pin or scientific policy changed.

The pinned `within` source solves RHSs through an outer parallel iterator while
gather/scatter operations also use parallel work inside each recurrence. The
earlier source-bound transport profile located the dominant cost inside that
solver. This experiment tests whether bounding concurrent recurrence storage
improves locality and memory traffic. It reuses the existing prepared solver
and ordered window assembly, including all-or-error behavior. No upstream
source was patched and no new preconditioner or numerical routing was tried.

## Measurements

Three rotated repetitions per cell with seed 202609221. Data generation,
builds, focused tests and oracle fixtures complete before timing. The 18-call
baseline and 27-call paired screen run without compilation, profiling,
archiving, or competing fits. All 45 attempts finish with passing application
sentinels, no timeout, no detected overlap and no monitor error.

Timers cover the complete public OLS command, native loading, IID inference
and publication, with matching threads, numeric regressors and no saved
outputs. External import and evidence export are excluded for both estimators.
Stock fixest 0.14.2 is the same pinned, uninstrumented reference artifact used
in the preceding reports. Every attempted cell and its scientific discrepancies
remain in the archive.

| Three-FE case, N / K / threads | Initial production / stock medians, ms | Paired production / experiment / stock medians, ms | Experiment/production paired min / median / max |
|---|---:|---:|---:|
| Dense, 245760 / 50 / 16 | 752 / 592 | 765 / 744 / 605 | .899 / .957 / .987 |
| Dense, 153600 / 31 / 4 | 617 / 554 | 624 / 632 / 568 | .998 / 1.011 / 1.036 |
| Sparse, 122880 / 31 / 8 | 395 / 2876, unranked | 409 / 374 / 2900, unranked | .889 / .914 / .924 |

Both dense stock comparisons pass sample, beta, V and RSS checks. Sparse stock
beta/V fail in both blocks and remain unranked; a fast native time cannot turn
this into an accepted comparator win. All nine native pairs have byte-identical
scientific exports, and within-artifact repeat checks pass.

The dense K50 paired median reduction is 4.3%, while its ratio of medians gives
2.7%; the difference reflects baseline dispersion. Dense K31 loses about 1.1%
by paired median. The sparse native reduction of 8.6% is real in this screen
but insufficient to override the declared broad-gain rule. The four predeclared
fresh confirmation cases were not run after this mixed result; no favorable
seed, width or thread guard was selected afterward.

Tracked native peak differences range from −161,442,812 to −2,537,644 bytes.
Process RSS medians in GiB are 1.121→1.004 for dense K50, .419→.377 for dense
K31, and .387→.343 for sparse K31. The original per-run peaks and sample counts
remain available. These reductions are useful diagnostic evidence, but there
is no constrained-memory test or acceptance of the lower diagnostic admission
model. The production memory model remains untouched.

## Validation and disposition

The isolated artifact passes its actual-plugin ABI harness and five focused
Rust transport tests: bit-preserving copies, full/windowed batch equivalence,
cold/prepared retained-policy equivalence, later-window failure atomicity and
allocation error classification. These tests include current supplied weights,
three preconditioner choices and multiple installed thread counts.

The new `ols_three_fe_batch_oracle.do` independently checks three-FE explicit
LSMR at N16,416/K17 under unit/AW weights, robust V and T1/T4/T16 against Stata
dummy OLS. It passes on both production and experiment, with six native fits
per artifact and explicit zero-return/terminal sentinels. Its separately
recorded executed-file hash binds this fixture, which was added after building
the experiment. Retain this useful regression fixture even though the native
change is rejected.

The reusable build option and source instrumenter reproduce only an isolated
diagnostic build. This closes the window experiment; it is not production
adoption, constrained-memory repair, qualification or a claim about all
three-FE designs. The frozen benchmark definitions and reserved seeds are
unchanged. Existing allocation-source drift, weighted constrained failures,
nonretaining CG instability and deferred SCC checks remain open.

The next bounded subsystem should be generated/factor-term command cost and
requested outputs, whose current comparative coverage remains incomplete.
Three-FE upstream operator/preconditioner work remains a larger independent
algorithmic opportunity; more RHS-window tuning is not justified here.
`evidence.zip` contains 1,100 verified portable entries, including plans,
unrun confirmation declarations, controllers, identities, all attempts/exports,
the rejected patch, tests and fixture receipts. Data, binaries and raw licensed
logs are excluded.
