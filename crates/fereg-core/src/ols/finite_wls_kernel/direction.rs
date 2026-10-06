//! A fixed-basis direction borrows its caller's admitted numerical banks.
use super::*;
use crate::data::DenseMatrix;

pub(crate) struct FiniteDirectionWorkspace<'a> {
    pub(crate) gram: &'a mut [f64],
    pub(crate) rhs: &'a mut [f64],
    pub(crate) factor: &'a mut [f64],
    pub(crate) forward: &'a mut [f64],
    pub(crate) solution: &'a mut [f64],
    pub(crate) scales: &'a mut [f64],
    pub(crate) coefficients: &'a mut [f64],
    pub(crate) permutation: &'a mut [usize],
}

/// Projected controls precede the working response. Rank is already frozen;
/// a failed factor must return to the caller's independently certified policy.
pub(crate) fn solve_finite_direction(
    projected: &DenseMatrix,
    weights: &[f64],
    tolerance: f64,
    workspace: FiniteDirectionWorkspace<'_>,
) -> Result<()> {
    let columns = workspace.coefficients.len();
    if projected.nrows() != weights.len()
        || projected.ncols() != columns + 1
        || workspace.scales.len() != columns
        || !tolerance.is_finite()
        || tolerance <= 0.0
    {
        return Err(invalid_argument(
            "finite direction dimensions or tolerance differ",
        ));
    }
    if columns == 0 {
        return Ok(());
    }
    let source = FixedBlockMomentSource::Dense {
        values: FiniteDenseRows::prefix(projected, columns),
        rows: projected.nrows(),
        stored_columns: columns,
        selected_columns: None,
        response: FiniteResponse::Column(projected, columns),
        weights: BorrowedMapWeights::Provided(weights),
        solver_weight_divisor: 1.0,
        column_scales: None,
    };
    // Dense retained moments need no per-worker Gram or observation bank.
    let layout = finite_wls_moment_layout(source)?;
    fill_finite_wls_moments(source, layout, &mut [], workspace.rhs, workspace.gram)?;
    for column in 0..columns {
        let scale = workspace.gram[column * columns + column].sqrt();
        if !scale.is_finite() || scale <= 0.0 {
            return Err(invalid_data("finite direction basis lost positive norm"));
        }
        workspace.scales[column] = scale;
    }
    for row in 0..columns {
        workspace.rhs[row] /= workspace.scales[row];
        for column in 0..columns {
            workspace.gram[row * columns + column] /=
                workspace.scales[row] * workspace.scales[column];
        }
    }
    if factor_guarded_cholesky_in_place(
        workspace.gram,
        workspace.rhs,
        workspace.factor,
        workspace.permutation,
        CholeskyPivotPolicy::MaximumDiagonal,
        tolerance.max(64.0 * f64::EPSILON).powi(2),
    )?
    .is_none()
    {
        return Err(invalid_data("finite direction basis lost rank"));
    }
    if !solve_guarded_cholesky(
        workspace.factor,
        workspace.rhs,
        workspace.forward,
        workspace.solution,
    )? {
        return Err(invalid_data("finite direction solve is nonfinite"));
    }
    for position in 0..columns {
        let original = workspace.permutation[position];
        workspace.coefficients[original] =
            workspace.solution[position] / workspace.scales[original];
    }
    if workspace
        .coefficients
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(invalid_data("finite direction coefficients are nonfinite"));
    }
    Ok(())
}
