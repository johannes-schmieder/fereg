# Parallel retained-scratch residual reconstruction

Local development evidence, 2026-09-26; not qualification. Adopt **residualrows2**
under [ADR 0216](../../../../docs/adr/0216-parallel-retained-scratch-residuals.md).
The multi-FE PPML goal of at most 0.90 times comparable stock remains active.

## Source and scope

Baseline **multitopology1**, accepted at **20e764f5**, plugin SHA-256
`ec59c589f62fd3c0992fb2428355b6b315ee10e28f029762024bc9a4440cddb2`.
Candidate **residualrows2**, base 20e764f5 plus overlays in
`crates/fereg-core/src/ols/linear_session.rs` and `ols/prepared_fit.rs`, plugin
SHA-256 `ef3ee205cab739352ccc1024500e26e9c7c874c609001c58cf0bc7038c32f519`.
All **567 computational inputs** match the measured artifact. Identities retain
complete hashes, isolated builds and ABI/panic-containment checks.

The fresh/strict finite solve reuses its consumed moment workspace as complete
row buffers for parallel scratch residual reconstruction. Bound partitions by
the installed requested team, rows and available complete buffers. Each has an
independent immutable 8-KiB read cursor. Insufficient buffers, serial execution
and zero stored width retain the scalar path. There is no benchmark-specific
guard, new threshold or persistent numerical cache.

Read every stored column, including omitted ones; preserve row-local weight,
square-root, selected-column FMA and division order. No reduction is reordered.
Bounds, poison, nonfinite and I/O checks remain authoritative. Current-weight
projection, rank, finite solve, original-input scores, model space, objective,
reconstruction, final Hessian and transactional publication remain unchanged.
Dense/implicit retained reconstruction, frozen residual reconstruction and
public one-shot OLS keep their prior implementation.

