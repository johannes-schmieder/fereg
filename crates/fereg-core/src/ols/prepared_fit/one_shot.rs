//! One OLS lifecycle; storage implements projection and physical finite kernels.
use super::*;
use crate::fe::routing_observer::ObserverRef;
use crate::fe::routing_remaining::Demand;

struct DenseRaw {
    values: crate::ols::OlsProjectionInput,
    moments: Option<RawMomentCache>,
    reference_norms: Vec<f64>,
    reference_mean_scales: Vec<f64>,
    strided_score_means: Option<Vec<f64>>,
}

struct DenseProjected {
    within: Option<DenseMatrix>,
    raw: DenseRaw,
    cached_norms: Vec<f64>,
}
struct ScratchProjected {
    scratch: PreparedOlsScratch,
    outcome: Vec<f64>,
}

enum ProjectedStorage {
    Dense(DenseProjected),
    Scratch(ScratchProjected),
}
struct OneShotInputs {
    storage: ProjectedStorage,
    reference_norms: Vec<f64>,
    norms: Vec<f64>,
    within_outcome_norm: f64,
    initial_sum_of_squares: f64,
    absorption_report: SolveReport,
    initial_fe_coefficients: Option<FeCoefficientBatch>,
    scratch_bytes: u64,
    fast_score_cache: Option<PreparedOlsFastScoreCache>,
}
impl OneShotInputs {
    fn score_norms(&self) -> &[f64] {
        match self.storage {
            ProjectedStorage::Dense(_) => &self.norms[1..],
            ProjectedStorage::Scratch(_) => &self.norms,
        }
    }
}

fn dense_raw<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    frame: LinearSolveFrame<'_>,
    reservation: &mut R,
    retain_fast_score_cache: bool,
    max_fast_score_cache_bytes: usize,
    loaded_rows: Option<LoadedOlsRows>,
) -> Result<DenseRaw> {
    let weights = frame.weights();
    let generated_columns = prepared.fit_model().columns.len();
    let (rows, pending_moments) =
        loaded_rows.map_or((None, None), |loaded| (Some(loaded.rows), loaded.moments));
    let (raw, reused_rows) = prepared.fit_build_rhs_with_loaded_rows(frame.response(), rows)?;
    let pending_moments = if reused_rows && weights.values.is_none() {
        pending_moments
    } else {
        None
    };
    let use_strided_fast_score_cache =
        retain_fast_score_cache && !fast_score_cache_fits(prepared, max_fast_score_cache_bytes)?;
    let mut reference_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve prepared OLS reference norms",
    )?;
    let mut reference_mean_scales = if retain_fast_score_cache {
        reserved_filled(
            reservation,
            generated_columns,
            0.0_f64,
            "unable to reserve prepared OLS fast score mean scales",
        )?
    } else {
        Vec::new()
    };
    // IID already uses the deterministic row-major raw-moment cache for an
    // absorbed intercept.  Reuse that same primitive when an inference fit is
    // too large for the accepted packed score cache: it supplies norms,
    // scales, and means in two fused passes rather than walking the row-major
    // matrix once per generated column and then scanning it again for score
    // means.  The packed path remains unchanged.
    let raw_moments = if (prepared.fit_has_absorbed_intercept() && !retain_fast_score_cache)
        || use_strided_fast_score_cache
    {
        let cache = match pending_moments {
            Some(result) => result?,
            None => build_raw_moment_cache(&raw, weights, reservation)?,
        };
        for (column, norm) in reference_norms.iter_mut().enumerate() {
            *norm = cache.norms[column + 1];
        }
        for (column, scale) in reference_mean_scales.iter_mut().enumerate() {
            *scale = cache.scales[column + 1];
        }
        Some(cache)
    } else {
        None
    };
    let threads = rayon::current_num_threads();
    let blocked_raw = threads == 1 || generated_columns.div_ceil(8) >= threads;
    if raw_moments.is_none() && blocked_raw {
        // One row-major pass per eight-column block; per-column order is the
        // scalar norm's, and the column maxima are exact.
        fill_weighted_matrix_norms_blocked::<8>(&raw, 1, weights, &mut reference_norms)?;
        if retain_fast_score_cache {
            fill_column_maxima_blocked::<8>(&raw, 1, &mut reference_mean_scales)?;
        }
    } else if retain_fast_score_cache && raw_moments.is_none() {
        crate::parallel::stage_try_for_each(
            reference_norms
                .par_iter_mut()
                .zip(reference_mean_scales.par_iter_mut())
                .enumerate(),
            raw.nrows() as u128 * generated_columns as u128,
            |(column, (norm, mean_scale))| -> Result<()> {
                *norm = weighted_matrix_column_norm(&raw, column + 1, weights)?;
                for row in 0..raw.nrows() {
                    *mean_scale = mean_scale.max(raw[(row, column + 1)].abs());
                }
                Ok(())
            },
        )?;
    } else if raw_moments.is_none() {
        crate::parallel::stage_try_for_each(
            reference_norms.par_iter_mut().enumerate(),
            raw.nrows() as u128 * generated_columns as u128,
            |(column, norm)| -> Result<()> {
                *norm = weighted_matrix_column_norm(&raw, column + 1, weights)?;
                Ok(())
            },
        )?;
    }
    let strided_score_means = if use_strided_fast_score_cache {
        let cache = raw_moments
            .as_ref()
            .ok_or_else(|| invalid_argument("prepared OLS strided score raw moments are absent"))?;
        let mut means = reserved_empty(
            reservation,
            generated_columns,
            "unable to reserve prepared OLS strided score means",
        )?;
        means.extend_from_slice(&cache.means[1..]);
        Some(means)
    } else {
        None
    };

    Ok(DenseRaw {
        values: raw,
        moments: raw_moments,
        reference_norms,
        reference_mean_scales,
        strided_score_means,
    })
}

