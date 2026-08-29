# ADR-0012 focused SCC qualification

This directory is the durable evidence for the focused Linux/SCC qualification
of exact source `6be8f067f5c54e46e71e470b55beda7eb30a0ea1`. It is not the
exhaustive 116-scope release matrix.

## Identity and gates

- Run root: `/projectnb/welfgr/fereg/runs/20260829T043755Z-adr12-focused-6be8f06-a5`
- Source archive SHA-256: `913584f55669b636bf30a72a37543260e485b776866cf85daf1bfd9da67a79e5`
- Linux plugin SHA-256: `60d4b6525aedbb3bfbcfeb9f14d6b1b994545825f03f4fb2461cbfc352ee45a7`
- Setup `7357075`, smoke `7357076.1`, array `7357077.2-12`, and final
  collector `7358415` all have complete SGE accounting with `failed=0` and
  `exit_status=0`.
- All twelve shards have strict `SUCCESS` receipts, exactly 36 timing rows and
  one scientific preflight apiece, zero command failures, zero scientific
  failures, and the requested fereg native thread count.
- The merged result has 432 timing rows and 12 preflights. Its only command
  arms are fereg, Greg, and reghdfe; areg is absent.
- The twelve scopes cover one, two, and three fixed-effect dimensions; 0, 10,
  and 30 controls; standard and low-mobility graphs; fit and saved-FE modes;
  and requested native threads 1 and 16.

The SCC scheduler placed shards on heterogeneous 16-, 20-, and 32-processor
nodes. `shard_environments.json` preserves the original environment of every
shard. The focused collector normalized `processors_mach` only in a temporary
merger view and compares commands only within a shard.

## Performance interpretation

Against reghdfe, the focused candidate's geometric-mean fit ratio is 1.574 at
one requested thread and 0.916 at sixteen; saved-FE ratios are 1.471 and 0.839.
Thus fereg is faster on 7 of the 12 sixteen-thread fit/save comparisons. The
strongest multiway sixteen-thread fit cells are low-mobility worker-firm-year
(0.260 times reghdfe), standard worker-firm (0.647), and standard
worker-firm-year (0.650).

For the K=30 target population matched to the frozen pre-ADR baseline, the
geometric mean of the within-shard fereg/comparator ratio changed by a factor
of 0.672 against reghdfe and 0.777 against Greg. The primary one-FE/K=30,
one-thread fit moved from 212.063 to 7.381 seconds; its ratio to reghdfe moved
from 15.762 to 0.480. These absolute before/after times are cross-node and are
therefore secondary to same-run comparator ratios and the same-host macOS
prototype in ADR 0012.

Heterogeneous nodes create conspicuous cell noise. In particular, the SCC
one-FE/K=10 one-thread shard recorded 17.378 seconds, while a post-collection
same-source macOS rerun on the identical benchmark specification recorded a
1.010-second warm median, 0.391 seconds native, and `science_fail=0`. Similarly,
the SCC one-FE/K=30 sixteen-thread shard is not a credible paired scaling
estimate because its one- and sixteen-thread runs landed on different node
families. These cells are retained rather than filtered; no performance
observation was discarded.

Saved-FE publication remains a small increment on the hard multiway cells:
5.9--9.2 percent in this focused run. Across all twelve scopes, including the
boundary-dominated zero-control one-FE cells, the geometric-mean save/fit ratio
is 1.147.

`adr12-baseline-comparison.json` contains every matched cell and the aggregate
ratio changes. Absolute times are retained there with an explicit cross-node
caveat. `report.md` and `evidence.json` are the collector's unedited focused
outputs.

## Operational provenance

Attempts 1--4 failed before scientific benchmarking because the exact archive
needed ephemeral Git metadata, direct invocation of non-package Python tests,
and the pinned CMG revision in the SCC offline Cargo cache. Their evidence is
preserved at their SCC run roots. In attempt 5, all benchmark shards succeeded.
Collector `7357078` then failed because its wrapper omitted the source root
from `PYTHONPATH`; collector `7358073` correctly rejected a partial manifest
and heterogeneous `processors_mach`. Their evidence remains under the run
root's `failure-evidence/` directory. Collector `7358415` changed only the
collection view, preserved every shard environment, and passed. No benchmark
scope was rerun for a collector failure.
