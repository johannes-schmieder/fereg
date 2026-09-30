use super::{
    dual, filled, nullspace, reserve,
    tableau::{phase_one_cone, PhaseOneConeOutcome},
    DirectionProposal, Method, Model, SimplexEvidence, SimplexReport, Status, View,
};
use crate::{
    data::DenseMatrix,
    error::{ErrorCode, Result},
    ppml::{Interrupt, InterruptPoint},
    threads::compensated_sum,
};

pub(super) fn run(
    view: View<'_>,
    model: &Model<'_>,
    matrix: &DenseMatrix,
    mut report: SimplexReport,
    interrupt: &mut Interrupt<'_>,
) -> Result<SimplexReport> {
    let y = &view.sample().outcome;
    let mut positive = Vec::new();
    reserve(&mut positive, y.len(), "simplex positive rows")?;
    positive.extend(
        y.iter()
            .enumerate()
            .filter_map(|(row, &v)| (v > 0.).then_some(row)),
    );
    let mut boundary = Vec::new();
    reserve(&mut boundary, y.len(), "simplex boundary rows")?;
    boundary.extend(
        y.iter()
            .enumerate()
            .filter_map(|(row, &v)| (v == 0.).then_some(row)),
    );
    let nullspace::Nullspace {
        mut basis,
        mut cone,
    } = nullspace::reduce(
        matrix,
        &positive,
        &boundary,
        model.controls.tolerance,
        interrupt,
    )?;
    drop(positive);
    drop(boundary);
    if basis.is_empty() {
        report.status = Status::NoSeparation;
        report.evidence = Some(SimplexEvidence::FullPositiveRank {
            columns: matrix.ncols(),
        });
        return Ok(report);
    }
    // The free coordinates have arbitrary scale. Put nonzero boundary images
    // at unit strength so a harmless small contrast cannot make phase-I treat
    // its feasible cone as zero. This changes coordinates, not acceptance.
    normalize_cone(&mut cone, &mut basis)?;
    // Homogeneous sign checks have no unit floor: a small coefficient-space
    // image can be a perfectly valid normalized recession direction.
    for (column, direction) in basis.iter().enumerate() {
        interrupt(InterruptPoint::SimplexReduction)?;
        for sign in [1., -1.] {
            let minimum = cone
                .iter()
                .map(|row| sign * row[column])
                .fold(f64::INFINITY, f64::min);
            let maximum = cone
                .iter()
                .map(|row| sign * row[column])
                .fold(f64::NEG_INFINITY, f64::max);
            let scale = minimum.abs().max(maximum.abs());
            if scale > f64::MIN_POSITIVE
                && minimum >= -model.controls.tolerance * scale
                && maximum > 0.
            {
                let z = multiply(matrix, direction, -sign)?;
                match view.direction_proposal(Method::Simplex, 1, z, model.controls) {
                    Ok(proposal) => {
                        report.status = Status::Certified;
                        report.direction = Some(proposal);
                        return Ok(report);
                    }
                    Err(error) if error.code == ErrorCode::NonConvergence => (),
                    Err(error) => return Err(error),
                }
            }
        }
    }
    match phase_one_cone(
        &cone,
        report.limits.max_pivots,
        model.controls.tolerance,
        interrupt,
    )? {
        PhaseOneConeOutcome::Feasible {
            coordinates,
            iterations,
        } => {
            report.pivots = iterations;
            let gamma = combine_basis(&basis, &coordinates)?;
            let direction = multiply(matrix, &gamma, -1.)?;
            // A shortcut/zero-pivot solution is still one executed detector
            // pass; the actual phase-I pivot count is separately exact.
            let proposal: DirectionProposal = view.direction_proposal(
                Method::Simplex,
                iterations.max(1),
                direction,
                model.controls,
            )?;
            report.status = Status::Certified;
            report.direction = Some(proposal);
        }
        PhaseOneConeOutcome::Infeasible {
            dual: multipliers,
            iterations,
        } => {
            let certificate =
                dual::verify(&cone, &multipliers, model.controls.certificate_tolerance)?;
            report.pivots = iterations;
            report.status = Status::NoSeparation;
            report.evidence = Some(SimplexEvidence::DualInfeasibility {
                dual: multipliers,
                certificate,
            });
        }
    }
    Ok(report)
}
fn normalize_cone(cone: &mut [Vec<f64>], basis: &mut [Vec<f64>]) -> Result<()> {
    for (column, direction) in basis.iter_mut().enumerate() {
        let scale = cone.iter().fold(0_f64, |s, row| s.max(row[column].abs()));
        if !scale.is_finite() {
            return Err(super::failed("simplex boundary image is not finite"));
        }
        if scale == 0. {
            continue;
        }
        for row in cone.iter_mut() {
            row[column] /= scale;
        }
        for value in direction.iter_mut() {
            *value /= scale;
        }
        if direction.iter().any(|value| !value.is_finite()) {
            return Err(super::failed(
                "simplex boundary coordinate scale is not representable",
            ));
        }
    }
    Ok(())
}
fn combine_basis(basis: &[Vec<f64>], coordinates: &[f64]) -> Result<Vec<f64>> {
    let mut result = filled(basis[0].len(), 0., "simplex coefficient direction")?;
    for (column, value) in result.iter_mut().enumerate() {
        *value = compensated_sum(basis.iter().zip(coordinates).map(|(b, &c)| b[column] * c));
    }
    Ok(result)
}
fn multiply(matrix: &DenseMatrix, gamma: &[f64], sign: f64) -> Result<Vec<f64>> {
    let mut result = Vec::new();
    reserve(&mut result, matrix.nrows(), "simplex observation direction")?;
    result.extend((0..matrix.nrows()).map(|row| sign * matrix.row_combination(row, gamma, 0.)));
    if result.iter().any(|v| !v.is_finite()) {
        return Err(super::failed("simplex direction is not representable"));
    }
    Ok(result)
}
