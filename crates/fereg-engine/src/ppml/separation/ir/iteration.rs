use super::{failed, Interrupt, InterruptPoint, IrFixedPoint, IrReport};
#[cfg(test)]
mod interruption_tests;
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, Result},
    execution::{self, Poll},
    fe::{Design, Engine, Plan},
    ppml::separation::{Method, Model, Status, View},
    spec::{Estimator, Options},
    threads::ThreadTeam,
    wls::{self, AbsorbedControls, FrozenBasis, Role, Work},
};

pub(super) fn run(
    view: View<'_>,
    model: &Model<'_>,
    options: Options,
    mut report: IrReport,
    team: &ThreadTeam,
    interrupt: &mut Interrupt<'_>,
) -> Result<IrReport> {
    let outcome = &view.sample().outcome;
    let poll = model.probe.dense();
    let boundary = execution::fold(outcome, 0, poll, |n, &y| n + usize::from(y == 0.))?;
    // Divide the reference interior weight N0/tol^2 out of the whole problem;
    // this preserves its projection without huge weights. Prior PPML weights
    // and offsets do not change the homogeneous separation cone.
    let boundary_weight =
        (model.controls.tolerance.powi(2) / boundary.max(1) as f64).max(f64::MIN_POSITIVE);
    let mut weights = reserved(outcome.len())?;
    for (row, &y) in outcome.iter().enumerate() {
        execution::check_chunk(poll, row)?;
        weights.push(if y > 0. { 1. } else { boundary_weight });
    }
    let design = Design::new_polled(view.sample(), Weights::Provided(&weights), poll)?;
    Plan::new(&design, Estimator::Ppml, &options)?;
    if boundary == 0 {
        execution::check(poll)?;
        return Ok(report);
    }
    // Artificial boundary weights cannot decide the homogeneous model span.
    // Freeze the ordinary unit-metric basis before using rectifier weights;
    // neither this auxiliary basis nor its factors cross a generation boundary.
    interrupt(InterruptPoint::LinearFrame)?;
    let (basis, columns) = span_basis(view, model, model.span_options()?, &mut report.work, team)?;
    report.span_frames = 1;
    let mut engine = Engine::new_polled(
        &design,
        Estimator::Ppml,
        &options,
        team,
        model.probe.dense(),
    )?;
    let mut u = reserved(outcome.len())?;
    for (row, &y) in outcome.iter().enumerate() {
        execution::check_chunk(poll, row)?;
        u.push(if y == 0. { 1. } else { 0. });
    }
    for iteration in 1..=report.budget {
        interrupt(InterruptPoint::RectifierIteration)?;
        interrupt(InterruptPoint::LinearFrame)?;
        let fit = wls::solve_absorbed_recorded(
            &mut engine,
            &columns,
            &u,
            model.roles,
            AbsorbedControls {
                probe: model.probe,
                finite: model.linear_policy(),
                basis: Some(&basis),
            },
            &mut report.work,
            team,
        )?;
        let fitted = super::super::numerical::fitted(&columns, &design, &fit, team, poll)?;
        drop(fit);
        let point = metrics(outcome, &fitted, poll)?;
        let threshold = model.controls.tolerance * point.direction_scale.max(1.);
        if fixed(point, model.controls) {
            report.iterations = iteration;
            report.status = Status::NoSeparation;
            report.fixed_point = Some(point);
            execution::check(poll)?;
            return Ok(report);
        }
        if point.boundary_violation_relative <= model.controls.tolerance
            && point.boundary_maximum_relative > model.controls.tolerance
        {
            let mut direction = reserved(outcome.len())?;
            for (row, &value) in fitted.as_slice().iter().enumerate() {
                execution::check_chunk(poll, row)?;
                direction.push(-value);
            }
            match view.direction_proposal_polled(
                Method::IteratedRectifier,
                iteration,
                direction,
                model.controls,
                poll,
            ) {
                Ok(proposal) => {
                    report.iterations = iteration;
                    report.status = Status::Certified;
                    report.direction = Some(proposal);
                    return Ok(report);
                }
                // An uncertified intermediate guess is another recurrence
                // state, never a sample decision or a terminal report.
                Err(error) if error.code == ErrorCode::NonConvergence => (),
                Err(error) => return Err(error),
            }
        }
        rectify(outcome, &fitted, threshold, &mut u, poll)?;
    }
    Err(failed(&format!(
        "IR exhausted {} iterations without certified separation or a fixed point",
        report.budget
    )))
}