#[allow(clippy::too_many_arguments)]
fn project_dense<'a, P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: &mut PreparedOlsFitConfig,
    reservation: &mut R,
    cmg: &mut crate::fe::FeSession<'a>,
    retain_fast_score_cache: bool,
    max_fast_score_cache_bytes: usize,
    loaded_rows: Option<LoadedOlsRows>,
    retained_fe_parent: u128,
    reuse_absorption_norms: bool,
    remaining: Option<ObserverRef<'_>>,
) -> Result<OneShotInputs> {
    let weights = frame.weights();
    let mut raw = dense_raw(
        prepared,
        frame,
        reservation,
        retain_fast_score_cache,
        max_fast_score_cache_bytes,
        loaded_rows,
    )?;
    let mut absorbed = cmg.project_initial_ols(
        weights.borrowed(),
        std::mem::replace(
            &mut raw.values,
            DenseMatrix::from_row_major(0, 0, Vec::new())?.into(),
        ),
        config.absorb,
        frame.dense_norm_handoff(config.execution.mode, reuse_absorption_norms),
        remaining.map(|state| {
            (
                state,
                Demand::Packed {
                    regressors: prepared.fit_model().columns.len(),
                },
            )
        }),
    )?;
    if !absorbed.report().certified() {
        return Err(invalid_data(
            "prepared OLS initial absorption returned an uncertified result",
        ));
    }
    let cached_norms = if reuse_absorption_norms {
        absorbed.take_residual_norms()
    } else {
        Vec::new()
    };
    let (within, mut absorption_report, initial_fe_coefficients) = absorbed.into_full_parts();
    if config.allow_final_fe_recovery && initial_fe_coefficients.is_none() {
        // Without initial products, a residual-only coefficient product cannot
        // form saved effects. The final-target recovery remains authoritative.
        config.absorb.retain_coefficients = false;
    }
    check_retained_fe_initial_capacity(initial_fe_coefficients.as_ref(), retained_fe_parent)?;
    absorption_report.solver_workspace_peak = absorption_report
        .solver_workspace_peak
        .with_retained(retained_fe_parent)?;

    let reference_norms = std::mem::take(&mut raw.reference_norms);
    Ok(OneShotInputs {
        storage: ProjectedStorage::Dense(DenseProjected {
            within: Some(within),
            raw,
            cached_norms,
        }),
        reference_norms,
        norms: Vec::new(),
        within_outcome_norm: 0.,
        initial_sum_of_squares: 0.,
        absorption_report,
        initial_fe_coefficients,
        scratch_bytes: 0,
        fast_score_cache: None,
    })
}

