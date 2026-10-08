//! Frozen numeric inputs keep bias reference identities independent of libm.
//!
//! The compact fixture preserves the seed-823, 120-row DensePanel input once;
//! columns are outcome/x0/x1 IEEE bits followed by two zero-based FE codes.
//! Targets are still calculated independently from these fixed inputs.
use super::{expected, wire};
use fereg_oracle::{
    designs::{Design, WeightKind},
    explicit_ols::DummyEffect,
};
use std::path::Path;

fn design() -> Design {
    let mut lines = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/reference/ppml_bias_input.txt"
    ))
    .lines();
    assert_eq!(lines.next(), Some("FEREG_BIAS_INPUT_V1"));
    let mut numeric = [Vec::new(), Vec::new(), Vec::new()];
    let mut codes = [Vec::new(), Vec::new()];
    for line in lines {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        assert_eq!(fields.len(), 5);
        for (column, field) in numeric.iter_mut().zip(&fields[..3]) {
            let value = f64::from_bits(u64::from_str_radix(field, 16).unwrap());
            assert!(value.is_finite());
            column.push(value);
        }
        for (column, field) in codes.iter_mut().zip(&fields[3..]) {
            column.push(field.parse::<u32>().unwrap());
        }
    }
    let [outcome, first, second] = numeric;
    let rows = outcome.len();
    assert_eq!(rows, 120);
    Design {
        outcome,
        regressors: vec![first, second],
        effects: codes
            .into_iter()
            .map(|values| {
                let levels = *values.iter().max().unwrap() as usize + 1;
                DummyEffect::new(values, levels, true, Vec::new()).unwrap()
            })
            .collect(),
        clusters: Vec::new(),
        weights: vec![1.; rows],
        weight_kind: WeightKind::Unit,
        missing: vec![false; rows],
    }
}

pub fn generate(directory: &Path) {
    let design = design();
    for randomized in [false, true] {
        let mut request = wire::make(&design, true, "robust", "auto", false);
        request.bias(&design, randomized);
        let name = if randomized {
            "ppml-bias-randomized"
        } else {
            "ppml-bias-exact"
        };
        request.write(&directory.join(format!("{name}.case")));
        expected::write(
            &directory.join(format!("{name}.json")),
            &design,
            true,
            "robust",
            &request,
        );
    }
}
