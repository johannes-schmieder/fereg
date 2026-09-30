use fereg_engine::ppml::bias::{DiagnosticTable, Summary};

#[test]
fn public_diagnostic_names_match_the_retained_stata_transaction() {
    let ado = include_str!("../../../stata/_fereg_estimate_ppml.ado");
    let row_names = ado
        .split("matrix rownames `bias_diagnostics_shadow' = ")
        .nth(1)
        .unwrap()
        .split("matrix colnames `bias_diagnostics_shadow'")
        .next()
        .unwrap()
        .split_whitespace()
        .filter(|&word| word != "///")
        .collect::<Vec<_>>();
    assert_eq!(row_names, DiagnosticTable::ROWS);
    let support_names = ado
        .split("matrix colnames `bias_support_shadow' = ")
        .nth(1)
        .unwrap()
        .split("ereturn matrix bias_support")
        .next()
        .unwrap()
        .split_whitespace()
        .filter(|&word| word != "///")
        .collect::<Vec<_>>();
    assert_eq!(support_names, Summary::COLUMN_NAMES);
    assert!(ado.contains(&format!(
        "matrix colnames `bias_diagnostics_shadow' = {}",
        DiagnosticTable::COLUMN
    )));
    assert!(ado.contains(&format!(
        "ereturn local bias_validity \"{}\"",
        DiagnosticTable::VALIDITY
    )));
}
