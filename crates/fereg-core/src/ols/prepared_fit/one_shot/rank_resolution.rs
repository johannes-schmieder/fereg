//! OLS-only admission and publication for exceptional FE rank verification.
use super::*;
use crate::fe::BorrowedInterceptFeDesign;
use rank_repair::{add_up, Forest};

pub(super) struct ResolvedScales {
    pub(super) scales: Vec<f64>,
    pub(super) absolute_errors: Vec<f64>,
    pub(super) outcome_error: f64,
    pub(super) relative_error: f64,
}
impl ResolvedScales {
    pub(super) fn complete_scales(&mut self, raw: &[f64]) {
        for (scale, &reference) in self.scales.iter_mut().zip(raw) {
            if *scale == 0. {
                *scale = reference;
            }
        }
    }
}

struct RepairedColumn {
    values: Vec<f64>,
    norm: f64,
    absolute_error: f64,
    relative_error: f64,
    coefficients: Vec<f64>,
    report: Option<SolveReport>,
    peak: crate::memory::SolverWorkspacePeak,
}

/// A small raw/within ratio activates verification only when original-value
/// equality cannot already prove absorption. Once activated, certify every
/// quotient column and the response: an ordinary column may duplicate a
/// repaired one, and finite moments alone do not validate the original model.
pub(super) fn resolve_inputs<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    frame: LinearSolveFrame<'_>,
    inputs: &mut OneShotInputs,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    session: &mut crate::fe::FeSession<'_>,
) -> Result<()> {
    if inputs.rank_resolution.is_some()
        || !frame.prepared_ols
        || !prepared.fit_has_absorbed_intercept()
        || matches!(inputs.storage, ProjectedStorage::Centered(_))
    {
        return Ok(());
    }
    let columns = prepared.fit_model().columns.len();
    let suspect = |column: usize| {
        !matches!(
            prepared.fit_model().columns[column].kind,
            PreparedOlsColumnKind::Intercept
        ) && inputs.reference_norms[column] > 0.
            && (inputs.score_norms()[column] / inputs.reference_norms[column]).powi(2)
                <= config.fixed_effect_collinearity_tolerance
    };
    if !(0..columns).any(suspect) {
        return Ok(());
    }
    let Some(design) = prepared.fit_borrowed_intercept_map_design()? else {
        return Ok(());
    };
    if design.effects().len() > 2 {
        return Ok(());
    }
    let mut active = false;
    for column in 0..columns {
        if suspect(column)
            && rank_repair::structurally_absorbed(&design, |row| {
                prepared.fit_column_value(column, row)
            })?
            .is_none()
        {
            active = true;
            break;
        }
    }
    if !active {
        return Ok(());
    }

    let rows = prepared.fit_sample().n_active();
    let levels = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .sum::<usize>()
        .max(1);
    let helper = Forest::workspace_bytes(&design)?
        .checked_add(rows as u128 * 32 + levels as u128 * 32 + columns as u128 * 16)
        .ok_or_else(|| allocation("rank verification workspace overflows"))?;
    // The original FE session is no longer needed once repair is active.
    // Release its actual owners before allocating helpers, and rebuild the
    // terminal session after the quotient witness is prepared. A historical
    // report peak (possibly unmodeled) is not a bound on live session storage.
    // The report itself remains unchanged, preserving any unmodeled history.
    drop(std::mem::take(session));
    let retained = helper;
    let mut absorb = config.absorb;
    if let Some(budget) = &mut absorb.solver_memory_budget {
        budget.execution_bytes = budget
            .execution_bytes
            .checked_add(retained)
            .ok_or_else(|| allocation("rank verification execution budget overflows"))?;
        if !budget.fits(0) {
            return Err(allocation(
                "rank verification exceeds the requested memory limit",
            ));
        }
    }
    let mut resolved = ResolvedScales {
        scales: reserved_filled(reservation, columns, 0., "rank verification finite scales")?,
        absolute_errors: reserved_filled(
            reservation,
            columns,
            0.,
            "rank verification absolute errors",
        )?,
        outcome_error: 0.,
        relative_error: 0.,
    };
    let allowance = config.score_tolerance / (32. * ((columns + 1) as f64).sqrt());
    for column in 0..columns {
        let repaired = resolve_rhs(
            prepared,
            &design,
            |row| prepared.fit_column_value(column, row),
            frame.weights(),
            config,
            absorb,
            retained,
            allowance,
            reservation,
        )?;
        resolved.scales[column] = if repaired.norm == 0. {
            inputs.reference_norms[column]
        } else {
            repaired.norm
        };
        resolved.absolute_errors[column] = repaired.absolute_error;
        resolved.relative_error = add_up(resolved.relative_error, repaired.relative_error)?;
        patch_column(inputs, Some(column), &repaired.values, repaired.norm)?;
        record(inputs, column + 1, repaired)?;
    }
    let outcome = resolve_rhs(
        prepared,
        &design,
        |row| Ok(frame.response()[row]),
        frame.weights(),
        config,
        absorb,
        retained,
        allowance,
        reservation,
    )?;
    resolved.outcome_error = outcome.absolute_error;
    patch_column(inputs, None, &outcome.values, outcome.norm)?;
    record(inputs, 0, outcome)?;
    inputs.initial_sum_of_squares =
        initial_within_sum_of_squares_low(prepared, inputs.within_outcome_norm, None)?;
    inputs.rank_resolution = Some(resolved);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn resolve_rhs<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    design: &BorrowedInterceptFeDesign<'_>,
    mut value: impl FnMut(usize) -> Result<f64>,
    weights: NormalizedWeights<'_>,
    config: PreparedOlsFitConfig,
    mut absorb: AbsorbConfig,
    retained: u128,
    allowance: f64,
    reservation: &mut R,
) -> Result<RepairedColumn> {
    let rows = design.nobs();
    let levels = design
        .effects()
        .iter()
        .map(|effect| effect.n_levels())
        .sum::<usize>()
        .max(1);
    let mut peak = crate::memory::SolverWorkspacePeak::from_bytes(retained)?;
    if let Some(column) = structural_column(design, &mut value, levels, peak, reservation)? {
        return Ok(column);
    }
    let mut raw = reserved_empty(reservation, rows, "rank verification original column")?;
    for row in 0..rows {
        if row % 4096 == 0 {
            crate::interrupt::checkpoint()?;
        }
        raw.push(value(row)?);
    }
    let mut forest = Forest::new(design, reservation)?;
    let deflated = forest.deflate(raw, weights, reservation)?;
    if deflated.exact_absorbed {
        return Ok(RepairedColumn {
            values: deflated.values.into_row_major(),
            norm: 0.,
            absolute_error: 0.,
            relative_error: 0.,
            coefficients: deflated.gauge,
            report: None,
            peak,
        });
    }
    absorb.retain_coefficients = true;
    // Auto selects a coefficient-producing family. Explicit solver and
    // preconditioner choices remain authoritative in this rare verification.
    if absorb.solver == FeSolverKind::Auto
        && absorb.lsmr.preconditioner == crate::fe::LsmrPreconditioner::Auto
    {
        absorb.solver = FeSolverKind::Map;
    }
    absorb.map.tolerance = absorb.map.tolerance.min(1e-12);
    absorb.lsmr.tolerance = absorb.lsmr.tolerance.min(1e-12);
    let mut combined_report = None;
    for attempt in 0..2 {
        crate::interrupt::checkpoint()?;
        let mut coefficients = if design.effects().len() < 2 {
            direct_coefficients(design, deflated.values.as_slice(), weights, reservation)?
        } else {
            let mut values = reserved_empty(reservation, rows, "rank verification solver input")?;
            values.extend_from_slice(deflated.values.as_slice());
            let solved = prepared.fit_absorb_matrix(
                weights.borrowed(),
                DenseMatrix::from_row_major(rows, 1, values)?,
                absorb,
            )?;
            let (returned, mut report, coefficients) = solved.into_full_parts();
            if !report.certified() {
                return Err(unresolved());
            }
            report.requested_solver = config.absorb.solver;
            report.solver_workspace_peak = report.solver_workspace_peak.with_retained(retained)?;
            peak = peak.max(report.solver_workspace_peak);
            let iterations = report.iterations;
            if let Some(previous) = &mut combined_report {
                merge_solve_report(previous, report)?;
            } else {
                combined_report = Some(report);
            }
            absorb.map.max_iterations = absorb.map.max_iterations.saturating_sub(iterations);
            absorb.lsmr.max_iterations = absorb.lsmr.max_iterations.saturating_sub(iterations);
            projection_coefficients(
                &mut forest,
                deflated.values.as_slice(),
                returned,
                coefficients,
                weights,
                levels,
                reservation,
            )?
        };
        let reconstructed = forest.reconstruct(
            deflated.values.as_slice(),
            &coefficients,
            weights,
            reservation,
        )?;
        let error = add_up(
            add_up(deflated.roundoff_bound, reconstructed.roundoff_bound)?,
            forest.flow_error(reconstructed.values.as_slice(), weights)?,
        )?;
        let norm = weighted_slice_norm(reconstructed.values.as_slice(), weights)?;
        let guard = 64. * f64::EPSILON * rows.max(1) as f64;
        let lower_norm = norm * (1. - guard);
        let relative = if norm == 0. && error == 0. {
            0.
        } else if lower_norm > 0. && guard < 0.5 {
            add_up(error / lower_norm, 4. * f64::EPSILON)?
        } else {
            f64::INFINITY
        };
        if relative.is_finite() && relative <= allowance {
            for (coefficient, &gauge) in coefficients.iter_mut().zip(&deflated.gauge) {
                *coefficient += gauge;
                if !coefficient.is_finite() {
                    return Err(unresolved());
                }
            }
            return Ok(RepairedColumn {
                values: reconstructed.values.into_row_major(),
                norm,
                absolute_error: error,
                relative_error: relative,
                coefficients,
                report: combined_report,
                peak,
            });
        }
        if attempt == 1
            || design.effects().len() < 2
            || absorb.map.max_iterations == 0
            || absorb.lsmr.max_iterations == 0
        {
            break;
        }
        absorb.map.tolerance *= 0.01;
        absorb.lsmr.tolerance *= 0.01;
    }
    Err(unresolved())
}

