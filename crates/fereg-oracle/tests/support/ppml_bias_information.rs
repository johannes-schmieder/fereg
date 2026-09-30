//! Explicit anchored full-design QR actions; no production projection or algebra.
use fereg_engine::{
    data::*,
    ppml::{bias, irls},
    threads::ThreadTeam,
    wls::Role,
};
use fereg_oracle::explicit_ols::{
    fit_explicit_ols, DummyEffect, ExplicitOlsProblem, OracleWeights,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub fn columns(source: usize) -> CompiledColumns {
    CompiledColumns {
        slots: vec![
            ColumnSlot {
                name: "base".into(),
                syntax: SyntaxStatus::Base,
                generated: None,
            },
            ColumnSlot {
                name: "x".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(0),
            },
            ColumnSlot {
                name: "duplicate".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(1),
            },
            ColumnSlot {
                name: "omitted".into(),
                syntax: SyntaxStatus::ExplicitlyOmitted,
                generated: None,
            },
            ColumnSlot {
                name: "_cons".into(),
                syntax: SyntaxStatus::Active,
                generated: Some(2),
            },
        ],
        recipes: vec![
            ColumnRecipe::Continuous { source },
            ColumnRecipe::Continuous { source },
            ColumnRecipe::Intercept,
        ],
        roles: vec![Role::Ordinary, Role::Ordinary, Role::AbsorbedIntercept],
        constant_slot: Some(4),
        structural_intercept: true,
        implicit_constant: false,
    }
}
fn close(a: f64, b: f64, seed: usize) {
    assert!(
        (a - b).abs() <= 2e-9 * a.abs().max(b.abs()).max(1.),
        "information seed={seed}: {a:.17e} != {b:.17e}"
    );
}
struct Explicit {
    columns: Vec<Vec<f64>>,
    left: BTreeMap<u32, usize>,
    right: BTreeMap<u32, Option<usize>>,
    finite: Vec<usize>,
}
impl Explicit {
    fn new(effects: &[DummyEffect], fit: &irls::Fit<'_>) -> Self {
        let n = fit.problem().sample.n_active();
        let left_codes = effects[0].codes();
        let right_codes = effects[1].codes();
        let mut neighbors = BTreeMap::<(u8, u32), BTreeSet<(u8, u32)>>::new();
        for (&a, &b) in left_codes.iter().zip(right_codes) {
            neighbors.entry((0, a)).or_default().insert((1, b));
            neighbors.entry((1, b)).or_default().insert((0, a));
        }
        let mut visited = BTreeSet::new();
        let mut roots = BTreeSet::new();
        for &right in &right_codes.iter().copied().collect::<BTreeSet<_>>() {
            if !visited.insert((1, right)) {
                continue;
            }
            roots.insert(right);
            let mut queue = VecDeque::from([(1, right)]);
            while let Some(node) = queue.pop_front() {
                for &other in &neighbors[&node] {
                    if visited.insert(other) {
                        queue.push_back(other);
                    }
                }
            }
        }
        let mut columns = vec![];
        let mut left = BTreeMap::new();
        for code in left_codes.iter().copied().collect::<BTreeSet<_>>() {
            left.insert(code, columns.len());
            columns.push(left_codes.iter().map(|&v| f64::from(v == code)).collect());
        }
        let mut right = BTreeMap::new();
        for code in right_codes.iter().copied().collect::<BTreeSet<_>>() {
            let position = if roots.contains(&code) {
                None
            } else {
                let position = columns.len();
                columns.push(right_codes.iter().map(|&v| f64::from(v == code)).collect());
                Some(position)
            };
            right.insert(code, position);
        }
        let mut finite = vec![];
        for &column in &fit.linear.finite.kept {
            finite.push(columns.len());
            columns.push((0..n).map(|i| fit.problem().raw[(i, column)]).collect());
        }
        Self {
            columns,
            left,
            right,
            finite,
        }
    }
    fn solve(&self, response: &[f64], fit: &irls::Fit<'_>) -> (Vec<f64>, Vec<f64>) {
        let expected = fit_explicit_ols(&ExplicitOlsProblem {
            outcome: response.to_vec(),
            regressors: self.columns.clone(),
            effects: vec![],
            weights: OracleWeights::Wls(fit.hessian_weights.clone()),
            clusters: vec![],
        })
        .unwrap();
        assert_eq!(expected.kept_regressors.len(), self.columns.len());
        (
            expected.coefficients.iter().map(|v| v.unwrap()).collect(),
            expected.fitted_values,
        )
    }
    fn compare_coefficients(
        &self,
        fit: &irls::Fit<'_>,
        actual: &[f64],
        expected: &[f64],
        seed: usize,
    ) {
        let topology = fit.problem().sample.topology.as_ref().unwrap();
        let first = topology.effect_partition(0).unwrap().n_levels();
        let second = topology.effect_partition(1).unwrap().n_levels();
        for effect in 0..2 {
            let partition = topology.effect_partition(effect).unwrap();
            for level in 0..partition.n_levels() {
                let row = partition.membership().members(level).unwrap()[0];
                // Read the original categorical value; the engine's dense codes
                // and component labels are not oracle identities.
                let source = topology.effects()[effect].categorical_sources[0].get() as usize - 1;
                let code = fit.problem().sample.regressors[source][row] as u32;
                let wanted = if effect == 0 {
                    expected[self.left[&code]]
                } else {
                    self.right[&code].map_or(0., |j| expected[j])
                };
                close(
                    actual[if effect == 0 { level } else { first + level }],
                    wanted,
                    seed,
                );
            }
        }
        for (j, &position) in self.finite.iter().enumerate() {
            close(actual[first + second + j], expected[position], seed);
        }
    }
}
pub fn compare(
    fit: &irls::Fit<'_>,
    columns: &CompiledColumns,
    effects: &[DummyEffect],
    team: &ThreadTeam,
    seed: usize,
) -> (Vec<Vec<f64>>, bias::Diagnostics) {
    let explicit = Explicit::new(effects, fit);
    let sample = fit.problem().sample;
    let offset: Vec<_> = match &sample.adjustment {
        PreparedAdjustment::None => vec![0.; sample.n_active()],
        PreparedAdjustment::Offset(v) => v.clone(),
        PreparedAdjustment::Exposure(v) => v.iter().map(|v| v.ln()).collect(),
    };
    let raw_response: Vec<_> = fit
        .point
        .eta
        .iter()
        .zip(&offset)
        .map(|(eta, offset)| eta - offset)
        .collect();
    let (raw_expected, _) = explicit.solve(&raw_response, fit);
    let a: Vec<_> = (0..sample.n_active())
        .map(|i| ((i * 17 + seed) % 13) as f64 * 0.15 - 0.7)
        .collect();
    let b: Vec<_> = (0..sample.n_active())
        .map(|i| (i as f64 * 1.7 + seed as f64).sin())
        .collect();
    let total: Vec<_> = a.iter().zip(&b).map(|(a, b)| a + b).collect();
    bias::with_information(fit, columns, team, &mut || Ok(()), |info, interrupt| {
        explicit.compare_coefficients(fit, info.raw(), &raw_expected, seed);
        assert_eq!(info.requested(), fit.options().solver);
        assert_eq!(info.finite_slots(), &[1]);
        let mut results = vec![info.raw().to_vec()];
        for response in [&a, &b, &total] {
            let (expected, expected_prediction) = explicit.solve(response, fit);
            let mut prediction = vec![f64::NAN; sample.n_active()];
            let mut coefficients = vec![f64::NAN; info.dimension()];
            info.solve(
                response,
                &mut prediction,
                Some(&mut coefficients),
                interrupt,
            )?;
            explicit.compare_coefficients(fit, &coefficients, &expected, seed);
            for (&a, &b) in prediction.iter().zip(&expected_prediction) {
                close(a, b, seed);
            }
            results.push(coefficients);
            results.push(prediction);
        }
        for coordinate in 0..info.dimension() {
            close(
                results[1][coordinate] + results[3][coordinate],
                results[5][coordinate],
                seed,
            );
        }
        for row in 0..sample.n_active() {
            close(results[2][row] + results[4][row], results[6][row], seed);
        }
        assert_eq!(info.diagnostics().solves, 3);
        assert_eq!(info.diagnostics().raw_recoveries, 1);
        assert!(info.diagnostics().maximum_score <= fit.options().tolerance);
        assert!(info.diagnostics().maximum_reconstruction <= fit.options().tolerance);
        Ok((results, info.diagnostics()))
    })
    .unwrap_or_else(|e| panic!("information seed={seed}/{:?}: {e}", fit.options().solver))
}
