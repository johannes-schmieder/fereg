//! Scoped no-FE representation, admission and canonical compatibility boundary.
use super::*;

fn workspace_bytes(rows: usize, columns: usize) -> Result<u128> {
    let summary = CenteredSummary::workspace_bytes(rows, columns)?;
    let moments =
        crate::ols::finite_wls_kernel::centered_moment_workspace_bytes(rows, columns - 1)?;
    summary
        .checked_add(moments)
        .and_then(|v| v.checked_add(columns as u128 * 5 * 8))
        .ok_or_else(|| allocation("centered OLS workspace envelope overflows"))
}

pub(super) fn admitted<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    config: PreparedOlsFitConfig,
) -> Result<bool> {
    let rows = prepared.fit_sample().n_active();
    let columns = prepared.fit_model().columns.len() + 1;
    let bytes = workspace_bytes(rows, columns)?;
    // The incumbent may accept the unprojected constant at a loose tolerance.
    // Preserve that decision and every explicit memory admission.
    let root = (rows as f64).sqrt();
    Ok(rows > 0
        && (1.0 / root) * root > config.absorb.map.tolerance
        && config
            .absorb
            .solver_memory_budget
            .is_none_or(|budget| budget.fits(bytes)))
}

pub(super) fn prepare_raw<R: FitReservation>(
    values: crate::ols::OlsProjectionInput,
    pending: Option<Result<CenteredSummary>>,
    reservation: &mut R,
) -> Result<Option<DenseRaw>> {
    let summary = match pending.unwrap_or_else(|| CenteredSummary::build(&values, reservation)) {
        Ok(summary) => summary,
        Err(error) if error.code == ErrorCode::InvalidData => return Ok(None),
        Err(error) => return Err(error),
    };
    // Constructors keep binding private; prove adoption has not changed owner
    // or dimensions before any cached arithmetic participates in the solve.
    CenteredRows::new(&values, &summary)?;
    let copy = |source: &[f64], reservation: &mut R| -> Result<Vec<f64>> {
        let mut result = reserved_empty(
            reservation,
            source.len(),
            "unable to reserve centered OLS summary handoff",
        )?;
        result.extend_from_slice(source);
        Ok(result)
    };
    let moments = RawMomentCache {
        means: copy(summary.means(), reservation)?,
        scales: copy(summary.maxima(), reservation)?,
        norms: copy(summary.raw_norms(), reservation)?,
        weight_sum: values.nrows() as f64,
    };
    Ok(Some(DenseRaw {
        reference_norms: copy(&summary.raw_norms()[1..], reservation)?,
        values,
        moments: Some(moments),
        centered: Some(summary),
        reference_mean_scales: Vec::new(),
        strided_score_means: None,
    }))
}

