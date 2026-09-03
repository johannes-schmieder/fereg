//! PPML-only fused FWL engine for categorical LSMR.
//!
//! Unlike the legacy frozen session, this route residualizes the active
//! controls and current working response in one shared `within::Solver`
//! batch and certifies the resulting finite-plus-FE normal equations exactly.

use std::time::Instant;

use within::{
    Effect, LsmrOptions, Preconditioner, PreconditionerConfig, PreconditionerInput, Solver,
};

use crate::error::{ErrorCode, FeregError, Result};
use crate::fe::FeDesign;
use crate::options::ThreadConfig;
use crate::parallel::{InstalledThreadTeam, ThreadTeam};
use crate::prepared_fit_view::PreparedOlsFitView;

use super::categorical_lsmr::{
    add_relative, bytes_for_memory, checked_add_memory, checked_mul_memory, checked_sum_memory,
    CategoricalLsmrMemoryShape, CategoricalLsmrPolicy, FeTopologyIdentity, FiniteCompensatedSum,
    JointLsmrCertificate, ScaledSquareSum,
};
use super::PpmlIterationWorkspace;

#[cfg(test)]
static TEST_FUSED_LSMR_EXECUTING_WIDTH: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FusedFwlLsmrReport {
    pub(crate) iterations: usize,
    pub(crate) effective_tolerance: f64,
    pub(crate) setup_seconds: f64,
    pub(crate) solve_seconds: f64,
    pub(crate) preconditioner_reuse_attempted: bool,
    pub(crate) preconditioner_reuse_certified: bool,
    pub(crate) rebuilt_preconditioner: bool,
    pub(crate) continued_after_certificate: bool,
    pub(crate) used_schwarz: bool,
    pub(crate) certificate: JointLsmrCertificate,
}

pub(crate) struct ReusableCategoricalFwlLsmr {
    team: InstalledThreadTeam,
    active_columns: Vec<Vec<f64>>,
    active_slots: Vec<usize>,
    topology: FeTopologyIdentity,
    preconditioner: Option<Preconditioner>,
    gram: Vec<f64>,
    rhs: Vec<f64>,
    factor: Vec<f64>,
    forward: Vec<f64>,
    standardized_solution: Vec<f64>,
    scales: Vec<f64>,
    coefficients: Vec<f64>,
    permutation: Vec<usize>,
    residual: Vec<f64>,
    fitted_without_offset: Vec<f64>,
    policy: CategoricalLsmrPolicy,
}

