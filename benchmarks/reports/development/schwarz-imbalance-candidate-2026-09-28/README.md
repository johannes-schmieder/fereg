# Schwarz scratch bound: isolated imbalance-aware candidate

This candidate targets a general overestimate of categorical Schwarz scratch.
It follows the [large OLS timeout diagnosis](../ols-timeout-memory-registration-scc-2026-09-28/README.md),
whose runtime attribution is still pending. **No production input has changed.**
The implementation and tests are frozen in `candidate.patch`; adoption requires
the queued core checks, public independent science and performance evidence.

## Bound and implementation

Let L be the sum of FE level counts, M their maximum, and
S = min(M, L − M). Every factor pair's smaller block has at most S levels,
including pairs not containing the largest FE. Disconnected subcomponents and
inactive levels can only reduce these counts.

The exact pinned `within` source first orients each component to eliminate its
larger block. For an unsigned categorical component with dimensions q and r:

- Each local buffer contains q+r values and a reduced factor on min(q,r).
- The existing bound allows two extra grounding values in each buffer.
- Both buffers therefore require at most
  **2(q+r+min(q,r)+2) ≤ 2M+4S+4 values**.

Replace the previous **6M+4** unsigned local-buffer bound with **2M+4S+4**.
When M ≤ L−M, the two expressions are identical. Signed-cover paths retain
**10M+4**, unchanged. Overflow remains checked; an inconsistent M>L unsigned
shape fails closed. No topology cache, new allocation or numerical recurrence
is introduced. The existing worker/re-entry bound, four retained pools, all
concurrent RHS applies, dense cross tabs, construction and recurrence storage,
execution parent and memory headroom remain charged.

Primary source: `within`/`schwarz-precond`
**1ac3522a251c2622dbeab451415924f9ebe117b9** and `approx-chol` **0.5.0**
(Cargo checksum `affa21d6…`). `proof-source-bindings.json` binds the inspected
factor-pair orientation, component construction, local scratch sizing, pooled
buffers and grounding implementation. In particular, `ReducedFactor::Direct`
uses no embedding scratch; the separate cover arm is deliberately unchanged.

For the current N3,932,160/K10/T4 degree-two OLS design, the retained-plus-live
apply subset falls from **339,769,803,360 bytes / 316.435 GiB** to
**151,144,088,160 bytes / 140.764 GiB**. This is scalar model arithmetic,
not a measured reduction in allocation or time and not the full admission
estimate. The implementation adds a full-model 192-GiB admission/128-GiB refusal
regression test, including a 4-GiB execution parent and existing headroom.
That Rust test has not run yet.

The shared model reaches borrowed/owned categorical LSMR and PPML joint and
checkpoint admission. Reaching a faster solver may increase actual memory use
relative to the former fallback. Any accepted route must still satisfy the
complete conservative envelope and its ordinary independent certificates.
This does not repair physical-cap accounting or static allocation-inventory drift.

## Validation completed and pending

Local scalar enumeration covers **576 aggregate frames and 32,400 possible
component shapes**, including both effect orders and disconnected subcomponents.
Every separately counted two-buffer requirement fits the proposed bound;
balanced frames preserve the previous value. This finite enumeration supplements
the source proof and does not replace it. The Rust patch parses and applies
cleanly; Python AST and shell syntax checks pass. All **259 registered accepted
computational inputs** still match. No compilation, numerical fit or licensed
fixture was run alongside the live headline sweep.

**7767319** is submitted with eight slots, 3G/core and a 30-minute cap, held
behind **7766045, 7766146, 7767016 and 7767269**. Root:
`/projectnb/welfgr/fereg/dev/20260928-schwarz-imbalance-01`.
Eight slots cover the selected tests' actual teams up to eight. It copies the
bound accepted Rust/package inputs, applies only this patch, records candidate
source and executable hashes, and builds in a fresh target directory.

The job requires:

- All core `memory::` tests and all categorical PPML LSMR tests, with complete
  discovered-test inventories and zero ignored/missing tests.
- Three new component-bound, signed/balanced/overflow and full-admission tests.
- Exact existing slow-chain routing and constrained LSMR-to-CG boundary tests.
- Existing multiway rank, weighted known-orthogonal residual and low-memory
  batch controls, including changing weights and wider regressors.

Each test group has a five-minute bound. Failures/timeouts preserve logs and
fail closed; the three new test names must exist and execute. Require clean
accounting, both core terminal markers, all expected records and the summary.
Passing this job permits public validation; it is not performance acceptance.
No public binary is built or published by this job. The separate nonretaining-CG
candidate is not included, so failures and gains can be attributed separately.

Deployment SHA:
`b4b4645a1d625827d3faf72890191f0e0ee448d45bff886bb545f0d07268b9ad`.
If the candidate passes and the earlier two-budget diagnostic confirms routing,
next build its public plugin, run independent OLS/Poisson fixtures and the
exact restricted-memory reproducer, then perform uncontended paired/fresh
measurements with relevant balanced, signed/slope and PPML controls. Preserve
all baseline failures and scientific NR. Further headline jobs must wait until
this last diagnostic dependency is terminal.
