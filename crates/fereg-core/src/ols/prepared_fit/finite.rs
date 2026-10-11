//! One-shot adapter to the common stateless immutable-row finite service.
use super::*;
use crate::ols::finite_operations::{resize, solve_factor, FactorBanks, RankRequest};
use crate::ols::finite_wls_kernel::{
    fill_finite_wls_moments, FiniteDenseRows, FiniteResponse, FiniteWorkspace,
};

pub(super) enum ProjectedFiniteRows<'a> {
    Dense(&'a DenseMatrix),
    Scratch {
        scratch: &'a PreparedOlsScratch,
        outcome: &'a [f64],
    },
}
impl ProjectedFiniteRows<'_> {
    fn source<'a>(
        &'a self,
        selected: &'a [usize],
        scales: &'a [f64],
        weights: NormalizedWeights<'a>,
        weight_scale: PreparedOlsBreadWeightScale,
    ) -> FixedBlockMomentSource<'a> {
        match self {
            Self::Dense(values) => FixedBlockMomentSource::Dense {
                values: FiniteDenseRows::projected(values),
                rows: values.nrows(),
                stored_columns: values.ncols() - 1,
                selected_columns: Some(selected),
                response: FiniteResponse::Column(values, 0),
                weights: weights.borrowed(),
                solver_weight_divisor: weight_scale.solver_weight_divisor,
                column_scales: Some(scales),
            },
            Self::Scratch { scratch, outcome } => FixedBlockMomentSource::Scratch {
                values: scratch,
                selected_columns: Some(selected),
                response: (*outcome).into(),
                weights: weights.borrowed(),
                solver_weight_divisor: weight_scale.solver_weight_divisor,
                column_scales: Some(scales),
            },
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn solve_one_shot<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    storage: ProjectedFiniteRows<'_>,
    weights: NormalizedWeights<'_>,
    bread_weight_scale: PreparedOlsBreadWeightScale,
    reference_norms: &[f64],
    score_norms: &[f64],
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_first_coefficient: bool,
    perturb_bread: bool,
    build_bread: bool,
) -> Result<FiniteFit> {
    let rows = prepared.fit_sample().n_active();
    let columns = prepared.fit_model().columns.len();
    let valid = match &storage {
        ProjectedFiniteRows::Dense(values) => {
            values.nrows() == rows && values.ncols() == columns + 1
        }
        ProjectedFiniteRows::Scratch { scratch, outcome } => {
            scratch.rows() == rows && scratch.columns() == columns && outcome.len() == rows
        }
    };
    if !valid || reference_norms.len() != columns || score_norms.len() != columns {
        return Err(invalid_argument(
            "prepared finite projected axes are inconsistent",
        ));
    }
    let mut workspace = FiniteWorkspace::<()>::default();
    let mut statuses = reserved_filled(
        reservation,
        columns,
        PreparedOlsFitStatus::NumericallyRankDeficient,
        "unable to reserve finite statuses",
    )?;
    reservation.reserve(
        &mut workspace.candidate_ids,
        columns,
        "unable to reserve finite candidates",
    )?;
    classify_qr_candidates(
        prepared,
        &mut statuses,
        &mut workspace.candidate_ids,
        config.fixed_effect_collinearity_tolerance,
        "finite candidate capacity exhausted",
        |column| match storage {
            ProjectedFiniteRows::Dense(_) => {
                fast_qr_norms(reference_norms[column], score_norms[column])
            }
            ProjectedFiniteRows::Scratch { .. } => {
                low_qr_norms(reference_norms[column], score_norms[column])
            }
        },
    )?;
    reservation.reserve(
        &mut workspace.candidate_scales,
        workspace.candidate_ids.len(),
        "unable to reserve finite scales",
    )?;
    workspace.candidate_scales.extend(
        workspace
            .candidate_ids
            .iter()
            .map(|&column| reference_norms[column]),
    );
    let relative_threshold =
        config.rank_tolerance_multiplier * f64::EPSILON * rows.max(columns).max(1) as f64;
    let (report, bread) = if workspace.candidate_ids.is_empty() {
        let report = super::super::finite_wls_kernel::RankRevealingQrReport {
            rank: 0,
            initial_norm: 0.,
            maximum_dropped_relative_norm: 0.,
            condition_proxy: 1.,
            ambiguous: false,
        };
        let bread = if build_bread {
            Some(build_one_shot_bread(
                &mut workspace,
                reference_norms,
                bread_weight_scale,
                config,
                reservation,
                perturb_bread,
            )?)
        } else {
            None
        };
        (report, bread)
    } else {
        let source = storage.source(
            &workspace.candidate_ids,
            &workspace.candidate_scales,
            weights,
            bread_weight_scale,
        );
        let layout_source = validate_moment_source(source)?;
        let layout = layout_source.layout();
        let count = workspace.candidate_ids.len();
        resize(
            &mut workspace.moment_partials,
            layout.partial_elements,
            0.,
            reservation,
        )?;
        resize(
            &mut workspace.gram,
            checked_square(count, "finite Gram dimensions overflow")?,
            0.,
            reservation,
        )?;
        resize(&mut workspace.rhs, count, 0., reservation)?;
        fill_finite_wls_moments(
            layout_source,
            &mut workspace.moment_partials,
            &mut workspace.rhs,
            &mut workspace.gram,
        )?;
        factor_and_bread(
            &storage,
            weights,
            &mut workspace,
            relative_threshold,
            reference_norms,
            bread_weight_scale,
            config,
            reservation,
            perturb_bread,
            build_bread,
        )?
    };
    finish(
        prepared,
        storage,
        workspace,
        statuses,
        report,
        relative_threshold,
        bread,
        reference_norms,
        score_norms,
        reservation,
        perturb_first_coefficient,
    )
}

