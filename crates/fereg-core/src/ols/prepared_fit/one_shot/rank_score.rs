//! Quotient-coordinate checks for the exceptional rank repair.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn terminal_projection<'a, P: PreparedOlsFitView + ?Sized>(
    prepared: &'a P,
    frame: LinearSolveFrame<'a>,
    config: PreparedOlsFitConfig,
    requested: PreparedOlsFitConfig,
    inputs: &OneShotInputs,
    qr: &FiniteFit,
    mut cmg: crate::fe::FeSession<'a>,
    rank_parent: u128,
) -> Result<(crate::fe::FeSession<'a>, PreparedOlsFitConfig, Option<f64>)> {
    let weights = frame.weights();
    let dense = config.execution.mode == PreparedOlsExecutionMode::Fast;
    let mut final_config = config;
    if rank_parent != 0 {
        // The witness survives the terminal projection. Rebuild its session
        // after releasing the old one, with that retained row bank admitted.
        drop(cmg);
        if let Some(budget) = &mut final_config.absorb.solver_memory_budget {
            budget.execution_bytes = budget
                .execution_bytes
                .checked_add(rank_parent)
                .ok_or_else(|| allocation("rank repair final parent budget overflows"))?;
            if !budget.fits(0) {
                return Err(allocation("rank repair final parent exceeds memory limit"));
            }
        }
        cmg = crate::fe::FeSession::for_ols_rhs(prepared, weights.borrowed(), final_config, 1)?;
    }
    let derived_scale = if rank_parent == 0 {
        derived_scale(dense, requested, frame.response(), weights, inputs, qr)?
    } else {
        None
    };
    Ok((cmg, final_config, derived_scale))
}

/// A repaired Low fit must retain its quotient signal during final projection.
/// Reconstructing y-Xb in the original units can round that signal away.
pub(super) fn scratch_residual<R: FitReservation>(
    low: &ScratchProjected,
    qr: &FiniteFit,
    reservation: &mut R,
) -> Result<Vec<f64>> {
    let mut residual = reserved_empty(
        reservation,
        low.outcome.len(),
        "rank repair within residual",
    )?;
    let mut row = reserved_filled(
        reservation,
        low.scratch.columns(),
        0.,
        "rank repair residual row",
    )?;
    let mut reader = low.scratch.shared_reader();
    for (observation, &outcome) in low.outcome.iter().enumerate() {
        if observation % 256 == 0 {
            crate::interrupt::checkpoint()?;
        }
        reader.read_linear(observation * row.len(), &mut row)?;
        let mut value = outcome;
        for &column in &qr.kept {
            value = (-qr.generated_coefficients[column].expect("kept coefficient"))
                .mul_add(row[column], value);
        }
        if !value.is_finite() {
            return Err(invalid_data("rank repair within residual is nonfinite"));
        }
        residual.push(value);
    }
    Ok(residual)
}

pub(super) struct Witness {
    target: Vec<f64>,
    scale: f64,
    target_error: f64,
    response_lower: f64,
    certificate: PreparedOlsScoreCertificate,
    exact_zero: bool,
}

pub(super) fn admit(
    inputs: &mut OneShotInputs,
    config: PreparedOlsFitConfig,
    rows: usize,
) -> Result<()> {
    let extra = (rows as u128)
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(inputs.reference_norms.len() as u128 * 48 + 8))
        .ok_or_else(|| allocation("rank repair score workspace overflows"))?;
    let prior = match inputs.absorption_report.solver_workspace_peak.bytes() {
        Some(bytes) => u128::from(bytes),
        None if config.absorb.solver_memory_budget.is_some() => {
            return Err(allocation(
                "rank repair score cannot admit an unmodeled FE workspace",
            ))
        }
        None => 0,
    };
    let peak = extra
        .checked_add(prior)
        .ok_or_else(|| allocation("rank repair score workspace overflows"))?;
    if config
        .absorb
        .solver_memory_budget
        .is_some_and(|budget| !budget.fits(peak))
    {
        return Err(allocation(
            "rank repair score exceeds the requested memory limit",
        ));
    }
    inputs.absorption_report.solver_workspace_peak = inputs
        .absorption_report
        .solver_workspace_peak
        .max(crate::memory::SolverWorkspacePeak::from_bytes(peak)?);
    Ok(())
}

