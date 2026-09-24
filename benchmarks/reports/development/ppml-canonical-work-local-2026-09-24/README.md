# Observed-work canonical PPML admission — local development, 2026-09-24

Adopt the bounded routing and ownership change in
[ADR 0171](../../../../docs/adr/0171-observed-work-canonical-ppml-routing.md).
Fresh disconnected targets improve about 5% and 9%; a narrow connected target
improves about 2%. These are useful incremental gains from the existing fused
solver, not closure of the broad <=0.90× fixest goal. Initial solving and ordinary
joint iterations still dominate enough work to leave important losses.

## Exact source and scope

- Baseline: accepted **canonicalcomponents2**, plugin SHA-256
  55639c6687a26b2ad8f2edd6cc248d26a6baa764c38a13b9a936d41e996896c3.
  Its source-bound component report remains authoritative for earlier claims.
- Candidate: **canonicalwork1**, plugin SHA-256
  ae8859fd082e6aa6481ab814c893d5ff119a5c0ee2ff15350577002c893b5579.
  Captured from b78202b6a849b74b0b80235675273517609fee8b plus only
  fe/borrowed_map.rs, ols/canonical_products.rs and ols/linear_session.rs.
  All **548 captured inputs match** final source; no diagnostic instrumentation
  is present in the public candidate.
- Stock: pinned, unmodified fixest 0.14.2; source archive SHA-256
  17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9.
  The exact reference identity in each plan is authoritative. Comparable public
  Poisson calls use glm.tol=1e-10, fixef.tol=1e-11, robust inference and fitted
  means. Default-reference discrepancies remain separate historical evidence.
- Local Mac development only, requested T1/T4/T8. Low mode unless stated.
  Positive unweighted numeric PPML comparisons, identical prepared data/sample
  and requested outputs. This is not qualification or a frequency-weight oracle.
- Development seed 202609347; fresh seeds **202609383 and 202609384** were
  declared after candidate capture and before use and are now consumed/frozen.
  The 77/16/111 frozen definitions and previous validation seeds are unchanged.

The corrected process monitor is separately captured, SHA-256
1c7fe37c714ba2516869b3e42b6d33d639792865152d42b348a75b1959e52686.
All public timing calls have no detected overlaps or monitor errors. No
compilation, profiling, archiving, oracle audit or competing fit ran alongside
headline measurements. Two launch reviews reported already-completed processes;
fresh process checks confirmed their exit before the same declared commands
were retried. Neither rejected launch started a measurement. The unrelated
Stata GUI was left untouched and its CPU activity monitored. Ambient OS variation
remains a limitation.

## Identical-weight diagnosis

The isolated b782-based probe compares the accepted reusable PCG projection plus
separate scratch Gram against the existing fused direct product engine. Only the
diagnostic direct factor-work refusal is bypassed; full byte admission, factors,
rank safeguards and independent original-input certificates remain. This is a
comparison of two native kernels, not a new instrumented fixest inner-solve
measurement or a headline public speed claim.

The four frozen frames use two development designs, N21120/K2 and disconnected
N42240/K10. Frame0 takes final accepted public means as weights and recomputes
the Poisson working response. Frame1 uses exp(log(mu)+0.4*x1), with its working
response recomputed; it is a model-space perturbation, not a captured actual
IRLS iteration. Both kernels see identical bytes. Seven alternating pairs at
T1/T4/T8 give 168 calls. Immutable baseline-owner setup is recorded separately;
candidate time includes topology, weighted setup, factors, all RHS, full
certificate/products and the scratch write. Repeated same-frame calls may favor
baseline cache reuse.

Whole-product medians at the representative public thread counts, milliseconds:

| Frame | PCG + separate products | Fused direct | Direct setup | Direct solve | Direct certificate |
|---|---:|---:|---:|---:|---:|
| c0-f0 / T4 | 3.305 | 1.286 | 0.482 | 0.207 | 0.125 |
| c0-f1 / T4 | 3.458 | 1.210 | 0.486 | 0.210 | 0.108 |
| c1-f0 / T8 | 12.236 | 7.027 | 3.542 | 1.695 | 0.691 |
| c1-f1 / T8 | 12.604 | 7.121 | 3.512 | 1.751 | 0.654 |


Across thread counts and frames, direct whole-product cost is 29–39% of baseline
on the connected case and 56–73% on the disconnected case when baseline-owner
setup is charged. The probe records 19/20 operator applications and 29/30
evaluated RHS lanes on the connected frames, versus 24/25 and 132/142 on the
disconnected frames. Every baseline call has two initial and two full projector
certificates. Its legacy certificate-seconds field is zero despite those calls;
zero must not be interpreted as free verification. All **24 explicit sparse
WLS projection/Gram oracle configurations pass**, including changed weights.
The full distributions, operation counters, input hashes and phase timings are
in probe/summary.json, probe/oracle.json and the source-bound probe receipts.

## Adopted work organization