Stock fixest is unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`, with
glm.tol=1e-10, fixef.tol=1e-11, matched threads, BLAS one, matched robust/cluster
inference and fitted means/full outputs. Public clocks include loading,
estimation and requested publication, excluding harness setup/import.
No compilation, profiling, auditing, archiving or competing fit ran alongside
headline timing. All completed process monitors are clean.

## Public performance

B=multitopology1, C=residualrows2, S=comparable stock. Medians are milliseconds.
[Ordered distributions](distributions.md) retain every loss and control.
Pilot seed202609347, three rotations:

|Case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric low;983040/6/8|993 /962 /900|1.066–1.081|
|Matched fast|891 /886 /901|0.981–0.987|
|Two-FE two slopes low;30720/6/4|75 /73 /116|0.621–0.629|
|Three-FE two slopes fast;61440/10/8|159 /152 /231|0.654–0.661|
|Unbalanced three-FE factors, cluster, fast;153600/19/8|584 /583 /719|0.809–0.818|
|Two-FE serial fast;245760/6/1|562 /561 /986|0.569–0.574|

Pilot B/C outcomes: **16 wins, two losses**. Four of six families meet 0.90
stock in every pair. Million-row low improves **3.1%**, but still loses to stock.
Dense and serial changes are controls, not gains attributed to this branch.

Fresh **seed202609446**, five predeclared rotations, is consumed. No source or
guard changed after the pilot. All **40** fresh direct C/stock comparisons pass.

|Fresh case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;737280/6/8|680 /672 /769|0.858–0.900|
|Matched low|754 /730 /773|0.927–0.955|
|Two-FE numeric low;368640/6/4|450 /441 /681|0.643–0.656|
|Unbalanced three-FE factors, cluster, low;184320/19/8|653 /641 /3212|0.199–0.201|
|Three-FE four slopes low;38400/6/8|106 /106 /117|0.889–0.922|
|Two-FE serial low;245760/6/1|565 /566 /929|0.607–0.615|
|Weak_d3 low;61440/6/4|132 /131 /871|0.147–0.153|
|Disconnected two FE low;61440/6/4|94 /93 /99|0.921–0.959|

Fresh outcomes: **28 wins, five losses, seven ties**. Five of eight families meet
0.90 stock in every pair. Large low improves **3.2%**, medium low **2.0%**, and
wide-factor low **1.8%**. Retain the four-slope tie and serial 1-ms loss. Large
low, four-slope low and disconnected low miss the target. These fresh results
do not retrospectively repair discrepancies or timing losses on other seeds.

## Measured attribution and next implementation

Isolated **rankprofile1/rankprofile2** use identical finer instrumentation on
the baseline and final candidate. Two alternating diagnostic pairs per case;
diagnostic clocks never enter stock rankings. An earlier six-fit baseline
diagnostic is also retained, including the evidence used to choose this change.

|Boundary, summed over a fit's two canonical calls|Million low B/C ms|
|---|---:|
|Residual reconstruction|45.715 /15.397|
|Containing retained-rank solve|127.474 /97.190|
|Finite rank/factor work|4.080 /3.994|
|Norm scans|15.840 /15.798|
|Canonical geometry setup|40.121 /40.292|
|Retained-control writing|23.279 /22.967|

The 30-ms reconstruction saving explains the public pilot improvement. Dense
reconstruction is **4.636/4.324 ms**. Whole diagnostic plugin times are
**936.914/912.697 ms** low and **836.687/859.374 ms** fast; retain that fast
diagnostic loss rather than implying a universal non-regression. Phases nest:
the containing solve includes reconstruction and norm scans. Worker sums are
not independent wall times.

Current candidate canonical preparation totals **229/248 ms** fast/low. It
includes setup **42/40**, reduced solving **45/44**, original-input certificate
and product construction **22/22**, and retained-rank solving **86/97**. Within
the last item, residual projection costs **20/22** and assembly **39/40**.
Outer original-input score checks cost another **40**. Loading is about **41**,
publication **24**, and inference **7/10**. Do not add nested spans twice.

The next bounded hypothesis is **one-pass current-weight incidence construction**
in `ols/canonical_products.rs::build`. Setup currently traverses each left group,
accumulates and sorts weighted right neighbors to form the diagonal, then repeats
that traversal/accumulation/sort to pack the implicit operator. Measured matrix
and packing phases cost **11.9 +15.2 ms** in low mode; these are disjoint spans,
not all assumed removable. Store the first pass's ordered weighted incidences in
the already admitted factor/index banks, then scale and compact them after
current-weight roots/anchors are known. Keep prefix/suffix diagonal arithmetic,
finite checks, exact memory admission and the fresh original-input certificate.
This can benefit fast and low canonical construction without a persistent cache.

Immutable component planning is only **5 ms**: do not build a complex topology
cache to chase it. Low control writing already parallelizes row encoding within
bounded tiles; its **23 ms** cannot be called a wholly serial loop. Any successor
must distinguish encoding from sequential tile I/O or fuse work at the certified
canonical-product boundary. Norm scans (**16 ms**) may be another one-use handoff
opportunity only after proving current-frame units and independent acceptance.

Median deficits to 0.90 stock remain about **75/152 ms** in the million-row
fast/low pilot, **34 ms** in fresh large low, **1 ms** in four-slope low and
**4 ms** in disconnected low. Stop residual-loop tuning here; the measured
remaining work calls for canonical-frame organization, not a favorable seed.

## Science, controls and memory

**76 distinct Rust tests** and **nine licensed fixtures** pass. The new weighted
Walsh oracle checks closed-form coefficients/residuals with 6/65 independent
columns plus an exact duplicate, T1/T3/T8, uneven row partitions and changing
weights. Controls cover scratch corruption/truncation, independent reader pages,
zero/single rank, wide QR, requested thread teams, strict rollback, weighted
canonical contrasts, Poisson/Hessian inference and shared multi-FE OLS.

All **204 PPML public states** pass original-input sample, score, model-space and
objective checks: 186 production and 18 diagnostic. **12** explicit Poisson/
Hessian/sandwich comparisons pass. All **68 native PPML pairs** have exact
coefficient, covariance, mean and requested full-output exports. Anchored
comparisons pass **129/130**; direct C/stock pass **61/62**. The one old-seed small
full-output residual discrepancy remains unranked. No completed pair is missing.

All **18 protected OLS pairs** have exact coefficient/covariance exports. B/C
medians for slopes, three-FE low, small numeric, factors, large numeric and one FE
are **65/65, 76/76, 14/14, 209/209, 159/158, 17/17 ms**. This bounded screen does
not support universal timing non-regression or full qualification.

No explicit heap bank, file, pool or allowance is added. Existing moment storage
holds the row buffers. Concurrent cursor pages use 8 KiB per active partition on
existing worker stacks, increasing simultaneous pages versus the scalar scan.
Tracked whole-fit heap peak and scratch deltas are **zero**; modeled bounds vary
**-48..+48 bytes** with unchanged admission. Existing stack/runtime headroom is
used; physical-cap and platform qualification are not claimed.

The initial new unit test failed to compile because an optional generated-column
index lacked an unwrap. Preserve that log and the unmeasured **residualrows1**
build; **residualrows2** differs only by the test correction. No production source
changed afterward. No timed application failure, timeout, interruption or
contended measurement occurred. Known static inventory drift, wire-minor and
checkpoint-count failures, weighted constrained-fit/nonretaining-CG failures and
SCC deferrals remain open. Frozen 77/16/111 definitions and stopped campaigns are
unchanged. No push or release.

## Receipts

The content-addressed archive retains source snapshots, build and process-monitor
identities, predeclarations, ordered timing/science results, test summaries,
filtered licensed sentinels, raw phase records and analysis recipes. Generated
datasets and binary output banks are hashed but omitted; no raw licensed logs or
personal paths are committed. `archive-verification.json` records container and
restoration checks. Restore with the adjacent `restore_receipts.py` into a new
directory. Local work root: `target/ppml-canonical-rank-20260926`.