/// Certify the original quotient problem, not raw moments amplified by an
/// arbitrarily large FE offset. All coordinates have original-source forward
/// bounds; the independent finite factor alone is not this certificate.
pub(super) fn prepare<R: FitReservation>(
    inputs: &OneShotInputs,
    qr: &FiniteFit,
    weights: NormalizedWeights<'_>,
    tolerance: f64,
    reservation: &mut R,
) -> Result<Witness> {
    let resolved = inputs
        .rank_resolution
        .as_ref()
        .expect("resolved projection");
    let columns = resolved.scales.len();
    let rows = match &inputs.storage {
        ProjectedStorage::Dense(dense) => dense.within.as_ref().expect("projected rows").nrows(),
        ProjectedStorage::Scratch(low) => low.outcome.len(),
        ProjectedStorage::Centered(_) => return Err(unresolved()),
    };
    if inputs.within_outcome_norm == 0. {
        let zero_rows = match &inputs.storage {
            ProjectedStorage::Dense(dense) => {
                (0..rows).all(|row| dense.within.as_ref().expect("projected rows")[(row, 0)] == 0.)
            }
            ProjectedStorage::Scratch(low) => low.outcome.iter().all(|value| *value == 0.),
            ProjectedStorage::Centered(_) => false,
        };
        if !zero_rows
            || resolved.outcome_error != 0.
            || qr
                .generated_coefficients
                .iter()
                .flatten()
                .any(|coefficient| *coefficient != 0.)
        {
            return Err(unresolved());
        }
        return Ok(Witness {
            target: reserved_filled(reservation, rows, 0., "rank repair zero residual")?,
            scale: 1.,
            target_error: 0.,
            response_lower: 1.,
            exact_zero: true,
            certificate: PreparedOlsScoreCertificate {
                relative_infinity_norm: 0.,
                relative_two_norm: 0.,
                moments: qr
                    .generated_statuses
                    .iter()
                    .zip(inputs.score_norms())
                    .filter(|(status, norm)| finite_column(**status, **norm))
                    .count(),
                passed: true,
            },
        });
    }
    let ProjectionMoments {
        target,
        divisor,
        moments,
        norms,
        outcome_norm,
        residual_norm,
        formation_norm,
    } = projection_moments(inputs, qr, weights, reservation, rows)?;
    let mut error = upper(resolved.outcome_error / divisor);
    for (&bound, coefficient) in resolved
        .absolute_errors
        .iter()
        .zip(&qr.generated_coefficients)
    {
        if let Some(coefficient) = coefficient {
            error = upper(error + upper(upper(coefficient.abs() * bound) / divisor));
        }
    }
    error = upper(error + formation_norm.norm_upper()?);
    let residual_upper = residual_norm.norm_upper()?;
    let response_lower =
        lower(outcome_norm.norm_lower()? - upper(resolved.outcome_error / divisor))
            .max(lower(residual_norm.norm_lower()? - error));
    if !error.is_finite() || response_lower <= 0. {
        return Err(unresolved());
    }
    let mut maximum = 0.0_f64;
    let mut square_sum = 0.;
    let mut count = 0;
    for column in 0..columns {
        if !finite_column(qr.generated_statuses[column], inputs.score_norms()[column]) {
            continue;
        }
        let column_error = upper(resolved.absolute_errors[column] / resolved.scales[column]);
        let norm_lower = lower(norms[column].norm_lower()? - column_error);
        let norm_upper = norms[column].norm_upper()?;
        if norm_lower <= 0. {
            return Err(unresolved());
        }
        let numerator = upper(moments[column].abs_upper()? + upper(column_error * residual_upper));
        let numerator = upper(numerator + upper(upper(norm_upper + column_error) * error));
        let denominator = lower(norm_lower * response_lower);
        if !denominator.is_finite() || denominator <= 0. {
            return Err(unresolved());
        }
        let relative = upper(numerator / denominator);
        if !relative.is_finite() {
            return Err(unresolved());
        }
        maximum = maximum.max(relative);
        square_sum = upper(square_sum + upper(relative * relative));
        count += 1;
    }
    let certificate = PreparedOlsScoreCertificate {
        relative_infinity_norm: maximum,
        relative_two_norm: if count == 0 {
            0.
        } else {
            upper(upper(square_sum / count as f64).sqrt())
        },
        moments: count,
        passed: maximum <= tolerance,
    };
    if !certificate.passed {
        return Err(unresolved());
    }
    Ok(Witness {
        target,
        scale: divisor,
        target_error: error,
        response_lower,
        certificate,
        exact_zero: false,
    })
}

struct ProjectionMoments {
    target: Vec<f64>,
    divisor: f64,
    moments: Vec<Interval>,
    norms: Vec<Interval>,
    outcome_norm: Interval,
    residual_norm: Interval,
    formation_norm: Interval,
}