Keep cold finite-Gram work admission. A later call can also spend the actual
two-pass Schur operator work observed during its previous completed canonical
projection/rank stage. The hint depends on the immutable prepared model, not
benchmark names, seeds or wall-clock noise. It is performance metadata only:
every weight frame still rebuilds geometry and independently certifies results.

The complete topology/work/memory plan may consume an obsolete temporary
projection block when it needs that block's existing reservation. Earlier work
or memory refusal preserves the block; a later numerical refusal can rebuild it
through the normal fallible fallback. Retained scratch is untouched until
certification. No extra data-sized cache or larger memory budget is introduced.
The same fused grounded component-factor engine serves response and controls.
No tolerance, sample, rank policy, weight semantics, inference, DoF, separation
or publication guarantee changed.

## Public distributions and losses

The initial current-source baseline had medians 52/132 ms against stock 30/78.
A new three-variant development rotation measured 52→49 ms against stock 31,
and 131→122 ms against stock 78. All six native pairs improve; direct
candidate/stock comparisons and public states pass. Fresh confirmation below
uses five baseline/candidate/stock rotations per case. Entries are median
[min–max]; timings include the whole public command.

| Fresh case | Baseline ms | Candidate ms | Stock ms | C/stock medians | Paired C/B range; median |
|---|---:|---:|---:|---:|---:|
| Connected narrow N22320/K2/T4 | 53 [53–55] | 52 [51–55] | 31 [29–32] | 1.677 | 0.945–1.038; 0.964 |
| Disconnected narrow N43200/K10/T8 | 133 [132–136] | 126 [124–128] | 80 [75–84] | 1.575 | 0.912–0.970; 0.947 |
| Connected K6 (already admitted) N26400/K6/T4 | 63 [62–66] | 63 [61–63] | 49 [47–50] | 1.286 | 0.955–1.016; 0.984 |
| Disconnected serial N19680/K3/T1 | 69 [68–71] | 63 [61–66] | 54 [53–55] | 1.167 | 0.884–0.956; 0.926 |
| Unbalanced control N23760/K16/T4 | 83 [82–84] | 84 [83–87] | 73 [70–75] | 1.151 | 1.000–1.036; 1.012 |
| Wide memory refusal N22320/K100/T8 | 533 [524–536] | 531 [525–537] | 447 [444–448] | 1.188 | 0.996–1.006; 1.002 |
| Fast control N21600/K6/T4 | 65 [63–66] | 64 [63–73] | 43 [40–45] | 1.488 | 0.969–1.123; 1.000 |
| Two slope partitions N22320/K10/T4 | 108 [107–111] | 110 [108–110] | 114 [109–115] | 0.965 | 0.973–1.028; 1.019 |
| Three FE N23760/K10/T4 | 115 [115–117] | 115 [114–116] | 273 [270–276] | 0.421 | 0.974–1.009; 1.000 |
| Weak barbell N12240/K8/T1 | 51 [48–53] | 50 [49–56] | 1873 [1863–1897] | 0.027 | 0.943–1.098; 1.000 |
| Wide admitted control N31920/K76/T8 | 321 [320–333] | 320 [318–325] | 440 [431–458] | 0.727 | 0.961–1.016; 0.991 |


The disconnected serial median improves 8.7%, but its paired median gain is
7.4%; the narrow connected paired median gain is 3.6% despite a 1.9% ratio of
medians. These distinctions and all losses remain visible. The connected K6
case was already admitted and is a useful control rather than a new route win.
Three of these 11 selected medians meet the 10%-faster-than-stock target; seven
still lose to stock. This is not a coverage-wide win rate. The fast control has
a candidate outlier at 73 ms and a paired median ratio of 1.000. Unbalanced and
slope controls lose 1 and 2 ms at their medians. No extra tuning followed those
fresh results.

All 165 fresh states, 55 native pairs, 55 baseline/stock pairs and 55 direct
candidate/stock pairs pass. Two small explicit-design Poisson/Hessian/sandwich
cases independently pass for all six baseline/candidate/stock fits. Their
one-rotation runtimes remain descriptive: 23→24 versus 21 ms connected, and
30→30 versus 31 disconnected; they are scientific checks, not timing confirmation.

Selected public native phase medians, milliseconds:

| Case | Preparation B→C | Projection B→C | Finite B→C | Strict final B→C | Inference B→C | Publication B→C |
|---|---:|---:|---:|---:|---:|---:|
| Connected narrow | 5.71→5.81 | 7.99→8.35 | 3.88→3.97 | 11.03→8.14 | 0.30→0.41 | 3.95→4.02 |
| Disconnected narrow | 14.32→14.37 | 31.31→31.33 | 10.26→10.50 | 35.22→27.10 | 1.21→1.17 | 8.12→8.28 |
| Disconnected serial | 5.97→5.50 | 13.71→13.04 | 5.09→4.68 | 15.73→11.24 | 0.43→0.43 | 3.49→3.53 |
| Wide memory refusal | 34.98→34.94 | 118.82→119.43 | 58.25→57.96 | 154.87→152.80 | 35.58→35.56 | 27.13→27.15 |
| Fast control | 5.65→5.75 | 16.71→16.16 | 4.26→4.18 | 10.90→11.33 | 0.37→0.37 | 3.60→3.69 |


