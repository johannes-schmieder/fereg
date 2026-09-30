use super::{failed, Interrupt, InterruptPoint, IrFixedPoint, IrReport};
use crate::{
    data::{DenseMatrix, Weights},
    error::{reserved, ErrorCode, Result},
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
    let boundary = outcome.iter().filter(|&&y| y == 0.).count();
    // Divide the reference interior weight N0/tol^2 out of the whole problem;
    // this preserves its projection without huge weights. Prior PPML weights
    // and offsets do not change the homogeneous separation cone.
    let boundary_weight =
        (model.controls.tolerance.powi(2) / boundary.max(1) as f64).max(f64::MIN_POSITIVE);
    let mut weights = reserved(outcome.len())?;
    weights.extend(
        outcome
            .iter()
            .map(|&y| if y > 0. { 1. } else { boundary_weight }),
    );
    let design = Design::new(view.sample(), Weights::Provided(&weights))?;
    Plan::new(&design, Estimator::Ppml, &options)?;
    if boundary == 0 {
        return Ok(report);
    }
    // Artificial boundary weights cannot decide the homogeneous model span.
    // Freeze the ordinary unit-metric basis before using rectifier weights;
    // neither this auxiliary basis nor its factors cross a generation boundary.
    interrupt(InterruptPoint::LinearFrame)?;
    let (basis, columns) = span_basis(view, model, model.span_options()?, &mut report.work, team)?;
    report.span_frames = 1;
    let mut engine = Engine::new(&design, Estimator::Ppml, &options, team)?;
    let mut u = reserved(outcome.len())?;
    u.extend(outcome.iter().map(|&y| if y == 0. { 1. } else { 0. }));
    for iteration in 1..=report.budget {
        interrupt(InterruptPoint::RectifierIteration)?;
        interrupt(InterruptPoint::LinearFrame)?;
        let fit = wls::solve_absorbed_recorded(
            &mut engine,
            &columns,
            &u,
            model.roles,
            AbsorbedControls {
                finite: model.linear_policy(),
                basis: Some(&basis),
            },
            &mut report.work,
            team,
        )?;
        let fitted = super::super::numerical::fitted(&columns, &design, &fit, team)?;
        drop(fit);
        let point = metrics(outcome, &fitted)?;
        let threshold = model.controls.tolerance * point.direction_scale.max(1.);
        if fixed(point, model.controls) {
            report.iterations = iteration;
            report.status = Status::NoSeparation;
            report.fixed_point = Some(point);
            return Ok(report);
        }
        if point.boundary_violation_relative <= model.controls.tolerance
            && point.boundary_maximum_relative > model.controls.tolerance
        {
            let mut direction = reserved(outcome.len())?;
            direction.extend(fitted.as_slice().iter().map(|v| -v));
            match view.direction_proposal(
                Method::IteratedRectifier,
                iteration,
                direction,
                model.controls,
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
        rectify(outcome, &fitted, threshold, &mut u);
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
    let design = Design::new(view.sample(), Weights::Unit)?;
    let mut engine = Engine::new(&design, Estimator::Ppml, &options, team)?;
    let mut zero = reserved(view.sample().n_active())?;
    zero.resize(view.sample().n_active(), 0.);
    let fit = wls::solve_absorbed_recorded(
        &mut engine,
        model.raw,
        &zero,
        model.roles,
        AbsorbedControls {
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
        if role != Role::AbsorbedIntercept {
            continue;
        }
        if !design.topology().effects().iter().any(|e| e.intercept)
            || (0..columns.nrows()).any(|row| model.raw[(row, column)] != 1.)
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
            columns[(row, column)] = 0.;
        }
    }
    Ok((basis, columns))
}

fn metrics(outcome: &[f64], fitted: &DenseMatrix) -> Result<IrFixedPoint> {
    if fitted.as_slice().iter().any(|v| !v.is_finite()) {
        return Err(failed("IR projection has a nonfinite fitted direction"));
    }
    let scale = fitted.as_slice().iter().fold(0_f64, |m, &v| m.max(v.abs()));
    let recurrence_scale = scale.max(1.);
    let mut interior = 0_f64;
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for (row, &y) in outcome.iter().enumerate() {
        let value = fitted[(row, 0)];
        if y > 0. {
            interior = interior.max(value.abs());
        } else {
            minimum = minimum.min(value);
            maximum = maximum.max(value);
        }
    }
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
fn rectify(outcome: &[f64], fitted: &DenseMatrix, threshold: f64, u: &mut [f64]) {
    let mut maximum = 0_f64;
    for (row, (&y, value)) in outcome.iter().zip(u.iter_mut()).enumerate() {
        let projected = fitted[(row, 0)];
        *value = if y > 0. || projected.abs() <= threshold {
            0.
        } else {
            projected.max(0.)
        };
        maximum = maximum.max(*value);
    }
    if maximum > 0. {
        for value in u {
            *value /= maximum;
        }
    }
}
