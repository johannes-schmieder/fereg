//! Proposal-only transposed phase I for a short, wide system of boundary constraints.
//! For boundary rows D_i, seek D' u = -D' 1, u >= 0. A feasible basis gives
//! positive balancing weights 1 + u. An infeasible basis can supply y with
//! D y >= 0 and at least one strict inequality. Return -y in the input axes.
//! The caller must still certify either outcome against its full raw design.

use super::fallible_filled;
use crate::error::Result;
use crate::linalg::neumaier_add;

#[derive(Debug)]
pub(crate) enum Proposal {
    Direction {
        coordinates: Vec<f64>,
        iterations: usize,
    },
    PositiveWeights {
        weights: Vec<f64>,
        iterations: usize,
    },
}

/// Includes the tableau, the pricing row, signed axis scales, basic indices,
/// and the larger of the two possible output vectors. All sizes are eight-byte
/// cells, additional to the caller's immutable cone and other live storage.
pub(crate) fn workspace_cells(boundary: usize, dimension: usize) -> Option<usize> {
    let width = boundary.checked_add(dimension)?.checked_add(1)?;
    width
        .checked_mul(dimension.checked_add(1)?)?
        .checked_add(dimension.checked_mul(2)?)?
        .checked_add(boundary.max(dimension))
}

