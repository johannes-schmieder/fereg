# Documentation index

## Start here

- [`../README.md`](../README.md) — repository boundary and common commands.
- [`user_guide.md`](user_guide.md) — complete public syntax, absorbed-term
  notation, inference, saved outputs, and benchmark guidance.
- [`../stata/fereg.sthlp`](../stata/fereg.sthlp) — installed help with twelve
  self-contained examples.
- [`../stata/fereg_postestimation.sthlp`](../stata/fereg_postestimation.sthlp)
  — prediction, `test`, and `estat summarize`.
- [`../AGENTS.md`](../AGENTS.md) — concise operating rules for repository work.
- [`../STATUS.md`](../STATUS.md) — sole mutable development status and resume
  point.
- [`development_workflow.md`](development_workflow.md) — local development,
  commit, testing, and exact-SHA acceptance procedure.
- [`../STATA_CI_RUNNER.md`](../STATA_CI_RUNNER.md) — licensed Mac runner,
  profiles, security, and operations.
- [`adr/`](adr/) — accepted architectural and scientific decisions.

## Architecture and scientific contracts

- [`architecture.md`](architecture.md)
- [`data_plane.md`](data_plane.md)
- [`factor_protocol.md`](factor_protocol.md)
- [`plugin_protocol.md`](plugin_protocol.md)
- [`ols_core.md`](ols_core.md)
- [`scientific_contracts.md`](scientific_contracts.md)
- [`fe_solver.md`](fe_solver.md)
- [`weight_semantics.md`](weight_semantics.md)
- [`memory_model.md`](memory_model.md)
- [`threading.md`](threading.md)
- [`performance_contract.md`](performance_contract.md)

## Validation and release

- [`compatibility_matrix.md`](compatibility_matrix.md)
- [`clean_machine_test_plan.md`](clean_machine_test_plan.md)
- [`release_process.md`](release_process.md)
- [`upstream_map.md`](upstream_map.md)
- [`stata_parser_review.md`](stata_parser_review.md)

`STATUS.md` records current progress. The user guide and Stata help describe the
public command. The remaining documents in this directory describe contracts
and design; they should not contain competing live milestone or handover state.