impl ReusableCategoricalFwlLsmr {
    pub(crate) fn new_with_threads<P: PreparedOlsFitView + Sync + ?Sized>(
        prepared: &P,
        active_generated: &[usize],
        fe_design: &FeDesign,
        threads: ThreadConfig,
        policy: CategoricalLsmrPolicy,
    ) -> Result<Self> {
        let rows = fe_design.nobs();
        if rows != prepared.fit_sample().n_active()
            || fe_design
                .effects()
                .iter()
                .any(|effect| !effect.has_intercept() || !effect.slopes().is_empty())
        {
            return Err(invalid_argument(
                "fused PPML FWL requires an aligned categorical intercept-only design",
            ));
        }
        let topology = FeTopologyIdentity::capture(fe_design)?;
        let team = ThreadTeam::new(threads).acquire()?;
        let mut active_columns = Vec::new();
        active_columns
            .try_reserve_exact(active_generated.len())
            .map_err(|_| allocation("unable to reserve fused PPML active controls"))?;
        let mut active_slots = Vec::new();
        active_slots
            .try_reserve_exact(active_generated.len())
            .map_err(|_| allocation("unable to reserve fused PPML coefficient slots"))?;
        for &generated in active_generated {
            let mut column = fallible_filled(rows, 0.0, "fused PPML active control")?;
            for (row, value) in column.iter_mut().enumerate() {
                *value = prepared.fit_column_value(generated, row)?;
            }
            active_columns.push(column);
            let slot = prepared
                .fit_model()
                .coefficients
                .iter()
                .position(|coefficient| coefficient.generated_column_index == Some(generated))
                .ok_or_else(|| {
                    FeregError::new(
                        ErrorCode::Internal,
                        "fused PPML active column has no coefficient slot",
                    )
                })?;
            active_slots.push(slot);
        }
        let columns = active_columns.len();
        let square = columns
            .checked_mul(columns)
            .ok_or_else(|| allocation("fused PPML finite dimensions overflow"))?;
        Ok(Self {
            team,
            active_columns,
            active_slots,
            topology,
            preconditioner: None,
            gram: fallible_filled(square, 0.0, "fused PPML finite Gram")?,
            rhs: fallible_filled(columns, 0.0, "fused PPML finite RHS")?,
            factor: fallible_filled(square, 0.0, "fused PPML finite factor")?,
            forward: fallible_filled(columns, 0.0, "fused PPML finite forward solve")?,
            standardized_solution: fallible_filled(
                columns,
                0.0,
                "fused PPML standardized coefficients",
            )?,
            scales: fallible_filled(columns, 0.0, "fused PPML finite scales")?,
            coefficients: fallible_filled(columns, 0.0, "fused PPML finite coefficients")?,
            permutation: fallible_filled(columns, 0, "fused PPML finite permutation")?,
            residual: fallible_filled(rows, 0.0, "fused PPML residual")?,
            fitted_without_offset: fallible_filled(rows, 0.0, "fused PPML fitted predictor")?,
            policy,
        })
    }

    pub(crate) fn solve(
        &mut self,
        fe_design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
        local_size: Option<usize>,
    ) -> Result<FusedFwlLsmrReport> {
        self.validate_frame(
            fe_design,
            working_response,
            weights,
            tolerance,
            max_iterations,
        )?;
        let team = self.team.clone();
        team.install(|| {
            self.solve_installed(
                fe_design,
                working_response,
                weights,
                tolerance,
                max_iterations,
                local_size,
            )
        })
    }

