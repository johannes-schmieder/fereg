use super::{
    allocate, fixed, invalid, numerical, validate_scatter, Kind, OutputCertificate, Outputs,
    Request,
};
use crate::{data::DenseMatrix, error::Result, fe::Design, threads::ThreadTeam};

#[derive(Debug)]
pub struct OlsFrame<'a, 'data> {
    pub design: &'a Design<'data>,
    pub finite_design: &'a DenseMatrix,
    pub coefficients: &'a [f64],
    /// Single final-model RHS: alpha_y - sum_j beta_j alpha_xj.
    /// These coefficients precede transfer of the reported constant.
    pub absorbed_coefficients: &'a DenseMatrix,
    pub residuals: &'a [f64],
    /// Absorbed-model normalization coordinate; zero for an ordinary constant
    /// already present in finite_design or a suppressed reported constant.
    pub reported_constant: f64,
}
pub fn ols(
    frame: &OlsFrame<'_, '_>,
    request: &Request,
    tolerance: f64,
    team: &ThreadTeam,
) -> Result<Outputs> {
    validate(frame, request, tolerance)?;
    let sample = frame.design.sample();
    let kinds = request.kinds(false)?;
    if kinds.iter().all(|k| *k == Kind::DropReason) {
        let output = allocate(sample, kinds)?;
        validate_scatter(sample, &output)?;
        return Ok(output);
    }
    let mut alpha = fixed::normalized(frame.design, frame.absorbed_coefficients, tolerance, team)?;
    let n = sample.n_active();
    let mut rhs = DenseMatrix::zeros(n, 1)?;
    let mut residual = DenseMatrix::zeros(n, 1)?;
    for row in 0..n {
        rhs[(row, 0)] = sample.outcome[row] - finite_row(frame, row, 0.);
        residual[(row, 0)] = frame.residuals[row];
    }
    let certificate = frame
        .design
        .certify(&rhs, &alpha, &residual, tolerance, team)?;
    transfer_constant(frame, &mut alpha)?;
    let absorbed = frame.design.multiply(&alpha, team)?;
    let mut output = allocate(sample, kinds)?;
    let mut reconstruction = 0_f64;
    for (row, &original) in sample.retained_rows().iter().enumerate() {
        let xb = finite_row(frame, row, frame.reported_constant);
        let d = absorbed[(row, 0)];
        let r = frame.residuals[row];
        let y = sample.outcome[row];
        let xbd = y - r;
        let fitted = xb + d;
        let scale = y
            .abs()
            .max(xb.abs())
            .max(d.abs())
            .max(r.abs())
            .max(f64::MIN_POSITIVE);
        let error = (xbd - fitted).abs() / scale;
        if !error.is_finite() || error > tolerance {
            return Err(numerical("OLS output reconstruction failed"));
        }
        reconstruction = reconstruction.max(error);
        for column in &mut output.columns {
            let value = match column.kind {
                Kind::Residuals => r,
                Kind::Xb => xb,
                Kind::Xbd => xbd,
                Kind::Absorbed => d,
                Kind::DropReason => continue,
                Kind::Mean => return Err(invalid("OLS mean output is unsupported")),
                Kind::Saved { effect, channel } => {
                    fixed::contribution(frame.design, &alpha, effect, channel, row)?
                }
            };
            if !value.is_finite() {
                return Err(numerical("OLS observation output is not representable"));
            }
            column.values[original] = value;
        }
    }
    validate_scatter(sample, &output)?;
    output.certificate = Some(OutputCertificate {
        fixed_effects: certificate,
        relative_reconstruction: reconstruction,
    });
    Ok(output)
}
fn finite_row(frame: &OlsFrame<'_, '_>, row: usize, constant: f64) -> f64 {
    frame
        .finite_design
        .row_combination(row, frame.coefficients, constant)
}
// Certify the original numerical RHS before splitting out the reported
// constant. A row with y == _cons must retain its original response scale;
// centering it to zero cannot turn roundoff into a unit reconstruction error.
fn transfer_constant(frame: &OlsFrame<'_, '_>, alpha: &mut DenseMatrix) -> Result<()> {
    if frame.reported_constant != 0. {
        let effect = frame
            .design
            .topology()
            .effects()
            .iter()
            .position(|e| e.intercept)
            .ok_or_else(|| invalid("reported absorbed constant requires an intercept FE"))?;
        // A global transfer from the first intercept preserves the ordinary
        // per-component gauge of all later effects and every slope channel.
        for level in 0..frame.design.layout().effects[effect].levels {
            let index = frame.design.coefficient_index(effect, 0, level)?;
            alpha[(index, 0)] -= frame.reported_constant;
        }
    }
    Ok(())
}

fn validate(frame: &OlsFrame<'_, '_>, request: &Request, tolerance: f64) -> Result<()> {
    let n = frame.design.nobs();
    if !tolerance.is_finite()
        || tolerance <= 0.
        || tolerance >= 1.
        || !frame.reported_constant.is_finite()
        || frame.finite_design.nrows() != n
        || frame.finite_design.ncols() != frame.coefficients.len()
        || frame.absorbed_coefficients.nrows() != frame.design.layout().total
        || frame.absorbed_coefficients.ncols() != 1
        || frame.residuals.len() != n
        || frame
            .residuals
            .iter()
            .chain(frame.coefficients)
            .chain(frame.finite_design.as_slice())
            .chain(frame.absorbed_coefficients.as_slice())
            .any(|v| !v.is_finite())
    {
        return Err(invalid("OLS output frame is malformed or nonfinite"));
    }
    fixed::validate_channels(frame.design.topology(), request)
}
