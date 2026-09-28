# Private implicit finite moments and macOS runtime initialization

Accepted bounded development improvement, 2026-09-28. The broader scientific
and performance implementation goal remains active.

## Scientific and allocation validation

The baseline is `07ec4df5`, package `a9fb2307…` and Linux `f919ea96…`.
The first candidate `c7032da9…` has no headline timing. Linux 7763918 passes
101 focused Rust tests and six fixtures plus PPML smoke, but Mac has a real
zero-allocation audit failure. Its failure receipt, paired diagnostic blocks,
and the complete diagnosis are preserved in [allocation-diagnosis.txt](allocation-diagnosis.txt).

An isolated corrected trace records one 48-byte macOS allocation inside the
sixteen-iteration audit window. The [symbolicated stack](allocation-trace02-stack.txt)
identifies Rayon worker sleep initialization during PPML mean construction.
The old startup broadcast could leave some worker condition variables cold.
The initial trace incorrectly combined action and audit probes; its outcomes
are retained as invalid attribution, never acceptance or headline evidence.

[ADR0244](../../../../docs/adr/0244-eager-macos-worker-sleep-state.md) initializes
this runtime state during pool construction. Original upstream source/licenses
are preserved and file-bound; only the macOS constructor's behavior changes.
The independent cold-state test fails on the upstream constructor and passes
on the patch at 1/4/16 workers. No event is exempted from the audit, and no warm-up
delay or convergence tolerance changes.

Revised package [c09eeab3…](candidate02-source.json) binds 259 inputs, including
the vendor source. [Mac](mac-validation02.json) and [Linux 7764784](linux-validation02.json)
pass 104 distinct Rust tests and six licensed fixtures each. The Mac additionally
passes the cold-state test and [twenty fresh-process low-memory audits](repeated-audit02-results.json)
at T4/pool1 and T4/pool2, all unchanged zero-event, capacity and equivalence checks.
Linux also passes PPML smoke. The six fixtures include explicit weighted
Poisson/Hessian/sandwich, FW/PW/cluster/saved-output, low memory, categorical,
three-partition slopes and OLS checks. Scalar moment oracles cover changing
weight magnitudes, selected columns, the 64/65 boundary and T1/2/4/16.
There are 42 source-identity/calibration/arithmetic harness tests and 13 rotation
tests. No qualification, Windows run or restarted campaign is claimed.

Mac universal is `38bf6916b7c0c59732269df7658281b267440411382fa1ae34631a5be7d7101a`;
Linux is `59d0341ae2c760b4701381e1a2153f1238ab94d752457429bb8642380f503920`.
Validation job 7764784 uses 280s and 5.433G maxvmem, with clean accounting.

## Computational change

At up to 64 active columns, implicit retained finite moments keep row scratch,
RHS and Gram rows private on the worker stack and publish completed rows once.
The scaled dense route uses the same bounded owner, including layouts assigning
one left column per partition. Wide, unscaled dense and scratch routes retain
their previous loops. The existing implicit scratch bank remains admitted for
its other/wide uses; there is no new heap or observation bank. All per-entry row
orders, divisions and FMA recurrences remain unchanged.

The fixed payload is 33,792 bytes per active partition (33 KiB), at most
2,162,688 bytes across 64 eligible simultaneous partitions. No additional thread
or explicit stack reservation is made. The earlier ADR0243 statement about an
8 MiB reservation must not be generalized to the shared `ThreadTeam`: that team
forwards a configured Rayon stack size, otherwise using the Rust runtime default;
CMG has its separate explicit stack allowance. Complete stack/physical-cap
qualification remains open. Heap receipts do not include this stack payload.

Mac eager initialization materializes existing lazy runtime backing storage
earlier, even for workers that never sleep. Its live state enters the existing
PPML embedding observation before core admission. This is a small possible
startup-resident increase, not a claimed memory reduction.

## Matched command timings and distributions

Public `feppml` is compared with pinned, unmodified `fixest::fepois` (robust VCE), and public `fereg` with `fixest::feols` (IID). No optional observation outputs are timed. The slope reference uses glm.tol=1e-10 and fixef.tol=1e-11; other stock cases retain their registered defaults. The canonical warm-up, imports, sample audits and unchanged science gates apply. Setup outside the command is excluded; loading/preparation/publication inside it is charged. Low-mode comparisons use stock fixest at its normal memory policy, not a matched physical-memory cap.

| Block | Job | Seed | Applications |
|---|---|---:|---:|
| Initial | 7764833 | 2026092817 | 78 |
| Fresh | 7764860 | 2026092833 | 117 |
| Direct scaling | 7764860 | 2026092835 | 54 |

Both timing allocations reserve 28 isolated E5-2680v4 cores. Arms alternate order; fresh/scaling run sequentially. No compilation, profiling, archiving or competing fit overlaps headline clocks. Accounting is clean: initial 261s/1.998G; fresh plus scaling 561s/1.999G. Every collected byte is SHA-checked and local re-summarization reproduces the remote summary. Fresh/scaling seeds are now validation-only, never tuning inputs. All jobs are terminal.

**249 applications, 83 native and 83 stock scientific comparisons pass. Every one of the 83 native b/V export pairs is byte-identical**, including all 18 cross-thread pairs. All 65 same-thread iteration and selected-memory checks agree. No timing NR, timeout or application failure is hidden in these final blocks. Earlier Mac audit failures remain separate evidence; these clocks do not adjudicate older campaign discrepancies.

Ratios are candidate/baseline (C/B) or candidate/stock (C/S); smaller is faster. Ranges are paired minima/maxima, not confidence intervals. Case dimensions and exact commands remain in the registrations.