    fn solve_installed(
        &mut self,
        fe_design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
        local_size: Option<usize>,
    ) -> Result<FusedFwlLsmrReport> {
        #[cfg(test)]
        TEST_FUSED_LSMR_EXECUTING_WIDTH.store(
            rayon::current_num_threads(),
            std::sync::atomic::Ordering::Relaxed,
        );
        let preconditioner_reuse_attempted =
            self.policy == CategoricalLsmrPolicy::Reuse && self.preconditioner.is_some();
        let mut setup_seconds = 0.0_f64;
        let mut solve_seconds = 0.0_f64;
        let mut iterations = 0_usize;
        let mut rebuilt_preconditioner = false;
        let mut retry = false;
        let mut continued_after_certificate = false;
        let mut used_schwarz = false;
        let mut effective_tolerance = tolerance;

        loop {
            let setup_started = Instant::now();
            let effects = within_effects(fe_design)?;
            let owned_weights = fallible_copy(weights, "fused PPML LSMR weights")?;
            let reused_this_attempt = !retry
                && self.policy == CategoricalLsmrPolicy::Reuse
                && self.preconditioner.is_some();
            let preconditioner = match (self.policy, self.preconditioner.as_ref()) {
                (CategoricalLsmrPolicy::Diagonal, _) => {
                    PreconditionerInput::Config(PreconditionerConfig::Diagonal)
                }
                (CategoricalLsmrPolicy::Reuse, Some(value)) if !retry => {
                    PreconditionerInput::Prebuilt(value.clone())
                }
                (CategoricalLsmrPolicy::Reuse | CategoricalLsmrPolicy::Rebuild, _) => {
                    rebuilt_preconditioner = true;
                    PreconditionerInput::Config(PreconditionerConfig::default())
                }
            };
            let solver =
                Solver::new(effects, Some(owned_weights), preconditioner).map_err(within_error)?;
            setup_seconds += setup_started.elapsed().as_secs_f64();
            used_schwarz |=
                self.policy != CategoricalLsmrPolicy::Diagonal && solver.preconditioner().is_some();

            let mut responses = Vec::new();
            responses
                .try_reserve_exact(self.active_columns.len() + 1)
                .map_err(|_| allocation("unable to reserve fused PPML response views"))?;
            responses.extend(self.active_columns.iter().map(Vec::as_slice));
            responses.push(working_response);
            let options = LsmrOptions {
                tol: effective_tolerance,
                maxiter: max_iterations,
                local_size,
            };
            let solve_started = Instant::now();
            let projected = solver
                .solve_batch(&responses, Some(&options))
                .map_err(within_error)?;
            solve_seconds += solve_started.elapsed().as_secs_f64();
            iterations = iterations
                .checked_add(projected.iterations.iter().copied().max().unwrap_or(0))
                .ok_or_else(|| allocation("fused PPML FWL iteration count overflowed"))?;
            if self.policy == CategoricalLsmrPolicy::Reuse && !reused_this_attempt {
                self.preconditioner = solver.preconditioner().cloned();
            }

            let converged = projected.converged.iter().all(|value| *value);
            let certificate = if converged {
                let finish_started = Instant::now();
                self.solve_finite_system(&projected, weights, tolerance)?;
                let within_response = projected.demeaned(self.active_columns.len());
                for row in 0..fe_design.nobs() {
                    let mut residual = within_response[row];
                    for column in 0..self.active_columns.len() {
                        residual -= projected.demeaned(column)[row] * self.coefficients[column];
                    }
                    self.residual[row] = residual;
                    self.fitted_without_offset[row] = working_response[row] - residual;
                    if !residual.is_finite() || !self.fitted_without_offset[row].is_finite() {
                        return Err(nonconvergence(
                            "fused PPML FWL produced a nonfinite residual or predictor",
                        ));
                    }
                }
                let certificate = certify_fwl(
                    fe_design,
                    &self.active_columns,
                    weights,
                    working_response,
                    &self.residual,
                    tolerance,
                )?;
                solve_seconds += finish_started.elapsed().as_secs_f64();
                Some(certificate)
            } else {
                None
            };

            if let Some(certificate) = certificate.filter(|value| value.passed) {
                return Ok(FusedFwlLsmrReport {
                    iterations,
                    effective_tolerance,
                    setup_seconds,
                    solve_seconds,
                    preconditioner_reuse_attempted,
                    preconditioner_reuse_certified: preconditioner_reuse_attempted && !retry,
                    rebuilt_preconditioner,
                    continued_after_certificate,
                    used_schwarz,
                    certificate,
                });
            }
            if retry {
                return Err(nonconvergence(if converged {
                    "fused PPML FWL did not pass its exact combined normal-equation certificate"
                } else {
                    "fused PPML FWL projection did not converge for every right-hand side"
                }));
            }
            retry = true;
            continued_after_certificate = converged;
            effective_tolerance = (tolerance * 0.01).max(1e-14);
        }
    }

    pub(crate) fn fill_candidate(
        &self,
        workspace: &mut PpmlIterationWorkspace,
        offset: &[f64],
    ) -> Result<()> {
        self.team.install(|| {
            workspace.fill_absolute_candidate(
                &self.fitted_without_offset,
                offset,
                &self.active_slots,
                &self.coefficients,
            )
        })
    }

    pub(crate) fn active_rank(&self) -> usize {
        self.active_slots.len()
    }

    pub(crate) fn active_slots(&self) -> &[usize] {
        &self.active_slots
    }

