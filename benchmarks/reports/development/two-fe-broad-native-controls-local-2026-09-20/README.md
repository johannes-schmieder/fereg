# Full broad two-way native-control inventory

Local development on 2026-09-20, M2 Ultra, Stata/MP 19.0 with eight licensed
processors. Native requests use sixteen workers except the declared thread
controls. The embedded build date is September 19. This is not release or
cross-platform qualification, and it contains no new fixest timing claims.

All 111 declared controls now have three-pair scientific comparisons. The
single-call timing gate remains open for one small weighted case. Production
remains `cdfbcf58`, measured as the existing `usefulpilot` artifact:
`79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`.
Its base is `558f4c46` plus the three recorded core overlays; all 516 captured
source files match. The baseline plugin is
`1e0bf355e5e9cd9b37494386ee6808c33122dffb4aac699122517beb12ecffdd`,
whose 514-file provenance binds it to milestone baseline `e9d593bd`.
Both actual-plugin ABI checks passed in their retained build evidence.

## Coverage and timing

These are geometric means and maxima of the cell median paired
candidate/baseline ratios. The table combines the 99 complete original cells
with the twelve repaired exporter cells described below. It retains the
initial ratio for the subsequently confirmed small-sample cell.

| Control family | Cells | Geometric mean | Worst median ratio |
|---|---:|---:|---:|
| Frequency weights | 15 | 0.731 | 0.882 |
| Weight distributions | 12 | 0.536 | 0.681 |
| Tiny weighted bridges | 4 | 0.608 | 0.724 |
| Thread counts | 12 | 0.950 | 1.017 |
| Width boundaries | 12 | 0.970 | 1.018 |
| Sample boundaries | 8 | 0.902 | 1.059 |
| Low memory | 4 | 0.929 | 1.009 |
| Protected one-FE | 24 | 0.886 | 1.008 |
| Protected dense unit | 20 | 1.005 | 1.028 |

The only ratio above 1.05 is AW IID at N=7,680, K=6 and sixteen workers.
Its initial three-pair median ratio is 1.05882 (18 versus 17 ms); a fresh
five-pair confirmation is 1.0625 (17 versus 16 ms). These measurements remain
failed timing checks. A separate five-pair diagnostic timed sixteen repeated
commands per worker, producing a median ratio of 0.992. This suggests the
one-millisecond result needs finer timing resolution, but the batch changes
the measurement scope and does not replace the single-call gate.

The five single-call confirmation receipts also retain high-resolution native
phase times. Median plugin time is 10.142 versus 9.874 ms, while native compute
medians are 9.002 versus 8.692 ms. Their averages are closer; the candidate's
native observations vary between approximately 8.4 and 9.0 ms. The remainder
includes Stata work and the millisecond command timer's resolution. A separate
high-resolution timer surrounding the same public command is the next useful
check; no estimator change is justified from the rounded total alone.

## Preserved exporter failure and repair

The first complete inventory attempted every cell: 99 cells completed and
12 one-FE/two-cluster/saved-output cells failed in the baseline worker. The
cached worker requested `out_fe2` after one-FE estimation. Every affected
Stata estimation returned zero, then export failed with `r(3598)`. The
candidate was not run in those initial cells. They remain failures in the
original plan, progress, score and measurement receipts.

The current staging source already supported a single exported FE, but the
old cached controller bypassed staging. The repair used a fresh controller,
the same binaries and audited inputs, and the original three repetitions for
exactly those twelve cells. All twelve pass; their worst median paired ratio
is 0.99498. `control-disposition.json` maps each original cell to its complete
comparison and retains the open timing cell. It does not rewrite the original
score or relabel the failed attempts. `export-failure-adjudication.json`
records exact original result/log hashes and the repair mapping.

`original-controller/` preserves the worker actually used for the first pass;
`repair/controller/` preserves its successor. The Stata diff fixes one-FE
export after timing and makes the regressor loops explicit. Commands, native
source, inputs and repetitions are unchanged. The updated runner now binds
staging sources and generated files in a manifest and rejects missing, stale
or modified cached controllers before dispatch. Its focused test passes fresh,
unversioned, modified-worker and stale-source cases.

## Science and memory

There are 688 standard worker invocations: 606 original (including twelve
export failures), 72 repaired, and ten single-call confirmations. Ten further
workers are explicitly separate sixteen-command batch diagnostics. The 343
successful native pairs comprise 333 inventory pairs, five confirmation pairs
and five batch pairs. All meet registered numerical comparisons and exact
sample/status/DoF requirements; 253 preserve all six scientific files bitwise.
No estimator scientific failure was observed.

All 129 saved-output pairs pass independent weighted normal-equation, FE
constancy, normalization, RSS and reconstruction audits; 66 output pairs are
bitwise exact. The auditor now handles the protected one-FE output layout
without applying two-FE component labels. Its independent construction passes
six valid unit/AW/FW saved/full-output cases and rejects six corrupted cases.
The same binary passes the weighted explicit-dummy fixture across K=1/3/6/13,
AW/PW/FW and inference choices, including literal frequency expansion and
AW/PW rescaling. The input fixture passes missing-value, storage-type,
singleton, excluded-invalid-weight and row-order checks. Earlier tests and
three final-artifact fixtures for this exact binary remain in the
[useful-pilot report](../two-fe-useful-pilot-local-2026-09-20/README.md).

Native allocation peaks rise in seven distinct cells (21 paired observations):
four FW K=1 IID/robust/cluster controls by 5,111,856 bytes; the N=61,440,
K=1 AW width control by 319,536 bytes; and two protected K=1 unit one-FE
robust/saved controls by 72 bytes. All increased peaks remain below their
unchanged early selected-memory estimates. Earlier activation of existing
owners has a measurable lifetime cost; this is not a memory-neutral claim.

Fourteen distinct cells exceed 1.25 times the early selected-memory estimate,
including zero-regressor, small-sample, difficult-weight and low-memory cases.
Every such peak and estimate is exactly unchanged from the baseline. This
ratio uses the early runtime estimate and is not a new complete memory-ledger
qualification; the retained earlier
[narrow-input adjudication](../two-fe-narrow-input-local-2026-09-20/README.md)
also distinguishes that estimate from full solver/workspace coverage. The
maximum early ratio is 3.28144. All observations, including these gaps, remain
in `memory-observations.json`; no estimate was enlarged to make them pass.

The full milestone remains open. Resolve the small single-call timing question,
then measure the sixteen predeclared fresh-seed holdouts and both five-pair
77-cell primary blocks on the unchanged estimator artifact. Use a fresh,
manifest-checked controller directory for further application runs. No
installed package, remote state or release qualification changed.
