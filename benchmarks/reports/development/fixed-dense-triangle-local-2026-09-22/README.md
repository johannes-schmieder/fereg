# Fixed small covariance triangles: local development

Retain the bounded covariance update in ADR 0100. Two unchanged five-pair
blocks confirm modest whole-command gains for serial robust slope fits.
The numerical results and tracked allocation peaks are unchanged. This does
not close the remaining serial fixest losses or the broader OLS milestone.

## Change

The shared dense sandwich reducer selects a fixed-array update once for each
reported dimension from 1 through 16. The update borrows the existing score
and meat slices as fixed arrays, allowing the compiler to eliminate dynamic
triangle-offset and bounds bookkeeping. It adds no heap owner. Wider designs
retain the previous contiguous loop. This is a consecutive bounded range,
not a selection fitted to particular weights, benchmark widths or solvers.

Each production/certificate coordinate retains its multiplication operands
and observation order. Separate multipliers, score scaling, finite checks,
block partitions, forward production merge, reverse certificate merge and
compensated certification remain unchanged. All four borrow shapes are checked
before either meat is modified. OLS and PPML use the shared reducer; scratch
inference, clustered inference and solver selection are unchanged.

## Measurement

Apple M2 Ultra, Stata MP 19, N=245,760, fresh worker processes, fixed native/R
teams and alternating pairs. The screen uses three pairs across sixteen slope
cells, with the pinned fixest comparator except for FW. Two subsequent blocks
use five native pairs across the same seven serial confirmation cells; the
candidate is unchanged. Ordinary one-/two-FE robust controls use three pairs
at K=1/5/15/16 and T=1/16. K15/K16 exercise both sides of the reported-dimension
16 boundary. All timed roots report no detected competing-process overlap.
No compilation, tests, profiling or archiving ran alongside headline fits.

| Serial slope case | Block 1 prior/candidate seconds | Block 1 paired ratio | Block 2 prior/candidate seconds | Block 2 paired ratio |
|---|---:|---:|---:|---:|
| AW K10 robust, full outputs | 0.243 / 0.228 | 0.9556 | 0.240 / 0.232 | 0.9667 |
| PW K6 robust, full outputs | 0.218 / 0.209 | 0.9543 | 0.217 / 0.212 | 0.9821 |
| PW K6 robust, saved effects | 0.197 / 0.194 | 0.9700 | 0.201 / 0.192 | 0.9552 |
| Unit K10 robust, full outputs | 0.214 / 0.200 | 0.9340 | 0.216 / 0.200 | 0.9358 |
| FW K10 robust, full outputs | 0.248 / 0.229 | 0.9347 | 0.241 / 0.232 | 0.9620 |
| AW K10 cluster2, full outputs | 0.286 / 0.287 | 1.0142 | 0.282 / 0.287 | 1.0253 |
| PW K6 cluster2, full outputs | 0.238 / 0.245 | 1.0041 | 0.243 / 0.243 | 1.0000 |

Ratios are medians of per-repeat candidate/prior ratios, not ratios of the
displayed marginal medians. Clustered fits do not use the changed dense robust
update; their small changes remain recorded. All sixteen ordinary-FE controls
are below the 1.05 native-control limit; the largest paired ratio is 1.0333.
The serial one-FE controls improve at K1/K5/K15, while K16 retains the generic
loop. Threaded and two-FE gains are small or mixed. Full screen, confirmation
and control results are in [timings.md](timings.md).

The screen still has serial losses: AW K10 robust/full is 0.226 seconds versus
fixest 0.188; PW K6 robust/full is 0.209 versus 0.152. Threaded and K50 cells
remain favorable in this screen, but these are not new final-matrix claims.
FW timings compare native versions only; ordinary fixest weighted inference
is not a Stata frequency-weight oracle.

## Profiling and safeguards

Before changing the kernel, statistical sampling of owned Stata workers
localized the prior covariance cost. Two controls run one fit each; two
sampled workers run eighty successful target fits each. Their final exports,
samples and 28 non-timing solver fields match exactly. Repeated-fit durations
are diagnostic and never included in headline timing tables.

| Prior covariance stack attribution | AW K10 | PW K6 |
|---|---:|---:|
| Row construction | 246 / 1,510 samples (16.3%) | 137 / 722 (19.0%) |
| Multiplier/reference weight helpers | 63 / 1,510 (4.2%) | 50 / 722 (6.9%) |
| Reduction, scaling and traversal remainder | 1,201 / 1,510 (79.5%) | 535 / 722 (74.1%) |

