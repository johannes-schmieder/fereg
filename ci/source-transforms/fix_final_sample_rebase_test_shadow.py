#!/usr/bin/env python3
"""Repair the staged final-sample rebase test's helper shadowing."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "crates/stfixest-core/src/ols/prepared_categorical_rebase.rs"

text = TARGET.read_text(encoding="utf-8")
old = '''    #[test]
    fn candidate_continuous_lookup_is_identity_checked() {
        let source = source(17);
        let active = vec![
            PreparedCategoricalModelSource {
                source,
                prepared_source_index: 1,
            },
            PreparedCategoricalModelSource {
                source,
                prepared_source_index: 9,
            },
        ];
        assert_eq!(
            candidate_continuous_kind(&active, slot(1), source).expect("active continuous source"),
            PreparedOlsColumnKind::Continuous {
                source,
                prepared_source_index: 9,
            }
        );
        assert!(candidate_continuous_kind(&active, slot(1), source(18)).is_err());
    }
'''
new = '''    #[test]
    fn candidate_continuous_lookup_is_identity_checked() {
        let active_source = source(17);
        let active = vec![
            PreparedCategoricalModelSource {
                source: active_source,
                prepared_source_index: 1,
            },
            PreparedCategoricalModelSource {
                source: active_source,
                prepared_source_index: 9,
            },
        ];
        assert_eq!(
            candidate_continuous_kind(&active, slot(1), active_source)
                .expect("active continuous source"),
            PreparedOlsColumnKind::Continuous {
                source: active_source,
                prepared_source_index: 9,
            }
        );
        assert!(candidate_continuous_kind(&active, slot(1), source(18)).is_err());
    }
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one shadowed rebase test, found {count}")
TARGET.write_text(text.replace(old, new, 1), encoding="utf-8")
print("repaired final-sample rebase test helper shadowing")
