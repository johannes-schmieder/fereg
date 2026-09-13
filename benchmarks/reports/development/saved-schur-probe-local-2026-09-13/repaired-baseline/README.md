# Original saved-Schur PW screen resumed after the energy repair

The prototype remains **unadopted**. Reapply the archived two-file patch against
accepted `dadd956` and resume the exact original PW missing/two-way-cluster/
partial-saved worker, N245760/K50/16 native workers, seed104729. Original data,
warm-up and requested outputs are unchanged; data SHA remains
`eecf8666668f96734ee06c52310b9920f29c65fc9cce86fd3707be22399fc7d0`.
The preceding r(459) failure and baseline-only repair diagnostics are separate.

[Identities](built-identities.json) bind 176 inputs, the [two-file patch](implementation.patch),
accepted baseline plugin `e831a657` and candidate
`18427c19e6fc5512657092bfe0018b180f4e826f5e6a3ea25869d7644f4f77b5`.
Both changed crates compiled for both architectures; the universal static audit
and actual arm64 ABI/self-test pass. The focused accepted/rejected Schur guard
test passes. The candidate passes all 48 saved-output dummy-oracle fits and
rollback with actual new Schur admission; the identical baseline artifact's
48-fit receipt is retained in the [CG repair report](../../cg-energy-audit-local-2026-09-13/README.md).
The candidate oracle was inadvertently launched with repository cwd; its task
log was relocated to the isolated oracle directory after verifying both rc=0
sentinels. Subsequent commands use the existing isolated worker harness.

A fresh rotated pair runs candidate first, with no overlapping build/test work,
on the local M2 Ultra/Stata MP 19. [Science](science.json), [decision](screen-decision.json)
and [checksummed full numeric exports](archive.json) retain both commands.

| Measurement | Repaired baseline | Prototype |
|---|---:|---:|
| Complete command, seconds | 3.498 | 2.518 |
| SPI input, seconds | 0.08097 | 0.08012 |
| Native command, seconds | 3.38267 | 2.40301 |
| Initial absorption setup, seconds | 0.06437 | 0.03796 |
| Initial absorption solve, seconds | 2.90960 | 1.42946 |
| Native peak, MB | 841.84 | 581.41 |
| Reported selected model, MB | 424.69 | 424.69 |
| Sampled process RSS, GiB | 1.084 | 0.944 |
| Initial iterations / fallback flag | 66 / 1 | 138 / 1 |

Command time falls 28.0% and native peak 30.9%. Full b/V and saved residuals are
exact; saved FE maximum difference is 6.05e-10, within the original unchanged
saved-output tolerances. Sample/status/rank/DoF/FE/cluster structure and native
certificates pass. This is one paired screen, not a repeatability or qualification
claim; RSS includes Stata and allocator retention.

The probe accepts at contraction 0.00794698, but the full Schur attempt reaches
its 64-step cap and continues through ordinary CG. The candidate's 138 iterations
charge eight probe steps plus 64 attempted Schur steps plus the baseline's 66 CG
steps. Both final residual checks use zero additional iterations. Thus this gain
includes discarded-attempt and recovery costs; it is not a successful Schur-only
solve. Omitting the initial retained coefficient batch reduces work even on this
continuation path. Remaining native work outside initial absorption increases;
no isolated final-recovery timer or recovery-only speed claim is available.

Both observed peaks exceed the reported model. The [source audit](memory-audit-start.md)
finds final recovery can overlap retained score storage and try retained-product,
CG, SD-LSMR, LSMR and MAP paths without a corresponding final-recovery budget.
The initial Schur/CG guard alone does not prove that boundary. The [resumption
protocol](protocol.md) therefore stops further timing/repeats until checked
workspace/output bounds and constrained-budget failure/rollback behavior exist.
The prototype remains on main as an uncommitted two-file experiment; this report
is evidence only. Installed dist, unrelated Veneto work and old receipts remain preserved. No
agents, SCC, qualification or push occurred.