fn projection_moments<R: FitReservation>(
    inputs: &OneShotInputs,
    qr: &FiniteFit,
    weights: NormalizedWeights<'_>,
    reservation: &mut R,
    rows: usize,
) -> Result<ProjectionMoments> {
    let resolved = inputs
        .rank_resolution
        .as_ref()
        .expect("resolved projection");
    let columns = resolved.scales.len();
    let mut target = reserved_empty(reservation, rows, "rank repair certified residual")?;
    let mut row = reserved_filled(reservation, columns + 1, 0., "rank repair score row")?;
    let mut moments = reserved_filled(
        reservation,
        columns,
        Interval::default(),
        "rank repair score moments",
    )?;
    let mut norms = reserved_filled(
        reservation,
        columns,
        Interval::default(),
        "rank repair score norms",
    )?;
    let mut reader = match &inputs.storage {
        ProjectedStorage::Scratch(low) => Some(low.scratch.shared_reader()),
        _ => None,
    };
    // Scale the entire certificate to avoid squaring original-unit magnitudes.
    let scale = inputs.within_outcome_norm;
    if !scale.is_finite() || scale < 0. {
        return Err(unresolved());
    }
    let divisor = if scale == 0. { 1. } else { scale };
    let mut outcome_norm = Interval::default();
    let mut residual_norm = Interval::default();
    let mut formation_norm = Interval::default();
    for observation in 0..rows {
        if observation % 256 == 0 {
            crate::interrupt::checkpoint()?;
        }
        match &inputs.storage {
            ProjectedStorage::Dense(dense) => {
                let dense = dense.within.as_ref().expect("projected rows");
                row.copy_from_slice(
                    &dense.as_slice()
                        [observation * (columns + 1)..(observation + 1) * (columns + 1)],
                );
            }
            ProjectedStorage::Scratch(low) => {
                row[0] = low.outcome[observation];
                reader
                    .as_mut()
                    .expect("scratch reader")
                    .read_linear(observation * columns, &mut row[1..])?;
            }
            ProjectedStorage::Centered(_) => return Err(unresolved()),
        }
        let mut value = row[0];
        let mut rounding = 0.;
        for &column in &qr.kept {
            value = (-qr.generated_coefficients[column].expect("kept coefficient"))
                .mul_add(row[column + 1], value);
            if !value.is_finite() {
                return Err(unresolved());
            }
            // Each correctly rounded FMA adds at most one ULP of its result.
            // The prior residual enters additively, so these errors sum.
            let ulp = upper(value.abs()) - value.abs();
            rounding = upper(rounding + ulp);
        }
        target.push(value);
        let weight = Interval::point(weights.value(observation)?);
        let y = Interval::point(row[0]).divide(divisor);
        let q = Interval::point(value).divide(divisor);
        let e = Interval::point(rounding).divide(divisor);
        outcome_norm = outcome_norm.add(weight.multiply(y).multiply(y));
        residual_norm = residual_norm.add(weight.multiply(q).multiply(q));
        formation_norm = formation_norm.add(weight.multiply(e).multiply(e));
        for column in 0..columns {
            if !finite_column(qr.generated_statuses[column], inputs.score_norms()[column]) {
                continue;
            }
            let x = Interval::point(row[column + 1]).divide(resolved.scales[column]);
            moments[column] = moments[column].add(weight.multiply(x).multiply(q));
            norms[column] = norms[column].add(weight.multiply(x).multiply(x));
        }
    }
    Ok(ProjectionMoments {
        target,
        divisor,
        moments,
        norms,
        outcome_norm,
        residual_norm,
        formation_norm,
    })
}

impl Witness {
    pub(super) fn retained_bytes(&self) -> u128 {
        self.target.capacity() as u128 * 8
    }
    /// The existing terminal FE projection remains in place. Its movement
    /// from the certified quotient residual is checked explicitly, so a small
    /// FE moment alone cannot hide damage to published residuals/inference.
    pub(super) fn finish<P: PreparedOlsFitView + ?Sized>(
        self,
        prepared: &P,
        residuals: &[f64],
        weights: NormalizedWeights<'_>,
        config: PreparedOlsFitConfig,
    ) -> Result<PreparedOlsScoreCertificate> {
        if residuals.len() != self.target.len() {
            return Err(unresolved());
        }
        let tolerance = config.score_tolerance;
        if self.exact_zero {
            if residuals.iter().any(|value| *value != 0.) {
                return Err(unresolved());
            }
            return Ok(self.certificate);
        }
        let mut difference = Interval::default();
        for (row, (&actual, &target)) in residuals.iter().zip(&self.target).enumerate() {
            if row % 256 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let delta = Interval::point(actual)
                .add(Interval::point(-target))
                .divide(self.scale);
            let magnitude = delta.abs_upper()?;
            let value = Interval::point(magnitude);
            difference = difference.add(
                Interval::point(weights.value(row)?)
                    .multiply(value)
                    .multiply(value),
            );
        }
        if upper(self.target_error + difference.norm_upper()?)
            > lower(tolerance * self.response_lower)
        {
            return Err(unresolved());
        }
        self.certify_fe(
            prepared,
            residuals,
            weights,
            config.final_absorb_config().map.tolerance,
        )?;
        Ok(self.certificate)
    }

