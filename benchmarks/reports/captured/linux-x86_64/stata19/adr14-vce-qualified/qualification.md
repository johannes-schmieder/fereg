# ADR-0014 Linux/SCC VCE qualification

This directory preserves the three exact-source stages used to qualify the
certified VCE hot-reduction work.  The full data files are intentionally
compact merged CSVs, identities, receipts, and reports; large datasets and
intermediate work remain on SCC.

## Stage 1: broad 34-scope surface

- Run root: `/projectnb/welfgr/fereg/runs/20260829T190950Z-vce-9789f84-a2`
- Source: `9789f8463b8d2b8f635d228a3db9ad8b265eb1f4`
- Archive SHA-256:
  `95b6f82edbb124aa8eb9a267e76c317485188298d4d7acec39855112eb5e6016`
- Linux plugin SHA-256:
  `1d5b3cd695db9a1e8031f7719f8ef0853e4d455d1db39e43452b65857fa8ba9a`
- Coverage: IID, robust, cluster 1--5; analytic/probability weights; N=1m/K=10
  and N=10m/K=30; one/sixteen native threads.
- Result: 34 strict shards, 34 green preflights, 264 timing rows, zero command
  or science failures, and only fereg/Greg/reghdfe arms.

The original benchmark wrappers published atomic success shards and then hit a
post-publication cleanup defect.  Validation array `7366246.1-34` and repaired
collector `7366247` completed with `failed=0`/`exit_status=0`; no benchmark was
rerun.  Evidence is in `full-34/`.

## Stage 2: rejected transposed-certificate repair

- Run root: `/projectnb/welfgr/fereg/runs/20260829T225038Z-vce-repair-a8fbad2`
- Source: `a8fbad2d26e67bcfd575cf8d44534d0d0df0d72f`
- Archive SHA-256:
  `b5653aa728d3daef9b5d1c46f2d062705203c05a6284fa0b857e282c364c86bf`
- Linux plugin SHA-256:
  `4e8ce56c646a1eb0fdfc21fec49ff7db56c435d1f72888cc0ef3c80bd56d533d`
- Jobs: setup `7366384`, array `7366385.1-13`, collector `7366387`.
- Result: complete successful accounting, 13 strict shards, 13 green
  preflights, 108 timing rows, and zero command/science failures.

This stage confirmed the bread-hoisting science and strong weighted/multiway
results, but its transposed certificate triangle caused a material K=30 cache
regression.  It was not accepted or pushed.  Evidence is retained in
`transposed-repair-13/` because it explains the final repair and preserves the
multiway/weight boundary checks.

## Stage 3: accepted contiguous-certificate repair

- Run root: `/projectnb/welfgr/fereg/runs/20260830T012900Z-vce-cachefix-490e03a`
- Source: `490e03a3e2458d8bc463c3f60389062e2f8fae42`
- Archive SHA-256:
  `4f4e44cc539419ae67043cfca824c92aed7c6b1969c1a37825939e2397d02aab`
- Linux plugin SHA-256:
  `9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2`
- Jobs: setup `7367526`, array `7367527.1-8`, collector `7367528`.
- Result: every accounting record has `failed=0`/`exit_status=0`; eight strict
  shards, eight green preflights, exactly 72 timing rows, zero command/science
  failures, exact thread requests, and only fereg/Greg/reghdfe.

The final stage covers N=1m/10m, K=30, robust/one-way clustered VCE, and
one/sixteen native threads.  Detailed same-shard medians and cross-stage
diagnostics are in `contiguous-repair-8/report.md`; raw merged evidence and
task/setup/collector receipts are alongside it.

## Acceptance

ADR 0014 is accepted on the combined stages.  The 34-scope matrix supplies the
broad VCE, weight, scale, and thread science surface.  The 13-scope stage
requalifies bread hoisting and documents the rejected layout.  The final
eight-scope stage directly retests every changed robust/cluster-one K=30 hot
boundary.  Same-shard final ratios show fereg faster than reghdfe in 8/8 cells
and Greg in all 4/4 sixteen-thread cells.  No areg arm appears anywhere.
