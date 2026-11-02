//! Exact structural centering with trusted raw maxima from the OLS row owner.
//! The packed RHS stays owned and contiguous; no alternate finite solver.
use super::*;
use crate::fe::{norms, AbsorbConfig};

/// Caller proves that these are the exact maxima of this owned RHS, and that
/// `constant` is its generated all-one column. Adoption of loader statistics
/// must already have checked the authoritative sample and every numeric value.
pub(crate) struct RawMaxima<'a> {
    pub(crate) scales: &'a [f64],
    pub(crate) constant: usize,
}

pub(crate) fn project(
    design: &BorrowedInterceptFeDesign<'_>,
    input: crate::ols::OlsProjectionInput,
    hint: RawMaxima<'_>,
    config: AbsorbConfig,
) -> Result<CertifiedBorrowedMap> {
    validate_config(config.map)?;
    if !design.effects().is_empty()
        || design.nobs() != input.nrows()
        || input.nrows() == 0
        || hint.scales.len() != input.ncols()
        || hint.constant >= input.ncols()
        || hint.scales[hint.constant] != 1.0
        || hint.scales.iter().any(|s| !s.is_finite() || *s < 0.0)
    {
        return Err(invalid_argument(
            "direct structural centering axes are invalid",
        ));
    }
    input.validate_finite("borrowed MAP values must be finite")?;
    let root_n = (input.nrows() as f64).sqrt();
    // The incumbent's complete certificate includes this all-one RHS. Its
    // scaled mean is exactly one and its fixed-leaf norm is sqrt(N). Reject
    // only when this exact certificate entry rejects; preserve loose-tolerance
    // zero-iteration decisions by using the complete original operation.
    if (1.0 / root_n) * root_n <= config.map.tolerance {
        return project_direct_categorical(
            design,
            BorrowedMapWeights::Unit,
            input.into_values(),
            config.map,
            config.solver_memory_budget,
            true,
        );
    }
    let planned = BorrowedMapWorkspace::planned_payload_bytes(
        design,
        input.nrows(),
        input.ncols(),
        rayon::current_num_threads(),
    )?;
    if config
        .solver_memory_budget
        .is_some_and(|b| !b.fits(planned))
    {
        return Err(allocation(
            "direct FE projection exceeds admitted workspace",
        ));
    }
    let mut trace = crate::diagnostics::phase_ledger::PhaseLedger::new(
        "structural_direct",
        ["raw_statistics", "center", "residual_norms", "certificate"],
    );
    trace.fact("rows", input.nrows() as u64);
    trace.fact("rhs", input.ncols() as u64);
    let started = Instant::now();
    let mut values = input.into_values();
    let mut workspace = BorrowedMapWorkspace::new(
        design,
        values.nrows(),
        values.ncols(),
        &mut SystemWorkspaceReservation,
    )?;
    raw_statistics(&values, hint.scales, &mut workspace)?;
    let setup_seconds = started.elapsed().as_secs_f64();
    let peak = workspace.payload_peak()?;
    trace.lap(0);
    let solve_started = Instant::now();
    subtract_structural_constant(&mut values, &workspace.group_sums)?;
    trace.lap(1);
    let weights = validate_weights(BorrowedMapWeights::Unit, values.nrows())?;
    update_workspace_norms(&values, weights, &mut workspace, false)?;
    trace.lap(2);
    // The norm pass computed exact maxima of the stored centered entries.
    // Independently sum those entries; do not infer a zero residual moment
    // from the raw statistics that constructed the projection.
    let total = structural_constant_means_with_scales(weights, &values, &mut workspace, true)?;
    for rhs in 0..values.ncols() {
        workspace.rhs_scales[rhs] =
            workspace.reference_norms[rhs].max(workspace.residual_norms[rhs]);
    }
    let certificate = structural_constant_certificate(
        weights,
        total,
        values.ncols(),
        config.map.tolerance,
        &workspace,
    )?;
    if !certificate.passed {
        return Err(invalid_data(
            "borrowed MAP structural-constant projection failed certification",
        ));
    }
    trace.lap(3);
    let mut result = certified_result(
        values,
        SolveStopReason::Certified,
        1,
        certificate,
        setup_seconds,
        solve_started.elapsed().as_secs_f64(),
    )?;
    result.set_solver_workspace_peak(peak);
    let result = result.with_residual_norms(std::mem::take(&mut workspace.residual_norms))?;
    mark_auto(result, 0, 0., 0., 0.)
}

fn raw_statistics(
    values: &DenseMatrix,
    maxima: &[f64],
    workspace: &mut BorrowedMapWorkspace,
) -> Result<()> {
    const BAND: usize = 64;
    let columns = values.ncols();
    let work = values.as_slice().len() as u128 * 2;
    let band = crate::parallel::StagePlan::current(work, columns)
        .chunk(columns)
        .min(BAND);
    crate::parallel::stage_try_for_each(
        workspace
            .group_sums
            .par_chunks_mut(band)
            .zip(workspace.group_corrections.par_chunks_mut(band))
            .zip(workspace.reference_norms.par_chunks_mut(band))
            .enumerate(),
        work,
        |(part, ((means, corrections), norms_out))| {
            let first = part * band;
            let width = means.len();
            let mut sums = [0.; BAND];
            let mut correction = [0.; BAND];
            let mut merged_scale = [0.; BAND];
            let mut merged_sum = [1.; BAND];
            let mut scale = [0.; BAND];
            let mut sum = [1.; BAND];
            for start in (0..values.nrows()).step_by(norms::LEAF_ROWS) {
                scale.fill(0.);
                sum.fill(1.);
                let end = (start + norms::LEAF_ROWS).min(values.nrows());
                for row in start..end {
                    crate::interrupt::row_checkpoint(row)?;
                    for rhs in 0..width {
                        let value = values[(row, first + rhs)];
                        let maximum = maxima[first + rhs];
                        if maximum != 0. {
                            compensated_add(value / maximum, &mut sums[rhs], &mut correction[rhs])?;
                        }
                        norms::update(
                            value,
                            &mut scale[rhs],
                            &mut sum[rhs],
                            "borrowed MAP weighted value is not finite",
                        )?;
                    }
                }
                for rhs in 0..width {
                    norms::merge(
                        scale[rhs],
                        sum[rhs],
                        &mut merged_scale[rhs],
                        &mut merged_sum[rhs],
                    );
                }
            }
            for rhs in 0..width {
                means[rhs] = if maxima[first + rhs] == 0. {
                    0.
                } else {
                    maxima[first + rhs] * ((sums[rhs] + correction[rhs]) / values.nrows() as f64)
                };
                if !means[rhs].is_finite() {
                    return Err(invalid_data(
                        "borrowed MAP structural-constant mean is not finite",
                    ));
                }
                norms_out[rhs] = norms::finish(
                    merged_scale[rhs],
                    merged_sum[rhs],
                    "borrowed MAP weighted column norm is not finite",
                )?;
            }
            corrections.copy_from_slice(&correction[..width]);
            Ok(())
        },
    )
}

#[cfg(test)]
#[path = "structural_direct_tests.rs"]
mod tests;
