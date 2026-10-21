# CMG setup attribution (diagnostic only)

`bench/cmg_setup_attribution.py` instruments private source copies of the
accepted (fereg `3f9d54ee` / CMG `35a095f3`) and candidate (fereg `0407060c`
/ CMG `355d328a`) setup arms. It separates two explanations of the
candidate's broader hierarchy phase. Production sources and dependency pins
are never edited, and no output is timing or activation evidence.

## What is instrumented

The tool exports the exact commits with `git archive` and verifies the
pristine SHA-256 of every edited file. It applies root's unchanged
`cmg_setup_profile.patch` (SHA-256 pinned), then the exact-anchor edits in the
tool; each anchor must occur once. Finally it redirects the copy's CMG
dependency to the patched copy in the manifest and the lockfile, and builds
remain `--locked`. Everything else is bound to the result:

- `attribution.diff`, which excludes root's profile patch;
- a deterministic `source.tar` of both patched trees;
- `attribution-source.json`.

One added module, `cmg/src/setup_attribution.rs`, holds process-global
scalar storage: fixed atomic tables and per-thread counter cells. There is no
admitted bank, extra factor walk, alternative constructor, pool or graph
copy. The applier sets one compile-time mode:

- **counts**: operation counters only, no timers. Counter-build wall time is
  never evidence.
- **timers**: scalar `Instant` timers only, no counters.

### Canonical raw-edge sort records

There is one record per `Laplacian::canonical` / `canonical_parallel` /
hosted-initial call, in call order, with these fields:

- `site`: `contraction` (with its hierarchy `level`) or `outside` (the initial
  graph).
- `path`: `serial`, `team` or `hosted`.
- `n`: input records; `out`: canonical edges after duplicate reduction.
- `sort_ns` (sort only) and `canonical_ns` (sort plus duplicate reduction):
  timers build only.

In the counts build, only `serial` sorts are counted. Counting happens only
on the calling thread while that sort runs, so prepared and LDL heapsorts and
team lanes are never counted. The counters are:

- `cmp`: calls of the canonical comparator, including the 3-record
  median-of-three sample.
- `partitions`, `partition_steps` (one per record classified) and
  `partition_swaps` (`slice::swap` calls).
- `insertion_leaves`, `insertion_records` and `insertion_swaps` (adjacent
  swaps).
- `heapsorts` and `heapsort_records`: the whole accepted sort, or the
  candidate's depth fallbacks.
- `heap_sift_steps` (sift-loop iterations) and `heap_swaps` (sift and
  extraction swaps).
- `meter_units` (units counted by the caller's `CancelMeter`) and `queries`
  (its cancellation polls).

The meter's cadence and arithmetic are unchanged. The validator checks
`meter_units // 66559 <= queries <= meter_units // 65536 + 1`. Record moves
are `2 x swaps`, plus three pivot-sample copies per partition.

### Timers

- Every existing `measure_hierarchy_phase` boundary, per level.
- `coarse_isolate_pruning`.
- `hierarchy_total`.
- `finest_components`, `coarse_centering` and `terminal_factor`.
- `finalize_total` and `build_with_total`.

Timers nest:

- `build_with_total` contains `hierarchy_total` and `finalize_total`.
- A contraction phase contains its `canonical_ns`, which contains `sort_ns`.

`validate_call` checks this nesting (1 µs slack). It reports `exclusive_ms`:
disjoint pieces whose sum is exactly the fereg profile's `hierarchy_seconds`.
Never add the inclusive timers.

## Usage (root executes; no timing in the cloud)

```sh
python3 bench/cmg_setup_attribution.py prepare --arm accepted|candidate \
    --mode counts|timers --fereg-git FEREG --cmg-git CMG --out ATTR
python3 bench/cmg_setup_attribution.py smoke ATTR --cmg-git CMG   # optional pre-check
(cd ATTR/fereg && ./scripts/build_linux.sh)   # or build_macos.sh; caller captures the log
python3 bench/cmg_setup_attribution.py bind ATTR --plugin PLUGIN --build-log LOG \
    --baseline-sha256 ARM_BASELINE_PLUGIN_SHA256
python3 bench/cmg_setup_attribution.py run REPOSITORY TASK_ROOT OUTPUT \
    --arm accepted_hidden|setup_hidden --revision FEREG_REVISION \
    --source ATTR/profile-source.json --attribution ATTR/attribution-source.json \
    --plugin PLUGIN --plan PLAN --baseline COMPLETED_RESULTS
```

`smoke` builds a small harness against the pristine and the prepared CMG
copy. It requires identical hierarchy, repeat, factor and apply-bit digests at
W1 and W4, reaches a direct terminal on one of its two graphs, and validates
the real attribution lines.

`run` binds everything through `cmg_setup_profile.load_context`: plan,
frozen inputs, completed baseline, plugin, root profile patch and source
binding. It also binds the attribution source, tool and module. It then runs
the fixed 18 calls: high-degree-weak and weak-low at W1/W4/W8, one warmup and
two recorded calls each. It keeps the exact science fingerprint, route and
hierarchy checks, and requires per call:

- exactly one built six-phase profile line and one attribution line;
- no overflow or unknown phase;
- arm identities (accepted: one heapsort of every serial input; candidate:
  partitions);
- per-level contraction records that match the contraction timers;
- nested-timer consistency.

Within a case, every call must have identical canonical sorts, and in the
counts build identical counters.

## What it can and cannot distinguish

- **Can (counts):** exact, input-determined operation counts of every serial
  canonical sort per contraction level in both arms. These include
  comparisons, swaps, partition and insertion work, heapsort fallbacks and
  meter queries. Counts show whether the candidate does more or fewer
  primitive operations, and where.
- **Cannot (counts):** the cost of an operation (memory traffic, branch
  prediction, cache behaviour), or anything on team lanes. The hosted initial
  sort is recorded by size only.
- **Can (timers):** split the fereg hierarchy phase into disjoint pieces:
  - coarse sort, duplicate reduction and mapping;
  - forest; aggregation; pruning; level finalization; bookkeeping;
  - finest components, coarse centering and the terminal factor;
  - finalize/build/admission remainders.

  Accepted-versus-candidate differences can then be attributed descriptively.
- **Cannot (timers):** establish causality or a kernel cost. Their overhead is
  small but nonzero (about 2 timer reads per phase per level). They do not
  split work inside team lanes. The hosted initial sort is timed only, and it
  lies outside the hierarchy phase (fereg's initial-Laplacian phase).