pub(super) fn compatible_classification<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    raw: &DenseRaw,
    config: PreparedOlsFitConfig,
) -> Result<bool> {
    let summary = raw.centered.as_ref().expect("centered summary");
    if summary
        .centered_relative_moments()
        .iter()
        .any(|value| *value > config.absorb.map.tolerance)
    {
        return Ok(false);
    }
    for (column, metadata) in prepared.fit_model().columns.iter().enumerate() {
        if matches!(metadata.kind, PreparedOlsColumnKind::Intercept) {
            continue;
        }
        let column = column + 1;
        let norm = summary.raw_norms()[column];
        if norm == 0.0 {
            continue;
        }
        let ratio = summary.within_norms()[column] / norm;
        if !ratio.is_finite()
            || ratio * ratio <= config.fixed_effect_collinearity_tolerance
            || !canonical_retention_bound(
                summary.minimum_values()[column],
                summary.maximum_values()[column],
                summary.maxima()[column],
                raw.values.nrows(),
                config.fixed_effect_collinearity_tolerance,
            )
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One-sided bound on the *canonical* within/raw norm ratio. No estimate of
/// the changed center's error participates in a column omission decision.
///
/// Any mean leaves an endpoint at least half the original span away. Downward
/// rounding gives L <= max |RN(x-mean)| <= canonical within norm. Unweighted
/// LASSQ's scaled sum is <= N (each rounded ratio is <= 1; monotone rounding
/// preserves the integer row-count bound at every update/merge for N<=2^53).
/// Thus its canonical raw norm is <= RN(maxabs * RN(sqrt(N))) = U. Monotone
/// division and squaring give a lower bound on its actual classification ratio.
/// An unsuccessful bound delegates to canonical arithmetic; it never omits.
fn canonical_retention_bound(
    minimum: f64,
    maximum: f64,
    maxabs: f64,
    rows: usize,
    tolerance: f64,
) -> bool {
    let span = maximum - minimum;
    let upper = maxabs * (rows as f64).sqrt();
    if rows as u128 > (1u128 << 53)
        || !span.is_finite()
        || span <= 0.0
        || !upper.is_finite()
        || upper <= 0.0
    {
        return false;
    }
    let down = |value: f64| {
        if value > 0.0 {
            f64::from_bits(value.to_bits() - 1)
        } else {
            0.0
        }
    };
    let lower = down(0.5 * down(span));
    let ratio = lower / upper;
    ratio * ratio > tolerance
}

#[cfg(test)]
mod tests;

pub(super) fn project<R: FitReservation>(
    mut raw: DenseRaw,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
) -> Result<OneShotInputs> {
    let summary = raw.centered.as_ref().expect("centered summary");
    let rows = raw.values.nrows();
    let columns = raw.values.ncols();
    let mut absolute = 0.0_f64;
    let mut relative = 0.0_f64;
    let mut squares = 0.0;
    for column in 0..columns {
        let moment = summary.centered_means()[column].abs();
        let value = summary.centered_relative_moments()[column];
        absolute = absolute.max(moment * rows as f64);
        relative = relative.max(value);
        squares += value * value;
    }
    let certificate = FeCertificate {
        absolute_infinity_norm: absolute,
        relative_infinity_norm: relative,
        relative_two_norm: (squares / columns as f64).sqrt(),
        passed: relative <= config.absorb.map.tolerance,
    };
    if !certificate.passed {
        return Err(invalid_data(
            "centered OLS structural moment certificate failed",
        ));
    }
    let report = SolveReport {
        requested_solver: FeSolverKind::Auto,
        iterations: 1,
        solver_workspace_peak: crate::memory::SolverWorkspacePeak::from_bytes(workspace_bytes(
            rows, columns,
        )?)?,
        ..SolveReport::single_route(FeSolverKind::Map, SolveStopReason::Certified, certificate)
    };
    let mut norms = reserved_empty(
        reservation,
        columns,
        "unable to reserve centered within norms",
    )?;
    norms.extend_from_slice(summary.within_norms());
    let within_outcome_norm = norms[0];
    let initial_sum_of_squares = within_outcome_norm * within_outcome_norm;
    if !initial_sum_of_squares.is_finite() {
        return Err(invalid_data("centered OLS TSS is not finite"));
    }
    let mut trace = crate::diagnostics::phase_ledger::PhaseLedger::new("centered_projection", []);
    trace.facts(&[
        ("rows", rows as u64),
        ("columns", columns as u64),
        ("summary_resident_bytes", summary.resident_bytes() as u64),
    ]);
    Ok(OneShotInputs {
        reference_norms: std::mem::take(&mut raw.reference_norms),
        storage: ProjectedStorage::Centered(raw),
        norms,
        within_outcome_norm,
        initial_sum_of_squares,
        absorption_report: report,
        initial_fe_coefficients: None,
        scratch_bytes: 0,
        fast_score_cache: None,
        rank_resolution: None,
        rank_score: None,
    })
}

pub(super) fn compatibility_projection<'a, P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    inputs: &mut OneShotInputs,
    mut config: PreparedOlsFitConfig,
    reservation: &mut R,
    cmg: &mut crate::fe::FeSession<'a>,
) -> Result<()> {
    let peak = inputs.absorption_report.solver_workspace_peak;
    let ProjectedStorage::Centered(raw) = &mut inputs.storage else {
        return Err(invalid_argument(
            "finite compatibility fallback needs a centered owner",
        ));
    };
    let values = std::mem::replace(
        &mut raw.values,
        DenseMatrix::from_row_major(0, 0, Vec::new())?.into(),
    );
    raw.centered = None;
    raw.moments = None;
    let mut replacement = project_dense(
        prepared,
        frame,
        &mut config,
        reservation,
        cmg,
        false,
        0,
        Some(LoadedOlsRows::from(values.into_values().into_row_major())),
        0,
        true,
        None,
        false,
    )?;
    replacement.absorption_report.solver_workspace_peak = replacement
        .absorption_report
        .solver_workspace_peak
        .max(peak);
    *inputs = replacement;
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(
            "FEREG_SOLVER_TRACE|phase=centered_compatibility|reason=numerical_authority",
        );
    }
    Ok(())
}

