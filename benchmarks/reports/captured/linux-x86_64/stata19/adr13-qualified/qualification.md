# ADR-0013 Linux/SCC qualification

This directory is the durable qualification evidence for ADR 0013. It
preserves two exact-source runs because the first run established the full
scientific matrix and exposed a portable-x86 performance defect, while the
second exact-source run changed only the defective arithmetic kernel and
retested the affected performance surface.

## Full twelve-scope scientific run

- Run root: `/projectnb/welfgr/fereg/runs/20260829T111000Z-adr13-focused-93292b9`
- Source: `93292b94f84aee7fced05dcd6985037765dfeb3f`
- Source archive SHA-256:
  `e299029e749e26842f7cdbede41a4a09358f8de68926c1679651db962e6139f3`
- Linux plugin SHA-256:
  `b1f98f7b735487555f8695287afaaf18a87bfd6911275ede54a7d6fc3db41ad5`
- Setup `7359551`, smoke `7359554.1`, array `7359555.2-12`, and collector
  `7359557` all have complete SGE accounting with `failed=0` and
  `exit_status=0`.
- All twelve shards have strict `SUCCESS` receipts, exactly 36 timing rows and
  one passing scientific preflight apiece, zero command failures, zero science
  failures, and the requested fereg thread count. The merged inventory has 432
  timing rows and 12 preflights.
- The matrix covers one, two, and three fixed-effect dimensions; 0, 10, and 30
  controls; standard and low-mobility graphs; fit and saved-FE modes; and one
  and sixteen requested fereg threads. Its only arms are fereg, Greg, and
  reghdfe.

That run failed the performance gate. Portable x86-64 lowered every
`f64::mul_add` in the dominant Gram kernel to a software `fma()` call. The
one-FE/K=30/one-thread fit therefore took 104.278 seconds, and the standard
two-FE/K=30/one-thread fit took 109.292 seconds. The failure is retained in
`pre-fma-full-12/`; none of those timings is claimed as production
performance.

## Portable-x86 repair run

- Run root: `/projectnb/welfgr/fereg/runs/20260829T144057Z-adr13-fma-repair-947f793`
- Source: `947f79373306760a635494935cc84c0f5ebfcda8`
- Source archive SHA-256:
  `11466f6fb15f173fc7b70dfbb8599fbf03f845444b56b9b312f18426dc8af914`
- Linux plugin SHA-256:
  `94017d23d97c1a0890cd9a1a180dad4ce3482b5b29cbe6efdbece5016b3e7099`
- Setup `7362157`, smoke `7362158.1`, and array `7362162.2-5` all have
  complete SGE accounting with `failed=0` and `exit_status=0`.
- All five focused shards have strict `SUCCESS` receipts, exactly 36 timing
  rows and one passing scientific preflight apiece, zero command failures,
  zero science failures, and the requested fereg thread count. The inventory
  has 180 timing rows and five preflights, with only fereg, Greg, and reghdfe.
- The focused matrix covers the changed kernel at one-FE/K=10, one-FE/K=30,
  and standard two-FE/K=30; fit and saved-FE modes; and one and sixteen
  requested fereg threads. Every shard ran on a 32-processor SCC node.

The portable repair removed the catastrophic regression. The repaired
one-FE/K=30/one-thread fit is 3.210 seconds, 3.1 percent of the failed build's
time. The repaired standard two-FE/K=30/one-thread fit is 5.080 seconds, 4.6
percent of the failed build's time. The same-shard performance results are:

| Design | K | Threads | Mode | fereg | Greg | reghdfe | f/Greg | f/reghdfe |
|---|---:|---:|---|---:|---:|---:|---:|---:|
| one FE | 10 | 1 | fit | 1.438 | 0.413 | 2.204 | 3.482 | 0.652 |
| one FE | 10 | 1 | save | 1.910 | 0.607 | 2.749 | 3.147 | 0.695 |
| one FE | 30 | 1 | fit | 3.210 | 1.257 | 8.245 | 2.554 | 0.389 |
| one FE | 30 | 1 | save | 4.026 | 1.529 | 8.697 | 2.633 | 0.463 |
| one FE | 30 | 16 | fit | 1.424 | 1.135 | 4.967 | 1.255 | 0.287 |
| one FE | 30 | 16 | save | 2.002 | 1.417 | 5.438 | 1.413 | 0.368 |
| worker-firm | 30 | 1 | fit | 5.080 | 5.637 | 25.424 | 0.901 | 0.200 |
| worker-firm | 30 | 1 | save | 5.939 | 7.160 | 29.473 | 0.829 | 0.202 |
| worker-firm | 30 | 16 | fit | 2.129 | 5.791 | 11.958 | 0.368 | 0.178 |
| worker-firm | 30 | 16 | save | 2.448 | 5.941 | 12.474 | 0.412 | 0.196 |

Across these five focused fit cells, the geometric-mean fereg ratio is 1.299
against Greg and 0.304 against reghdfe. For saved output it is 1.320 against
Greg and 0.342 against reghdfe. Fereg beats reghdfe in every focused cell and
beats Greg in all four standard two-FE fit/save cells. The remaining gap is
concentrated in the single-FE, single-thread path; at sixteen threads the
one-FE/K=30 fit gap to Greg is 25.5 percent.

Cross-run before/after absolute times are diagnostic because SCC nodes can
differ. They are decisive here only because the regression was one to two
orders of magnitude and the focused repaired run places every shard on a
32-processor node. Comparator claims use same-shard ratios only. Detailed
timings, preflights, environments, identities, and accounting are in
`fma-repair-5/`.

## Acceptance

ADR 0013 is accepted on the combination of the full twelve-scope scientific
run and the exact-source five-scope repair qualification. The arithmetic
repair changed only the guarded Gram accumulation; the unchanged independent
score and bread certificates, Cholesky rejection guard, and QR fallback remain
the fail-closed authority. The next optimization target is the one-FE,
single-thread boundary and publication path, not multiway finite solving.