    fn validate_frame(
        &self,
        fe_design: &FeDesign,
        working_response: &[f64],
        weights: &[f64],
        tolerance: f64,
        max_iterations: usize,
    ) -> Result<()> {
        if fe_design.nobs() == 0
            || !self.topology.matches(fe_design)
            || working_response.len() != fe_design.nobs()
            || weights.len() != fe_design.nobs()
            || self
                .active_columns
                .iter()
                .any(|column| column.len() != fe_design.nobs())
            || !tolerance.is_finite()
            || tolerance <= 0.0
            || max_iterations == 0
        {
            return Err(invalid_argument(
                "fused PPML FWL frame dimensions or controls are invalid",
            ));
        }
        if working_response.iter().any(|value| !value.is_finite())
            || weights
                .iter()
                .any(|value| !value.is_finite() || *value <= 0.0)
        {
            return Err(nonconvergence(
                "fused PPML FWL frame contains a nonfinite response or invalid weight",
            ));
        }
        Ok(())
    }

    fn solve_finite_system(
        &mut self,
        projected: &within::BatchSolveResult,
        weights: &[f64],
        tolerance: f64,
    ) -> Result<()> {
        let columns = self.active_columns.len();
        if columns == 0 {
            return Ok(());
        }
        self.gram.fill(0.0);
        self.rhs.fill(0.0);
        for left in 0..columns {
            let left_values = projected.demeaned(left);
            for right in left..columns {
                let right_values = projected.demeaned(right);
                let mut value = 0.0_f64;
                for row in 0..weights.len() {
                    value = (weights[row] * left_values[row]).mul_add(right_values[row], value);
                }
                self.gram[left * columns + right] = value;
                self.gram[right * columns + left] = value;
            }
            let response = projected.demeaned(columns);
            let mut value = 0.0_f64;
            for row in 0..weights.len() {
                value = (weights[row] * left_values[row]).mul_add(response[row], value);
            }
            self.rhs[left] = value;
        }
        for column in 0..columns {
            self.scales[column] = self.gram[column * columns + column].sqrt();
            if !self.scales[column].is_finite() || self.scales[column] <= 0.0 {
                return Err(nonconvergence(
                    "fused PPML FWL active finite basis lost positive weighted norm",
                ));
            }
        }
        for row in 0..columns {
            self.rhs[row] /= self.scales[row];
            for column in 0..columns {
                self.gram[row * columns + column] /= self.scales[row] * self.scales[column];
            }
        }
        pivoted_cholesky_solve(
            &mut self.gram,
            &mut self.rhs,
            &mut self.factor,
            &mut self.forward,
            &mut self.standardized_solution,
            &mut self.permutation,
            tolerance.max(64.0 * f64::EPSILON).powi(2),
        )?;
        self.coefficients.fill(0.0);
        for position in 0..columns {
            let original = self.permutation[position];
            self.coefficients[original] =
                self.standardized_solution[position] / self.scales[original];
        }
        if self.coefficients.iter().any(|value| !value.is_finite()) {
            return Err(nonconvergence(
                "fused PPML FWL finite coefficients are nonfinite",
            ));
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn pivoted_cholesky_solve(
    gram: &mut [f64],
    rhs: &mut [f64],
    factor: &mut [f64],
    forward: &mut [f64],
    solution: &mut [f64],
    permutation: &mut [usize],
    pivot_guard: f64,
) -> Result<()> {
    let dimension = rhs.len();
    if gram.len() != dimension.saturating_mul(dimension)
        || factor.len() != gram.len()
        || forward.len() != dimension
        || solution.len() != dimension
        || permutation.len() != dimension
    {
        return Err(invalid_argument(
            "fused PPML FWL finite workspace dimensions are inconsistent",
        ));
    }
    factor.fill(0.0);
    for (position, value) in permutation.iter_mut().enumerate() {
        *value = position;
    }
    for step in 0..dimension {
        let mut pivot = step;
        for candidate in (step + 1)..dimension {
            if gram[candidate * dimension + candidate] > gram[pivot * dimension + pivot] {
                pivot = candidate;
            }
        }
        if pivot != step {
            for column in 0..dimension {
                gram.swap(step * dimension + column, pivot * dimension + column);
            }
            for row in 0..dimension {
                gram.swap(row * dimension + step, row * dimension + pivot);
            }
            rhs.swap(step, pivot);
            permutation.swap(step, pivot);
            for column in 0..step {
                factor.swap(step * dimension + column, pivot * dimension + column);
            }
        }
        let pivot_value = gram[step * dimension + step];
        if !pivot_value.is_finite() || pivot_value <= pivot_guard {
            return Err(nonconvergence(
                "fused PPML FWL active finite basis lost rank",
            ));
        }
        let diagonal = pivot_value.sqrt();
        factor[step * dimension + step] = diagonal;
        for row in (step + 1)..dimension {
            factor[row * dimension + step] = gram[row * dimension + step] / diagonal;
        }
        for row in (step + 1)..dimension {
            for column in row..dimension {
                let value = gram[column * dimension + row]
                    - factor[row * dimension + step] * factor[column * dimension + step];
                if !value.is_finite() {
                    return Err(nonconvergence(
                        "fused PPML FWL Cholesky update is nonfinite",
                    ));
                }
                gram[column * dimension + row] = value;
                gram[row * dimension + column] = value;
            }
        }
    }
    for row in 0..dimension {
        let mut value = rhs[row];
        for column in 0..row {
            value -= factor[row * dimension + column] * forward[column];
        }
        forward[row] = value / factor[row * dimension + row];
    }
    for row in (0..dimension).rev() {
        let mut value = forward[row];
        for column in (row + 1)..dimension {
            value -= factor[column * dimension + row] * solution[column];
        }
        solution[row] = value / factor[row * dimension + row];
    }
    if forward
        .iter()
        .chain(solution.iter())
        .any(|value| !value.is_finite())
    {
        return Err(nonconvergence("fused PPML FWL Cholesky solve is nonfinite"));
    }
    Ok(())
}

fn certify_fwl(
    design: &FeDesign,
    active: &[Vec<f64>],
    weights: &[f64],
    target: &[f64],
    residual: &[f64],
    tolerance: f64,
) -> Result<JointLsmrCertificate> {
    let mut target_square = ScaledSquareSum::default();
    let mut residual_square = ScaledSquareSum::default();
    for row in 0..design.nobs() {
        let root_weight = weights[row].sqrt();
        target_square.add(root_weight * target[row])?;
        residual_square.add(root_weight * residual[row])?;
    }
    let reference = target_square.norm()?.max(residual_square.norm()?);
    let mut finite_absolute = 0.0_f64;
    let mut fe_absolute = 0.0_f64;
    let mut relative_infinity = 0.0_f64;
    let mut relative_square = FiniteCompensatedSum::default();
    let mut count = 0_usize;
    for column in active {
        let mut moment = FiniteCompensatedSum::default();
        let mut basis_square = ScaledSquareSum::default();
        for row in 0..design.nobs() {
            let root_weight = weights[row].sqrt();
            moment.add((root_weight * residual[row]) * (root_weight * column[row]))?;
            basis_square.add(root_weight * column[row])?;
        }
        let moment = moment.total()?;
        finite_absolute = finite_absolute.max(moment.abs());
        add_relative(
            moment,
            basis_square.norm()?,
            reference,
            &mut relative_infinity,
            &mut relative_square,
            &mut count,
        )?;
    }
    for effect in design.effects() {
        for level in 0..effect.n_levels() {
            let mut moment = FiniteCompensatedSum::default();
            let mut basis_square = ScaledSquareSum::default();
            for &row in effect.level_index().members(level) {
                let root_weight = weights[row].sqrt();
                moment.add((root_weight * residual[row]) * root_weight)?;
                basis_square.add(root_weight)?;
            }
            let moment = moment.total()?;
            fe_absolute = fe_absolute.max(moment.abs());
            add_relative(
                moment,
                basis_square.norm()?,
                reference,
                &mut relative_infinity,
                &mut relative_square,
                &mut count,
            )?;
        }
    }
    let relative_two = if count == 0 {
        0.0
    } else {
        (relative_square.total()?.max(0.0) / count as f64).sqrt()
    };
    if !finite_absolute.is_finite()
        || !fe_absolute.is_finite()
        || !relative_infinity.is_finite()
        || !relative_two.is_finite()
    {
        return Err(nonconvergence(
            "fused PPML FWL exact certificate is nonfinite",
        ));
    }
    Ok(JointLsmrCertificate {
        finite_absolute_infinity_norm: finite_absolute,
        fixed_effect_absolute_infinity_norm: fe_absolute,
        relative_infinity_norm: relative_infinity,
        relative_two_norm: relative_two,
        passed: relative_infinity <= tolerance && relative_two <= tolerance,
    })
}

fn within_effects(design: &FeDesign) -> Result<Vec<Effect<'_>>> {
    let mut effects = Vec::new();
    effects
        .try_reserve_exact(design.effects().len())
        .map_err(|_| allocation("unable to reserve fused PPML effects"))?;
    for effect in design.effects() {
        effects.push(
            Effect::new(effect.codes(), true, std::iter::empty::<&[f64]>())
                .map_err(within_error)?,
        );
    }
    Ok(effects)
}

/// Conservative heap peak added by the fused PPML FWL-LSMR engine.
///
/// `within::solve_batch` owns an independent LSMR recurrence and result for
/// each concurrently executing RHS. It then allocates the consolidated batch
/// output while the per-RHS outputs are still live. The generic FE estimate
/// models neither behavior and is therefore not safe for this route.
pub(crate) fn estimate_fused_fwl_lsmr_workspace_bytes(
    rows: usize,
    active_columns: usize,
    effect_level_counts: &[usize],
    threads: usize,
    local_reorthogonalization_size: Option<usize>,
) -> Result<u128> {
    if local_reorthogonalization_size == Some(0) {
        return Err(invalid_argument(
            "fused PPML LSMR memory estimate has an invalid reorthogonalization count",
        ));
    }
    let shape =
        CategoricalLsmrMemoryShape::new(rows, active_columns, effect_level_counts, threads)?;
    let n = shape.rows;
    let k = shape.active_columns;
    let rhs = checked_add_memory(k, 1, "fused PPML LSMR RHS count")?;
    let square = checked_mul_memory(k, k, "fused PPML finite square")?;

    // Fit-lifetime buffers in ReusableCategoricalFwlLsmr, including the
    // retained factor that can be reused by the next changing-weight frame.
    let retained_values = checked_sum_memory(&[
        checked_mul_memory(n, k, "fused PPML retained active controls")?,
        checked_mul_memory(n, 2, "fused PPML retained observation buffers")?,
        checked_mul_memory(square, 2, "fused PPML retained finite squares")?,
        checked_mul_memory(k, 5, "fused PPML retained finite vectors")?,
    ])?;
    let retained_indices = checked_sum_memory(&[
        checked_mul_memory(k, 2, "fused PPML retained finite indices")?,
        checked_mul_memory(shape.effects, 4, "fused PPML topology identity words")?,
    ])?;
    let retained_engine = checked_sum_memory(&[
        bytes_for_memory::<f64>(retained_values, "fused PPML retained values")?,
        bytes_for_memory::<usize>(retained_indices, "fused PPML retained indices")?,
        shape.schwarz.retained_bytes,
    ])?;

    let local = local_reorthogonalization_size.unwrap_or(0) as u128;
    // Per active RHS: caller-order and locality-sorted/weighted response
    // copies, Modified Golub-Kahan's buffers/history, the three-vector LSMR
    // solution recurrence, and DesignOperator's maximum-level scatter scratch.
    let recurrence_vectors = checked_add_memory(
        6,
        checked_mul_memory(local, 2, "fused PPML local history lanes")?,
        "fused PPML recurrence vector count",
    )?;
    let active_rhs_values = checked_sum_memory(&[
        checked_mul_memory(n, 2, "fused PPML active RHS response copies")?,
        checked_mul_memory(
            shape.levels,
            recurrence_vectors,
            "fused PPML active RHS recurrence",
        )?,
        shape.maximum_levels,
    ])?;
    let active_rhs_bytes =
        bytes_for_memory::<f64>(active_rhs_values, "fused PPML active RHS recurrence bytes")?;
    let result_values = checked_add_memory(n, shape.levels, "fused PPML per-RHS result")?;
    let result_bytes = bytes_for_memory::<f64>(result_values, "fused PPML per-RHS result bytes")?;
    let concurrent_rhs = rhs.min(shape.threads);

    // Completed RHS results accumulate while the remaining Rayon tasks still
    // own recurrences. Charging every result plus every concurrently live
    // recurrence is a simple upper bound independent of scheduling order.
    let parallel_solve_peak = checked_sum_memory(&[
        checked_mul_memory(rhs, result_bytes, "fused PPML retained RHS results")?,
        checked_mul_memory(
            concurrent_rhs,
            active_rhs_bytes,
            "fused PPML concurrent RHS recurrences",
        )?,
        shape.schwarz.apply_peak_bytes,
    ])?;

    // Batch consolidation reserves final x and demeaned buffers before it
    // consumes the individual results, temporarily doubling result storage.
    let consolidated_results = checked_mul_memory(
        checked_mul_memory(rhs, result_bytes, "fused PPML batch result bytes")?,
        2,
        "fused PPML doubled batch result storage",
    )?;
    let batch_scalars = checked_sum_memory(&[
        bytes_for_memory::<bool>(rhs, "fused PPML convergence flags")?,
        bytes_for_memory::<usize>(rhs, "fused PPML iteration counts")?,
        bytes_for_memory::<f64>(
            checked_mul_memory(rhs, 2, "fused PPML result scalar arrays")?,
            "fused PPML result scalar bytes",
        )?,
        checked_mul_memory(
            rhs,
            2 * std::mem::size_of::<usize>() as u128,
            "fused PPML response views",
        )?,
    ])?;
    let consolidation_peak = checked_add_memory(
        consolidated_results,
        batch_scalars,
        "fused PPML batch consolidation peak",
    )?;
    let solve_peak = checked_add_memory(
        shape.schwarz.solver_frame_bytes,
        parallel_solve_peak.max(consolidation_peak),
        "fused PPML solve peak",
    )?;

    checked_add_memory(
        retained_engine,
        shape.schwarz.build_peak_bytes.max(solve_peak),
        "fused PPML FWL LSMR workspace peak",
    )
}

fn fallible_filled<T: Clone>(elements: usize, value: T, message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(elements)
        .map_err(|_| allocation(message))?;
    output.resize(elements, value);
    Ok(output)
}

fn fallible_copy<T: Clone>(values: &[T], message: &'static str) -> Result<Vec<T>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(values.len())
        .map_err(|_| allocation(message))?;
    output.extend_from_slice(values);
    Ok(output)
}

fn within_error(error: impl std::fmt::Display) -> FeregError {
    FeregError::new(
        ErrorCode::NonConvergence,
        format!("fused PPML FWL LSMR failed: {error}"),
    )
}

fn invalid_argument(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}

fn allocation(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::Allocation, message)
}

fn nonconvergence(message: &'static str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fe::EffectTerm;

    #[test]
    fn pivoted_cholesky_recovers_a_permuted_spd_system() {
        let mut gram = vec![1.0, 0.5, 0.5, 4.0];
        let mut rhs = vec![2.0, 9.0];
        let mut factor = vec![0.0; 4];
        let mut forward = vec![0.0; 2];
        let mut solution = vec![0.0; 2];
        let mut permutation = vec![0; 2];
        pivoted_cholesky_solve(
            &mut gram,
            &mut rhs,
            &mut factor,
            &mut forward,
            &mut solution,
            &mut permutation,
            1e-14,
        )
        .expect("solve");
        let mut canonical = [0.0; 2];
        for position in 0..2 {
            canonical[permutation[position]] = solution[position];
        }
        assert!((canonical[0] - 14.0 / 15.0).abs() <= 1e-12);
        assert!((canonical[1] - 32.0 / 15.0).abs() <= 1e-12);
    }

    #[test]
    fn exact_certificate_does_not_turn_overflowing_squares_into_a_false_pass() {
        let design = FeDesign::new(
            2,
            vec![EffectTerm::new(vec![0, 0], 1, true, Vec::new()).expect("effect")],
        )
        .expect("design");
        let values = [1e200, 1e200];
        let certificate = certify_fwl(&design, &[], &[1.0; 2], &values, &values, 1e-8)
            .expect("finite scaled certificate");
        assert!(!certificate.passed);
        assert!((certificate.relative_infinity_norm - 1.0).abs() <= 1e-15);
    }

    #[test]
    fn fused_solver_runs_upstream_work_under_the_retained_team() {
        let design = FeDesign::new(
            4,
            vec![EffectTerm::new(vec![0, 0, 1, 1], 2, true, Vec::new()).expect("effect")],
        )
        .expect("design");
        let mut solver = ReusableCategoricalFwlLsmr {
            team: ThreadTeam::new(ThreadConfig::from_requested(3, true).expect("thread config"))
                .acquire()
                .expect("installed team"),
            active_columns: Vec::new(),
            active_slots: Vec::new(),
            topology: FeTopologyIdentity::capture(&design).expect("topology"),
            preconditioner: None,
            gram: Vec::new(),
            rhs: Vec::new(),
            factor: Vec::new(),
            forward: Vec::new(),
            standardized_solution: Vec::new(),
            scales: Vec::new(),
            coefficients: Vec::new(),
            permutation: Vec::new(),
            residual: vec![0.0; 4],
            fitted_without_offset: vec![0.0; 4],
            policy: CategoricalLsmrPolicy::Diagonal,
        };
        let unrelated_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .expect("unrelated pool");
        let report = unrelated_pool
            .install(|| solver.solve(&design, &[1.0, 1.0, 2.0, 2.0], &[1.0; 4], 1e-10, 100, None))
            .expect("fused solve");
        assert!(report.certificate.passed);
        assert_eq!(
            TEST_FUSED_LSMR_EXECUTING_WIDTH.load(std::sync::atomic::Ordering::Relaxed),
            3,
        );
    }

    #[test]
    fn fused_memory_estimate_scales_with_rhs_concurrency_and_history() {
        let serial = estimate_fused_fwl_lsmr_workspace_bytes(10_000, 3, &[2_000, 1_000], 1, None)
            .expect("serial estimate");
        let parallel = estimate_fused_fwl_lsmr_workspace_bytes(10_000, 3, &[2_000, 1_000], 4, None)
            .expect("parallel estimate");
        let with_history =
            estimate_fused_fwl_lsmr_workspace_bytes(10_000, 3, &[2_000, 1_000], 4, Some(8))
                .expect("history estimate");
        assert!(parallel > serial);
        assert!(with_history > parallel);
    }

    #[test]
    fn fused_memory_estimate_rejects_invalid_and_overflowing_shapes() {
        let invalid = estimate_fused_fwl_lsmr_workspace_bytes(100, 2, &[20, 10], 4, Some(0))
            .expect_err("zero local history must fail");
        assert_eq!(invalid.code, ErrorCode::InvalidArgument);

        let overflow = estimate_fused_fwl_lsmr_workspace_bytes(
            usize::MAX,
            usize::MAX,
            &[usize::MAX, usize::MAX],
            usize::MAX,
            Some(usize::MAX),
        )
        .expect_err("adversarial dimensions must fail closed");
        assert_eq!(overflow.code, ErrorCode::Allocation);
    }
}
