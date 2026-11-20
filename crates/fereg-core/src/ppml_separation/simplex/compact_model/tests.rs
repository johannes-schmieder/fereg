use super::*;

fn disconnected_tables(separated: bool) -> (Vec<f64>, BinaryDesign) {
    let mut outcome = Vec::new();
    let mut values = Vec::new();
    let width = 12;
    for block in 0..3 {
        let repeats = if separated && block == 0 { 0 } else { 1 };
        for (left, right, y, n) in [
            (0, 0, 1., 4),
            (1, 1, 1., 4),
            (0, 1, 0., 30),
            (1, 0, 0., 10 * repeats),
        ] {
            for _ in 0..n {
                outcome.push(y);
                let start = values.len();
                values.resize(start + width, 0.);
                values[start + block * 4 + left] = 1.;
                values[start + block * 4 + 2 + right] = 1.;
            }
        }
    }
    let rows = outcome.len();
    (
        outcome,
        BinaryDesign::from_rows(rows, width, 10_000, |i, row| {
            row.copy_from_slice(&values[i * width..(i + 1) * width]);
            Ok(())
        })
        .unwrap()
        .unwrap(),
    )
}

#[test]
fn compresses_a_strict_dual_even_if_positive_rows_do_not_span_the_model() {
    let (outcome, design) = disconnected_tables(false);
    let proposal =
        compact_model(&outcome, &design, 1e-10, 1e-8, 1000, 10_000, &mut || Ok(())).unwrap();
    let Some(CompactModel::Terminal(terminal)) = proposal else {
        panic!("expected a raw-design dual on the three unbalanced tables");
    };
    assert!(matches!(
        terminal.certificate.unwrap().proof,
        SeparationCertificateProof::SimplexDualInfeasibility { .. }
    ));
}

#[test]
fn certifies_a_strict_ray_with_no_deletions_elsewhere() {
    let (outcome, design) = disconnected_tables(true);
    let proposal =
        compact_model(&outcome, &design, 1e-10, 1e-8, 1000, 10_000, &mut || Ok(())).unwrap();
    let Some(CompactModel::Direction { coefficients, .. }) = proposal else {
        panic!("expected a raw-design separating direction");
    };
    for i in 0..outcome.len() {
        let score: i64 = design
            .row(i)
            .iter()
            .map(|&j| coefficients[j as usize])
            .sum();
        assert_eq!(score < 0, (8..38).contains(&i));
        assert!(score <= 0);
    }
}

#[test]
fn binary_admission_respects_the_original_values_and_cell_limit() {
    let too_small = BinaryDesign::from_rows(3, 2, 8, |_, row| {
        row.copy_from_slice(&[1., 1.]);
        Ok(())
    })
    .unwrap();
    assert!(too_small.is_none());

    let nonbinary = BinaryDesign::from_rows(3, 2, 64, |_, row| {
        row.copy_from_slice(&[1., 1. - f64::EPSILON]);
        Ok(())
    })
    .unwrap();
    assert!(nonbinary.is_none());

    let binary = BinaryDesign::from_rows(3, 2, 12, |i, row| {
        row.copy_from_slice(&[1., if i > 0 { 1. } else { 0. }]);
        Ok(())
    })
    .unwrap()
    .unwrap();
    assert_eq!(binary.row(0), &[0]);
    assert_eq!(binary.row(2), &[0, 1]);
}

#[test]
fn verifies_binary_model_with_aliased_columns_and_reversed_rows() {
    let (mut outcome, design) = disconnected_tables(false);
    let width = design.ncols();
    outcome.reverse();
    let alias = BinaryDesign::from_rows(outcome.len(), width + 1, 10_000, |i, row| {
        row.fill(0.);
        for &j in design.row(outcome.len() - 1 - i) {
            row[j as usize] = 1.;
        }
        row[width] = row[1];
        Ok(())
    })
    .unwrap()
    .unwrap();
    assert!(matches!(
        compact_model(&outcome, &alias, 1e-10, 1e-8, 1000, 10_000, &mut || Ok(())).unwrap(),
        Some(CompactModel::Terminal(_))
    ));
}
