from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROTOCOL = ROOT / "crates/fereg-core/src/protocol.rs"
EXPORTS = ROOT / "crates/fereg-core/src/lib.rs"
OLS_RESULT = ROOT / "crates/fereg-plugin/src/result.rs"
DISPATCH = ROOT / "crates/fereg-plugin/src/dispatch.rs"


class PpmlProtocolContractTests(unittest.TestCase):
    def test_versions_are_additive_and_existing_versions_are_frozen(self) -> None:
        protocol = PROTOCOL.read_text(encoding="utf-8")
        for declaration in (
            "ACTION_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::new(1, 5)",
            "MODEL_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(2, 1)",
            "OLS_RESULT_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 4)",
            "PPML_REQUEST_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 5)",
            "PPML_RESULT_WIRE_VERSION: ProtocolVersion = ProtocolVersion::new(1, 2)",
        ):
            self.assertIn(declaration, protocol)

        dispatch = DISPATCH.read_text(encoding="utf-8")
        existing_lines = (
            '"fereg.action_protocol={}.{}\\n"',
            '"fereg.model_wire={}.{}\\n"',
            '"fereg.ols_result_wire={}.{}\\n"',
        )
        positions = [dispatch.index(line) for line in existing_lines]
        self.assertEqual(positions, sorted(positions))

    def test_ppml_receipt_has_a_distinct_closed_thirty_three_column_layout(self) -> None:
        protocol = PROTOCOL.read_text(encoding="utf-8")
        self.assertIn("PPML_RESULT_MAGIC: f64 = 5_354_602.0", protocol)
        self.assertIn("PPML_RESULT_RECEIPT_COLUMNS: usize = 33", protocol)
        self.assertIn(
            "PPML_RESULT_RECEIPT_COMPLETE_COLUMN: usize = "
            "PPML_RESULT_RECEIPT_COLUMNS",
            protocol,
        )
        body = re.search(
            r"pub enum PpmlResultReceiptFieldV1 \{(?P<body>.*?)\n\}",
            protocol,
            re.DOTALL,
        )
        self.assertIsNotNone(body)
        fields = re.findall(r"^\s*(\w+)\s*=\s*(\d+),", body.group("body"), re.MULTILINE)
        self.assertEqual(
            fields,
            [
                ("Magic", "1"),
                ("Major", "2"),
                ("Minor", "3"),
                ("Vce", "4"),
                ("CoefficientCount", "5"),
                ("CovarianceDimension", "6"),
                ("OriginalObservations", "7"),
                ("CandidateObservations", "8"),
                ("FinalObservations", "9"),
                ("ClusterDimensions", "10"),
                ("FixedEffectRows", "11"),
                ("ClusterSubsetRows", "12"),
                ("SamplePresent", "13"),
                ("ResidualPresent", "14"),
                ("MandatoryCertificatesPassed", "15"),
                ("SeparatedObservations", "16"),
                ("SeparationReportRows", "17"),
                ("SavedFixedEffectChannels", "18"),
                ("Adjustment", "19"),
                ("InferenceRows", "20"),
                ("DropCountRows", "21"),
                ("SeparationSequenceRows", "22"),
                ("SeparationExecutionCount", "23"),
                ("SeparationGenerationCount", "24"),
                ("InitialGenerationHashHigh", "25"),
                ("InitialGenerationHashLow", "26"),
                ("FinalGenerationHashHigh", "27"),
                ("FinalGenerationHashLow", "28"),
                ("SeparationTraceDigestHigh", "29"),
                ("SeparationTraceDigestLow", "30"),
                ("GenerationHashDigestHigh", "31"),
                ("GenerationHashDigestLow", "32"),
                ("Complete", "33"),
            ],
        )

    def test_complete_is_a_transaction_sentinel_not_result_data(self) -> None:
        protocol = PROTOCOL.read_text(encoding="utf-8")
        receipt = re.search(
            r"pub struct PpmlResultReceiptV1 \{(?P<body>.*?)\n\}",
            protocol,
            re.DOTALL,
        )
        self.assertIsNotNone(receipt)
        self.assertNotIn("complete", receipt.group("body").lower())
        self.assertIn("pub fn encode_incomplete", protocol)
        self.assertIn("PpmlResultReceiptFieldV1::Complete.index()", protocol)

    def test_ols_serializer_identity_is_unchanged_and_ppml_types_are_exported(self) -> None:
        ols = OLS_RESULT.read_text(encoding="utf-8")
        self.assertIn("const RECEIPT_COLUMNS: usize = 16", ols)
        self.assertIn("const RESULT_MAGIC: f64 = 5_354_601.0", ols)
        self.assertIn("use fereg_core::protocol::OLS_RESULT_WIRE_VERSION", ols)

        exports = EXPORTS.read_text(encoding="utf-8")
        for symbol in (
            "PPML_RESULT_WIRE_VERSION",
            "PPML_REQUEST_WIRE_VERSION",
            "PPML_RESULT_MAGIC",
            "PPML_RESULT_RECEIPT_COLUMNS",
            "PpmlResultReceiptFieldV1",
            "PpmlResultReceiptV1",
            "PpmlResultVceV1",
            "PpmlAdjustmentV1",
        ):
            self.assertIn(symbol, exports)


if __name__ == "__main__":
    unittest.main()