#[allow(clippy::too_many_arguments)]
fn factor_and_bread<R: FitReservation>(
    storage: &ProjectedFiniteRows<'_>,
    weights: NormalizedWeights<'_>,
    workspace: &mut FiniteWorkspace,
    threshold: f64,
    reference_norms: &[f64],
    weight_scale: PreparedOlsBreadWeightScale,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_bread: bool,
    build_bread: bool,
) -> Result<(
    super::super::finite_wls_kernel::RankRevealingQrReport,
    Option<FiniteBreadDraft>,
)> {
    let mut allow_cholesky = true;
    loop {
        let source = storage.source(
            &workspace.candidate_ids,
            &workspace.candidate_scales,
            weights,
            weight_scale,
        );
        let factor = solve_factor(
            source,
            &workspace.candidate_ids,
            threshold,
            RankRequest::OlsDiscover,
            allow_cholesky,
            FactorBanks {
                gram: &workspace.gram,
                rhs: &workspace.rhs,
                factor: &mut workspace.factor,
                auxiliary: &mut workspace.inverse_lower,
                solve: &mut workspace.solve,
                scores: &mut workspace.scores,
                row: &mut workspace.row,
                permutation: &mut workspace.permutation,
                coefficients: &mut workspace.coefficients,
                inverse: &mut workspace.bread,
                selected: &mut workspace.kept,
            },
            reservation,
            |_, _, _| Ok(()),
        )?;
        let bread = if build_bread {
            match build_one_shot_bread(
                workspace,
                reference_norms,
                weight_scale,
                config,
                reservation,
                perturb_bread,
            ) {
                Ok(bread) => Some(bread),
                Err(error)
                    if !factor.used_qr
                        && !perturb_bread
                        && error.code == ErrorCode::InvalidData =>
                {
                    // The complete inverse/reference-scale finalization is the
                    // OLS trial boundary. Its one rejected Cholesky trial may
                    // retry QR; allocation/argument errors and QR errors fail.
                    allow_cholesky = false;
                    continue;
                }
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        return Ok((factor.report, bread));
    }
}

fn build_one_shot_bread<R: FitReservation>(
    workspace: &mut FiniteWorkspace,
    reference_norms: &[f64],
    weight_scale: PreparedOlsBreadWeightScale,
    config: PreparedOlsFitConfig,
    reservation: &mut R,
    perturb_bread: bool,
) -> Result<FiniteBreadDraft> {
    let mut identities = reserved_empty(
        reservation,
        workspace.kept.len(),
        "unable to reserve finite bread identities",
    )?;
    identities.extend_from_slice(&workspace.kept);
    let mut positions = reserved_empty(
        reservation,
        workspace.kept.len(),
        "unable to reserve finite selected positions",
    )?;
    for &column in &workspace.kept {
        positions.push(
            workspace
                .candidate_ids
                .iter()
                .position(|&id| id == column)
                .ok_or_else(|| invalid_argument("finite selected identity is absent"))?,
        );
    }
    finalize_finite_bread_draft(
        &workspace.candidate_ids,
        &workspace.gram,
        identities,
        positions,
        std::mem::take(&mut workspace.bread),
        reference_norms,
        weight_scale,
        config.bread_tolerance,
        reservation,
        perturb_bread,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish<P: PreparedOlsFitView + ?Sized, R: FitReservation>(
    prepared: &P,
    storage: ProjectedFiniteRows<'_>,
    workspace: FiniteWorkspace,
    mut statuses: Vec<PreparedOlsFitStatus>,
    report: super::super::finite_wls_kernel::RankRevealingQrReport,
    relative_threshold: f64,
    bread: Option<FiniteBreadDraft>,
    reference_norms: &[f64],
    score_norms: &[f64],
    reservation: &mut R,
    perturb_coefficient: bool,
) -> Result<FiniteFit> {
    let columns = prepared.fit_model().columns.len();
    let mut coefficients = reserved_filled(
        reservation,
        columns,
        None,
        "unable to reserve finite coefficients",
    )?;
    for (position, &generated) in workspace.kept.iter().enumerate() {
        let value = workspace.coefficients[position] / reference_norms[generated];
        if !value.is_finite() {
            return Err(invalid_data("prepared OLS QR coefficient is not finite"));
        }
        coefficients[generated] = Some(value);
        statuses[generated] = PreparedOlsFitStatus::Kept;
    }
    if perturb_coefficient {
        *coefficients.iter_mut().flatten().next().ok_or_else(|| {
            invalid_argument("injected perturbation requires a kept coefficient")
        })? += 0.25;
    }
    let finite_score = build_finite_score_draft(
        &workspace.candidate_ids,
        &workspace.gram,
        &workspace.rhs,
        &coefficients,
        &statuses,
        reference_norms,
        score_norms,
    )?;
    let within_residuals = match storage {
        ProjectedFiniteRows::Dense(within) => Some(dense_within_residuals(
            within,
            &workspace.kept,
            &coefficients,
            reservation,
        )?),
        ProjectedFiniteRows::Scratch { .. } => None,
    };
    let mut dropped = reserved_empty(
        reservation,
        columns,
        "unable to reserve finite dropped identities",
    )?;
    for (column, status) in statuses.iter().enumerate() {
        if *status != PreparedOlsFitStatus::Kept {
            dropped.push(column);
        }
    }
    // The finite service never overwrites projected rows, including QR cases.
    Ok(FiniteFit {
        generated_coefficients: coefficients,
        generated_statuses: statuses,
        kept: workspace.kept,
        dropped,
        rank: report.rank,
        certificate: PreparedOlsRankCertificate {
            relative_threshold,
            maximum_dropped_trailing_norm: report.maximum_dropped_relative_norm,
            condition_proxy: report.condition_proxy,
            passed: true,
        },
        finite_score,
        bread,
        within_residuals,
    })
}

fn dense_within_residuals<R: FitReservation>(
    within: &DenseMatrix,
    kept: &[usize],
    coefficients: &[Option<f64>],
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let mut residuals = reserved_empty(
        reservation,
        within.nrows(),
        "unable to reserve finite residuals",
    )?;
    let hardware = crate::kernels::installed();
    let residual = |row: &[f64]| {
        // Immutable projected coordinates retain the physical FE owner. Each
        // row keeps the same selected-column order and fused arithmetic.
        let value = crate::kernels::dispatch!(
            hardware,
            selected_residual(row[0], &row[1..], kept, coefficients)
        );
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid_data("prepared finite residual is not finite"))
        }
    };
    let work = within.nrows() as u128 * (kept.len() as u128 + 1);
    if rayon::current_thread_index().is_some()
        && crate::parallel::StagePlan::current(work, within.nrows()).lanes > 1
    {
        // The reservation and capacity guard above remain fallible and serial.
        // Workers borrow initialized, disjoint outputs; ordered error merging
        // retains the first failing row without creating a separate team.
        residuals.resize(within.nrows(), 0.0);
        crate::parallel::stage_try_for_each(
            residuals
                .par_iter_mut()
                .zip(within.as_slice().par_chunks_exact(within.ncols())),
            work,
            |(target, row)| {
                *target = residual(row)?;
                Ok(())
            },
        )?;
    } else {
        for row in within.as_slice().chunks_exact(within.ncols()) {
            residuals.push(residual(row)?);
        }
    }
    Ok(residuals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_parallel_finite_residuals_match_ordered_scalar_bits() {
        for (rows, columns) in [(0, 0), (1, 1), (16_385, 7), (32_771, 20), (8_193, 65)] {
            let within = DenseMatrix::from_row_major(
                rows,
                columns + 1,
                (0..rows * (columns + 1))
                    .map(|i| [0.0, -0.0, 1e-200, 1e100, -1e100, 0.125, -0.75][i % 7])
                    .collect(),
            )
            .unwrap();
            let kept: Vec<_> = (0..columns).filter(|c| c % 3 != 1).rev().collect();
            let mut coefficients: Vec<Option<f64>> = vec![None; columns];
            for &column in &kept {
                coefficients[column] = Some([-0.0, 0.5, -0.25, 1e-100][column % 4]);
            }
            // Independent copy of the scalar recurrence: do not call the
            // production residual kernel or share its row partitioning.
            let expected: Vec<_> = (0..rows)
                .map(|row| {
                    let mut value = within[(row, 0)];
                    for &column in &kept {
                        value = (-coefficients[column].unwrap())
                            .mul_add(within[(row, column + 1)], value);
                    }
                    value.to_bits()
                })
                .collect();
            for workers in [1, 4, 16] {
                let actual = ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap())
                    .install(|| {
                        dense_within_residuals(
                            &within,
                            &kept,
                            &coefficients,
                            &mut SystemFitReservation,
                        )
                    })
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    expected,
                    "rows={rows}, columns={columns}, workers={workers}"
                );
            }
        }
    }

    #[test]
    fn row_parallel_finite_residuals_preserve_nonfinite_failure() {
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for bad_row in [0, 16_384, 32_770] {
                let mut values = vec![1.0; 32_771 * 2];
                values[bad_row * 2] = invalid;
                let within = DenseMatrix::from_row_major(32_771, 2, values).unwrap();
                for workers in [1, 4, 16] {
                    let error =
                        ThreadTeam::new(ThreadConfig::from_requested(workers, false).unwrap())
                            .install(|| {
                                dense_within_residuals(
                                    &within,
                                    &[0],
                                    &[Some(0.5)],
                                    &mut SystemFitReservation,
                                )
                            })
                            .unwrap()
                            .unwrap_err();
                    assert_eq!(
                        error,
                        invalid_data("prepared finite residual is not finite")
                    );
                }
            }
        }
    }
}
