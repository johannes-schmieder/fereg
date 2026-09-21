# Input ownership and metadata admission development check

The candidate admits decoder/compiler tables and input arrays before construction,
counts the retained ABI argument owner, and replaces opaque fixed-width string
dictionaries with explicit capacity-accounted entry/bucket banks. Growth charges
old and new storage plus all simultaneously live source dictionaries. Production
SPI reserves one admitted cell buffer and returns it without a second copy.
Registry counts and cluster arities must match supplied payloads before reserve.
Sorting/recode preserves raw byte order and original sample/missingness semantics.
The core headroom rule is shared without changing its value.

These are checked component admissions. Whole-action coverage is **incomplete**,
its ratio is undefined, and this checkpoint is not a production candidate.
Transport/error parents, remaining construction/numerical/generation overlaps,
and dependency/runtime lifetimes still need complete proofs. No public memory
field definition or numerical policy changes. See [validation.json](validation.json)
and the [source comparison](incremental-implementation.patch).

## Identity and checks

Base main: `5240e589400c888690666265263ad96f9b694aa6`. Candidate snapshot:
`5240e58-input-memory3`; baseline: unpublished `5240e58-core-certification1`.
The [source verification](source-verification.json) binds the frozen source;
subsequent inventory-only script updates do not change tested production Rust,
Stata or build bytes. Baseline Linux artifact SHA256:
`a8e5a0078de990c22eb581566f40d58e20cd1b64e90ec4792a2794cc420208f3`;
candidate: `b1b6ff656080f322f5c1659083bb8a5f6b07052054aaeea863f4016c9bf3422d`.

All 137 plugin and 46 PPML memory checks pass, including independent sorted and
allocation oracles, simultaneous dictionary growth, repeated calls, exact/short
budgets, malformed metadata, interruption, allocation failure and rollback.
The 39 Python checks, repository check and OLS v27/action v10 inventories pass.
Linux export and actual ABI/mock-SPI loading pass. Mac execution remains blocked
by the unaccepted Xcode license. The installed Mac artifact is unchanged.

[Job receipts](jobs.json) preserve all six 28-slot runs; tests and commands use
four execution cores. Input2's original two fixture failures remain failed:
the publication fixture omitted the new metadata minimum, and the core-rejection
fixture imposed a one-byte limit while still constructing its inputs. Corrected
fixtures retain their original acceptance/error requirements. Cost3 built and
passed ABI checks, then failed before estimation because the new generator used
a hash format incompatible with the existing importer. The one narrow wrapper
fix restores the importer’s existing numeric hash convention and converts string
sources inside Stata after import validation. No scientific retry was used.

## Paired cost results

Four cases were fixed before measurement. Each has three alternating fresh-process
baseline/candidate pairs. All 24 commands have exact scientific, sample/order,
solver non-time and saved-output agreement. Node occupancy is checked around
each command, with a full 28-core reservation and four physical execution cores.

| Case | Baseline median seconds | Candidate median seconds | Initial cost gate |
| --- | ---: | ---: | --- |
| Narrow numeric OLS | 0.103 | 0.145 | Flag |
| Wide string robust OLS | 0.463 | 0.467 | Pass |
| String clustered low-memory OLS | 0.549 | 0.476 | Pass |
| String clustered PPML | 0.524 | 0.600 | Flag |

The declared flag requires both >5% and >10 ms median slowdown. One bounded
investigation used the same binaries, phase traces and Stata profiling for the
two flagged cases, plus identical-baseline controls. Its 24 commands also preserve
exact science. Narrow A/B medians are 0.054/0.055 seconds (A/A 0.054/0.055);
PPML A/B 0.281/0.245 (A/A 0.261/0.254). No command or reported phase flags recur.
This different host/profiling context does not prove the cause of the original
flags. Original flags and receipts remain preserved; no further retries ran.

[Phase costs](phase-costs.json) retain command/phase timing, allocation, RSS,
scratch and solver metrics. OLS runtime rows 18–21 are metadata preflight, input
loading, post-input native work and plugin precommit total, respectively.
Median native allocation peaks match within each original paired case; no
whole-command memory-reduction or universal speedup claim is made. Input and
artifact hashes, explicit worker sentinels and scheduler success accompany the
archived receipts. Raw licensed logs remain external; their hashes are preserved
in [original-log-hashes.json](original-log-hashes.json).

All checkpoint jobs are terminal. No empirical estimation, stopped campaign,
installed-package replacement, commit, push, tag or release was performed.