Strict-final work accounts for the targeted gain. The cold solve does not yet
have observed work, and ordinary joint iterations use their established engine.
The frozen-kernel improvement therefore cannot multiply the whole public fit.
All targeted fits still take four outer iterations.

## Memory, tests and boundaries

Tracked peaks fall by 356,973 / 1,775,392 / 302,072 bytes on fresh cases 0/1/3.
Most excluded paths add 24 bytes of fixed owner metadata; the slope peak is
unchanged. Scratch sizes are unchanged throughout. Maximum fresh tracked/model
ratio is 0.85628. These are tracked fereg allocations, not measured total RSS.
Counter, session hint and rollback hint are fixed-size scalars; the shared
operator performs one integer counter update per application, without changing
OLS numerical arithmetic or adding an observation-sized allocation.

**32 distinct focused core tests** pass: seven canonical-product tests, nineteen
Schur tests, and six strict-IRLS tests. Tests cover independent weighted
contrasts, connected/disconnected graphs, scale/rank boundaries, current-weight
refresh, strict rollback, OLS routes, overflow refusal and exact ownership
reclamation. Five licensed fixtures pass with explicit Stata return codes and
terminal sentinels, including canonical products, generated terms, categorical
joint PPML, multipartition slopes and three-FE OLS. No broad qualification ran.

All 24 OLS calls and 12 science/output pairs pass. Medians B→C are 17→17 ms
(one FE/all outputs), 29→30 (two FE), 104→104 (low slopes), 63→63 (low three FE).
Raw per-call distributions and comparisons remain in ols-audit.json/summary.json.

Preserved failures and corrections:

- The first counter wrongly used the disconnected-component scaling branch and
  missed connected graphs. The original test failure and source are retained.
  Actual operator counting fixes that boundary.
- The next test expected a new direct solve after rollback despite an already
  current-weight retained design. The existing exact-weight reuse correctly
  avoided it. The final test advances the weight generation before each strict
  attempt; it still tests drop/commit, accuracy and reclamation.
- The pre-existing initial_full_width_lsmr_preserves_canonical_multiway_rank_policy
  assertion fails at fallback_used=false versus used_solver=Map on both the
  b782 diagnostic snapshot (reporting code unchanged) and candidate. It is not
  hidden, relaxed, or claimed repaired. The broad initial canonical filter has
  28 passes and two failures; focused final evidence is the 32 tests above.
- Diagnostic build1 had an unsupported Clone call. Builds2/3 lacked a fixture;
  the first archive repair used a Python without tarfile filter support. The
  bundled runtime and complete fixture capture fixed build4. All failed logs,
  source hashes and the final diagnostic binary hash are preserved.
- final-source-check.json initially used a nonexistent revision key for base;
  the v2 receipt corrects that metadata field without altering any hashes.
- Static allocation inventory drift, weighted constrained-fit failures,
  insufficient-budget/nonretaining-CG span instability, SCC authentication,
  platform qualification and the prior seed378 comparator discrepancy remain
  open. No stopped campaign was restarted.

## Next bounded implementation milestone

Move to **wide certificate workspace**. The fresh N22320/K100/T8 low case
still takes 531 ms against 447 ms stock. Projection and strict-final phases cost
119.4 and 152.8 ms; inference is 35.6 ms. Direct-product admission still refuses
the certificate's left-group coefficient bank and worker-private Gram/FE
reductions. Test streaming/recomputing left-group coefficients by bounded tiles,
or a separately bounded certificate reduction team, inside the existing memory
reservation. Keep shared current-weight factors and the independent full
original-input certificate. This is a concrete opportunity to change the work
organization and potentially cross the stock target.

Narrow PPML remains a larger public loss, particularly fast execution. Revisit
cold factor selection and ordinary joint-iteration organization only as a
separate measured redesign; do not spend another cycle tuning this hint.
Preparation/publication and finite solving are also material. No further
relaxation of the work allowance or scientific tolerances is implied.

## Receipts

Use restore_receipts.py with a new destination directory. The content-addressed
archive contains declarations, exact artifact/source identities, controllers,
timing/science exports, independent oracle results, probe source, test logs,
failure evidence, and sanitized fixture protocols. Licensed logs, binaries and
raw generated data stay local; their hashes and regeneration recipes are kept.
Historical archives are unchanged. The unmodified custom generated-term fixture
is also supplied as generated-products-oracle.do; its hash is in the verification
receipt. It exercises FW multiplicity and PW cluster checks independently of
the unweighted stock timing comparisons.

Archive: **7,577 logical receipts / 2,005 payloads**, SHA-256
026080e6cdc116a630bed12a1b8316a968a2be3fd14f3c715fa73f3bb5fe2ab8. All are verified and restored;
see archive-verification.json.