pub(crate) fn propose(
    cone: &[Vec<f64>],
    tolerance: f64,
    max_iterations: usize,
    available_cells: usize,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Proposal>> {
    let boundary = cone.len();
    let dimension = cone.first().map_or(0, Vec::len);
    if dimension == 0 || !(0.0..1.0).contains(&tolerance) || tolerance == 0.0 {
        return Ok(None);
    }
    if workspace_cells(boundary, dimension).is_none_or(|cells| cells > available_cells) {
        return Ok(None);
    }
    let width = boundary + dimension + 1;
    let cells = width * dimension;
    // Count dense cell visits independently of allocation and the requested
    // pivot limit. Setup and the terminal pass have sixteen tableau traversals.
    // A pivot prices the basic rows once, updates and finite-checks the
    // tableau, and may check the original cone once if its mass is near zero.
    // Pricing scratch, output weights, normalization, and ratio/basis scans
    // cost at most eight row widths and four basis lengths beyond those passes.
    const CELL_VISITS: usize = 1 << 30;
    if cells > CELL_VISITS / 16 || max_iterations == 0 {
        return Ok(None);
    }
    let per_pivot = 4 * cells + 8 * width + 4 * dimension;
    let pivot_limit = max_iterations.min((CELL_VISITS - 16 * cells) / per_pivot);
    interrupt()?;
    let mut tableau = fallible_filled(cells, 0., "unable to reserve transposed simplex tableau")?;
    let mut scales = fallible_filled(dimension, 0., "unable to reserve transposed simplex scales")?;
    if !initialize(cone, width, &mut tableau, &mut scales, interrupt)? {
        return Ok(None);
    }
    let mut basis = fallible_filled(
        dimension,
        0usize,
        "unable to reserve transposed simplex basis",
    )?;
    for (axis, basic) in basis.iter_mut().enumerate() {
        *basic = boundary + axis;
    }
    let mut reduced = fallible_filled(width, 0., "unable to reserve transposed simplex pricing")?;
    let mut degenerate = 0usize;
    let mut bland = false;
    for iterations in 0..=pivot_limit {
        interrupt()?;
        // For objective -sum(artificials), c_B is -1 precisely on basic
        // artificial variables. Their tableau rows also yield -c_B' B^{-1}
        // from the artificial identity block, in the signed, scaled equations.
        reduced.fill(0.);
        for (row, &basic) in tableau.chunks_exact(width).zip(&basis) {
            if basic >= boundary {
                for (dest, &value) in reduced.iter_mut().zip(row) {
                    *dest += value;
                }
            }
        }
        if reduced.iter().any(|v| !v.is_finite()) {
            return Ok(None);
        }
        for &basic in &basis {
            if basic < boundary {
                reduced[basic] = 0.;
            }
        }
        let objective = reduced[width - 1];
        if objective.abs() <= tolerance {
            if let Some(weights) =
                positive_weights(cone, &scales, &tableau, &basis, tolerance, interrupt)?
            {
                return Ok(Some(Proposal::PositiveWeights {
                    weights,
                    iterations,
                }));
            }
        }
        let Some(column) = entering_column(&reduced[..boundary], tolerance, bland) else {
            return Ok(direction(
                cone,
                &scales,
                &reduced[boundary..width - 1],
                tolerance,
                interrupt,
            )?
            .map(|coordinates| Proposal::Direction {
                coordinates,
                iterations,
            }));
        };
        let Some((leaving, step)) = leaving_row(&tableau, &basis, width, column, tolerance) else {
            return Ok(None);
        };
        if iterations == pivot_limit || !pivot(&mut tableau, width, leaving, column) {
            return Ok(None);
        }
        basis[leaving] = column;
        if step <= tolerance {
            degenerate += 1;
            bland |= degenerate >= dimension;
        } else {
            degenerate = 0;
        }
    }
    Ok(None)
}

fn initialize(
    cone: &[Vec<f64>],
    width: usize,
    tableau: &mut [f64],
    scales: &mut [f64],
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<bool> {
    let boundary = cone.len();
    for (index, row) in cone.iter().enumerate() {
        crate::interrupt::row_checkpoint(index)?;
        if row.len() != scales.len() || row.iter().any(|v| !v.is_finite()) {
            return Ok(false);
        }
        for (scale, &value) in scales.iter_mut().zip(row) {
            *scale = scale.max(value.abs());
        }
    }
    for (axis, (row, scale)) in tableau.chunks_exact_mut(width).zip(scales).enumerate() {
        interrupt()?;
        // A power-of-two divisor preserves small integer and dyadic equations.
        let bits = scale.to_bits();
        *scale = if bits == 0 {
            1.
        } else if bits < (1 << 52) {
            f64::from_bits(1 << (63 - bits.leading_zeros()))
        } else {
            f64::from_bits(bits & 0x7ff0_0000_0000_0000)
        };
        let mut sum = 0.;
        let mut correction = 0.;
        for (value, original) in row.iter_mut().zip(cone) {
            *value = original[axis] / *scale;
            neumaier_add(*value, &mut sum, &mut correction);
        }
        let rhs = -(sum + correction);
        if rhs < 0. {
            *scale = -*scale;
            for value in &mut row[..boundary] {
                *value = -*value;
            }
        }
        row[boundary + axis] = 1.;
        row[width - 1] = rhs.abs();
    }
    Ok(true)
}

/// Never reintroduce a nonbasic artificial. At termination original-column
/// reduced costs and the original-cone checks suffice for either proposal.
fn entering_column(reduced: &[f64], tolerance: f64, bland: bool) -> Option<usize> {
    let mut entering = None;
    let mut largest = tolerance;
    for (i, &value) in reduced.iter().enumerate() {
        if value > largest {
            entering = Some(i);
            largest = value;
            if bland {
                break;
            }
        }
    }
    entering
}

fn leaving_row(
    tableau: &[f64],
    basis: &[usize],
    width: usize,
    column: usize,
    tolerance: f64,
) -> Option<(usize, f64)> {
    let mut best = None;
    let mut best_ratio = f64::INFINITY;
    let mut best_index = usize::MAX;
    for (index, (row, &basic)) in tableau.chunks_exact(width).zip(basis).enumerate() {
        if row[width - 1] < -tolerance {
            return None;
        }
        if row[column] > tolerance {
            let ratio = row[width - 1].max(0.) / row[column];
            if ratio.is_finite()
                && (ratio < best_ratio || (ratio == best_ratio && basic < best_index))
            {
                best = Some(index);
                best_ratio = ratio;
                best_index = basic;
            }
        }
    }
    best.map(|i| (i, best_ratio))
}

fn pivot(tableau: &mut [f64], width: usize, leaving: usize, column: usize) -> bool {
    let (before, tail) = tableau.split_at_mut(leaving * width);
    let (pivot, after) = tail.split_at_mut(width);
    let diagonal = pivot[column];
    for value in pivot.iter_mut() {
        *value /= diagonal;
    }
    pivot[column] = 1.;
    for row in before
        .chunks_exact_mut(width)
        .chain(after.chunks_exact_mut(width))
    {
        let factor = row[column];
        if factor != 0. {
            for (value, &source) in row.iter_mut().zip(pivot.iter()) {
                *value -= factor * source;
            }
            row[column] = 0.;
        }
    }
    tableau.iter().all(|v| v.is_finite())
}

fn positive_weights(
    cone: &[Vec<f64>],
    scales: &[f64],
    tableau: &[f64],
    basis: &[usize],
    tolerance: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<f64>>> {
    let width = cone.len() + scales.len() + 1;
    let mut weights = fallible_filled(
        cone.len(),
        1.,
        "unable to reserve transposed simplex weights",
    )?;
    for (row, &basic) in tableau.chunks_exact(width).zip(basis) {
        if row[width - 1] < -tolerance {
            return Ok(None);
        }
        if basic < cone.len() {
            weights[basic] += row[width - 1].max(0.);
        }
    }
    let largest = weights.iter().copied().fold(1., f64::max);
    for (axis, &scale) in scales.iter().enumerate() {
        interrupt()?;
        let mut sum = 0.;
        let mut correction = 0.;
        let mut magnitude = 0.;
        for (row, &weight) in cone.iter().zip(&weights) {
            let term = (row[axis] / scale) * (weight / largest);
            neumaier_add(term, &mut sum, &mut correction);
            magnitude += term.abs();
        }
        if (sum + correction).abs() > tolerance * magnitude {
            return Ok(None);
        }
    }
    Ok(Some(weights))
}

fn direction(
    cone: &[Vec<f64>],
    scales: &[f64],
    transformed: &[f64],
    tolerance: f64,
    interrupt: &mut dyn FnMut() -> Result<()>,
) -> Result<Option<Vec<f64>>> {
    // Common positive factors keep original-axis coordinates representable
    // even for very small equality scales. Test the resulting orientation
    // afresh on each untransformed cone row.
    let lambda = transformed.iter().fold(0.0_f64, |a, &v| a.max(v.abs()));
    if lambda == 0. {
        return Ok(None);
    }
    let smallest = transformed
        .iter()
        .zip(scales)
        .filter_map(|(&v, &scale)| (v != 0.).then_some(scale.abs()))
        .fold(f64::INFINITY, f64::min);
    let mut coordinates = fallible_filled(
        scales.len(),
        0.,
        "unable to reserve transposed simplex direction",
    )?;
    for ((c, &v), &scale) in coordinates.iter_mut().zip(transformed).zip(scales) {
        if v != 0. {
            *c = (v / lambda) * (smallest / scale);
        }
    }
    let mut negative = 0.0_f64;
    let mut positive = 0.0_f64;
    for (i, row) in cone.iter().enumerate() {
        if i % crate::interrupt::CHECKPOINT_ROW_STRIDE == 0 {
            interrupt()?;
        }
        let score = row
            .iter()
            .zip(&coordinates)
            .fold(0., |sum, (&v, &c)| v.mul_add(c, sum));
        if !score.is_finite() {
            return Ok(None);
        }
        negative = negative.max(-score);
        positive = positive.max(score);
    }
    Ok((negative > 0. && positive <= tolerance * negative).then_some(coordinates))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solve(cone: &[Vec<f64>]) -> Option<Proposal> {
        propose(cone, 1e-10, 64, usize::MAX, &mut || Ok(())).unwrap()
    }

    #[test]
    fn inconsistent_positive_balance_supplies_an_original_negative_direction() {
        // Both coordinate columns take both signs. A positive combination of
        // their negatives vanishes on the first pair and decreases the third.
        for scale in [2.0_f64.powi(-100), 1., 2.0_f64.powi(100)] {
            let cone = vec![vec![1., -scale], vec![-1., scale], vec![2., -scale]];
            let Some(Proposal::Direction {
                coordinates,
                iterations,
            }) = solve(&cone)
            else {
                panic!("expected Farkas direction")
            };
            assert!(iterations <= 64);
            let scores: Vec<_> = cone
                .iter()
                .map(|r| r[0] * coordinates[0] + r[1] * coordinates[1])
                .collect();
            assert!(scores[2] < 0.);
            assert!(scores[..2].iter().all(|&v| v.abs() <= -1e-12 * scores[2]));
        }
    }

    #[test]
    fn positive_multiple_equations_duplicate_axes_and_zero_equalities() {
        let cone = vec![vec![3., 3., 0.], vec![-1., -1., 0.]];
        let Some(Proposal::PositiveWeights {
            weights,
            iterations,
        }) = solve(&cone)
        else {
            panic!("expected positive boundary weights")
        };
        assert_eq!(iterations, 1);
        assert!(weights.iter().all(|&w| w >= 1.));
        assert_eq!(weights[0] * 3., weights[1]);
        let Some(Proposal::PositiveWeights {
            weights,
            iterations,
        }) = solve(&[vec![0.], vec![-0.]])
        else {
            panic!("all zero equations are already balanced")
        };
        assert_eq!(iterations, 0);
        assert_eq!(weights, [1., 1.]);
    }

    #[test]
    fn integer_planar_cones_agree_with_independent_ray_enumeration() {
        // In the plane an extreme ray is perpendicular to a constraint.
        // Include row directions for the rank-one half-plane case.
        let mut state = 13u32;
        for sample in 0..96 {
            let mut draw = || {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                ((state >> 16) % 7) as i64 - 3
            };
            let rows: Vec<_> = (0..(3 + sample % 6)).map(|_| [draw(), draw()]).collect();
            let separates = rows.iter().any(|&[x, y]| {
                [[x, y], [-x, -y], [y, -x], [-y, x]].iter().any(|&[a, b]| {
                    let mut strict = false;
                    for &[x, y] in &rows {
                        let score = a * x + b * y;
                        if score > 0 {
                            return false;
                        }
                        strict |= score < 0;
                    }
                    strict
                })
            });
            let cone: Vec<_> = rows
                .iter()
                .map(|r| r.iter().map(|&x| x as f64).collect())
                .collect();
            match solve(&cone).expect("small integer cone must resolve") {
                Proposal::Direction { .. } => assert!(separates, "{rows:?}"),
                Proposal::PositiveWeights { .. } => assert!(!separates, "{rows:?}"),
            }
        }
    }

    #[test]
    fn both_branches_check_resources_alignment_and_interrupts() {
        let cone = vec![vec![3., -2.], vec![-1., 0.], vec![0., 1.]];
        let cells = workspace_cells(3, 2).unwrap();
        let run = |cells, limit| propose(&cone, 1e-10, limit, cells, &mut || Ok(())).unwrap();
        assert!(run(cells - 1, 64).is_none());
        assert!(run(cells, 1).is_none());
        assert!(matches!(
            run(cells, 2),
            Some(Proposal::PositiveWeights { .. })
        ));
        assert!(workspace_cells(usize::MAX, 1).is_none());
        assert!(solve(&[vec![1., 2.], vec![1.]]).is_none());
        assert!(solve(&[vec![1.], vec![f64::NAN]]).is_none());
        let interrupted = propose(&cone, 1e-10, 64, cells, &mut || {
            Err(crate::interrupt::interrupted())
        });
        assert_eq!(
            interrupted.unwrap_err().code,
            crate::error::ErrorCode::Interrupted
        );
    }
}
