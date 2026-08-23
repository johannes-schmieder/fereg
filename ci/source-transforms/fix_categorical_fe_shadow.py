#!/usr/bin/env python3
"""Repair and lock categorical FE identity validation in the hidden OLS shadow."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ESTIMATE = ROOT / "crates/stfixest-plugin/src/estimate.rs"
CONTRACT = ROOT / "tests/python/test_categorical_hidden_transport_contract.py"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one occurrence, found {count}")
    return text.replace(old, new, 1)


estimate = ESTIMATE.read_text(encoding="utf-8")
old_validation = '''    for row in shadow.fixed_effects.chunks_exact(FE_COLUMNS) {
        if !matches!(row[0], 1.0 | 2.0)
            || exact_usize(row[1], "OLS FE term identity").is_err()
            || exact_usize(row[2], "OLS FE source identity").is_err()
            || exact_usize(row[3], "OLS FE category count").is_err()
            || row[3] < 1.0
            || exact_usize(row[4], "OLS FE redundancy").is_err()
            || exact_usize(row[5], "OLS FE coefficient count").is_err()
            || !matches!(row[6], 0.0 | 1.0)
            || !matches!(row[7], 0.0 | 1.0)
        {
            return Err(invalid("OLS FE shadow metadata is invalid"));
        }
    }
'''
new_validation = '''    for row in shadow.fixed_effects.chunks_exact(FE_COLUMNS) {
        validate_shadow_fixed_effect_row(row)?;
    }
'''
estimate = replace_once(
    estimate,
    old_validation,
    new_validation,
    "categorical FE shadow validation loop",
)

helper = '''fn validate_shadow_fixed_effect_row(row: &[f64]) -> Result<()> {
    if row.len() != FE_COLUMNS {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    }
    let Ok(identity) = exact_usize(row[0], "OLS FE identity") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(term) = exact_usize(row[1], "OLS FE term identity") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(source) = exact_usize(row[2], "OLS FE source identity") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(categories) = exact_usize(row[3], "OLS FE category count") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(redundant) = exact_usize(row[4], "OLS FE redundancy") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let Ok(coefficients) = exact_usize(row[5], "OLS FE coefficient count") else {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    };
    let identity_is_valid = match identity {
        // One categorical source is represented directly.
        1 => term > 0 && source > 0,
        // The implicit no-absorb constant has no term or source identity.
        2 => term == 0 && source == 0,
        // A categorical tuple retains the absorbed-term identity, while the
        // single source cell is deliberately zero because no one source is
        // authoritative for the interaction partition.
        3 => term > 0 && source == 0,
        _ => false,
    };
    if !identity_is_valid
        || categories == 0
        || redundant > categories
        || coefficients != categories - redundant
        || !matches!(row[6], 0.0 | 1.0)
        || !matches!(row[7], 0.0 | 1.0)
    {
        return Err(invalid("OLS FE shadow metadata is invalid"));
    }
    Ok(())
}

'''
estimate = replace_once(
    estimate,
    "fn store_shadow(\n",
    helper + "fn store_shadow(\n",
    "categorical FE shadow validation helper",
)

unit_tests = '''

    #[test]
    fn shadow_fe_identity_accepts_single_implicit_and_interaction_rows() {
        for row in [
            [1.0, 1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 0.0],
            [2.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0],
            [3.0, 1.0, 0.0, 6.0, 1.0, 5.0, 1.0, 0.0],
        ] {
            validate_shadow_fixed_effect_row(&row).expect("valid FE shadow row");
        }
    }

    #[test]
    fn shadow_fe_identity_rejects_malformed_combinations() {
        for row in [
            [1.0, 1.0, 0.0, 3.0, 1.0, 2.0, 1.0, 0.0],
            [2.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0],
            [3.0, 0.0, 0.0, 6.0, 1.0, 5.0, 1.0, 0.0],
            [3.0, 1.0, 2.0, 6.0, 1.0, 5.0, 1.0, 0.0],
            [3.0, 1.0, 0.0, 2.0, 3.0, 0.0, 1.0, 0.0],
            [3.0, 1.0, 0.0, 2.0, 1.0, 2.0, 1.0, 0.0],
        ] {
            assert!(validate_shadow_fixed_effect_row(&row).is_err());
        }
    }
'''
module_close = estimate.rfind("\n}\n")
if module_close < 0:
    raise SystemExit("categorical FE shadow unit tests: test-module close was not found")
estimate = estimate[:module_close] + unit_tests + estimate[module_close:]
ESTIMATE.write_text(estimate, encoding="utf-8")

contract = CONTRACT.read_text(encoding="utf-8")
contract_method = '''    def test_native_fe_shadow_validator_accepts_only_structured_interactions(self):
        estimate = (PLUGIN / "estimate.rs").read_text(encoding="utf-8")
        self.assertIn("validate_shadow_fixed_effect_row(row)?", estimate)
        self.assertIn("3 => term > 0 && source == 0", estimate)
        self.assertIn("coefficients != categories - redundant", estimate)

'''
contract = replace_once(
    contract,
    "    def test_stata_gate_is_intercept_only_but_allows_categorical_tuples(self):\n",
    contract_method
    + "    def test_stata_gate_is_intercept_only_but_allows_categorical_tuples(self):\n",
    "categorical FE shadow source contract",
)
CONTRACT.write_text(contract, encoding="utf-8")

print("repaired categorical FE shadow metadata validation")