| Case | Initial C/B | Fresh C/B [range] | Fresh C/S [range] |
|---|---:|---:|---:|
| one-low-t1 | 0.826 | 0.849 [0.576, 1.046] | 2.610 [2.337, 2.673] |
| one-low-t2 | 0.810 | 0.840 [0.708, 0.875] | 2.171 [1.406, 2.590] |
| one-low-t4 | 0.758 | 0.759 [0.745, 0.781] | 1.682 [1.575, 2.420] |
| one-low-wide-t4 | 0.829 | 0.858 [0.805, 0.903] | 0.955 [0.915, 1.004] |
| one-low-larger-t8 | 0.669 | 0.653 [0.637, 0.684] | 0.954 [0.922, 0.960] |
| one-single-row-t8 | 0.756 | 0.748 [0.734, 0.803] | 1.533 [1.235, 1.861] |
| control-fast-t4 | 0.977 | 1.039 [0.972, 1.044] | 2.391 [1.955, 2.443] |
| control-fast-wide | 0.980 | 1.031 [0.967, 1.041] | 0.876 [0.857, 0.890] |
| control-fast-larger | 1.029 | 1.004 [0.977, 1.028] | 0.929 [0.918, 0.937] |
| control-two-fe | 1.008 | 1.048 [1.016, 1.057] | 1.798 [1.780, 1.889] |
| control-three-fe | 0.970 | 0.999 [0.966, 1.065] | 0.520 [0.507, 0.541] |
| control-slopes | 0.859 | 0.966 [0.816, 1.006] | 0.330 [0.306, 0.405] |
| control-ols | 0.981 | 0.990 [0.894, 1.080] | 1.541 [1.447, 1.570] |

The five low-memory cases improve 14–35% in fresh medians; narrow fast T8 improves 25%. Serial low has one 4.6% fresh loss and a zero-gain initial pair. Fast T4/wide/two-FE controls lose 3.9%/3.1%/4.8% fresh; the two-FE loss follows a 0.8% initial loss. Larger fast is +0.4% fresh after +2.9% initially. Three-FE is effectively flat, slopes improve 3.4%, and OLS improves 1.0% in the fresh medians. These controls are not universally performance-neutral.

Fresh finite-solve medians (baseline→candidate) are low T1 **65.5→52.9ms**, T2 **71.2→40.4ms**, T4 **63.3→29.3ms**, wide **189.3→143.5ms**, larger **359.5→124.4ms**, and narrow fast T8 **48.2→21.3ms**. Inference remains mixed: low wide **14.9→17.9ms**, larger **10.7→14.3ms**, T2 about **10ms** in both arms. Phase clocks may nest and must not be added. Serial gains also show that cache-coherence avoidance alone is not a complete measured explanation; private storage/compiler access effects remain an inference.

| Same candidate: native threads lower→higher | High/low median [range] |
|---|---:|
| one-low-t2-1-to-2 | 0.776 [0.730, 0.984] |
| one-low-t4-2-to-4 | 0.878 [0.854, 0.901] |
| one-low-t4-4-to-8 | 0.901 [0.882, 0.905] |
| one-single-row-t8-4-to-8 | 0.924 [0.872, 0.941] |
| one-low-wide-t4-1-to-4 | 0.608 [0.494, 0.650] |
| one-low-larger-t8-4-to-8 | 0.851 [0.810, 0.857] |

**All 18 direct scaling pairs improve**, with actual requested/effective counts matching each registration. Low 1→2/2→4/4→8 medians are .776/.878/.901; narrow fast4→8 is .924, wide low1→4 is .608, larger low4→8 is .851. No thread ceiling changes. This resolves the tested transitions, not a universal no-slowdown guarantee.

## Memory, adoption and remaining work

Fresh C/B RSS median ratios span 0.977–1.028; individual ratios span 0.942–1.102. PPML native heap peaks agree in 55/60 same-thread pairs. Three-FE peaks vary: initial candidate peaks decrease 3.44/6.64MB, while fresh peaks increase 3.98/4.13/6.14MB (up to 6.0%). Changed overlap of existing independent work is plausible, not proved. OLS selected memory and exported native-allocation bytes agree in all five pairs. More threads retain the existing larger worker/reduction footprint; the scaling block is not memory-neutral.

Adopt the bounded ownership change and the required runtime initialization repair: substantial, repeated low-memory and narrow high-thread gains outweigh the recorded small control losses, with no extra heap bank, numerical recurrence or scientific policy. The controls remain follow-up targets; do not report this as universal non-regression or tune this candidate to the validation seeds. [ADR0245](../../../../docs/adr/0245-private-implicit-finite-moments.md) records the solver decision.

The <=1.20 stock ceiling and <=.90 aspiration remain unmet. Fresh stock ratios are **2.61/2.17/1.68** for small low T1/T2/T4, **1.53** for narrow fastT8, **2.39** for fastT4, **1.80** for small two-FE and **1.54** for OLS. Wide fast **.876**, larger fast **.929**, low wide/larger **.955/.954**, three-FE **.520** and slopes **.330** are substantially more competitive. These refer only to these source-bound designs and outputs.

Next, return to the approved broader current-source development inventory/sweep and original failure cases; avoid another long tuning loop for these small control losses. For small categorical PPML, remaining measured costs include preparation, strict/final work, inference and publication. FastT4 finite solving is unchanged at 26.0ms while inference rises 3.8→6.6ms; two-FE strict/final work rises 40.8→47.0ms and publication16.6→20.5ms. These identify concrete boundaries for the next implementation after the broad screen, not an explanation of every millisecond or a promise of parity. Older weak-SE discrepancies, large timeouts, comparator sample/VCE/extraction issues, static allocation drift, constrained fits and nonretaining-CG instability remain open. Frozen 77/16/111 definitions are unchanged. No push, release or qualification.
