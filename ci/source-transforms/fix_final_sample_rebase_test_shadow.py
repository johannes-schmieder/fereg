#!/usr/bin/env python3
"""Repair the staged final-sample rebase test's helper shadowing."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "crates/stfixest-core/src/ols/prepared_categorical_rebase.rs"

text = TARGET.read_text(encoding="utf-8")
old = '''    #[test]
    fn continuous_rebase_rejects_source_or_status_mismatch() {
        let source = source(17);
        let active = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(0),
            }],
            columns: vec![PreparedOlsColumn {
                coefficient_slot: slot(1),
                kind: PreparedOlsColumnKind::Continuous {
                    source,
                    prepared_source_index: 3,
                },
            }],
        };
        assert!(candidate_continuous_kind(&active, slot(1), source(18)).is_err());

        let omitted = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }],
            columns: Vec::new(),
        };
        assert!(candidate_continuous_kind(&omitted, slot(1), source).is_err());
    }
'''
new = '''    #[test]
    fn continuous_rebase_rejects_source_or_status_mismatch() {
        let active_source = source(17);
        let active = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::Active,
                generated_column_index: Some(0),
            }],
            columns: vec![PreparedOlsColumn {
                coefficient_slot: slot(1),
                kind: PreparedOlsColumnKind::Continuous {
                    source: active_source,
                    prepared_source_index: 3,
                },
            }],
        };
        assert!(candidate_continuous_kind(&active, slot(1), source(18)).is_err());

        let omitted = PreparedOlsModel {
            coefficients: vec![PreparedOlsCoefficient {
                coefficient_slot: slot(1),
                status: PreparedOlsCoefficientStatus::SyntaxOmitted,
                generated_column_index: None,
            }],
            columns: Vec::new(),
        };
        assert!(candidate_continuous_kind(&omitted, slot(1), active_source).is_err());
    }
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one shadowed rebase test, found {count}")
TARGET.write_text(text.replace(old, new, 1), encoding="utf-8")
print("repaired final-sample rebase test helper shadowing")