fn dense_finite_inputs<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    inputs: &mut OneShotInputs,
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
    retain_fast_score_cache: bool,
    max_fast_score_cache_bytes: usize,
    config: PreparedOlsFitConfig,
) -> Result<()> {
    let ProjectedStorage::Dense(dense) = &mut inputs.storage else {
        return Ok(());
    };
    let within = dense.within.as_ref().expect("projected dense owner");
    let reference_mean_scales = &dense.raw.reference_mean_scales;
    let cached_norms = std::mem::take(&mut dense.cached_norms);
    let generated_columns = prepared.fit_model().columns.len();
    // Decide before finite omission: selected views retain the complete
    // projected allocation, including outcome, omitted columns and spare capacity.
    let owned_scores = retain_fast_score_cache
        && config.ownership.retain_projected_scores
        && weights.values.is_none()
        && fast_score_cache_fits(prepared, max_fast_score_cache_bytes)?
        && config
            .absorb
            .solver_memory_budget
            .is_none_or(|budget| budget.fits(0))
        && config.retained_score_capacity_limit.is_some_and(|limit| {
            projected_score_capacity_bytes(
                within.storage_capacity(),
                generated_columns,
                generated_columns,
            )
            .is_ok_and(|bytes| bytes <= limit)
        });
    let fast_score_cache = if retain_fast_score_cache && !owned_scores {
        build_fast_score_cache(
            prepared,
            within,
            reference_mean_scales,
            weights,
            reservation,
            max_fast_score_cache_bytes,
            // Borrow numeric sources once instead of per-cell recipe dispatch;
            // tests pin the bound and unbound caches as bit-identical.
            true,
        )?
    } else {
        None
    };
    let has_cached_norms = !cached_norms.is_empty();
    let mut within_norms = if has_cached_norms {
        cached_norms
    } else {
        reserved_filled(
            reservation,
            generated_columns + 1,
            0.0_f64,
            "unable to reserve prepared OLS fused within norms",
        )?
    };
    if !has_cached_norms {
        if rayon::current_num_threads() == 1 {
            fill_weighted_matrix_norms_fused(within, weights, &mut within_norms, reservation)?;
        } else if within.ncols().div_ceil(8) >= rayon::current_num_threads() {
            // Keep enough independent blocks to occupy the existing worker team.
            fill_weighted_matrix_norms_blocked::<8>(within, 0, weights, &mut within_norms)?;
        } else {
            crate::parallel::stage_try_for_each(
                within_norms.par_iter_mut().enumerate(),
                within.nrows() as u128 * within.ncols() as u128,
                |(column, norm)| -> Result<()> {
                    *norm = weighted_matrix_column_norm(within, column, weights)?;
                    Ok(())
                },
            )?;
        }
    }
    let within_outcome_norm = within_norms[0];
    let normalized_initial_within_sum_of_squares =
        initial_within_sum_of_squares(prepared, within, weights, within_outcome_norm)?;

    inputs.fast_score_cache = fast_score_cache;
    inputs.norms = within_norms;
    inputs.within_outcome_norm = within_outcome_norm;
    inputs.initial_sum_of_squares = normalized_initial_within_sum_of_squares;
    Ok(())
}

