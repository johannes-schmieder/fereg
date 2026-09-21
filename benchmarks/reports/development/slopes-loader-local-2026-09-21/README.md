# Slope-only loading checkpoint, 2026-09-21

Plugin `slope-rhs2`, SHA
`87a05c6b4d457bdf3ef965c5b58321e6961be71f6f76cfc11dc25df6f3a08202`,
uses `ffa3bde1` plus the two recorded plugin overlays. Production prefixes match
the checkpoint; later test-only corrections are identified in `source-binding.json`.
ABI and panic containment pass. Nothing is installed or pushed.

The bounded loader now handles the actual no-constant recipe of a public
slope-only model. See [ADR 0089](../../../../docs/adr/0089-slope-only-loaded-rhs.md).
Three alternating fresh-process pairs at N=245,760, K6, 16 workers, seed 104729,
PW and three slope-only channels give:

| Request | Prior median ms | Candidate ms | fixest ms |
|---|---:|---:|---:|
| Robust, all outputs | 103 | 95 | 113 |
| Two-way clustering, all outputs | 136 | 129 | 135 |

All eighteen calls, native/reference science and independent output checks pass.
Native allocation peaks remain 84,790,720 and 90,699,788 bytes. AW/FW IID,
robust and two-way clustered full-output checks at N=16,385 also pass. Ordinary
fixest weights are not used as an FW inference oracle. The archive retains 48
calls across the first guard experiment and final science/timing campaigns.
These screens do not replace the planned final blocks or Linux confirmation.

Six pipeline tests and the expanded 22-case SPI test pass. Coverage includes
optional constants, multiple slopes, missing slope/category values, rejected
four-slope loading, insufficient RHS budget, no worker team and exact byte
agreement with the ordinary native path. A fully absorbed finite regressor
remains an explicit equal-error check; this change does not repair that boundary.

The whole-fit memory diagnostic uses N=65,535, K50 and 90,000,000 usable bytes.
Fast mode rejects with r(909): its forecast is 111,692,448 bytes. Low and auto
select the 47,730,304-byte plan, agree exactly with each other and agree
numerically with unconstrained fast mode. Inputs are hash-identical. These
science-only timings overlapped compilation and are not performance evidence.
The separate `memory/` archive contains all four successful/rejected attempts;
this is not complete allocation proof or saved-output memory qualification.

Failed evidence is retained: the first guard change did not admit the public
no-constant recipe; two SPI fixture drafts were corrected; four N=65,536 memory
attempts failed input generation before Stata. The first archive attempt failed
on those absent run directories; the repaired archive includes their plans and
controller logs. Historical evidence is unchanged.

Portable archives contain numerical exports, identities, workers, plans,
checks and phase records. Large arrays and licensed logs remain local; array
hashes, byte counts and independent audit receipts bind the omitted data.
