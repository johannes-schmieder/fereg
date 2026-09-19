# One-FE optimization development campaign

`build.py ROOT NAME --revision SHA --overlay PATH...` creates a new immutable
source snapshot from the named commit and explicit working-tree overlays. Every
variant gets its own Cargo target directory, complete source hashes, arm64
release build and real-plugin ABI check. It never replaces the installed plugin.

Select Rust 1.85.1 and the Apple Command Line Tools in the process environment.
Use the preceding one-FE profile's Python dependencies and frozen datasets.
Set `STATA_MP_PATH` to licensed local Stata, and put its pinned stock fixest R
library first in `R_LIBS_USER`, followed by the existing R dependencies.

`run.py ROOT --part primary --variants BASE CANDIDATE fixest --label STEP`
runs three rotated fresh-process blocks for both primary cases. `--part controls`
covers all eleven protected cells; `--repeats 5` produces confirmation blocks.
Use a distinct label for each block. Complete-command timing includes metadata,
allocation, input loading, preparation, computation and publication; import and
the same small warm-up are outside timing. Each call audits imported doubles,
sample membership, scientific output and a terminal sentinel. Native variants
must match scientific matrices exactly at this stage. Run timings without
concurrent builds or test processes.

Run the existing `benchmarks/stata_input_probe/fixtures.py` function for each
candidate's focused licensed scientific and transaction fixtures. Then
`archive.py ROOT REPORT STEP BASE CANDIDATE` creates path-free numerical receipts
and source/binary identities without copying licensed logs or data. Explanatory
reports state precisely which tests and benchmark blocks were accepted.