/// Adopt an explicit coefficient product or derive an arbitrary FE guess.
/// Reconstruction and the graph certificate remain the accuracy authority.
fn projection_coefficients<R: FitReservation>(
    forest: &mut Forest<'_, '_>,
    shifted: &[f64],
    returned: DenseMatrix,
    coefficients: Option<FeCoefficientBatch>,
    weights: NormalizedWeights<'_>,
    levels: usize,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    Ok(if let Some(batch) = coefficients {
        let mut values = reserved_empty(reservation, levels, "rank verification coefficient copy")?;
        values.extend_from_slice(batch.rhs(0)?);
        values
    } else {
        // Some explicit accelerators return no coefficient product.
        // Their residual chooses an arbitrary FE guess only: its
        // accuracy never substitutes for feasible reconstruction and
        // the independent flow bound below. Reuse the returned row
        // bank for the guess target, keeping the helper envelope.
        let mut target = returned.into_row_major();
        if target.len() != shifted.len() {
            return Err(unresolved());
        }
        for (row, value) in target.iter_mut().enumerate() {
            if row % 4096 == 0 {
                crate::interrupt::checkpoint()?;
            }
            *value = shifted[row] - *value;
            if !value.is_finite() {
                return Err(unresolved());
            }
        }
        forest.deflate(target, weights, reservation)?.gauge
    })
}

