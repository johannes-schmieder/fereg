# P6 saved two-FE Schur probe: promising, not adopted

The isolated prototype permits ordinary OLS to omit its initial FE coefficient
batch only after the existing weighted Schur probe accepts. The publisher then
uses its existing accepted-final-target recovery. Rejected probes retain the
original coefficient policy. Other weighted-linear callers default to requiring
their existing products; PPML session/reconstruction source is unchanged.

**Disposition: incomplete evaluation, no adoption.** One original unit-weight
screen shows a substantial command/memory gain. The original AW operator rejects
Schur and retains the baseline behavior. The original PW baseline fails before
the candidate is run, exposing a separately reproduced CG energy-roundoff issue.
That failure and overlapping recovery memory admission must be resolved before
repeating or accepting the optimization. No target or seed has been substituted.

## Source and validation

Accepted numerical source is `257f4ce`; evidence HEAD is `75ad177`. The baseline
plugin is `c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`;
the prototype is `002a3089bd3b1a40cbe51542c9954ceed7703073c2388aaa297f8acf2cf161a1`.
[Identities](built-identities.json) bind all 176 build/fixture inputs, including
the two changed sources. The complete [patch](implementation.patch) and isolated
source remain available. Both architecture slices compiled the changed crates;
static audits and actual native arm64 ABI/self-tests passed.

The focused core guard test passes with new accepted/rejected saved-probe checks
and the existing default-policy, solver, width, memory and cycle controls.
The independent [oracle](oracle.do) passes **48 fits on each artifact** at
N=18,432 and K=16: unit/AW/FW/PW, one/two FE, fast/low, missing/restricted sample,
full/partial/combined outputs, explicit-dummy slopes/covariance/residuals,
mean-zero normalized saved channels, row ordering and rollback. Candidate fast
two-FE oracle fits assert actual accepted Schur dispatch. The constant remains
a deferred normalization coordinate. These are bounded local development tests.

## Original first screens

The [prospective protocol](protocol.md) fixes seed 104729, fresh Stata processes,
an untimed N=7,680 warm-up and rotated arm order. Complete commands include
requested residuals and all saved-effect recovery/publication. `strong_d2` means
two FE with the degree-two graph generator; it does not guarantee acceptance of
the actual weighted operator. No build, test or statistical profile overlapped
the paired timing.

| Original case | N / K / native threads | Command seconds, baseline → candidate | Native peak MB, baseline → candidate |
|---|---|---:|---:|
| Unit, both FE channels saved | 245760 / 50 / 16 | 2.897 → 0.650 | 839.05 → 378.71 |
| AW, both FE channels saved | 983040 / 50 / 4 | 84.063 → 83.710 | 3363.96 → 3363.96 |
| PW, missing, two-way clusters, one FE channel saved | 245760 / 50 / 16 | baseline r(459); candidate not run | no successful action peak |

The unit pair saves **77.6% command time and 54.9% native peak**. Input time is
essentially unchanged, 0.07516→0.07462 s; native time falls 2.78439→0.53830 s.
The initial solve report attributes setup/solve 0.06486/2.59258→0.02331/0.13660 s.
The candidate accepts contraction 0.0087764, charges eight probe iterations and
reports 65 total initial iterations versus 59 for the baseline; these are
different engines, not an assertion that iteration work became cheaper by a
fixed factor. The baseline initial fallback flag is one, the candidate's zero.
Final residual projection uses zero iterations in both. There is no isolated
final-recovery timer or actual hardware-traffic count.

AW contraction **0.0106146 exceeds the unchanged 0.01 guard**. The candidate
correctly adds eight charged probe iterations and retains the same underlying
solve, full b/V, fit/certificate exports and saved values exactly. Both commands
spend about 82 seconds in initial solving. Its 0.4% timing difference is not an
optimization gain. The original checker incorrectly asserted all nominally
strong cases must select Schur; its failure, original harness/identity and
unchanged paired data are retained. The corrected check requires exact science
when the probe rejects; no AW command was rerun to replace this pair.

The [comparisons](comparisons.json) and [complete numeric archive](numeric-evidence.zip)
retain b/V, samples/status/DoF, FE/cluster/subset tables, solver fields and saved
outputs under the original tolerances. Unit and AW science passes. Sampled RSS
falls 0.97246→0.53917 GiB for unit and rises 3.72612→3.73636 GiB for AW.

The public modeled peak is 418.80 MB for unit and 1675.12 MB for AW. Only the
unit candidate lies within that public model. The unit baseline and both AW
artifacts exceed it. These are P0 accounting gaps; a measured candidate peak
below a generic public model is also insufficient to prove every possible
recovery/fallback lifetime is admitted. OLS recovery can still enter legacy
LSMR with additional owned/operator storage. No memory-capacity acceptance or
new automatic region is claimed.

## PW baseline failure and scalar diagnosis

The original PW baseline returns r(459) after 0.495 s. Its candidate was never
launched. A fresh direct command and a direct command following the same small
warm-up both pass, so neither replaces the failed worker evidence. Replaying
the exact worker setup with errors visible reproduces r(459) at 0.494 s:

> borrowed CG Hestenes improvement potential became negative

This is an absorption failure before finite regression, not a publication
failure. A separate accepted-baseline build changes only a phase-gated scalar
diagnostic immediately before that existing error. Both architecture builds
and native ABI tests pass. Its exact-worker replay fails again at 0.488 s:

```text
rhs=51
previous=1.39583428571428522e5
decrease=1.39583428571428580e5
remaining=-5.82076609134674072e-11
roundoff_floor=3.09937472512241733e-11
```

RHS 51 is the zero-based explicit constant after outcome and 50 controls. The
subtraction overshoots by two ulps, beyond the present epsilon-scaled guard.
The diagnostic changes neither arithmetic nor error policy. Its
[source identity](energy-diagnostic-source-identities.json),
[patch](energy-diagnostic.patch), [result](energy-diagnostic-result.json) and
[phase lines](phase-diagnostics.json) are separate from the P6 prototype.
The exact cause of the direct-versus-worker difference is not established.

The next numerical experiment is a narrowly triggered independent energy
recomputation when the incremental energy becomes slightly negative. It must
distinguish roundoff from invalid steps and retain Hestenes convergence, raw
reference scales, coefficient reconstruction and final FE certificates. No
such repair is implemented or accepted by this report. Preserve and resume the
original PW workload after independently validating a repair; do not replace it
with a simpler cluster or graph case.

## Preservation and remaining work

[Failures](failures.json) also records initial snapshot/diagnostic-launch
mistakes; no failed command is counted as a pass. Licensed raw logs, binaries
and generated datasets remain local outside Git. The P6 patch is archived and
removed from main; [all 176 accepted inputs are restored](restoration.json).
Installed dist, all 358 original untracked paths and the 97 unrelated STATUS
additions are preserved. There was no agent, SCC, qualification or push.

P6 remains unchecked. After the baseline repair, resume the exact original PW
screen, establish recovery/fallback allocation admission and constrained-memory
behavior, then run the prescribed protected controls and repeated primary seeds
before deciding adoption. The single unit pair is a promising mechanism result,
not a cumulative or portable speed claim.