fn project_scratch<'a, P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    cmg: &mut crate::fe::FeSession<'a>,
    reuse_absorption_norms: bool,
    mut remaining: Option<ObserverRef<'_>>,
) -> Result<OneShotInputs> {
    let rows = prepared.fit_sample().n_active();
    let generated_columns = prepared.fit_model().columns.len();
    let weights = frame.weights();
    let raw_outcome = prepared.fit_build_rhs_block_with_response(0, 0, true, frame.response())?;
    let mut absorbed_outcome = cmg.project_ols_with_remaining(
        weights.borrowed(),
        raw_outcome,
        config.absorb,
        false,
        false,
        remaining.as_mut().map(ObserverRef::reborrow).map(|state| {
            (
                state,
                Demand::Outcome {
                    regressors: generated_columns,
                    block: config.execution.poolsize.min(generated_columns.max(1)),
                },
            )
        }),
    )?;
    if !absorbed_outcome.report().certified() {
        return Err(invalid_data(
            "prepared OLS low-memory outcome absorption was not certified",
        ));
    }
    let outcome_norm = reuse_absorption_norms
        .then(|| absorbed_outcome.take_residual_norms().first().copied())
        .flatten();
    let (within_outcome_matrix, mut absorption_report) = absorbed_outcome.into_parts();
    let within_outcome = within_outcome_matrix.into_row_major();
    let within_outcome_norm = match outcome_norm {
        Some(norm) => norm,
        None => weighted_slice_norm(&within_outcome, weights)?,
    };

    let mut reference_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve low-memory prepared OLS reference norms",
    )?;
    let mut score_norms = reserved_filled(
        reservation,
        generated_columns,
        0.0_f64,
        "unable to reserve low-memory prepared OLS within score norms",
    )?;
    let mut scratch = PreparedOlsScratch::create_row_major(rows, generated_columns)?;
    let scratch_bytes = scratch.bytes();
    let intercept_cross = project_scratch_columns(
        prepared,
        frame,
        config,
        reservation,
        cmg,
        reuse_absorption_norms,
        &within_outcome,
        &mut reference_norms,
        &mut score_norms,
        &mut absorption_report,
        &mut scratch,
        remaining,
    )?;
    if let Some(lsmr) = cmg.lsmr.take() {
        cmg.binding = Some(lsmr.into_design());
    }
    let initial_sum_of_squares =
        initial_within_sum_of_squares_low(prepared, within_outcome_norm, intercept_cross)?;
    Ok(OneShotInputs {
        storage: ProjectedStorage::Scratch(ScratchProjected {
            scratch,
            outcome: within_outcome,
        }),
        reference_norms,
        norms: score_norms,
        within_outcome_norm,
        initial_sum_of_squares,
        absorption_report,
        initial_fe_coefficients: None,
        scratch_bytes,
        fast_score_cache: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn project_scratch_columns<'a, P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    cmg: &mut crate::fe::FeSession<'a>,
    reuse_absorption_norms: bool,
    within_outcome: &[f64],
    reference_norms: &mut [f64],
    score_norms: &mut [f64],
    absorption_report: &mut SolveReport,
    scratch: &mut PreparedOlsScratch,
    mut remaining: Option<ObserverRef<'_>>,
) -> Result<Option<(f64, f64)>> {
    let rows = prepared.fit_sample().n_active();
    let generated_columns = prepared.fit_model().columns.len();
    let weights = frame.weights();
    let mut column_buffer = reserved_filled(
        reservation,
        rows,
        0.0_f64,
        "unable to reserve low-memory prepared OLS column buffer",
    )?;
    let block_width = config.execution.poolsize.min(generated_columns.max(1));
    let mut intercept_cross = None;
    for start in (0..generated_columns).step_by(block_width) {
        crate::interrupt::checkpoint()?;
        let end = start.saturating_add(block_width).min(generated_columns);
        let raw_block =
            prepared.fit_build_rhs_block_with_response(start, end, false, frame.response())?;
        for (generated, reference_norm) in
            reference_norms.iter_mut().enumerate().take(end).skip(start)
        {
            let block_column = generated - start;
            for row in 0..rows {
                column_buffer[row] = raw_block[(row, block_column)];
            }
            *reference_norm = weighted_slice_norm(&column_buffer, weights)?;
        }
        let mut absorbed_block = cmg.project_ols_with_remaining(
            weights.borrowed(),
            raw_block,
            config.absorb,
            false,
            false,
            remaining.as_mut().map(ObserverRef::reborrow).map(|state| {
                (
                    state,
                    Demand::Regressors {
                        start,
                        end,
                        total: generated_columns,
                        block: block_width,
                    },
                )
            }),
        )?;
        if !absorbed_block.report().certified() {
            return Err(invalid_data(
                "prepared OLS low-memory regressor-block absorption was not certified",
            ));
        }
        let block_norms = if reuse_absorption_norms {
            absorbed_block.take_residual_norms()
        } else {
            Vec::new()
        };
        let (within_block, block_report) = absorbed_block.into_parts();
        merge_solve_report(absorption_report, block_report)?;
        scratch.patch_row_major_block(start, end - start, within_block.as_slice())?;
        for (generated, score_norm) in score_norms.iter_mut().enumerate().take(end).skip(start) {
            let block_column = generated - start;
            for row in 0..rows {
                column_buffer[row] = within_block[(row, block_column)];
            }
            *score_norm = match block_norms.get(block_column) {
                Some(norm) => *norm,
                None => weighted_slice_norm(&column_buffer, weights)?,
            };
            if prepared.fit_report_intercept()
                && !prepared.fit_has_absorbed_intercept()
                && matches!(
                    prepared.fit_model().columns[generated].kind,
                    PreparedOlsColumnKind::Intercept
                )
            {
                intercept_cross = Some((
                    weighted_slice_cross(within_outcome, &column_buffer, weights)?,
                    *score_norm,
                ));
            }
        }
    }

    Ok(intercept_cross)
}

fn solve_one_shot<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    inputs: &mut OneShotInputs,
    frame: LinearSolveFrame<'_>,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retain_fast_score_cache: bool,
) -> Result<FiniteFit> {
    let norms = match &inputs.storage {
        ProjectedStorage::Dense(_) => &inputs.norms[1..],
        ProjectedStorage::Scratch(_) => &inputs.norms,
    };
    let storage = match &mut inputs.storage {
        ProjectedStorage::Dense(dense) => finite::ProjectedFiniteRows::Dense(
            dense.within.as_mut().expect("projected dense owner"),
        ),
        ProjectedStorage::Scratch(low) => finite::ProjectedFiniteRows::Scratch {
            scratch: &mut low.scratch,
            outcome: &mut low.outcome,
        },
    };
    let qr = finite::solve_one_shot(
        prepared,
        storage,
        frame.weights(),
        frame.bread_weight_scale(),
        &inputs.reference_norms,
        norms,
        config,
        reservation,
        perturbations.first_coefficient,
        perturbations.finite_bread,
        true,
    )?;
    if let ProjectedStorage::Dense(dense) = &mut inputs.storage {
        if retain_fast_score_cache && inputs.fast_score_cache.is_none() {
            inputs.fast_score_cache = Some(build_selected_fast_score_cache(
                prepared,
                dense.within.take().expect("projected dense owner"),
                &qr,
                match dense.raw.strided_score_means.as_deref() {
                    Some(means) => SelectedScoreMeans::Strided(means),
                    None => SelectedScoreMeans::Packed {
                        weights: frame.weights(),
                        reference_scales: &dense.raw.reference_mean_scales,
                    },
                },
                reservation,
                config.retained_score_capacity_limit,
            )?);
        }
    }
    if crate::diagnostics::phase_timing_enabled() {
        trace_score_ownership(prepared, inputs, &qr);
    }
    Ok(qr)
}

