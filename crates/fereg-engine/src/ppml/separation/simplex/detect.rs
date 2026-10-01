use super::{
    dual, filled, nullspace, reserve,
    tableau::{phase_one_cone, PhaseOneConeOutcome},
    DirectionProposal, Method, Model, SimplexEvidence, SimplexReport, Status, View,
};
use crate::{
    data::DenseMatrix,
    error::{ErrorCode, Result},
    execution::{self, Poll, WorkPoll},
    ppml::{Interrupt, InterruptPoint},
};

pub(super) fn run(
    view: View<'_>,
    model: &Model<'_>,
    matrix: &DenseMatrix,
    mut report: SimplexReport,
    interrupt: &mut Interrupt<'_>,
) -> Result<SimplexReport> {
    let poll = model.probe.dense();
    let mut work = WorkPoll::new(poll)?;
    let y = &view.sample().outcome;
    let mut positive = Vec::new();
    reserve(&mut positive, y.len(), "simplex positive rows")?;
    for (row, &v) in y.iter().enumerate() {
        work.step()?;
        if v > 0. {
            positive.push(row);
        }
    }
    let mut boundary = Vec::new();
    reserve(&mut boundary, y.len(), "simplex boundary rows")?;
    for (row, &v) in y.iter().enumerate() {
        work.step()?;
        if v == 0. {
            boundary.push(row);
        }
    }
    let nullspace::Nullspace {
        mut basis,
        mut cone,
    } = nullspace::reduce(
        matrix,
        &positive,
        &boundary,
        model.controls.tolerance,
        interrupt,
        poll,
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
    normalize_cone(&mut cone, &mut basis, poll)?;
    // Homogeneous sign checks have no unit floor: a small coefficient-space
    // image can be a perfectly valid normalized recession direction.
    for (column, direction) in basis.iter().enumerate() {
        interrupt(InterruptPoint::SimplexReduction)?;
        for sign in [1., -1.] {
            execution::check(poll)?;
            let minimum = execution::fold(
                cone.iter().map(|row| sign * row[column]),
                f64::INFINITY,
                poll,
                f64::min,
            )?;
            let maximum = execution::fold(
                cone.iter().map(|row| sign * row[column]),
                f64::NEG_INFINITY,
                poll,
                f64::max,
            )?;
            let scale = minimum.abs().max(maximum.abs());
            if scale > f64::MIN_POSITIVE
                && minimum >= -model.controls.tolerance * scale
                && maximum > 0.
            {
                let z = multiply(matrix, direction, -sign, poll)?;
                match view.direction_proposal_polled(
                    Method::Simplex,
                    1,
                    z,
                    model.controls,
                    model.probe.dense(),
                ) {
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
        poll,
    )? {
        PhaseOneConeOutcome::Feasible {
            coordinates,
            iterations,
        } => {
            report.pivots = iterations;
            let gamma = combine_basis(&basis, &coordinates, poll)?;
            let direction = multiply(matrix, &gamma, -1., poll)?;
            // A shortcut/zero-pivot solution is still one executed detector
            // pass; the actual phase-I pivot count is separately exact.
            let proposal: DirectionProposal = view.direction_proposal_polled(
                Method::Simplex,
                iterations.max(1),
                direction,
                model.controls,
                model.probe.dense(),
            )?;
            report.status = Status::Certified;
            report.direction = Some(proposal);
        }
        PhaseOneConeOutcome::Infeasible {
            dual: multipliers,
            iterations,
        } => {
            let certificate = dual::verify(
                &cone,
                &multipliers,
                model.controls.certificate_tolerance,
                poll,
            )?;
            report.pivots = iterations;
            report.status = Status::NoSeparation;
            report.evidence = Some(SimplexEvidence::DualInfeasibility {
                dual: multipliers,
                certificate,
            });
        }
    }
    execution::check(poll)?;
    Ok(report)
}
pub(super) fn normalize_cone(
    cone: &mut [Vec<f64>],
    basis: &mut [Vec<f64>],
    poll: Poll<'_>,
) -> Result<()> {
    let mut work = WorkPoll::new(poll)?;
    for (column, direction) in basis.iter_mut().enumerate() {
        work.step()?;
        let mut scale = 0_f64;
        for row in cone.iter() {
            work.step()?;
            scale = scale.max(row[column].abs());
        }
        if !scale.is_finite() {
            return Err(super::failed("simplex boundary image is not finite"));
        }
        if scale == 0. {
            continue;
        }
        for row in cone.iter_mut() {
            work.step()?;
            row[column] /= scale;
        }
        for value in direction.iter_mut() {
            work.step()?;
            *value /= scale;
        }
        if execution::any(direction.iter(), |value| !value.is_finite(), poll)? {
            return Err(super::failed(
                "simplex boundary coordinate scale is not representable",
            ));
        }
    }
    execution::check(poll)?;
    Ok(())
}
pub(super) fn combine_basis(
    basis: &[Vec<f64>],
    coordinates: &[f64],
    poll: Poll<'_>,
) -> Result<Vec<f64>> {
    let mut work = WorkPoll::new(poll)?;
    let mut result = filled(basis[0].len(), 0., "simplex coefficient direction", poll)?;
    for (column, value) in result.iter_mut().enumerate() {
        *value = execution::sum(
            basis.iter().zip(coordinates).map(|(b, &c)| b[column] * c),
            poll,
        )?;
        work.advance(basis.len())?;
    }
    Ok(result)
}
pub(super) fn multiply(
    matrix: &DenseMatrix,
    gamma: &[f64],
    sign: f64,
    poll: Poll<'_>,
) -> Result<Vec<f64>> {
    let mut work = WorkPoll::new(poll)?;
    let mut result = Vec::new();
    reserve(&mut result, matrix.nrows(), "simplex observation direction")?;
    for row in 0..matrix.nrows() {
        result.push(sign * matrix.row_combination_polled(row, gamma, 0., poll)?);
        work.advance(gamma.len().max(1))?;
    }
    if execution::any(&result, |v| !v.is_finite(), poll)? {
        return Err(super::failed("simplex direction is not representable"));
    }
    execution::check(poll)?;
    Ok(result)
}
