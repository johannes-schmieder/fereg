use super::*;
use crate::ppml_separation::SeparationConfig;

fn config() -> SeparationConfig {
    SeparationConfig {
        certificate_tolerance: 1e-8,
        simplex_max_iterations: 1000,
        simplex_max_tableau_cells: 10_000,
        ..SeparationConfig::default()
    }
}

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
    let proposal = compact_model(&outcome, &design, config(), &mut 0, &mut || Ok(())).unwrap();
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
    let proposal = compact_model(&outcome, &design, config(), &mut 0, &mut || Ok(())).unwrap();
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
        compact_model(&outcome, &alias, config(), &mut 0, &mut || Ok(())).unwrap(),
        Some(CompactModel::Terminal(_))
    ));
}

#[test]
fn grouped_proofs_cover_every_copy_with_unequal_multiplicities() {
    use super::grouped::CompactInput;
    for separated in [true, false] {
        let (original, design) = disconnected_tables(separated);
        // Interleave groups rather than relying on adjacent repeated rows.
        let origins: Vec<_> = (0..4)
            .flat_map(|copy| (0..original.len()).rev().filter(move |i| copy <= i % 4))
            .collect();
        let outcome: Vec<_> = origins.iter().map(|&i| original[i]).collect();
        let mut read = |i: usize, row: &mut Vec<u32>| {
            row.extend_from_slice(design.row(origins[i]));
            Ok(true)
        };
        let limit = 2000;
        let result = BinaryDesign::prepare(
            &outcome,
            (design.ncols(), 2, 2),
            limit,
            SeparationConfig {
                simplex_max_tableau_cells: limit,
                ..config()
            },
            &mut 0,
            &mut read,
            &mut || Ok(()),
        )
        .unwrap();
        match result {
            Some(CompactInput::Proof(CompactModel::Direction { coefficients, .. }))
                if separated =>
            {
                for &i in &origins {
                    let score: i64 = design
                        .row(i)
                        .iter()
                        .map(|&j| coefficients[j as usize])
                        .sum();
                    assert_eq!(score < 0, (8..38).contains(&i));
                    assert!(score <= 0);
                }
            }
            Some(CompactInput::Proof(CompactModel::Terminal(proposal))) if !separated => {
                assert!(matches!(proposal.certificate.unwrap().proof,
                    SeparationCertificateProof::SimplexDualInfeasibility { boundary_rows, .. }
                      if boundary_rows == outcome.iter().filter(|&&y| y == 0.).count()));
            }
            _ => panic!("expected a certified result from the grouped raw design"),
        }
    }
}

#[test]
fn distinct_input_reuses_identification_scan_and_small_input_keeps_the_packed_route() {
    for (n, distinct, limit) in [(512, true, 3000), (16, false, 10_000)] {
        let width = 9;
        let outcome: Vec<_> = (0..n).map(|i| f64::from(i % 3 == 0)).collect();
        let mut reads = vec![0; n];
        let input = BinaryDesign::prepare(
            &outcome,
            (width, width, width),
            limit,
            SeparationConfig {
                simplex_max_tableau_cells: limit,
                ..config()
            },
            &mut 0,
            &mut |i, row| {
                reads[i] += 1;
                let bits = if distinct { i } else { i % 2 };
                row.extend(
                    (0..width)
                        .filter(|j| bits & (1 << j) != 0)
                        .map(|j| j as u32),
                );
                Ok(true)
            },
            &mut || Ok(()),
        )
        .unwrap();
        assert!(matches!(input, Some(CompactInput::Design(_))));
        assert!(reads.iter().all(|&r| r == 2));
    }
}

#[test]
fn sparse_grouping_exceeds_dense_scan_limit_and_separates_only_outside_positive_span() {
    let n = 133_000;
    let width = 512;
    let limit = 1_000_000;
    assert!(2 * n * width > MAX_WORK);
    let outcome: Vec<_> = (0..n).map(|i| f64::from(i % 3 == 0)).collect();
    let input = BinaryDesign::prepare(
        &outcome,
        (width, 1, 1),
        limit,
        SeparationConfig {
            simplex_max_tableau_cells: limit,
            ..config()
        },
        &mut 0,
        &mut |i, row| {
            // The first coordinate appears with both signs of the outcome.
            row.push(if i % 3 == 2 { width as u32 - 1 } else { 0 });
            Ok(true)
        },
        &mut || Ok(()),
    )
    .unwrap();
    let Some(CompactInput::Proof(CompactModel::Direction { coefficients, .. })) = input else {
        panic!("expected a ray on the third row class");
    };
    assert_eq!(coefficients[0], 0);
    assert!(coefficients[width - 1] < 0);
}