fn trace_score_ownership<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    inputs: &OneShotInputs,
    qr: &FiniteFit,
) {
    let admitted = matches!(&inputs.storage, ProjectedStorage::Dense(dense)
        if dense.raw.strided_score_means.is_none())
        && matches!(
            inputs.fast_score_cache,
            Some(PreparedOlsFastScoreCache::Selected { .. })
        );
    let cached_columns = prepared
        .fit_model()
        .columns
        .iter()
        .filter(|column| {
            !(prepared.fit_has_absorbed_intercept()
                && matches!(column.kind, PreparedOlsColumnKind::Intercept))
        })
        .count();
    let avoided_elements = if admitted {
        prepared.fit_sample().n_active() as u128 * cached_columns as u128
    } else {
        0
    };
    let retained_matrix_bytes = match &inputs.fast_score_cache {
        Some(PreparedOlsFastScoreCache::Selected { within, .. }) if admitted => {
            within.storage_capacity() as u128 * std::mem::size_of::<f64>() as u128
        }
        _ => 0,
    };
    eprintln!(
        "FEREG_OWNERSHIP|score_admitted={}|avoided_packed_elements={avoided_elements}|avoided_packed_bytes={}|retained_matrix_capacity_bytes={retained_matrix_bytes}|generated_columns={}|kept_columns={}",
        u8::from(admitted),
        avoided_elements * std::mem::size_of::<f64>() as u128,
        prepared.fit_model().columns.len(),
        qr.bread.as_ref().map_or(0, |bread| bread.generated_columns.len()),
    );
}

fn original_residuals<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    frame: LinearSolveFrame<'_>,
    qr: &FiniteFit,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let rows = prepared.fit_sample().n_active();
    let mut raw_residuals = reserved_filled(
        reservation,
        rows,
        0.0_f64,
        "unable to reserve low-memory prepared OLS final residual",
    )?;
    crate::parallel::stage_try_for_each(
        raw_residuals.par_iter_mut().enumerate(),
        rows as u128 * qr.generated_coefficients.len() as u128,
        |(row, residual)| -> Result<()> {
            let mut value = frame.response()[row];
            for (column, coefficient) in qr.generated_coefficients.iter().enumerate() {
                if let Some(coefficient) = coefficient {
                    value = (-*coefficient).mul_add(prepared.fit_column_value(column, row)?, value);
                }
            }
            if !value.is_finite() {
                return Err(invalid_data(
                    "prepared OLS low-memory raw residual is not finite",
                ));
            }
            *residual = value;
            Ok(())
        },
    )?;

    Ok(raw_residuals)
}

#[allow(clippy::too_many_arguments)]
fn reported_bread<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    inputs: &OneShotInputs,
    frame: LinearSolveFrame<'_>,
    qr: &mut FiniteFit,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retain_fast_score_cache: bool,
) -> Result<(PreparedOlsBread, Vec<f64>)> {
    let (mut raw_residuals, moments, means) = match &inputs.storage {
        ProjectedStorage::Dense(dense) => {
            let needs_raw = prepared.fit_has_absorbed_intercept()
                && dense.raw.moments.is_some()
                && qr.generated_coefficients.iter().any(Option::is_some);
            (
                if needs_raw {
                    Some(build_independent_raw_residuals(
                        prepared,
                        frame.response(),
                        qr,
                        reservation,
                    )?)
                } else {
                    None
                },
                dense.raw.moments.as_ref(),
                retain_fast_score_cache.then_some(dense.raw.reference_mean_scales.as_slice()),
            )
        }
        ProjectedStorage::Scratch(_) => (
            Some(original_residuals(prepared, frame, qr, reservation)?),
            None,
            None,
        ),
    };
    let bread = finalize_reported_bread_and_intercept(
        prepared,
        frame.response(),
        qr,
        match (&inputs.storage, raw_residuals.as_deref_mut()) {
            (ProjectedStorage::Scratch(_), Some(values)) => Some(RawResiduals::Seed(values)),
            (_, values) => values.map(|values| RawResiduals::MeanOnly(values)),
        },
        frame.weights(),
        config.bread_tolerance,
        reservation,
        perturbations,
        inputs.fast_score_cache.as_ref(),
        moments,
        means,
        None,
        None,
    )?;
    let residuals = match inputs.storage {
        ProjectedStorage::Dense(_) => {
            let mut values = qr.within_residuals.take().ok_or_else(|| {
                invalid_argument("fast prepared OLS QR did not retain its within residual")
            })?;
            if perturbations.reconstructed_within_residual_constant {
                for residual in &mut values {
                    *residual += 0.25;
                }
            }
            values
        }
        ProjectedStorage::Scratch(_) => raw_residuals.expect("original residual seed"),
    };
    Ok((bread, residuals))
}

