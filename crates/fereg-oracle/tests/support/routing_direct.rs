//! Full-rank, bounded wide-RHS witnesses independent of production routing.
use fereg_oracle::designs::{Design, WeightKind};
use fereg_oracle::explicit_ols::{DummyEffect, ExplicitOlsResult};

fn walsh(row: usize, mask: usize) -> f64 {
    if (row & mask).count_ones() % 2 == 0 {
        1.
    } else {
        -1.
    }
}

pub fn design(weighted: bool) -> Design {
    let n = 128;
    let worker: Vec<_> = (0..n).map(|r| (r / 4) as u32).collect();
    let firm: Vec<_> = (0..n)
        .map(|r| (if weighted { 2 * (r / 16) } else { 0 } + (r / 2) % 2) as u32)
        .collect();
    // Dummy span: all worker characters, plus firm characters. Excluding
    // these leaves 95 independent directions (unit) or 88 (weighted).
    let masks: Vec<_> = (1..128)
        .filter(|mask| mask % 4 != 0 && if weighted { mask & 13 != 0 } else { *mask != 2 })
        .take(63)
        .collect();
    assert_eq!(masks.len(), 63);
    let regressors: Vec<Vec<f64>> = masks
        .iter()
        .enumerate()
        .map(|(j, &mask)| {
            (0..n)
                .map(|r| {
                    walsh(r, mask)
                        + 2.
                        + (1 + j % 3) as f64 / 8. * walsh(r, 4)
                        + (1 + j % 5) as f64 / 16. * walsh(r, 2)
                })
                .collect()
        })
        .collect();
    let outcome = (0..n)
        .map(|r| {
            5. + worker[r] as f64 / 16. - firm[r] as f64 / 8.
                + walsh(r, 127) / 4.
                + regressors
                    .iter()
                    .enumerate()
                    .map(|(j, x)| (j as f64 % 7. - 3.) / 32. * x[r])
                    .sum::<f64>()
        })
        .collect();
    Design {
        outcome,
        regressors,
        effects: vec![
            DummyEffect::new(worker, 32, true, vec![]).unwrap(),
            DummyEffect::new(firm, if weighted { 16 } else { 2 }, true, vec![]).unwrap(),
        ],
        weights: (0..n)
            .map(|r| {
                if weighted {
                    1. + ((r * 13 + r / 4) % 7) as f64 / 8.
                } else {
                    1.
                }
            })
            .collect(),
        weight_kind: if weighted {
            WeightKind::Analytic
        } else {
            WeightKind::Unit
        },
        missing: vec![false; n],
        clusters: vec![],
    }
}

pub fn reported_constant(design: &Design, oracle: &ExplicitOlsResult) -> f64 {
    design
        .weights
        .iter()
        .zip(&oracle.absorbed_fitted_values)
        .map(|(w, value)| w * value)
        .sum::<f64>()
        / design.weights.iter().sum::<f64>()
}

/// SPEC gauge: each later intercept effect has component-weighted mean zero;
/// the first effect receives the remainder after the reported global constant.
/// Recover the two firm contrasts directly from QR fitted values at one worker,
/// not through another normal-equation or Schur solve.
pub fn saved_channels(design: &Design, oracle: &ExplicitOlsResult) -> [Vec<f64>; 2] {
    let components = design.effects[1].n_levels() / 2;
    let component_rows = 128 / components;
    let constant = reported_constant(design, oracle);
    let mut channels = [vec![0.; 128], vec![0.; 128]];
    for component in 0..components {
        let start = component * component_rows;
        let difference =
            oracle.absorbed_fitted_values[start + 2] - oracle.absorbed_fitted_values[start];
        let mut masses = [0., 0.];
        for row in start..start + component_rows {
            masses[(row / 2) % 2] += design.weights[row];
        }
        let total = masses[0] + masses[1];
        let firm = [
            -difference * masses[1] / total,
            difference * masses[0] / total,
        ];
        for row in start..start + component_rows {
            channels[1][row] = firm[(row / 2) % 2];
            channels[0][row] = oracle.absorbed_fitted_values[row] - constant - channels[1][row];
        }
    }
    channels
}
