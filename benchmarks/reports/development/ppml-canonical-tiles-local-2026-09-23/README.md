# Retained canonical PPML tiles and norms — local development

Accept `canontile1` under [ADR 0138](../../../../docs/adr/0138-retained-scratch-standardization-and-norms.md).
Private scratch tiles standardize selected values once per row and Gram-row
partition, and retained one-FE projection borrows same-call certified norms
instead of scanning projected columns again. Low K32 and fresh K65 public
medians improve by **10.2% and 11.1%**, with unchanged memory peaks. Narrow
models show smaller or no median gains. **Every measured one-FE case still
loses to comparable stock fixest.** This supporting improvement does not close
the whole-command target of at most 0.90 times stock.

## Source and measurement boundary

Baseline is production `68e3ba75`, artifact `onelow1`, plugin SHA-256
`c69cc27f61955232ee73f3183ec60f947203c0d5d9e91164ef97f8d58aac78c0`.
Candidate overlays only `ols/finite_wls_kernel.rs` and `ols/linear_session.rs`,
plugin `5ede5b83f42a4a291e56dab2b27a94c3711197ac3a32d3ef6d64870461a1b205`.
All **539 captured computational inputs** match the accepted source. Identities,
complete source captures, declarations and harness recipes are archived.
Documentation does not alter those computational inputs.

The reference is pinned, unmodified fixest 0.14.2, `glm.tol=1e-10`,
`fixef.tol=1e-11`, matched threads and serial BLAS. Its source archive SHA is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Both commands perform robust inference and return fitted means. This is the
existing positive-outcome numeric PPML design; weighted/generated cases have
separate independent scientific fixtures. Low versus ordinary stock is not
physically memory-matched. Default-reference accuracy failures remain separate.

Each application warms once, then times the whole public command. Data setup
and process launch precede timing; plugin loading, preparation, estimation,
inference and publication are included. Baseline/candidate/stock rotate under
the existing harness. Seed 202609335 is development; seed 202609338 and its five
fresh cases were declared before candidate results. Candidate source was frozen
before the fresh block. No builds, profiles or archives overlap headline fits.
No monitor error, competing fit, application failure or timeout occurred.

## Decision and fresh distributions

Median [minimum,maximum] milliseconds, five decision rotations:

| One FE, T4 | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Low N30720 K6 | 59 [59,62] | 59 [58,60] | 25 [24,27] | 2.360 |
| Low N61440 K10 | 122 [120,124] | 119 [117,124] | 53 [50,55] | 2.245 |
| Low N30720 K32 | 167 [163,171] | 150 [148,156] | 121 [120,123] | 1.240 |
| Fast N61440 K10 control | 104 [101,106] | 104 [101,107] | 54 [54,55] | 1.926 |

Low K6 has two gains, two ties and one loss; K10 four gains and one loss;
K32 all five gains. The fast control has two gains, one tie and two losses.
The small pilot's single-rotation baseline/candidate/stock medians are
13/13/10, 18/17/11 and 23/23/14 ms; use it for science, not a speed-margin claim.

Fresh cases, three rotations each:

| Low one FE | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N46080 K8 T8 | 86 [84,86] | 85 [83,86] | 39 [39,39] | 2.179 |
| N61440 K10 T1 | 165 [164,166] | 160 [157,161] | 95 [93,97] | 1.684 |
| Unbalanced N61440 K16 T4 | 158 [158,168] | 149 [148,155] | 113 [109,118] | 1.319 |
| N12288 K65 T4 | 234 [234,236] | 208 [204,212] | 127 [126,131] | 1.638 |
| Small N3840 K3 T1 | 23 [17,23] | 22 [17,23] | 13 [10,13] | 1.692 |

Fresh reductions range 1.2–11.1%. Twelve native pairs improve, two tie and one
loses; the loss is 86 versus 84 ms in the K8 case. All K10/K16/K65 pairs improve.
Small-fit millisecond rounding and the 17–23 ms spread preclude a precise small
margin claim. One-FE topology names describe group occupancy, not connectivity
between FE partitions. Every public state and comparator check passes.

## Controls, science and memory

All **376 distinct Rust tests** pass: 343 PPML, 25 finite-moment, five retained
linear-session and three selected-norm tests. The expanded independent scalar
test checks exact Gram/RHS arithmetic through K257, scaled/unscaled and
unit/provided weights, contiguous/noncontiguous selection and T1/T4/T16. It
rereads the original scratch artifact to prove immutability. Existing tests
exercise stored width 1025, bounded reads, rank/QR fallback, changing frames,
strict rollback and invalid norm reuse. No tolerance or test bound changed.

Production-plugin ABI/panic checks and **ten licensed runs of eight fixtures**
pass with explicit application return codes and terminal sentinels. Independent
full-dummy Poisson/Hessian/sandwich checks cover unit/FW/PW, offset, robust and
clustered inference, generated terms and zero identified controls. The real
allocation audits at pool1/T1, pool1/T4 and pool2/T4 run in separate processes,
retain zero events/growth, stable capacities and warm/cold equivalence. Separation,
fast one-FE, categorical, slope and OLS memory-mode fixtures also pass.

Eight excluded multi-FE/slope PPML pairs and eight OLS pairs retain **exact
scientific exports**. The two-rotation PPML control medians (baseline/candidate)
are fast three-FE 182/184.5, low three-FE 349/347.5, low slopes 154.5/154.5 and
fast slopes 130.5/130 ms. Keep the fast categorical loss and individual pair
losses visible; this is no universal timing non-regression claim. OLS control
medians are 67.5/66.5, 64.5/61.5, 18/17 and 66/65.5 ms. Their one-shot execution
does not use this changed retained scratch moment source.