fn retained_fe_budget<P: PreparedOlsFitView + ?Sized>(
    prepared: &P,
    config: &mut PreparedOlsFitConfig,
    perturbations: FitPerturbations,
) -> Result<u128> {
    if config.execution.mode != PreparedOlsExecutionMode::Fast {
        return Ok(0);
    }
    if !perturbations.one_fe_final_recovery_disabled
        && config.allow_final_fe_recovery
        && config.absorb.retain_coefficients
        && config.absorb.solver == FeSolverKind::Auto
        && (prepared.fit_has_one_ordinary_intercept_effect() || prepared.fit_has_one_slope_effect())
    {
        // Automatic one-partition absorption already omits coefficient
        // products. Clear the unused request so fresh absorption can use its
        // direct path; publication keeps the existing certified final-target
        // recovery and memory admission. Other callers retain their policy.
        config.absorb.retain_coefficients = false;
    }
    // Admit retained products before constructing any retained solver session,
    // so its captured budget covers the later residual call as well. The
    // execution parent excludes FE storage; sequential solver calls share the
    // reserve, while the initial and combined products may coexist.
    let retained_fe_parent = if config.absorb.retain_coefficients {
        prepared.fit_retained_fe_parent_bytes()?
    } else {
        0
    };
    if let Some(budget) = config.absorb.solver_memory_budget.as_mut() {
        budget.execution_bytes = budget
            .execution_bytes
            .checked_add(retained_fe_parent)
            .ok_or_else(|| allocation("retained FE parent budget overflows"))?;
        if !budget.fits(0) {
            return Err(allocation("retained FE products exceed usable memory"));
        }
    }

    Ok(retained_fe_parent)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fit<'a, P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    mut config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturbations: FitPerturbations,
    retain_fast_score_cache: bool,
    max_fast_score_cache_bytes: usize,
    phase_probe: Option<LinearSessionPhaseAllocatorProbe>,
    mut phase_observation: Option<&mut LinearSessionPhaseAllocationObservation>,
    loaded_rows: Option<LoadedOlsRows>,
    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,
) -> Result<CertifiedPreparedOlsFit> {
    let mut trace = PhaseTrace::new();
    crate::interrupt::checkpoint()?;
    frame.validate_for(prepared)?;
    let weights = frame.weights();
    let reuse_absorption_norms = weights.matches_fe_norm_scale();
    let dense = config.execution.mode == PreparedOlsExecutionMode::Fast;
    let retained_fe_parent = retained_fe_budget(prepared, &mut config, perturbations)?;
    let mut cmg = crate::fe::FeSession::for_ols(prepared, weights.borrowed(), config)?;
    let mut legacy = cmg.remaining_state();
    let mut gates = cmg.gates_state();
    let mut remaining = legacy
        .as_mut()
        .map(ObserverRef::Remaining)
        .or_else(|| gates.as_mut().map(ObserverRef::Gates));
    let projection_phase = LinearSessionPhaseAllocationGuard::new(
        phase_probe,
        phase_observation.as_deref_mut(),
        LinearSessionAllocationPhase::Projection,
    );
    let projection = if dense {
        project_dense(
            prepared,
            frame,
            &mut config,
            reservation,
            &mut cmg,
            retain_fast_score_cache,
            max_fast_score_cache_bytes,
            loaded_rows,
            retained_fe_parent,
            reuse_absorption_norms,
            remaining.as_mut().map(ObserverRef::reborrow),
        )
    } else {
        project_scratch(
            prepared,
            frame,
            config,
            reservation,
            &mut cmg,
            reuse_absorption_norms,
            remaining.as_mut().map(ObserverRef::reborrow),
        )
    };
    cmg.close_routing_attempt();
    // Cancellation requested during the projection takes precedence.
    crate::interrupt::checkpoint()?;
    let mut inputs = projection?;
    drop(projection_phase);
    let projection_seconds = trace.lap();
    let finite_phase = LinearSessionPhaseAllocationGuard::new(
        phase_probe,
        phase_observation.as_deref_mut(),
        LinearSessionAllocationPhase::FiniteSolve,
    );
    dense_finite_inputs(
        prepared,
        &mut inputs,
        weights,
        reservation,
        retain_fast_score_cache,
        max_fast_score_cache_bytes,
        config,
    )?;
    crate::interrupt::checkpoint()?;
    let mut qr = solve_one_shot(
        prepared,
        &mut inputs,
        frame,
        config,
        reservation,
        perturbations,
        retain_fast_score_cache,
    )?;
    let finite_seconds = trace.lap();
    let (bread, seed) = reported_bread(
        prepared,
        &inputs,
        frame,
        &mut qr,
        config,
        reservation,
        perturbations,
        retain_fast_score_cache,
    )?;
    let initial_absorbed_outcome_sum_of_squares = scaled_initial_sum_of_squares(
        inputs.initial_sum_of_squares,
        bread.weight_scale().reference_weight_multiplier,
        dense,
    )?;
    drop(finite_phase);
    let bread_seconds = trace.lap();
    let residual_projection_phase = LinearSessionPhaseAllocationGuard::new(
        phase_probe,
        phase_observation,
        LinearSessionAllocationPhase::Projection,
    );
    crate::interrupt::checkpoint()?;
    let derived_scale = derived_scale(dense, config, frame.response(), weights, &inputs, &qr)?;
    let (residuals, residual_absorption_report, residual_fe_coefficients) = project_residual(
        &mut cmg,
        prepared.fit_sample().n_active(),
        seed,
        weights,
        config,
        retained_fe_parent,
        perturbations.final_residual_constant,
        remaining.as_mut().map(ObserverRef::reborrow),
        derived_scale.map(|scale| (scale, &inputs.absorption_report)),
    )?;
    if !dense {
        drop(cmg);
    }
    drop(residual_projection_phase);
    let residual_seconds = trace.lap();
    let fit = finish_one_shot(
        prepared,
        inputs,
        qr,
        residuals,
        residual_absorption_report,
        residual_fe_coefficients,
        retained_fe_parent,
        initial_absorbed_outcome_sum_of_squares,
        bread,
        config,
        weights,
        reservation,
        retained_scratch,
    )?;
    if trace.enabled {
        eprintln!("FEREG_PHASE|fit|projection={projection_seconds:.9}|finite_solve={finite_seconds:.9}|bread_raw_residual={bread_seconds:.9}|residual_absorb={residual_seconds:.9}|finalize={:.9}|total={:.9}",
            trace.lap(), trace.total());
    }
    Ok(fit)
}