/// Construct an exact zero quotient and an explicit FE representation when
/// original values are constant within one available partition.
fn structural_column<R: FitReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    mut value: impl FnMut(usize) -> Result<f64>,
    levels: usize,
    peak: crate::memory::SolverWorkspacePeak,
    reservation: &mut R,
) -> Result<Option<RepairedColumn>> {
    let rows = design.nobs();
    if let Some(effect_index) = rank_repair::structurally_absorbed(design, &mut value)? {
        let mut coefficients = reserved_filled(
            reservation,
            levels,
            0.,
            "rank verification exact FE coefficients",
        )?;
        if let Some(effect) = design.effects().get(effect_index) {
            let offset: usize = design.effects()[..effect_index]
                .iter()
                .map(|e| e.n_levels())
                .sum();
            for level in 0..effect.n_levels() {
                if level % 4096 == 0 {
                    crate::interrupt::checkpoint()?;
                }
                if let Some(&row) = effect.level_index().members(level).first() {
                    coefficients[offset + level] = value(row)?;
                }
            }
        } else {
            coefficients[0] = value(0)?;
        }
        return Ok(Some(RepairedColumn {
            values: reserved_filled(reservation, rows, 0., "rank verification exact zero rows")?,
            norm: 0.,
            absolute_error: 0.,
            relative_error: 0.,
            coefficients,
            report: None,
            peak,
        }));
    }
    Ok(None)
}

