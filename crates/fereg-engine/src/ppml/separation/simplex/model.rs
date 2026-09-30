use super::{overflow, Model, View};
use crate::{
    data::{DenseMatrix, Weights},
    error::Result,
    execution::Poll,
    fe::{Design, Plan},
    linalg::WeightedQr,
    ppml::{Interrupt, InterruptPoint},
    spec::Estimator,
    wls::Role,
};

pub(super) fn design<'a>(view: View<'a>, model: &Model<'_>) -> Result<Design<'a>> {
    model.validate(view)?;
    let design = Design::new(view.sample(), Weights::Unit)?;
    Plan::new(&design, Estimator::Ppml, &model.options)?;
    Ok(design)
}

/// Quotient only full-model gauges, using the common auxiliary QR rank rule.
/// Public finite-rank selection still happens after separation. Recheck every
/// original column so an incomplete auxiliary span cannot close this detector.
pub(super) fn span(
    matrix: DenseMatrix,
    tolerance: f64,
    interrupt: &mut Interrupt<'_>,
    poll: Poll<'_>,
) -> Result<DenseMatrix> {
    let copy = |matrix: &DenseMatrix| -> Result<DenseMatrix> {
        let mut out = DenseMatrix::zeros(matrix.nrows(), matrix.ncols())?;
        out.as_mut_slice().copy_from_slice(matrix.as_slice());
        Ok(out)
    };
    let threshold = crate::wls::Policy::default().rank_multiplier
        * f64::EPSILON
        * matrix.nrows().max(matrix.ncols()).max(1) as f64;
    interrupt(InterruptPoint::SimplexModel)?;
    let qr = WeightedQr::factor(copy(&matrix)?, Weights::Unit, threshold, poll)?;
    let coefficients = qr.solve(copy(&matrix)?, poll)?;
    for column in 0..matrix.ncols() {
        interrupt(InterruptPoint::SimplexModel)?;
        let mut distance = 0_f64;
        let mut norm = 0_f64;
        for row in 0..matrix.nrows() {
            let fitted = crate::threads::compensated_sum(
                qr.kept()
                    .iter()
                    .map(|&basis| matrix[(row, basis)] * coefficients[(basis, column)]),
            );
            distance = distance.hypot(matrix[(row, column)] - fitted);
            norm = norm.hypot(matrix[(row, column)]);
        }
        if distance > tolerance * norm || !distance.is_finite() {
            return Err(super::failed(
                "simplex auxiliary QR lost an original model column",
            ));
        }
    }
    let mut span = DenseMatrix::zeros(matrix.nrows(), qr.rank())?;
    for (column, &basis) in qr.kept().iter().enumerate() {
        for row in 0..matrix.nrows() {
            span[(row, column)] = matrix[(row, basis)];
        }
    }
    Ok(span)
}
pub(super) fn width(design: &Design<'_>, model: &Model<'_>) -> Result<usize> {
    model
        .raw
        .ncols()
        .checked_add(design.layout().total)
        .ok_or_else(overflow)
}
/// The detector gets the whole model, before public finite-rank selection.
/// User-declared omitted/base slots consume width but have no model authority.
pub(super) fn explicit(
    design: &Design<'_>,
    model: &Model<'_>,
    interrupt: &mut Interrupt<'_>,
) -> Result<DenseMatrix> {
    let finite = model.raw.ncols();
    let mut matrix = DenseMatrix::zeros(design.nobs(), width(design, model)?)?;
    for row in 0..matrix.nrows() {
        if row % crate::threads::REDUCTION_CHUNK == 0 {
            interrupt(InterruptPoint::SimplexModel)?;
        }
        for (column, role) in model.roles.iter().enumerate() {
            if matches!(role, Role::Ordinary | Role::AbsorbedIntercept) {
                matrix[(row, column)] = model.raw[(row, column)];
            }
        }
        for (effect, layout) in design.layout().effects.iter().enumerate() {
            let level = design.topology().effect_partition(effect)?.codes()[row] as usize;
            for channel in 0..layout.channels {
                matrix[(
                    row,
                    finite + layout.offset + channel * layout.levels + level,
                )] = design.basis_value(effect, channel, row)?;
            }
        }
    }
    // Invertible column scaling preserves the full cone and avoids both
    // squared-norm overflow and small columns becoming absolute zero. Binary
    // categorical channels retain their exact coefficients in this frame.
    for column in 0..matrix.ncols() {
        interrupt(InterruptPoint::SimplexModel)?;
        let scale = (0..matrix.nrows()).fold(0_f64, |s, row| s.max(matrix[(row, column)].abs()));
        if scale > 0. {
            for row in 0..matrix.nrows() {
                let original = matrix[(row, column)];
                let value = original / scale;
                if original != 0. && value == 0. {
                    return Err(super::failed("simplex model column scaling underflow"));
                }
                matrix[(row, column)] = value;
            }
        }
    }
    Ok(matrix)
}