fn scaled_initial_sum_of_squares(value: f64, multiplier: f64, dense: bool) -> Result<f64> {
    let scaled = value * multiplier;
    if !scaled.is_finite() || scaled < 0.0 {
        return Err(invalid_data(if dense {
            "prepared OLS initial absorbed outcome sum of squares is invalid"
        } else {
            "prepared OLS low-memory initial absorbed outcome sum of squares is invalid"
        }));
    }
    Ok(scaled)
}

/// Fast OLS without residual, xbd, d or saved-FE outputs keeps its within
/// residual when its derived certificate passes (SPEC §7, §16); this is the
/// scale at which it is checked, or `None` when the final projection runs.
fn derived_scale(
    dense: bool,
    config: PreparedOlsFitConfig,
    response: &[f64],
    weights: NormalizedWeights<'_>,
    inputs: &OneShotInputs,
    qr: &FiniteFit,
) -> Result<Option<f64>> {
    if !dense || config.absorb.retain_coefficients || config.final_residual_tolerance.is_some() {
        return Ok(None);
    }
    derived_residual_scale(
        weighted_response_norm(response, weights)?,
        &inputs.reference_norms,
        &inputs.norms,
        &qr.generated_coefficients,
    )
    .map(Some)
}

fn weighted_response_norm(response: &[f64], weights: NormalizedWeights<'_>) -> Result<f64> {
    let mut scale = 0.0_f64;
    let mut scaled_square_sum = 1.0_f64;
    for (row, &value) in response.iter().enumerate() {
        update_scaled_sum_of_squares(
            weights.root(row)? * value,
            &mut scale,
            &mut scaled_square_sum,
        )?;
    }
    checked_norm(scale, scaled_square_sum)
}

/// `s_y + Σ|b_j| s_j` over kept columns, each `s` the §16 scale at which the
/// projection certified that column (the larger of its raw and within weighted
/// norms): by linearity the moments of `ỹ - X̃b` are at most tolerance times it.
fn derived_residual_scale(
    raw_outcome_norm: f64,
    raw_norms: &[f64],
    within_norms: &[f64],
    coefficients: &[Option<f64>],
) -> Result<f64> {
    if within_norms.len() != coefficients.len() + 1 || raw_norms.len() != coefficients.len() {
        return Err(invalid_argument(
            "derived residual scale needs one raw and within norm per column",
        ));
    }
    let mut scale = raw_outcome_norm.max(within_norms[0]);
    for ((raw, within), coefficient) in raw_norms.iter().zip(&within_norms[1..]).zip(coefficients) {
        if let Some(value) = coefficient {
            scale += value.abs() * raw.max(*within);
        }
    }
    if scale.is_finite() && scale >= 0.0 {
        Ok(scale)
    } else {
        Err(invalid_data("derived residual scale is invalid"))
    }
}

/// The derived certificate of a fast within residual, or `None` when this
/// session cannot recompute it or it does not pass.
fn derived_residual(
    cmg: &mut crate::fe::FeSession<'_>,
    matrix: &DenseMatrix,
    weights: NormalizedWeights<'_>,
    config: PreparedOlsFitConfig,
    scale: f64,
    absorption: &SolveReport,
) -> Result<Option<SolveReport>> {
    let started = std::time::Instant::now();
    let norm = weighted_matrix_column_norm(matrix, 0, weights)?;
    // r is at most its terms' sum; rounding can only make the ratio smaller.
    let amplification = if norm > 0.0 {
        (scale / norm).max(1.0)
    } else {
        1.0
    };
    let tolerance = config.final_absorb_config().map.tolerance;
    let Some(certificate) =
        cmg.certify_derived_residual(weights.borrowed(), matrix, tolerance, amplification)?
    else {
        return Ok(None);
    };
    if !certificate.passed {
        return Ok(None);
    }
    Ok(Some(SolveReport {
        iterations: 0,
        checkpoint_retries: 0,
        stop_reason: SolveStopReason::DerivedCertificate,
        certificate,
        normal_equation_residual: 0.0,
        setup_seconds: 0.0,
        solve_seconds: 0.0,
        certification_seconds: started.elapsed().as_secs_f64(),
        fallback_used: false,
        pilot_iterations: 0,
        pilot_contraction: 0.0,
        warnings: Vec::new(),
        ..absorption.clone()
    }))
}

