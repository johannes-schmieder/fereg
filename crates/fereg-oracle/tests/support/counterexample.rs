//! Original 800-row CMG/CG counterexample; one unchanged generator for both cores.
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::DummyEffect;

pub fn counterexample() -> Design {
    let rows = 800;
    let worker = (0..rows).map(|i| (i / 8) as u32).collect::<Vec<_>>();
    let firm = (0..rows)
        .map(|i| ((i / 8 + i % 8) % 10) as u32)
        .collect::<Vec<_>>();
    let x = (0..rows)
        .map(|i| ((i * 37 + 11) % 997) as f64 / 97.0 - 5.0)
        .collect::<Vec<_>>();
    let outcome = (0..rows)
        .map(|i| {
            0.7 * x[i]
                + (f64::from(worker[i]) * 0.13).sin()
                + (f64::from(firm[i]) * 0.29).cos()
                + (((i * 53 + 7) % 101) as f64 - 50.0) * 0.001
        })
        .collect();
    Design {
        outcome,
        regressors: vec![x],
        effects: vec![
            DummyEffect::new(worker, 100, true, vec![]).unwrap(),
            DummyEffect::new(firm, 10, true, vec![]).unwrap(),
        ],
        weight_kind: WeightKind::Unit,
        weights: vec![1.0; rows],
        missing: vec![false; rows],
        clusters: vec![],
    }
}