fn direct_coefficients<R: FitReservation>(
    design: &BorrowedInterceptFeDesign<'_>,
    shifted: &[f64],
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let levels = design
        .effects()
        .first()
        .map_or(1, |effect| effect.n_levels());
    let mut coefficients =
        reserved_filled(reservation, levels, 0., "rank verification direct means")?;
    for (level, coefficient) in coefficients.iter_mut().enumerate() {
        crate::interrupt::checkpoint()?;
        let mut sum = 0.;
        let mut correction = 0.;
        let mut mass = 0.;
        let mut mass_correction = 0.;
        let mut add = |row: usize| -> Result<()> {
            if row % 4096 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let weight = weights.value(row)?;
            compensated_add(weight * shifted[row], &mut sum, &mut correction)?;
            compensated_add(weight, &mut mass, &mut mass_correction)
        };
        if let Some(effect) = design.effects().first() {
            for &row in effect.level_index().members(level) {
                add(row)?;
            }
        } else {
            for row in 0..shifted.len() {
                add(row)?;
            }
        }
        *coefficient = if mass + mass_correction == 0. {
            0.
        } else {
            (sum + correction) / (mass + mass_correction)
        };
        if !coefficient.is_finite() {
            return Err(unresolved());
        }
    }
    Ok(coefficients)
}

fn patch_column(
    inputs: &mut OneShotInputs,
    column: Option<usize>,
    values: &[f64],
    norm: f64,
) -> Result<()> {
    match &mut inputs.storage {
        ProjectedStorage::Dense(dense) => {
            let within = dense
                .within
                .as_mut()
                .expect("rank verification projected rows");
            let physical = column.map_or(0, |column| column + 1);
            for (row, &value) in values.iter().enumerate() {
                if row % 4096 == 0 {
                    crate::interrupt::checkpoint()?;
                }
                within[(row, physical)] = value;
            }
            inputs.norms[physical] = norm;
        }
        ProjectedStorage::Scratch(low) => {
            if let Some(column) = column {
                low.scratch.patch_row_major_block(column, 1, values)?;
                inputs.norms[column] = norm;
            } else {
                for (target, source) in low.outcome.chunks_mut(4096).zip(values.chunks(4096)) {
                    crate::interrupt::checkpoint()?;
                    target.copy_from_slice(source);
                }
            }
        }
        ProjectedStorage::Centered(_) => return Err(unresolved()),
    }
    if column.is_none() {
        inputs.within_outcome_norm = norm;
    }
    if let Some(column) = column {
        match &mut inputs.fast_score_cache {
            Some(PreparedOlsFastScoreCache::Packed {
                generated_columns,
                within,
                ..
            }) => {
                if let Some(position) = generated_columns
                    .iter()
                    .position(|&generated| generated == column)
                {
                    for (row, &value) in values.iter().enumerate() {
                        if row % 4096 == 0 {
                            crate::interrupt::checkpoint()?;
                        }
                        within[(row, position)] = value;
                    }
                }
            }
            Some(PreparedOlsFastScoreCache::Selected {
                generated_columns,
                within,
                ..
            }) => {
                if generated_columns.contains(&column) {
                    for (row, &value) in values.iter().enumerate() {
                        if row % 4096 == 0 {
                            crate::interrupt::checkpoint()?;
                        }
                        within[(row, column + 1)] = value;
                    }
                }
            }
            None => {}
        }
    }
    Ok(())
}

fn record(inputs: &mut OneShotInputs, rhs: usize, repaired: RepairedColumn) -> Result<()> {
    if let Some(initial) = &mut inputs.initial_fe_coefficients {
        initial.replace_intercept_rhs(rhs, &repaired.coefficients)?;
    }
    if let Some(extra) = repaired.report {
        merge_solve_report(&mut inputs.absorption_report, extra)?;
    }
    inputs.absorption_report.solver_workspace_peak = inputs
        .absorption_report
        .solver_workspace_peak
        .max(repaired.peak);
    if crate::diagnostics::solver_trace_enabled() {
        crate::diagnostics::emit_solver_trace(&format!(
            "FEREG_SOLVER_TRACE|phase=ols_rank_verification|rhs={rhs}|status={}|relative_forward_error={:.17e}",
            if repaired.norm == 0. { "structural" } else { "resolved" }, repaired.relative_error,
        ));
    }
    Ok(())
}

fn unresolved() -> FeregError {
    invalid_data("OLS rank verification could not resolve a nearly absorbed regressor; no estimate was published")
}
