# Rust-owned input pipeline adoption

The Mac input pipeline overlaps main-thread numeric reads with one transpose
and missing-classification job on the existing bounded Rust team. Two admitted
4,096-row vectors replace the earlier C prototype's unaccounted buffers and
independent pthread. The calling thread alone holds the SPI session. Both
peers signal closure on error or unwind; scoped completion joins the worker
before returning. Unsupported shapes, one-thread calls and insufficient
optional scratch use the existing column reader before any tiled read.

Eligibility is unrestricted IID OLS with unit weights, one ordinary categorical
effect, numeric sources, at least 16,384 observations and 16–64 input columns.
Scratch is fallibly reserved, its actual capacity admitted, and its bytes included
in the input peak. The ledger explicitly retains the existing worker-runtime
coverage gap in the loading phase; this does not claim a bound on opaque Rayon
or native-thread allocations. No numerical or sample calculation is replaced.

The isolated candidate starts at `5ac9e9c5` plus the source-hashed overlays in
[identities.json](identities.json). Its arm64 plugin is
`889386e2feb59d5ecb9a63e0a15afd89903279dd3c02aa9df5e3270f5d8e3d17`.
It uses a separate Cargo target directory and the same release settings as
the accepted accessor baseline. Three rotated primary blocks give:

| N | K | Threads | Accessor median | Pipeline median | Pinned fixest median |
|---:|---:|---:|---:|---:|---:|
| 983,040 | 10 | 16 | 215 ms | 215 ms | 137 ms |
| 983,040 | 50 | 16 | 528 ms | 506 ms | 447 ms |

The wide command improves 4.2%. Native phase tracing was enabled for these
primary blocks: median wide loading falls from 273.2 to 240.4 ms; native time
after loading rises from 178.5 to 188.2 ms. Full-command timing includes both.
The narrow case declines the pipeline. Eleven protected controls, measured
without phase tracing, have a maximum candidate/baseline median ratio of
1.0089 and geometric mean 0.9693. This remains short of the owner's win target.

All 84 accepted calls preserve input/sample identity and bit-identical native
coefficient, covariance, status, DoF, fit and certificate matrices. Fixest
checks retain the existing tolerances. [Numeric measurements](measurements.json),
[summaries](summary.json) and [scientific checks](science.json) are preserved.
The three licensed scientific/transaction fixtures pass explicit terminal and
fixture sentinels, including a wide explicit-dummy oracle and incomplete samples.

Four focused Rust tests pass: transpose/tail/missing identity and caller-thread
reads; early decline for dimensions/thread count/memory; joining after read
failure, invalid missing boundary or producer panic; and waking the producer
after a worker panic. Mac UBSan bridge tests cover numeric row and column
guards, overflow, reordered addresses, thread refusal, exact write bounds and
mid-read failures. The portable bridge and real-plugin ABI/panic harness pass.
The result is local development evidence, not release or platform qualification.
Raw licensed logs and binaries remain ignored; no installed package was replaced.