fn span_basis(
    view: View<'_>,
    model: &Model<'_>,
    options: Options,
    work: &mut Work,
    team: &ThreadTeam,
) -> Result<(FrozenBasis, DenseMatrix)> {
    let poll = model.probe.dense();
    let design = Design::new_polled(view.sample(), Weights::Unit, poll)?;
    let mut engine = Engine::new_polled(
        &design,
        Estimator::Ppml,
        &options,
        team,
        model.probe.dense(),
    )?;
    let mut zero = reserved(view.sample().n_active())?;
    zero.resize(view.sample().n_active(), 0.);
    let fit = wls::solve_absorbed_recorded(
        &mut engine,
        model.raw,
        &zero,
        model.roles,
        AbsorbedControls {
            probe: model.probe,
            finite: model.linear_policy(),
            basis: None,
        },
        work,
        team,
    )?;
    // Removing the unit-metric FE part preserves the full model span while
    // avoiding cancellation between a large finite coefficient and its FE
    // counterpart in the artificial weighted recurrence. Fresh certification
    // still uses the original model, independently of this auxiliary frame.
    let basis = FrozenBasis::from_fit(&fit.finite)?;
    let mut columns = fit.within;
    for (column, &role) in model.roles.iter().enumerate() {
        execution::check_chunk(poll, column)?;
        if role != Role::AbsorbedIntercept {
            continue;
        }
        if !execution::any(design.topology().effects(), |e| e.intercept, poll)?
            || execution::any(
                0..columns.nrows(),
                |row| model.raw[(row, column)] != 1.,
                poll,
            )?
        {
            return Err(super::invalid(
                "deferred IR constant lacks the exact intercept span",
            ));
        }
        // This declared constant is already in the intercept-FE span and
        // has no auxiliary finite coefficient. Its exact within column is
        // zero; reprojecting subtraction roundoff creates a spurious RHS.
        // Ordinary columns keep the certified unit-frame residual unchanged.
        for row in 0..columns.nrows() {
            execution::check_chunk(poll, row)?;
            columns[(row, column)] = 0.;
        }
    }
    execution::check(poll)?;
    Ok((basis, columns))
}

fn metrics(outcome: &[f64], fitted: &DenseMatrix, poll: Poll<'_>) -> Result<IrFixedPoint> {
    execution::check(poll)?;
    if execution::any(fitted.as_slice(), |v| !v.is_finite(), poll)? {
        return Err(failed("IR projection has a nonfinite fitted direction"));
    }
    let scale = execution::fold(fitted.as_slice(), 0_f64, poll, |m, &v| m.max(v.abs()))?;
    let recurrence_scale = scale.max(1.);
    let mut interior = 0_f64;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (row, &y) in outcome.iter().enumerate() {
        execution::check_chunk(poll, row)?;
        let value = fitted[(row, 0)];
        if y > 0. {
            interior = interior.max(value.abs());
        } else {
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
    execution::check(poll)?;
    Ok(IrFixedPoint {
        direction_scale: scale,
        interior_zero_relative: interior / recurrence_scale,
        boundary_violation_relative: (-minimum).max(0.) / recurrence_scale,
        boundary_maximum_relative: maximum / recurrence_scale,
    })
}
fn fixed(point: IrFixedPoint, controls: super::Controls) -> bool {
    point.boundary_violation_relative <= controls.tolerance
        && point.boundary_maximum_relative <= controls.tolerance
        && point.interior_zero_relative <= controls.certificate_tolerance
        && point.boundary_violation_relative <= controls.certificate_tolerance
}
fn rectify(
    outcome: &[f64],
    fitted: &DenseMatrix,
    threshold: f64,
    u: &mut [f64],
    poll: Poll<'_>,
) -> Result<()> {
    execution::check(poll)?;
    let mut maximum = 0_f64;
    for (row, (&y, value)) in outcome.iter().zip(u.iter_mut()).enumerate() {
        execution::check_chunk(poll, row)?;
        let projected = fitted[(row, 0)];
        *value = if y > 0. || projected.abs() <= threshold {
            0.
        } else {
            projected.max(0.)
        };
        maximum = maximum.max(*value);
    }
    if maximum > 0. {
        for (row, value) in u.iter_mut().enumerate() {
            execution::check_chunk(poll, row)?;
            *value /= maximum;
        }
    }
    execution::check(poll)
}