    fn certify_fe<P: PreparedOlsFitView + ?Sized>(
        &self,
        prepared: &P,
        residuals: &[f64],
        weights: NormalizedWeights<'_>,
        tolerance: f64,
    ) -> Result<()> {
        if residuals.iter().all(|value| *value == 0.) {
            return Ok(());
        }
        let design = prepared
            .fit_borrowed_intercept_map_design()?
            .ok_or_else(unresolved)?;
        let mut reference_norm = Interval::default();
        let mut residual_norm = Interval::default();
        for (row, (&target, &residual)) in self.target.iter().zip(residuals).enumerate() {
            if row % 256 == 0 {
                crate::interrupt::checkpoint()?;
            }
            let weight = Interval::point(weights.value(row)?);
            let target = Interval::point(target).divide(self.scale);
            let residual = Interval::point(residual).divide(self.scale);
            reference_norm = reference_norm.add(weight.multiply(target).multiply(target));
            residual_norm = residual_norm.add(weight.multiply(residual).multiply(residual));
        }
        let scale_lower = reference_norm
            .norm_lower()?
            .max(residual_norm.norm_lower()?);
        if scale_lower <= 0. {
            return Err(unresolved());
        }
        let check = |rows: &mut dyn Iterator<Item = usize>| -> Result<()> {
            let mut moment = Interval::default();
            let mut mass = Interval::default();
            for (index, row) in rows.enumerate() {
                if index % 256 == 0 {
                    crate::interrupt::checkpoint()?;
                }
                let weight = Interval::point(weights.value(row)?);
                mass = mass.add(weight);
                moment =
                    moment.add(weight.multiply(Interval::point(residuals[row]).divide(self.scale)));
            }
            let denominator = lower(mass.norm_lower()? * scale_lower);
            if denominator <= 0.
                || !denominator.is_finite()
                || upper(moment.abs_upper()? / denominator) > tolerance
            {
                return Err(unresolved());
            }
            Ok(())
        };
        if design.effects().is_empty() {
            check(&mut (0..residuals.len()))?;
        } else {
            for effect in design.effects() {
                for level in 0..effect.n_levels() {
                    let members = effect.level_index().members(level);
                    if !members.is_empty() {
                        check(&mut members.iter().copied())?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn finite_column(status: PreparedOlsFitStatus, norm: f64) -> bool {
    norm > 0.
        && matches!(
            status,
            PreparedOlsFitStatus::Kept
                | PreparedOlsFitStatus::NumericallyRankDeficient
                | PreparedOlsFitStatus::SyntaxOmitted
        )
}

fn unresolved() -> FeregError {
    invalid_data(
        "OLS rank repair could not certify original-model coefficient or residual accuracy",
    )
}

#[derive(Clone, Copy, Default)]
struct Interval {
    lo: f64,
    hi: f64,
}
impl Interval {
    fn point(value: f64) -> Self {
        Self {
            lo: value,
            hi: value,
        }
    }
    fn add(self, other: Self) -> Self {
        Self {
            lo: lower(self.lo + other.lo),
            hi: upper(self.hi + other.hi),
        }
    }
    fn multiply(self, other: Self) -> Self {
        let values = [
            self.lo * other.lo,
            self.lo * other.hi,
            self.hi * other.lo,
            self.hi * other.hi,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Self::point(f64::NAN);
        }
        Self {
            lo: lower(values.iter().copied().fold(f64::INFINITY, f64::min)),
            hi: upper(values.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        }
    }
    fn divide(self, positive: f64) -> Self {
        if !positive.is_finite() || positive <= 0. {
            return Self::point(f64::NAN);
        }
        Self {
            lo: lower(self.lo / positive),
            hi: upper(self.hi / positive),
        }
    }
    fn abs_upper(self) -> Result<f64> {
        if !self.lo.is_finite() || !self.hi.is_finite() {
            return Err(unresolved());
        }
        Ok(self.lo.abs().max(self.hi.abs()))
    }
    fn norm_upper(self) -> Result<f64> {
        self.abs_upper()?;
        if self.hi < 0. {
            return Err(unresolved());
        }
        Ok(upper(self.hi.sqrt()))
    }
    fn norm_lower(self) -> Result<f64> {
        self.abs_upper()?;
        Ok(lower(self.lo.max(0.).sqrt()).max(0.))
    }
}
fn upper(value: f64) -> f64 {
    if !value.is_finite() {
        value
    } else if value == 0. {
        f64::from_bits(1)
    } else if value > 0. {
        f64::from_bits(value.to_bits() + 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}
fn lower(value: f64) -> f64 {
    -upper(-value)
}