The remainder does not isolate pure triangle arithmetic. Matching disassembly
shows dynamic offset, loop and bounds bookkeeping in the reduction. Audit v2
adds tail-called reference-weight frames omitted by v1, without double-counting
frames beneath explicit multiplier callers; both audits are preserved. The
outer sampling plans explicitly exclude headline timing use. The nested
generic harness labels phase instrumentation as disabled; that label does not
make sampled repeated-fit durations eligible as headline comparisons.

Three separate diagnostic triples compare prior diagnostic, uninstrumented
candidate and candidate diagnostic artifacts. All exports and saved arrays
match exactly. In single traces, AW K10 covariance falls from 24.031 to 11.511
ms and PW K6 from 11.285 to 6.327 ms. The uninstrumented candidate under phase
tracing gives 12.164 and 6.131 ms respectively. K50 diagnostic covariance is
170.860 versus 170.807 ms, consistent with retaining the generic wide loop.
These traces localize the gain; only the paired uninstrumented blocks support
the whole-command performance result.

Nineteen focused release Rust fixtures pass in six invocations. They include
exact comparison with the prior independently written reduction over every
dimension 1..17 plus 31/50/65, different teams/block sizes and distinct
production/certificate multipliers; malformed-shape rejection before mutation;
packed/strided OLS equality; inference allocation/mutation failure; PPML thread
selection, literal FW replication and finite-input overflow rejection. Both
new plugins pass actual ABI/self-test harnesses. No broader licensed PPML or
cross-platform qualification was run.

All 166 timed native pairs preserve six numerical exports, sample/input
identity and all 28 non-timing solver fields exactly, with equal tracked
allocation peaks. Requested saved arrays are exact. This change relies on
those arithmetic fixtures and exact comparisons with the accepted baseline;
it does not claim a new independent large-design oracle campaign. Process RSS
is not a measured memory acceptance claim here.

## Evidence identity and limits

| Artifact | Plugin SHA-256 |
|---|---|
| Prior scb1 | `c68c679ecb3790139ece883bfe93da422cae212e338d693891a631d8986e1dc9` |
| Candidate dtk1 | `77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151` |
| Prior diagnostic scb1d | `766c06d36dc221f1cbaab65b2057303fb744c138ac8f8688caa27e916b52a1a8` |
| Candidate diagnostic dtk1d | `12161eee6e6cc37c8920183f63fe2be34812e590f26cbb5652b27a1f0401b769` |

Candidate base: `c52a59e3008b638153cfb1ada905f29becab1e06`, with the six separately
pending graph/window drafts and this sandwich change. The complete captured
523-file trees reconstruct from each identity's Git base plus stored overlays.
The local checkpoint commits only this covariance change, not the graph/window
drafts. Evidence applies to the captured full source, not an unmeasured clean
checkout or another artifact.

There are 390 successful worker result records: 141 screen, 140 confirmation,
96 ordinary controls, nine diagnostic traces and four sampling workers. The
sampling workers contain 162 successful target fits, so the archive represents
548 target fits in total, excluding warm-ups. These counts are not interchangeable.

`evidence.zip` retains sanitized measurement records, worker sources, command
contracts, numerical exports and hashes of local-only large saved arrays.
`sources.zip` contains source overlays and exact controllers/auditors.
`checks.zip` contains test/build evidence, audits, filtered phase traces,
portable stack samples and disassembly. Raw licensed logs and binaries are
not committed. `verify.py` verifies the archive and complete source trees;
`integrity.json` and `privacy-check.json` record the checks.

Archive verification passes for 11,089 stored entries, 272 explicitly omitted
local arrays and four complete source trees. The first verifier rejected
macOS sample's already-redacted `/Users/*/fereg_macos.plugin` placeholder; the
second attempt had an indentation error. Both failed verifier versions remain
alongside the passing verifier. Its sole path-scan exception is that exact
already-redacted placeholder; the captured archive data was not rewritten.

Linux confirmation, the six pending graph/window changes, routing adoption,
remaining serial/reference discrepancies and final integration remain open.
Earlier final blocks do not transfer to this source. No push, installation,
release or qualification is implied.