#[allow(clippy::too_many_arguments)]
fn project_residual(
    cmg: &mut crate::fe::FeSession<'_>,
    rows: usize,
    seed: Vec<f64>,
    weights: NormalizedWeights<'_>,
    config: PreparedOlsFitConfig,
    retained_fe_parent: u128,
    perturb_constant: bool,
    remaining: Option<ObserverRef<'_>>,
    derived: Option<(f64, &SolveReport)>,
) -> Result<(DenseMatrix, SolveReport, Option<FeCoefficientBatch>)> {
    let dense = config.execution.mode == PreparedOlsExecutionMode::Fast;
    let matrix = DenseMatrix::from_row_major(rows, 1, seed)?;
    if let Some((scale, absorption)) = derived {
        if let Some(report) = derived_residual(cmg, &matrix, weights, config, scale, absorption)? {
            let mut residuals = matrix;
            if perturb_constant {
                for row in 0..residuals.nrows() {
                    residuals[(row, 0)] += 0.25;
                }
            }
            return Ok((residuals, report, None));
        }
    }
    // Original-source seeds contain the full FE contribution. They require the
    // full projector; only dense within seeds may use terminal residual polish.
    let residual_absorbed = cmg.project_ols_with_remaining(
        weights.borrowed(),
        matrix,
        config.final_absorb_config(),
        true,
        !dense,
        remaining.map(|state| (state, Demand::Final)),
    )?;
    if !residual_absorbed.report().certified() {
        return Err(invalid_data(if dense {
            "prepared OLS residual absorption returned an uncertified result"
        } else {
            "prepared OLS low-memory residual absorption was not certified"
        }));
    }
    let (mut residuals, mut residual_absorption_report, residual_fe_coefficients) =
        residual_absorbed.into_full_parts();
    if dense {
        residual_absorption_report.solver_workspace_peak = residual_absorption_report
            .solver_workspace_peak
            .with_retained(retained_fe_parent)?;
    }
    if perturb_constant {
        for row in 0..residuals.nrows() {
            residuals[(row, 0)] += 0.25;
        }
    }
    Ok((
        residuals,
        residual_absorption_report,
        residual_fe_coefficients,
    ))
}

#[allow(clippy::too_many_arguments)]
fn finish_one_shot<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    mut inputs: OneShotInputs,
    qr: FiniteFit,
    residuals: DenseMatrix,
    residual_absorption_report: SolveReport,
    residual_fe_coefficients: Option<FeCoefficientBatch>,
    retained_fe_parent: u128,
    initial_absorbed_outcome_sum_of_squares: f64,
    bread: PreparedOlsBread,
    config: PreparedOlsFitConfig,
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
    retained_scratch: Option<&mut Option<PreparedOlsScratch>>,
) -> Result<CertifiedPreparedOlsFit> {
    let score_certificate = certify_scores(
        prepared,
        residuals.as_slice(),
        weights,
        &inputs.reference_norms,
        inputs.score_norms(),
        qr.finite_score,
        &qr.generated_statuses,
        inputs.within_outcome_norm,
        config.score_tolerance,
    )?;
    if !score_certificate.passed {
        return Err(invalid_data(
            if config.execution.mode == PreparedOlsExecutionMode::Fast {
                "prepared OLS finite score certificate did not pass"
            } else {
                "prepared OLS low-memory finite score certificate did not pass"
            },
        ));
    }
    let recovered_fe_coefficients = if config.execution.mode == PreparedOlsExecutionMode::Fast {
        combine_recovered_fe_coefficients(
            inputs.initial_fe_coefficients.take(),
            residual_fe_coefficients,
            residual_absorption_report.iterations,
            &qr.generated_coefficients,
            retained_fe_parent,
            reservation,
        )?
    } else {
        None
    };
    let coefficients = build_coefficient_slots(prepared, &qr, reservation)?;
    if let (ProjectedStorage::Scratch(low), Some(destination)) = (inputs.storage, retained_scratch)
    {
        *destination = Some(low.scratch);
    }
    Ok(qr.publish(
        coefficients,
        FitPublication {
            score_certificate,
            bread,
            absorption_report: inputs.absorption_report,
            residual_absorption_report,
            initial_absorbed_outcome_sum_of_squares,
            residuals,
            recovered_fe_coefficients,
            fast_score_cache: inputs.fast_score_cache,
            execution: PreparedOlsExecutionReport::from_config(config, inputs.scratch_bytes),
        },
    ))
}
