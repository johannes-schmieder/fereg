# SCC benchmark tooling

For the second program, pass `--manifest bench/program-v1.json --engine next`
to `prepare.py` and select `development-scc` or `holdout-scc`. Preparation reads
the manifest from the exact committed source and records engine policy; program
tasks keep a complete native-thread ladder on one whole node. The original
tiers retain their legacy default. The program collector permits an 8 MiB raw
scalar summary in an ignored collection directory; commit compact per-family
evidence rather than the entire matrix. These extensions require a real compute
node smoke before M29 scaling acceptance; local syntax/tests alone do not qualify SCC.

The scripts use `/projectnb/welfgr/fereg/dev/<id>` for smoke and
`/projectnb/welfgr/fereg/runs/<id>` for standard/full runs. Inspect current
quota and jobs before deploying. Source archives contain an explicit allowlist
from a Git commit, including the source-pinned reference bundles. Existing
runs, environments and data are never overlaid or deleted.
Run roots are owner-only. The submitted controllers are extracted from the
verified commit archive, so uncommitted local edits cannot enter a remote run.

Prepare comparator archives with `bench/setup.py` locally, then prepare inputs
from a commit that contains these scripts:

```sh
python3 bench/scc/prepare.py --tier smoke --revision HEAD \
  --environment /tmp/fereg-bench-env --root /tmp/fereg-scc-input
bash bench/scc/submit.sh RUN_ID smoke /tmp/fereg-scc-input
```

Submission creates one setup job and an SGE-dependent benchmark array. Smoke
is one task; standard/full are partitioned by cell and native-thread count.
Both stages reserve 28 slots on E5-2680v4, 3 GB per core. Setup/smoke request two
hours; full array tasks request at most 22 hours. No host or queue is forced.
The benchmark caps Stata/MP at four processors and native/R threads at the
manifest value. BLAS pools remain at one thread.

`setup.sh` verifies transfer hashes, extracts the commit snapshot, builds the
Linux plugin with isolated Cargo/Rust 1.85.1 and offline Cargo resolution, and
builds the pinned comparators under the run root. Uploaded fixest/ppmlhdfe
archives avoid acquisition during the compute job; the tested module stack
provides their dependency packages. A missing dependency or archive is a setup
failure to investigate before retrying. A held benchmark verifies setup success
itself because SGE dependencies alone do not imply success.

The command stopwatch build loads `llvm/16.0.6` in an isolated subshell. Setup
then uses the existing runner for one licensed OLS clock smoke outside that
compiler environment and requires complete identities and five successful
measurements before its completion signal. This qualifies clock operation,
not SCC timing calibration or the program's performance envelope.

Full runs require the external prepared Veneto DTA path via `--veneto-data` to
`prepare.py`. Confirm the data's existing project location and classification
before supplying it. Do not upload or commit private data to make a cell pass.

Inspect `jobs.tsv`, `qstat`, the bounded job logs and `qacct` after completion.
The collector requires clean accounting for every expected task, 28 reserved
slots, application completion signals, source identity, and every declared
cell/thread/comparator group. Smoke additionally requires all accuracy checks.
For baseline full runs, explicit command failures/timeouts remain in the CSV.
Raw datasets and Stata logs remain on SCC.

```sh
bash bench/scc/collect.sh /projectnb/welfgr/fereg/dev/RUN_ID \
  /tmp/fereg-scc-collected bench/history/DATE-SHA-linux-smoke.csv
```

Set `FEREG_PYTHON_BIN` for collection if `python3` is older than 3.11. Collection
uses a new local directory and a new output filename; it never rewrites an
accepted summary. If accounting has not appeared yet, wait and collect into a
new diagnostic directory. Monitor short smoke jobs directly without a heartbeat.
