from __future__ import annotations

import unittest
from pathlib import Path

from scripts.feppml_help_contract import require_public_help_bytes


ROOT = Path(__file__).resolve().parents[2]
SEPARATION = ROOT / "crates/fereg-core/src/ppml/prepared_separation.rs"
SEPARATION_MOD = ROOT / "crates/fereg-core/src/ppml_separation/mod.rs"
SIMPLEX = ROOT / "crates/fereg-core/src/ppml_separation/simplex/mod.rs"
ADR = ROOT / "docs/adr/0017-certified-ppml-separation-ledger.md"
INVENTORY = ROOT / "docs/ppml_stale_module_inventory.md"
ARCHITECTURE = ROOT / "docs/architecture.md"
PERFORMANCE = ROOT / "docs/performance_contract.md"
HELP = ROOT / "stata/feppml.sthlp"


class PpmlFullModelSimplexContractTests(unittest.TestCase):
    def test_production_simplex_receives_the_complete_prepared_model_span(self) -> None:
        separation = SEPARATION.read_text(encoding="utf-8")
        body = separation.split("fn prepared_simplex_proposal", 1)[1].split(
            "\nfn prepared_iterated_rectifier", 1
        )[0]
        for fragment in (
            "let finite_columns",
            "let mut absorbed_columns",
            "usize::from(spec.intercept)",
            "spec.slopes.len()",
            "levels.checked_mul(channels)",
            "let fit_view = generation.fit_view()",
            "fit_view.column_value(column, row)",
            "categorical.numeric_source_values(slope.source)",
            "DenseMatrix::from_row_major(rows, columns, values)",
            "model_simplex_interruptible(",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, body)
        self.assertNotIn("&dyn PreparedOlsFitView", body)
        self.assertNotIn("SkippedUnsupported", body)

        module = SEPARATION_MOD.read_text(encoding="utf-8")
        self.assertIn("mod simplex;", module)
        self.assertIn(
            "pub(crate) use simplex::{model_simplex_interruptible, simplex_workspace_peak_cells};",
            module,
        )
        self.assertNotIn(
            "pub(crate) use simplex::{model_simplex, simplex_workspace_peak_cells};",
            module,
        )
        self.assertNotIn("#[cfg(test)]\nmod simplex;", module)

        kernel = SIMPLEX.read_text(encoding="utf-8")
        self.assertIn("pub(crate) fn model_simplex_interruptible", kernel)
        self.assertIn("#[cfg(test)]\npub(crate) fn model_simplex(", kernel)
        self.assertNotIn("fixed_effects: Option<&FeDesign>", kernel)
        self.assertNotIn("Mixed finite/FE directions are left", kernel)

    def test_joint_peak_precedes_design_allocation_and_skip_requires_ir_closure(self) -> None:
        separation = SEPARATION.read_text(encoding="utf-8")
        body = separation.split("fn prepared_simplex_proposal", 1)[1].split(
            "\nfn prepared_iterated_rectifier", 1
        )[0]
        ordered = (
            "let kernel_peak_cells = simplex_workspace_peak_cells",
            "let construction_peak_cells",
            "let peak_cells = kernel_peak_cells.max(construction_peak_cells)",
            "if peak_cells > config.simplex_max_tableau_cells",
            "MethodStatus::SkippedMemory",
            "let mut values = Vec::new()",
        )
        positions = [body.index(fragment) for fragment in ordered]
        self.assertEqual(positions, sorted(positions))

        coordinator = separation.split(
            "let (simplex_unresolved, restart_after_simplex)", 1
        )[1].split("// Mu is a post-fit detector", 1)[0]
        self.assertIn("let (comprehensive_certified, restart_after_ir)", coordinator)
        self.assertIn(
            "simplex_unresolved.filter(|_| !comprehensive_certified)", coordinator
        )
        self.assertIn("ErrorCode::Allocation", coordinator)
        self.assertIn(
            "prepared simplex remained unresolved and IR did not certify the same generation",
            coordinator,
        )

    def test_documented_contract_no_longer_quarantines_a_finite_only_simplex(self) -> None:
        adr = ADR.read_text(encoding="utf-8")
        self.assertIn("bounded full-model simplex over the finite-plus-FE model span", adr)
        self.assertIn("level indicator times the raw", adr)
        self.assertIn("same ledger generation", adr)
        self.assertNotIn("finite-regressor simplex", adr)

        inventory = INVENTORY.read_text(encoding="utf-8")
        simplex_row = next(
            line
            for line in inventory.splitlines()
            if "simplex/mod.rs" in line and "workspace planner" in line
        )
        internals_row = next(
            line
            for line in inventory.splitlines()
            if "simplex/nullspace.rs" in line
        )
        self.assertIn("| Rewrite |", simplex_row)
        self.assertIn("full-model span", simplex_row)
        self.assertNotIn("Quarantine", simplex_row)
        self.assertIn("| Retain after audit |", internals_row)
        self.assertNotIn("Quarantine", internals_row)

    def test_architecture_performance_and_public_help_contract(self) -> None:
        architecture = ARCHITECTURE.read_text(encoding="utf-8")
        self.assertIn("complete prepared model span", architecture)
        self.assertIn("level-by-raw-slope columns", architecture)
        self.assertIn("never a fitting or inference", architecture)

        performance = PERFORMANCE.read_text(encoding="utf-8")
        self.assertIn("full-model simplex width, checked peak-cell bound", performance)
        self.assertIn("same ledger generation", performance)
        self.assertIn("simplex-only `r(909)`", performance)

        canonical = ROOT / "tests/stata/documentation/feppml_activation.sthlp"
        require_public_help_bytes(HELP.read_bytes(), canonical.read_bytes())


if __name__ == "__main__":
    unittest.main()
