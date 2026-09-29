//! One coefficient reconstruction and weighted local-span acceptance rule.
use super::{
    certificate_basis::level_basis, design::validate_matrix, Certificate, Design, Projection,
    RhsDiagnostics,
};
use crate::{
    data::DenseMatrix,
    error::{reserved, ErrorCode, FeregError, Result},
    threads::{compensated_sum, ThreadTeam, REDUCTION_CHUNK},
};
use rayon::prelude::*;
impl Design<'_> {
    pub fn certify(
        &self,
        rhs: &DenseMatrix,
        coefficients: &DenseMatrix,
        residuals: &DenseMatrix,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<Certificate> {
        let certificate =
            self.measure_certificate(rhs, coefficients, residuals, tolerance, team)?;
        for (kind, errors) in [
            ("local-span", &certificate.relative_score),
            ("reconstruction", &certificate.relative_reconstruction),
        ] {
            if let Some((column, value)) = errors
                .iter()
                .enumerate()
                .find(|(_, value)| !value.is_finite() || **value > tolerance)
            {
                return Err(numerical(&format!(
                    "FE {kind} certificate failed for RHS {column}: {value:.17e} > {tolerance:.17e}"
                )));
            }
        }
        Ok(certificate)
    }
    /// Unaccepted score evidence for an in-progress backend's residual refinement.
    pub(super) fn measure_certificate(
        &self,
        rhs: &DenseMatrix,
        coefficients: &DenseMatrix,
        residuals: &DenseMatrix,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<Certificate> {
        check_tolerance(tolerance)?;
        validate_matrix(residuals, self.nobs())?;
        let reconstructed = self.reconstruct(rhs, coefficients, team)?;
        if residuals.ncols() != rhs.ncols() {
            return Err(invalid("certificate RHS counts differ"));
        }
        let reconstruction = reconstruction_errors(rhs, &reconstructed, residuals)?;
        let reference_norm = weighted_norms(self, rhs, team)?;
        let residual_norm = weighted_norms(self, residuals, team)?;
        let mut scales = reserved(rhs.ncols())?;
        scales.extend(
            reference_norm
                .iter()
                .zip(&residual_norm)
                .map(|(&a, &b)| a.max(b)),
        );
        let mut score = zeros(rhs.ncols())?;
        let mut aggregate = zeros(rhs.ncols())?;
        let mut raw = zeros(rhs.ncols())?;
        let mut groups = 0;
        for effect in 0..self.layout().effects.len() {
            let levels = self.layout().effects[effect].levels;
            let mut results = reserved(levels)?;
            results.resize_with(levels, || None);
            team.install(|| {
                results
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(level, result)| {
                        *result = Some(check_level(self, effect, level, residuals, &scales));
                    })
            });
            for result in results {
                let (relative, moments) = result.expect("visited certificate level")?;
                for column in 0..rhs.ncols() {
                    score[column] = score[column].max(relative[column]);
                    aggregate[column] = aggregate[column].hypot(relative[column]);
                    raw[column] = raw[column].max(moments[column]);
                }
                groups += 1;
            }
        }
        if groups > 0 {
            for value in &mut aggregate {
                *value /= (groups as f64).sqrt();
            }
        }
        Ok(Certificate {
            relative_score: score,
            relative_score_rms: aggregate,
            raw_score_max: raw,
            weighted_rhs_norm: reference_norm,
            weighted_residual_norm: residual_norm,
            relative_reconstruction: reconstruction,
            tolerance,
        })
    }
    /// This is the only conversion from a numerical candidate into a projection.
    pub fn finish_candidate(
        &self,
        rhs: &DenseMatrix,
        mut coefficients: DenseMatrix,
        diagnostics: Vec<RhsDiagnostics>,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<Projection> {
        check_tolerance(tolerance)?;
        if diagnostics.len() != rhs.ncols() {
            return Err(invalid("FE diagnostics and RHS counts differ"));
        }
        self.normalize_coefficients(&mut coefficients, tolerance, team)?;
        self.certified_projection(rhs, coefficients, diagnostics, tolerance, team)
    }
    pub(super) fn certified_projection(
        &self,
        rhs: &DenseMatrix,
        coefficients: DenseMatrix,
        diagnostics: Vec<RhsDiagnostics>,
        tolerance: f64,
        team: &ThreadTeam,
    ) -> Result<Projection> {
        if diagnostics.len() != rhs.ncols() {
            return Err(invalid("FE diagnostics and RHS counts differ"));
        }
        let residuals = self.reconstruct(rhs, &coefficients, team)?;
        let certificate = self.certify(rhs, &coefficients, &residuals, tolerance, team)?;
        Ok(Projection {
            coefficients,
            residuals,
            certificate,
            diagnostics,
            setup_diagnostics: reserved(0)?,
        })
    }
}
fn check_level(
    design: &Design<'_>,
    effect: usize,
    level: usize,
    residuals: &DenseMatrix,
    scales: &[f64],
) -> Result<(Vec<f64>, Vec<f64>)> {
    let members = design
        .topology()
        .effect_partition(effect)?
        .membership()
        .members(level)?;
    let basis = level_basis(design, effect, level)?;
    let mut scores = zeros(residuals.ncols())?;
    let mut raw = zeros(residuals.ncols())?;
    for column in 0..residuals.ncols() {
        for direction in &basis {
            let projection = compensated_sum(members.iter().enumerate().map(|(local, &row)| {
                let value = design.weights().at(row).sqrt() * residuals[(row, column)];
                direction[local]
                    * if scales[column] == 0. {
                        value
                    } else {
                        value / scales[column]
                    }
            }));
            scores[column] = scores[column].hypot(projection);
        }
        for channel in 0..design.layout().effects[effect].channels {
            raw[column] = raw[column].max(raw_moment(
                design, effect, channel, members, residuals, column,
            )?);
        }
        if !scores[column].is_finite() {
            return Err(numerical("FE normalized score is not finite"));
        }
    }
    Ok((scores, raw))
}
fn raw_moment(
    design: &Design<'_>,
    effect: usize,
    channel: usize,
    members: &[usize],
    residuals: &DenseMatrix,
    column: usize,
) -> Result<f64> {
    let mut left_scale = 0_f64;
    let mut right_scale = 0_f64;
    for &row in members {
        let root = design.weights().at(row).sqrt();
        left_scale = left_scale.max((root * design.basis_value(effect, channel, row)?).abs());
        right_scale = right_scale.max((root * residuals[(row, column)]).abs());
    }
    if left_scale == 0. || right_scale == 0. {
        return Ok(0.);
    }
    let normalized = compensated_sum(members.iter().map(|&row| {
        let root = design.weights().at(row).sqrt();
        (root
            * design
                .basis_value(effect, channel, row)
                .expect("validated channel")
            / left_scale)
            * (root * residuals[(row, column)] / right_scale)
    }));
    if normalized == 0. {
        return Ok(0.);
    }
    // Raw units may overflow while the invariant span check remains meaningful.
    let result = (normalized.abs() * left_scale.min(right_scale)) * left_scale.max(right_scale);
    Ok(if result.is_finite() {
        result
    } else {
        f64::INFINITY
    })
}
pub(super) fn reconstruction_errors(
    reference: &DenseMatrix,
    expected: &DenseMatrix,
    actual: &DenseMatrix,
) -> Result<Vec<f64>> {
    let mut errors = zeros(reference.ncols())?;
    for column in 0..reference.ncols() {
        for row in 0..reference.nrows() {
            let scale = reference[(row, column)]
                .abs()
                .max(expected[(row, column)].abs())
                .max(actual[(row, column)].abs());
            let difference = if scale == 0. {
                0.
            } else {
                (expected[(row, column)] / scale - actual[(row, column)] / scale).abs()
            };
            errors[column] = errors[column].max(difference);
        }
    }
    Ok(errors)
}
fn weighted_norms(
    design: &Design<'_>,
    matrix: &DenseMatrix,
    team: &ThreadTeam,
) -> Result<Vec<f64>> {
    let width = matrix.ncols();
    let mut partials = DenseMatrix::zeros(matrix.nrows().div_ceil(REDUCTION_CHUNK), width)?;
    // Hypot retains range safety. Fixed row partitions and an ordered merge
    // keep the arithmetic identical across worker counts, including the tail.
    team.install(|| {
        partials
            .as_mut_slice()
            .par_chunks_mut(width.max(1))
            .enumerate()
            .for_each(|(chunk, norms)| {
                let start = chunk * REDUCTION_CHUNK;
                for row in start..start.saturating_add(REDUCTION_CHUNK).min(matrix.nrows()) {
                    let root = design.weights().at(row).sqrt();
                    for (column, norm) in norms.iter_mut().enumerate() {
                        *norm = norm.hypot(root * matrix[(row, column)]);
                    }
                }
            });
    });
    let mut norms = zeros(matrix.ncols())?;
    for (column, norm) in norms.iter_mut().enumerate() {
        for chunk in 0..partials.nrows() {
            *norm = norm.hypot(partials[(chunk, column)]);
        }
        if !norm.is_finite() {
            return Err(numerical(
                "weighted FE certificate norm is not representable",
            ));
        }
    }
    Ok(norms)
}
pub(super) fn zeros(length: usize) -> Result<Vec<f64>> {
    let mut values = reserved(length)?;
    values.resize(length, 0.);
    Ok(values)
}
pub(super) fn check_tolerance(tolerance: f64) -> Result<()> {
    if !tolerance.is_finite() || tolerance <= 0. || tolerance >= 1. {
        return Err(invalid("invalid FE certificate tolerance"));
    }
    Ok(())
}
fn invalid(message: &str) -> FeregError {
    FeregError::new(ErrorCode::InvalidArgument, message)
}
fn numerical(message: &str) -> FeregError {
    FeregError::new(ErrorCode::NonConvergence, message)
}
