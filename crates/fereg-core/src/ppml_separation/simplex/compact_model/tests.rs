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

fn disconnected_tables(separated: bool) -> (Vec<f64>, BinaryDesign<'static>) {
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
    assert_eq!(&*binary.row(0), &[0]);
    assert_eq!(&*binary.row(2), &[0, 1]);
}

#[test]
fn verifies_binary_model_with_aliased_columns_and_reversed_rows() {
    let (mut outcome, design) = disconnected_tables(false);
    let width = design.ncols();
    outcome.reverse();
    let alias = BinaryDesign::from_rows(outcome.len(), width + 1, 10_000, |i, row| {
        row.fill(0.);
        for &j in design.row(outcome.len() - 1 - i).iter() {
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
            row.extend_from_slice(&design.row(origins[i]));
            Ok(true)
        };
        let limit = 2000;
        let result = BinaryDesign::prepare(
            &outcome,
            (design.ncols(), 2, 2),
            (limit, &mut |_| None),
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
            (limit, &mut |_| None),
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
        (limit, &mut |_| None),
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

#[test]
fn adaptive_physical_group_index_preserves_the_compact_solver_limit() {
    let n = 1024;
    let limit = 256;
    let outcome: Vec<_> = (0..n).map(|i| f64::from(i % 4 < 2)).collect();
    let run = |(columns, cheap, ceiling), calls: &mut Vec<usize>| {
        BinaryDesign::prepare(
            &outcome,
            (columns, 2, 2),
            (cheap, &mut |peak| {
                calls.push(peak);
                (peak <= ceiling).then_some(ceiling)
            }),
            SeparationConfig {
                simplex_max_tableau_cells: cheap,
                ..config()
            },
            &mut 0,
            &mut |i, row| {
                match i % 4 {
                    0 => row.push(1),
                    1 => row.extend_from_slice(&[0, 2]),
                    _ => row.extend_from_slice(&[0, 1]),
                }
                Ok(true)
            },
            &mut || Ok(()),
        )
        .unwrap()
    };
    let sufficient = limit + n / 2;
    let mut calls = Vec::new();
    let minimum = n / 2 + 3;
    assert!(run((3, limit, minimum - 1), &mut calls).is_none());
    assert_eq!(calls, [minimum]);
    calls.clear();
    let Some(CompactInput::Proof(CompactModel::Direction { coefficients, .. })) =
        run((3, limit, sufficient - 32), &mut calls)
    else {
        panic!("admitted physical index did not uncover the grouped ray")
    };
    assert_eq!(calls, [minimum]);
    assert_eq!(coefficients[1], 0);
    assert_eq!(coefficients[0] + coefficients[2], 0);
    assert!(coefficients[0] + coefficients[1] < 0);
    calls.clear();
    assert!(run((32, limit, usize::MAX), &mut calls).is_none());
    assert!(
        calls.is_empty(),
        "the representative width square is still capped"
    );
    assert!(run((3, 8192, 8192), &mut calls).is_some());
    assert!(
        calls.is_empty(),
        "ordinary packed input needed no sparse extension"
    );
}

#[test]
fn repeated_dense_support_charges_observed_nonzeros_and_late_solver_scratch() {
    let n = 1024;
    let width = 8;
    let cheap = 840;
    let y: Vec<_> = (0..n).map(|i| f64::from(i % 64 < 32)).collect();
    let check = |ceiling, charges: &mut Vec<usize>| {
        BinaryDesign::prepare(
            &y,
            (width, width, width),
            (cheap, &mut |need| {
                charges.push(need);
                (need <= ceiling).then_some(need)
            }),
            SeparationConfig {
                simplex_max_tableau_cells: cheap,
                ..config()
            },
            &mut 0,
            &mut |i, row| {
                row.extend((0..5).filter(|j| (i >> j) & 1 != 0).map(|j| j as u32));
                row.extend_from_slice(&[5, 6]);
                if y[i] == 0. {
                    row.push(7);
                }
                Ok(true)
            },
            &mut || Ok(()),
        )
        .unwrap()
    };
    let mut charges = Vec::new();
    // The old estimate held + 4*distinct + 1 + width² = 833 fits.
    // The packed 320 distinct ones and physical-index overlap need 933.
    assert!(check(932, &mut charges).is_none());
    assert_eq!(charges, [933]);
    charges.clear();
    let Some(CompactInput::Proof(CompactModel::Direction { coefficients, .. })) =
        check(cheap + n / 2, &mut charges)
    else {
        panic!("the independently admitted physical index leaves the compact solve within its original allowance");
    };
    assert_eq!(charges[0], 933);
    assert!(charges.iter().any(|&p| p > 933));
    for i in 0..n {
        let score: i64 = coefficients[..5]
            .iter()
            .enumerate()
            .filter(|(j, _)| (i >> j) & 1 != 0)
            .map(|(_, v)| v)
            .sum::<i64>()
            + coefficients[5]
            + coefficients[6]
            + if y[i] == 0. { coefficients[7] } else { 0 };
        assert_eq!(score < 0, y[i] == 0.);
        assert!(score <= 0);
    }
}

/// Two coarse factors share a hidden order: `b`'s upper half never exceeds
/// `a`'s. The cross cell (a high, b low) contains only zero outcomes, but no
/// single level is zero-only. Every other pattern has a positive, so the
/// separated set is exactly that cell. `close` adds one positive to it.
struct HiddenCell {
    outcome: Vec<f64>,
    codes: [Vec<u32>; 4],
    x: Vec<bool>,
    separated: Vec<bool>,
}

const HIDDEN_LEVELS: [u32; 4] = [4, 4, 3, 2];

fn hidden_cell(copies: usize, close: bool) -> HiddenCell {
    let mut cell = HiddenCell {
        outcome: Vec::new(),
        codes: Default::default(),
        x: Vec::new(),
        separated: Vec::new(),
    };
    for copy in 0..copies {
        for a in 0..4u32 {
            for b in 0..4u32 {
                if a < 2 && b >= 2 {
                    continue;
                }
                for c in 0..3u32 {
                    for d in 0..2u32 {
                        let hidden = a >= 2 && b < 2;
                        let opened = close && copy == 0 && (a, b, c, d) == (2, 0, 0, 0);
                        for y in [0., 1.] {
                            if y > 0. && hidden && !opened {
                                continue;
                            }
                            cell.outcome.push(y);
                            for (codes, value) in cell.codes.iter_mut().zip([a, b, c, d]) {
                                codes.push(value);
                            }
                            cell.x.push(c == 2);
                            cell.separated.push(hidden && !close);
                        }
                    }
                }
            }
        }
    }
    cell
}

fn hidden_reader(cell: &HiddenCell) -> impl FnMut(usize, &mut Vec<u32>) -> Result<bool> + '_ {
    move |i, row| {
        row.push(0);
        if cell.x[i] {
            row.push(1);
        }
        let mut offset = 2;
        for (codes, levels) in cell.codes.iter().zip(HIDDEN_LEVELS) {
            row.push(offset + codes[i]);
            offset += levels;
        }
        Ok(true)
    }
}

fn hidden_effects(cell: &HiddenCell) -> Vec<StreamedEffect<'_>> {
    let mut offset = 2;
    cell.codes
        .iter()
        .zip(HIDDEN_LEVELS)
        .map(|(codes, levels)| {
            let effect = StreamedEffect {
                codes,
                offset,
                levels,
            };
            offset += levels;
            effect
        })
        .collect()
}

const HIDDEN_WIDTH: usize = 2 + 4 + 4 + 3 + 2;

#[test]
fn streamed_rows_equal_the_packed_reader_and_refuse_any_disagreement() {
    let cell = hidden_cell(3, false);
    let n = cell.outcome.len();
    let streamed = BinaryDesign::streamed(
        n,
        (HIDDEN_WIDTH, 2),
        hidden_effects(&cell),
        &mut hidden_reader(&cell),
        &mut || Ok(()),
    )
    .unwrap()
    .expect("binary intercept design streams");
    let mut reader = hidden_reader(&cell);
    let packed = BinaryDesign::from_rows(n, HIDDEN_WIDTH, 1 << 20, |i, dense| {
        dense.fill(0.);
        let mut row = Vec::new();
        reader(i, &mut row)?;
        for j in row {
            dense[j as usize] = 1.;
        }
        Ok(())
    })
    .unwrap()
    .unwrap();
    for i in 0..n {
        assert_eq!(*streamed.row(i), *packed.row(i));
    }
    assert!(streamed.cells() < packed.cells());
    // A reader that disagrees with one code, a nonbinary value, a missing
    // effect or more finite columns than one mask word all decline to stream.
    let mut honest = hidden_reader(&cell);
    let mut shifted = |i: usize, row: &mut Vec<u32>| {
        let ok = honest(i, row)?;
        if i == n / 2 {
            *row.last_mut().unwrap() ^= 1;
        }
        Ok(ok)
    };
    let refuse = |read: &mut dyn FnMut(usize, &mut Vec<u32>) -> Result<bool>, finite| {
        BinaryDesign::streamed(
            n,
            (HIDDEN_WIDTH, finite),
            hidden_effects(&cell),
            read,
            &mut || Ok(()),
        )
        .unwrap()
        .is_err()
    };
    assert!(refuse(&mut shifted, 2));
    assert!(refuse(&mut |_, _| Ok(false), 2));
    assert!(refuse(&mut hidden_reader(&cell), 3));
    assert!(BinaryDesign::streamed(
        n,
        (HIDDEN_WIDTH + 64, 2 + 64),
        hidden_effects(&cell),
        &mut hidden_reader(&cell),
        &mut || Ok(())
    )
    .unwrap()
    .is_err());
}

#[test]
fn scaled_streamed_search_finds_the_hidden_cell_the_fixed_allowance_refuses() {
    for close in [false, true] {
        let cell = hidden_cell(40, close);
        let n = cell.outcome.len();
        let design = BinaryDesign::streamed(
            n,
            (HIDDEN_WIDTH, 2),
            hidden_effects(&cell),
            &mut hidden_reader(&cell),
            &mut || Ok(()),
        )
        .unwrap()
        .ok()
        .unwrap();
        // The fixed allowance below the boundary index cannot even start.
        let limit = n / 2;
        let fixed = SeparationConfig {
            simplex_max_tableau_cells: limit,
            ..config()
        };
        assert!(
            compact_model(&cell.outcome, &design, fixed, &mut 0, &mut || Ok(()))
                .unwrap()
                .is_none()
        );
        let mut requests = Vec::new();
        let result = compact_model_scaled(
            &cell.outcome,
            &design,
            fixed,
            (limit, CompactWork::scaled(n, 6, HIDDEN_WIDTH), false),
            &mut 0,
            &mut || Ok(()),
            &mut |needed| {
                requests.push(needed);
                Some(needed)
            },
        )
        .unwrap();
        assert!(
            requests.iter().any(|&r| r > limit),
            "storage is requested by size"
        );
        match result {
            Some(CompactModel::Direction { coefficients, .. }) => {
                assert!(!close, "a closed cell must not produce a ray");
                for i in 0..n {
                    let score: i64 = design
                        .row(i)
                        .iter()
                        .map(|&j| coefficients[j as usize])
                        .sum();
                    assert!(score <= 0);
                    assert_eq!(score < 0, cell.separated[i]);
                    if cell.outcome[i] > 0. {
                        assert_eq!(score, 0);
                    }
                }
            }
            Some(CompactModel::Terminal(_) | CompactModel::NumericalDual { .. }) => {
                assert!(close, "the open hidden cell is separated")
            }
            Some(CompactModel::NumericalDirection { .. }) => {
                panic!("the hidden cell's ray has an exact integer lift")
            }
            None => panic!("scaled compact search left the hidden cell unresolved"),
        }
    }
}

#[test]
fn physical_dual_balances_every_original_column_on_a_closed_cell() {
    let cell = hidden_cell(5, true);
    let n = cell.outcome.len();
    let design = BinaryDesign::streamed(
        n,
        (HIDDEN_WIDTH, 2),
        hidden_effects(&cell),
        &mut hidden_reader(&cell),
        &mut || Ok(()),
    )
    .unwrap()
    .ok()
    .unwrap();
    let boundary: Vec<_> = (0..n).filter(|&i| cell.outcome[i] == 0.).collect();
    let weights = vec![1.; boundary.len()];
    let mut positive: Vec<_> = (0..n).filter(|&i| cell.outcome[i] > 0.).collect();
    positive_nullspace(&design, &mut positive, 1e-10, &mut || Ok(()))
        .unwrap()
        .unwrap();
    let dual = physical_dual(
        &design,
        &cell.outcome,
        &boundary,
        &weights,
        &positive,
        usize::MAX,
    )
    .unwrap()
    .expect("closed cell has a balancing positive dual");
    let mut moments = [0.; HIDDEN_WIDTH];
    let mut scale = 0.;
    for (i, &p) in dual.iter().enumerate() {
        if cell.outcome[i] == 0. {
            assert_eq!(p, 1.);
        }
        scale += p.abs();
        for &j in design.row(i).iter() {
            moments[j as usize] += p;
        }
    }
    assert!(
        moments.iter().all(|m| m.abs() <= 1e-10 * scale),
        "{moments:?}"
    );
    // An open cell has no balancing dual: the least-squares weights leave
    // the hidden contrast unbalanced, which the IR certificate must refuse.
    let open = hidden_cell(5, false);
    let open_design = BinaryDesign::streamed(
        open.outcome.len(),
        (HIDDEN_WIDTH, 2),
        hidden_effects(&open),
        &mut hidden_reader(&open),
        &mut || Ok(()),
    )
    .unwrap()
    .ok()
    .unwrap();
    let boundary: Vec<_> = (0..open.outcome.len())
        .filter(|&i| open.outcome[i] == 0.)
        .collect();
    let mut positive: Vec<_> = (0..open.outcome.len())
        .filter(|&i| open.outcome[i] > 0.)
        .collect();
    positive_nullspace(&open_design, &mut positive, 1e-10, &mut || Ok(()))
        .unwrap()
        .unwrap();
    let dual = physical_dual(
        &open_design,
        &open.outcome,
        &boundary,
        &vec![1.; boundary.len()],
        &positive,
        usize::MAX,
    )
    .unwrap()
    .unwrap();
    let mut moments = [0.; HIDDEN_WIDTH];
    for (i, &p) in dual.iter().enumerate() {
        for &j in open_design.row(i).iter() {
            moments[j as usize] += p;
        }
    }
    assert!(moments.iter().any(|m| m.abs() > 1e-3));
}

#[test]
fn valued_lane_separates_the_hidden_cell_beside_a_continuous_regressor() {
    for close in [false, true] {
        let cell = hidden_cell(6, close);
        let n = cell.outcome.len();
        // A continuous column that no exact ray may use.
        let z: Vec<f64> = (0..n).map(|i| ((i * 37 % 101) as f64 - 50.) / 7.).collect();
        let width = HIDDEN_WIDTH + 1;
        let read = |i: usize, f: &mut dyn FnMut(usize, f64)| -> Result<()> {
            f(0, 1.);
            if cell.x[i] {
                f(1, 1.);
            }
            let mut offset = 2;
            for (codes, levels) in cell.codes.iter().zip(HIDDEN_LEVELS) {
                f(offset + codes[i] as usize, 1.);
                offset += levels as usize;
            }
            f(HIDDEN_WIDTH, z[i]);
            Ok(())
        };
        let design = ValuedDesign {
            width,
            row_terms: 7,
            read: &read,
        };
        let mut search = ValuedSearch {
            outcome: &cell.outcome,
            design: &design,
            config: config(),
            work: CompactWork::scaled(n, 7, width),
            available: 1 << 20,
            held: 0,
            exact_dual: true,
        };
        let mut verify = |c: &[i64]| -> Result<bool> {
            if c[HIDDEN_WIDTH] != 0 {
                return Ok(false);
            }
            let mut strict = false;
            for i in 0..n {
                let mut score = i128::from(c[0]) + if cell.x[i] { i128::from(c[1]) } else { 0 };
                let mut offset = 2;
                for (codes, levels) in cell.codes.iter().zip(HIDDEN_LEVELS) {
                    score += i128::from(c[offset + codes[i] as usize]);
                    offset += levels as usize;
                }
                if score > 0 || (cell.outcome[i] > 0. && score != 0) {
                    return Ok(false);
                }
                strict |= score < 0;
            }
            Ok(strict)
        };
        // An independent exact check of a proposed integer dual: every zero
        // row positive and every coordinate's moment exactly zero.
        let mut exact = |dual: &ExactDual| -> Result<bool> {
            let mut weight = vec![None; n];
            for &(row, k) in &dual.integers {
                weight[row] = Some(dual.scale.mul_i64(k)?);
            }
            for (row, d) in &dual.basis {
                weight[*row] = Some(d.clone());
            }
            for (i, &y) in cell.outcome.iter().enumerate() {
                if y == 0. && weight[i].as_ref().is_none_or(|d| d.signum() <= 0) {
                    return Ok(false);
                }
            }
            for column in 0..width {
                let mut sum = Big::zero();
                for (i, d) in weight.iter().enumerate() {
                    let Some(d) = d else { continue };
                    let mut value = 0.;
                    read(i, &mut |j, v| {
                        if j == column {
                            value += v;
                        }
                    })?;
                    if value != 0. {
                        // Scale every value by 2^1100 to integers exactly.
                        let (m, e) = super::exact_dyadic::dyadic_for_tests(value);
                        sum = sum.add(&d.mul_i64(m)?.shl((e + 1100) as u32)?)?;
                    }
                }
                if !sum.is_zero() {
                    return Ok(false);
                }
            }
            Ok(true)
        };
        let result = search
            .run(
                &mut 0,
                &mut || Ok(()),
                &mut |needed| Some(needed),
                &mut Verifiers {
                    ray: &mut verify,
                    wide: &mut |_| Ok(false),
                    dual: &mut exact,
                },
            )
            .unwrap();
        match result {
            Some(ValuedModel::Direction { coefficients, .. }) => {
                assert!(!close);
                assert!(verify(&coefficients).unwrap());
            }
            Some(ValuedModel::Terminal(_)) => assert!(close, "only the closed cell has no ray"),
            Some(ValuedModel::WideDirection { .. }) => {
                panic!("the hidden cell's ray has a small integer lift")
            }
            Some(ValuedModel::NumericalDual { .. }) => {
                panic!("the closed cell has an exact rational dual")
            }
            Some(ValuedModel::NumericalDirection { .. }) => {
                panic!("the hidden cell's ray has an exact integer lift")
            }
            None => panic!("valued lane left the design unresolved (close = {close})"),
        }
        if close {
            continue;
        }
        // A refused exact replay leaves the float ray for the IR direction
        // certificate instead of an unresolved generation.
        let mut search = ValuedSearch {
            outcome: &cell.outcome,
            design: &design,
            config: config(),
            work: CompactWork::scaled(n, 7, width),
            available: 1 << 20,
            held: 0,
            exact_dual: true,
        };
        let refused = search
            .run(
                &mut 0,
                &mut || Ok(()),
                &mut |needed| Some(needed),
                &mut Verifiers {
                    ray: &mut |_| Ok(false),
                    wide: &mut |_| Ok(false),
                    dual: &mut |_| Ok(false),
                },
            )
            .unwrap();
        let Some(ValuedModel::NumericalDirection { coefficients, .. }) = refused else {
            panic!("a refused exact lift must return the numerical ray");
        };
        let scale = coefficients.iter().fold(0f64, |m, v| m.max(v.abs()));
        for i in 0..n {
            let mut score = 0.;
            read(i, &mut |j, v| score += v * coefficients[j]).unwrap();
            let score = score / scale;
            if cell.outcome[i] > 0. {
                assert!(score.abs() < 1e-9);
            } else {
                assert!(score < 1e-9);
            }
        }
        assert!(coefficients[HIDDEN_WIDTH].abs() < 1e-9 * scale);
    }
}