Public receipts contain **146 passing applications**, **130 passing PPML
states**, **46 native PPML pairs**, **38 baseline/stock pairs**, **38 additional
direct candidate/stock checks**, nine small independent Poisson/Hessian checks
and eight exact OLS pairs. The eight exact excluded PPML pairs are included in
the native count. Separate diagnostics add sixteen passing applications/states
and six pairs. No scientific discrepancy or independent-oracle skip occurred.

Selected modeled peaks, tracked peak deltas, scratch bytes and reported outer/
inner iteration counts are unchanged on every measured cell. Low N61440/K10/T4
has tracked peak delta **34,603,565 bytes**, modeled peak **43,061,399**, observed
scratch **4,915,200** and modeled scratch **5,406,720**, in both production arms.
Forward tile compaction uses existing storage; selected norm reuse adds none.
These are modeled/allocator observations, not physical-cap qualification.

## Attribution and remaining budget

The source comparison found that retained scratch moments repeated selected
column lookup and scaling inside every Gram entry. Dense storage already reused
standardized values. Reusing the existing scratch tile is a generalization of
that organization, with exact per-coordinate arithmetic. The projector also
already computed the returned column norms in its fresh one-FE certificate;
the low retained caller discarded them. The accessor's current-call and exact
normalization gates remain authoritative.

The first two-case unpaired diagnostic traces showed unrelated timing variation
in the unchanged fast finite kernel. Preserve them, then use three separately
declared instrumented baseline/candidate rotations for attribution. Both
diagnostic builds are isolated; neither supplies headline stock timings:

- `onecanonledger1`: `22ab339cd90f90bccc9c516ed403dd0556b97b241828b2955fc7fd01860d73d0`.
- `canontileledger1`: `84d4d2bd6f57ccd710e07971cb7ce5d21318974ac5cbb734be379e6bf5a57960`.

Each instrument uses a shared monotonic epoch. The summarizer subtracts the
union of nested spans and verifies that exclusive costs sum to the plugin
span. The following are medians across paired diagnostic runs at N61440/K10/T4;
rows marked inclusive must not be summed with their children.

| Low path | Baseline | Candidate |
| --- | ---: | ---: |
| Plugin, inclusive ms | 109.126 | 102.217 |
| Initial canonical WLS, inclusive ms | 24.847 | 22.325 |
| Strict canonical WLS, inclusive ms | 23.500 | 20.128 |
| Finite moments, two calls, ms | 10.176 | 7.245 |
| Initial raw/within norm scans, calls / ms | 2 / 2.775 | 1 / 1.409 |
| Separate refreshed within-norm scan, calls / ms | 1 / 1.544 | 0 / 0 |
| Current raw norm/mean refresh, one call, ms | 4.025 | 4.131 |
| Objective evaluation, six calls, ms | 10.999 | 11.030 |
| Two ordinary profiled directions, inclusive ms | 5.732 | 5.656 |
| Inference, ms | 1.780 | 1.698 |
| Core publication, inclusive ms | 9.383 | 9.248 |

There are still six canonical projection calls in both low arms; the candidate
uses the norm-returning helper for two. Both canonical full-rank solves remain.
The paired fast diagnostic plugin medians are 87.601/88.860 ms, reinforcing why
instrumented timing is not a production non-regression result. Fast initial and
strict WLS still cost 16.849 and 14.160 ms inclusive in the candidate.

This evidence rules out small factor/inverse tuning as the next large lever.
The existing full-rank path already tries guarded pivoted Cholesky before
retained QR. The important remaining work is how canonical design/products are
constructed and independently certified, not a missing small-matrix solver.
The larger grouped-original-column representation was investigated but is not
implemented here: session ownership, rank fallback, reconstruction and inference
currently consume a retained dense or scratch design. Replacing that dependency
requires one coherent product owner rather than an additional cache layer.

At the decision stock median the 10% target is **<=47.7 ms** for low N61440/K10,
against 119 ms here; the fast target is **<=48.6 ms**, against 104 ms. Even
eliminating all six objective evaluations would be insufficient. A subsequent
design must combine less canonical materialization with shared accepted-state
work and examine the public control-plane budget. Independent original-input
rank/projection, score, objective, reconstruction and final-Hessian witnesses
remain required; the current implementation of those witnesses is revisable.

## Preserved limits and receipts

No sample, weight, rank, DoF, separation, certificate, numerical envelope or
failure-atomicity policy changed. Static allocation-inventory drift, mixed-thread
pool-retirement audit failure, constrained weighted/nonretaining-CG issues,
earlier unresolved comparisons and SCC/platform/physical-cap deferrals remain.
The frozen 77 primary, 16 holdout and 111 native-control definitions are untouched.
No full qualification, stopped campaign, push or release was performed.

The content-addressed archive has **7,126 logical receipts**, **1,717 payloads**,
**15,635,559 bytes**, SHA-256
`f8fbd79ab2fce16bf7b605b2ea038cceb7eac489a882bb41fadea6b7d9ec842b`.
`restore_receipts.py` verified and restored every logical file to a new local
directory. Both original and sanitized hashes are retained. Raw licensed logs,
data matrices and binaries are omitted; hashes, source and regeneration recipes
are retained. These are bounded local development receipts, not qualification.