/// Independent scores use the reported coefficients and authoritative original
/// values. Compensated products/sums are distinct from the centered moment
/// kernel and independently witness its returned residual as well as its score.
pub(super) fn original_scores<R: FitReservation>(
    raw: &DenseRaw,
    residuals: &[f64],
    fit: &FiniteFit,
    norms: &[f64],
    outcome_norm: f64,
    tolerance: f64,
    reservation: &mut R,
) -> Result<Option<FiniteScoreDraft>> {
    let count = fit.kept.len();
    let rows = residuals.len();
    let scale = outcome_norm.max(residuals.iter().copied().map(f64::abs).fold(0.0, f64::max));
    let divisor = if scale == 0.0 { 1.0 } else { scale };
    let stride = count * 2 + 1;
    let blocks = crate::parallel::reduction_blocks(rows, 16_384, 64, stride, 1024 * 1024)?;
    let mut partials = reserved_filled(
        reservation,
        blocks * stride,
        0.0,
        "unable to reserve original-source score certificate",
    )?;
    let chunk = rows.div_ceil(blocks);
    crate::parallel::stage_try_for_each(
        partials.par_chunks_mut(stride).enumerate(),
        rows as u128 * count.max(1) as u128,
        |(block, bank)| {
            let (sum, tail) = bank.split_at_mut(count);
            let (correction, difference) = tail.split_at_mut(count);
            for (row, residual) in residuals
                .iter()
                .enumerate()
                .take(((block + 1) * chunk).min(rows))
                .skip(block * chunk)
            {
                crate::interrupt::row_checkpoint(row)?;
                let mut original = raw.values[(row, 0)];
                let mut low = 0.0;
                for (column, coefficient) in fit.generated_coefficients.iter().enumerate() {
                    if let Some(coefficient) = coefficient {
                        let value = raw.values[(row, column + 1)];
                        let product = -*coefficient * value;
                        let error = (-*coefficient).mul_add(value, -product);
                        compensated_add(product, &mut original, &mut low)?;
                        compensated_add(error, &mut original, &mut low)?;
                    }
                }
                let original = original + low;
                let delta = (original - residual) / divisor;
                difference[0] += delta * delta;
                for (position, &column) in fit.kept.iter().enumerate() {
                    let term =
                        (raw.values[(row, column + 1)] / norms[column]) * (original / divisor);
                    compensated_add(term, &mut sum[position], &mut correction[position])?;
                }
            }
            Ok(())
        },
    )?;
    let relative_difference = partials
        .chunks_exact(stride)
        .map(|bank| bank[count * 2])
        .sum::<f64>()
        .sqrt();
    if !relative_difference.is_finite() || relative_difference > tolerance {
        return Ok(None);
    }
    let mut result = FiniteScoreDraft {
        moments: count,
        ..FiniteScoreDraft::default()
    };
    for position in 0..count {
        let mut sum = 0.0;
        let mut correction = 0.0;
        for bank in partials.chunks_exact(stride) {
            compensated_add(
                bank[position] + bank[count + position],
                &mut sum,
                &mut correction,
            )?;
        }
        let value = (sum + correction).abs();
        if !value.is_finite() || value > tolerance {
            return Ok(None);
        }
        let unscaled = value * divisor;
        result.infinity_scale = result.infinity_scale.max(unscaled);
        result.square_sum_scale += unscaled * unscaled;
    }
    Ok(Some(result))
}
