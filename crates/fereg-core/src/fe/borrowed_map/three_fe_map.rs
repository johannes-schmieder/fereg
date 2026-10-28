//! A geometry-qualified, bounded MAP attempt on untouched OLS inputs.
use super::*;

mod geometry;
#[cfg(test)]
mod tests;
pub(crate) use geometry::Geometry;

pub(super) const SWEEPS: usize = 4;

crate::fault::fault_plan! {
    static SETUP_FAILURE: Option<ErrorCode> = None;
}

pub(super) struct Attempt {
    pub result: Option<CertifiedBorrowedMap>,
    pub iterations: usize,
    pub setup_seconds: f64,
    pub solve_seconds: f64,
    pub peak: SolverWorkspacePeak,
}

impl Default for Attempt {
    fn default() -> Self {
        Self {
            result: None,
            iterations: 0,
            setup_seconds: 0.0,
            solve_seconds: 0.0,
            peak: SolverWorkspacePeak::ZERO,
        }
    }
}

impl Attempt {
    pub(super) fn record(&self, report: &mut SolveReport) {
        report.iterations += self.iterations;
        report.pilot_iterations = self.iterations;
        report.setup_seconds += self.setup_seconds;
        report.solve_seconds += self.solve_seconds;
        report.fallback_used |= self.iterations > 0;
        report.solver_workspace_peak = report.solver_workspace_peak.max(self.peak);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn attempt(
    design: &BorrowedInterceptFeDesign<'_>,
    original: &DenseMatrix,
    map: MapConfig,
    budget: Option<FeSolverMemoryBudget>,
    retain: bool,
    coefficient_bytes: u128,
    predicted_checkpoint: usize,
    dense_norms: bool,
) -> Result<Attempt> {
    let started = Instant::now();
    let mut trial = Attempt::default();
    let workspace_bytes = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        original.nrows(),
        original.ncols(),
        rayon::current_num_threads(),
    )?;
    let bytes = workspace_bytes
        .checked_add(original.as_slice().len() as u128 * 8)
        .and_then(|b| b.checked_add(if retain { coefficient_bytes } else { 0 }));
    let Some(bytes) = bytes.filter(|&b| budget.is_none_or(|limit| limit.fits(b))) else {
        return Ok(trial);
    };
    // Record the admitted construction overlap even if a later optional
    // reservation fails after an earlier bank was allocated.
    trial.peak = SolverWorkspacePeak::from_bytes(bytes)?;
    let setup: Result<_> = (|| {
        let candidate = fallible_matrix_copy(original, "unable to reserve bounded MAP candidate")?;
        if let Some(code) = SETUP_FAILURE.replace(None) {
            return Err(FeregError::new(
                code,
                "bounded MAP workspace reservation failed",
            ));
        }
        let workspace = BorrowedMapWorkspace::new(
            design,
            original.nrows(),
            original.ncols(),
            &mut SystemWorkspaceReservation,
        )?;
        let coefficients = retain
            .then(|| FeCoefficientBatch::intercept_zeros(design, original.ncols()))
            .transpose()?;
        Ok((candidate, workspace, coefficients))
    })();
    let (mut values, mut workspace, mut coefficients) = match setup {
        Ok(state) => state,
        Err(e) if e.code == ErrorCode::Allocation => {
            trial.setup_seconds = started.elapsed().as_secs_f64();
            return Ok(trial);
        }
        Err(e) => return Err(e),
    };
    // The original remains live in its execution admission. The new candidate,
    // workspace and coefficient batch are charged separately. Record directly
    // in the final RHS-major batch: no conversion allocation follows a pass.
    let retained_coefficients = coefficients
        .as_ref()
        .map_or(0, FeCoefficientBatch::retained_bytes);
    let actual = modeled_bytes(workspace.payload_peak()?)?
        + values.storage_capacity() as u128 * 8
        + retained_coefficients;
    if actual > bytes {
        trial.peak = SolverWorkspacePeak::from_bytes(actual)?;
        trial.setup_seconds = started.elapsed().as_secs_f64();
        return Ok(trial);
    }
    let weights = validate_weights(BorrowedMapWeights::Unit, original.nrows())?;
    if let Err(e) = update_workspace_norms(original, weights, &mut workspace, true) {
        if e.code != ErrorCode::InvalidData {
            return Err(e);
        }
        trial.setup_seconds = started.elapsed().as_secs_f64();
        return Ok(trial);
    }
    trial.setup_seconds = started.elapsed().as_secs_f64();
    let solve = Instant::now();
    // The exact-arithmetic bound is conservative, so the certificate one sweep
    // before its predicted sweep usually already passes. A miss costs one
    // certificate and continues to the predicted sweep; acceptance is always
    // the unchanged requested-tolerance certificate.
    let early_checkpoint = predicted_checkpoint.saturating_sub(1).max(1);
    for iteration in 0..=SWEEPS {
        crate::interrupt::checkpoint()?;
        if iteration > 0 {
            trial.iterations = iteration;
            if let Err(e) = borrowed_residual_sweep_recording(
                design,
                weights,
                &mut values,
                &mut workspace,
                coefficients.as_mut(),
                true,
            ) {
                if e.code != ErrorCode::InvalidData {
                    return Err(e);
                }
                break;
            }
        }
        // Geometry proposes a checkpoint; only the unchanged requested
        // certificate accepts it. A miss still reaches the ordinary checks.
        if iteration == 0
            || iteration == early_checkpoint
            || iteration == predicted_checkpoint
            || iteration % map.check_every == 0
            || iteration == SWEEPS
        {
            let certificate = match initial_screen::pilot_certificate(
                iteration == 0,
                design,
                weights,
                &values,
                map.tolerance,
                &mut workspace,
            ) {
                Ok(Some(certificate)) => certificate,
                Ok(None) => continue,
                Err(e) if e.code == ErrorCode::InvalidData => break,
                Err(e) => return Err(e),
            };
            if certificate.passed {
                trial.result = Some(finish(
                    values,
                    iteration,
                    certificate,
                    &trial,
                    solve.elapsed().as_secs_f64(),
                    workspace,
                    coefficients,
                    dense_norms.then_some(weights),
                )?);
                return Ok(trial);
            }
        }
    }
    trial.solve_seconds = solve.elapsed().as_secs_f64();
    // Failed candidate, coefficient and norm storage are dropped here, before
    // the caller constructs LSMR on the unchanged original matrix.
    Ok(trial)
}

#[allow(clippy::too_many_arguments)]
fn finish(
    values: DenseMatrix,
    iteration: usize,
    certificate: FeCertificate,
    trial: &Attempt,
    seconds: f64,
    mut workspace: BorrowedMapWorkspace,
    coefficients: Option<FeCoefficientBatch>,
    dense_norms: Option<ValidatedWeights<'_>>,
) -> Result<CertifiedBorrowedMap> {
    // Dense OLS hands off the same full-pass norm state as certified LSMR, from
    // the same kernel on the accepted matrix, so the finite solve need not
    // recompute it. Low keeps its fixed-leaf norms and receives no handoff.
    let norms = match dense_norms {
        Some(weights) if iteration > 0 => handoff_norms(&values, weights, &mut workspace)?,
        _ => None,
    };
    let mut result = certified_result(
        values,
        SolveStopReason::Certified,
        iteration,
        certificate,
        trial.setup_seconds,
        seconds,
    )?;
    result.set_solver_workspace_peak(trial.peak);
    drop(workspace);
    let result = attach_optional_coefficients(result, coefficients)?;
    match norms {
        Some(norms) => result.with_residual_norms(norms),
        None => Ok(result),
    }
}

/// Full-pass weighted column norms of the accepted matrix, computed by the
/// kernel certified LSMR hands off. None when a norm state is not finite.
fn handoff_norms(
    values: &DenseMatrix,
    weights: ValidatedWeights<'_>,
    workspace: &mut BorrowedMapWorkspace,
) -> Result<Option<Vec<f64>>> {
    let columns = values.ncols();
    if workspace.rhs_scales.len() != columns {
        return Ok(None);
    }
    let valid = crate::fe::norms::column_norms_with_dense(
        values,
        |row| {
            let root = weights.normalized(row).sqrt();
            if root.is_finite() && root > 0.0 {
                Ok(root)
            } else {
                Err(invalid_data(
                    "borrowed MAP normalized weight is not finite and positive",
                ))
            }
        },
        &mut workspace.residual_norms,
        &mut workspace.value_scales,
        &mut workspace.group_sums,
        &mut workspace.rhs_scales,
        "borrowed MAP weighted value is not finite",
        "borrowed MAP weighted column norm is not finite",
    )?;
    Ok(valid.then(|| std::mem::take(&mut workspace.rhs_scales)))
}
